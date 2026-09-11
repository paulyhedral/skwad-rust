use std::collections::BTreeMap;

use async_trait::async_trait;
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct ToolDefinition {
    pub name: String,
    pub description: String,
    pub input_schema: ToolInputSchema,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolInputSchema {
    #[serde(rename = "type")]
    pub schema_type: &'static str,
    pub properties: BTreeMap<String, PropertySchema>,
    pub required: Vec<String>,
}

impl Default for ToolInputSchema {
    fn default() -> Self {
        Self {
            schema_type: "object",
            properties: BTreeMap::new(),
            required: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct PropertySchema {
    #[serde(rename = "type")]
    pub schema_type: String,
    pub description: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolContent {
    #[serde(rename = "type")]
    pub content_type: &'static str,
    pub text: String,
}

impl ToolContent {
    pub fn text(text: impl Into<String>) -> Self {
        Self {
            content_type: "text",
            text: text.into(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolCallResult {
    pub content: Vec<ToolContent>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub is_error: Option<bool>,
}

impl ToolCallResult {
    pub fn ok(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            is_error: None,
        }
    }

    pub fn error(text: impl Into<String>) -> Self {
        Self {
            content: vec![ToolContent::text(text)],
            is_error: Some(true),
        }
    }
}

/// The set of MCP tools an `McpServer` dispatches `tools/list`/`tools/call`
/// through. The concrete catalog (`mcp-tools`) plugs in here without this
/// crate changing.
#[async_trait]
pub trait ToolCatalog: Send + Sync {
    fn list(&self) -> Vec<ToolDefinition>;
    async fn call(&self, name: &str, arguments: serde_json::Value) -> ToolCallResult;
}

/// A `ToolCatalog` with no tools; every call fails with "unknown tool".
pub struct EmptyCatalog;

#[async_trait]
impl ToolCatalog for EmptyCatalog {
    fn list(&self) -> Vec<ToolDefinition> {
        Vec::new()
    }

    async fn call(&self, name: &str, _arguments: serde_json::Value) -> ToolCallResult {
        ToolCallResult::error(format!("unknown tool: {name}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn empty_catalog_reports_unknown_tool() {
        let result = EmptyCatalog
            .call("does-not-exist", serde_json::json!({}))
            .await;
        assert_eq!(result.is_error, Some(true));
        assert_eq!(result.content.len(), 1);
        assert!(result.content[0].text.contains("does-not-exist"));
    }

    #[test]
    fn empty_catalog_lists_no_tools() {
        assert!(EmptyCatalog.list().is_empty());
    }
}
