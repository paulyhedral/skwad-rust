//! Durable configuration store: scalar settings plus the serialized
//! collections (saved agents, workspaces, personas, bench templates, recent
//! repos), with decode-tolerant migration, first-launch source-folder
//! detection, and a bounded recent-repos MRU.
//!
//! Contract: `openspec/specs/settings-persistence/spec.md`.
//!
//! The whole store is one [`Settings`] value serialized as a single JSON
//! document. Every mutating helper writes the document immediately. A
//! collection blob that fails to decode yields an empty collection rather than
//! failing the load, so the app always starts.

mod records;

pub use records::{BenchAgent, Persona, PersonaState, PersonaType, SavedAgent, Workspace};

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use directories::{BaseDirs, ProjectDirs};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::consts::{
    APP_NAME, APPEARANCE_MODE_DEFAULT, DEFAULT_PERSONAS, MARKDOWN_FONT_SIZE_DEFAULT,
    MCP_PORT_DEFAULT, MERMAID_THEME_DEFAULT, ORG_NAME, ORG_QUALIFIER, RECENT_REPOS_MAX,
    SETTINGS_FILE, SOURCE_FOLDER_CANDIDATES, TERMINAL_FONT_DEFAULT, TERMINAL_FONT_SIZE_DEFAULT,
};
use crate::error::{Error, Result};

/// The whole persisted configuration surface. Load with [`Settings::load`],
/// mutate through the helpers (each persists), or set fields directly and call
/// [`Settings::persist`].
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    pub appearance_mode: String,
    pub restore_layout_on_launch: bool,
    pub keep_in_menu_bar: bool,
    pub mcp_server_enabled: bool,
    pub mcp_server_port: u16,
    pub source_base_folder: String,
    #[serde(rename = "sourceBaseFolderInitialized")]
    pub source_folder_detected: bool,
    pub desktop_notifications_enabled: bool,
    pub markdown_font_size: i32,
    pub mermaid_theme: String,
    pub mermaid_scale: f64,
    pub agent_commands: BTreeMap<String, String>,
    pub agent_options: BTreeMap<String, String>,
    pub terminal_font_name: String,
    pub terminal_font_size: f64,

    #[serde(deserialize_with = "de_tolerant_vec")]
    pub saved_agents: Vec<SavedAgent>,
    #[serde(deserialize_with = "de_tolerant_vec")]
    pub saved_workspaces: Vec<Workspace>,
    #[serde(deserialize_with = "de_tolerant_vec")]
    pub personas: Vec<Persona>,
    #[serde(deserialize_with = "de_tolerant_vec")]
    pub bench_agents: Vec<BenchAgent>,
    #[serde(deserialize_with = "de_tolerant_vec")]
    pub recent_repos: Vec<String>,

    #[serde(skip)]
    store_path: Option<PathBuf>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            appearance_mode: APPEARANCE_MODE_DEFAULT.to_string(),
            restore_layout_on_launch: true,
            keep_in_menu_bar: false,
            mcp_server_enabled: true,
            mcp_server_port: MCP_PORT_DEFAULT,
            source_base_folder: String::new(),
            source_folder_detected: false,
            desktop_notifications_enabled: true,
            markdown_font_size: MARKDOWN_FONT_SIZE_DEFAULT,
            mermaid_theme: MERMAID_THEME_DEFAULT.to_string(),
            mermaid_scale: 1.0,
            agent_commands: BTreeMap::new(),
            agent_options: BTreeMap::new(),
            terminal_font_name: TERMINAL_FONT_DEFAULT.to_string(),
            terminal_font_size: TERMINAL_FONT_SIZE_DEFAULT,
            saved_agents: Vec::new(),
            saved_workspaces: Vec::new(),
            personas: Vec::new(),
            bench_agents: Vec::new(),
            recent_repos: Vec::new(),
            store_path: None,
        }
    }
}

impl Settings {
    /// Load from the platform config directory. A missing file, an unreadable
    /// blob, or a non-object document all yield defaults with `Ok` so the app
    /// still starts. Only an I/O error other than "not found" is returned as
    /// `Err`.
    pub fn load() -> Result<Self> {
        match Self::platform_store_path() {
            Some(path) => Self::load_at(&path, Some(path.clone())),
            None => Ok(Self::default()),
        }
    }

