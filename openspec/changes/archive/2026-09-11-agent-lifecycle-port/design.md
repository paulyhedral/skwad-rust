## Context

See proposal.md - Why. The contract is `openspec/specs/agent-lifecycle/spec.md`.

The Swift reference is `Skwad/Models/Agent.swift` (the `Agent` struct, its
custom `Codable` for the durable subset, `CodingKeys`) and
`Skwad/Models/AgentManager.swift` (the `@Observable` class holding
`agents: [Agent]` and `workspaces: [Workspace]`). `AgentManager` mixes three
concerns: (1) the lifecycle operations this spec names (create, remove,
restart, resume, edit, bench deploy, ordering), (2) terminal/controller
wiring and split-pane layout selection, and (3) git-stats/markdown/mermaid
panel state. Only (1) is in scope; `openspec/specs/agent-lifecycle/spec.md`
covers exactly the scenarios ported here, and `Skwad/Models/Workspace.swift`'s
layout fields already exist as opaque data on `skwad_core::Workspace`.

Constraints from repo conventions: no async runtime inside the crate (same
rule as `skwad-git`/`skwad-history`), `Result` + `thiserror`, no panics in
library code, constants in one module, functions <= 5-6 args, `cargo
+nightly fmt`.

## Goals / Non-Goals

**Goals:**

- One crate, `skwad-agents`, exposing an `Agent` runtime type and an
  `AgentStore` that implements every requirement in the spec, backed by
  `skwad_core::Settings` for persistence.
- A durable/runtime field split that is structurally enforced: `to_saved`
  only ever reads the eight durable fields, so a new runtime field added
  later can't leak into persistence by accident.
- Fixture-free unit tests, one per spec scenario - the domain is in-memory
  structs, not files or sockets.

**Non-Goals:**

- Terminal lifecycle, MCP registration, activity hooks - those are separate
  specs/crates that will depend on `Agent`/`AgentStore`, not the reverse.
- Reproducing `AgentManager`'s split-pane/layout selection methods
  (`enterSplit`, `applyCompanionLayout`, `selectAgent`, `focusPane`, ...) -
  none are in `agent-lifecycle`'s spec; they belong with the eventual GPUI
  shell.
- A `HashMap<Uuid, Agent>` index. The Swift reference and the spec both treat
  agents as an ordered list (creation order, insert-after position); a `Vec`
  with linear lookup matches it and the agent counts involved (tens, not
  thousands) make that fine.

## Decisions

### Crate layout

`skwad-agents` as a sibling of `skwad-git`/`skwad-history`, depending only on
`skwad-core` (for `Settings`, `SavedAgent`, `Workspace`, `BenchAgent`,
`Persona`) and `thiserror`/`serde`/`uuid` via workspace. Modules: `consts`,
`error`, `agent` (the `Agent` struct + `AgentState`), `convert`
(`to_saved`/`from_saved`), `store` (`AgentStore` and its operations).
`lib.rs` re-exports `Agent`, `AgentState`, `AgentStore`, `AgentError`,
`Result`.

Alternative: extend `skwad_core::settings` in place, the way `personas-port`
extended it. Rejected - personas are a durable record with no runtime
half; `Agent` is the opposite (mostly runtime state layered on a durable
core), and folding it into `settings` would make that module own both the
storage format and the state machine. Keeping `skwad-agents` separate
mirrors how `skwad-history` stayed out of `skwad-core` despite also reading
`Settings`-adjacent data.

### `Agent` shape and the durable/runtime split

`Agent` is one struct (not a durable-part + runtime-part composition) with
every field the spec names, matching `Skwad/Models/Agent.swift`. The split is
enforced by `convert::to_saved(&Agent) -> SavedAgent`, which is the *only*
function allowed to read the eight durable fields for persistence, and
`convert::from_saved(&SavedAgent) -> Agent`, which is the *only* place a
fresh `Agent` is built with runtime fields at their documented defaults
(`AgentState::Idle`, empty strings, `false` flags, `None` ids, a fresh
`restart_token`). No `Default` impl on `Agent` beyond what `from_saved`
produces - a caller cannot accidentally construct an agent with stale runtime
state.

