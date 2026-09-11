//! The only place durable/runtime `Agent` fields cross: `from_saved` builds
//! a fresh `Agent` with runtime fields at their documented defaults,
//! `to_saved` reads only the eight durable fields back out.

use std::collections::BTreeMap;

use skwad_core::SavedAgent;
use uuid::Uuid;

use crate::agent::{Agent, AgentState};

/// Build a runtime agent from its persisted record. Runtime fields always
/// start at their documented defaults, never copied from anywhere.
pub fn from_saved(saved: &SavedAgent) -> Agent {
    Agent {
        id: saved.id,
        name: saved.name.clone(),
        avatar: saved.avatar.clone(),
        folder: saved.folder.clone(),
        agent_type: saved.agent_type.clone(),
        created_by: saved.created_by,
        is_companion: saved.is_companion,
        shell_command: saved.shell_command.clone(),
        persona_id: saved.persona_id,

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

/// Extract the durable subset of a runtime agent for persistence.
pub fn to_saved(agent: &Agent) -> SavedAgent {
    SavedAgent {
        id: agent.id,
        name: agent.name.clone(),
        avatar: agent.avatar.clone(),
        folder: agent.folder.clone(),
        agent_type: agent.agent_type.clone(),
        created_by: agent.created_by,
        is_companion: agent.is_companion,
        shell_command: agent.shell_command.clone(),
        persona_id: agent.persona_id,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn saved_agent() -> SavedAgent {
        SavedAgent::new(Uuid::new_v4(), "proj", None, "/tmp/proj")
    }

    #[test]
    fn from_saved_drops_runtime_state() {
        // A record that, in the Swift reference, had accumulated runtime state
        // before being persisted (runtime fields are never serialized, so this
        // is really pinning that `from_saved` never invents non-default state).
        let saved = saved_agent();

        let agent = from_saved(&saved);

        assert_eq!(agent.state, AgentState::Idle);
        assert!(!agent.is_registered);
        assert_eq!(agent.session_id, None);
        assert_eq!(agent.terminal_title, "");
    }

    #[test]
    fn to_saved_from_saved_round_trips_durable_fields() {
        let saved = saved_agent();

        let agent = from_saved(&saved);
        let back = to_saved(&agent);

        assert_eq!(saved, back);
    }

    #[test]
    fn legacy_record_without_companion_fields_loads_with_defaults() {
        let json = format!(
            r#"{{"id":"{}","name":"A","avatar":"x","folder":"/tmp"}}"#,
            Uuid::new_v4()
        );
        let saved: SavedAgent = serde_json::from_str(&json).unwrap();

        let agent = from_saved(&saved);

        assert_eq!(agent.created_by, None);
        assert!(!agent.is_companion);
        assert_eq!(agent.agent_type, "claude");
    }
}
