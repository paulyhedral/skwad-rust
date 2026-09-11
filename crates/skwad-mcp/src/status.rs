use std::collections::BTreeMap;

use serde::Serialize;
use skwad_agents::Agent;
use uuid::Uuid;

/// One entry in the `GET /api/v1/agent/status` response.
#[derive(Debug, Serialize)]
pub struct AgentStatusEntry {
    pub agent_id: Uuid,
    pub name: String,
    pub folder: String,
    pub state: String,
    pub status: String,
    pub registered: bool,
    pub agent_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub session_id: Option<String>,
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

/// Maps live agents to their status entries.
pub fn agent_status(agents: &[Agent]) -> Vec<AgentStatusEntry> {
    agents
        .iter()
        .map(|agent| AgentStatusEntry {
            agent_id: agent.id,
            name: agent.name.clone(),
            folder: agent.folder.clone(),
            state: serde_json::to_value(agent.state)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default(),
            status: agent.status_text.clone(),
            registered: agent.is_registered,
            agent_type: agent.agent_type.clone(),
            session_id: agent.session_id.clone(),
            metadata: agent.metadata.clone(),
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use skwad_agents::AgentState;

    fn agent(name: &str) -> Agent {
        Agent {
            id: Uuid::new_v4(),
            name: name.to_string(),
            avatar: String::new(),
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

    #[test]
    fn status_reflects_live_agents() {
        let mut registered = agent("one");
        registered.is_registered = true;
        registered.session_id = Some("sess-1".to_string());
        let unregistered = agent("two");

        let entries = agent_status(&[registered, unregistered]);

        assert_eq!(entries.len(), 2);
        let registered_json = serde_json::to_value(&entries[0]).unwrap();
        assert_eq!(registered_json["session_id"], "sess-1");
        let unregistered_json = serde_json::to_value(&entries[1]).unwrap();
        assert!(unregistered_json.get("session_id").is_none());
    }
}
