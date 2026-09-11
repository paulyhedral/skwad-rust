use std::sync::Arc;

use async_trait::async_trait;
use serde_json::{Value, json};
use skwad_agents::{Agent, AgentState};
use skwad_mcp::{
    DEFAULT_PORT, EmptyCatalog, McpServer, ToolCallResult, ToolCatalog, ToolDefinition,
    ToolInputSchema,
};
use uuid::Uuid;

fn no_agents() -> Arc<dyn Fn() -> Vec<Agent> + Send + Sync> {
    Arc::new(Vec::new)
}

async fn start_server(catalog: Arc<dyn ToolCatalog>) -> (McpServer, String) {
    let mut server = McpServer::new(0, catalog, no_agents());
    server.start().await.expect("server starts");
    let addr = server
        .bound_addr()
        .expect("bound address known after start");
    (server, format!("http://{addr}"))
}

#[tokio::test]
async fn default_bind_uses_documented_port() {
    let server = McpServer::new(DEFAULT_PORT, Arc::new(EmptyCatalog), no_agents());
    assert_eq!(server.port(), DEFAULT_PORT);
}

#[tokio::test]
async fn health_check_reports_ok() {
    let (mut server, base) = start_server(Arc::new(EmptyCatalog)).await;

    let resp = reqwest::get(format!("{base}/health")).await.unwrap();
    assert_eq!(resp.status(), 200);
    assert_eq!(resp.text().await.unwrap(), "OK");

    server.stop();
}

#[tokio::test]
async fn info_endpoint_responds_without_initialization() {
    let (mut server, base) = start_server(Arc::new(EmptyCatalog)).await;

    let resp = reqwest::get(&base).await.unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["name"], "skwad-mcp");

    server.stop();
}

#[tokio::test]
async fn initialize_handshake_over_http() {
    let (mut server, base) = start_server(Arc::new(EmptyCatalog)).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/mcp"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    assert!(resp.headers().get("Mcp-Session-Id").is_some());
    let body: Value = resp.json().await.unwrap();
    assert!(body["result"]["capabilities"]["tools"].is_object());
    assert_eq!(body["result"]["serverInfo"]["name"], "skwad-mcp");

    server.stop();
}

#[tokio::test]
async fn unknown_method_over_http_is_method_not_found() {
    let (mut server, base) = start_server(Arc::new(EmptyCatalog)).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/mcp"))
        .json(&json!({ "jsonrpc": "2.0", "id": 7, "method": "not-a-method" }))
        .send()
        .await
        .unwrap();

    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    assert_eq!(body["error"]["code"], -32601);
    assert_eq!(body["id"], 7);

    server.stop();
}

#[tokio::test]
async fn sse_accept_header_wraps_response_as_sse() {
    let (mut server, base) = start_server(Arc::new(EmptyCatalog)).await;
    let client = reqwest::Client::new();

    let resp = client
        .post(format!("{base}/mcp"))
        .header("Accept", "text/event-stream")
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "initialize" }))
        .send()
        .await
        .unwrap();

    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    let body = resp.text().await.unwrap();
    assert!(body.starts_with("event: message"));

    server.stop();
}

#[tokio::test]
async fn get_mcp_opens_sse_stream_with_connected_event() {
    let (mut server, base) = start_server(Arc::new(EmptyCatalog)).await;

    let resp = reqwest::get(format!("{base}/mcp")).await.unwrap();
    assert_eq!(
        resp.headers().get("content-type").unwrap(),
        "text/event-stream"
    );
    let body = resp.text().await.unwrap();
    assert!(body.starts_with("event: connected"));

    server.stop();
}

fn test_agent(name: &str, registered: bool) -> Agent {
    Agent {
        id: Uuid::new_v4(),
        name: name.to_string(),
        avatar: String::new(),
        folder: "/tmp/proj".to_string(),
        agent_type: "claude".to_string(),
        created_by: None,
        is_companion: false,
        shell_command: None,
        persona_id: None,
        state: AgentState::Idle,
        status_text: String::new(),
        is_registered: registered,
        is_pending_start: false,
        terminal_title: String::new(),
        restart_token: Uuid::new_v4(),
        session_id: registered.then(|| "sess-42".to_string()),
        resume_session_id: None,
        fork_session: false,
        metadata: Default::default(),
        markdown_file: None,
        markdown_maximized: false,
        markdown_history: Vec::new(),
        mermaid_source: None,
        mermaid_title: None,
    }
}

#[tokio::test]
async fn status_endpoint_reflects_live_agents() {
    let agents = vec![test_agent("one", true), test_agent("two", false)];
    let snapshot: Arc<dyn Fn() -> Vec<Agent> + Send + Sync> = Arc::new(move || agents.clone());

    let mut server = McpServer::new(0, Arc::new(EmptyCatalog), snapshot);
    server.start().await.unwrap();
    let base = format!("http://{}", server.bound_addr().unwrap());

    let resp = reqwest::get(format!("{base}/api/v1/agent/status"))
        .await
        .unwrap();
    assert_eq!(resp.status(), 200);
    let body: Value = resp.json().await.unwrap();
    let entries = body.as_array().unwrap();
    assert_eq!(entries.len(), 2);
    let registered_entry = entries.iter().find(|e| e["name"] == "one").unwrap();
    assert_eq!(registered_entry["session_id"], "sess-42");

    server.stop();
}

struct OneToolCatalog;

#[async_trait]
impl ToolCatalog for OneToolCatalog {
    fn list(&self) -> Vec<ToolDefinition> {
        vec![ToolDefinition {
            name: "ping".to_string(),
            description: "Replies pong".to_string(),
            input_schema: ToolInputSchema::default(),
        }]
    }

    async fn call(&self, _name: &str, _arguments: Value) -> ToolCallResult {
        ToolCallResult::ok("pong")
    }
}

#[tokio::test]
async fn tools_list_and_call_over_http() {
    let (mut server, base) = start_server(Arc::new(OneToolCatalog)).await;
    let client = reqwest::Client::new();

    let list_resp = client
        .post(format!("{base}/mcp"))
        .json(&json!({ "jsonrpc": "2.0", "id": 1, "method": "tools/list" }))
        .send()
        .await
        .unwrap();
    let list_body: Value = list_resp.json().await.unwrap();
    assert_eq!(list_body["result"]["tools"][0]["name"], "ping");

    let call_resp = client
        .post(format!("{base}/mcp"))
        .json(&json!({
            "jsonrpc": "2.0", "id": 2, "method": "tools/call",
            "params": { "name": "ping", "arguments": {} }
        }))
        .send()
        .await
        .unwrap();
    let call_body: Value = call_resp.json().await.unwrap();
    assert_eq!(call_body["result"]["content"][0]["text"], "pong");
    assert!(call_body["result"]["isError"].is_null());

    server.stop();
}

#[tokio::test]
async fn stop_closes_the_listener() {
    let (mut server, base) = start_server(Arc::new(EmptyCatalog)).await;
    server.stop();

    let result = reqwest::get(format!("{base}/health")).await;
    assert!(result.is_err());
}
