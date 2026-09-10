## Context

See proposal.md - Why. The `settings-persistence` spec has four parts: one
scalar settings surface, a set of serialized collections with decode-tolerant
migration, first-launch source-folder detection, and a bounded recent-repos
MRU. The spec is explicit that the Rust port MAY use any backing store - the
persisted shapes and behaviors are the contract, not `@AppStorage`.

The Swift reference is `AppSettings` (`Skwad/Models/AppSettings.swift`): a
singleton of `@AppStorage` scalars plus `Data`-blob collections decoded with
`try? JSONDecoder().decode(...) ?? []` and per-type `init(from:)` migration
shims. `Persona` / `SavedAgent` / `BenchAgent` / `Workspace` are `Codable`
structs. Defaults personas are a static array keyed by fixed UUIDs.

This lands as a `settings` module in `skwad-core`, not a new crate: it is the
shared config surface the foundation change already earmarked `skwad-core` for
("shared types, error model, constants"), it has no async / GUI / MCP
dependency, and every later crate needs to read it.

## Goals / Non-Goals

**Goals:**

- One `Settings` value that owns every persisted field, serializes as a single
  JSON document, and writes that document on each mutation.
- `serde(default)` on every collection record field that the spec's migration
  rules cover, so a legacy record decodes without a custom deserializer.
- A decode failure on any one collection is contained: that collection loads
  empty, the rest of `Settings` still loads, `load()` returns `Ok`.
- Pure, unit-testable helpers for detection, the MRU, bench replacement, and the
  persona active-list projection - no filesystem needed except detection, which
  takes the candidate roots as an argument.

**Non-Goals:**

- Concurrent-writer safety beyond "last write wins" - `Settings` is owned by one
  caller (the app), mutations are `&mut self`.
- Atomic write (temp file + rename) - a torn write degrades to the corrupt-blob
  path, which the spec already requires to be survivable. Noted as a risk.
- A general migration framework or schema-version field - the spec's migration
  is additive-field tolerance only.
- Change observation / callbacks - a mutation writes and returns `()`.

## Decisions

### `settings` module in `skwad-core`, not a new crate

`skwad-git` and `skwad-discovery` are one-crate-per-capability because each
pulls distinct heavy deps (`git`, `notify`+`tokio`). Settings pulls only
`serde` + `serde_json` + `directories` + `uuid` - the exact deps the foundation
plan assigned to `skwad-core` for config. A new crate would split the shared
type home for no isolation gain. Rejected: `skwad-settings` crate.

### One `Settings` struct, `#[serde(default)]` at the container level

The whole document is `Settings`, `Serialize + Deserialize`, with
`#[serde(default)]` on the struct so a JSON file missing any field (a scalar
added in a later version, or a first-ever load of `{}`) fills from `Default`.
Each collection is a `Vec<T>` field. The MCP port default `8766`, terminal font
`"SF Mono"` / size `13`, and the other non-zero scalar defaults come from a
hand-written `impl Default for Settings` that reads the constants module.

Rejected: separate keyed entries like `@AppStorage`. A single document is one
read, one write, one decode path, and makes the "app still starts on corrupt
data" guarantee a single `unwrap_or_default` at the top level.

### Per-collection decode tolerance: decode into `Value` first, then per-field

