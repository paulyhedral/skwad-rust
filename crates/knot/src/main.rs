#![allow(dead_code)]

use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::base::Selectable;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::input::{Input, InputState};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::*;
use gpui_kit::{
    App, AppContext, AsyncApp, ClickEvent, Context, Entity, InteractiveElement, IntoElement, Menu,
    MenuItem, ParentElement, PathPromptOptions, Render, StatefulInteractiveElement, Styled,
    Subscription, SystemMenuType, SystemNotification, SystemNotificationResponse, WeakEntity,
    Window, WindowBounds, WindowOptions, actions, div, px, size,
};
use knot_activity::EventSink;
use knot_mcp::ToolCatalog;
use knot_messaging::{DeliveryEvent, QueuedNotifier};
use knot_terminal::{PtyTransport, SessionConfig, SessionPlan, TerminalSession};
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
    state: knot_agents::AgentState,
    unread_count: usize,
}

/// User-facing label for the agent's automatic state-machine state, matching
/// the Swift reference's raw strings (not the Rust enum names).
fn state_label(state: knot_agents::AgentState) -> &'static str {
    match state {
        knot_agents::AgentState::Idle => "Idle",
        knot_agents::AgentState::Running => "Working",
        knot_agents::AgentState::Input => "Awaiting input",
        knot_agents::AgentState::Error => "Error",
    }
}

#[derive(Debug, PartialEq)]
struct LayoutModel {
    workspace_rows: Vec<WorkspaceRow>,
    selected_agent_rows: Vec<AgentRow>,
}

