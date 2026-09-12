//! Concrete MCP tool catalog exposed to agents.
//!
//! Implements `openspec/specs/mcp-tools/spec.md`.

mod agents;
mod args;
mod consts;
mod lookup;
mod messaging;
mod panels;
mod repos;
mod responses;

use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use skwad_agents::{AgentState, AgentStore};
use skwad_core::BenchAgent;
use skwad_discovery::RepoInfo;
use skwad_mcp::{
    AgentHookHandler, HookRequest, HookStatus, PropertySchema, ToolCallResult, ToolCatalog,
    ToolDefinition, ToolInputSchema, claude_status, codex_turn_complete, extract_metadata,
};
use skwad_messaging::{DeliveryNotifier, MessageStore};
use tokio::sync::watch;

use crate::lookup::state_string;

/// The concrete `ToolCatalog` for the thirteen tools in
/// `openspec/specs/mcp-tools/spec.md`. Holds every piece of shared state a
/// handler needs; each `call` locks only what that tool touches.
pub struct McpToolCatalog {
    agents: Mutex<AgentStore>,
    messages: Mutex<MessageStore>,
    notifier: Arc<dyn DeliveryNotifier + Send + Sync>,
    repos: watch::Receiver<Vec<RepoInfo>>,
    bench_agents: Mutex<Vec<BenchAgent>>,
}

impl McpToolCatalog {
    pub fn new(
        agents: AgentStore,
        repos: watch::Receiver<Vec<RepoInfo>>,
        notifier: Arc<dyn DeliveryNotifier + Send + Sync>,
    ) -> Self {
        Self {
            agents: Mutex::new(agents),
            messages: Mutex::new(MessageStore::new()),
            notifier,
            repos,
            bench_agents: Mutex::new(Vec::new()),
        }
    }

    /// Refreshes the bench-agent templates `create-agent`'s `benchAgentId`
    /// resolves against. The catalog has no settings dependency of its own;
    /// the caller (`crates/skwad`) pushes updates in whenever settings
    /// change.
    pub fn set_bench_agents(&self, bench_agents: Vec<BenchAgent>) {
        *self.bench_agents.lock().unwrap() = bench_agents;
    }

    /// A clone of the current agent list, for `McpServer`'s
    /// `AgentsSnapshotFn` (the `GET /api/v1/agent/status` endpoint).
    pub fn agents_snapshot(&self) -> Vec<skwad_agents::Agent> {
        self.agents.lock().unwrap().agents().to_vec()
    }
}

impl AgentHookHandler for McpToolCatalog {
    fn register(&self, request: &HookRequest) -> Result<serde_json::Value, skwad_mcp::HookError> {
        if request.agent != "claude" {
            return Err(skwad_mcp::HookError::UnknownAgent(request.agent.clone()));
        }
        let id = request.agent_id()?;
        let mut agents = self.agents.lock().unwrap();
        let Some(agent) = agents.agent(id) else {
            return Err(skwad_mcp::HookError::InvalidPayload(
                "Agent not found".to_string(),
            ));
        };
        let is_resuming = agent.resume_session_id.is_some() && !agent.fork_session;
        let is_fork = agent.fork_session;
        let source = request.source.as_deref().unwrap_or("startup");
        agents.set_registered(id, true);
        agents.update_metadata(id, extract_metadata("claude", &request.payload));
        if source == "resume" {
            if !is_fork && let Some(session_id) = request.session_id.clone() {
                agents.set_session_id(id, session_id);
            }
        } else {
            if !is_resuming && let Some(session_id) = request.session_id.clone() {
                agents.set_session_id(id, session_id);
            }
        }
        Ok(serde_json::json!({
            "success": true,
            "message": "Registered",
            "unreadMessageCount": 0,
            "skwadMembers": agents.agents().iter().map(|agent| serde_json::json!({
                "id": agent.id.to_string(),
                "name": agent.name,
                "folder": agent.folder,
                "status": state_string(agent.state),
                "isRegistered": agent.is_registered,
            })).collect::<Vec<_>>(),
        }))
    }

