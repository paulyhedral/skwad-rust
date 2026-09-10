//! The serialized record types stored inside [`super::Settings`]: personas,
//! saved agents, bench templates, and workspaces. Field names use the Swift
//! `CodingKeys` (camelCase) so a document written by either implementation
//! round-trips. `#[serde(default)]` on the later-added fields is the
//! decode-tolerant migration path.

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::consts::{DEFAULT_AGENT_TYPE, DEFAULT_AVATAR};

// ---------------------------------------------------------------------------
// Persona
// ---------------------------------------------------------------------------

/// Origin of a persona: shipped with the app or created by the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PersonaType {
    System,
    User,
}

/// Lifecycle state. `Deleted` is a soft delete so a removed system persona is
/// not reinstalled on the next launch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PersonaState {
    Enabled,
    Disabled,
    Deleted,
}

/// A named instruction block an agent can adopt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Persona {
    pub id: Uuid,
    pub name: String,
    pub instructions: String,
    #[serde(rename = "type", default = "default_persona_type")]
    pub persona_type: PersonaType,
    #[serde(default = "default_persona_state")]
    pub state: PersonaState,
}

// ---------------------------------------------------------------------------
// SavedAgent
// ---------------------------------------------------------------------------

/// A persisted agent. Runtime-only fields are not stored; loading reconstructs
/// them at their defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SavedAgent {
    pub id: Uuid,
    pub name: String,
    #[serde(default = "default_avatar")]
    pub avatar: String,
    pub folder: String,
    #[serde(default = "default_agent_type")]
    pub agent_type: String,
    #[serde(default)]
    pub created_by: Option<Uuid>,
    #[serde(default)]
    pub is_companion: bool,
    #[serde(default)]
    pub shell_command: Option<String>,
    #[serde(default)]
    pub persona_id: Option<Uuid>,
}

impl SavedAgent {
    /// Build a saved agent, substituting the default robot avatar when `avatar`
    /// is `None` or empty. Remaining fields start at their defaults; set them on
    /// the returned value as needed.
    pub fn new(
        id: Uuid,
        name: impl Into<String>,
        avatar: Option<String>,
        folder: impl Into<String>,
    ) -> Self {
        let avatar = avatar
            .filter(|a| !a.is_empty())
            .unwrap_or_else(default_avatar);
        Self {
            id,
            name: name.into(),
            avatar,
            folder: folder.into(),
            agent_type: default_agent_type(),
            created_by: None,
            is_companion: false,
            shell_command: None,
            persona_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// BenchAgent
// ---------------------------------------------------------------------------

/// A reusable agent template on the bench.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BenchAgent {
    pub id: Uuid,
    pub name: String,
    #[serde(default = "default_avatar")]
    pub avatar: String,
    pub folder: String,
    #[serde(default = "default_agent_type")]
    pub agent_type: String,
    #[serde(default)]
    pub shell_command: Option<String>,
    #[serde(default)]
    pub persona_id: Option<Uuid>,
}

impl BenchAgent {
    /// Build a bench entry with default avatar/agent-type fallback.
    pub fn new(
        id: Uuid,
        name: impl Into<String>,
        avatar: Option<String>,
        folder: impl Into<String>,
    ) -> Self {
        let avatar = avatar
            .filter(|a| !a.is_empty())
            .unwrap_or_else(default_avatar);
        Self {
            id,
            name: name.into(),
            avatar,
            folder: folder.into(),
            agent_type: default_agent_type(),
            shell_command: None,
            persona_id: None,
        }
    }
}

// ---------------------------------------------------------------------------
// Workspace
// ---------------------------------------------------------------------------

/// A saved group of agents with its own layout state. The port stores the
/// layout fields as opaque data; it does not interpret them here.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Workspace {
    pub id: Uuid,
    pub name: String,
    pub color_hex: String,
    #[serde(default)]
    pub agent_ids: Vec<Uuid>,
    #[serde(default)]
    pub layout_mode: String,
    #[serde(default)]
    pub active_agent_ids: Vec<Uuid>,
    #[serde(default)]
    pub focused_pane_index: i32,
    #[serde(default)]
    pub split_ratio: f64,
    #[serde(default)]
    pub split_ratio_secondary: Option<f64>,
    #[serde(default)]
    pub show_dashboard: Option<bool>,
    #[serde(default)]
    pub is_detached: Option<bool>,
}

// ---------------------------------------------------------------------------
// serde defaults
// ---------------------------------------------------------------------------

pub(super) fn default_avatar() -> String {
    DEFAULT_AVATAR.to_string()
}

pub(super) fn default_agent_type() -> String {
    DEFAULT_AGENT_TYPE.to_string()
}

fn default_persona_type() -> PersonaType {
    PersonaType::User
}

fn default_persona_state() -> PersonaState {
    PersonaState::Enabled
}

#[cfg(test)]
mod tests {
    use super::*;