    /// Load from an explicit path (tests, alternate profiles). Same tolerance
    /// rules as [`Settings::load`].
    pub fn load_from(path: impl AsRef<Path>) -> Result<Self> {
        let path = path.as_ref();
        Self::load_at(path, Some(path.to_path_buf()))
    }

    /// An empty settings value bound to an explicit store path.
    pub fn with_store_path(path: impl Into<PathBuf>) -> Self {
        Self {
            store_path: Some(path.into()),
            ..Self::default()
        }
    }

    fn load_at(path: &Path, store: Option<PathBuf>) -> Result<Self> {
        let bytes = match fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == io::ErrorKind::NotFound => {
                return Ok(Self::bound(store));
            }
            Err(err) => return Err(err.into()),
        };

        let value: Value = match serde_json::from_slice::<Value>(&bytes) {
            Ok(value) if value.is_object() => value,
            _ => return Ok(Self::bound(store)),
        };

        let mut settings: Self = serde_json::from_value(value).unwrap_or_default();
        settings.store_path = store;
        Ok(settings)
    }

    fn bound(store: Option<PathBuf>) -> Self {
        Self {
            store_path: store,
            ..Self::default()
        }
    }

    fn platform_store_path() -> Option<PathBuf> {
        ProjectDirs::from(ORG_QUALIFIER, ORG_NAME, APP_NAME)
            .map(|dirs| dirs.config_dir().join(SETTINGS_FILE))
    }

    /// The resolved path this value writes to.
    pub fn store_path(&self) -> Option<PathBuf> {
        self.store_path.clone().or_else(Self::platform_store_path)
    }

    /// Write the document to [`Settings::store_path`], creating the parent
    /// directory as needed.
    pub fn persist(&self) -> Result<()> {
        let path = self
            .store_path()
            .ok_or_else(|| Error::Config("no config directory available".to_string()))?;
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let json = serde_json::to_string_pretty(self)?;
        fs::write(&path, json)?;
        Ok(())
    }

    /// On first launch with no source folder set, adopt the first existing
    /// directory among the common source locations and mark detection done so
    /// it never runs again.
    pub fn init_source_folder(&mut self) -> Result<()> {
        if self.source_folder_detected {
            return Ok(());
        }
        let expanded: Vec<PathBuf> = SOURCE_FOLDER_CANDIDATES
            .iter()
            .map(|c| expand_tilde(c))
            .collect();
        let refs: Vec<&Path> = expanded.iter().map(PathBuf::as_path).collect();
        if let Some(found) = detect_source_base_folder(&refs) {
            self.source_base_folder = found.to_string_lossy().into_owned();
        }
        self.source_folder_detected = true;
        self.persist()
    }

    /// Move `name` to the front of the recent-repos list, de-duplicating, and
    /// cap the list length.
    pub fn add_recent_repo(&mut self, name: impl Into<String>) -> Result<()> {
        let name = name.into();
        self.recent_repos.retain(|entry| entry != &name);
        self.recent_repos.insert(0, name);
        self.recent_repos.truncate(RECENT_REPOS_MAX);
        self.persist()
    }

    /// Add a bench template, replacing any existing entry for the same folder.
    pub fn add_bench_agent(&mut self, entry: BenchAgent) -> Result<()> {
        self.bench_agents.retain(|b| b.folder != entry.folder);
        self.bench_agents.push(entry);
        self.persist()
    }

    /// Personas excluding soft-deleted ones, sorted case-insensitively by name.
    pub fn active_personas(&self) -> Vec<&Persona> {
        let mut personas: Vec<&Persona> = self
            .personas
            .iter()
            .filter(|p| p.state != PersonaState::Deleted)
            .collect();
        personas.sort_by_key(|p| p.name.to_lowercase());
        personas
    }

    /// Install any shipped default persona whose id is not already present
    /// (deleted entries count as present, so a removed default stays removed).
    pub fn install_default_personas(&mut self) -> Result<()> {
        let mut changed = false;
        for persona in default_personas() {
            if self.personas.iter().any(|p| p.id == persona.id) {
                continue;
            }
            self.personas.push(persona);
            changed = true;
        }
        if changed {
            self.persist()?;
        }
        Ok(())
    }

    /// Add a new user persona, enabled by default.
    pub fn add_persona(
        &mut self,
        name: impl Into<String>,
        instructions: impl Into<String>,
    ) -> Result<&Persona> {
        let persona = Persona {
            id: Uuid::new_v4(),
            name: name.into(),
            instructions: instructions.into(),
            persona_type: PersonaType::User,
            state: PersonaState::Enabled,
        };
        self.personas.push(persona);
        self.persist()?;
        Ok(self.personas.last().expect("just pushed"))
    }

    /// Rewrite name/instructions for an existing persona of any type. A no-op
    /// if `id` is not present.
    pub fn update_persona(
        &mut self,
        id: Uuid,
        name: impl Into<String>,
        instructions: impl Into<String>,
    ) -> Result<()> {
        let Some(persona) = self.personas.iter_mut().find(|p| p.id == id) else {
            return Ok(());
        };
        persona.name = name.into();
        persona.instructions = instructions.into();
        self.persist()
    }

    /// Look up a persona by id, restricted to the active (non-deleted) list.
    pub fn persona(&self, id: Uuid) -> Option<&Persona> {
        self.active_personas().into_iter().find(|p| p.id == id)
    }

    /// Remove a persona: soft delete (state becomes `deleted`, record kept)
    /// for a system persona, hard delete (record removed) for a user persona.
    /// A no-op if `id` is not present.
    pub fn remove_persona(&mut self, id: Uuid) -> Result<()> {
        let Some(index) = self.personas.iter().position(|p| p.id == id) else {
            return Ok(());
        };
        if self.personas[index].persona_type == PersonaType::System {
            self.personas[index].state = PersonaState::Deleted;
        } else {
            self.personas.remove(index);
        }
        self.persist()
    }

    /// Reset every shipped default persona already present (matched by id,
    /// including soft-deleted ones) to its shipped name, instructions, type,
    /// and state; append any shipped default that is entirely missing. User
    /// personas are untouched.
    pub fn restore_default_personas(&mut self) -> Result<()> {
        for default in default_personas() {
            match self.personas.iter_mut().find(|p| p.id == default.id) {
                Some(existing) => *existing = default,
                None => self.personas.push(default),
            }
        }
        self.persist()
    }
}

