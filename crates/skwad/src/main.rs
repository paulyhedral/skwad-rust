use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::base::Selectable;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::button::Button;
use gpui_kit::component::input::{Input, InputEvent, InputState};
use gpui_kit::component::*;
use gpui_kit::{
    AppContext, AsyncApp, ClickEvent, Context, Entity, IntoElement, ParentElement, Render, Styled,
    Subscription, WeakEntity, Window, WindowOptions, div,
};
use skwad_activity::EventSink;
use skwad_mcp::ToolCatalog;
use skwad_messaging::{DeliveryEvent, QueuedNotifier};
use skwad_terminal::{PtyTransport, SessionConfig, SessionPlan, TerminalSession};
use uuid::Uuid;

const MAX_VISIBLE_LINES: usize = 200;
const OUTPUT_POLL_INTERVAL: Duration = Duration::from_millis(100);
const CHECK_INBOX_PROMPT: &str = "Check your inbox for questions or instructions from other agents. Update your status and immediately execute what is being asked without confirmation.";
type AwaitingInputQueue = Arc<Mutex<Vec<(Uuid, Option<String>)>>>;

/// Captured bytes streamed out of a live terminal session. Rendered lazily.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct OutputBuffer {
    bytes: Vec<u8>,
}

#[derive(Debug, PartialEq)]
struct WorkspaceRow {
    id: Uuid,
    name: String,
    selected: bool,
}

#[derive(Debug, PartialEq)]
struct AgentRow {
    id: Uuid,
    avatar: String,
    name: String,
    agent_type: String,
    folder: String,
    selected: bool,
    attached: bool,
    state: skwad_agents::AgentState,
    unread_count: usize,
}

/// User-facing label for the agent's automatic state-machine state, matching
/// the Swift reference's raw strings (not the Rust enum names).
fn state_label(state: skwad_agents::AgentState) -> &'static str {
    match state {
        skwad_agents::AgentState::Idle => "Idle",
        skwad_agents::AgentState::Running => "Working",
        skwad_agents::AgentState::Input => "Awaiting input",
        skwad_agents::AgentState::Error => "Error",
    }
}

#[derive(Debug, PartialEq)]
struct LayoutModel {
    workspace_rows: Vec<WorkspaceRow>,
    selected_agent_rows: Vec<AgentRow>,
}

