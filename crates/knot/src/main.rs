#![allow(dead_code)]

use std::cell::RefCell;
use std::collections::{BTreeMap, BTreeSet};
use std::path::PathBuf;
use std::rc::Rc;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::base::Selectable;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::button::{Button, ButtonVariants};
use gpui_kit::component::group_box::{GroupBox, GroupBoxVariants};
use gpui_kit::component::input::{Input, InputEvent, InputState, Textarea, TextareaState};
use gpui_kit::component::menu::{DropdownMenu, PopupMenuItem};
use gpui_kit::component::switch::Switch;
use gpui_kit::component::tab::{Tab, TabBar};
use gpui_kit::component::*;
use gpui_kit::{
    AnyWindowHandle, App, AppContext, AsyncApp, ClickEvent, ClipboardItem, Context, Entity,
    InteractiveElement, IntoElement, KeyBinding, Menu, MenuItem, ParentElement, PathPromptOptions,
    Render, StatefulInteractiveElement, Styled, Subscription, SystemMenuType, SystemNotification,
    SystemNotificationResponse, WeakEntity, Window, WindowBounds, WindowOptions, actions, div, px,
    rgb, size,
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

/// Drives the real macOS font panel (`NSFontPanel`) for the terminal font
/// picker, since the user wants the system chooser rather than an in-app
/// dropdown. `NSFontManager.selectedFont` updates live as the user clicks
/// around the panel, so we just poll it from GPUI (see [`poll_selection`])
/// instead of relying on the `changeFont:` target/action message - AppKit's
/// own responder chain (e.g. text views taking first responder) can steal
/// that target, but the property read is unaffected either way.
#[cfg(target_os = "macos")]
mod native_font_panel {
    use objc2::MainThreadMarker;
    use objc2_app_kit::NSFontManager;
    use objc2_foundation::NSString;
    use std::sync::Mutex;

    static LAST_SEEN_FAMILY: Mutex<Option<String>> = Mutex::new(None);

    /// Opens the system font panel pre-selected to `current_family`. No-op
    /// off the main thread.
    pub fn open(current_family: &str) {
        let Some(mtm) = MainThreadMarker::new() else {
            return;
        };
        *LAST_SEEN_FAMILY.lock().unwrap() = Some(current_family.to_string());
        let manager = NSFontManager::sharedFontManager(mtm);
        if let Some(font) =
            objc2_app_kit::NSFont::fontWithName_size(&NSString::from_str(current_family), 13.0)
        {
            manager.setSelectedFont_isMultiple(&font, false);
        }
        if let Some(panel) = manager.fontPanel(true) {
            panel.makeKeyAndOrderFront(None);
        }
    }

    /// Returns the newly chosen family name if it differs from the last
    /// value seen (by `open` or a prior poll). No-op off the main thread.
    pub fn poll_selection() -> Option<String> {
        let mtm = MainThreadMarker::new()?;
        let manager = NSFontManager::sharedFontManager(mtm);
        let selected = manager.selectedFont()?;
        let converted = manager.convertFont(&selected);
        let family = converted.familyName()?.to_string();
        let mut last_seen = LAST_SEEN_FAMILY.lock().unwrap();
        if last_seen.as_deref() == Some(family.as_str()) {
            return None;
        }
        *last_seen = Some(family.clone());
        Some(family)
    }
}

/// Embedded UI font (SIL OFL licensed; see `assets/fonts/ADAMINA-LICENSE.txt`),
/// so the app looks the same regardless of what's installed on the system.
/// Adamina ships one weight only; the renderer synthesizes bold for
/// `font_semibold`/`font_bold` text.
const ADAMINA_REGULAR: &[u8] = include_bytes!("../assets/fonts/Adamina-Regular.ttf");

/// Registers the embedded Adamina family and sets it as the UI font, plus a
/// distinct accent color, so the app doesn't rely on the platform's generic
/// UI font and neutral-gray default theme.
fn apply_visual_identity(cx: &mut App) {
    if let Err(error) = cx
        .text_system()
        .add_fonts(vec![std::borrow::Cow::Borrowed(ADAMINA_REGULAR)])
    {
        eprintln!("failed to register Adamina font: {error}");
    }

    let theme = cx.global_mut::<Theme>();
    theme.font_family = "Adamina".into();
    let accent: gpui_kit::Hsla = rgb(0x3B82F6).into();
    let accent_hover: gpui_kit::Hsla = rgb(0x2563EB).into();
    let accent_active: gpui_kit::Hsla = rgb(0x1D4ED8).into();
    let white = gpui_kit::white();
    theme.colors.primary = accent;
    theme.colors.primary_hover = accent_hover;
    theme.colors.primary_active = accent_active;
    theme.colors.primary_foreground = white;
    theme.colors.button_primary = accent;
    theme.colors.button_primary_hover = accent_hover;
    theme.colors.button_primary_active = accent_active;
    theme.colors.button_primary_foreground = white;
    theme.colors.ring = accent;
    theme.colors.selection = accent.opacity(0.25);

    // `tokens` is a legacy snapshot of `colors` taken at construction time,
    // not re-derived on mutation (that's what Button/Switch actually read
    // for paint colors, e.g. `cx.theme().tokens.button_primary`).
    theme.tokens = ThemeTokens::from(&theme.colors);

    // Radius/scrollbar/typography reach rendering through a separately
    // mirrored Base layer that only `Theme::sync_base` re-derives.
    Theme::sync_base(cx);
}

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
        ..TitleBar::window_options()
    }
}

fn workspace_window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(960.), px(640.)), cx)),
        window_min_size: Some(size(px(760.), px(520.))),
        ..TitleBar::window_options()
    }
}

fn agent_window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        window_bounds: Some(WindowBounds::centered(size(px(520.), px(500.)), cx)),
        window_min_size: Some(size(px(460.), px(460.))),
        ..WindowOptions::default()
    }
}

/// Fixed width for the settings window; only height varies per pane.
const SETTINGS_WINDOW_WIDTH: gpui_kit::Pixels = px(620.);

fn settings_window_options(cx: &App) -> WindowOptions {
    WindowOptions {
        titlebar: Some(gpui_kit::TitlebarOptions {
            title: Some("Settings".into()),
            ..Default::default()
        }),
        window_bounds: Some(WindowBounds::centered(
            size(
                SETTINGS_WINDOW_WIDTH,
                SettingsWindow::pane_target_height(SettingsTab::General),
            ),
            cx,
        )),
        window_min_size: Some(size(px(480.), px(320.))),
        ..WindowOptions::default()
    }
}