    fn status(&self, request: &HookRequest) -> Result<serde_json::Value, skwad_mcp::HookError> {
        let id = request.agent_id()?;
        let mut agents = self.agents.lock().unwrap();
        if agents.agent(id).is_none() {
            return Err(skwad_mcp::HookError::InvalidPayload(
                "Agent not found".to_string(),
            ));
        }
        agents.update_metadata(id, extract_metadata(&request.agent, &request.payload));
        let status = match request.agent.as_str() {
            "claude" => claude_status(
                request
                    .status
                    .as_deref()
                    .ok_or_else(|| skwad_mcp::HookError::InvalidStatus("missing".to_string()))?,
            )?,
            "codex" => {
                let thread_id = codex_turn_complete(&request.payload)?;
                if let Some(thread_id) = thread_id {
                    agents.set_session_id(id, thread_id);
                }
                HookStatus::Idle
            }
            agent => return Err(skwad_mcp::HookError::UnknownAgent(agent.to_string())),
        };
        agents.set_state(
            id,
            match status {
                HookStatus::Working => AgentState::Running,
                HookStatus::Idle => AgentState::Idle,
                HookStatus::AwaitingInput => AgentState::Input,
            },
        );
        Ok(serde_json::json!({"success": true}))
    }
}

fn prop(schema_type: &str, description: &str) -> PropertySchema {
    PropertySchema {
        schema_type: schema_type.to_string(),
        description: description.to_string(),
    }
}

fn schema(properties: &[(&str, &str, &str)], required: &[&str]) -> ToolInputSchema {
    ToolInputSchema {
        properties: properties
            .iter()
            .map(|(name, ty, desc)| ((*name).to_string(), prop(ty, desc)))
            .collect(),
        required: required.iter().map(|s| s.to_string()).collect(),
        ..Default::default()
    }
}

fn tool(
    name: &str,
    description: &str,
    properties: &[(&str, &str, &str)],
    required: &[&str],
) -> ToolDefinition {
    ToolDefinition {
        name: name.to_string(),
        description: description.to_string(),
        input_schema: schema(properties, required),
    }
}

