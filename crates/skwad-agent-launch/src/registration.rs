//! Registration prompt text, MCP argument assembly, and the inline
//! registration arguments appended when MCP is enabled and an agent id is
//! known.

use std::path::{Path, PathBuf};

use skwad_core::Persona;
use uuid::Uuid;

use crate::consts::{COPILOT_ALLOWED_TOOLS, REGISTRATION_USER_PROMPT};
use crate::escape::{persona_prompt, shell_escape};

/// The user prompt sent on first launch to trigger the agent list table.
pub fn registration_user_prompt() -> &'static str {
    REGISTRATION_USER_PROMPT
}

/// The skwad system instructions, with `agent_id` embedded.
pub fn skwad_instructions(agent_id: Uuid) -> String {
    format!(
        "You are part of a team of agents called a skwad. A skwad is made of high-performing agents who collaborate to achieve complex goals so engage with them: ask for help and in return help them succeed. Your skwad agent ID: {agent_id}. CRITICAL RULE: Before you start working on anything, your FIRST action must be calling set-status with what you are about to do. When you finish, call set-status again. When you change direction, call set-status. Other agents depend on your status to coordinate — if you do not update it, the team cannot function. This is not optional."
    )
}

/// The combined registration prompt for agent types without system-prompt
/// support: skwad instructions plus an explicit registration request.
pub fn registration_prompt(agent_id: Uuid) -> String {
    format!("{} Register with the skwad", skwad_instructions(agent_id))
}

/// Resolve `<plugin_root>/<agent_type>` when it exists on disk.
fn resolve_plugin_dir(plugin_root: Option<&Path>, agent_type: &str) -> Option<PathBuf> {
    let dir = plugin_root?.join(agent_type);
    dir.is_dir().then_some(dir)
}

/// MCP configuration/allow-list/hook arguments for `agent_type`, or an empty
/// string for types with no MCP wiring.
pub fn mcp_arguments(agent_type: &str, mcp_url: &str, plugin_root: Option<&Path>) -> String {
    match agent_type {
        "claude" => {
            let mut args = format!(
                r#" --mcp-config '{{"mcpServers":{{"skwad":{{"type":"http","url":"{mcp_url}"}}}}}}' --allowed-tools 'mcp__skwad__*'"#
            );
            if let Some(dir) = resolve_plugin_dir(plugin_root, agent_type) {
                args.push_str(&format!(" --plugin-dir \"{}\"", dir.display()));
            }
            args
        }
        "codex" => match resolve_plugin_dir(plugin_root, agent_type) {
            Some(dir) => {
                format!(
                    r#" -c 'notify=["bash","{}/scripts/notify.sh"]'"#,
                    dir.display()
                )
            }
            None => String::new(),
        },
        "gemini" => " --allowed-mcp-server-names skwad".to_string(),
        "copilot" => {
            let mcp_config = format!(
                r#"--additional-mcp-config '{{"mcpServers":{{"skwad":{{"type":"http","url":"{mcp_url}","tools":["*"]}}}}}}'"#
            );
            let allowed_tools = COPILOT_ALLOWED_TOOLS
                .iter()
                .map(|tool| format!("--allow-tool 'skwad({tool})'"))
                .collect::<Vec<_>>()
                .join(" ");
            format!(" {mcp_config} {allowed_tools}")
        }
        _ => String::new(),
    }
}