/// Opens the settings window, or brings it forward if already open.
fn open_settings_window(
    handle: &Rc<RefCell<Option<AnyWindowHandle>>>,
    settings: knot_core::Settings,
    cx: &mut App,
) {
    if let Some(existing) = *handle.borrow()
        && existing
            .update(cx, |_, window, _| window.activate_window())
            .is_ok()
    {
        return;
    }
    let options = settings_window_options(cx);
    match cx.open_window(options, move |window, cx| {
        let selected_agent_type = "claude".to_string();
        let initial_options = settings
            .agent_options
            .get(&selected_agent_type)
            .cloned()
            .unwrap_or_default();
        let agent_options_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Extra CLI options")
                .default_value(initial_options)
        });
        let ai_api_key_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("API key")
                .default_value(settings.ai_api_key.clone())
        });
        let autopilot_custom_prompt_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Custom prompt")
                .default_value(settings.autopilot_custom_prompt.clone())
        });
        let mcp_port_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Port")
                .default_value(settings.mcp_server_port.to_string())
        });
        let terminal_font_size_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Size")
                .default_value(settings.terminal_font_size.to_string())
        });
        let view = cx.new(|cx| {
            let agent_options_subscription = cx.subscribe(
                &agent_options_input,
                |this: &mut SettingsWindow, _, event, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.save_agent_options(cx);
                    }
                },
            );
            let ai_api_key_subscription = cx.subscribe(
                &ai_api_key_input,
                |this: &mut SettingsWindow, _, event, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.save_ai_api_key(cx);
                    }
                },
            );
            let autopilot_custom_prompt_subscription = cx.subscribe(
                &autopilot_custom_prompt_input,
                |this: &mut SettingsWindow, _, event, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.save_autopilot_custom_prompt(cx);
                    }
                },
            );
            let mcp_port_subscription = cx.subscribe(
                &mcp_port_input,
                |this: &mut SettingsWindow, _, event, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.save_mcp_port(cx);
                    }
                },
            );
            let terminal_font_size_subscription = cx.subscribe(
                &terminal_font_size_input,
                |this: &mut SettingsWindow, _, event, cx| {
                    if matches!(event, InputEvent::Change) {
                        this.save_terminal_font_size(cx);
                    }
                },
            );
            SettingsWindow {
                settings,
                selected_tab: SettingsTab::General,
                selected_agent_type,
                mcp_selected_agent_type: "claude".to_string(),
                agent_options_input,
                ai_api_key_input,
                autopilot_custom_prompt_input,
                mcp_port_input,
                terminal_font_size_input,
                _agent_options_subscription: agent_options_subscription,
                _ai_api_key_subscription: ai_api_key_subscription,
                _autopilot_custom_prompt_subscription: autopilot_custom_prompt_subscription,
                _mcp_port_subscription: mcp_port_subscription,
                _terminal_font_size_subscription: terminal_font_size_subscription,
            }
        });
        #[cfg(target_os = "macos")]
        {
            let settings_window = view.clone();
            cx.spawn(async move |cx| {
                loop {
                    cx.background_executor()
                        .timer(Duration::from_millis(300))
                        .await;
                    if let Some(family) = native_font_panel::poll_selection() {
                        cx.update(|app| {
                            settings_window.update(app, |view, cx| {
                                view.settings.terminal_font_name = family;
                                view.persist();
                                cx.notify();
                            });
                        });
                    }
                }
            })
            .detach();
        }
        cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
    }) {
        Ok(window) => *handle.borrow_mut() = Some(window.into()),
        Err(error) => eprintln!("failed to open settings window: {error}"),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SettingsTab {
    General,
    Coding,
    Personas,
    Autopilot,
    Voice,
    Mcp,
    Terminal,
}

impl SettingsTab {
    const ALL: [SettingsTab; 7] = [
        SettingsTab::General,
        SettingsTab::Coding,
        SettingsTab::Personas,
        SettingsTab::Autopilot,
        SettingsTab::Voice,
        SettingsTab::Mcp,
        SettingsTab::Terminal,
    ];

    fn label(self) -> &'static str {
        match self {
            SettingsTab::General => "General",
            SettingsTab::Coding => "Coding",
            SettingsTab::Personas => "Personas",
            SettingsTab::Autopilot => "Autopilot",
            SettingsTab::Voice => "Voice",
            SettingsTab::Mcp => "MCP",
            SettingsTab::Terminal => "Terminal",
        }
    }
}

struct SettingsWindow {
    settings: knot_core::Settings,
    selected_tab: SettingsTab,
    selected_agent_type: String,
    mcp_selected_agent_type: String,
    agent_options_input: Entity<InputState>,
    ai_api_key_input: Entity<InputState>,
    autopilot_custom_prompt_input: Entity<InputState>,
    mcp_port_input: Entity<InputState>,
    terminal_font_size_input: Entity<InputState>,
    _agent_options_subscription: Subscription,
    _ai_api_key_subscription: Subscription,
    _autopilot_custom_prompt_subscription: Subscription,
    _mcp_port_subscription: Subscription,
    _terminal_font_size_subscription: Subscription,
}

impl SettingsWindow {
    fn persist(&self) {
        if let Err(error) = self.settings.persist() {
            eprintln!("failed to persist settings: {error}");
        }
    }

    /// Right-aligned label column width shared by every settings row, so
    /// labels line up across a pane regardless of their length.
    const LABEL_WIDTH: f32 = 200.;

    /// A titled, bordered card grouping related controls. The title is
    /// deliberately larger than row content (`text_lg` vs. the default
    /// `text_base` used by row labels/controls) - a section header should
    /// never read smaller than what it's heading.
    fn group(title: &'static str) -> GroupBox {
        GroupBox::new()
            .outline()
            .title(div().text_lg().font_semibold().child(title))
    }

    /// A label + control row with the label right-aligned in a fixed-width
    /// column, matching the alignment convention already used by
    /// `AgentEditor`/`PersonaEditor`.
    fn row(label: &'static str, control: impl IntoElement) -> impl IntoElement {
        h_flex()
            .gap_3()
            .items_center()
            .child(
                div()
                    .w(px(Self::LABEL_WIDTH))
                    .flex_shrink_0()
                    .text_right()
                    .child(label),
            )
            .child(control)
    }

    /// Like `row`, but baseline-aligned instead of center-aligned - for rows
    /// whose control is itself text (a read-only value, not a switch/button/
    /// input), so the value's text baseline lines up with the label's.
    fn text_row(label: &'static str, control: impl IntoElement) -> impl IntoElement {
        h_flex()
            .gap_3()
            .items_baseline()
            .child(
                div()
                    .w(px(Self::LABEL_WIDTH))
                    .flex_shrink_0()
                    .text_right()
                    .child(label),
            )
            .child(control)
    }

    /// Muted description text lined up under a row's *control* column,
    /// not spanning the full card width - it explains the control above
    /// it, not the section as a whole.
    fn hint(cx: &Context<Self>, text: &'static str) -> impl IntoElement {
        h_flex()
            .gap_3()
            .child(div().w(px(Self::LABEL_WIDTH)).flex_shrink_0())
            .child(
                div()
                    .flex_1()
                    .min_w_0()
                    .text_sm()
                    .whitespace_normal()
                    .text_color(cx.theme().muted_foreground)
                    .child(text),
            )
    }

    /// Renders `text` in the theme's monospace font, for values that are
    /// literally code/commands/identifiers (install commands, model names).
    fn mono_text(cx: &Context<Self>, text: impl Into<gpui_kit::SharedString>) -> gpui_kit::Div {
        div()
            .text_sm()
            .font_family(cx.theme().mono_font_family.clone())
            .child(text.into())
    }

    /// A small icon-only action button with a tooltip, used for utility
    /// actions (choose/clear/add/edit/delete/copy) instead of a text label -
    /// text buttons read as arbitrary activators, an icon reads as what it
    /// does. `danger` tints destructive actions (clear/delete) red.
    fn icon_button(
        id: impl Into<gpui_kit::ElementId>,
        icon_path: &'static str,
        tooltip: &'static str,
        danger: bool,
    ) -> Button {
        let mut icon = Icon::default().path(icon_path);
        if danger {
            // `.ghost()` and `.danger()` are both button *variants* - only one
            // can apply, and ghost (no background) is what we want here - so
            // tint the icon itself red instead of switching variants.
            icon = icon.text_color(rgb(0xEF4444));
        }
        Button::new(id).icon(icon).tooltip(tooltip).ghost().small()
    }