fn layout_model(
    store: &knot_agents::AgentStore,
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
    store: &knot_agents::AgentStore,
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

fn stale_session_ids(session_ids: &[Uuid], live_ids: &BTreeSet<Uuid>) -> Vec<Uuid> {
    session_ids
        .iter()
        .copied()
        .filter(|id| !live_ids.contains(id))
        .collect()
}

/// Builds the agent store from persisted layout when
/// `restore_layout_on_launch` is set, otherwise starts empty. Shared between
/// the GPUI shell and the MCP catalog so both render the same data.
///
/// When `restore_conversation_on_launch` is also set, resolves each restored
/// agent's resume-session id: its own persisted session id when present (an
/// exact restore), otherwise the most recent session for its `(folder,
/// agent type)` via the `knot-history` provider registry.
fn build_agent_store(settings: &knot_core::Settings) -> knot_agents::AgentStore {
    if !settings.restore_layout_on_launch {
        return knot_agents::AgentStore::new();
    }

    let mut store = knot_agents::AgentStore::from_saved(
        &settings.saved_agents,
        settings.saved_workspaces.clone(),
    );

    if settings.restore_conversation_on_launch {
        let persisted: BTreeMap<Uuid, String> = settings
            .saved_agents
            .iter()
            .filter_map(|agent| agent.session_id.clone().map(|sid| (agent.id, sid)))
            .collect();
        store.resolve_resume_sessions(&persisted, |folder, agent_type| {
            let provider = knot_history::provider(agent_type)?;
            provider
                .load_sessions(folder)
                .into_iter()
                .next()
                .map(|session| session.id)
        });
    }

    store
}

fn agent_selection_for_workspace(
    store: &knot_agents::AgentStore,
    workspace_id: Uuid,
) -> Option<Uuid> {
    let workspace = store
        .workspaces()
        .iter()
        .find(|workspace| workspace.id == workspace_id)?;
    workspace
        .active_agent_ids
        .iter()
        .chain(workspace.agent_ids.iter())
        .find(|id| store.agent(**id).is_some())
        .copied()
}

fn initial_agent_selection(store: &knot_agents::AgentStore) -> Option<Uuid> {
    store
        .current_workspace_id()
        .and_then(|id| agent_selection_for_workspace(store, id))
}

/// The slice of an agent the shell paints. [`agent_status_snapshot`] diffs
/// these so the poller only wakes the UI on visible changes, not on every
/// buffer append.
#[derive(Debug, Clone, PartialEq, Eq)]
struct AgentStatusKey {
    id: Uuid,
    state: knot_agents::AgentState,
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
    agents: &[knot_agents::Agent],
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

fn agent_status_snapshot(store: &knot_agents::AgentStore) -> Vec<AgentStatusKey> {
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
    messages: &knot_messaging::MessageStore,
    agent_ids: &[Uuid],
) -> BTreeMap<Uuid, usize> {
    agent_ids
        .iter()
        .copied()
        .map(|id| (id, messages.unread_count(id)))
        .collect()
}

fn apply_terminal_status(
    store: &Arc<Mutex<knot_agents::AgentStore>>,
    agent_id: Uuid,
    state: knot_agents::AgentState,
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

const AWAITING_INPUT_DEFAULT_BODY: &str = "Needs your attention";

/// Whether a desktop notification should be raised for an agent entering
/// Awaiting input, gating the same "is this a fresh prompt for an agent the
/// user isn't already looking at" signal `should_show_awaiting_notice`
/// computes for the in-window toast behind the
/// `desktop_notifications_enabled` setting.
fn should_notify(desktop_notifications_enabled: bool, show_awaiting_notice: bool) -> bool {
    desktop_notifications_enabled && show_awaiting_notice
}

/// The notification body: the hook-supplied message when non-empty,
/// otherwise a default.
fn notification_body(message: &str) -> &str {
    if message.is_empty() {
        AWAITING_INPUT_DEFAULT_BODY
    } else {
        message
    }
}

/// Parses a [`SystemNotificationResponse`]'s tag back into the agent id it
/// was posted for, or `None` if the tag isn't a valid uuid.
///
/// The full "select this exact agent" click-to-navigate parity with the
/// Swift reference needs a registry mapping agent id -> owning workspace
/// window, which doesn't exist yet (each workspace is an independent
/// `Shell` window/entity with its own `agent_selection`, and nothing
/// currently tracks which window owns which agent across windows). Until
/// that exists, a click only raises the app to the front
/// (`App::activate(true)`, same as the existing `ShowAllWindows` action);
/// it doesn't switch the front window's selection to the clicked agent.
fn notification_response_agent_id(response: &SystemNotificationResponse) -> Option<Uuid> {
    Uuid::parse_str(&response.tag).ok()
}

struct Shell {
    store: Arc<Mutex<knot_agents::AgentStore>>,
    settings: knot_core::Settings,
    agent_selection: Option<Uuid>,
    buffers: BTreeMap<Uuid, Arc<Mutex<OutputBuffer>>>,
    sessions: BTreeMap<Uuid, Arc<Mutex<TerminalSession<PtyTransport>>>>,
    command_input: Entity<InputState>,
    input_subscription: Option<Subscription>,
    new_agent_name_input: Entity<InputState>,
    new_agent_folder_input: Entity<InputState>,
    show_new_agent: bool,
    agent_error: Option<String>,
    new_workspace_name_input: Entity<InputState>,
    show_new_workspace: bool,
    editing_workspace_id: Option<Uuid>,
    notifier: Arc<QueuedNotifier>,
    delivery_notice: Option<DeliveryNotice>,
    messages: Arc<Mutex<knot_messaging::MessageStore>>,
    check_requests: Arc<Mutex<Vec<Uuid>>>,
    process_exits: Arc<Mutex<Vec<Uuid>>>,
    awaiting_input: AwaitingInputQueue,
    mcp_stop: Option<tokio::sync::oneshot::Sender<()>>,
    awaiting_notice: Option<AwaitingNotice>,
    /// The terminal/tracker background tasks (`TerminalSession::spawn_pty`
    /// calls `tokio::spawn`) need a runtime, but the UI thread only carries
    /// gpui's own executor. `attach_session` enters this one around each
    /// spawn so the tracker keeps running on its worker threads.
    runtime: tokio::runtime::Runtime,
}

impl Drop for Shell {
    fn drop(&mut self) {
        if let Some(stop) = self.mcp_stop.take() {
            let _ = stop.send(());
        }
        for session in self.sessions.values() {
            if let Ok(mut session) = session.lock() {
                let _ = session.shutdown();
            }
        }
    }
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
        let process_exits = Arc::clone(&self.process_exits);
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
        let session = TerminalSession::<PtyTransport>::spawn_pty_with_exit(
            &config,
            status_sink,
            move |bytes| {
                if let Ok(mut guard) = buffer_sink.lock() {
                    guard.bytes.extend_from_slice(bytes);
                }
            },
            move |_| {
                if let Ok(mut process_exits) = process_exits.lock() {
                    process_exits.push(id);
                }
            },
        )
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

    fn persist_store(&mut self) {
        let Ok(store) = self.store.lock() else {
            self.agent_error = Some("Agent store is unavailable.".to_string());
            return;
        };
        self.settings.saved_agents =
            store.saved_agents(self.settings.restore_conversation_on_launch);
        self.settings.saved_workspaces = store.saved_workspaces();
        if let Err(error) = self.settings.persist() {
            self.agent_error = Some(format!("Could not save agent: {error}"));
        }
    }

    fn create_agent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let folder = self
            .new_agent_folder_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        if folder.is_empty() {
            self.agent_error = Some("Choose a folder for the agent.".to_string());
            cx.notify();
            return;
        }
        let path = PathBuf::from(&folder);
        if !path.is_dir() {
            self.agent_error = Some("The agent folder must be an existing directory.".to_string());
            cx.notify();
            return;
        }

        let name = self
            .new_agent_name_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let id = {
            let mut store = self.store.lock().unwrap();
            store.create(
                folder,
                knot_agents::CreateOptions {
                    name: (!name.is_empty()).then_some(name),
                    ..Default::default()
                },
            )
        };
        self.persist_store();
        self.agent_selection = Some(id);
        self.show_new_agent = false;
        self.agent_error = None;
        cx.update_entity(&self.new_agent_name_input, |input, input_cx| {
            input.clean(window, input_cx);
        });
        cx.update_entity(&self.new_agent_folder_input, |input, input_cx| {
            input.clean(window, input_cx);
        });
        self.attach_session(id);
        cx.notify();
    }

    fn cancel_new_agent(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_new_agent = false;
        self.agent_error = None;
        cx.update_entity(&self.new_agent_name_input, |input, input_cx| {
            input.clean(window, input_cx);
        });
        cx.update_entity(&self.new_agent_folder_input, |input, input_cx| {
            input.clean(window, input_cx);
        });
        cx.notify();
    }

    fn close_agent(&mut self, id: Uuid, cx: &mut Context<Self>) {
        let removed = self.store.lock().unwrap().remove(id);
        if removed.is_empty() {
            return;
        }
        for agent in &removed {
            self.remove_session(agent.id, true);
        }
        if removed
            .iter()
            .any(|agent| Some(agent.id) == self.agent_selection)
        {
            self.agent_selection = {
                let store = self.store.lock().unwrap();
                initial_agent_selection(&store)
            };
            if let Some(selection) = self.agent_selection {
                self.attach_session(selection);
            }
        }
        self.persist_store();
        cx.notify();
    }

    fn restart_agent(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.remove_session(id, true);
        if let Err(error) = self.store.lock().unwrap().restart(id) {
            self.agent_error = Some(format!("Could not restart agent: {error}"));
            cx.notify();
            return;
        }
        self.agent_selection = Some(id);
        self.attach_session(id);
        self.persist_store();
        cx.notify();
    }

    fn attach_agent(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.agent_selection = Some(id);
        self.attach_session(id);
        cx.notify();
    }

    fn open_workspace_manager(&self, cx: &mut Context<Self>) {
        let store = Arc::clone(&self.store);
        let settings = self.settings.clone();
        let options = manager_window_options(cx);
        cx.open_window(options, move |window, cx| {
            let name_input = cx.new(|cx| InputState::new(window, cx).placeholder("Workspace name"));
            let view = cx.new(|_| WorkspaceManager {
                store,
                settings,
                name_input,
                editing_id: None,
                workspace_dialog_id: None,
                show_workspace_dialog: false,
                delete_workspace_id: None,
                error: None,
                _mcp_stop: None,
            });
            cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
        })
        .expect("failed to open workspace manager");
    }

    fn create_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self
            .new_workspace_name_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        if name.is_empty() {
            self.agent_error = Some("Choose a name for the workspace.".to_string());
            cx.notify();
            return;
        }
        if let Some(id) = self.editing_workspace_id {
            if !self.store.lock().unwrap().rename_workspace(id, name) {
                self.agent_error = Some("Workspace no longer exists.".to_string());
                cx.notify();
                return;
            }
            self.show_new_workspace = false;
            self.editing_workspace_id = None;
            self.agent_error = None;
            self.persist_store();
            cx.update_entity(&self.new_workspace_name_input, |input, input_cx| {
                input.clean(window, input_cx);
            });
            cx.notify();
            return;
        }
        let id = Uuid::new_v4();
        self.store
            .lock()
            .unwrap()
            .add_workspace(knot_core::Workspace {
                id,
                name,
                color_hex: "#1B4FB2".to_string(),
                agent_ids: Vec::new(),
                layout_mode: "single".to_string(),
                active_agent_ids: Vec::new(),
                focused_pane_index: 0,
                split_ratio: 0.5,
                split_ratio_secondary: None,
                show_dashboard: None,
                is_detached: None,
            });
        self.store.lock().unwrap().set_current_workspace(id);
        self.agent_selection = None;
        self.show_new_workspace = false;
        self.editing_workspace_id = None;
        self.agent_error = None;
        self.persist_store();
        cx.update_entity(&self.new_workspace_name_input, |input, input_cx| {
            input.clean(window, input_cx);
        });
        cx.notify();
    }

    fn cancel_new_workspace(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.show_new_workspace = false;
        self.editing_workspace_id = None;
        self.agent_error = None;
        cx.update_entity(&self.new_workspace_name_input, |input, input_cx| {
            input.clean(window, input_cx);
        });
        cx.notify();
    }

    fn rename_workspace(&mut self, id: Uuid, window: &mut Window, cx: &mut Context<Self>) {
        let Some(name) = self
            .store
            .lock()
            .unwrap()
            .workspaces()
            .iter()
            .find(|workspace| workspace.id == id)
            .map(|workspace| workspace.name.clone())
        else {
            return;
        };
        self.editing_workspace_id = Some(id);
        self.show_new_workspace = true;
        self.agent_error = None;
        cx.update_entity(&self.new_workspace_name_input, |input, input_cx| {
            input.set_value(name, window, input_cx);
        });
        cx.notify();
    }

    fn reap_sessions(&mut self, live_ids: &BTreeSet<Uuid>) {
        let session_ids = self.sessions.keys().copied().collect::<Vec<_>>();
        for id in stale_session_ids(&session_ids, live_ids) {
            self.remove_session(id, true);
        }
    }

    fn remove_exited_sessions(&mut self, ids: &[Uuid]) {
        for id in ids {
            self.remove_session(*id, false);
        }
    }

    fn remove_session(&mut self, id: Uuid, shutdown: bool) {
        if let Some(session) = self.sessions.remove(&id)
            && shutdown
            && let Ok(mut session) = session.lock()
        {
            let _ = session.shutdown();
        }
        self.buffers.remove(&id);
    }
}

fn manager_window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(800.), px(600.)), cx)),
        window_min_size: Some(size(px(640.), px(420.))),
        ..WindowOptions::default()
    }
}

fn workspace_window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(960.), px(640.)), cx)),
        window_min_size: Some(size(px(760.), px(520.))),
        ..WindowOptions::default()
    }
}

fn agent_window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(520.), px(500.)), cx)),
        window_min_size: Some(size(px(460.), px(460.))),
        ..WindowOptions::default()
    }
}

struct WorkspaceWindow {
    store: Arc<Mutex<knot_agents::AgentStore>>,
    settings: knot_core::Settings,
    workspace_id: Uuid,
    selected_agent: Option<Uuid>,
    new_agent_name_input: Entity<InputState>,
    new_agent_folder_input: Entity<InputState>,
    show_new_agent: bool,
    error: Option<String>,
}

