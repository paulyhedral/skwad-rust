## 1. Dependencies and module scaffold

- [x] 1.1 Add `serde` (features `derive`), `serde_json`, `directories`, and `uuid` (features `v4`, `serde`) to `[workspace.dependencies]` in the root `Cargo.toml`; add the four to `crates/skwad-core/Cargo.toml` `[dependencies]`; verify `cargo metadata` resolves and `cargo build -p skwad-core` succeeds.
- [x] 1.2 Add `pub mod settings;` to `crates/skwad-core/src/lib.rs` with a doc comment naming `openspec/specs/settings-persistence/spec.md` as the contract; create `crates/skwad-core/src/settings.rs` with a module doc and empty `Settings` struct; verify `cargo build -p skwad-core`.
- [x] 1.3 Extend `crates/skwad-core/src/consts.rs` with `SETTINGS_FILE = "settings.json"`, `MCP_PORT_DEFAULT: u16 = 8766`, `RECENT_REPOS_MAX: usize = 5`, `DEFAULT_AVATAR` (robot emoji), `DEFAULT_AGENT_TYPE = "claude"`, `TERMINAL_FONT_DEFAULT = "SF Mono"`, `TERMINAL_FONT_SIZE_DEFAULT: f64 = 13.0`, and `SOURCE_FOLDER_CANDIDATES: [&str; 3] = ["~/src", "~/source", "~/sources"]`; verify `cargo build -p skwad-core`.
- [x] 1.4 Extend `crates/skwad-core/src/error.rs` with `Serde(#[from] serde_json::Error)`; verify `cargo build -p skwad-core`.

## 2. Persisted record types (spec: Durable agent field set, Decode-tolerant migration, Bench templates)

- [x] 2.1 In `settings.rs` define `PersonaType { System, User }` and `PersonaState { Enabled, Disabled, Deleted }` with `#[serde(rename_all = "lowercase")]`, `Serialize + Deserialize + Clone + Copy + PartialEq + Eq + Debug`; unit test: `serde_json` round-trip of each variant matches the lowercase string.
- [x] 2.2 Define `Persona { id: Uuid, name: String, instructions: String, #[serde(default = "default_persona_type")] r#type: PersonaType, #[serde(default = "default_persona_state")] state: PersonaState }` with `#[serde(rename_all = "camelCase")]`; unit test: a JSON object with only `id`/`name`/`instructions` deserializes to `type = User`, `state = Enabled` (spec: "Legacy persona record").
- [x] 2.2b Rename the `type` field access via `#[serde(rename = "type")]` on a `persona_type` field (avoid `r#type` if it complicates call sites); keep the wire name `type`. Verify the 2.2 test still passes.
- [x] 2.3 Define `SavedAgent { id: Uuid, name: String, #[serde(default = "default_avatar")] avatar: String, folder: String, #[serde(default = "default_agent_type")] agent_type: String, #[serde(default)] created_by: Option<Uuid>, #[serde(default)] is_companion: bool, #[serde(default)] shell_command: Option<String>, #[serde(default)] persona_id: Option<Uuid> }` with `#[serde(rename_all = "camelCase")]`; unit test: a JSON object without `agentType`/`createdBy`/`isCompanion`/`personaId` deserializes with `agent_type = "claude"`, the three optionals `None`/`false`.
- [x] 2.4 Add `SavedAgent::new(id, name, avatar: Option<String>, folder, ...)` that substitutes `DEFAULT_AVATAR` when `avatar` is `None` or empty; unit test: constructing with `None` avatar yields the robot emoji (spec: "Avatar default on save").
- [x] 2.5 Define `BenchAgent { id: Uuid, name: String, avatar: String, folder: String, #[serde(default = "default_agent_type")] agent_type: String, #[serde(default)] shell_command: Option<String>, #[serde(default)] persona_id: Option<Uuid> }` with `#[serde(rename_all = "camelCase")]`; unit test: legacy-shaped JSON (no `agentType`/`personaId`) decodes with defaults.
- [x] 2.6 Define `Workspace` mirroring the Swift persisted fields (`id: Uuid`, `name`, `color_hex` as `colorHex`, `agent_ids` as `agentIds`, `layout_mode` as `layoutMode: String`, `active_agent_ids`, `focused_pane_index`, `split_ratio`, `#[serde(default)] split_ratio_secondary: Option<f64>`, `#[serde(default)] show_dashboard: Option<bool>`, `#[serde(default)] is_detached: Option<bool>`) with `#[serde(rename_all = "camelCase")]`, `Clone + Debug + PartialEq`; unit test: round-trip a full `Workspace` value.
- [x] 2.7 Add the `default_*` helper fns (`default_avatar`, `default_agent_type`, `default_persona_type`, `default_persona_state`) in `settings.rs`; covered by 2.2-2.5 tests.

## 3. Settings struct and store (spec: Single settings store, Decode-tolerant migration)

