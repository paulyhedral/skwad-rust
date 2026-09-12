use std::path::PathBuf;
use std::sync::Arc;

use gpui_kit::component::*;
use gpui_kit::*;
use skwad_mcp::ToolCatalog;

struct Shell;

impl Render for Shell {
    fn render(&mut self, _window: &mut Window, _cx: &mut Context<Self>) -> impl IntoElement {
        div().size_full()
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
            let catalog = Arc::new(skwad_mcp_tools::McpToolCatalog::new(
                agent_store,
                repos_rx,
                Arc::new(skwad_messaging::NoopNotifier),
            ));
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

    gpui_kit::application().run(move |cx| {
        gpui_kit::init(cx);

        cx.spawn(async move |cx| {
            cx.open_window(WindowOptions::default(), |window, cx| {
                let view = cx.new(|_| Shell);
                cx.new(|cx| Root::new(view, window, cx).bg(cx.theme().background))
            })
            .expect("failed to open window");
        })
        .detach();
    });
}