fn tool_catalog() -> Vec<ToolDefinition> {
    vec![
        tool(
            consts::REGISTER_AGENT,
            "Register this agent with Skwad crew. Call this first before using other tools.",
            &[
                ("agentId", "string", "The agent ID provided by Skwad"),
                ("sessionId", "string", "Your internal session ID."),
            ],
            &["agentId"],
        ),
        tool(
            consts::LIST_AGENTS,
            "List all registered agents with their status (name, folder, working/idle)",
            &[("agentId", "string", "Your agent ID")],
            &["agentId"],
        ),
        tool(
            consts::SEND_MESSAGE,
            "Send a message to another agent by name or ID",
            &[
                ("from", "string", "Your agent ID"),
                ("to", "string", "Recipient agent name or ID"),
                ("content", "string", "Message content"),
            ],
            &["from", "to", "content"],
        ),
        tool(
            consts::CHECK_MESSAGES,
            "Check your inbox for new messages from other agents",
            &[
                ("agentId", "string", "Your agent ID"),
                (
                    "markAsRead",
                    "boolean",
                    "Mark messages as read (default: true)",
                ),
            ],
            &["agentId"],
        ),
        tool(
            consts::BROADCAST_MESSAGE,
            "Send a message to all other registered agents",
            &[
                ("from", "string", "Your agent ID"),
                ("content", "string", "Message content"),
            ],
            &["from", "content"],
        ),
        tool(
            consts::LIST_REPOS,
            "List all git repositories in the configured source folder",
            &[],
            &[],
        ),
        tool(
            consts::LIST_WORKTREES,
            "List all worktrees for a given repository",
            &[("repoPath", "string", "Path to the repository")],
            &["repoPath"],
        ),
        tool(
            consts::CREATE_AGENT,
            "Create a new agent in Skwad. Can optionally create a new git worktree for the agent. Note: shell agents are plain terminals without an AI agent, so do not try to send messages to them.",
            &[
                (
                    "agentId",
                    "string",
                    "Your agent ID (used to track who created the agent)",
                ),
                (
                    "benchAgentId",
                    "string",
                    "ID of a bench agent to deploy. When provided, name/agentType/repoPath are optional and default to the bench agent's configuration.",
                ),
                ("name", "string", "Name for the agent"),
                ("icon", "string", "Emoji icon for the agent (e.g., '🤖')"),
                (
                    "agentType",
                    "string",
                    "Agent type: claude, codex, opencode, gemini, copilot, custom1, custom2, or shell",
                ),
                (
                    "repoPath",
                    "string",
                    "Path to the repository or worktree folder",
                ),
                (
                    "createWorktree",
                    "boolean",
                    "If true, create a new worktree from repoPath",
                ),
                (
                    "branchName",
                    "string",
                    "Branch name for new worktree (required if createWorktree is true)",
                ),
                (
                    "companion",
                    "boolean",
                    "If true, the new agent is a companion of the creator: it won't appear in the agent list, its visibility is linked to the creator, and it will be closed when the creator is closed. Only use this flag if the user has explicitly asked for a companion agent.",
                ),
                (
                    "command",
                    "string",
                    "Command to run (only for shell agent type)",
                ),
                (
                    "personaId",
                    "string",
                    "ID of a persona to apply. Only works with agents that support system prompts (claude, codex).",
                ),
            ],
            &["agentId"],
        ),
        tool(
            consts::CLOSE_AGENT,
            "Close an agent that you created. You can only close agents that you created, not agents created by the user or other agents.",
            &[
                ("agentId", "string", "Your agent ID"),
                ("target", "string", "The agent to close (name or ID)"),
            ],
            &["agentId", "target"],
        ),
        tool(
            consts::CREATE_WORKTREE,
            "Create a new git worktree from a repository. Returns the path to the new worktree.",
            &[
                ("repoPath", "string", "Path to the source repository"),
                ("branchName", "string", "Branch name for the new worktree"),
            ],
            &["repoPath", "branchName"],
        ),
        tool(
            consts::SET_STATUS,
            "MANDATORY: Set your status so other agents know what you are doing. Call before starting any task, after completing it, and when changing direction. Keep it short and specific (e.g. 'Implementing auth module', 'Running tests', 'Done — PR ready'). Use empty string to clear.",
            &[
                ("agentId", "string", "Your agent ID"),
                (
                    "status",
                    "string",
                    "Short status text describing what you are currently doing. Use empty string to clear.",
                ),
            ],
            &["agentId", "status"],
        ),
        tool(
            consts::DISPLAY_MARKDOWN,
            "Display a markdown file in a panel for the user to review. Use this to show plans, documentation, or any markdown content that needs user attention. Also use if the user asks you to show him a file. Never assume the panel is open or displaying the right file as the user may have closed it: call the tool again when relevant.",
            &[
                ("agentId", "string", "Your agent ID"),
                (
                    "filePath",
                    "string",
                    "Absolute path to the markdown file to display",
                ),
                (
                    "maximized",
                    "boolean",
                    "If true, maximize the panel to fill the available space. Only set to true if the user explicitly requests it. Default: false",
                ),
            ],
            &["agentId", "filePath"],
        ),
        tool(
            consts::VIEW_MERMAID,
            "Display a Mermaid diagram in a panel for the user to view. Supports flowcharts (graph TD/LR), state diagrams, sequence diagrams, class diagrams, and ER diagrams. Pass the mermaid source text directly. The diagram will be rendered natively alongside any open markdown panel.",
            &[
                ("agentId", "string", "Your agent ID"),
                (
                    "source",
                    "string",
                    "Mermaid diagram source text (e.g. 'graph TD; A-->B;')",
                ),
                (
                    "title",
                    "string",
                    "Optional title to display above the diagram",
                ),
            ],
            &["agentId", "source"],
        ),
    ]
}

#[async_trait]
impl ToolCatalog for McpToolCatalog {
    fn list(&self) -> Vec<ToolDefinition> {
        tool_catalog()
    }