- [x] 3.1 Define `Settings` with `#[serde(default, rename_all = "camelCase")]`: the scalar fields (`appearance_mode: String`, `restore_layout_on_launch: bool`, `keep_in_menu_bar: bool`, `mcp_server_enabled: bool`, `mcp_server_port: u16`, `source_base_folder: String`, `source_folder_detected: bool`, `desktop_notifications_enabled: bool`, `markdown_font_size: i32`, `mermaid_theme: String`, `mermaid_scale: f64`, per-agent-type command/options `String`s, `terminal_font_name: String`, `terminal_font_size: f64`) and the collections (`saved_agents`, `saved_workspaces`, `personas`, `bench_agents`, `recent_repos: Vec<String>`); write `impl Default for Settings` pulling non-zero defaults from `consts`; unit test: `Settings::default().mcp_server_port == 8766` and `terminal_font_name == "SF Mono"`.
- [x] 3.2 Implement `Settings::store_path() -> PathBuf` via `directories::ProjectDirs::from(ORG_QUALIFIER, ORG_NAME, APP_NAME)` + `SETTINGS_FILE`, plus an internal `with_store_path(PathBuf)` constructor for tests; verify `cargo build -p skwad-core`.
- [x] 3.3 Implement `Settings::load() -> Result<Settings>`: missing file -> `Ok(default)`; file present but not valid JSON or not an object -> `Ok(default)` (log WARN with path); real I/O error other than not-found -> `Err`. Deserialize via `serde_json::Value` then `serde_json::from_value::<Settings>(v).unwrap_or_default()`. Unit test: point at a tempdir with no file -> defaults; with a `"{ not json"` file -> defaults, `Ok`.
- [x] 3.4 Implement `Settings::persist(&self) -> Result<()>`: create the parent dir, write pretty JSON to `store_path()`; unit test: `persist` then `load` from the same path round-trips a mutated value (e.g. `mcp_server_port = 9000`) (spec: "Scalar persists across restart").
- [x] 3.5 Implement per-collection tolerance in `load()`: when the top-level object decodes but one collection field is structurally invalid, that `Vec` loads empty and the rest survives. Unit test: a JSON doc whose `savedAgents` is `"broken"` (a string) still loads, `saved_agents` is empty, `mcp_server_port` reads the stored value (spec: "Corrupt blob").

## 4. Behaviors (spec: First-launch detection, Recent repos MRU, Bench templates, personas)

- [x] 4.1 Implement `detect_source_base_folder(candidates: &[&Path]) -> Option<PathBuf>` returning the first `is_dir()` entry; unit test against a tempdir: with only the second candidate existing, it is returned (spec: "Picks the first that exists").
- [x] 4.2 Implement `Settings::init_source_folder(&mut self)`: no-op when `source_folder_detected`; otherwise tilde-expand `SOURCE_FOLDER_CANDIDATES`, call 4.1, set `source_base_folder` on a hit, set `source_folder_detected = true`, `persist()`. Unit test: after one call `source_folder_detected` is true and a second call does not change `source_base_folder`.
- [x] 4.3 Implement `Settings::add_recent_repo(&mut self, name: &str)`: `retain` out an existing equal entry, `insert(0, ..)`, `truncate(RECENT_REPOS_MAX)`, `persist()`. Unit test: from `[A, B, C]` adding `B` yields `[B, A, C]`; adding a 6th entry drops the tail (spec: "Re-adding moves to front").
- [x] 4.4 Implement `Settings::add_bench_agent(&mut self, entry: BenchAgent)`: `retain(|b| b.folder != entry.folder)`, `push`, `persist()`. Unit test: two entries with the same folder leaves only the second (spec: "Same-folder bench entry replaced").
- [x] 4.5 Implement `Settings::active_personas(&self) -> Vec<&Persona>`: filter `state != Deleted`, sort by `name.to_lowercase()`. Unit test: a deleted persona is excluded; `"beta"` sorts before `"Alpha"`? no - assert case-insensitive order `Alpha` before `beta`.
- [x] 4.6 Add the six default system personas as `DEFAULT_PERSONAS` (fixed `Uuid`s + name + instructions, `type = System`) in `consts.rs`; implement `Settings::install_default_personas(&mut self)`: push each default whose id is absent from `personas` (checking all, including `Deleted`), `persist()` if changed. Unit test: calling twice installs six once; a persona pre-seeded as `Deleted` with a default id is not re-added.

## 5. Exports and verification

- [x] 5.1 Re-export `Settings`, `SavedAgent`, `Persona`, `PersonaType`, `PersonaState`, `BenchAgent`, `Workspace`, and `detect_source_base_folder` from `crates/skwad-core/src/lib.rs`; verify `cargo doc -p skwad-core` builds with no warnings.
- [x] 5.2 Add `crates/skwad-core/tests/settings.rs` with a checked-in JSON fixture captured in the Swift `CodingKeys` shape (camelCase, `type`/`state` on persona); assert `Settings::load` of that fixture round-trips field-for-field, guarding the `#[serde(rename_all)]` alignment.
- [x] 5.3 Run `make rust-fmt rust-lint rust-test` - all pass with the new module and deps; `cargo +nightly fmt --check` clean; `openspec validate settings-persistence-port --strict` -> valid.
- [x] 5.4 Cross-check every `settings-persistence` spec scenario against a test:

  | Requirement | Scenario | Test |
  |---|---|---|
  | Single settings store | Scalar persists across restart | 3.4 |
  | Durable agent field set | Avatar default on save | 2.4 |
  | Decode-tolerant migration | Legacy persona record | 2.2 |
  | Decode-tolerant migration | Corrupt blob | 3.3, 3.5 |
  | First-launch source-folder detection | Picks the first that exists | 4.1, 4.2 |
  | Recent repositories MRU | Re-adding moves to front | 4.3 |
  | Bench templates | Same-folder bench entry replaced | 4.4 |

## Implementation notes

- Per-agent-type command/options are stored as `agent_commands` /
  `agent_options` `BTreeMap<String, String>` rather than the Swift app's six
  named `@AppStorage` keys - one field pair instead of ten, same data.
- Task 2.2b resolved by `#[serde(rename = "type")]` on `Persona::persona_type`;
  no `r#type` needed.
- `source_folder_detected` serializes as `sourceBaseFolderInitialized` to match
  the Swift key.
- Collection decode tolerance is a `deserialize_with` helper per collection
  field (`de_tolerant_vec`): a structurally invalid collection yields an empty
  `Vec` while the rest of the document still loads.