    fn appearance_label(mode: &str) -> &'static str {
        match mode {
            "system" => "System",
            "light" => "Light",
            "dark" => "Dark",
            _ => "Auto",
        }
    }

    /// "Restore last conversation" only takes effect when layout restore is
    /// on; gate its toggle on that rather than hiding it.
    fn restore_conversation_toggle_enabled(restore_layout_on_launch: bool) -> bool {
        restore_layout_on_launch
    }

    fn agent_type_label(agent_type: &str) -> &'static str {
        match agent_type {
            "codex" => "Codex",
            "opencode" => "OpenCode",
            "gemini" => "Gemini",
            "copilot" => "Copilot",
            "shell" => "Shell",
            _ => "Claude",
        }
    }

    fn choose_source_folder(&mut self, cx: &mut Context<Self>) {
        let receiver = cx.prompt_for_paths(PathPromptOptions {
            files: false,
            directories: true,
            multiple: false,
            prompt: Some("Choose Source Folder".into()),
        });
        let settings_window = cx.entity();
        cx.spawn(async move |_this, cx| {
            let Ok(Ok(Some(paths))) = receiver.await else {
                return;
            };
            let Some(path) = paths.into_iter().next() else {
                return;
            };
            cx.update(|app| {
                settings_window.update(app, |view, cx| {
                    view.settings.source_base_folder = path.to_string_lossy().into_owned();
                    view.persist();
                    cx.notify();
                });
            });
        })
        .detach();
    }

    fn clear_source_folder(&mut self, cx: &mut Context<Self>) {
        self.settings.source_base_folder.clear();
        self.persist();
        cx.notify();
    }

    fn select_agent_type(&mut self, agent_type: &str, window: &mut Window, cx: &mut Context<Self>) {
        self.selected_agent_type = agent_type.to_string();
        let value = self
            .settings
            .agent_options
            .get(agent_type)
            .cloned()
            .unwrap_or_default();
        cx.update_entity(&self.agent_options_input, |input, input_cx| {
            input.set_value(value, window, input_cx);
        });
        cx.notify();
    }

    fn save_agent_options(&mut self, cx: &mut Context<Self>) {
        let value = self.agent_options_input.read(cx).value().to_string();
        self.settings
            .agent_options
            .insert(self.selected_agent_type.clone(), value);
        self.persist();
    }

    fn ai_provider_label(provider: &str) -> &'static str {
        match provider {
            "anthropic" => "Anthropic",
            "google" => "Google",
            _ => "OpenAI",
        }
    }

    /// Hardcoded model for each AI provider (cheapest/fastest options),
    /// matching the Swift reference's `AppSettings.aiModel(for:)`.
    fn ai_model_for(provider: &str) -> &'static str {
        match provider {
            "anthropic" => "claude-haiku-4-5",
            "google" => "gemini-flash-lite-latest",
            "openai" => "gpt-5-mini",
            _ => "",
        }
    }

    fn autopilot_action_label(action: &str) -> &'static str {
        match action {
            "ask" => "Ask me",
            "continue" => "Auto-continue",
            "custom" => "Custom",
            _ => "Mark conversation",
        }
    }

    fn autopilot_action_description(action: &str) -> &'static str {
        match action {
            "ask" => "Show a dialog letting you switch to the agent, dismiss, or auto-continue.",
            "continue" => "Automatically send \"yes, continue\" to the agent.",
            "custom" => {
                "Use your own prompt to decide what to reply. The LLM response is injected \
                 directly into the agent."
            }
            _ => "Set the agent status to indicate input is needed and send a notification.",
        }
    }

    fn select_ai_provider(&mut self, provider: &str, cx: &mut Context<Self>) {
        self.settings.ai_provider = provider.to_string();
        self.persist();
        cx.notify();
    }

    fn select_autopilot_action(&mut self, action: &str, cx: &mut Context<Self>) {
        self.settings.autopilot_action = action.to_string();
        self.persist();
        cx.notify();
    }

    fn save_ai_api_key(&mut self, cx: &mut Context<Self>) {
        self.settings.ai_api_key = self.ai_api_key_input.read(cx).value().to_string();
        self.persist();
    }

    fn save_autopilot_custom_prompt(&mut self, cx: &mut Context<Self>) {
        self.settings.autopilot_custom_prompt = self
            .autopilot_custom_prompt_input
            .read(cx)
            .value()
            .to_string();
        self.persist();
    }

    /// Display name for a modifier key code, ported from the Swift
    /// reference's `ModifierKeyCode.name(for:)`.
    fn key_name_for_code(code: i32) -> String {
        match code {
            54 => "Right Command",
            55 => "Left Command",
            56 => "Left Shift",
            57 => "Caps Lock",
            58 => "Left Option",
            59 => "Left Control",
            60 => "Right Shift",
            61 => "Right Option",
            62 => "Right Control",
            63 => "Fn",
            _ => return format!("Key {code}"),
        }
        .to_string()
    }

    fn mcp_server_url(port: u16) -> String {
        format!("http://127.0.0.1:{port}")
    }

    /// The command to copy for registering `agent_type` against Knot's MCP
    /// server, ported from the Swift reference's
    /// `MCPCommandView.mcpCommandCopy` (Skwad -> Knot renamed).
    fn mcp_install_command(agent_type: &str, url: &str) -> String {
        match agent_type {
            "claude" => format!("claude mcp add --transport http --scope user knot {url}"),
            "codex" => format!("codex mcp add knot --url {url}"),
            "opencode" => "opencode mcp add".to_string(),
            "gemini" => format!("gemini mcp add --transport http knot {url} --scope user"),
            _ => String::new(),
        }
    }

    fn save_mcp_port(&mut self, cx: &mut Context<Self>) {
        let value = self.mcp_port_input.read(cx).value().to_string();
        if let Ok(port) = value.parse::<u16>() {
            self.settings.mcp_server_port = port;
            self.persist();
        }
    }

    fn select_mcp_agent_type(&mut self, agent_type: &str, cx: &mut Context<Self>) {
        self.mcp_selected_agent_type = agent_type.to_string();
        cx.notify();
    }

    /// Monospace font shortlist ported from the Swift reference's
    /// `TerminalSettingsView.monospaceFonts`, without the availability
    /// filter (no font-enumeration API is surfaced through `gpui-kit`).
    const TERMINAL_FONTS: [&'static str; 11] = [
        "SF Mono",
        "Menlo",
        "Monaco",
        "Courier New",
        "Andale Mono",
        "JetBrains Mono",
        "Fira Code",
        "Source Code Pro",
        "IBM Plex Mono",
        "Hack",
        "Inconsolata",
    ];

    fn select_terminal_font(&mut self, font_name: &str, cx: &mut Context<Self>) {
        self.settings.terminal_font_name = font_name.to_string();
        self.persist();
        cx.notify();
    }

    fn save_terminal_font_size(&mut self, cx: &mut Context<Self>) {
        let value = self.terminal_font_size_input.read(cx).value().to_string();
        if let Ok(size) = value.parse::<f64>() {
            self.settings.terminal_font_size = size;
            self.persist();
        }
    }

    /// Truncates `instructions` to `max_chars`, appending an ellipsis when
    /// truncated so a persona list row stays a single line.
    fn persona_preview(instructions: &str, max_chars: usize) -> String {
        let truncated: String = instructions.chars().take(max_chars).collect();
        if instructions.chars().count() > max_chars {
            format!("{truncated}…")
        } else {
            truncated
        }
    }

    fn delete_persona(&mut self, id: Uuid, cx: &mut Context<Self>) {
        if let Err(error) = self.settings.remove_persona(id) {
            eprintln!("failed to remove persona: {error}");
        }
        cx.notify();
    }

    fn restore_default_personas(&mut self, cx: &mut Context<Self>) {
        if let Err(error) = self.settings.restore_default_personas() {
            eprintln!("failed to restore default personas: {error}");
        }
        cx.notify();
    }
}

