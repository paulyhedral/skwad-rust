use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Automatic state-machine state, driven by terminal activity and hooks.
/// Distinct from `Agent::status_text` (agent-set) and `Agent::terminal_title`
/// (terminal escape sequences).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum AgentState {
    #[default]
    #[serde(rename = "Idle")]
    Idle,
    #[serde(rename = "Working")]
    Running,
    #[serde(rename = "Awaiting input")]
    Input,
    #[serde(rename = "Error")]
    Error,
}

/// A running agent: the durable fields mirrored from `SavedAgent` plus
/// runtime-only state that resets to its default on every reload.
///
/// Build one with [`crate::convert::from_saved`], never by hand, so runtime
/// fields can't leak stale values from persistence.
#[derive(Debug, Clone, PartialEq)]
pub struct Agent {
    // Durable (mirrors `skwad_core::SavedAgent`)
    pub id: Uuid,
    pub name: String,
    pub avatar: String,
    pub folder: String,
    pub agent_type: String,
    pub created_by: Option<Uuid>,
    pub is_companion: bool,
    pub shell_command: Option<String>,
    pub persona_id: Option<Uuid>,

    // Runtime-only
    pub state: AgentState,
    pub status_text: String,
    pub is_registered: bool,
    pub is_pending_start: bool,
    pub terminal_title: String,
    pub restart_token: Uuid,
    pub session_id: Option<String>,
    pub resume_session_id: Option<String>,
    pub fork_session: bool,
    pub metadata: BTreeMap<String, String>,

    // Panel state, set by the `display-markdown` / `view-mermaid` MCP
    // tools. Not yet consumed by any UI.
    pub markdown_file: Option<PathBuf>,
    pub markdown_maximized: bool,
    /// Most recent first.
    pub markdown_history: Vec<PathBuf>,
    pub mermaid_source: Option<String>,
    pub mermaid_title: Option<String>,
}

impl Agent {
    /// Whether this is a plain shell agent (no AI).
    pub fn is_shell(&self) -> bool {
        self.agent_type == "shell"
    }

    /// Title for the terminal header: prefers agent-set status text over the
    /// terminal-reported title.
    pub fn header_title(&self) -> &str {
        if self.status_text.is_empty() {
            &self.terminal_title
        } else {
            &self.status_text
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn agent_state_serializes_to_swift_raw_strings() {
        assert_eq!(
            serde_json::to_string(&AgentState::Idle).unwrap(),
            "\"Idle\""
        );
        assert_eq!(
            serde_json::to_string(&AgentState::Running).unwrap(),
            "\"Working\""
        );
        assert_eq!(
            serde_json::to_string(&AgentState::Input).unwrap(),
            "\"Awaiting input\""
        );
        assert_eq!(
            serde_json::to_string(&AgentState::Error).unwrap(),
            "\"Error\""
        );
    }

    #[test]
    fn header_title_prefers_status_text() {
        let mut agent = test_agent();
        agent.status_text = "Refactoring auth".to_string();
        agent.terminal_title = "zsh".to_string();
        assert_eq!(agent.header_title(), "Refactoring auth");
    }

    #[test]
    fn header_title_falls_back_to_terminal_title() {
        let mut agent = test_agent();
        agent.status_text = String::new();
        agent.terminal_title = "zsh".to_string();
        assert_eq!(agent.header_title(), "zsh");
    }

    pub(crate) fn test_agent() -> Agent {
        Agent {
            id: Uuid::new_v4(),
            name: "proj".to_string(),
            avatar: "🤖".to_string(),
            folder: "/tmp/proj".to_string(),
            agent_type: "claude".to_string(),
            created_by: None,
            is_companion: false,
            shell_command: None,
            persona_id: None,
            state: AgentState::Idle,
            status_text: String::new(),
            is_registered: false,
            is_pending_start: false,
            terminal_title: String::new(),
            restart_token: Uuid::new_v4(),
            session_id: None,
            resume_session_id: None,
            fork_session: false,
            metadata: BTreeMap::new(),
            markdown_file: None,
            markdown_maximized: false,
            markdown_history: Vec::new(),
            mermaid_source: None,
            mermaid_title: None,
        }
    }
}
