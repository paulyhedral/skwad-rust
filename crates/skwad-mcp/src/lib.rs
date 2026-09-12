//! Local MCP HTTP server.
//!
//! Implements `openspec/specs/mcp-server/spec.md`.

mod consts;
mod error;
mod hooks;
mod rpc;
mod server;
mod session;
mod status;
mod tools;

pub use consts::DEFAULT_PORT;
pub use error::{McpError, Result};
pub use hooks::{
    HookError, HookRequest, HookStatus, claude_status, codex_turn_complete, extract_metadata,
    last_assistant_message_from_transcript,
};
pub use rpc::{JsonRpcError, JsonRpcId, JsonRpcRequest, JsonRpcResponse, dispatch};
pub use server::{AgentsSnapshotFn, McpServer};
pub use session::{McpSession, McpSessionManager};
pub use status::{AgentStatusEntry, agent_status};
pub use tools::{
    EmptyCatalog, PropertySchema, ToolCallResult, ToolCatalog, ToolContent, ToolDefinition,
    ToolInputSchema,
};