impl SettingsWindow {
    /// Target window height for each pane's content, capped so a long
    /// Personas list can't push the window arbitrarily tall - it scrolls
    /// within the cap instead (see `render` below).
    fn pane_target_height(tab: SettingsTab) -> gpui_kit::Pixels {
        match tab {
            SettingsTab::General => px(560.),
            SettingsTab::Coding => px(440.),
            SettingsTab::Personas => px(600.),
            SettingsTab::Autopilot => px(660.),
            SettingsTab::Voice => px(520.),
            SettingsTab::Mcp => px(600.),
            SettingsTab::Terminal => px(380.),
        }
    }

    fn render_tab_strip(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let settings_window = cx.entity();
        let selected_index = SettingsTab::ALL
            .iter()
            .position(|tab| *tab == self.selected_tab)
            .unwrap_or(0);
        TabBar::new("settings-tabs")
            .underline()
            .selected_index(selected_index)
            .children(SettingsTab::ALL.map(|tab| Tab::new().label(tab.label())))
            .on_click(move |index, window, app| {
                let tab = SettingsTab::ALL[*index];
                settings_window.update(app, |view, cx| {
                    view.selected_tab = tab;
                    cx.notify();
                });
                window.resize(size(SETTINGS_WINDOW_WIDTH, Self::pane_target_height(tab)));
            })
    }

    fn render_general(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let settings_window = cx.entity();
        let restore_layout_on_launch = self.settings.restore_layout_on_launch;
        let restore_conversation_on_launch = self.settings.restore_conversation_on_launch;
        let keep_in_menu_bar = self.settings.keep_in_menu_bar;
        let desktop_notifications_enabled = self.settings.desktop_notifications_enabled;
        let appearance_label = Self::appearance_label(&self.settings.appearance_mode);

        v_flex()
            .gap_3()
            .child(
                Self::group("Appearance")
                    .child(Self::row(
                        "Appearance",
                        Button::new("appearance-picker")
                            .label(appearance_label)
                            .dropdown_caret(true)
                            .dropdown_menu({
                                let settings_window = settings_window.clone();
                                move |menu, _, _| {
                                    let mut menu = menu;
                                    for (label, value) in [
                                        ("Auto", "auto"),
                                        ("System", "system"),
                                        ("Light", "light"),
                                        ("Dark", "dark"),
                                    ] {
                                        menu = menu.item(PopupMenuItem::new(label).on_click({
                                            let settings_window = settings_window.clone();
                                            move |_, _, app| {
                                                settings_window.update(app, |view, _| {
                                                    view.settings.appearance_mode =
                                                        value.to_string();
                                                    view.persist();
                                                })
                                            }
                                        }));
                                    }
                                    menu
                                }
                            }),
                    ))
                    .child(Self::hint(
                        cx,
                        "Derives color scheme from terminal background color.",
                    )),
            )
            .child(
                Self::group("Startup")
                    .child(Self::row(
                        "Restore agents on launch",
                        Switch::new("restore-layout-on-launch")
                            .checked(restore_layout_on_launch)
                            .on_click({
                                let settings_window = settings_window.clone();
                                move |checked, _, app| {
                                    let checked = *checked;
                                    settings_window.update(app, |view, _| {
                                        view.settings.restore_layout_on_launch = checked;
                                        view.persist();
                                    })
                                }
                            }),
                    ))
                    .child(Self::row(
                        "Restore last conversation",
                        Switch::new("restore-conversation-on-launch")
                            .checked(restore_conversation_on_launch)
                            .disabled(!Self::restore_conversation_toggle_enabled(
                                restore_layout_on_launch,
                            ))
                            .on_click({
                                let settings_window = settings_window.clone();
                                move |checked, _, app| {
                                    let checked = *checked;
                                    settings_window.update(app, |view, _| {
                                        view.settings.restore_conversation_on_launch = checked;
                                        view.persist();
                                    })
                                }
                            }),
                    ))
                    .child(Self::row(
                        "Keep running in menu bar when closed",
                        Switch::new("keep-in-menu-bar")
                            .checked(keep_in_menu_bar)
                            .on_click({
                                let settings_window = settings_window.clone();
                                move |checked, _, app| {
                                    let checked = *checked;
                                    settings_window.update(app, |view, _| {
                                        view.settings.keep_in_menu_bar = checked;
                                        view.persist();
                                    })
                                }
                            }),
                    )),
            )
            .child(
                Self::group("Notifications").child(Self::row(
                    "Desktop notifications",
                    Switch::new("desktop-notifications-enabled")
                        .checked(desktop_notifications_enabled)
                        .on_click({
                            let settings_window = settings_window.clone();
                            move |checked, _, app| {
                                let checked = *checked;
                                settings_window.update(app, |view, _| {
                                    view.settings.desktop_notifications_enabled = checked;
                                    view.persist();
                                })
                            }
                        }),
                )),
            )
    }

    fn render_coding(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let settings_window = cx.entity();
        let source_base_folder = self.settings.source_base_folder.clone();
        let folder_label = if source_base_folder.is_empty() {
            "Not configured".to_string()
        } else {
            source_base_folder
        };
        let agent_type_label = Self::agent_type_label(&self.selected_agent_type);

        v_flex()
            .gap_3()
            .child(
                Self::group("Source Folder").child(Self::row(
                    "Folder",
                    h_flex()
                        .flex_1()
                        .justify_between()
                        .child(div().text_sm().child(folder_label))
                        .child(
                            h_flex()
                                .gap_1()
                                .child(
                                    Self::icon_button(
                                        "coding-choose-source-folder",
                                        "icons/folder-open.svg",
                                        "Choose source folder",
                                        false,
                                    )
                                    .on_click({
                                        let settings_window = settings_window.clone();
                                        move |_, _, app| {
                                            settings_window.update(app, |view, cx| {
                                                view.choose_source_folder(cx);
                                            })
                                        }
                                    }),
                                )
                                .child(
                                    Self::icon_button(
                                        "coding-clear-source-folder",
                                        "icons/x.svg",
                                        "Clear source folder",
                                        true,
                                    )
                                    .on_click({
                                        let settings_window = settings_window.clone();
                                        move |_, window, app| {
                                            let settings_window = settings_window.clone();
                                            window.open_alert_dialog(app, move |alert, _, _| {
                                                let settings_window = settings_window.clone();
                                                alert
                                                    .title("Clear Source Folder")
                                                    .description(
                                                        "The source folder path will be cleared.",
                                                    )
                                                    .confirm()
                                                    .on_ok(move |_, _, app| {
                                                        settings_window.update(app, |view, cx| {
                                                            view.clear_source_folder(cx);
                                                        });
                                                        true
                                                    })
                                            });
                                        }
                                    }),
                                ),
                        ),
                )),
            )
            .child(
                Self::group("Agent Options")
                    .child(Self::row(
                        "Coding agent",
                        Button::new("coding-agent-type-picker")
                            .label(agent_type_label)
                            .dropdown_caret(true)
                            .dropdown_menu({
                                let settings_window = settings_window.clone();
                                move |menu, _, _| {
                                    let mut menu = menu;
                                    for (label, value) in [
                                        ("Claude", "claude"),
                                        ("Codex", "codex"),
                                        ("OpenCode", "opencode"),
                                        ("Gemini", "gemini"),
                                        ("Copilot", "copilot"),
                                        ("Shell", "shell"),
                                    ] {
                                        menu = menu.item(PopupMenuItem::new(label).on_click({
                                            let settings_window = settings_window.clone();
                                            move |_, window, app| {
                                                settings_window.update(app, |view, cx| {
                                                    view.select_agent_type(value, window, cx);
                                                })
                                            }
                                        }));
                                    }
                                    menu
                                }
                            }),
                    ))
                    .child(Self::row(
                        "Options",
                        Input::new(&self.agent_options_input).flex_1(),
                    )),
            )
    }