impl WorkspaceWindow {
    fn open(
        store: Arc<Mutex<knot_agents::AgentStore>>,
        settings: knot_core::Settings,
        workspace_id: Uuid,
        cx: &mut Context<WorkspaceManager>,
    ) {
        let options = workspace_window_options(cx);
        if let Err(error) = cx.open_window(options, move |window, cx| {
            let new_agent_name_input =
                cx.new(|cx| InputState::new(window, cx).placeholder("Agent name (optional)"));
            let new_agent_folder_input =
                cx.new(|cx| InputState::new(window, cx).placeholder("Agent folder path"));
            let selected_agent = store
                .lock()
                .ok()
                .and_then(|store| agent_selection_for_workspace(&store, workspace_id));
            let view = cx.new(|_| WorkspaceWindow {
                store,
                settings,
                workspace_id,
                selected_agent,
                new_agent_name_input,
                new_agent_folder_input,
                show_new_agent: false,
                error: None,
            });
            cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
        }) {
            eprintln!("failed to open workspace window: {error}");
        }
    }

    fn create_agent(&mut self, window: &mut Window, cx: &mut Context<Self>) -> bool {
        let folder = self
            .new_agent_folder_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        if folder.is_empty() || !PathBuf::from(&folder).is_dir() {
            self.error = Some("Choose an existing agent folder.".to_string());
            cx.notify();
            return false;
        }
        let name = self
            .new_agent_name_input
            .read(cx)
            .value()
            .trim()
            .to_string();
        let id = {
            let mut store = self.store.lock().unwrap();
            store.set_current_workspace(self.workspace_id);
            store.create(
                folder,
                knot_agents::CreateOptions {
                    name: (!name.is_empty()).then_some(name),
                    ..Default::default()
                },
            )
        };
        if let Ok(store) = self.store.lock() {
            self.settings.saved_agents =
                store.saved_agents(self.settings.restore_conversation_on_launch);
            self.settings.saved_workspaces = store.saved_workspaces();
        }
        let _ = self.settings.persist();
        self.selected_agent = Some(id);
        self.show_new_agent = false;
        self.error = None;
        cx.update_entity(&self.new_agent_name_input, |input, input_cx| {
            input.clean(window, input_cx);
        });
        cx.update_entity(&self.new_agent_folder_input, |input, input_cx| {
            input.clean(window, input_cx);
        });
        cx.notify();
        true
    }

    fn open_new_agent_dialog(&mut self, _window: &mut Window, cx: &mut Context<Self>) {
        let store = Arc::clone(&self.store);
        let settings = self.settings.clone();
        let workspace_id = self.workspace_id;
        let options = agent_window_options(cx);
        let _ = cx.open_window(options, move |window, cx| {
            let name_input =
                cx.new(|cx| InputState::new(window, cx).placeholder("Name (optional)"));
            let shell_command_input =
                cx.new(|cx| InputState::new(window, cx).placeholder("Shell command"));
            let view = cx.new(|_| AgentEditor {
                store,
                settings,
                workspace_id,
                name_input,
                shell_command_input,
                folder_path: String::new(),
                avatar: "🤖".to_string(),
                agent_type: "claude".to_string(),
                persona_id: None,
                error: None,
            });
            cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
        });
    }
}

struct AgentEditor {
    store: Arc<Mutex<knot_agents::AgentStore>>,
    settings: knot_core::Settings,
    workspace_id: Uuid,
    name_input: Entity<InputState>,
    shell_command_input: Entity<InputState>,
    folder_path: String,
    avatar: String,
    agent_type: String,
    persona_id: Option<Uuid>,
    error: Option<String>,
}

impl AgentEditor {
    fn create(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let folder = self.folder_path.trim().to_string();
        if folder.is_empty() || !PathBuf::from(&folder).is_dir() {
            self.error = Some("Choose an existing agent folder.".to_string());
            cx.notify();
            return;
        }
        let name = self.name_input.read(cx).value().trim().to_string();
        let avatar = self.avatar.clone();
        let agent_type = self.agent_type.clone();
        let shell_command = self.shell_command_input.read(cx).value().trim().to_string();
        {
            let mut store = self.store.lock().unwrap();
            store.set_current_workspace(self.workspace_id);
            store.create(
                folder,
                knot_agents::CreateOptions {
                    name: (!name.is_empty()).then_some(name),
                    avatar: (!avatar.is_empty()).then_some(avatar),
                    agent_type: (!agent_type.is_empty()).then_some(agent_type),
                    shell_command: (!shell_command.is_empty()).then_some(shell_command),
                    persona_id: self.persona_id,
                    ..Default::default()
                },
            );
            self.settings.saved_agents =
                store.saved_agents(self.settings.restore_conversation_on_launch);
            self.settings.saved_workspaces = store.saved_workspaces();
        }
        let _ = self.settings.persist();
        window.remove_window();
    }

    fn choose_folder(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose Agent Folder".into()),
        });
        let editor = cx.entity();
        cx.spawn(async move |_this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            cx.update(|app| {
                editor.update(app, |editor, cx| {
                    editor.folder_path = path.to_string_lossy().into_owned();
                    cx.notify();
                });
            });
        })
        .detach();
    }
}