    fn id() -> Uuid {
        Uuid::new_v4()
    }

    #[test]
    fn persona_enum_round_trips_lowercase() {
        assert_eq!(
            serde_json::to_string(&PersonaType::System).unwrap(),
            "\"system\""
        );
        assert_eq!(
            serde_json::to_string(&PersonaState::Deleted).unwrap(),
            "\"deleted\""
        );
        let parsed: PersonaState = serde_json::from_str("\"enabled\"").unwrap();
        assert_eq!(parsed, PersonaState::Enabled);
    }

    #[test]
    fn legacy_persona_defaults_type_and_state() {
        let json =
            r#"{"id":"a1000001-0000-0000-0000-000000000001","name":"X","instructions":"do X"}"#;
        let persona: Persona = serde_json::from_str(json).unwrap();
        assert_eq!(persona.persona_type, PersonaType::User);
        assert_eq!(persona.state, PersonaState::Enabled);
    }

    #[test]
    fn legacy_saved_agent_defaults_added_fields() {
        let json = format!(
            r#"{{"id":"{}","name":"A","avatar":"x","folder":"/tmp"}}"#,
            id()
        );
        let agent: SavedAgent = serde_json::from_str(&json).unwrap();
        assert_eq!(agent.agent_type, "claude");
        assert_eq!(agent.created_by, None);
        assert!(!agent.is_companion);
        assert_eq!(agent.persona_id, None);
    }

    #[test]
    fn saved_agent_new_defaults_avatar() {
        let a = SavedAgent::new(id(), "A", None, "/tmp");
        assert_eq!(a.avatar, DEFAULT_AVATAR);
        let b = SavedAgent::new(id(), "B", Some(String::new()), "/tmp");
        assert_eq!(b.avatar, DEFAULT_AVATAR);
        let c = SavedAgent::new(id(), "C", Some("🦀".to_string()), "/tmp");
        assert_eq!(c.avatar, "🦀");
    }

    #[test]
    fn legacy_bench_agent_defaults() {
        let json = format!(
            r#"{{"id":"{}","name":"A","avatar":"x","folder":"/tmp"}}"#,
            id()
        );
        let bench: BenchAgent = serde_json::from_str(&json).unwrap();
        assert_eq!(bench.agent_type, "claude");
        assert_eq!(bench.persona_id, None);
    }

    #[test]
    fn workspace_round_trips() {
        let ws = Workspace {
            id: id(),
            name: "Main".to_string(),
            color_hex: "#1B4FB2".to_string(),
            agent_ids: vec![id()],
            layout_mode: "grid".to_string(),
            active_agent_ids: vec![id()],
            focused_pane_index: 1,
            split_ratio: 0.5,
            split_ratio_secondary: Some(0.3),
            show_dashboard: Some(false),
            is_detached: Some(true),
        };
        let json = serde_json::to_string(&ws).unwrap();
        let back: Workspace = serde_json::from_str(&json).unwrap();
        assert_eq!(ws, back);
    }
}
