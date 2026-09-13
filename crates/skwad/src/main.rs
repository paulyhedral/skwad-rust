#![recursion_limit = "256"]

use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::base::Selectable;
use gpui_kit::base::{h_flex, v_flex};
use gpui_kit::component::button::Button;
use gpui_kit::component::*;
use gpui_kit::{
    AppContext, ClickEvent, Context, IntoElement, ParentElement, Render, Styled, Window,
    WindowOptions, div,
};
use skwad_mcp::ToolCatalog;
use uuid::Uuid;

#[derive(Debug, PartialEq)]
struct WorkspaceRow {
    id: Uuid,
    name: String,
    selected: bool,
}

#[derive(Debug, PartialEq)]
struct AgentRow {
    avatar: String,
    name: String,
    agent_type: String,
    folder: String,
}

#[derive(Debug, PartialEq)]
struct LayoutModel {
    workspace_rows: Vec<WorkspaceRow>,
    selected_agent_rows: Vec<AgentRow>,
}

fn layout_model(store: &skwad_agents::AgentStore) -> LayoutModel {
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
                    avatar: agent.avatar.clone(),
                    name: agent.name.clone(),
                    agent_type: agent.agent_type.clone(),
                    folder: agent.folder.clone(),
                })
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    LayoutModel {
        workspace_rows,
        selected_agent_rows,
    }
}

struct Shell {
    store: skwad_agents::AgentStore,
}

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, cx: &mut Context<Self>) -> impl IntoElement {
        let model = layout_model(&self.store);

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

        let agent_rows = model
            .selected_agent_rows
            .into_iter()
            .map(|row| {
                div().child(format!(
                    "{} {} [{}] {}",
                    row.avatar, row.name, row.agent_type, row.folder
                ))
            })
            .collect::<Vec<_>>();

        h_flex()
            .size_full()
            .child(v_flex().size_full().children(workspace_buttons))
            .child(v_flex().size_full().children(agent_rows))
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
                });
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
        let model = layout_model(&store);
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
        let model = layout_model(&store);

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

        let model = layout_model(&store);
        assert_eq!(model.selected_agent_rows.len(), 1);
        assert_eq!(model.selected_agent_rows[0].name, "alpha");
    }
}