    fn render_personas(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let settings_window = cx.entity();
        let personas: Vec<knot_core::Persona> = self
            .settings
            .active_personas()
            .into_iter()
            .cloned()
            .collect();

        let list =
            if personas.is_empty() {
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child("No personas defined.")
                    .into_any_element()
            } else {
                v_flex()
                    .gap_3()
                    .children(personas.into_iter().enumerate().map(|(index, persona)| {
                        let id = persona.id;
                        let preview = Self::persona_preview(&persona.instructions, 80);
                        h_flex()
                            .justify_between()
                            .items_center()
                            .gap_2()
                            .child(
                                v_flex()
                                    .flex_1()
                                    .min_w_0()
                                    .child(div().child(persona.name.clone()))
                                    .child(
                                        div()
                                            .text_sm()
                                            .text_color(cx.theme().muted_foreground)
                                            .child(preview),
                                    ),
                            )
                            .child(
                                h_flex()
                                    .flex_shrink_0()
                                    .gap_1()
                                    .child(
                                        Self::icon_button(
                                            ("persona-edit", index),
                                            "icons/pencil.svg",
                                            "Edit persona",
                                            false,
                                        )
                                        .on_click({
                                            let parent = settings_window.downgrade();
                                            let persona = persona.clone();
                                            move |_, _, app| {
                                                open_persona_editor(
                                                    parent.clone(),
                                                    Some(persona.clone()),
                                                    app,
                                                );
                                            }
                                        }),
                                    )
                                    .child(
                                        Self::icon_button(
                                            ("persona-delete", index),
                                            "icons/trash.svg",
                                            "Delete persona",
                                            true,
                                        )
                                        .on_click({
                                            let settings_window = settings_window.clone();
                                            let name = persona.name.clone();
                                            move |_, window, app| {
                                                let settings_window = settings_window.clone();
                                                window.open_alert_dialog(app, {
                                                    let name = name.clone();
                                                    move |alert, _, _| {
                                                        let settings_window =
                                                            settings_window.clone();
                                                        alert
                                                        .title("Delete Persona")
                                                        .description(format!(
                                                            "This permanently deletes \"{name}\". \
                                                             This can't be undone."
                                                        ))
                                                        .confirm()
                                                        .on_ok(move |_, _, app| {
                                                            settings_window.update(app, |view, cx| {
                                                                view.delete_persona(id, cx);
                                                            });
                                                            true
                                                        })
                                                    }
                                                });
                                            }
                                        }),
                                    ),
                            )
                    }))
                    .into_any_element()
            };
        // Bounded so the list scrolls in place instead of pushing the group's
        // title/action row (which must stay visible) off the top of the
        // window - same cap philosophy as the window's own per-pane height.
        let list = div()
            .id("personas-list")
            .max_h(px(420.))
            .overflow_y_scroll()
            .child(list);

        v_flex().gap_3().child(
            Self::group("Personas")
                .child(
                    h_flex()
                        .justify_between()
                        .child(
                            Self::icon_button(
                                "personas-add",
                                "icons/plus.svg",
                                "Add Persona…",
                                false,
                            )
                            .on_click({
                                let parent = settings_window.downgrade();
                                move |_, _, app| {
                                    open_persona_editor(parent.clone(), None, app);
                                }
                            }),
                        )
                        .child(
                            Self::icon_button(
                                "personas-restore-defaults",
                                "icons/rotate-ccw.svg",
                                "Restore Defaults",
                                false,
                            )
                            .on_click({
                                let settings_window = settings_window.clone();
                                move |_, window, app| {
                                    let settings_window = settings_window.clone();
                                    window.open_alert_dialog(app, move |alert, _, _| {
                                        let settings_window = settings_window.clone();
                                        alert
                                            .title("Restore Defaults")
                                            .description(
                                                "Resets built-in personas to their \
                                                     original name and instructions. \
                                                     Personas you created are not affected.",
                                            )
                                            .confirm()
                                            .on_ok(move |_, _, app| {
                                                settings_window.update(app, |view, cx| {
                                                    view.restore_default_personas(cx);
                                                });
                                                true
                                            })
                                    });
                                }
                            }),
                        ),
                )
                .child(list),
        )
    }

    fn render_autopilot(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let settings_window = cx.entity();
        let autopilot_enabled = self.settings.autopilot_enabled;
        let ai_provider = self.settings.ai_provider.clone();
        let autopilot_action = self.settings.autopilot_action.clone();
        let provider_label = Self::ai_provider_label(&ai_provider);
        let model_name = Self::ai_model_for(&ai_provider);
        let action_label = Self::autopilot_action_label(&autopilot_action);
        let is_custom_action = autopilot_action == "custom";

        v_flex()
            .gap_3()
            .child(
                Self::group("Enable")
                    .child(Self::row(
                        "Enable autopilot",
                        Switch::new("autopilot-enabled")
                            .checked(autopilot_enabled)
                            .on_click({
                                let settings_window = settings_window.clone();
                                move |checked, _, app| {
                                    let checked = *checked;
                                    settings_window.update(app, |view, _| {
                                        view.settings.autopilot_enabled = checked;
                                        view.persist();
                                    })
                                }
                            }),
                    ))
                    .child(Self::hint(
                        cx,
                        "Automatically detect when agents need input and take action — no need \
                         to babysit your agents. Only available with Claude Code.",
                    )),
            )
            .child(
                Self::group("AI Provider")
                    .child(Self::row(
                        "Provider",
                        Button::new("autopilot-provider-picker")
                            .label(provider_label)
                            .dropdown_caret(true)
                            .dropdown_menu({
                                let settings_window = settings_window.clone();
                                move |menu, _, _| {
                                    let mut menu = menu;
                                    for (label, value) in [
                                        ("OpenAI", "openai"),
                                        ("Anthropic", "anthropic"),
                                        ("Google", "google"),
                                    ] {
                                        menu = menu.item(PopupMenuItem::new(label).on_click({
                                            let settings_window = settings_window.clone();
                                            move |_, _, app| {
                                                settings_window.update(app, |view, cx| {
                                                    view.select_ai_provider(value, cx);
                                                })
                                            }
                                        }));
                                    }
                                    menu
                                }
                            }),
                    ))
                    .child(Self::row(
                        "API Key",
                        Input::new(&self.ai_api_key_input)
                            .font_family(cx.theme().mono_font_family.clone())
                            .flex_1(),
                    ))
                    .child(Self::text_row(
                        "Model",
                        Self::mono_text(cx, model_name).text_color(cx.theme().muted_foreground),
                    )),
            )
            .child(
                Self::group("Action")
                    .child(Self::row(
                        "When input is detected",
                        Button::new("autopilot-action-picker")
                            .label(action_label)
                            .dropdown_caret(true)
                            .dropdown_menu({
                                let settings_window = settings_window.clone();
                                move |menu, _, _| {
                                    let mut menu = menu;
                                    for (label, value) in [
                                        ("Mark conversation", "mark"),
                                        ("Ask me", "ask"),
                                        ("Auto-continue", "continue"),
                                        ("Custom", "custom"),
                                    ] {
                                        menu = menu.item(PopupMenuItem::new(label).on_click({
                                            let settings_window = settings_window.clone();
                                            move |_, _, app| {
                                                settings_window.update(app, |view, cx| {
                                                    view.select_autopilot_action(value, cx);
                                                })
                                            }
                                        }));
                                    }
                                    menu
                                }
                            }),
                    ))
                    .children(is_custom_action.then(|| {
                        Self::row(
                            "Custom prompt",
                            Input::new(&self.autopilot_custom_prompt_input).flex_1(),
                        )
                        .into_any_element()
                    }))
                    .children((!is_custom_action).then(|| {
                        Self::hint(cx, Self::autopilot_action_description(&autopilot_action))
                            .into_any_element()
                    })),
            )
    }

