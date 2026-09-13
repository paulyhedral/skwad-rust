use std::sync::Arc;

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use tokio::net::TcpListener;
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::consts;
use crate::hooks::{AgentHookHandler, HookRequest};
use crate::rpc::{self, JsonRpcId, JsonRpcRequest, JsonRpcResponse};
use crate::session::McpSessionManager;
use crate::status::{self, AgentStatusEntry};
use crate::tools::ToolCatalog;

const SESSION_HEADER: &str = "Mcp-Session-Id";

/// Supplies the live agent snapshot the status endpoint serializes.
pub type AgentsSnapshotFn = Arc<dyn Fn() -> Vec<skwad_agents::Agent> + Send + Sync>;

#[derive(Clone)]
struct AppState {
    catalog: Arc<dyn ToolCatalog>,
    agents: AgentsSnapshotFn,
    hooks: Option<Arc<dyn AgentHookHandler>>,
    sessions: McpSessionManager,
}

/// The local MCP HTTP server: health/info, JSON-RPC `/mcp`, SSE `/mcp`, and
/// the `GET /api/v1/agent/status` status endpoint.
pub struct McpServer {
    port: u16,
    state: AppState,
    handle: Option<JoinHandle<()>>,
    bound_addr: Option<std::net::SocketAddr>,
}

impl McpServer {
    pub fn new(port: u16, catalog: Arc<dyn ToolCatalog>, agents: AgentsSnapshotFn) -> Self {
        Self {
            port,
            state: AppState {
                catalog,
                agents,
                hooks: None,
                sessions: McpSessionManager::new(),
            },
            handle: None,
            bound_addr: None,
        }
    }

    pub fn with_hook_handler(mut self, handler: Arc<dyn AgentHookHandler>) -> Self {
        self.state.hooks = Some(handler);
        self
    }

    pub fn with_default_port(catalog: Arc<dyn ToolCatalog>, agents: AgentsSnapshotFn) -> Self {
        Self::new(consts::DEFAULT_PORT, catalog, agents)
    }

    pub fn port(&self) -> u16 {
        self.port
    }

    /// The port actually bound after `start()`, once known (differs from
    /// `port()` when constructed with port `0`).
    pub fn bound_addr(&self) -> Option<std::net::SocketAddr> {
        self.bound_addr
    }

    pub async fn start(&mut self) -> crate::Result<()> {
        let router = build_router(self.state.clone());
        let addr: std::net::SocketAddr = ([127, 0, 0, 1], self.port).into();
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|e| crate::McpError::Bind(addr, e))?;
        self.bound_addr = Some(listener.local_addr().map_err(crate::McpError::Serve)?);
        self.handle = Some(tokio::spawn(async move {
            let _ = axum::serve(listener, router).await;
        }));
        Ok(())
    }

    pub fn stop(&mut self) {
        if let Some(handle) = self.handle.take() {
            handle.abort();
        }
        self.bound_addr = None;
    }
}

fn build_router(state: AppState) -> Router {
    Router::new()
        .route("/health", get(health))
        .route("/", get(info))
        .route("/mcp", post(mcp_rpc).get(mcp_sse))
        .route(
            "/api/v1/agent/status",
            get(agent_status_endpoint).post(agent_status_hook_endpoint),
        )
        .route("/api/v1/agent/register", post(agent_register_endpoint))
        .with_state(state)
}

async fn health() -> &'static str {
    "OK"
}

async fn info() -> Response {
    axum::Json(serde_json::json!({
        "name": consts::SERVER_NAME,
        "version": consts::SERVER_VERSION,
    }))
    .into_response()
}

async fn agent_status_endpoint(State(state): State<AppState>) -> axum::Json<Vec<AgentStatusEntry>> {
    let agents = (state.agents)();
    axum::Json(status::agent_status(&agents))
}

async fn agent_register_endpoint(State(state): State<AppState>, body: Bytes) -> Response {
    hook_response(state.hooks.as_deref(), &body, HookAction::Register)
}

async fn agent_status_hook_endpoint(State(state): State<AppState>, body: Bytes) -> Response {
    hook_response(state.hooks.as_deref(), &body, HookAction::Status)
}

enum HookAction {
    Register,
    Status,
}

fn hook_response(
    handler: Option<&dyn AgentHookHandler>,
    body: &[u8],
    action: HookAction,
) -> Response {
    let request: HookRequest = match serde_json::from_slice(body) {
        Ok(request) => request,
        Err(error) => return (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
    };
    if let Err(error) = request.agent_id().and(request.validate_agent()) {
        return (StatusCode::BAD_REQUEST, error.to_string()).into_response();
    }

    let Some(handler) = handler else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "Agent hooks are not configured",
        )
            .into_response();
    };
    let result = match action {
        HookAction::Register => handler.register(&request),
        HookAction::Status => handler.status(&request),
    };
    match result {
        Ok(value) => axum::Json(value).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, error.to_string()).into_response(),
    }
}

fn sse_response(event: &str, data: &str, session_id: Option<&str>) -> Response {
    let body = format!("event: {event}\ndata: {data}\n\n");
    let mut response = Response::builder()
        .status(StatusCode::OK)
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .header(header::CONNECTION, "keep-alive");
    if let Some(id) = session_id
        && let Ok(value) = HeaderValue::from_str(id)
    {
        response = response.header(SESSION_HEADER, value);
    }
    response.body(axum::body::Body::from(body)).unwrap()
}

async fn mcp_sse() -> Response {
    sse_response("connected", "{\"status\":\"connected\"}", None)
}

async fn mcp_rpc(State(state): State<AppState>, headers: HeaderMap, body: Bytes) -> Response {
    state.sessions.cleanup_stale_default();
    let session_id = headers
        .get(SESSION_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::to_string);
    let accepts_sse = headers
        .get(header::ACCEPT)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.contains("text/event-stream"));

    let request: JsonRpcRequest = match serde_json::from_slice(&body) {
        Ok(r) => r,
        Err(e) => return json_rpc_error_response(-32700, format!("Parse error: {e}")),
    };

    let response_session_id = if request.method == "initialize" {
        state.sessions.create_session(Uuid::nil()).id
    } else if let Some(session_id) = session_id {
        if state.sessions.session(&session_id).is_none() {
            return session_error_response(request.id, "Invalid or expired MCP session");
        }
        state.sessions.touch(&session_id);
        session_id
    } else {
        state.sessions.create_session(Uuid::nil()).id
    };

    if request.method.starts_with("notifications/") {
        return StatusCode::ACCEPTED.into_response();
    }

    let response = rpc::dispatch(&request, state.catalog.as_ref()).await;

    if accepts_sse {
        let data = serde_json::to_string(&response).unwrap_or_else(|_| "{}".to_string());
        return sse_response("message", &data, Some(&response_session_id));
    }

    let mut resp = axum::Json(response).into_response();
    if let Ok(value) = HeaderValue::from_str(&response_session_id) {
        resp.headers_mut().insert(SESSION_HEADER, value);
    }
    resp
}

fn session_error_response(id: Option<JsonRpcId>, message: &str) -> Response {
    let response = JsonRpcResponse::error(id, -32000, message);
    (StatusCode::BAD_REQUEST, axum::Json(response)).into_response()
}

fn json_rpc_error_response(code: i64, message: String) -> Response {
    let response = JsonRpcResponse::error(None::<JsonRpcId>, code, message);
    (StatusCode::BAD_REQUEST, axum::Json(response)).into_response()
}