impl Render for AgentEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let editor = cx.entity();
        let personas = self.settings.personas.clone();
        v_flex()
            .size_full()
            .gap_3()
            .p_5()
            .bg(cx.theme().background)
            .child(div().text_xl().child("New Agent"))
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("add a new agent to your knot"),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(div().w(px(100.)).text_right().child("Name"))
                    .child(Input::new(&self.name_input).flex_1())
                    .child(
                        Button::new("agent-avatar-picker")
                            .label(self.avatar.clone())
                            .tooltip("Choose avatar")
                            .dropdown_menu({
                                let editor = editor.clone();
                                move |menu, _, _| {
                                    menu.item(PopupMenuItem::new("🤖 Robot").on_click({
                                        let editor = editor.clone();
                                        move |_, _, app| {
                                            editor.update(app, |e, _| e.avatar = "🤖".to_string())
                                        }
                                    }))
                                    .item(PopupMenuItem::new("🧠 Brain").on_click({
                                        let editor = editor.clone();
                                        move |_, _, app| {
                                            editor.update(app, |e, _| e.avatar = "🧠".to_string())
                                        }
                                    }))
                                    .item(
                                        PopupMenuItem::new("💻 Computer").on_click({
                                            let editor = editor.clone();
                                            move |_, _, app| {
                                                editor
                                                    .update(app, |e, _| e.avatar = "💻".to_string())
                                            }
                                        }),
                                    )
                                }
                            }),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(div().w(px(100.)).text_right().child("Coding agent"))
                    .child(
                        Button::new("agent-type-picker")
                            .label(self.agent_type.clone())
                            .dropdown_menu({
                                let editor = editor.clone();
                                move |menu, _, _| {
                                    menu.item(PopupMenuItem::new("Claude").on_click({
                                        let editor = editor.clone();
                                        move |_, _, app| {
                                            editor.update(app, |e, _| {
                                                e.agent_type = "claude".to_string()
                                            })
                                        }
                                    }))
                                    .item(PopupMenuItem::new("Codex").on_click({
                                        let editor = editor.clone();
                                        move |_, _, app| {
                                            editor.update(app, |e, _| {
                                                e.agent_type = "codex".to_string()
                                            })
                                        }
                                    }))
                                    .item(
                                        PopupMenuItem::new("Shell").on_click({
                                            let editor = editor.clone();
                                            move |_, _, app| {
                                                editor.update(app, |e, _| {
                                                    e.agent_type = "shell".to_string()
                                                })
                                            }
                                        }),
                                    )
                                }
                            }),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(div().w(px(100.)).text_right().child("Command"))
                    .child(Input::new(&self.shell_command_input).flex_1()),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(div().w(px(100.)).text_right().child("Persona"))
                    .child(
                        Button::new("agent-persona-picker")
                            .label(
                                self.persona_id
                                    .and_then(|id| {
                                        personas.iter().find(|p| p.id == id).map(|p| p.name.clone())
                                    })
                                    .unwrap_or_else(|| "None".to_string()),
                            )
                            .flex_1()
                            .dropdown_menu({
                                let editor = editor.clone();
                                move |mut menu, _, _| {
                                    menu = menu.item(PopupMenuItem::new("None").on_click({
                                        let editor = editor.clone();
                                        move |_, _, app| {
                                            editor.update(app, |e, _| e.persona_id = None)
                                        }
                                    }));
                                    for persona in &personas {
                                        let id = persona.id;
                                        menu = menu.item(
                                            PopupMenuItem::new(persona.name.clone()).on_click({
                                                let editor = editor.clone();
                                                move |_, _, app| {
                                                    editor
                                                        .update(app, |e, _| e.persona_id = Some(id))
                                                }
                                            }),
                                        );
                                    }
                                    menu
                                }
                            }),
                    ),
            )
            .child(
                h_flex()
                    .gap_2()
                    .child(div().w(px(100.)).text_right().child("Folder"))
                    .child(
                        div()
                            .flex_1()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .child(if self.folder_path.is_empty() {
                                "No folder selected".to_string()
                            } else {
                                self.folder_path.clone()
                            }),
                    )
                    .child(
                        Button::new("choose-agent-folder")
                            .label("Choose...")
                            .on_click(cx.listener(|editor, _, _, cx| editor.choose_folder(cx))),
                    ),
            )
            .children(
                self.error
                    .as_ref()
                    .map(|error| div().text_sm().child(error.clone())),
            )
            .child(
                h_flex()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cancel-agent-editor")
                            .label("Cancel")
                            .on_click(|_, window, _| window.remove_window()),
                    )
                    .child(
                        Button::new("create-agent-editor")
                            .label("Add Agent")
                            .primary()
                            .on_click(
                                cx.listener(|editor, _, window, cx| editor.create(window, cx)),
                            ),
                    ),
            )
    }
}
impl Render for WorkspaceWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let (workspace_name, agents) = {
            let store = self.store.lock().unwrap();
            let Some(workspace) = store
                .workspaces()
                .iter()
                .find(|workspace| workspace.id == self.workspace_id)
            else {
                return v_flex().size_full().child("Workspace no longer exists.");
            };
            let agents = workspace
                .agent_ids
                .iter()
                .filter_map(|id| store.agent(*id))
                .map(|agent| {
                    (
                        agent.id,
                        agent.avatar.clone(),
                        agent.name.clone(),
                        agent.folder.clone(),
                    )
                })
                .collect::<Vec<_>>();
            (workspace.name.clone(), agents)
        };

        let agent_rows = agents.into_iter().map(|(id, avatar, name, folder)| {
            Button::new(format!("workspace-agent-{id}"))
                .h(px(76.))
                .child(
                    v_flex()
                        .w_full()
                        .gap_2()
                        .child(div().text_lg().child(format!("{avatar}  {name}")))
                        .child(
                            div()
                                .w_full()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .overflow_hidden()
                                .whitespace_nowrap()
                                .text_ellipsis()
                                .child(folder),
                        ),
                )
                .selected(self.selected_agent == Some(id))
                .on_click(cx.listener(move |view, _: &ClickEvent, _window, cx| {
                    view.selected_agent = Some(id);
                    cx.notify();
                }))
        });

        let selected_title = self
            .selected_agent
            .and_then(|id| {
                self.store.lock().ok().and_then(|store| {
                    store
                        .agent(id)
                        .map(|agent| agent.header_title().to_string())
                })
            })
            .unwrap_or_else(|| "Choose an agent from the sidebar".to_string());

        h_flex()
            .size_full()
            .relative()
            .bg(cx.theme().background)
            .child(
                v_flex()
                    .w(px(250.))
                    .h_full()
                    .gap_2()
                    .p_4()
                    .bg(cx.theme().muted)
                    .child(
                        v_flex()
                            .gap_1()
                            .child(div().text_lg().child(workspace_name.clone()))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Workspace"),
                            ),
                    )
                    .children(agent_rows)
                    .child(div().flex_1())
                    .children(
                        self.error
                            .as_ref()
                            .map(|error| div().text_sm().child(error.clone())),
                    )
                    .child(
                        Button::new("workspace-new-agent")
                            .icon(IconName::Plus)
                            .tooltip("New agent")
                            .on_click(cx.listener(|view, _: &ClickEvent, window, cx| {
                                view.open_new_agent_dialog(window, cx);
                            })),
                    ),
            )
            .child(
                v_flex()
                    .flex_1()
                    .child(
                        h_flex()
                            .h(px(56.))
                            .px_5()
                            .items_center()
                            .border_b_1()
                            .border_color(cx.theme().border)
                            .child(
                                v_flex()
                                    .gap_1()
                                    .child(div().text_lg().child(selected_title))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child("Terminal"),
                                    ),
                            ),
                    )
                    .child(
                        div()
                            .size_full()
                            .p_6()
                            .flex()
                            .items_center()
                            .justify_center()
                            .bg(cx.theme().muted)
                            .child(
                                div()
                                    .text_color(cx.theme().muted_foreground)
                                    .child("Terminal display will appear here"),
                            ),
                    ),
            )
    }
}

struct WorkspaceManager {
    store: Arc<Mutex<knot_agents::AgentStore>>,
    settings: knot_core::Settings,
    name_input: Entity<InputState>,
    editing_id: Option<Uuid>,
    workspace_dialog_id: Option<Uuid>,
    show_workspace_dialog: bool,
    delete_workspace_id: Option<Uuid>,
    error: Option<String>,
    _mcp_stop: Option<tokio::sync::oneshot::Sender<()>>,
}

#[derive(Clone)]
struct WorkspaceDrag(Uuid);

struct WorkspaceDragPreview;

impl Render for WorkspaceDragPreview {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().p_2().child("Workspace")
    }
}

impl WorkspaceManager {
    fn persist(&mut self) {
        let Ok(store) = self.store.lock() else {
            self.error = Some("Agent store is unavailable.".to_string());
            return;
        };
        self.settings.saved_agents =
            store.saved_agents(self.settings.restore_conversation_on_launch);
        self.settings.saved_workspaces = store.saved_workspaces();
        if let Err(error) = self.settings.persist() {
            self.error = Some(format!("Could not save workspace: {error}"));
        }
    }

    fn save_name(
        &mut self,
        name: String,
        editing_id: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        if name.is_empty() {
            self.error = Some("Workspace name cannot be empty.".to_string());
            cx.notify();
            return;
        }
        let mut store = self.store.lock().unwrap();
        if let Some(id) = editing_id {
            if !store.rename_workspace(id, name) {
                self.error = Some("Workspace no longer exists.".to_string());
                cx.notify();
                return;
            }
        } else {
            let id = Uuid::new_v4();
            store.add_workspace(knot_core::Workspace {
                id,
                name,
                color_hex: "#1B4FB2".to_string(),
                agent_ids: Vec::new(),
                layout_mode: "single".to_string(),
                active_agent_ids: Vec::new(),
                focused_pane_index: 0,
                split_ratio: 0.5,
                split_ratio_secondary: None,
                show_dashboard: None,
                is_detached: None,
            });
            store.set_current_workspace(id);
        }
        drop(store);
        self.persist();
        self.editing_id = None;
        self.error = None;
        cx.update_entity(&self.name_input, |input, input_cx| {
            input.clean(window, input_cx);
        });
        cx.notify();
    }

    fn open_workspace_dialog(
        &mut self,
        editing_id: Option<Uuid>,
        window: &mut Window,
        cx: &mut Context<Self>,
    ) {
        let name = editing_id
            .and_then(|id| {
                self.store.lock().ok().and_then(|store| {
                    store
                        .workspaces()
                        .iter()
                        .find(|workspace| workspace.id == id)
                        .map(|workspace| workspace.name.clone())
                })
            })
            .unwrap_or_default();
        self.workspace_dialog_id = editing_id;
        self.show_workspace_dialog = true;
        self.error = None;
        cx.update_entity(&self.name_input, |input, input_cx| {
            input.set_value(name, window, input_cx);
        });
        cx.notify();
    }

