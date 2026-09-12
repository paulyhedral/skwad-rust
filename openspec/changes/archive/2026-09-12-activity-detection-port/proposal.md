## Why

The port has crates that can launch, register, and message agents, but no one
derives an agent's status from what its terminal is actually doing. The MCP
tool catalog's `agent_status` endpoint currently serializes whatever state the
`AgentStore` holds, which never changes at runtime because nothing drives the
state machine. The status signal that `StatusSummaryView`-style UI and
dashboard sorting depend on needs the Rust equivalent of Swift's
`TerminalSessionController` activity logic.

## What Changes

- Add a new crate `crates/skwad-activity` implementing the activity-detection
  state machine described in `openspec/specs/activity-detection/spec.md`:
  - Exact status states and status-change-time recording for dashboard
    sorting.
  - Activity-tracking presets per agent type: shell agents track nothing
    (forced Idle), all other agents track user-input and terminal-output,
    downgradable at runtime.
  - Terminal-output-driven Working then Idle via a single rescheduling idle
    timer; user-input-driven Working with a longer timeout plus the
    input-protection guard.
  - The input-protection guard: suppresses automatic text injection while the
    user types, queues the injections for retry on guard expiry or next Idle,
    and is cancelled by any hook status.
  - The hook-entered / keystroke-exited Awaiting-input state, with desktop
    notification raising and Return/Escape transitions.
  - Hook statuses as an authoritative source that overrides local detection.
  - Process-exit handling (non-zero exit -> Error, clean -> Idle).
  - Deferred registration-prompt gating (delay elapsed + agent idle at least
    once, injected exactly once, respecting the guard) for agent types that
    do not register inline.
  - An idle transition that triggers an unread-message check.
- Pure library only - the terminal adapter, hook HTTP routes, notifications,
  and queue integration are later consumers. This change ships the state
  machine and its tests.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. `openspec/specs/activity-detection/spec.md` is the unchanged contract
this change implements. `skip_specs: true`.

## Impact

- New crate `crates/skwad-activity`, depending on `skwad-agents` (`Agent`,
  `AgentState`), `skwad-agent-launch` (registration prompt), `skwad-core`
  (`Settings` for `mcp_server_enabled`), and `tokio` (timers).
- No changes to existing crates; `skwad-mcp-tools`, `skwad-mcp`, `skwad` are
  untouched (wire-in is a later integration change).
- Timing values match `Skwad/Utilities/TimingConstants.swift`: idle 3s,
  user-input idle 10s, registration delays 1.5s/5s/0.5s.