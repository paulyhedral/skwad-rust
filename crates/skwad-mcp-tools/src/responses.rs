//! Success-payload structs, one per tool, matching the Swift reference's
//! `MCPTypes.swift` JSON shape (`#[serde(rename_all = "camelCase")]` so
//! Rust's `snake_case` fields serialize to the same wire keys).

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AgentInfo {
    pub id: String,
    pub name: String,
    pub folder: String,
    pub status: String,
    pub is_registered: bool,
}

#[derive(Debug, Serialize)]
pub struct ListAgentsResponse {
    pub agents: Vec<AgentInfo>,
}

#[derive(Debug, Serialize)]
pub struct SendMessageResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct CheckMessagesResponse {
    pub messages: Vec<MessageInfo>,
}

#[derive(Debug, Serialize)]
pub struct MessageInfo {
    pub id: String,
    pub from: String,
    pub content: String,
    pub timestamp: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BroadcastResponse {
    pub success: bool,
    pub recipient_count: usize,
}

#[derive(Debug, Serialize)]
pub struct RepoInfoResponse {
    pub name: String,
    pub worktrees: Vec<WorktreeInfoResponse>,
}

#[derive(Debug, Serialize)]
pub struct ListReposResponse {
    pub repos: Vec<RepoInfoResponse>,
}

#[derive(Debug, Serialize)]
pub struct WorktreeInfoResponse {
    pub name: String,
    pub path: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ListWorktreesResponse {
    pub repo_path: String,
    pub worktrees: Vec<WorktreeInfoResponse>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateAgentResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct CloseAgentResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RegisterAgentResponse {
    pub success: bool,
    pub message: String,
    pub unread_message_count: usize,
    pub skwad_members: Vec<AgentInfo>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateWorktreeResponse {
    pub success: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub path: Option<String>,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ShowMarkdownResponse {
    pub success: bool,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ShowMermaidResponse {
    pub success: bool,
    pub message: String,
}

/// Pretty-prints `value` into a [`skwad_mcp::ToolCallResult`], matching the
/// Swift reference's `successResult` (pretty-printed JSON as the tool's text
/// content).
pub fn success<T: Serialize>(value: &T) -> skwad_mcp::ToolCallResult {
    match serde_json::to_string_pretty(value) {
        Ok(text) => skwad_mcp::ToolCallResult::ok(text),
        Err(err) => skwad_mcp::ToolCallResult::error(format!("Failed to encode result: {err}")),
    }
}
