use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::consts;
use crate::tools::ToolCatalog;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum JsonRpcId {
    String(String),
    Int(i64),
}

#[derive(Debug, Deserialize)]
pub struct JsonRpcRequest {
    #[serde(default)]
    pub jsonrpc: String,
    #[serde(default)]
    pub id: Option<JsonRpcId>,
    pub method: String,
    #[serde(default)]
    pub params: Option<Value>,
}

#[derive(Debug, Serialize)]
pub struct JsonRpcResponse {
    pub jsonrpc: &'static str,
    pub id: Option<JsonRpcId>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl JsonRpcResponse {
    pub fn success(id: Option<JsonRpcId>, result: impl Serialize) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: serde_json::to_value(result).ok(),
            error: None,
        }
    }

    pub fn error(id: Option<JsonRpcId>, code: i64, message: impl Into<String>) -> Self {
        Self {
            jsonrpc: "2.0",
            id,
            result: None,
            error: Some(JsonRpcError {
                code,
                message: message.into(),
                data: None,
            }),
        }
    }
}

#[derive(Debug, Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub message: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

const METHOD_NOT_FOUND: i64 = -32601;
const INVALID_PARAMS: i64 = -32602;

/// Dispatches a parsed JSON-RPC request to the built-in MCP lifecycle
/// methods, routing `tools/list`/`tools/call` through `catalog`.
pub async fn dispatch(request: &JsonRpcRequest, catalog: &dyn ToolCatalog) -> JsonRpcResponse {
    match request.method.as_str() {
        "initialize" => JsonRpcResponse::success(
            request.id.clone(),
            serde_json::json!({
                "protocolVersion": consts::PROTOCOL_VERSION,
                "capabilities": { "tools": { "listChanged": false } },
                "serverInfo": { "name": consts::SERVER_NAME, "version": consts::SERVER_VERSION },
            }),
        ),
        "tools/list" => JsonRpcResponse::success(
            request.id.clone(),
            serde_json::json!({ "tools": catalog.list() }),
        ),
        "tools/call" => {
            let Some(name) = request
                .params
                .as_ref()
                .and_then(|p| p.get("name"))
                .and_then(Value::as_str)
            else {
                return JsonRpcResponse::error(
                    request.id.clone(),
                    INVALID_PARAMS,
                    "Invalid params: missing tool name",
                );
            };
            let arguments = request
                .params
                .as_ref()
                .and_then(|p| p.get("arguments"))
                .cloned()
                .unwrap_or_else(|| serde_json::json!({}));
            let result = catalog.call(name, arguments).await;
            JsonRpcResponse::success(request.id.clone(), result)
        }
        "shutdown" => JsonRpcResponse::success(request.id.clone(), serde_json::json!({})),
        other => JsonRpcResponse::error(
            request.id.clone(),
            METHOD_NOT_FOUND,
            format!("Method not found: {other}"),
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tools::{EmptyCatalog, ToolCallResult, ToolDefinition, ToolInputSchema};
    use async_trait::async_trait;

    fn request(method: &str, params: Option<Value>) -> JsonRpcRequest {
        JsonRpcRequest {
            jsonrpc: "2.0".to_string(),
            id: Some(JsonRpcId::Int(1)),
            method: method.to_string(),
            params,
        }
    }

    #[test]
    fn json_rpc_id_round_trips_string_and_int() {
        let string_id: JsonRpcId = serde_json::from_str("\"abc\"").unwrap();
        assert_eq!(string_id, JsonRpcId::String("abc".to_string()));
        assert_eq!(serde_json::to_string(&string_id).unwrap(), "\"abc\"");

        let int_id: JsonRpcId = serde_json::from_str("42").unwrap();
        assert_eq!(int_id, JsonRpcId::Int(42));
        assert_eq!(serde_json::to_string(&int_id).unwrap(), "42");
    }

    #[tokio::test]
    async fn initialize_returns_protocol_and_server_info() {
        let response = dispatch(&request("initialize", None), &EmptyCatalog).await;
        let result = response.result.unwrap();
        assert_eq!(result["protocolVersion"], consts::PROTOCOL_VERSION);
        assert!(result["capabilities"]["tools"].is_object());
        assert_eq!(result["serverInfo"]["name"], consts::SERVER_NAME);
    }

    #[tokio::test]
    async fn unknown_method_is_method_not_found() {
        let response = dispatch(&request("nonexistent", None), &EmptyCatalog).await;
        let error = response.error.unwrap();
        assert_eq!(error.code, METHOD_NOT_FOUND);
        assert_eq!(response.id, Some(JsonRpcId::Int(1)));
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
    async fn tool_call_result_has_one_text_content_item() {
        let params = serde_json::json!({ "name": "ping", "arguments": {} });
        let response = dispatch(&request("tools/call", Some(params)), &OneToolCatalog).await;
        let result = response.result.unwrap();
        assert_eq!(result["content"].as_array().unwrap().len(), 1);
        assert!(result.get("isError").is_none());
    }
}
