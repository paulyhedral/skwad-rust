//! Guards the `#[serde(rename_all = "camelCase")]` alignment: a document in the
//! Swift `CodingKeys` shape must load field-for-field, and re-serializing it
//! must reproduce the same keys.

use skwad_core::{PersonaState, PersonaType, Settings};
use uuid::Uuid;

const FIXTURE: &str = include_str!("fixtures/settings_swift_shape.json");

#[test]
fn loads_swift_shaped_document() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(&path, FIXTURE).unwrap();

    let s = Settings::load_from(&path).unwrap();

    assert_eq!(s.appearance_mode, "dark");
    assert!(!s.restore_layout_on_launch);
    assert!(s.keep_in_menu_bar);
    assert!(!s.mcp_server_enabled);
    assert_eq!(s.mcp_server_port, 9123);
    assert_eq!(s.source_base_folder, "~/code");
    assert!(s.source_folder_detected);
    assert!(!s.desktop_notifications_enabled);
    assert_eq!(s.markdown_font_size, 16);
    assert_eq!(s.mermaid_theme, "forest");
    assert_eq!(s.mermaid_scale, 1.5);
    assert_eq!(
        s.agent_commands.get("custom1").map(String::as_str),
        Some("my-agent")
    );
    assert_eq!(
        s.agent_options.get("claude").map(String::as_str),
        Some("--verbose")
    );
    assert_eq!(s.terminal_font_name, "Menlo");
    assert_eq!(s.terminal_font_size, 12.5);

    assert_eq!(s.saved_agents.len(), 1);
    let agent = &s.saved_agents[0];
    assert_eq!(agent.name, "Builder");
    assert_eq!(agent.agent_type, "claude");
    assert!(agent.is_companion);
    assert_eq!(agent.shell_command.as_deref(), Some("zsh -l"));
    assert_eq!(
        agent.persona_id,
        Some(Uuid::parse_str("a1000001-0000-0000-0000-000000000001").unwrap())
    );

    assert_eq!(s.saved_workspaces.len(), 1);
    let ws = &s.saved_workspaces[0];
    assert_eq!(ws.name, "Main");
    assert_eq!(ws.layout_mode, "splitVertical");
    assert_eq!(ws.split_ratio_secondary, Some(0.4));
    assert_eq!(ws.is_detached, Some(true));

    assert_eq!(s.personas.len(), 2);
    assert_eq!(s.personas[0].persona_type, PersonaType::System);
    assert_eq!(s.personas[0].state, PersonaState::Enabled);
    // Legacy record with no type/state falls back.
    assert_eq!(s.personas[1].persona_type, PersonaType::User);
    assert_eq!(s.personas[1].state, PersonaState::Enabled);

    assert_eq!(s.bench_agents.len(), 1);
    assert_eq!(s.bench_agents[0].agent_type, "codex");

    assert_eq!(s.recent_repos, vec!["app", "lib", "cli"]);
}

#[test]
fn reserializes_with_swift_keys() {
    let dir = tempfile::tempdir().unwrap();
    let path = dir.path().join("settings.json");
    std::fs::write(&path, FIXTURE).unwrap();
    let s = Settings::load_from(&path).unwrap();

    let json = serde_json::to_value(&s).unwrap();
    let obj = json.as_object().unwrap();

    for key in [
        "appearanceMode",
        "restoreLayoutOnLaunch",
        "mcpServerPort",
        "sourceBaseFolderInitialized",
        "terminalFontName",
        "savedAgents",
        "savedWorkspaces",
        "benchAgents",
        "recentRepos",
    ] {
        assert!(obj.contains_key(key), "missing key {key}");
    }
    assert!(
        !obj.contains_key("storePath"),
        "store_path must not serialize"
    );

    let agent = obj["savedAgents"][0].as_object().unwrap();
    for key in [
        "agentType",
        "createdBy",
        "isCompanion",
        "shellCommand",
        "personaId",
    ] {
        assert!(agent.contains_key(key), "saved agent missing key {key}");
    }

    let persona = obj["personas"][0].as_object().unwrap();
    assert!(persona.contains_key("type"));
    assert!(persona.contains_key("state"));
}
