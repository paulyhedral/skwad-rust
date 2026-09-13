use skwad_agents::{Agent, AgentStore, CreateOptions};
use skwad_core::BenchAgent;
use skwad_mcp::ToolCallResult;
use uuid::Uuid;

use crate::args::{optional_bool, optional_str, require_str};
use crate::lookup::{agent_not_found, find_by_name_or_id, state_string, workspace_members};
use crate::responses::{
    AgentInfo, CloseAgentResponse, CreateAgentResponse, RegisterAgentResponse, success,
};

fn agent_info(agent: &Agent) -> AgentInfo {
    AgentInfo {
        id: agent.id.to_string(),
        name: agent.name.clone(),
        folder: agent.folder.clone(),
        status: state_string(agent.state),
        is_registered: agent.is_registered,
    }
}

/// `list-agents`, scoped to the caller's own workspace: companions are
/// excluded unless the caller created them.
fn list_agents_response(store: &AgentStore, caller_id: Uuid) -> Vec<AgentInfo> {
    workspace_members(store, caller_id)
        .into_iter()
        .filter(|a| !a.is_companion || a.created_by == Some(caller_id))
        .map(|a| agent_info(&a))
        .collect()
}

pub fn register_agent(store: &mut AgentStore, arguments: &serde_json::Value) -> ToolCallResult {
    let agent_id_str = match require_str(arguments, "agentId") {
        Ok(v) => v,
        Err(err) => return err,
    };
    let Ok(agent_id) = Uuid::parse_str(agent_id_str) else {
        return agent_not_found(store, agent_id_str);
    };
    if store.agent(agent_id).is_none() {
        return agent_not_found(store, agent_id_str);
    }

    store.set_registered(agent_id, true);
    if let Some(session_id) = optional_str(arguments, "sessionId") {
        store.set_session_id(agent_id, session_id.to_string());
    }

    let members = list_agents_response(store, agent_id);
    success(&RegisterAgentResponse {
        success: true,
        message: "Successfully registered with Skwad crew. Note: skwad members can change over time as agents join or leave. Use list-agents to get the current list.".to_string(),
        unread_message_count: 0,
        skwad_members: members,
    })
}

pub fn list_agents(store: &AgentStore, arguments: &serde_json::Value) -> ToolCallResult {
    let agent_id_str = match require_str(arguments, "agentId") {
        Ok(v) => v,
        Err(err) => return err,
    };
    let Some(caller) = find_by_name_or_id(store, agent_id_str) else {
        return agent_not_found(store, agent_id_str);
    };

    let agents = list_agents_response(store, caller.id);
    success(&crate::responses::ListAgentsResponse { agents })
}

/// Resolves `create-agent`'s fields, applying bench-template defaults for
/// whichever of name/icon/agentType/repoPath/command/personaId weren't
/// passed explicitly.
struct ResolvedCreateFields {
    name: Option<String>,
    icon: Option<String>,
    agent_type: Option<String>,
    repo_path: Option<String>,
    shell_command: Option<String>,
    persona_id: Option<Uuid>,
}

fn resolve_create_agent_fields(
    arguments: &serde_json::Value,
    bench: Option<&BenchAgent>,
) -> ResolvedCreateFields {
    ResolvedCreateFields {
        name: optional_str(arguments, "name")
            .map(str::to_string)
            .or_else(|| bench.map(|b| b.name.clone())),
        icon: optional_str(arguments, "icon")
            .map(str::to_string)
            .or_else(|| bench.map(|b| b.avatar.clone())),
        agent_type: optional_str(arguments, "agentType")
            .map(str::to_string)
            .or_else(|| bench.map(|b| b.agent_type.clone())),
        repo_path: optional_str(arguments, "repoPath")
            .map(str::to_string)
            .or_else(|| bench.map(|b| b.folder.clone())),
        shell_command: optional_str(arguments, "command")
            .map(str::to_string)
            .or_else(|| bench.and_then(|b| b.shell_command.clone())),
        persona_id: optional_str(arguments, "personaId")
            .and_then(|s| Uuid::parse_str(s).ok())
            .or_else(|| bench.and_then(|b| b.persona_id)),
    }
}