    fn render_voice(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let settings_window = cx.entity();
        let voice_enabled = self.settings.voice_enabled;
        let voice_auto_insert = self.settings.voice_auto_insert;
        let key_name = Self::key_name_for_code(self.settings.voice_push_to_talk_key);

        v_flex()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Voice input allows you to speak commands to your agents using \
                     push-to-talk. Hold the configured key to record, release to stop.",
                    ),
            )
            .child(
                Self::group("Engine")
                    .child(Self::row(
                        "Enable voice input",
                        Switch::new("voice-enabled")
                            .checked(voice_enabled)
                            .on_click({
                                let settings_window = settings_window.clone();
                                move |checked, _, app| {
                                    let checked = *checked;
                                    settings_window.update(app, |view, _| {
                                        view.settings.voice_enabled = checked;
                                        view.persist();
                                    })
                                }
                            }),
                    ))
                    .child(Self::row(
                        "Engine",
                        Button::new("voice-engine-picker")
                            .label("Apple SpeechAnalyzer")
                            .disabled(true),
                    ))
                    .child(Self::hint(
                        cx,
                        "Uses on-device speech recognition. No data is sent to the cloud.",
                    )),
            )
            .child(
                Self::group("Input")
                    .child(Self::row(
                        "Push-to-Talk Key",
                        div()
                            .text_sm()
                            .text_color(cx.theme().muted_foreground)
                            .opacity(if voice_enabled { 1.0 } else { 0.5 })
                            .child(key_name),
                    ))
                    .child(Self::row(
                        "Auto-insert transcription",
                        Switch::new("voice-auto-insert")
                            .checked(voice_auto_insert)
                            .disabled(!voice_enabled)
                            .on_click({
                                let settings_window = settings_window.clone();
                                move |checked, _, app| {
                                    let checked = *checked;
                                    settings_window.update(app, |view, _| {
                                        view.settings.voice_auto_insert = checked;
                                        view.persist();
                                    })
                                }
                            }),
                    ))
                    .child(Self::hint(
                        cx,
                        if voice_auto_insert {
                            "Transcribed text will be automatically inserted into the terminal."
                        } else {
                            "Transcribed text will be shown in a popup for review before \
                             insertion."
                        },
                    )),
            )
    }

    fn render_mcp(&self, cx: &mut Context<Self>) -> impl IntoElement {
        let settings_window = cx.entity();
        let mcp_server_enabled = self.settings.mcp_server_enabled;
        let server_url = Self::mcp_server_url(self.settings.mcp_server_port);
        let agent_type_label = Self::agent_type_label(&self.mcp_selected_agent_type);
        let install_command = Self::mcp_install_command(&self.mcp_selected_agent_type, &server_url);

        v_flex()
            .gap_3()
            .child(
                div()
                    .text_sm()
                    .text_color(cx.theme().muted_foreground)
                    .child(
                        "Knot runs a local MCP server so coding agents can coordinate with each \
                     other and control the app.",
                    ),
            )
            .child(
                Self::group("Server Settings")
                    .child(Self::row(
                        "Enable MCP server",
                        Switch::new("mcp-server-enabled")
                            .checked(mcp_server_enabled)
                            .on_click({
                                let settings_window = settings_window.clone();
                                move |checked, _, app| {
                                    let checked = *checked;
                                    settings_window.update(app, |view, _| {
                                        view.settings.mcp_server_enabled = checked;
                                        view.persist();
                                    })
                                }
                            }),
                    ))
                    .child(Self::row(
                        "Port",
                        Input::new(&self.mcp_port_input).w(px(100.)),
                    ))
                    .child(Self::text_row(
                        "URL",
                        h_flex()
                            .gap_2()
                            .items_center()
                            .child(
                                Self::mono_text(cx, server_url.clone())
                                    .text_color(cx.theme().muted_foreground),
                            )
                            .child(
                                Self::icon_button(
                                    "mcp-copy-url",
                                    "icons/copy.svg",
                                    "Copy URL",
                                    false,
                                )
                                .on_click({
                                    let server_url = server_url.clone();
                                    move |_, _, app| {
                                        app.write_to_clipboard(ClipboardItem::new_string(
                                            server_url.clone(),
                                        ));
                                    }
                                }),
                            ),
                    )),
            )
            .child(
                Self::group("Installation Command")
                    .child(Self::row(
                        "Agent",
                        Button::new("mcp-agent-type-picker")
                            .label(agent_type_label)
                            .dropdown_caret(true)
                            .dropdown_menu({
                                let settings_window = settings_window.clone();
                                move |menu, _, _| {
                                    let mut menu = menu;
                                    for (label, value) in [
                                        ("Claude", "claude"),
                                        ("Codex", "codex"),
                                        ("OpenCode", "opencode"),
                                        ("Gemini", "gemini"),
                                        ("Copilot", "copilot"),
                                    ] {
                                        menu = menu.item(PopupMenuItem::new(label).on_click({
                                            let settings_window = settings_window.clone();
                                            move |_, _, app| {
                                                settings_window.update(app, |view, cx| {
                                                    view.select_mcp_agent_type(value, cx);
                                                })
                                            }
                                        }));
                                    }
                                    menu
                                }
                            }),
                    ))
                    .child(Self::text_row(
                        "Command",
                        h_flex()
                            .flex_1()
                            .min_w_0()
                            .gap_2()
                            .items_start()
                            .child(if install_command.is_empty() {
                                div()
                                    .flex_1()
                                    .min_w_0()
                                    .text_sm()
                                    .child("No manual setup needed.")
                                    .into_any_element()
                            } else {
                                Self::mono_text(cx, install_command.clone())
                                    .flex_1()
                                    .min_w_0()
                                    .whitespace_normal()
                                    .into_any_element()
                            })
                            .child(
                                Self::icon_button(
                                    "mcp-copy-install-command",
                                    "icons/copy.svg",
                                    "Copy command",
                                    false,
                                )
                                .on_click(move |_, _, app| {
                                    if !install_command.is_empty() {
                                        app.write_to_clipboard(ClipboardItem::new_string(
                                            install_command.clone(),
                                        ));
                                    }
                                }),
                            ),
                    )),
            )
    }

    fn render_terminal(&self, _cx: &mut Context<Self>) -> impl IntoElement {
        let terminal_font_name = self.settings.terminal_font_name.clone();

        v_flex().gap_3().child(
            Self::group("Font")
                .child(Self::row(
                    "Font",
                    Button::new("terminal-font-picker")
                        .label(terminal_font_name.clone())
                        .on_click(move |_, _, _| {
                            // Opens the OS font panel (NSFontPanel); the choice
                            // comes back asynchronously via `poll_selection`.
                            #[cfg(target_os = "macos")]
                            native_font_panel::open(&terminal_font_name);
                        }),
                ))
                .child(Self::row(
                    "Size",
                    Input::new(&self.terminal_font_size_input).w(px(60.)),
                )),
        )
    }
}

fn persona_editor_window_options(title: &'static str, cx: &App) -> WindowOptions {
    WindowOptions {
        titlebar: Some(gpui_kit::TitlebarOptions {
            title: Some(title.into()),
            ..Default::default()
        }),
        window_bounds: Some(WindowBounds::centered(size(px(460.), px(380.)), cx)),
        window_min_size: Some(size(px(400.), px(320.))),
        ..WindowOptions::default()
    }
}