Alternative: two structs, `AgentCore` (durable) + `Agent { core, runtime... }`.
Rejected - adds a field-access indirection (`agent.core.name` vs
`agent.name`) throughout `store.rs` for no behavior the spec asks for, and
the single-struct-plus-conversion-boundary shape already gives the same
safety property at zero runtime cost.

### `AgentState` string representation

`AgentState` is a C-like enum with `#[serde(rename_all = ...)]` values that
exactly match Swift's raw strings: `"Idle"`, `"Working"`, `"Awaiting input"`,
`"Error"`. Not stored (runtime-only, never in `SavedAgent`), but the wire
value matters for any future status/hook payload that names it by string
(`agent-hooks`), so getting the exact strings now avoids a breaking rename
later.

### `AgentStore` operations, not free functions over `&mut Vec<Agent>`

`AgentStore { agents: Vec<Agent>, workspaces: Vec<Workspace> }` with inherent
methods (`create`, `remove`, `restart`, `resume_session`, `edit`,
`deploy_bench`, `reorder`, `move_to_workspace`) mirrors `AgentManager`'s
shape closely enough that a later change wiring this into the app is a
straight swap, while keeping workspace-placement bookkeeping (insert-after,
workspace inheritance, "first agent becomes active") next to the agent list
it mutates instead of scattered across caller code.

`remove` returns `Vec<Uuid>` (the companion ids removed, then the agent
itself, in removal order) instead of performing IO. The spec's "if
registered, unregister first" is expressed as a `bool` the caller inspects
per returned id (`AgentStore::remove` doesn't call any MCP client - there
isn't one in this crate); this keeps the crate's "no side effects beyond its
own state" property from `skwad-git`/`skwad-history` intact.

### Restart token and "torn down" state

`restart`/`resume_session`/the restart branch of `edit` all funnel through
one private `fn start(&mut self, id: Uuid)` that regenerates `restart_token`,
clears `session_id`/`resume_session_id`/`fork_session`, resets `state` to
`Idle`, sets `is_registered = false`, and clears `terminal_title` - exactly
`AgentManager.startAgent`. A single choke point means the "which fields does
restart touch" answer lives in one place, matching the spec's own single
enumeration of them.

### Companion relocation during edit

`edit` takes a small `EditRequest` struct (name, avatar, folder, agent_type,
persona_id, persona_changed, relocate_companions) rather than 7+ positional
params, per the repo's <=5-6-arg convention. `relocate_companions` only
applies to companions whose `folder` equals the *old* folder (a companion
manually pointed elsewhere is left alone), matching
`AgentManager.updateAgent`'s `guard companion.folder == oldFolder`.

## Risks / Trade-offs

- [`Agent`/`AgentStore` duplicate structure that will later live behind a
  `Mutex`/actor for the real app] -> acceptable now; the spec doesn't
  constrain concurrency and a later change picks the wrapper once the
  terminal/MCP integration shape is known, same deferral `skwad-git` made for
  its own runtime-agnostic design.
- [`Vec`-backed store is O(n) per lookup] -> fine at agent-list scale (tens);
  revisit only if a later spec's requirements make it a measured bottleneck.
- [Companion cascade removal order (children before parent) must hold even
  for grandchildren, but the spec only shows one level] -> `remove` recurses
  per companion found via `created_by`, so multi-level cascades fall out for
  free; a test pins the one-level case the spec documents.

## Migration Plan

New crate, no existing behavior touched. Land behind the OpenSpec change,
merge with a merge commit (`feat(skwad-agents): ...`). Rollback is deleting
the crate directory and its one workspace-member line. Nothing consumes it
until a later change wires the app.