pub fn create_agent(
    store: &mut AgentStore,
    arguments: &serde_json::Value,
    bench_agents: &[BenchAgent],
) -> ToolCallResult {
    let agent_id_str = match require_str(arguments, "agentId") {
        Ok(v) => v,
        Err(err) => return err,
    };
    let Ok(created_by) = Uuid::parse_str(agent_id_str) else {
        return agent_not_found(store, agent_id_str);
    };

    let bench = match optional_str(arguments, "benchAgentId") {
        Some(id_str) => {
            let Ok(bench_id) = Uuid::parse_str(id_str) else {
                return ToolCallResult::error(format!("Bench agent not found: {id_str}"));
            };
            match bench_agents.iter().find(|b| b.id == bench_id) {
                Some(b) => Some(b),
                None => return ToolCallResult::error(format!("Bench agent not found: {id_str}")),
            }
        }
        None => None,
    };

    let fields = resolve_create_agent_fields(arguments, bench);

    let mut missing = Vec::new();
    if fields.name.is_none() {
        missing.push("name");
    }
    if fields.agent_type.is_none() {
        missing.push("agentType");
    }
    if fields.repo_path.is_none() {
        missing.push("repoPath");
    }
    if !missing.is_empty() {
        return ToolCallResult::error(format!(
            "Missing required parameters: {}. Provide these or use benchAgentId to deploy from a template.",
            missing.join(", ")
        ));
    }

    let create_worktree = optional_bool(arguments, "createWorktree").unwrap_or(false);
    let branch_name = optional_str(arguments, "branchName");
    if create_worktree && branch_name.is_none_or(str::is_empty) {
        return ToolCallResult::error("Missing required parameter: branchName");
    }

    let companion = optional_bool(arguments, "companion").unwrap_or(false);
    if companion && fields.agent_type.as_deref() != Some("shell") {
        return ToolCallResult::error("Companion agents must use agentType=shell");
    }
    let repo_path = fields.repo_path.unwrap();
    let folder = if create_worktree {
        match crate::repos::create_worktree_path(&repo_path, branch_name.unwrap()) {
            Ok(path) => path.to_string_lossy().into_owned(),
            Err(error) => return ToolCallResult::error(error),
        }
    } else {
        repo_path
    };

    let id = store.create(
        folder,
        CreateOptions {
            name: fields.name,
            avatar: fields.icon,
            agent_type: fields.agent_type,
            shell_command: fields.shell_command,
            persona_id: fields.persona_id,
            created_by: Some(created_by),
            is_companion: companion,
            insert_after: None,
        },
    );

    success(&CreateAgentResponse {
        success: true,
        agent_id: Some(id.to_string()),
        message: "Agent created successfully".to_string(),
    })
}

pub fn close_agent(store: &mut AgentStore, arguments: &serde_json::Value) -> ToolCallResult {
    let agent_id_str = match require_str(arguments, "agentId") {
        Ok(v) => v,
        Err(err) => return err,
    };
    let target_str = match require_str(arguments, "target") {
        Ok(v) => v,
        Err(err) => return err,
    };

    let Ok(caller_id) = Uuid::parse_str(agent_id_str) else {
        return agent_not_found(store, agent_id_str);
    };
    if store.agent(caller_id).is_none() {
        return agent_not_found(store, agent_id_str);
    }

    let Some(target) = crate::lookup::find_in_workspace(store, caller_id, target_str) else {
        return success(&CloseAgentResponse {
            success: false,
            message: format!("Target agent not found: {target_str}"),
        });
    };

    if target.created_by != Some(caller_id) {
        return success(&CloseAgentResponse {
            success: false,
            message: "Permission denied: you can only close agents that you created".to_string(),
        });
    }

    let name = target.name.clone();
    store.remove(target.id);
    success(&CloseAgentResponse {
        success: true,
        message: format!("Agent '{name}' closed successfully"),
    })
}

