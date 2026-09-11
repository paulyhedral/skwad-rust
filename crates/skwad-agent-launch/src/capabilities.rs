//! Per-agent-type capability lookups the command builder consults before
//! adding resume, fork, system-prompt, or registration arguments.

/// Whether `agent_type` supports resuming a prior session (`--resume`,
/// `resume <id>`).
pub fn can_resume(agent_type: &str) -> bool {
    matches!(agent_type, "claude" | "codex" | "gemini" | "copilot")
}

/// Whether `agent_type` supports forking a resumed session
/// (`--fork-session`, `fork <id>`).
pub fn can_fork(agent_type: &str) -> bool {
    matches!(agent_type, "claude" | "codex")
}

/// Whether `agent_type` accepts a system prompt (`--append-system-prompt`,
/// `-c developer_instructions=...`).
pub fn supports_system_prompt(agent_type: &str) -> bool {
    matches!(agent_type, "claude" | "codex")
}

/// Whether `agent_type` supports inline registration via CLI arguments.
/// `shell` is included so shell agents skip the deferred registration path
/// entirely.
pub fn supports_inline_registration(agent_type: &str) -> bool {
    matches!(
        agent_type,
        "claude" | "codex" | "opencode" | "gemini" | "copilot" | "shell"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resume_support_matches_spec_table() {
        for t in ["claude", "codex", "gemini", "copilot"] {
            assert!(can_resume(t), "{t} should support resume");
        }
        for t in ["opencode", "shell", "unknown-type"] {
            assert!(!can_resume(t), "{t} should not support resume");
        }
    }

    #[test]
    fn fork_support_matches_spec_table() {
        for t in ["claude", "codex"] {
            assert!(can_fork(t), "{t} should support fork");
        }
        for t in ["opencode", "gemini", "copilot", "shell", "unknown-type"] {
            assert!(!can_fork(t), "{t} should not support fork");
        }
    }

    #[test]
    fn system_prompt_support_matches_spec_table() {
        for t in ["claude", "codex"] {
            assert!(
                supports_system_prompt(t),
                "{t} should support system prompt"
            );
        }
        for t in ["opencode", "gemini", "copilot", "shell", "unknown-type"] {
            assert!(
                !supports_system_prompt(t),
                "{t} should not support system prompt"
            );
        }
    }

    #[test]
    fn inline_registration_support_matches_spec_table() {
        for t in ["claude", "codex", "opencode", "gemini", "copilot", "shell"] {
            assert!(
                supports_inline_registration(t),
                "{t} should support inline registration"
            );
        }
        assert!(!supports_inline_registration("unknown-type"));
    }
}