    fn cancel_workspace_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        self.workspace_dialog_id = None;
        self.show_workspace_dialog = false;
        self.error = None;
        cx.update_entity(&self.name_input, |input, input_cx| {
            input.clean(window, input_cx);
        });
        cx.notify();
    }

    fn confirm_workspace_dialog(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name_input.read(cx).value().trim().to_string();
        let editing_id = self.workspace_dialog_id;
        self.save_name(name, editing_id, window, cx);
        if self.error.is_none() {
            self.workspace_dialog_id = None;
            self.show_workspace_dialog = false;
        }
        cx.notify();
    }

    fn delete(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if !self.store.lock().unwrap().remove_workspace(id) {
            self.error = Some("At least one workspace must remain.".to_string());
        } else {
            self.persist();
            self.error = None;
        }
        cx.notify();
    }

    fn request_delete(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.delete_workspace_id = Some(id);
        self.error = None;
        cx.notify();
    }

    fn cancel_delete(&mut self, cx: &mut Context<Self>) {
        self.delete_workspace_id = None;
        cx.notify();
    }

    fn confirm_delete(&mut self, cx: &mut Context<Self>) {
        let Some(id) = self.delete_workspace_id.take() else {
            return;
        };
        self.delete(id, cx);
    }

    fn move_before(&mut self, id: Uuid, target_id: Uuid, cx: &mut Context<Self>) {
        if self
            .store
            .lock()
            .unwrap()
            .move_workspace_before(id, target_id)
        {
            self.persist();
            cx.notify();
        }
    }

    fn select(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.store.lock().unwrap().set_current_workspace(id);
        cx.notify();
    }

    fn open(&mut self, id: Uuid, cx: &mut Context<Self>) {
        self.select(id, cx);
        WorkspaceWindow::open(Arc::clone(&self.store), self.settings.clone(), id, cx);
    }
}

impl Render for WorkspaceManager {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let workspaces = self.store.lock().unwrap().workspaces().to_vec();
        let delete_name = self.delete_workspace_id.and_then(|id| {
            workspaces
                .iter()
                .find(|workspace| workspace.id == id)
                .map(|workspace| workspace.name.clone())
        });
        let rows = workspaces.into_iter().map(|workspace| {
            let id = workspace.id;
            let agent_count = workspace.agent_ids.len();
            let selected = self.store.lock().unwrap().current_workspace_id() == Some(id);
            h_flex()
                .id(format!("workspace-row-{id}"))
                .on_drop(
                    cx.listener(move |manager, drag: &WorkspaceDrag, _window, cx| {
                        manager.move_before(drag.0, id, cx);
                    }),
                )
                .w_full()
                .items_center()
                .gap_3()
                .p_3()
                .rounded(cx.theme().radius)
                .bg(if selected {
                    cx.theme().muted
                } else {
                    cx.theme().transparent
                })
                .child(
                    v_flex()
                        .flex_1()
                        .gap_1()
                        .child(div().text_lg().child(workspace.name.clone()))
                        .child(
                            div()
                                .text_sm()
                                .text_color(cx.theme().muted_foreground)
                                .child(format!("{agent_count} agents")),
                        ),
                )
                .child(
                    Button::new(format!("open-workspace-{id}"))
                        .icon(IconName::ExternalLink)
                        .ghost()
                        .tooltip("Open workspace")
                        .on_click(cx.listener(move |manager, _: &ClickEvent, _window, cx| {
                            manager.open(id, cx);
                        })),
                )
                .child(
                    Button::new(format!("rename-workspace-{id}"))
                        .icon(IconName::FileText)
                        .ghost()
                        .tooltip("Rename workspace")
                        .on_click(cx.listener(move |manager, _: &ClickEvent, window, cx| {
                            manager.open_workspace_dialog(Some(id), window, cx);
                        })),
                )
                .child(
                    Button::new(format!("delete-workspace-{id}"))
                        .icon(IconName::Delete)
                        .danger()
                        .tooltip("Delete workspace")
                        .on_click(cx.listener(move |manager, _: &ClickEvent, _window, cx| {
                            manager.request_delete(id, cx);
                        })),
                )
                .child(
                    div()
                        .id(format!("workspace-drag-{id}"))
                        .w(px(28.))
                        .h(px(28.))
                        .flex()
                        .items_center()
                        .justify_center()
                        .cursor_move()
                        .child(Icon::new(IconName::Menu))
                        .on_drag(WorkspaceDrag(id), |_drag, _position, _window, cx| {
                            cx.new(|_| WorkspaceDragPreview)
                        }),
                )
        });

