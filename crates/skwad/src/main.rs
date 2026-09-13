use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use gpui_kit::base::Selectable;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::button::Button;
use gpui_kit::component::*;
use gpui_kit::{
    AppContext, AsyncApp, ClickEvent, Context, IntoElement, ParentElement, Render, Styled,
    WeakEntity, Window, WindowOptions, div,
};
use skwad_activity::EventSink;
use skwad_mcp::ToolCatalog;
use skwad_terminal::{PtyTransport, SessionConfig, SessionPlan, TerminalSession};
use uuid::Uuid;

const MAX_VISIBLE_LINES: usize = 200;
const OUTPUT_POLL_INTERVAL: Duration = Duration::from_millis(100);

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

struct Shell {
    store: skwad_agents::AgentStore,
    settings: skwad_core::Settings,
    agent_selection: Option<Uuid>,
    buffers: BTreeMap<Uuid, Arc<Mutex<OutputBuffer>>>,
    sessions: BTreeMap<Uuid, Arc<Mutex<TerminalSession<PtyTransport>>>>,
}

impl Shell {
    /// Spawns a PTY-backed terminal session for the agent if one is not
    /// already running. The PTY read thread appends output into `buffers`;
    /// [`poll_outputs`] wakes this view when a buffer grows.
    fn attach_session(&mut self, id: Uuid) {
        if self.sessions.contains_key(&id) {
            return;
        }
        let Some(agent) = self.store.agent(id).cloned() else {
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

        let session = TerminalSession::<PtyTransport>::spawn_pty(
            &config,
            EventSink::default(),
            move |bytes| {
                if let Ok(mut guard) = buffer_sink.lock() {
                    guard.bytes.extend_from_slice(bytes);
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
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let attached_ids = self.sessions.keys().copied().collect::<Vec<_>>();
        let model = layout_model(&self.store, self.agent_selection, &attached_ids);

        let workspace_buttons = model
            .workspace_rows
            .into_iter()
            .map(|row| {
                let id = row.id;
                Button::new(id.to_string())
                    .label(row.name)
                    .selected(row.selected)
                    .on_click(cx.listener(move |shell, _: &ClickEvent, _window, cx| {
                        shell.store.set_current_workspace(id);
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
                    "{} {} [{}] {}{}",
                    row.avatar,
                    row.name,
                    row.agent_type,
                    row.folder,
                    if row.attached { " (attached)" } else { "" }
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

        let selected_buffer = self
            .agent_selection
            .and_then(|id| self.buffers.get(&id))
            .map(|buffer| buffer.lock().unwrap().clone());
        let terminal_model = terminal_model(
            &self.store,
            self.agent_selection,
            &attached_ids,
            selected_buffer.as_ref(),
        );

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
            }
            None => div().child("Select an agent to see its terminal."),
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
/// into shared buffers and re-renders the shell whenever a buffer grows.
async fn poll_outputs(weak: WeakEntity<Shell>, cx: &mut AsyncApp) {
    let mut seen = BTreeMap::<Uuid, usize>::new();
    loop {
        cx.background_executor().timer(OUTPUT_POLL_INTERVAL).await;
        if weak.upgrade().is_none() {
            break;
        }
        let changed = cx.update(|app| {
            let Some(entity) = weak.upgrade() else {
                return false;
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
            changed
        });
        if changed {
            cx.update(|app| {
                if let Some(entity) = weak.upgrade() {
                    entity.update(app, |_, cx| cx.notify());
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
/// The agent store starts empty: nothing in `crates/skwad` yet restores
/// `Settings::saved_agents`/`saved_workspaces` into a running `AgentStore`
/// (that loader is agent-lifecycle-port's integration surface, not
/// mcp-tools'). Repo discovery and bench-agent templates do come from real
/// settings, since those are plain field reads with no new loader needed.
fn start_mcp_server() {
    std::thread::spawn(|| {
        let runtime = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(err) => {
                eprintln!("failed to start MCP server runtime: {err}");
                return;
            }
        };
        runtime.block_on(async {
            let settings = skwad_core::Settings::load().unwrap_or_default();
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

            let agent_store = if settings.restore_layout_on_launch {
                skwad_agents::AgentStore::from_saved(
                    &settings.saved_agents,
                    settings.saved_workspaces.clone(),
                )
            } else {
                skwad_agents::AgentStore::new()
            };
            let catalog = Arc::new(
                skwad_mcp_tools::McpToolCatalog::new(
                    agent_store,
                    repos_rx,
                    Arc::new(skwad_messaging::NoopNotifier),
                )
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
    start_mcp_server();

    let settings = skwad_core::Settings::load().unwrap_or_default();
    let store = if settings.restore_layout_on_launch {
        skwad_agents::AgentStore::from_saved(
            &settings.saved_agents,
            settings.saved_workspaces.clone(),
        )
    } else {
        skwad_agents::AgentStore::new()
    };

    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|_| Shell {
                    store: store.clone(),
                    settings: settings.clone(),
                    agent_selection: None,
                    buffers: BTreeMap::new(),
                    sessions: BTreeMap::new(),
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
        let model = layout_model(&store, None, &[]);
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
        let model = layout_model(&store, None, &[]);

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

        let model = layout_model(&store, None, &[]);
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

        let model = layout_model(&store, Some(alpha_id), &[]);
        let alpha = model
            .selected_agent_rows
            .iter()
            .find(|row| row.id == alpha_id)
            .unwrap();
        assert!(alpha.selected);
        assert!(!alpha.attached);
        let beta = model
            .selected_agent_rows
            .iter()
            .find(|row| row.id != alpha_id)
            .unwrap();
        assert!(!beta.selected);

        let model = layout_model(&store, Some(alpha_id), &[alpha_id]);
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
    fn terminal_model_renders_header_and_output_for_attached_agent() {
        let mut store = skwad_agents::AgentStore::new();
        let ws = workspace("One");
        store.add_workspace(ws.clone());
        store.set_current_workspace(ws.id);
        let id = store.create("~/alpha", skwad_agents::CreateOptions::default());
        let buffer = OutputBuffer {
            bytes: b"hello\r\nworld\n".to_vec(),
        };

        let model = terminal_model(&store, Some(id), &[id], Some(&buffer));
        assert!(!model.can_attach);
        let header = model.header.unwrap();
        assert_eq!(header.name, "alpha");
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
}