pub fn set_status(store: &mut AgentStore, arguments: &serde_json::Value) -> ToolCallResult {
    let agent_id_str = match require_str(arguments, "agentId") {
        Ok(v) => v,
        Err(err) => return err,
    };
    let status = match require_str(arguments, "status") {
        Ok(v) => v,
        Err(err) => return err,
    };

    let Some(agent) = find_by_name_or_id(store, agent_id_str) else {
        return agent_not_found(store, agent_id_str);
    };
    let id = agent.id;
    store.set_status_text(id, status.to_string());
    ToolCallResult::ok("Status updated")
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use serde_json::json;
    use skwad_git::Runner;

    use super::*;

    #[test]
    fn register_agent_returns_roster() {
        let mut store = AgentStore::new();
        let id = store.create("/tmp/a", CreateOptions::default());

        let result = register_agent(&mut store, &json!({"agentId": id.to_string()}));

        assert_eq!(result.is_error, None);
        assert!(store.agent(id).unwrap().is_registered);
        assert!(result.content[0].text.contains("skwadMembers"));
    }

    #[test]
    fn list_agents_excludes_unowned_companions() {
        let mut store = AgentStore::new();
        let owner = store.create("/tmp/owner", CreateOptions::default());
        let other = store.create(
            "/tmp/other",
            CreateOptions {
                insert_after: Some(owner),
                ..Default::default()
            },
        );
        store.create(
            "/tmp/comp",
            CreateOptions {
                is_companion: true,
                created_by: Some(owner),
                insert_after: Some(owner),
                ..Default::default()
            },
        );

        let result = list_agents(&store, &json!({"agentId": other.to_string()}));

        assert_eq!(result.is_error, None);
        assert!(!result.content[0].text.contains("comp"));
    }

    #[test]
    fn list_agents_unknown_caller_errors() {
        let store = AgentStore::new();
        let result = list_agents(&store, &json!({"agentId": "nope"}));
        assert_eq!(result.is_error, Some(true));
    }

    #[test]
    fn create_agent_from_explicit_fields() {
        let mut store = AgentStore::new();
        let caller = store.create("/tmp/caller", CreateOptions::default());

        let result = create_agent(
            &mut store,
            &json!({
                "agentId": caller.to_string(),
                "name": "worker",
                "agentType": "claude",
                "repoPath": "/tmp/worker",
            }),
            &[],
        );

        assert_eq!(result.is_error, None);
        let created = store.agents().iter().find(|a| a.name == "worker").unwrap();
        assert_eq!(created.created_by, Some(caller));
    }

    #[test]
    fn create_agent_from_bench_template() {
        let mut store = AgentStore::new();
        let caller = store.create("/tmp/caller", CreateOptions::default());
        let bench_id = Uuid::new_v4();
        let bench = BenchAgent {
            id: bench_id,
            name: "Bench Worker".to_string(),
            avatar: "🤖".to_string(),
            folder: "/tmp/bench".to_string(),
            agent_type: "codex".to_string(),
            shell_command: None,
            persona_id: None,
        };

        let result = create_agent(
            &mut store,
            &json!({"agentId": caller.to_string(), "benchAgentId": bench_id.to_string()}),
            &[bench],
        );

        assert_eq!(result.is_error, None);
        let created = store
            .agents()
            .iter()
            .find(|a| a.name == "Bench Worker")
            .unwrap();
        assert_eq!(created.folder, "/tmp/bench");
        assert_eq!(created.agent_type, "codex");
    }

    #[test]
    fn create_agent_worktree_without_branch_name_errors() {
        let mut store = AgentStore::new();
        let caller = store.create("/tmp/caller", CreateOptions::default());

        let result = create_agent(
            &mut store,
            &json!({
                "agentId": caller.to_string(),
                "name": "worker",
                "agentType": "claude",
                "repoPath": "/tmp/worker",
                "createWorktree": true,
            }),
            &[],
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("branchName"));
    }

    #[test]
    fn companion_requires_shell_agent_type() {
        let mut store = AgentStore::new();
        let caller = store.create("/tmp/caller", CreateOptions::default());

        let result = create_agent(
            &mut store,
            &json!({
                "agentId": caller.to_string(),
                "name": "companion",
                "agentType": "claude",
                "repoPath": "/tmp/companion",
                "companion": true,
            }),
            &[],
        );

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("agentType=shell"));
    }

    #[test]
    fn create_agent_uses_new_worktree_path() {
        let dir = tempfile::tempdir().unwrap();
        let repo = dir.path().join("repo");
        std::fs::create_dir(&repo).unwrap();
        let run = |args: &[&str]| Runner::new(&repo).run(args).unwrap();
        run(&["init", "-q", "-b", "main"]);
        run(&["config", "user.email", "test@example.com"]);
        run(&["config", "user.name", "Test"]);
        std::fs::write(repo.join("seed.txt"), "seed\n").unwrap();
        run(&["add", "-A"]);
        run(&[
            "-c",
            "commit.gpgsign=false",
            "-c",
            "gpg.format=openpgp",
            "commit",
            "-qm",
            "init",
        ]);

        let mut store = AgentStore::new();
        let caller = store.create("/tmp/caller", CreateOptions::default());
        let result = create_agent(
            &mut store,
            &json!({
                "agentId": caller.to_string(),
                "name": "worker",
                "agentType": "claude",
                "repoPath": repo,
                "createWorktree": true,
                "branchName": "feature/worker",
            }),
            &[],
        );

        assert_eq!(result.is_error, None);
        let created = store.agents().iter().find(|a| a.name == "worker").unwrap();
        assert_ne!(created.folder, repo.to_string_lossy());
        assert!(Path::new(&created.folder).is_dir());
    }

    #[test]
    fn create_agent_missing_fields_without_bench_id_errors() {
        let mut store = AgentStore::new();
        let caller = store.create("/tmp/caller", CreateOptions::default());

        let result = create_agent(&mut store, &json!({"agentId": caller.to_string()}), &[]);

        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("name"));
        assert!(result.content[0].text.contains("agentType"));
        assert!(result.content[0].text.contains("repoPath"));
    }

    #[test]
    fn close_agent_by_creator_succeeds() {
        let mut store = AgentStore::new();
        let caller = store.create("/tmp/caller", CreateOptions::default());
        let target = store.create(
            "/tmp/target",
            CreateOptions {
                created_by: Some(caller),
                insert_after: Some(caller),
                ..Default::default()
            },
        );

        let result = close_agent(
            &mut store,
            &json!({"agentId": caller.to_string(), "target": target.to_string()}),
        );

        assert_eq!(result.is_error, None);
        assert!(store.agent(target).is_none());
    }

    #[test]
    fn close_agent_rejects_non_creator() {
        let mut store = AgentStore::new();
        let caller = store.create("/tmp/caller", CreateOptions::default());
        let target = store.create(
            "/tmp/target",
            CreateOptions {
                insert_after: Some(caller),
                ..Default::default()
            },
        );

        let result = close_agent(
            &mut store,
            &json!({"agentId": caller.to_string(), "target": target.to_string()}),
        );

        assert!(result.content[0].text.contains("Permission denied"));
        assert!(store.agent(target).is_some());
    }

    #[test]
    fn set_status_clears_to_empty() {
        let mut store = AgentStore::new();
        let id = store.create("/tmp/a", CreateOptions::default());
        store.set_status_text(id, "busy".to_string());

        let result = set_status(
            &mut store,
            &json!({"agentId": id.to_string(), "status": ""}),
        );

        assert_eq!(result.is_error, None);
        let agent = store.agent(id).unwrap();
        assert_eq!(agent.status_text, "");
        assert_eq!(agent.state, skwad_agents::AgentState::Idle);
    }
}