        v_flex()
            .size_full()
            .gap_4()
            .p_4()
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .items_center()
                    .child(div().text_2xl().child("Workspaces"))
                    .child(div().flex_1())
                    .child(
                        Button::new("new-workspace")
                            .icon(IconName::Plus)
                            .primary()
                            .tooltip("New workspace")
                            .on_click(cx.listener(|manager, _: &ClickEvent, window, cx| {
                                manager.open_workspace_dialog(None, window, cx);
                            })),
                    ),
            )
            .child(v_flex().gap_2().children(rows))
            .children(self.error.as_ref().map(|error| div().child(error.clone())))
            .children(self.show_workspace_dialog.then(|| {
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(cx.theme().overlay)
                    .child(
                        v_flex()
                            .w(px(360.))
                            .gap_3()
                            .p_4()
                            .rounded(cx.theme().radius_lg)
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .child(
                                div()
                                    .text_lg()
                                    .child(if self.workspace_dialog_id.is_some() {
                                        "Rename Workspace"
                                    } else {
                                        "New Workspace"
                                    }),
                            )
                            .child(Input::new(&self.name_input).h_full())
                            .child(
                                h_flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        Button::new("cancel-workspace-dialog")
                                            .label("Cancel")
                                            .on_click(cx.listener(
                                                |manager, _: &ClickEvent, window, cx| {
                                                    manager.cancel_workspace_dialog(window, cx);
                                                },
                                            )),
                                    )
                                    .child(
                                        Button::new("confirm-workspace-dialog")
                                            .label("Save")
                                            .primary()
                                            .on_click(cx.listener(
                                                |manager, _: &ClickEvent, window, cx| {
                                                    manager.confirm_workspace_dialog(window, cx);
                                                },
                                            )),
                                    ),
                            ),
                    )
            }))
            .children(self.delete_workspace_id.map(|_| {
                div()
                    .absolute()
                    .inset_0()
                    .flex()
                    .items_center()
                    .justify_center()
                    .bg(cx.theme().overlay)
                    .child(
                        v_flex()
                            .w(px(360.))
                            .gap_3()
                            .p_4()
                            .rounded(cx.theme().radius_lg)
                            .bg(cx.theme().background)
                            .border_1()
                            .border_color(cx.theme().border)
                            .child(div().text_lg().child("Delete Workspace?"))
                            .child(
                                div()
                                    .text_sm()
                                    .text_color(cx.theme().muted_foreground)
                                    .child(format!(
                                        "Delete \"{}\" and its agents?",
                                        delete_name.as_deref().unwrap_or("this workspace")
                                    )),
                            )
                            .child(
                                h_flex()
                                    .justify_end()
                                    .gap_2()
                                    .child(
                                        Button::new("cancel-delete-workspace")
                                            .label("Cancel")
                                            .on_click(cx.listener(
                                                |manager, _: &ClickEvent, _window, cx| {
                                                    manager.cancel_delete(cx);
                                                },
                                            )),
                                    )
                                    .child(
                                        Button::new("confirm-delete-workspace")
                                            .label("Delete")
                                            .danger()
                                            .on_click(cx.listener(
                                                |manager, _: &ClickEvent, _window, cx| {
                                                    manager.confirm_delete(cx);
                                                },
                                            )),
                                    ),
                            ),
                    )
            }))
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
                let name = row.name;
                h_flex()
                    .child(
                        Button::new(id.to_string())
                            .label(name)
                            .selected(row.selected)
                            .on_click(cx.listener(move |shell, _: &ClickEvent, _window, cx| {
                                let selection = {
                                    let mut store = shell.store.lock().unwrap();
                                    store.set_current_workspace(id);
                                    agent_selection_for_workspace(&store, id)
                                };
                                shell.agent_selection = selection;
                                if let Some(agent_id) = selection {
                                    shell.attach_session(agent_id);
                                }
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(format!("rename-workspace-{id}"))
                            .label("Rename")
                            .on_click(cx.listener(move |shell, _: &ClickEvent, window, cx| {
                                shell.rename_workspace(id, window, cx);
                            })),
                    )
            })
            .collect::<Vec<_>>();

        let new_workspace_button = Button::new("new-workspace")
            .label("New Workspace")
            .on_click(cx.listener(|shell, _: &ClickEvent, _window, cx| {
                shell.show_new_workspace = true;
                shell.agent_error = None;
                cx.notify();
            }));
        let manage_workspaces_button = Button::new("manage-workspaces")
            .label("Manage Workspaces")
            .on_click(cx.listener(|shell, _: &ClickEvent, _window, cx| {
                shell.open_workspace_manager(cx);
            }));
        let new_workspace_form = if self.show_new_workspace {
            v_flex()
                .child(Input::new(&self.new_workspace_name_input).h_full())
                .child(
                    h_flex()
                        .child(
                            Button::new("create-workspace")
                                .label(if self.editing_workspace_id.is_some() {
                                    "Save"
                                } else {
                                    "Create"
                                })
                                .on_click(cx.listener(|shell, _: &ClickEvent, window, cx| {
                                    shell.create_workspace(window, cx);
                                })),
                        )
                        .child(
                            Button::new("cancel-create-workspace")
                                .label("Cancel")
                                .on_click(cx.listener(|shell, _: &ClickEvent, window, cx| {
                                    shell.cancel_new_workspace(window, cx);
                                })),
                        ),
                )
        } else {
            v_flex()
        };

        let agent_buttons = model
            .selected_agent_rows
            .into_iter()
            .map(|row| {
                let id = row.id;
                let attached = row.attached;
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
                h_flex()
                    .child(
                        Button::new(format!("select-agent-{id}"))
                            .label(label)
                            .selected(row.selected)
                            .on_click(cx.listener(move |shell, _: &ClickEvent, _window, cx| {
                                shell.agent_selection = Some(id);
                                shell.attach_session(id);
                                cx.notify();
                            })),
                    )
                    .child(
                        Button::new(format!("attach-agent-{id}"))
                            .label(if attached { "Restart" } else { "Attach" })
                            .on_click(cx.listener(move |shell, _: &ClickEvent, _window, cx| {
                                if attached {
                                    shell.restart_agent(id, cx);
                                } else {
                                    shell.attach_agent(id, cx);
                                }
                            })),
                    )
                    .child(
                        Button::new(format!("close-agent-{id}"))
                            .label("Close")
                            .on_click(cx.listener(move |shell, _: &ClickEvent, _window, cx| {
                                shell.close_agent(id, cx);
                            })),
                    )
            })
            .collect::<Vec<_>>();

        let new_agent_button = Button::new("new-agent")
            .label("New Agent")
            .on_click(cx.listener(|shell, _: &ClickEvent, _window, cx| {
                shell.show_new_agent = true;
                shell.agent_error = None;
                cx.notify();
            }));
        let new_agent_form =
            if self.show_new_agent {
                v_flex()
                    .child(Input::new(&self.new_agent_folder_input).h_full())
                    .child(Input::new(&self.new_agent_name_input).h_full())
                    .child(
                        h_flex()
                            .child(Button::new("create-agent").label("Create").on_click(
                                cx.listener(|shell, _: &ClickEvent, window, cx| {
                                    shell.create_agent(window, cx);
                                }),
                            ))
                            .child(Button::new("cancel-create-agent").label("Cancel").on_click(
                                cx.listener(|shell, _: &ClickEvent, window, cx| {
                                    shell.cancel_new_agent(window, cx);
                                }),
                            )),
                    )
                    .children(
                        self.agent_error
                            .as_ref()
                            .map(|error| div().child(error.clone())),
                    )
            } else {
                v_flex()
            };

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
            .child(
                v_flex()
                    .size_full()
                    .child(manage_workspaces_button)
                    .child(new_workspace_button)
                    .child(new_workspace_form)
                    .children(workspace_buttons),
            )
            .child(
                v_flex()
                    .size_full()
                    .child(new_agent_button)
                    .child(new_agent_form)
                    .children(agent_buttons),
            )
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
    let mut last_workspace = None;
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
            let exited_ids = entity
                .read(app)
                .process_exits
                .lock()
                .unwrap()
                .drain(..)
                .collect::<Vec<_>>();
            if !exited_ids.is_empty() {
                entity.update(app, |shell, _| shell.remove_exited_sessions(&exited_ids));
                changed = true;
            }
            let live_ids = entity
                .read(app)
                .store
                .lock()
                .unwrap()
                .agents()
                .iter()
                .map(|agent| agent.id)
                .collect::<BTreeSet<_>>();
            let session_ids = entity
                .read(app)
                .sessions
                .keys()
                .copied()
                .collect::<Vec<_>>();
            if !stale_session_ids(&session_ids, &live_ids).is_empty() {
                entity.update(app, |shell, _| shell.reap_sessions(&live_ids));
                changed = true;
            }
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
            let (status, agents, workspace_id) = {
                let store = entity.read(app).store.lock().unwrap();
                (
                    agent_status_snapshot(&store),
                    store.agents().to_vec(),
                    store.current_workspace_id(),
                )
            };
            if last_status.as_ref() != Some(&status) {
                changed = true;
                last_status = Some(status);
            }
            if last_workspace != Some(workspace_id) {
                changed = true;
                last_workspace = Some(workspace_id);
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
                let selected = entity.read(app).agent_selection;
                let show_notice = should_show_awaiting_notice(
                    selected,
                    id,
                    &message,
                    last_awaiting_message.get(&id),
                );
                let Some(agent_name) = agents
                    .iter()
                    .find(|agent| agent.id == id)
                    .map(|agent| agent.name.clone())
                else {
                    continue;
                };
                if should_notify(
                    entity.read(app).settings.desktop_notifications_enabled,
                    show_notice,
                ) {
                    app.show_system_notification(SystemNotification {
                        tag: id.to_string().into(),
                        title: format!("Skwad - {agent_name}").into(),
                        body: notification_body(&message).into(),
                        actions: Vec::new(),
                    });
                }
                if !show_notice {
                    continue;
                }
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
    agents: Arc<Mutex<knot_agents::AgentStore>>,
    settings: knot_core::Settings,
    notifier: Arc<QueuedNotifier>,
    messages: Arc<Mutex<knot_messaging::MessageStore>>,
    awaiting_input: AwaitingInputQueue,
) -> tokio::sync::oneshot::Sender<()> {
    let (stop, stop_rx) = tokio::sync::oneshot::channel();
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

            let (discovery, repos_rx) = knot_discovery::Discovery::new();
            if !settings.source_base_folder.is_empty()
                && let Err(err) =
                    discovery.set_source_folder(Some(PathBuf::from(&settings.source_base_folder)))
            {
                eprintln!("failed to watch source folder: {err}");
            }

            let catalog = Arc::new(
                knot_mcp_tools::McpToolCatalog::new(agents, repos_rx, notifier)
                    .with_message_store(messages)
                    .with_awaiting_input_queue(awaiting_input)
                    .with_settings(settings.clone()),
            );
            catalog.set_bench_agents(settings.bench_agents.clone());

            let agents_snapshot: knot_mcp::AgentsSnapshotFn = {
                let catalog = catalog.clone();
                Arc::new(move || catalog.agents_snapshot())
            };
            let hook_handler = catalog.clone();
            let mut server = knot_mcp::McpServer::new(
                settings.mcp_server_port,
                catalog as Arc<dyn ToolCatalog>,
                agents_snapshot,
            )
            .with_hook_handler(hook_handler);
            if let Err(err) = server.start().await {
                eprintln!("failed to start MCP server: {err}");
                return;
            }

            tokio::select! {
                _ = stop_rx => {}
                _ = std::future::pending::<()>() => {}
            }
            server.stop();
            drop(discovery);
        });
    });
    stop
}