/// Return the first candidate that is an existing directory.
pub fn detect_source_base_folder(candidates: &[&Path]) -> Option<PathBuf> {
    candidates
        .iter()
        .find(|path| path.is_dir())
        .map(|path| path.to_path_buf())
}

/// The six personas shipped with the app, keyed by fixed ids.
fn default_personas() -> Vec<Persona> {
    DEFAULT_PERSONAS
        .iter()
        .map(|(id, name, instructions)| Persona {
            id: Uuid::parse_str(id).expect("default persona id is a valid uuid"),
            name: (*name).to_string(),
            instructions: (*instructions).to_string(),
            persona_type: PersonaType::System,
            state: PersonaState::Enabled,
        })
        .collect()
}

fn expand_tilde(path: &str) -> PathBuf {
    if let Some(rest) = path.strip_prefix("~/")
        && let Some(base) = BaseDirs::new()
    {
        return base.home_dir().join(rest);
    }
    PathBuf::from(path)
}

fn de_tolerant_vec<'de, D, T>(deserializer: D) -> std::result::Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: DeserializeOwned,
{
    let value = Value::deserialize(deserializer)?;
    Ok(serde_json::from_value(value).unwrap_or_default())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    fn agent_id() -> Uuid {
        Uuid::new_v4()
    }

    #[test]
    fn default_scalars() {
        let s = Settings::default();
        assert_eq!(s.mcp_server_port, 8766);
        assert_eq!(s.terminal_font_name, "SF Mono");
        assert!(s.restore_layout_on_launch);
        assert!(s.mcp_server_enabled);
    }

    #[test]
    fn missing_file_yields_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let s = Settings::load_from(&path).unwrap();
        assert_eq!(s, Settings::with_store_path(&path));
    }

    #[test]
    fn corrupt_file_yields_defaults() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(&path, "{ not json").unwrap();
        let s = Settings::load_from(&path).unwrap();
        assert_eq!(s.mcp_server_port, 8766);
        assert!(s.saved_agents.is_empty());
    }

    #[test]
    fn scalar_persists_across_reload() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut s = Settings::with_store_path(&path);
        s.mcp_server_port = 9000;
        s.persist().unwrap();
        let reloaded = Settings::load_from(&path).unwrap();
        assert_eq!(reloaded.mcp_server_port, 9000);
    }

    #[test]
    fn one_broken_collection_does_not_sink_the_rest() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        fs::write(
            &path,
            r#"{"mcpServerPort":9100,"savedAgents":"broken","recentRepos":["a","b"]}"#,
        )
        .unwrap();
        let s = Settings::load_from(&path).unwrap();
        assert_eq!(s.mcp_server_port, 9100);
        assert!(s.saved_agents.is_empty());
        assert_eq!(s.recent_repos, vec!["a", "b"]);
    }

    #[test]
    fn detect_picks_first_existing() {
        let dir = tempdir().unwrap();
        let missing = dir.path().join("src");
        let present = dir.path().join("source");
        fs::create_dir(&present).unwrap();
        let candidates: Vec<&Path> = vec![missing.as_path(), present.as_path()];
        assert_eq!(detect_source_base_folder(&candidates), Some(present));
    }

    #[test]
    fn init_source_folder_runs_once() {
        let dir = tempdir().unwrap();
        let path = dir.path().join("settings.json");
        let mut s = Settings::with_store_path(&path);
        s.init_source_folder().unwrap();
        assert!(s.source_folder_detected);
        s.source_base_folder = "/explicit".to_string();
        s.init_source_folder().unwrap();
        assert_eq!(s.source_base_folder, "/explicit");
    }

    #[test]
    fn recent_repos_moves_to_front_and_caps() {
        let dir = tempdir().unwrap();
        let mut s = Settings::with_store_path(dir.path().join("settings.json"));
        for name in ["c", "b", "a"] {
            s.add_recent_repo(name).unwrap();
        }
        assert_eq!(s.recent_repos, vec!["a", "b", "c"]);
        s.add_recent_repo("b").unwrap();
        assert_eq!(s.recent_repos, vec!["b", "a", "c"]);
        for name in ["d", "e", "f"] {
            s.add_recent_repo(name).unwrap();
        }
        assert_eq!(s.recent_repos.len(), RECENT_REPOS_MAX);
        assert_eq!(s.recent_repos[0], "f");
    }

    #[test]
    fn bench_replaces_same_folder() {
        let dir = tempdir().unwrap();
        let mut s = Settings::with_store_path(dir.path().join("settings.json"));
        s.add_bench_agent(BenchAgent::new(agent_id(), "old", None, "/repo"))
            .unwrap();
        s.add_bench_agent(BenchAgent::new(agent_id(), "new", None, "/repo"))
            .unwrap();
        assert_eq!(s.bench_agents.len(), 1);
        assert_eq!(s.bench_agents[0].name, "new");
    }

    #[test]
    fn active_personas_excludes_deleted_and_sorts_ci() {
        let dir = tempdir().unwrap();
        let mut s = Settings::with_store_path(dir.path().join("settings.json"));
        s.personas = vec![
            Persona {
                id: agent_id(),
                name: "beta".to_string(),
                instructions: String::new(),
                persona_type: PersonaType::User,
                state: PersonaState::Enabled,
            },
            Persona {
                id: agent_id(),
                name: "Alpha".to_string(),
                instructions: String::new(),
                persona_type: PersonaType::User,
                state: PersonaState::Enabled,
            },
            Persona {
                id: agent_id(),
                name: "gone".to_string(),
                instructions: String::new(),
                persona_type: PersonaType::System,
                state: PersonaState::Deleted,
            },
        ];
        let names: Vec<&str> = s
            .active_personas()
            .iter()
            .map(|p| p.name.as_str())
            .collect();
        assert_eq!(names, vec!["Alpha", "beta"]);
    }

    #[test]
    fn default_personas_install_once() {
        let dir = tempdir().unwrap();
        let mut s = Settings::with_store_path(dir.path().join("settings.json"));
        s.install_default_personas().unwrap();
        assert_eq!(s.personas.len(), 6);
        s.install_default_personas().unwrap();
        assert_eq!(s.personas.len(), 6);
    }

    #[test]
    fn add_update_and_lookup_persona() {
        let dir = tempdir().unwrap();
        let mut s = Settings::with_store_path(dir.path().join("settings.json"));
        let id = s.add_persona("Rookie", "be helpful").unwrap().id;
        assert_eq!(s.personas.len(), 1);
        assert_eq!(s.persona(id).unwrap().name, "Rookie");

        s.update_persona(id, "Veteran", "be terse").unwrap();
        let persona = s.persona(id).unwrap();
        assert_eq!(persona.name, "Veteran");
        assert_eq!(persona.instructions, "be terse");

        s.update_persona(agent_id(), "Nobody", "").unwrap();
        assert_eq!(s.personas.len(), 1);
    }

    #[test]
    fn persona_lookup_excludes_deleted() {
        let dir = tempdir().unwrap();
        let mut s = Settings::with_store_path(dir.path().join("settings.json"));
        s.personas = vec![Persona {
            id: agent_id(),
            name: "gone".to_string(),
            instructions: String::new(),
            persona_type: PersonaType::System,
            state: PersonaState::Deleted,
        }];
        assert!(s.persona(s.personas[0].id).is_none());
    }

    #[test]
    fn remove_persona_soft_deletes_system_and_hard_deletes_user() {
        let dir = tempdir().unwrap();
        let mut s = Settings::with_store_path(dir.path().join("settings.json"));
        let system_id = agent_id();
        let user_id = agent_id();
        s.personas = vec![
            Persona {
                id: system_id,
                name: "System".to_string(),
                instructions: String::new(),
                persona_type: PersonaType::System,
                state: PersonaState::Enabled,
            },
            Persona {
                id: user_id,
                name: "User".to_string(),
                instructions: String::new(),
                persona_type: PersonaType::User,
                state: PersonaState::Enabled,
            },
        ];

        s.remove_persona(system_id).unwrap();
        assert_eq!(s.personas.len(), 2);
        assert_eq!(
            s.personas.iter().find(|p| p.id == system_id).unwrap().state,
            PersonaState::Deleted
        );

        s.remove_persona(user_id).unwrap();
        assert_eq!(s.personas.len(), 1);
        assert!(s.personas.iter().all(|p| p.id != user_id));

        s.remove_persona(agent_id()).unwrap();
        assert_eq!(s.personas.len(), 1);
    }

    #[test]
    fn restore_default_personas_reverts_edits_and_adds_missing() {
        let dir = tempdir().unwrap();
        let mut s = Settings::with_store_path(dir.path().join("settings.json"));
        let (id, name, instructions) = DEFAULT_PERSONAS[0];
        let id = Uuid::parse_str(id).unwrap();
        s.personas = vec![
            Persona {
                id,
                name: "Renamed".to_string(),
                instructions: "different".to_string(),
                persona_type: PersonaType::System,
                state: PersonaState::Disabled,
            },
            Persona {
                id: agent_id(),
                name: "Mine".to_string(),
                instructions: "keep me".to_string(),
                persona_type: PersonaType::User,
                state: PersonaState::Enabled,
            },
        ];

        s.restore_default_personas().unwrap();

        assert_eq!(s.personas.len(), 7);
        let restored = s.personas.iter().find(|p| p.id == id).unwrap();
        assert_eq!(restored.name, name);
        assert_eq!(restored.instructions, instructions);
        assert_eq!(restored.state, PersonaState::Enabled);
        assert!(s.personas.iter().any(|p| p.name == "Mine"));
    }

    #[test]
    fn deleted_default_persona_not_reinstalled() {
        let dir = tempdir().unwrap();
        let mut s = Settings::with_store_path(dir.path().join("settings.json"));
        let (id, _, _) = DEFAULT_PERSONAS[0];
        s.personas = vec![Persona {
            id: Uuid::parse_str(id).unwrap(),
            name: "custom".to_string(),
            instructions: String::new(),
            persona_type: PersonaType::System,
            state: PersonaState::Deleted,
        }];
        s.install_default_personas().unwrap();
        assert_eq!(s.personas.len(), 6);
        assert_eq!(
            s.personas
                .iter()
                .filter(|p| p.state == PersonaState::Deleted)
                .count(),
            1
        );
    }
}