/// Opens the persona add/edit window. `persona` is `None` for "Add Persona…"
/// and `Some` (fields pre-filled) for a row's edit button.
fn open_persona_editor(
    parent: WeakEntity<SettingsWindow>,
    persona: Option<knot_core::Persona>,
    cx: &mut App,
) {
    let editing_id = persona.as_ref().map(|p| p.id);
    let title = if editing_id.is_some() {
        "Edit Persona"
    } else {
        "New Persona"
    };
    let name = persona.as_ref().map(|p| p.name.clone()).unwrap_or_default();
    let instructions = persona
        .as_ref()
        .map(|p| p.instructions.clone())
        .unwrap_or_default();
    let options = persona_editor_window_options(title, cx);
    let _ = cx.open_window(options, move |window, cx| {
        let name_input = cx.new(|cx| {
            InputState::new(window, cx)
                .placeholder("Persona name")
                .default_value(name)
        });
        name_input.update(cx, |state, cx| state.focus(window, cx));
        let instructions_input = cx.new(|cx| {
            TextareaState::new(window, cx)
                .placeholder("Instructions")
                .default_value(instructions)
        });
        let view = cx.new(|_| PersonaEditor {
            parent,
            editing_id,
            name_input,
            instructions_input,
            error: None,
        });
        cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
    });
}

struct PersonaEditor {
    parent: WeakEntity<SettingsWindow>,
    editing_id: Option<Uuid>,
    name_input: Entity<InputState>,
    instructions_input: Entity<TextareaState>,
    error: Option<String>,
}

impl PersonaEditor {
    fn save(&mut self, window: &mut Window, cx: &mut Context<Self>) {
        let name = self.name_input.read(cx).value().trim().to_string();
        if name.is_empty() {
            self.error = Some("Enter a persona name.".to_string());
            cx.notify();
            return;
        }
        let instructions = self.instructions_input.read(cx).value().trim().to_string();
        let Some(parent) = self.parent.upgrade() else {
            window.remove_window();
            return;
        };
        parent.update(cx, |view, view_cx| {
            let result = match self.editing_id {
                Some(id) => view.settings.update_persona(id, name, instructions),
                None => view.settings.add_persona(name, instructions).map(|_| ()),
            };
            if let Err(error) = result {
                eprintln!("failed to save persona: {error}");
            }
            view_cx.notify();
        });
        window.remove_window();
    }
}

impl Render for PersonaEditor {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        v_flex()
            .size_full()
            .gap_3()
            .p_5()
            .bg(cx.theme().background)
            .child(
                h_flex()
                    .gap_2()
                    .child(div().w(px(100.)).text_right().child("Name"))
                    .child(Input::new(&self.name_input).flex_1()),
            )
            .child(
                h_flex()
                    .flex_1()
                    .gap_2()
                    .child(div().w(px(100.)).text_right().child("Instructions"))
                    .child(
                        Textarea::new(&self.instructions_input)
                            .flex_1()
                            .h_full()
                            .font_family(cx.theme().mono_font_family.clone()),
                    ),
            )
            .children(
                self.error
                    .as_ref()
                    .map(|error| div().text_sm().child(error.clone())),
            )
            .child(
                h_flex()
                    .flex_shrink_0()
                    .justify_end()
                    .gap_2()
                    .child(
                        Button::new("cancel-persona-editor")
                            .label("Cancel")
                            .on_click(|_, window, _| window.remove_window()),
                    )
                    .child(
                        Button::new("save-persona-editor")
                            .label("Save")
                            .primary()
                            .on_click(cx.listener(|editor, _, window, cx| editor.save(window, cx))),
                    ),
            )
    }
}

impl Render for SettingsWindow {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let body = match self.selected_tab {
            SettingsTab::General => self.render_general(cx).into_any_element(),
            SettingsTab::Coding => self.render_coding(cx).into_any_element(),
            SettingsTab::Personas => self.render_personas(cx).into_any_element(),
            SettingsTab::Autopilot => self.render_autopilot(cx).into_any_element(),
            SettingsTab::Voice => self.render_voice(cx).into_any_element(),
            SettingsTab::Mcp => self.render_mcp(cx).into_any_element(),
            SettingsTab::Terminal => self.render_terminal(cx).into_any_element(),
        };

        // Personas manages its own scroll region (only the list scrolls, the
        // title/action row stays pinned) - scrolling the body too would let
        // both containers move at once and make the group's title/border
        // appear to drift.
        let mut settings_body = div().id("settings-body").flex_1();
        settings_body = if matches!(self.selected_tab, SettingsTab::Personas) {
            settings_body.overflow_hidden()
        } else {
            settings_body.overflow_y_scroll()
        };

        v_flex()
            .size_full()
            .gap_3()
            .p_4()
            .bg(cx.theme().background)
            .child(self.render_tab_strip(cx))
            .child(settings_body.child(body))
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
                return v_flex()
                    .size_full()
                    .child(TitleBar::new().border_color(gpui_kit::transparent_black()))
                    .child("Workspace no longer exists.");
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