actions!(
    knot_app,
    [Quit, HideApp, HideOthers, ShowAllWindows, AboutKnot]
);

fn quit(_: &Quit, cx: &mut App) {
    cx.quit();
}

fn hide_app(_: &HideApp, cx: &mut App) {
    cx.hide();
}

fn hide_others(_: &HideOthers, cx: &mut App) {
    cx.hide_other_apps();
}

fn show_all_windows(_: &ShowAllWindows, cx: &mut App) {
    cx.activate(true);
}

fn about_knot(_: &AboutKnot, cx: &mut App) {
    if let Some(window) = cx.active_window() {
        let _ = window.update(cx, |_, window, cx| {
            window.open_alert_dialog(cx, |alert, _, _| {
                alert
                    .title("About Knot")
                    .description("Knot is a workspace for coordinating coding agents.")
                    .show_cancel(false)
            });
        });
    }
}

fn set_app_menus(cx: &mut App) {
    cx.set_menus([
        Menu::new("Knot").items([
            MenuItem::action("About Knot", AboutKnot),
            MenuItem::separator(),
            MenuItem::os_submenu("Services", SystemMenuType::Services),
            MenuItem::separator(),
            MenuItem::action("Hide Knot", HideApp),
            MenuItem::action("Hide Others", HideOthers),
            MenuItem::action("Show All", ShowAllWindows),
            MenuItem::separator(),
            MenuItem::action("Quit Knot", Quit),
        ]),
        Menu::new("File").items([
            MenuItem::action("New Workspace", gpui_kit::NoAction).disabled(true),
            MenuItem::separator(),
            MenuItem::action("Close Window", gpui_kit::NoAction).disabled(true),
        ]),
        Menu::new("Edit").items([
            MenuItem::action("Undo", gpui_kit::NoAction).disabled(true),
            MenuItem::action("Redo", gpui_kit::NoAction).disabled(true),
            MenuItem::separator(),
            MenuItem::action("Cut", gpui_kit::NoAction).disabled(true),
            MenuItem::action("Copy", gpui_kit::NoAction).disabled(true),
            MenuItem::action("Paste", gpui_kit::NoAction).disabled(true),
        ]),
        Menu::new("View")
            .items([MenuItem::action("Enter Full Screen", gpui_kit::NoAction).disabled(true)]),
        Menu::new("Window").items([
            MenuItem::action("Minimize", gpui_kit::NoAction).disabled(true),
            MenuItem::action("Zoom", gpui_kit::NoAction).disabled(true),
        ]),
        Menu::new("Help")
            .items([MenuItem::action("Knot Help", gpui_kit::NoAction).disabled(true)]),
    ]);
}

fn main() {
    let mut settings = knot_core::Settings::load().unwrap_or_default();
    if let Err(err) = settings.init_source_folder() {
        eprintln!("failed to initialize source folder: {err}");
    }
    if let Err(err) = settings.install_default_personas() {
        eprintln!("failed to install default personas: {err}");
    }
    let store = Arc::new(Mutex::new(build_agent_store(&settings)));
    let notifier = Arc::new(QueuedNotifier::new());
    let messages = Arc::new(Mutex::new(knot_messaging::MessageStore::new()));
    let awaiting_input = Arc::new(Mutex::new(Vec::new()));
    let mcp_stop = start_mcp_server(
        Arc::clone(&store),
        settings.clone(),
        Arc::clone(&notifier),
        Arc::clone(&messages),
        Arc::clone(&awaiting_input),
    );

    gpui_kit::application()
        .with_assets(gpui_kit::assets::Assets)
        .run(move |cx| {
            gpui_kit::init(cx);
            Theme::change(cx.window_appearance(), None, cx);

            cx.on_action(quit);
            cx.on_action(about_knot);
            cx.on_action(hide_app);
            cx.on_action(hide_others);
            cx.on_action(show_all_windows);
            set_app_menus(cx);

            cx.on_system_notification_response(|response, cx| {
                if notification_response_agent_id(&response).is_some() {
                    cx.activate(true);
                }
            });

            let options = manager_window_options(cx);
            cx.open_window(options, |window, cx| {
                let name_input =
                    cx.new(|cx| InputState::new(window, cx).placeholder("Workspace name"));
                let view = cx.new(|_| WorkspaceManager {
                    store: Arc::clone(&store),
                    settings: settings.clone(),
                    name_input,
                    editing_id: None,
                    workspace_dialog_id: None,
                    show_workspace_dialog: false,
                    delete_workspace_id: None,
                    error: None,
                    _mcp_stop: Some(mcp_stop),
                });
                cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
            })
            .expect("failed to open workspace manager");
        });
}