    async fn call(&self, name: &str, arguments: serde_json::Value) -> ToolCallResult {
        match name {
            consts::REGISTER_AGENT => {
                agents::register_agent(&mut self.agents.lock().unwrap(), &arguments)
            }
            consts::LIST_AGENTS => agents::list_agents(&self.agents.lock().unwrap(), &arguments),
            consts::SEND_MESSAGE => messaging::send_message(
                &self.agents.lock().unwrap(),
                &mut self.messages.lock().unwrap(),
                self.notifier.as_ref(),
                &arguments,
            ),
            consts::CHECK_MESSAGES => messaging::check_messages(
                &self.agents.lock().unwrap(),
                &mut self.messages.lock().unwrap(),
                &arguments,
            ),
            consts::BROADCAST_MESSAGE => messaging::broadcast_message(
                &self.agents.lock().unwrap(),
                &mut self.messages.lock().unwrap(),
                self.notifier.as_ref(),
                &arguments,
            ),
            consts::LIST_REPOS => repos::list_repos(&self.repos.borrow()),
            consts::LIST_WORKTREES => repos::list_worktrees(&self.repos.borrow(), &arguments),
            consts::CREATE_AGENT => agents::create_agent(
                &mut self.agents.lock().unwrap(),
                &arguments,
                &self.bench_agents.lock().unwrap(),
            ),
            consts::CLOSE_AGENT => {
                agents::close_agent(&mut self.agents.lock().unwrap(), &arguments)
            }
            consts::CREATE_WORKTREE => repos::create_worktree(&arguments),
            consts::SET_STATUS => agents::set_status(&mut self.agents.lock().unwrap(), &arguments),
            consts::DISPLAY_MARKDOWN => {
                panels::display_markdown(&mut self.agents.lock().unwrap(), &arguments)
            }
            consts::VIEW_MERMAID => {
                panels::view_mermaid(&mut self.agents.lock().unwrap(), &arguments)
            }
            other => ToolCallResult::error(format!("unknown tool: {other}")),
        }
    }
}

#[cfg(test)]
mod tests {
    use skwad_messaging::NoopNotifier;

    use super::*;

    fn catalog() -> McpToolCatalog {
        let (_tx, rx) = watch::channel(Vec::new());
        McpToolCatalog::new(AgentStore::new(), rx, Arc::new(NoopNotifier))
    }

    #[test]
    fn lists_exactly_the_thirteen_tools_with_object_schemas() {
        let defs = catalog().list();
        assert_eq!(defs.len(), 13);
        for def in &defs {
            assert_eq!(def.input_schema.schema_type, "object");
        }

        let names: Vec<&str> = defs.iter().map(|d| d.name.as_str()).collect();
        for expected in [
            "register-agent",
            "list-agents",
            "send-message",
            "check-messages",
            "broadcast-message",
            "list-repos",
            "list-worktrees",
            "create-agent",
            "close-agent",
            "create-worktree",
            "set-status",
            "display-markdown",
            "view-mermaid",
        ] {
            assert!(names.contains(&expected), "missing tool: {expected}");
        }
    }

    #[tokio::test]
    async fn unknown_tool_name_is_a_tool_error() {
        let result = catalog()
            .call("does-not-exist", serde_json::json!({}))
            .await;
        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("does-not-exist"));
    }

    #[tokio::test]
    async fn dispatches_register_agent_by_name() {
        let cat = catalog();
        let id = cat
            .agents
            .lock()
            .unwrap()
            .create("/tmp/a", skwad_agents::CreateOptions::default());

        let result = cat
            .call(
                consts::REGISTER_AGENT,
                serde_json::json!({"agentId": id.to_string()}),
            )
            .await;

        assert_eq!(result.is_error, None);
        assert!(cat.agents.lock().unwrap().agent(id).unwrap().is_registered);
    }

    #[test]
    fn hook_registration_updates_session_and_metadata() {
        let cat = catalog();
        let id = cat
            .agents
            .lock()
            .unwrap()
            .create("/tmp/a", skwad_agents::CreateOptions::default());
        let request: HookRequest = serde_json::from_value(serde_json::json!({
            "agent_id": id,
            "session_id": "session-1",
            "payload": {"cwd": "/tmp/a", "model": "claude-sonnet"}
        }))
        .unwrap();

        cat.register(&request).unwrap();
        let agent = cat.agents.lock().unwrap().agent(id).unwrap().clone();
        assert!(agent.is_registered);
        assert_eq!(agent.session_id.as_deref(), Some("session-1"));
        assert_eq!(
            agent.metadata.get("model"),
            Some(&"claude-sonnet".to_string())
        );
    }

    #[test]
    fn hook_status_updates_state_and_codex_session() {
        let cat = catalog();
        let id = cat
            .agents
            .lock()
            .unwrap()
            .create("/tmp/a", skwad_agents::CreateOptions::default());
        let request: HookRequest = serde_json::from_value(serde_json::json!({
            "agent_id": id,
            "agent": "codex",
            "payload": {"type": "agent-turn-complete", "thread-id": "thread-1"}
        }))
        .unwrap();

        cat.status(&request).unwrap();
        let agent = cat.agents.lock().unwrap().agent(id).unwrap().clone();
        assert_eq!(agent.state, AgentState::Idle);
        assert_eq!(agent.session_id.as_deref(), Some("thread-1"));
    }
}