`load()` reads the file to a `serde_json::Value`. Each collection is pulled with
`serde_json::from_value::<Vec<T>>(v).unwrap_or_default()` so one malformed
collection yields `Vec::new()` without failing the others (spec: "Corrupt
blob"). Scalars are pulled the same way with `unwrap_or_default` per field, or
by deserializing the scalar subset as its own `#[serde(default)]` struct.
A completely unreadable / absent file yields `Settings::default()` and `load()`
still returns `Ok` (the spec wants the app to start).

Record-level migration (a `SavedAgent` missing `created_by`) is handled by
`#[serde(default)]` on those fields - `serde_json::from_value` on the `Vec`
succeeds because the missing fields fill in. Only a structurally broken blob
(not an array, wrong types) trips the `unwrap_or_default`.

### Record types mirror the Swift `Codable` shapes exactly

`SavedAgent`, `Persona` (+ `PersonaType`, `PersonaState` enums,
`#[serde(rename_all = "lowercase")]`), `BenchAgent`, `Workspace`. Field names
match the Swift `CodingKeys` (camelCase) via `#[serde(rename_all = "camelCase")]`
so a document written by either implementation round-trips. `id` fields are
`uuid::Uuid`. Optional fields are `Option<T>` with `#[serde(default)]`.
`Workspace` carries the persisted layout fields from the Swift model
(`layoutMode`, `activeAgentIds`, `focusedPaneIndex`, `splitRatio`,
`splitRatioSecondary`, `showDashboard`, `isDetached`) as plain data; the port
does not interpret them here.

### Migration defaults live in `#[serde(default = "...")]` functions

- `SavedAgent::avatar` - `#[serde(default = "default_avatar")]` returning the
  robot emoji constant; also applied when constructing from an agent that has
  no avatar (spec: "Avatar default on save").
- `SavedAgent::agent_type` / `BenchAgent::agent_type` -
  `default = "default_agent_type"` -> `"claude"`.
- `SavedAgent::created_by` / `is_companion` / `persona_id`,
  `BenchAgent::shell_command` / `persona_id` - `#[serde(default)]` (`None` /
  `false`).
- `Persona::type` -> `default_persona_type` (`User`), `Persona::state` ->
  `default_persona_state` (`Enabled`) (spec: "Legacy persona record").

### First-launch detection takes its candidates as an argument

```
fn detect_source_base_folder(candidates: &[&Path]) -> Option<PathBuf>
```

Returns the first candidate that `is_dir()`. `Settings::init_source_folder()`
calls it with the spec's list (`~/src`, `~/source`, `~/sources`, tilde-expanded)
only when `source_folder_detected` is `false`, then sets `source_folder_detected
= true` and persists - so it never runs twice (spec: "First-launch
source-folder detection"). Taking the list as a parameter keeps the function
pure and testable against a `tempdir` without touching `$HOME`.

Note: the spec fixes the candidate list to three entries; the Swift reference
has a longer tiered list. The port follows the spec.

### Recent-repos MRU and bench replacement are pure `Vec` ops

- `add_recent_repo(&mut self, name)`: `retain(|r| r != name)`, `insert(0, name)`,
  `truncate(RECENT_REPOS_MAX)` (5). Spec: "Re-adding moves to front".
- `add_bench_agent(&mut self, entry)`: `retain(|b| b.folder != entry.folder)`,
  `push(entry)`. Spec: "Same-folder bench entry replaced".
- Both call `self.persist()` after mutating.

### Persona projection matches Swift `personas` computed property

`active_personas(&self) -> Vec<&Persona>`: filter `state != Deleted`, sort by
`name.to_lowercase()`. `install_default_personas(&mut self)`: for each of the
six defaults (fixed `Uuid`s in consts), push if its id is not already present
(by id, including deleted ones - a soft-deleted default stays deleted). Storage
keeps deleted personas so they are not re-installed.

### File location via `directories::ProjectDirs`

`ProjectDirs::from(ORG_QUALIFIER, ORG_NAME, APP_NAME).config_dir()` +
`SETTINGS_FILE` (`"settings.json"`). `Settings::store_path()` is public so tests
can point at a `tempdir` (an internal `with_path` constructor, or a
`SKWAD_CONFIG_DIR` env override honored only in `cfg(test)` / debug). The
directory is created on first `persist()`.

### Error model

Extend `skwad_core::Error` with `Serde(#[from] serde_json::Error)` (I/O already
present as `Io`). `load()` maps a missing file to `Ok(Settings::default())`,
maps a present-but-unparseable file to `Ok(Settings::default())` as well (spec:
app still starts) while logging, and only returns `Err` for an I/O error that is
not "not found". `persist()` returns `Err` on a real write failure.

## Risks / Trade-offs

- [Non-atomic write: a crash mid-`persist()` leaves a truncated file] → Next
  `load()` hits the corrupt-blob path and returns defaults; the spec already
  requires surviving that. Atomic temp-file+rename is a cheap later addition if
  data loss shows up in practice.
- [Top-level `unwrap_or_default` on an unparseable file silently discards user
  settings] → Matches the Swift `try?` behavior and the spec's explicit "app
  still starts" requirement. Mitigation: log at WARN with the path so it is
  diagnosable; do not delete the bad file.
- [camelCase `#[serde(rename_all)]` must stay exactly aligned with the Swift
  `CodingKeys` or a shared document fails to round-trip] → Field names are
  asserted in a round-trip test against a checked-in JSON fixture captured from
  the Swift encoder shape.
- [`directories` config dir differs from where the Swift app's UserDefaults
  live] → Intentional; the proposal's non-goal is migrating real Swift data.
  Fresh install starts clean.
- [`Settings` mutations each do a full file write] → The document is small
  (tens of KB at most); write-on-mutation matches `@AppStorage`'s immediate
  persistence and the spec's "Writing a value SHALL persist it immediately".

## Migration Plan

Purely additive: a new module in `skwad-core` and four new workspace
dependencies. Nothing consumes `Settings` yet. Rollback = remove the module,
its re-exports, and the `serde` / `serde_json` / `directories` / `uuid`
workspace entries if unused elsewhere.