#[cfg(test)]
mod tests {
    use super::*;
    use knot_core::Workspace;

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
        let store = knot_agents::AgentStore::new();
        let model = layout_model(&store, None, &[], &BTreeMap::new());
        assert!(model.workspace_rows.is_empty());
        assert!(model.selected_agent_rows.is_empty());
    }

    #[test]
    fn selected_workspace_marks_and_filters_rows() {
        let mut store = knot_agents::AgentStore::new();
        let ws1 = workspace("One");
        let ws2 = workspace("Two");
        store.add_workspace(ws1.clone());
        store.add_workspace(ws2.clone());

        store.set_current_workspace(ws1.id);
        store.create("~/alpha", knot_agents::CreateOptions::default());
        store.create("~/beta", knot_agents::CreateOptions::default());

        store.set_current_workspace(ws2.id);
        store.create("~/gamma", knot_agents::CreateOptions::default());

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
        let gamma_id = store
            .agents()
            .iter()
            .find(|agent| agent.name == "gamma")
            .map(|agent| agent.id)
            .unwrap();
        assert_eq!(
            agent_selection_for_workspace(&store, ws2.id),
            Some(gamma_id)
        );
    }

    #[test]
    fn missing_agent_ids_are_skipped() {
        let mut store = knot_agents::AgentStore::new();
        let mut ws = workspace("One");
        ws.agent_ids.push(Uuid::new_v4());
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        store.create("~/alpha", knot_agents::CreateOptions::default());

        let model = layout_model(&store, None, &[], &BTreeMap::new());
        assert_eq!(model.selected_agent_rows.len(), 1);
        assert_eq!(model.selected_agent_rows[0].name, "alpha");
    }

    #[test]
    fn agent_selection_marks_and_tracks_attach_state() {
        let mut store = knot_agents::AgentStore::new();
        let ws = workspace("One");
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        let alpha_id = store.create("~/alpha", knot_agents::CreateOptions::default());
        store.create("~/beta", knot_agents::CreateOptions::default());

        let model = layout_model(&store, Some(alpha_id), &[], &BTreeMap::new());
        let alpha = model
            .selected_agent_rows
            .iter()
            .find(|row| row.id == alpha_id)
            .unwrap();
        assert!(alpha.selected);
        assert!(!alpha.attached);
        assert_eq!(alpha.state, knot_agents::AgentState::Idle);
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
        assert_eq!(state_label(knot_agents::AgentState::Idle), "Idle");
        assert_eq!(state_label(knot_agents::AgentState::Running), "Working");
        assert_eq!(
            state_label(knot_agents::AgentState::Input),
            "Awaiting input"
        );
        assert_eq!(state_label(knot_agents::AgentState::Error), "Error");
    }

    #[test]
    fn layout_model_carries_agent_state_into_rows() {
        let mut store = knot_agents::AgentStore::new();
        let ws = workspace("One");
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        let id = store.create("~/alpha", knot_agents::CreateOptions::default());
        store.set_state(id, knot_agents::AgentState::Input);

        let model = layout_model(&store, None, &[], &BTreeMap::new());
        assert_eq!(
            model.selected_agent_rows[0].state,
            knot_agents::AgentState::Input
        );
    }

    #[test]
    fn terminal_model_renders_header_and_output_for_attached_agent() {
        let mut store = knot_agents::AgentStore::new();
        let ws = workspace("One");
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        let id = store.create("~/alpha", knot_agents::CreateOptions::default());
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
        let mut store = knot_agents::AgentStore::new();
        let ws = workspace("One");
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        let id = store.create("~/alpha", knot_agents::CreateOptions::default());
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
        let store = knot_agents::AgentStore::new();
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
    fn stale_session_ids_excludes_live_agents() {
        let live = Uuid::new_v4();
        let stale = Uuid::new_v4();
        let live_ids = BTreeSet::from([live]);

        assert_eq!(stale_session_ids(&[live, stale], &live_ids), vec![stale]);
    }

    #[test]
    fn delivery_notice_names_the_last_known_recipient_and_counts_events() {
        let mut store = knot_agents::AgentStore::new();
        let first = store.create("~/first", knot_agents::CreateOptions::default());
        let second = store.create("~/second", knot_agents::CreateOptions::default());
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
        let store = knot_agents::AgentStore::new();
        let events = [DeliveryEvent {
            agent_id: Uuid::new_v4(),
            message_id: Uuid::new_v4(),
        }];

        assert_eq!(delivery_notice(&events, store.agents()), None);
    }

    #[test]
    fn unread_counts_snapshot_includes_zero_and_ignores_other_agents() {
        let mut messages = knot_messaging::MessageStore::new();
        let first = Uuid::new_v4();
        let second = Uuid::new_v4();
        let other = Uuid::new_v4();
        messages.add(knot_messaging::Message::new(other, first, "one"));
        messages.add(knot_messaging::Message::new(other, first, "two"));
        messages.add(knot_messaging::Message::new(other, other, "unrelated"));

        let counts = unread_counts_snapshot(&messages, &[first, second]);

        assert_eq!(counts.get(&first), Some(&2));
        assert_eq!(counts.get(&second), Some(&0));
        assert!(!counts.contains_key(&other));
    }

    #[test]
    fn layout_model_carries_unread_count_into_agent_rows() {
        let mut store = knot_agents::AgentStore::new();
        let ws = workspace("One");
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        let id = store.create("~/alpha", knot_agents::CreateOptions::default());
        let unread_counts = BTreeMap::from([(id, 3)]);

        let model = layout_model(&store, None, &[], &unread_counts);

        assert_eq!(model.selected_agent_rows[0].unread_count, 3);
    }

    #[test]
    fn terminal_status_updates_the_shared_agent_store() {
        let mut store = knot_agents::AgentStore::new();
        let id = store.create("~/alpha", knot_agents::CreateOptions::default());
        let shared = Arc::new(Mutex::new(store));

        apply_terminal_status(&shared, id, knot_agents::AgentState::Running);

        assert_eq!(
            shared.lock().unwrap().agent(id).unwrap().state,
            knot_agents::AgentState::Running
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
        let mut store = knot_agents::AgentStore::new();
        let ws = workspace("One");
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        let id = store.create("~/alpha", knot_agents::CreateOptions::default());
        store.create("~/beta", knot_agents::CreateOptions::default());

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

        store.set_state(id, knot_agents::AgentState::Running);
        store.set_status_text(id, "planning".to_string());
        store.set_registered(id, true);
        let updated = agent_status_snapshot(&store);
        assert_ne!(snapshot, updated);
        let key = updated.iter().find(|key| key.id == id).unwrap();
        assert_eq!(key.state, knot_agents::AgentState::Running);
        assert_eq!(key.status_text, "planning");
        assert!(key.is_registered);
        assert_eq!(
            updated.iter().find(|key| key.id != id).unwrap().state,
            knot_agents::AgentState::Idle
        );
    }

    #[test]
    fn build_agent_store_restores_layout_when_enabled() {
        let agent_id = Uuid::new_v4();
        let saved = knot_core::SavedAgent::new(agent_id, "alpha", None, "~/alpha");
        let mut ws = workspace("Restored");
        ws.agent_ids = vec![agent_id];

        let mut settings = knot_core::Settings::default();
        settings.restore_layout_on_launch = true;
        settings.saved_agents = vec![saved];
        settings.saved_workspaces = vec![ws.clone()];

        let store = build_agent_store(&settings);
        assert_eq!(store.agents().len(), 1);
        assert_eq!(store.workspaces(), &[ws.clone()]);
        assert_eq!(store.current_workspace_id(), Some(ws.id));
    }

    #[test]
    fn build_agent_store_restores_exact_session_id_when_conversation_enabled() {
        let agent_id = Uuid::new_v4();
        let mut saved = knot_core::SavedAgent::new(agent_id, "alpha", None, "~/alpha");
        saved.session_id = Some("s7".to_string());

        let mut settings = knot_core::Settings::default();
        settings.restore_layout_on_launch = true;
        settings.restore_conversation_on_launch = true;
        settings.saved_agents = vec![saved];

        let store = build_agent_store(&settings);
        let agent = store.agent(agent_id).unwrap();
        assert_eq!(agent.resume_session_id.as_deref(), Some("s7"));
        assert!(agent.session_id.is_none());
    }

    #[test]
    fn build_agent_store_leaves_resume_session_unset_when_conversation_disabled() {
        let agent_id = Uuid::new_v4();
        let mut saved = knot_core::SavedAgent::new(agent_id, "alpha", None, "~/alpha");
        saved.session_id = Some("s7".to_string());

        let mut settings = knot_core::Settings::default();
        settings.restore_layout_on_launch = true;
        settings.restore_conversation_on_launch = false;
        settings.saved_agents = vec![saved];

        let store = build_agent_store(&settings);
        let agent = store.agent(agent_id).unwrap();
        assert!(agent.resume_session_id.is_none());
    }

    #[test]
    fn build_agent_store_starts_empty_when_restore_disabled() {
        let mut settings = knot_core::Settings::default();
        settings.restore_layout_on_launch = false;
        settings.saved_agents = vec![knot_core::SavedAgent::new(
            Uuid::new_v4(),
            "alpha",
            None,
            "~/alpha",
        )];

        let store = build_agent_store(&settings);
        assert!(store.agents().is_empty());
        assert!(store.workspaces().is_empty());
    }

    #[test]
    fn initial_selection_prefers_active_agent_and_skips_stale_ids() {
        let mut store = knot_agents::AgentStore::new();
        let ws = workspace("One");
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        let first = store.create("~/first", knot_agents::CreateOptions::default());
        let second = store.create("~/second", knot_agents::CreateOptions::default());

        let mut saved = store.saved_workspaces()[0].clone();
        saved.active_agent_ids = vec![Uuid::new_v4(), second];
        let restored =
            knot_agents::AgentStore::from_saved(&store.saved_agents(false), vec![saved]);

        assert_eq!(initial_agent_selection(&restored), Some(second));
        assert_ne!(initial_agent_selection(&restored), Some(first));
    }

    #[test]
    fn should_notify_requires_setting_and_notice() {
        assert!(should_notify(true, true));
        assert!(!should_notify(false, true));
        assert!(!should_notify(true, false));
        assert!(!should_notify(false, false));
    }

    #[test]
    fn notification_body_uses_message_when_present() {
        assert_eq!(notification_body("Grant access?"), "Grant access?");
    }

    #[test]
    fn notification_body_defaults_on_empty() {
        assert_eq!(notification_body(""), AWAITING_INPUT_DEFAULT_BODY);
    }

    #[test]
    fn notification_response_agent_id_parses_valid_tag() {
        let id = Uuid::new_v4();
        let response = SystemNotificationResponse {
            tag: id.to_string().into(),
            action_id: None,
        };
        assert_eq!(notification_response_agent_id(&response), Some(id));
    }

    #[test]
    fn notification_response_agent_id_none_for_invalid_tag() {
        let response = SystemNotificationResponse {
            tag: "not-a-uuid".into(),
            action_id: None,
        };
        assert_eq!(notification_response_agent_id(&response), None);
    }
}
