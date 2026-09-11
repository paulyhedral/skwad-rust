//! Agent lookup helpers shared across tool handlers: resolve a caller- or
//! target-supplied `agentId`/name string, scope a lookup to the caller's
//! workspace, and produce the "you may have forgotten your ID" recovery
//! message every handler falls back to.

use skwad_agents::{Agent, AgentStore};
use skwad_mcp::ToolCallResult;
use uuid::Uuid;

/// Finds an agent anywhere in the store by UUID string or case-insensitive
/// name, mirroring the Swift reference's global `findAgent(byNameOrId:)`.
pub fn find_by_name_or_id<'a>(store: &'a AgentStore, identifier: &str) -> Option<&'a Agent> {
    if let Ok(id) = Uuid::parse_str(identifier)
        && let Some(agent) = store.agent(id)
    {
        return Some(agent);
    }
    store
        .agents()
        .iter()
        .find(|a| a.name.eq_ignore_ascii_case(identifier))
}

/// Every agent sharing a workspace with `agent_id`, that agent included.
/// Empty when `agent_id` isn't placed in any workspace.
pub fn workspace_members(store: &AgentStore, agent_id: Uuid) -> Vec<Agent> {
    store
        .workspaces()
        .iter()
        .find(|w| w.agent_ids.contains(&agent_id))
        .map(|w| {
            w.agent_ids
                .iter()
                .filter_map(|id| store.agent(*id).cloned())
                .collect()
        })
        .unwrap_or_default()
}

/// Finds `identifier` (UUID string or case-insensitive name) within the same
/// workspace as `caller_id`, mirroring `findAgentInSameWorkspace`.
pub fn find_in_workspace(store: &AgentStore, caller_id: Uuid, identifier: &str) -> Option<Agent> {
    let members = workspace_members(store, caller_id);
    if let Ok(id) = Uuid::parse_str(identifier)
        && let Some(agent) = members.iter().find(|a| a.id == id)
    {
        return Some(agent.clone());
    }
    members
        .into_iter()
        .find(|a| a.name.eq_ignore_ascii_case(identifier))
}

/// The shared "agent not found" recovery message: lists every known agent so
/// a caller that lost its id can re-identify itself by working directory.
pub fn agent_not_found(store: &AgentStore, provided_id: &str) -> ToolCallResult {
    let agents = store.agents();
    if agents.is_empty() {
        return ToolCallResult::error(format!(
            "Agent ID '{provided_id}' not found. No agents are currently available. Ask the user to check if Skwad is running correctly."
        ));
    }

    let mut message = format!(
        "Agent ID '{provided_id}' not found. You may have forgotten your ID due to context loss. Here are all agents - find yourself by matching your working directory:\n\n"
    );
    for agent in agents {
        message.push_str(&format!(
            "- {}: {} (ID: {})\n",
            agent.name, agent.folder, agent.id
        ));
    }
    message.push_str(
        "\nIf you're unsure which one you are, ask the user to right-click on your agent in Skwad and select 'Register'.",
    );
    ToolCallResult::error(message)
}

/// Maps the state enum to the same raw string `skwad_mcp::agent_status` uses
/// for the HTTP status endpoint, so the JSON string in a tool response
/// matches `AgentState`'s Swift raw values.
pub fn state_string(state: skwad_agents::AgentState) -> String {
    serde_json::to_value(state)
        .ok()
        .and_then(|v| v.as_str().map(str::to_string))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use skwad_agents::CreateOptions;

    use super::*;

    #[test]
    fn find_by_name_or_id_matches_uuid_then_name() {
        let mut store = AgentStore::new();
        let id = store.create("/tmp/a", CreateOptions::default());

        assert_eq!(find_by_name_or_id(&store, &id.to_string()).unwrap().id, id);
        assert_eq!(find_by_name_or_id(&store, "A").unwrap().id, id);
        assert!(find_by_name_or_id(&store, "nope").is_none());
    }

    #[test]
    fn workspace_members_includes_the_agent_itself() {
        let mut store = AgentStore::new();
        let a = store.create("/tmp/a", CreateOptions::default());
        let b = store.create(
            "/tmp/b",
            CreateOptions {
                insert_after: Some(a),
                ..Default::default()
            },
        );

        let members = workspace_members(&store, a);
        let ids: Vec<Uuid> = members.iter().map(|m| m.id).collect();
        assert!(ids.contains(&a));
        assert!(ids.contains(&b));
    }

    #[test]
    fn agent_not_found_lists_known_agents() {
        let mut store = AgentStore::new();
        store.create("/tmp/proj", CreateOptions::default());

        let result = agent_not_found(&store, "bogus");
        assert_eq!(result.is_error, Some(true));
        assert!(result.content[0].text.contains("proj"));
        assert!(result.content[0].text.contains("/tmp/proj"));
    }

    #[test]
    fn agent_not_found_handles_empty_store() {
        let store = AgentStore::new();
        let result = agent_not_found(&store, "bogus");
        assert!(
            result.content[0]
                .text
                .contains("No agents are currently available")
        );
    }
}