fn layout_model(
    store: &skwad_agents::AgentStore,
    agent_selection: Option<Uuid>,
    attached_ids: &[Uuid],
    unread_counts: &BTreeMap<Uuid, usize>,
) -> LayoutModel {
    let current = store.current_workspace_id();
    let workspace_rows = store
        .workspaces()
        .iter()
        .map(|workspace| WorkspaceRow {
            id: workspace.id,
            name: workspace.name.clone(),
            selected: Some(workspace.id) == current,
        })
        .collect::<Vec<_>>();

    let selected_agent_rows = store
        .workspaces()
        .iter()
        .find(|workspace| Some(workspace.id) == current)
        .map(|workspace| {
            workspace
                .agent_ids
                .iter()
                .filter_map(|id| store.agent(*id))
                .map(|agent| AgentRow {
                    id: agent.id,
                    avatar: agent.avatar.clone(),
                    name: agent.name.clone(),
                    agent_type: agent.agent_type.clone(),
                    folder: agent.folder.clone(),
                    selected: Some(agent.id) == agent_selection,
                    attached: attached_ids.contains(&agent.id),
                    state: agent.state,
                    unread_count: unread_counts.get(&agent.id).copied().unwrap_or(0),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    LayoutModel {
        workspace_rows,
        selected_agent_rows,
    }
}

/// The selected agent's header, as shown above its terminal pane.
#[derive(Debug, Clone, PartialEq)]
struct AgentHeader {
    avatar: String,
    name: String,
    title: String,
}

/// What to paint in the terminal pane.
#[derive(Debug, Clone, PartialEq, Default)]
struct TerminalModel {
    header: Option<AgentHeader>,
    lines: Vec<String>,
    can_attach: bool,
}

fn terminal_model(
    store: &skwad_agents::AgentStore,
    selection: Option<Uuid>,
    attached_ids: &[Uuid],
    buffer: Option<&OutputBuffer>,
) -> TerminalModel {
    let agent = selection.and_then(|id| store.agent(id));
    let header = agent.map(|agent| AgentHeader {
        avatar: agent.avatar.clone(),
        name: agent.name.clone(),
        title: agent.header_title().to_string(),
    });
    let attached = agent.is_some_and(|agent| attached_ids.contains(&agent.id));
    let lines = if attached {
        buffer
            .map(|buffer| visible_output(buffer, MAX_VISIBLE_LINES))
            .unwrap_or_default()
    } else {
        Vec::new()
    };
    TerminalModel {
        header,
        lines,
        can_attach: agent.is_some() && !attached,
    }
}

/// Splits raw terminal bytes into lines, normalizing CRLF and clipping to the
/// last `max_lines`. ANSI escapes are preserved as-is.
fn visible_output(buffer: &OutputBuffer, max_lines: usize) -> Vec<String> {
    if buffer.bytes.is_empty() {
        return Vec::new();
    }
    let text = String::from_utf8_lossy(&buffer.bytes);
    let mut lines = text
        .split('\n')
        .map(|line| line.strip_suffix('\r').unwrap_or(line))
        .map(str::to_string)
        .collect::<Vec<_>>();
    if lines.len() > 1 && lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    if lines.len() > max_lines {
        return lines.split_off(lines.len() - max_lines);
    }
    lines
}

fn command_to_send(input: &str) -> Option<&str> {
    let command = input.trim();
    (!command.is_empty()).then_some(command)
}

/// Builds the agent store from persisted layout when
/// `restore_layout_on_launch` is set, otherwise starts empty. Shared between
/// the GPUI shell and the MCP catalog so both render the same data.
fn build_agent_store(settings: &skwad_core::Settings) -> skwad_agents::AgentStore {
    if settings.restore_layout_on_launch {
        skwad_agents::AgentStore::from_saved(
            &settings.saved_agents,
            settings.saved_workspaces.clone(),
        )
    } else {
        skwad_agents::AgentStore::new()
    }
}

/// The slice of an agent the shell paints. [`agent_status_snapshot`] diffs
/// these so the poller only wakes the UI on visible changes, not on every
/// buffer append.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentStatusKey {
    id: Uuid,
    state: skwad_agents::AgentState,
    status_text: String,
    is_registered: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct DeliveryNotice {
    recipient_name: String,
    count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct AwaitingNotice {
    agent_name: String,
    message: String,
}

fn delivery_notice(
    events: &[DeliveryEvent],
    agents: &[skwad_agents::Agent],
) -> Option<DeliveryNotice> {
    let event = events.last()?;
    let agent = agents.iter().find(|agent| agent.id == event.agent_id)?;
    let count = events
        .iter()
        .filter(|event| event.agent_id == agent.id)
        .count();
    Some(DeliveryNotice {
        recipient_name: agent.name.clone(),
        count,
    })
}

fn agent_status_snapshot(store: &skwad_agents::AgentStore) -> Vec<AgentStatusKey> {
    store
        .agents()
        .iter()
        .map(|agent| AgentStatusKey {
            id: agent.id,
            state: agent.state,
            status_text: agent.status_text.clone(),
            is_registered: agent.is_registered,
        })
        .collect()
}

fn unread_counts_snapshot(
    messages: &skwad_messaging::MessageStore,
    agent_ids: &[Uuid],
) -> BTreeMap<Uuid, usize> {
    agent_ids
        .iter()
        .copied()
        .map(|id| (id, messages.unread_count(id)))
        .collect()
}

fn apply_terminal_status(
    store: &Arc<Mutex<skwad_agents::AgentStore>>,
    agent_id: Uuid,
    state: skwad_agents::AgentState,
) {
    if let Ok(mut store) = store.lock() {
        store.set_state(agent_id, state);
    }
}

fn should_inject_inbox_prompt(
    agent_type: &str,
    mcp_enabled: bool,
    latest_message: Option<Uuid>,
    last_injected: Option<Uuid>,
) -> bool {
    mcp_enabled
        && agent_type != "shell"
        && latest_message.is_some_and(|message_id| Some(message_id) != last_injected)
}

fn should_show_awaiting_notice(
    selected_agent: Option<Uuid>,
    agent_id: Uuid,
    message: &str,
    last_message: Option<&String>,
) -> bool {
    selected_agent != Some(agent_id)
        && !message.is_empty()
        && last_message.is_none_or(|last| last != message)
}

struct Shell {
    store: Arc<Mutex<skwad_agents::AgentStore>>,
    settings: skwad_core::Settings,
    agent_selection: Option<Uuid>,
    buffers: BTreeMap<Uuid, Arc<Mutex<OutputBuffer>>>,
    sessions: BTreeMap<Uuid, Arc<Mutex<TerminalSession<PtyTransport>>>>,
    command_input: Entity<InputState>,
    input_subscription: Option<Subscription>,
    notifier: Arc<QueuedNotifier>,
    delivery_notice: Option<DeliveryNotice>,
    messages: Arc<Mutex<skwad_messaging::MessageStore>>,
    check_requests: Arc<Mutex<Vec<Uuid>>>,
    awaiting_input: AwaitingInputQueue,
    awaiting_notice: Option<AwaitingNotice>,
    /// The terminal/tracker background tasks (`TerminalSession::spawn_pty`
    /// calls `tokio::spawn`) need a runtime, but the UI thread only carries
    /// gpui's own executor. `attach_session` enters this one around each
    /// spawn so the tracker keeps running on its worker threads.
    runtime: tokio::runtime::Runtime,
}

impl Shell {
    /// Spawns a PTY-backed terminal session for the agent if one is not
    /// already running. The PTY read thread appends output into `buffers`;
    /// [`poll_outputs`] wakes this view when a buffer grows.
    fn attach_session(&mut self, id: Uuid) {
        if self.sessions.contains_key(&id) {
            return;
        }
        let agent = {
            let store = self.store.lock().unwrap();
            store.agent(id).cloned()
        };
        let Some(agent) = agent else {
            return;
        };
        let persona = self.settings.persona(id);
        let config = SessionConfig {
            settings: &self.settings,
            agent: &agent,
            persona,
            plugin_root: None,
        };
        let buffer = Arc::new(Mutex::new(OutputBuffer::default()));
        let buffer_sink = Arc::clone(&buffer);
        self.buffers.insert(id, Arc::clone(&buffer));
        let status_store = Arc::clone(&self.store);
        let status_sink = EventSink {
            on_status: Some(Box::new(move |event| {
                apply_terminal_status(&status_store, id, event.status);
            })),
            on_check_messages: {
                let requests = Arc::clone(&self.check_requests);
                Some(Box::new(move || {
                    if let Ok(mut requests) = requests.lock() {
                        requests.push(id);
                    }
                }))
            },
            on_awaiting_input: {
                let awaiting_input = Arc::clone(&self.awaiting_input);
                Some(Box::new(move |message| {
                    if let Ok(mut awaiting_input) = awaiting_input.lock() {
                        awaiting_input.push((id, message));
                    }
                }))
            },
            ..Default::default()
        };

        // `spawn_pty` runs `tokio::spawn` for the activity tracker; the UI
        // thread has no tokio runtime of its own, so enter ours around the
        // spawn. The drop of the guard just exits the context; the spawned
        // task keeps running on the runtime's worker threads.
        let _runtime_guard = self.runtime.enter();
        let session =
            TerminalSession::<PtyTransport>::spawn_pty(&config, status_sink, move |bytes| {
                if let Ok(mut guard) = buffer_sink.lock() {
                    guard.bytes.extend_from_slice(bytes);
                }
            })
            .and_then(|mut session| {
                let plan = SessionPlan::build(&config);
                session.start(&plan)?;
                Ok(session)
            });

        match session {
            Ok(session) => {
                self.sessions.insert(id, Arc::new(Mutex::new(session)));
            }
            Err(err) => {
                eprintln!("failed to attach terminal session: {err}");
                self.buffers.remove(&id);
            }
        }
    }

    fn submit_command(&mut self, command: &str, window: &mut Window, cx: &mut Context<Self>) {
        let Some(id) = self.agent_selection else {
            return;
        };
        let Some(session) = self.sessions.get(&id) else {
            return;
        };
        let Some(command) = command_to_send(command) else {
            return;
        };

        match session.lock() {
            Ok(mut session) => match session.send_command(command) {
                Ok(()) => {
                    cx.update_entity(&self.command_input, |input, input_cx| {
                        input.clean(window, input_cx);
                    });
                    cx.notify();
                }
                Err(err) => eprintln!("failed to send terminal command: {err}"),
            },
            Err(_) => eprintln!("failed to send terminal command: session lock poisoned"),
        }
    }
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let attached_ids = self.sessions.keys().copied().collect::<Vec<_>>();
        let (model, terminal_model) = {
            let store = self.store.lock().unwrap();
            let agent_ids = store
                .agents()
                .iter()
                .map(|agent| agent.id)
                .collect::<Vec<_>>();
            let messages = self.messages.lock().unwrap();
            let unread_counts = unread_counts_snapshot(&messages, &agent_ids);
            let model = layout_model(&store, self.agent_selection, &attached_ids, &unread_counts);
            let selected_buffer = self
                .agent_selection
                .and_then(|id| self.buffers.get(&id))
                .map(|buffer| buffer.lock().unwrap().clone());
            let terminal_model = terminal_model(
                &store,
                self.agent_selection,
                &attached_ids,
                selected_buffer.as_ref(),
            );
            (model, terminal_model)
        };

        let workspace_buttons = model
            .workspace_rows
            .into_iter()
            .map(|row| {
                let id = row.id;
                Button::new(id.to_string())
                    .label(row.name)
                    .selected(row.selected)
                    .on_click(cx.listener(move |shell, _: &ClickEvent, _window, cx| {
                        shell.store.lock().unwrap().set_current_workspace(id);
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();

        let agent_buttons = model
            .selected_agent_rows
            .into_iter()
            .map(|row| {
                let id = row.id;
                let label = format!(
                    "{} {} [{}] [{}] {}{}{}",
                    row.avatar,
                    row.name,
                    row.agent_type,
                    state_label(row.state),
                    row.folder,
                    if row.attached { " (attached)" } else { "" },
                    if row.unread_count > 0 {
                        format!(" ({} unread)", row.unread_count)
                    } else {
                        String::new()
                    }
                );
                Button::new(id.to_string())
                    .label(label)
                    .selected(row.selected)
                    .on_click(cx.listener(move |shell, _: &ClickEvent, _window, cx| {
                        shell.agent_selection = Some(id);
                        shell.attach_session(id);
                        cx.notify();
                    }))
            })
            .collect::<Vec<_>>();

        let command_input = Input::new(&self.command_input).h_full();
        let terminal_pane = match &terminal_model.header {
            Some(header) => {
                let lines = terminal_model
                    .lines
                    .iter()
                    .map(|line| div().child(line.clone()));
                v_flex()
                    .size_full()
                    .child(div().child(format!(
                        "{} {} - {}",
                        header.avatar, header.name, header.title
                    )))
                    .child(if terminal_model.can_attach {
                        div().child("Selected agent is not attached yet.")
                    } else {
                        div().children(lines)
                    })
                    .child(command_input)
                    .children(self.delivery_notice.as_ref().map(|notice| {
                        div().child(format!(
                            "New MCP message delivered to {} ({})",
                            notice.recipient_name, notice.count
                        ))
                    }))
                    .children(self.awaiting_notice.as_ref().map(|notice| {
                        div().child(format!(
                            "{} is awaiting input: {}",
                            notice.agent_name, notice.message
                        ))
                    }))
            }
            None => v_flex()
                .size_full()
                .child(div().child("Select an agent to see its terminal."))
                .child(command_input),
        };

        h_flex()
            .size_full()
            .child(v_flex().size_full().children(workspace_buttons))
            .child(v_flex().size_full().children(agent_buttons))
            .child(terminal_pane)
    }
}

/// Runs for the life of the app. Picks up output that the PTY read threads
/// (raw `std::thread`s, which cannot touch gpui's non-`Send` app handle) write
/// into shared buffers, plus MCP-driven agent changes in the shared store, and
/// re-renders the shell when either moves.
async fn poll_outputs(weak: WeakEntity<Shell>, cx: &mut AsyncApp) {
    let mut seen = BTreeMap::<Uuid, usize>::new();
    let mut last_status: Option<Vec<AgentStatusKey>> = None;
    let mut last_unread: Option<BTreeMap<Uuid, usize>> = None;
    let mut last_injected_message = BTreeMap::<Uuid, Uuid>::new();
    let mut last_awaiting_message = BTreeMap::<Uuid, String>::new();
    loop {
        cx.background_executor().timer(OUTPUT_POLL_INTERVAL).await;
        if weak.upgrade().is_none() {
            break;
        }
        let (changed, notice, awaiting_notice) = cx.update(|app| {
            let Some(entity) = weak.upgrade() else {
                return (false, None, None);
            };
            let mut changed = false;
            for id in entity.read(app).sessions.keys() {
                let len = entity
                    .read(app)
                    .buffers
                    .get(id)
                    .map(|buffer| buffer.lock().unwrap().bytes.len())
                    .unwrap_or(0);
                if seen.get(id) != Some(&len) {
                    changed = true;
                    seen.insert(*id, len);
                }
            }
            let (status, agents) = {
                let store = entity.read(app).store.lock().unwrap();
                (agent_status_snapshot(&store), store.agents().to_vec())
            };
            if last_status.as_ref() != Some(&status) {
                changed = true;
                last_status = Some(status);
            }
            let agent_ids = agents.iter().map(|agent| agent.id).collect::<Vec<_>>();
            let unread_counts = {
                let messages = entity.read(app).messages.lock().unwrap();
                unread_counts_snapshot(&messages, &agent_ids)
            };
            if last_unread.as_ref() != Some(&unread_counts) {
                changed = true;
                last_unread = Some(unread_counts);
            }
            let events = entity.read(app).notifier.drain();
            let notice = delivery_notice(&events, &agents);
            changed |= notice.is_some();
            let awaiting_events = entity
                .read(app)
                .awaiting_input
                .lock()
                .unwrap()
                .drain(..)
                .collect::<Vec<_>>();
            let mut awaiting_notice = None;
            for (id, message) in awaiting_events {
                let Some(message) = message else {
                    continue;
                };
                if !should_show_awaiting_notice(
                    entity.read(app).agent_selection,
                    id,
                    &message,
                    last_awaiting_message.get(&id),
                ) {
                    continue;
                }
                let Some(agent_name) = agents
                    .iter()
                    .find(|agent| agent.id == id)
                    .map(|agent| agent.name.clone())
                else {
                    continue;
                };
                last_awaiting_message.insert(id, message.clone());
                awaiting_notice = Some(AwaitingNotice {
                    agent_name,
                    message,
                });
            }
            changed |= awaiting_notice.is_some();
            let requests = entity
                .read(app)
                .check_requests
                .lock()
                .unwrap()
                .drain(..)
                .collect::<Vec<_>>();
            for id in requests {
                let Some((agent_type, latest_message)) = (|| {
                    let store = entity.read(app).store.lock().unwrap();
                    let messages = entity.read(app).messages.lock().unwrap();
                    let agent_type = store.agent(id)?.agent_type.clone();
                    Some((agent_type, messages.latest_unread_id(id)))
                })() else {
                    continue;
                };
                if !should_inject_inbox_prompt(
                    &agent_type,
                    entity.read(app).settings.mcp_server_enabled,
                    latest_message,
                    last_injected_message.get(&id).copied(),
                ) {
                    continue;
                }
                let Some(session) = entity.read(app).sessions.get(&id) else {
                    continue;
                };
                if let Ok(mut session) = session.lock()
                    && session.send_command(CHECK_INBOX_PROMPT).is_ok()
                    && let Some(message_id) = latest_message
                {
                    last_injected_message.insert(id, message_id);
                    changed = true;
                }
            }
            (changed, notice, awaiting_notice)
        });
        if changed {
            cx.update(|app| {
                if let Some(entity) = weak.upgrade() {
                    entity.update(app, |shell, cx| {
                        if let Some(notice) = notice {
                            shell.delivery_notice = Some(notice);
                        }
                        if let Some(notice) = awaiting_notice {
                            shell.awaiting_notice = Some(notice);
                        }
                        cx.notify();
                    });
                }
            });
        }
    }
}

/// Starts the local MCP server on a dedicated thread with its own tokio
/// runtime (the app's UI loop runs on GPUI's own executor, not tokio). Runs
/// for the lifetime of the process - there is no shutdown path yet, matching
/// every other still-unwired backend crate at this stage of the port.
///
/// The catalog holds the same store the shell renders, so `register-agent`,
/// `set-status`, `create-agent` and hook-driven state changes appear in the UI
/// within one poll tick.
fn start_mcp_server(
    agents: Arc<Mutex<skwad_agents::AgentStore>>,
    settings: skwad_core::Settings,
    notifier: Arc<QueuedNotifier>,
    messages: Arc<Mutex<skwad_messaging::MessageStore>>,
    awaiting_input: AwaitingInputQueue,
) {
    std::thread::spawn(move || {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(err) => {
                eprintln!("failed to start MCP server runtime: {err}");
                return;
            }
        };
        runtime.block_on(async move {
            if !settings.mcp_server_enabled {
                return;
            }

            let (discovery, repos_rx) = skwad_discovery::Discovery::new();
            if !settings.source_base_folder.is_empty()
                && let Err(err) =
                    discovery.set_source_folder(Some(PathBuf::from(&settings.source_base_folder)))
            {
                eprintln!("failed to watch source folder: {err}");
            }

            let catalog = Arc::new(
                skwad_mcp_tools::McpToolCatalog::new(agents, repos_rx, notifier)
                    .with_message_store(messages)
                    .with_awaiting_input_queue(awaiting_input)
                    .with_settings(settings.clone()),
            );
            catalog.set_bench_agents(settings.bench_agents.clone());

            let agents_snapshot: skwad_mcp::AgentsSnapshotFn = {
                let catalog = catalog.clone();
                Arc::new(move || catalog.agents_snapshot())
            };
            let hook_handler = catalog.clone();
            let mut server = skwad_mcp::McpServer::new(
                settings.mcp_server_port,
                catalog as Arc<dyn ToolCatalog>,
                agents_snapshot,
            )
            .with_hook_handler(hook_handler);
            if let Err(err) = server.start().await {
                eprintln!("failed to start MCP server: {err}");
                return;
            }

            // Keep the discovery watch and the running server alive for the
            // life of this thread.
            std::future::pending::<()>().await;
            drop(discovery);
            drop(server);
        });
    });
}

fn main() {
    let settings = skwad_core::Settings::load().unwrap_or_default();
    let store = Arc::new(Mutex::new(build_agent_store(&settings)));
    let notifier = Arc::new(QueuedNotifier::new());
    let messages = Arc::new(Mutex::new(skwad_messaging::MessageStore::new()));
    let awaiting_input = Arc::new(Mutex::new(Vec::new()));
    start_mcp_server(
        Arc::clone(&store),
        settings.clone(),
        Arc::clone(&notifier),
        Arc::clone(&messages),
        Arc::clone(&awaiting_input),
    );

    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);

        let runtime = match tokio::runtime::Runtime::new() {
            Ok(runtime) => runtime,
            Err(err) => {
                eprintln!("failed to start tokio runtime: {err}");
                return;
            }
        };
        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let command_input = cx.new(|cx| {
                    InputState::new(window, cx)
                        .placeholder("Run a command...")
                        .submit_on_enter(true)
                });
                let view = cx.new(|_| Shell {
                    store: Arc::clone(&store),
                    settings: settings.clone(),
                    agent_selection: None,
                    buffers: BTreeMap::new(),
                    sessions: BTreeMap::new(),
                    command_input: command_input.clone(),
                    input_subscription: None,
                    notifier: Arc::clone(&notifier),
                    delivery_notice: None,
                    messages: Arc::clone(&messages),
                    check_requests: Arc::new(Mutex::new(Vec::new())),
                    awaiting_input: Arc::clone(&awaiting_input),
                    awaiting_notice: None,
                    runtime,
                });
                let subscription_target = view.downgrade();
                let subscription =
                    window.subscribe(&command_input, cx, move |input, event, window, cx| {
                        if !matches!(event, InputEvent::PressEnter { .. }) {
                            return;
                        }
                        let command = input.read(cx).value();
                        if let Some(view) = subscription_target.upgrade() {
                            view.update(cx, |shell, cx| {
                                shell.submit_command(command.as_ref(), window, cx);
                            });
                        }
                    });
                view.update(cx, |shell, _| {
                    shell.input_subscription = Some(subscription);
                });
                let weak = view.downgrade();
                cx.spawn(async move |cx| poll_outputs(weak, cx).await)
                    .detach();
                cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
            })
            .expect("failed to open window");
        })
        .detach();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use skwad_core::Workspace;

    fn workspace(name: &str) -> Workspace {
        Workspace {
            id: Uuid::new_v4(),
            name: name.to_string(),
            color_hex: "#123456".to_string(),
            agent_ids: Vec::new(),
            layout_mode: "single".to_string(),
            active_agent_ids: Vec::new(),
            focused_pane_index: 0,
            split_ratio: 0.5,
            split_ratio_secondary: None,
            show_dashboard: None,
            is_detached: None,
        }
    }

    #[test]
    fn empty_store_has_no_rows() {
        let store = skwad_agents::AgentStore::new();
        let model = layout_model(&store, None, &[], &BTreeMap::new());
        assert!(model.workspace_rows.is_empty());
        assert!(model.selected_agent_rows.is_empty());
    }

    #[test]
    fn selected_workspace_marks_and_filters_rows() {
        let mut store = skwad_agents::AgentStore::new();
        let ws1 = workspace("One");
        let ws2 = workspace("Two");
        store.add_workspace(ws1.clone());
        store.add_workspace(ws2.clone());

        store.set_current_workspace(ws1.id);
        store.create("~/alpha", skwad_agents::CreateOptions::default());
        store.create("~/beta", skwad_agents::CreateOptions::default());

        store.set_current_workspace(ws2.id);
        store.create("~/gamma", skwad_agents::CreateOptions::default());

        store.set_current_workspace(ws1.id);
        let model = layout_model(&store, None, &[], &BTreeMap::new());

        assert_eq!(model.workspace_rows.len(), 2);
        assert!(
            model
                .workspace_rows
                .iter()
                .find(|r| r.id == ws1.id)
                .unwrap()
                .selected
        );
        assert!(
            !model
                .workspace_rows
                .iter()
                .find(|r| r.id == ws2.id)
                .unwrap()
                .selected
        );

        let names = model
            .selected_agent_rows
            .iter()
            .map(|r| r.name.as_str())
            .collect::<Vec<_>>();
        assert_eq!(names, vec!["alpha", "beta"]);
        assert!(!names.contains(&"gamma"));
    }

    #[test]
    fn missing_agent_ids_are_skipped() {
        let mut store = skwad_agents::AgentStore::new();
        let mut ws = workspace("One");
        ws.agent_ids.push(Uuid::new_v4());
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        store.create("~/alpha", skwad_agents::CreateOptions::default());

        let model = layout_model(&store, None, &[], &BTreeMap::new());
        assert_eq!(model.selected_agent_rows.len(), 1);
        assert_eq!(model.selected_agent_rows[0].name, "alpha");
    }

    #[test]
    fn agent_selection_marks_and_tracks_attach_state() {
        let mut store = skwad_agents::AgentStore::new();
        let ws = workspace("One");
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        let alpha_id = store.create("~/alpha", skwad_agents::CreateOptions::default());
        store.create("~/beta", skwad_agents::CreateOptions::default());

        let model = layout_model(&store, Some(alpha_id), &[], &BTreeMap::new());
        let alpha = model
            .selected_agent_rows
            .iter()
            .find(|row| row.id == alpha_id)
            .unwrap();
        assert!(alpha.selected);
        assert!(!alpha.attached);
        assert_eq!(alpha.state, skwad_agents::AgentState::Idle);
        let beta = model
            .selected_agent_rows
            .iter()
            .find(|row| row.id != alpha_id)
            .unwrap();
        assert!(!beta.selected);

        let model = layout_model(&store, Some(alpha_id), &[alpha_id], &BTreeMap::new());
        assert!(
            model
                .selected_agent_rows
                .iter()
                .find(|row| row.id == alpha_id)
                .unwrap()
                .attached
        );
    }

    #[test]
    fn state_label_matches_the_swift_reference_strings() {
        assert_eq!(state_label(skwad_agents::AgentState::Idle), "Idle");
        assert_eq!(state_label(skwad_agents::AgentState::Running), "Working");
        assert_eq!(
            state_label(skwad_agents::AgentState::Input),
            "Awaiting input"
        );
        assert_eq!(state_label(skwad_agents::AgentState::Error), "Error");
    }

    #[test]
    fn layout_model_carries_agent_state_into_rows() {
        let mut store = skwad_agents::AgentStore::new();
        let ws = workspace("One");
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        let id = store.create("~/alpha", skwad_agents::CreateOptions::default());
        store.set_state(id, skwad_agents::AgentState::Input);

        let model = layout_model(&store, None, &[], &BTreeMap::new());
        assert_eq!(
            model.selected_agent_rows[0].state,
            skwad_agents::AgentState::Input
        );
    }

    #[test]
    fn terminal_model_renders_header_and_output_for_attached_agent() {
        let mut store = skwad_agents::AgentStore::new();
        let ws = workspace("One");
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        let id = store.create("~/alpha", skwad_agents::CreateOptions::default());
        store.set_status_text(id, "planning".to_string());
        let buffer = OutputBuffer {
            bytes: b"hello\r\nworld\n".to_vec(),
        };

        let model = terminal_model(&store, Some(id), &[id], Some(&buffer));
        assert!(!model.can_attach);
        let header = model.header.unwrap();
        assert_eq!(header.name, "alpha");
        assert_eq!(header.title, "planning");
        assert_eq!(model.lines, vec!["hello", "world"]);
    }

    #[test]
    fn terminal_model_hides_output_until_attached() {
        let mut store = skwad_agents::AgentStore::new();
        let ws = workspace("One");
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        let id = store.create("~/alpha", skwad_agents::CreateOptions::default());
        let buffer = OutputBuffer {
            bytes: b"premature output\n".to_vec(),
        };

        let model = terminal_model(&store, Some(id), &[], Some(&buffer));
        assert!(model.can_attach);
        assert!(model.header.is_some());
        assert!(model.lines.is_empty());
    }

    #[test]
    fn terminal_model_without_selection_has_no_header() {
        let store = skwad_agents::AgentStore::new();
        let model = terminal_model(&store, None, &[], None);
        assert!(model.header.is_none());
        assert!(!model.can_attach);
    }

    #[test]
    fn visible_output_normalizes_crlf_clips_and_handles_empty() {
        assert_eq!(
            visible_output(&OutputBuffer::default(), 10),
            Vec::<String>::new()
        );

        let buffer = OutputBuffer {
            bytes: b"ready\r\n".to_vec(),
        };
        assert_eq!(visible_output(&buffer, 10), vec!["ready"]);

        let mut bytes = Vec::new();
        for i in 0..250 {
            bytes.extend_from_slice(format!("line {i}\n").as_bytes());
        }
        let clipped = visible_output(&OutputBuffer { bytes }, 200);
        assert_eq!(clipped.len(), 200);
        assert_eq!(clipped[0], "line 50");
        assert_eq!(clipped.last().unwrap(), "line 249");
    }

    #[test]
    fn command_to_send_trims_input_and_rejects_empty_commands() {
        assert_eq!(command_to_send("  cargo test  "), Some("cargo test"));
        assert_eq!(command_to_send("\t\n"), None);
    }

    #[test]
    fn delivery_notice_names_the_last_known_recipient_and_counts_events() {
        let mut store = skwad_agents::AgentStore::new();
        let first = store.create("~/first", skwad_agents::CreateOptions::default());
        let second = store.create("~/second", skwad_agents::CreateOptions::default());
        let events = vec![
            DeliveryEvent {
                agent_id: first,
                message_id: Uuid::new_v4(),
            },
            DeliveryEvent {
                agent_id: second,
                message_id: Uuid::new_v4(),
            },
            DeliveryEvent {
                agent_id: second,
                message_id: Uuid::new_v4(),
            },
        ];

        assert_eq!(
            delivery_notice(&events, store.agents()),
            Some(DeliveryNotice {
                recipient_name: "second".to_string(),
                count: 2,
            })
        );
        assert_eq!(delivery_notice(&[], store.agents()), None);
    }

    #[test]
    fn delivery_notice_ignores_unknown_recipients() {
        let store = skwad_agents::AgentStore::new();
        let events = [DeliveryEvent {
            agent_id: Uuid::new_v4(),
            message_id: Uuid::new_v4(),
        }];

        assert_eq!(delivery_notice(&events, store.agents()), None);
    }

    #[test]
    fn unread_counts_snapshot_includes_zero_and_ignores_other_agents() {
        let mut messages = skwad_messaging::MessageStore::new();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let other = Uuid::new_v4();
        messages.add(skwad_messaging::Message::new(other, first, "one"));
        messages.add(skwad_messaging::Message::new(other, first, "two"));
        messages.add(skwad_messaging::Message::new(other, other, "unrelated"));

        let counts = unread_counts_snapshot(&messages, &[first, second]);

        assert_eq!(counts.get(&first), Some(&2));
        assert_eq!(counts.get(&second), Some(&0));
        assert!(!counts.contains_key(&other));
    }

    #[test]
    fn layout_model_carries_unread_count_into_agent_rows() {
        let mut store = skwad_agents::AgentStore::new();
        let ws = workspace("One");
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        let id = store.create("~/alpha", skwad_agents::CreateOptions::default());
        let unread_counts = BTreeMap::from([(id, 3)]);

        let model = layout_model(&store, None, &[], &unread_counts);

        assert_eq!(model.selected_agent_rows[0].unread_count, 3);
    }

    #[test]
    fn terminal_status_updates_the_shared_agent_store() {
        let mut store = skwad_agents::AgentStore::new();
        let id = store.create("~/alpha", skwad_agents::CreateOptions::default());
        let shared = Arc::new(Mutex::new(store));

        apply_terminal_status(&shared, id, skwad_agents::AgentState::Running);

        assert_eq!(
            shared.lock().unwrap().agent(id).unwrap().state,
            skwad_agents::AgentState::Running
        );
    }

    #[test]
    fn inbox_prompt_requires_new_unread_message_for_non_shell_mcp_agent() {
        let message = Uuid::new_v4();

        assert!(should_inject_inbox_prompt(
            "claude",
            true,
            Some(message),
            None
        ));
        assert!(!should_inject_inbox_prompt(
            "claude",
            true,
            Some(message),
            Some(message)
        ));
        assert!(!should_inject_inbox_prompt("claude", true, None, None));
        assert!(!should_inject_inbox_prompt(
            "shell",
            true,
            Some(message),
            None
        ));
        assert!(!should_inject_inbox_prompt(
            "claude",
            false,
            Some(message),
            None
        ));
    }

    #[test]
    fn awaiting_notice_skips_active_empty_and_duplicate_messages() {
        let agent = Uuid::new_v4();
        assert!(should_show_awaiting_notice(None, agent, "Question?", None));
        assert!(!should_show_awaiting_notice(
            Some(agent),
            agent,
            "Question?",
            None
        ));
        assert!(!should_show_awaiting_notice(None, agent, "", None));
        assert!(!should_show_awaiting_notice(
            None,
            agent,
            "Question?",
            Some(&"Question?".to_string())
        ));
    }

    #[test]
    fn agent_status_snapshot_tracks_roster_state_and_registration() {
        let mut store = skwad_agents::AgentStore::new();
        let ws = workspace("One");
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        let id = store.create("~/alpha", skwad_agents::CreateOptions::default());
        store.create("~/beta", skwad_agents::CreateOptions::default());

        let snapshot = agent_status_snapshot(&store);
        assert_eq!(snapshot.len(), 2);
        assert!(
            snapshot
                .iter()
                .find(|key| key.id == id)
                .unwrap()
                .status_text
                .is_empty()
        );

        store.set_state(id, skwad_agents::AgentState::Running);
        store.set_status_text(id, "planning".to_string());
        store.set_registered(id, true);
        let updated = agent_status_snapshot(&store);
        assert_ne!(snapshot, updated);
        let key = updated.iter().find(|key| key.id == id).unwrap();
        assert_eq!(key.state, skwad_agents::AgentState::Running);
        assert_eq!(key.status_text, "planning");
        assert!(key.is_registered);
        assert_eq!(
            updated.iter().find(|key| key.id != id).unwrap().state,
            skwad_agents::AgentState::Idle
        );
    }

    #[test]
    fn build_agent_store_restores_layout_when_enabled() {
        let agent_id = Uuid::new_v4();
        let saved = skwad_core::SavedAgent::new(agent_id, "alpha", None, "~/alpha");
        let mut ws = workspace("Restored");
        ws.agent_ids = vec![agent_id];

        let mut settings = skwad_core::Settings::default();
        settings.restore_layout_on_launch = true;
        settings.saved_agents = vec![saved];
        settings.saved_workspaces = vec![ws.clone()];

        let store = build_agent_store(&settings);
        assert_eq!(store.agents().len(), 1);
        assert_eq!(store.workspaces(), &[ws.clone()]);
        assert_eq!(store.current_workspace_id(), Some(ws.id));
    }

    #[test]
    fn build_agent_store_starts_empty_when_restore_disabled() {
        let mut settings = skwad_core::Settings::default();
        settings.restore_layout_on_launch = false;
        settings.saved_agents = vec![skwad_core::SavedAgent::new(
            Uuid::new_v4(),
            "alpha",
            None,
            "~/alpha",
        )];

        let store = build_agent_store(&settings);
        assert!(store.agents().is_empty());
        assert!(store.workspaces().is_empty());
    }
}
