//! `LaunchRequest` and the two top-level builders: the full agent command
//! and the terminal initialization wrapper.

use std::path::Path;

use skwad_core::{Persona, Settings};
use uuid::Uuid;

use crate::capabilities::{can_fork, can_resume, supports_inline_registration};
use crate::registration::{inline_registration_arguments, mcp_arguments};

/// Per-launch inputs to [`build_agent_command`]. Everything durable
/// (commands, options, MCP config) comes from `Settings` instead.
#[derive(Debug, Clone, Default)]
pub struct LaunchRequest<'a> {
    pub agent_type: &'a str,
    pub agent_id: Option<Uuid>,
    pub shell_command: Option<&'a str>,
    pub resume_session_id: Option<&'a str>,
    pub fork_session: bool,
    pub persona: Option<&'a Persona>,
    pub plugin_root: Option<&'a Path>,
}

/// The configured base command for `agent_type`: the stored override if
/// one is set, otherwise the agent type name itself for predefined types
/// (`custom1`/`custom2` default to empty - they have no built-in binary).
fn base_command(settings: &Settings, agent_type: &str) -> String {
    if let Some(cmd) = settings.agent_commands.get(agent_type) {
        return cmd.clone();
    }
    match agent_type {
        "custom1" | "custom2" => String::new(),
        _ => agent_type.to_string(),
    }
}

fn mcp_url(settings: &Settings) -> String {
    format!("http://127.0.0.1:{}/mcp", settings.mcp_server_port)
}

/// Build the full agent command: base command, resume/fork arguments, user
/// options, and (when MCP is enabled) MCP configuration plus inline
/// registration arguments. Per
/// `openspec/specs/agent-launch-command/spec.md`.
pub fn build_agent_command(settings: &Settings, request: &LaunchRequest<'_>) -> String {
    if request.agent_type == "shell" {
        return request.shell_command.unwrap_or_default().to_string();
    }

    let cmd = base_command(settings, request.agent_type);
    if cmd.is_empty() {
        return String::new();
    }

    let mut full = cmd;

    if let Some(session_id) = request.resume_session_id
        && can_resume(request.agent_type)
    {
        let fork = request.fork_session && can_fork(request.agent_type);
        match request.agent_type {
            "codex" => {
                if fork {
                    full.push_str(&format!(" fork {session_id}"));
                } else {
                    full.push_str(&format!(" resume {session_id}"));
                }
            }
            _ => {
                full.push_str(&format!(" --resume {session_id}"));
                if fork {
                    full.push_str(" --fork-session");
                }
            }
        }
    }

    if let Some(opts) = settings.agent_options.get(request.agent_type)
        && !opts.is_empty()
    {
        full.push(' ');
        full.push_str(opts);
    }

    if settings.mcp_server_enabled {
        full.push_str(&mcp_arguments(
            request.agent_type,
            &mcp_url(settings),
            request.plugin_root,
        ));

        if let Some(agent_id) = request.agent_id
            && supports_inline_registration(request.agent_type)
        {
            full.push_str(&inline_registration_arguments(
                request.agent_type,
                agent_id,
                request.resume_session_id.is_some(),
                request.persona,
            ));
        }
    }

    full
}

/// Build the terminal initialization command: `cd` into `folder`, clear the
/// screen, then (for a non-empty `agent_command`) set `SKWAD_AGENT_ID` and
/// run it. The leading space suppresses shell history under `ignorespace`.
pub fn build_initialization_command(
    folder: &str,
    agent_command: &str,
    agent_id: Option<Uuid>,
) -> String {
    if agent_command.is_empty() {
        return format!(" cd '{folder}' && clear");
    }
    let env_prefix = agent_id
        .map(|id| format!("SKWAD_AGENT_ID={id} "))
        .unwrap_or_default();
    format!(" cd '{folder}' && clear && {env_prefix}{agent_command}")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn settings() -> Settings {
        Settings::default()
    }

    #[test]
    fn missing_command_yields_nothing() {
        let mut s = settings();
        s.agent_commands.insert("claude".to_string(), String::new());
        let req = LaunchRequest {
            agent_type: "claude",
            ..Default::default()
        };
        assert_eq!(build_agent_command(&s, &req), "");
    }

    #[test]
    fn default_command_falls_back_to_agent_type() {
        let mut s = settings();
        s.mcp_server_enabled = false;
        let req = LaunchRequest {
            agent_type: "claude",
            ..Default::default()
        };
        assert_eq!(build_agent_command(&s, &req), "claude");
    }

    #[test]
    fn shell_agent_custom_command() {
        let s = settings();
        let req = LaunchRequest {
            agent_type: "shell",
            shell_command: Some("htop"),
            ..Default::default()
        };
        assert_eq!(build_agent_command(&s, &req), "htop");
    }

    #[test]
    fn shell_agent_no_custom_command_is_empty() {
        let s = settings();
        let req = LaunchRequest {
            agent_type: "shell",
            ..Default::default()
        };
        assert_eq!(build_agent_command(&s, &req), "");
    }

    #[test]
    fn claude_resume_with_fork() {
        let mut s = settings();
        s.mcp_server_enabled = false;
        let req = LaunchRequest {
            agent_type: "claude",
            resume_session_id: Some("abc123"),
            fork_session: true,
            ..Default::default()
        };
        assert_eq!(
            build_agent_command(&s, &req),
            "claude --resume abc123 --fork-session"
        );
    }

    #[test]
    fn codex_fork_uses_subcommand() {
        let mut s = settings();
        s.mcp_server_enabled = false;
        let req = LaunchRequest {
            agent_type: "codex",
            resume_session_id: Some("abc123"),
            fork_session: true,
            ..Default::default()
        };
        let cmd = build_agent_command(&s, &req);
        assert!(cmd.contains("fork abc123"));
        assert!(!cmd.contains("--resume"));
    }

    #[test]
    fn mcp_disabled_omits_all_mcp_and_registration_args() {
        let mut s = settings();
        s.mcp_server_enabled = false;
        let req = LaunchRequest {
            agent_type: "claude",
            agent_id: Some(Uuid::nil()),
            ..Default::default()
        };
        assert_eq!(build_agent_command(&s, &req), "claude");
    }

    #[test]
    fn mcp_enabled_appends_registration() {
        let s = settings();
        let req = LaunchRequest {
            agent_type: "claude",
            agent_id: Some(Uuid::nil()),
            ..Default::default()
        };
        let cmd = build_agent_command(&s, &req);
        assert!(cmd.contains("--mcp-config"));
        assert!(cmd.contains("--append-system-prompt"));
    }

    #[test]
    fn env_var_precedes_agent_command() {
        let cmd = build_initialization_command("/tmp/repo", "claude", Some(Uuid::nil()));
        assert_eq!(
            cmd,
            format!(
                " cd '/tmp/repo' && clear && SKWAD_AGENT_ID={} claude",
                Uuid::nil()
            )
        );
    }

    #[test]
    fn shell_agent_wrapper_has_no_env_var() {
        let cmd = build_initialization_command("/tmp/repo", "", None);
        assert_eq!(cmd, " cd '/tmp/repo' && clear");
    }
}