/// Inline registration arguments carrying `agent_id`, or an empty string
/// when `agent_type` does not support inline registration or the resume
/// rules drop them.
pub fn inline_registration_arguments(
    agent_type: &str,
    agent_id: Uuid,
    is_resume: bool,
    persona: Option<&Persona>,
) -> String {
    match agent_type {
        "claude" => {
            let mut system_prompt = skwad_instructions(agent_id);
            if let Some(p) = persona_prompt(persona) {
                system_prompt.push(' ');
                system_prompt.push_str(&shell_escape(&p));
            }
            if is_resume {
                format!(r#" --append-system-prompt "{system_prompt}""#)
            } else {
                format!(
                    r#" --append-system-prompt "{system_prompt}" "{}""#,
                    registration_user_prompt()
                )
            }
        }
        "codex" => {
            let mut system_prompt = skwad_instructions(agent_id);
            if let Some(p) = persona_prompt(persona) {
                system_prompt.push(' ');
                system_prompt.push_str(&shell_escape(&p));
            }
            if is_resume {
                format!(r#" -c 'developer_instructions="{system_prompt}"'"#)
            } else {
                format!(
                    r#" -c 'developer_instructions="{system_prompt}"' "{}""#,
                    registration_user_prompt()
                )
            }
        }
        "opencode" => {
            if is_resume {
                String::new()
            } else {
                format!(r#" --prompt "{}""#, registration_prompt(agent_id))
            }
        }
        "gemini" => {
            if is_resume {
                String::new()
            } else {
                format!(
                    r#" --prompt-interactive "{}""#,
                    registration_prompt(agent_id)
                )
            }
        }
        "copilot" => {
            if is_resume {
                String::new()
            } else {
                format!(r#" --interactive "{}""#, registration_prompt(agent_id))
            }
        }
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skwad_core::{PersonaState, PersonaType};
    use tempfile::tempdir;

    fn id() -> Uuid {
        Uuid::nil()
    }

    fn persona(instructions: &str) -> Persona {
        Persona {
            id: Uuid::nil(),
            name: "Ada".to_string(),
            instructions: instructions.to_string(),
            persona_type: PersonaType::User,
            state: PersonaState::Enabled,
        }
    }

    #[test]
    fn registration_prompt_embeds_agent_id_verbatim() {
        let prompt = registration_prompt(id());
        assert!(prompt.contains(&id().to_string()));
        assert!(prompt.ends_with("Register with the skwad"));
    }

    #[test]
    fn claude_mcp_arguments_without_plugin_dir() {
        let args = mcp_arguments("claude", "http://127.0.0.1:8766/mcp", None);
        assert!(args.contains("--mcp-config"));
        assert!(args.contains("--allowed-tools 'mcp__skwad__*'"));
        assert!(!args.contains("--plugin-dir"));
    }

    #[test]
    fn claude_mcp_arguments_with_plugin_dir() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("claude")).unwrap();
        let args = mcp_arguments("claude", "http://127.0.0.1:8766/mcp", Some(dir.path()));
        assert!(args.contains("--plugin-dir"));
    }

    #[test]
    fn codex_mcp_arguments_only_notify_hook() {
        let dir = tempdir().unwrap();
        std::fs::create_dir(dir.path().join("codex")).unwrap();
        let args = mcp_arguments("codex", "http://127.0.0.1:8766/mcp", Some(dir.path()));
        assert!(args.contains("notify=[\"bash\""));
        assert!(!args.contains("--mcp-config"));
    }

    #[test]
    fn codex_mcp_arguments_absent_plugin_dir_is_empty() {
        assert_eq!(
            mcp_arguments("codex", "http://127.0.0.1:8766/mcp", None),
            ""
        );
    }

    #[test]
    fn gemini_mcp_arguments() {
        assert_eq!(
            mcp_arguments("gemini", "http://127.0.0.1:8766/mcp", None),
            " --allowed-mcp-server-names skwad"
        );
    }

    #[test]
    fn copilot_mcp_arguments() {
        let args = mcp_arguments("copilot", "http://127.0.0.1:8766/mcp", None);
        assert!(args.contains("--additional-mcp-config"));
        assert!(args.contains("--allow-tool 'skwad(register-agent)'"));
        assert!(args.contains("--allow-tool 'skwad(broadcast-message)'"));
    }

    #[test]
    fn unsupported_type_has_no_mcp_arguments() {
        assert_eq!(
            mcp_arguments("opencode", "http://127.0.0.1:8766/mcp", None),
            ""
        );
        assert_eq!(
            mcp_arguments("shell", "http://127.0.0.1:8766/mcp", None),
            ""
        );
    }

    #[test]
    fn claude_resume_keeps_system_prompt_only() {
        let args = inline_registration_arguments("claude", id(), true, None);
        assert!(args.contains("--append-system-prompt"));
        assert!(!args.contains(registration_user_prompt()));
    }

    #[test]
    fn claude_fresh_launch_includes_registration_user_prompt() {
        let args = inline_registration_arguments("claude", id(), false, None);
        assert!(args.contains(registration_user_prompt()));
    }

    #[test]
    fn persona_reaches_claude_system_prompt() {
        let p = persona("Be terse.");
        let args = inline_registration_arguments("claude", id(), false, Some(&p));
        assert!(args.contains("impersonate Ada"));
    }

    #[test]
    fn persona_dropped_for_gemini() {
        let p = persona("Be terse.");
        let args = inline_registration_arguments("gemini", id(), false, Some(&p));
        assert!(!args.contains("impersonate"));
    }

    #[test]
    fn codex_fork_uses_developer_instructions() {
        let args = inline_registration_arguments("codex", id(), false, None);
        assert!(args.contains("developer_instructions"));
        assert!(args.contains(registration_user_prompt()));
    }

    #[test]
    fn gemini_resume_adds_no_registration_args() {
        assert_eq!(
            inline_registration_arguments("gemini", id(), true, None),
            ""
        );
    }

    #[test]
    fn opencode_and_copilot_resume_add_no_registration_args() {
        assert_eq!(
            inline_registration_arguments("opencode", id(), true, None),
            ""
        );
        assert_eq!(
            inline_registration_arguments("copilot", id(), true, None),
            ""
        );
    }

    #[test]
    fn unsupported_type_has_no_inline_registration() {
        assert_eq!(
            inline_registration_arguments("shell", id(), false, None),
            ""
        );
        assert_eq!(
            inline_registration_arguments("unknown", id(), false, None),
            ""
        );
    }
}
