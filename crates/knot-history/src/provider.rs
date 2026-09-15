//! The per-agent history provider trait and the registry that resolves one
//! by agent type.

use time::OffsetDateTime;

use crate::providers::{
    claude::ClaudeProvider, codex::CodexProvider, copilot::CopilotProvider, gemini::GeminiProvider,
};

/// A summary of one past agent session for a project folder.
#[derive(Debug, Clone, PartialEq)]
pub struct SessionSummary {
    /// Stable session identifier (provider-specific: a file stem, a DB row
    /// id, a directory name).
    pub id: String,
    /// First meaningful user message; empty when none could be derived.
    pub title: String,
    /// Best available recency signal for this session.
    pub timestamp: OffsetDateTime,
    /// User + assistant message count; zero when not derivable.
    pub message_count: usize,
}

/// Reads and deletes sessions for one coding-agent's on-disk history format.
///
/// Implementations swallow their own I/O and parse errors: an unreadable
/// session is dropped (or, for the most recent file, listed with an empty
/// title) rather than surfaced, and a failed delete is a no-op.
pub trait HistoryProvider {
    /// Load up to `MAX_SESSIONS` sessions for `folder`, most recent first.
    fn load_sessions(&self, folder: &str) -> Vec<SessionSummary>;
    /// Delete a session's on-disk files.
    fn delete_session(&self, id: &str, folder: &str);
}

/// Resolve a history provider by agent type. `None` for any type without a
/// provider.
pub fn provider(agent_type: &str) -> Option<Box<dyn HistoryProvider>> {
    match agent_type {
        "claude" => Some(Box::new(ClaudeProvider)),
        "codex" => Some(Box::new(CodexProvider)),
        "gemini" => Some(Box::new(GeminiProvider)),
        "copilot" => Some(Box::new(CopilotProvider)),
        _ => None,
    }
}

/// Whether conversation history is available for `agent_type`.
pub fn supports_history(agent_type: &str) -> bool {
    provider(agent_type).is_some()
}

#[cfg(test)]
mod tests {
    use super::supports_history;

    #[test]
    fn known_agent_types_supported() {
        for agent_type in ["claude", "codex", "gemini", "copilot"] {
            assert!(
                supports_history(agent_type),
                "{agent_type} should be supported"
            );
        }
    }

    #[test]
    fn unknown_agent_type_unsupported() {
        assert!(!supports_history("shell"));
    }
}
