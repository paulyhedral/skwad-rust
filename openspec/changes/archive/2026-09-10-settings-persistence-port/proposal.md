## Why

`skwad-git` and `skwad-discovery` cover their specs, but nothing persists app
state yet. `settings-persistence` is the next slice: a self-contained
capability with a complete spec, no GUI / async / MCP. It defines the durable
shapes (`SavedAgent`, `Persona`, `BenchAgent`, workspaces, recent repos) and the
scalar settings that every later change (agent lifecycle, personas, MCP config,
agent launch) reads and writes, so it is a prerequisite for that work.

## What Changes

- Add a `settings` module to `skwad-core` implementing
  `openspec/specs/settings-persistence/spec.md`:
  - `Settings` — one struct holding the scalar values (appearance mode,
    restore-layout, keep-in-menu-bar, MCP enabled, MCP port default `8766`,
    source base folder, notifications, markdown/mermaid options, per-agent-type
    command/options strings, terminal font name/size, and the rest) plus the
    serialized collections (`saved_agents`, `saved_workspaces`, `personas`,
    `bench_agents`, `recent_repos`).
  - Persisted record types with `serde` defaults for the migration rules:
    `SavedAgent` (id, name, avatar defaulting to a robot emoji, folder,
    agent_type defaulting to `claude`, created_by, is_companion, shell_command,
    persona_id), `Persona` (id, name, instructions, type defaulting to `user`,
    state defaulting to `enabled`), `BenchAgent` (id, name, avatar, folder,
    agent_type, shell_command, persona_id), `Workspace` (persisted fields from
    the Swift model, decode-tolerant).
  - A JSON file store under the platform config dir (`directories`), one file,
    written on every mutation. A collection blob that fails to decode yields an
    empty collection; the app still loads.
  - `detect_source_base_folder()` — on first launch with no folder set, pick the
    first existing directory among `~/src`, `~/source`, `~/sources`, record it,
    and mark detection done so it never re-runs.
  - `add_recent_repo(name)` — bounded MRU: move-to-front, de-duplicate,
    cap at 5.
  - `add_bench_agent(entry)` — replace any existing entry with the same folder.
  - Persona helpers matching the spec's model: active list excludes `deleted`
    and sorts case-insensitively by name; the six default system personas
    install by fixed id, skipping ids already present (including soft-deleted).
- Add `serde` (with `derive`), `serde_json`, `directories`, and `uuid` (v4,
  serde) to `[workspace.dependencies]`; `skwad-core` depends on all four.
- Constants (config file name, MCP port default, recent-repos cap, robot-emoji
  default, source-folder candidates, default persona ids/text) in
  `skwad-core/src/consts.rs`.
- Extend `skwad-core/src/error.rs` with the store's I/O / serialization variant.
- Unit tests: scalar round-trip, avatar default on save, legacy persona
  decode, corrupt-blob-yields-empty, first-launch detection order, recent-repos
  MRU, same-folder bench replacement.

Non-goals:

- Any GUI, settings window, or live-reload wiring — later changes read
  `Settings`.
- Agent runtime state, terminal sessions, or the `Agent` -> `SavedAgent`
  mapping beyond the persisted field set (agent lifecycle change).
- Reactive change notification / observers; a mutation writes the file and
  returns.
- Migrating real UserDefaults data from the Swift app (fresh install only).

## Capabilities

### New Capabilities

None. This change implements the existing `settings-persistence` spec without
changing its requirements.

### Modified Capabilities

None. `openspec/specs/settings-persistence/spec.md` is the unchanged contract;
this change adds the implementation. `skip_specs: true`.

## Impact

- New: `crates/skwad-core/src/settings.rs` (plus record submodules if it grows
  past the file-size limit), `crates/skwad-core/tests/`.
- Modified: `crates/skwad-core/src/lib.rs` (module + re-exports),
  `crates/skwad-core/src/consts.rs`, `crates/skwad-core/src/error.rs`,
  `crates/skwad-core/Cargo.toml`, root `Cargo.toml` (workspace deps),
  `Cargo.lock`.
- Dependencies added: `serde`, `serde_json`, `directories`, `uuid` (workspace).
- `skwad-git`, `skwad-discovery`, `skwad`, and the Swift build are unaffected.