        v_flex()
            .size_full()
            .child(
                TitleBar::new()
                    .border_color(gpui_kit::transparent_black())
                    .child(workspace_name.clone()),
            )
            .child(
                h_flex()
                    .flex_1()
                    .relative()
                    .bg(cx.theme().background)
                    .child(
                        v_flex()
                            .w(px(250.))
                            .h_full()
                            .gap_2()
                            .p_4()
                            .bg(cx.theme().muted)
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
            input.focus(window, input_cx);
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
            .bg(cx.theme().background)
            .child(
                TitleBar::new()
                    .border_color(gpui_kit::transparent_black())
                    .child("Workspaces"),
            )
            .child(
                v_flex()
                    .flex_1()
                    .gap_4()
                    .p_4()
                    .child(
                        h_flex().justify_end().child(
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
                                    .child(div().text_lg().child(
                                        if self.workspace_dialog_id.is_some() {
                                            "Rename Workspace"
                                        } else {
                                            "New Workspace"
                                        },
                                    ))
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
                                                            manager.cancel_workspace_dialog(
                                                                window, cx,
                                                            );
                                                        },
                                                    )),
                                            )
                                            .child(
                                                Button::new("confirm-workspace-dialog")
                                                    .label(if self.workspace_dialog_id.is_some() {
                                                        "Save"
                                                    } else {
                                                        "Create"
                                                    })
                                                    .primary()
                                                    .on_click(cx.listener(
                                                        |manager, _: &ClickEvent, window, cx| {
                                                            manager.confirm_workspace_dialog(
                                                                window, cx,
                                                            );
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
                    })),
            )
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
                        title: format!("Knot - {agent_name}").into(),
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
    [
        Quit,
        HideApp,
        HideOthers,
        ShowAllWindows,
        AboutKnot,
        OpenSettings
    ]
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
            MenuItem::action("Settings…", OpenSettings),
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
        Menu::new("Help").items([MenuItem::action("Knot Help", gpui_kit::NoAction).disabled(true)]),
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
        // `Assets` only embeds gpui-component's own curated icon subset; our
        // settings-window icon buttons (folder-open/pencil/trash/x/plus/copy)
        // aren't in it, so `Icon::path(...)` silently resolved to nothing and
        // rendered invisible. `AllAssets` embeds the complete Lucide catalog.
        .with_assets(gpui_kit::assets::AllAssets)
        .run(move |cx| {
            gpui_kit::init(cx);
            Theme::change(cx.window_appearance(), None, cx);
            apply_visual_identity(cx);

            cx.on_action(quit);
            cx.on_action(about_knot);
            cx.on_action(hide_app);
            cx.on_action(hide_others);
            cx.on_action(show_all_windows);
            cx.bind_keys([KeyBinding::new("cmd-,", OpenSettings, None)]);
            let settings_window: Rc<RefCell<Option<AnyWindowHandle>>> = Rc::new(RefCell::new(None));
            {
                let settings_window = Rc::clone(&settings_window);
                let settings = settings.clone();
                cx.on_action(move |_: &OpenSettings, cx| {
                    open_settings_window(&settings_window, settings.clone(), cx);
                });
            }
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
        let restored = knot_agents::AgentStore::from_saved(&store.saved_agents(false), vec![saved]);

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

    #[test]
    fn appearance_label_maps_known_modes() {
        assert_eq!(SettingsWindow::appearance_label("system"), "System");
        assert_eq!(SettingsWindow::appearance_label("light"), "Light");
        assert_eq!(SettingsWindow::appearance_label("dark"), "Dark");
    }

    #[test]
    fn appearance_label_defaults_to_auto() {
        assert_eq!(SettingsWindow::appearance_label("auto"), "Auto");
        assert_eq!(SettingsWindow::appearance_label("anything-else"), "Auto");
    }

    #[test]
    fn agent_type_label_maps_known_types() {
        assert_eq!(SettingsWindow::agent_type_label("codex"), "Codex");
        assert_eq!(SettingsWindow::agent_type_label("opencode"), "OpenCode");
        assert_eq!(SettingsWindow::agent_type_label("gemini"), "Gemini");
        assert_eq!(SettingsWindow::agent_type_label("copilot"), "Copilot");
        assert_eq!(SettingsWindow::agent_type_label("shell"), "Shell");
    }

    #[test]
    fn agent_type_label_defaults_to_claude() {
        assert_eq!(SettingsWindow::agent_type_label("claude"), "Claude");
        assert_eq!(SettingsWindow::agent_type_label("anything-else"), "Claude");
    }

    #[test]
    fn persona_preview_returns_short_instructions_unchanged() {
        assert_eq!(SettingsWindow::persona_preview("be terse", 80), "be terse");
    }

    #[test]
    fn persona_preview_truncates_long_instructions_with_ellipsis() {
        let instructions = "a".repeat(100);
        let preview = SettingsWindow::persona_preview(&instructions, 80);
        assert_eq!(preview.chars().count(), 81);
        assert!(preview.ends_with('…'));
        assert_eq!(&preview[..80], "a".repeat(80).as_str());
    }

    #[test]
    fn ai_provider_label_maps_known_providers() {
        assert_eq!(SettingsWindow::ai_provider_label("openai"), "OpenAI");
        assert_eq!(SettingsWindow::ai_provider_label("anthropic"), "Anthropic");
        assert_eq!(SettingsWindow::ai_provider_label("google"), "Google");
    }

    #[test]
    fn ai_provider_label_defaults_to_openai() {
        assert_eq!(SettingsWindow::ai_provider_label("anything-else"), "OpenAI");
    }

    #[test]
    fn ai_model_for_matches_swift_reference_defaults() {
        assert_eq!(SettingsWindow::ai_model_for("openai"), "gpt-5-mini");
        assert_eq!(
            SettingsWindow::ai_model_for("anthropic"),
            "claude-haiku-4-5"
        );
        assert_eq!(
            SettingsWindow::ai_model_for("google"),
            "gemini-flash-lite-latest"
        );
        assert_eq!(SettingsWindow::ai_model_for("anything-else"), "");
    }

    #[test]
    fn autopilot_action_label_maps_known_actions() {
        assert_eq!(
            SettingsWindow::autopilot_action_label("mark"),
            "Mark conversation"
        );
        assert_eq!(SettingsWindow::autopilot_action_label("ask"), "Ask me");
        assert_eq!(
            SettingsWindow::autopilot_action_label("continue"),
            "Auto-continue"
        );
        assert_eq!(SettingsWindow::autopilot_action_label("custom"), "Custom");
    }

    #[test]
    fn autopilot_action_label_defaults_to_mark() {
        assert_eq!(
            SettingsWindow::autopilot_action_label("anything-else"),
            "Mark conversation"
        );
    }

    #[test]
    fn autopilot_action_description_is_distinct_per_action() {
        let descriptions: BTreeSet<&str> = ["mark", "ask", "continue", "custom"]
            .iter()
            .map(|action| SettingsWindow::autopilot_action_description(action))
            .collect();
        assert_eq!(descriptions.len(), 4);
    }

    #[test]
    fn key_name_for_code_maps_known_modifier_codes() {
        assert_eq!(SettingsWindow::key_name_for_code(54), "Right Command");
        assert_eq!(SettingsWindow::key_name_for_code(56), "Left Shift");
        assert_eq!(SettingsWindow::key_name_for_code(63), "Fn");
    }

    #[test]
    fn key_name_for_code_falls_back_for_unknown_codes() {
        assert_eq!(SettingsWindow::key_name_for_code(999), "Key 999");
    }

    #[test]
    fn mcp_server_url_formats_localhost_with_port() {
        assert_eq!(
            SettingsWindow::mcp_server_url(8766),
            "http://127.0.0.1:8766"
        );
        assert_eq!(
            SettingsWindow::mcp_server_url(9000),
            "http://127.0.0.1:9000"
        );
    }

    #[test]
    fn mcp_install_command_matches_swift_reference_per_agent() {
        let url = "http://127.0.0.1:8766";
        assert_eq!(
            SettingsWindow::mcp_install_command("claude", url),
            "claude mcp add --transport http --scope user knot http://127.0.0.1:8766"
        );
        assert_eq!(
            SettingsWindow::mcp_install_command("codex", url),
            "codex mcp add knot --url http://127.0.0.1:8766"
        );
        assert_eq!(
            SettingsWindow::mcp_install_command("opencode", url),
            "opencode mcp add"
        );
        assert_eq!(
            SettingsWindow::mcp_install_command("gemini", url),
            "gemini mcp add --transport http knot http://127.0.0.1:8766 --scope user"
        );
        assert_eq!(SettingsWindow::mcp_install_command("copilot", url), "");
    }

    #[test]
    fn terminal_fonts_matches_swift_reference_monospace_list() {
        assert_eq!(
            SettingsWindow::TERMINAL_FONTS,
            [
                "SF Mono",
                "Menlo",
                "Monaco",
                "Courier New",
                "Andale Mono",
                "JetBrains Mono",
                "Fira Code",
                "Source Code Pro",
                "IBM Plex Mono",
                "Hack",
                "Inconsolata",
            ]
        );
    }

    #[test]
    fn restore_conversation_toggle_enabled_only_with_layout_restore() {
        assert!(SettingsWindow::restore_conversation_toggle_enabled(true));
        assert!(!SettingsWindow::restore_conversation_toggle_enabled(false));
    }

    #[test]
    fn turning_off_layout_restore_does_not_touch_conversation_restore() {
        let mut settings = knot_core::Settings::default();
        settings.restore_conversation_on_launch = true;
        settings.restore_layout_on_launch = false;
        assert!(settings.restore_conversation_on_launch);
    }

    #[test]
    fn settings_tab_default_is_general() {
        assert_eq!(SettingsTab::ALL[0], SettingsTab::General);
    }

    #[test]
    fn settings_tab_labels_are_distinct() {
        let labels: BTreeSet<&str> = SettingsTab::ALL.iter().map(|tab| tab.label()).collect();
        assert_eq!(labels.len(), SettingsTab::ALL.len());
    }

    #[test]
    fn settings_tab_covers_every_swift_pane() {
        let labels: Vec<&str> = SettingsTab::ALL.iter().map(|tab| tab.label()).collect();
        assert_eq!(
            labels,
            vec![
                "General",
                "Coding",
                "Personas",
                "Autopilot",
                "Voice",
                "MCP",
                "Terminal"
            ]
        );
    }
}
