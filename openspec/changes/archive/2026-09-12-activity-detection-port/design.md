## Context

`openspec/specs/activity-detection/spec.md` pins the target behavior (ported
from `Skwad/Services/TerminalSessionController.swift` + the timing constants
in `Skwad/Utilities/TimingConstants.swift`). No Rust crate implements the
state machine yet. Consumers do not exist either - the terminal host, hook
HTTP routes, and message-queue wiring are all later changes - so this is a
pure library with no binary-crate wiring.

## Goals / Non-Goals

**Goals:**
- A self-contained `ActivityTracker` state machine holding per-agent state:
  status, idle timer, input-protection guard, registration-prompt scheduling.
- Status-change events (with source) and status-change timestamps emitted to
  the caller, so a later dashboard/status layer can sort by last change.
- Cover every scenario in the spec with a unit test.

**Non-Goals:**
- The terminal adapter (`onActivity`, `onUserInput`, `onProcessExit` event
  sources). The crate exposes methods the adapter will call; the adapter
  itself belongs to the terminal-integration change.
- The hook HTTP routes (`POST /api/v1/agent/register|status`) - that is the
  `agent-hooks` port. The tracker accepts hook statuses via a method.
- Actual text injection into a terminal. The tracker decides *whether/when*
  to inject (guard, registration gating) and reports the text via a callback;
  a later consumer performs the send.
- Desktop notifications: the tracker raises a signal on Awaiting-input entry;
  the notification itself is a `skwad`/UI concern.
- Autopilot classification (see `agent-hooks` spec).

## Decisions

**New crate `skwad-activity`, one tracker per agent.** A `Tracker` owns
timers that are inherently one-agent-scoped (idle, input-protection,
registration). A shared `ActivityService`/registry across agents is not
needed yet and would be an extra layer nobody asked for; the state machine
stays a single struct the terminal host can hold per terminal.

**Timers use a tokio `Delay`/task handle, not a callback-on-thread
abstraction.** `tokio::time::Sleep` is already a workspace dependency; a
single spawned task that awaits the remaining interval and fires the callback
mirrors `ManagedTimer`'s single-in-flight-timer semantics. Each timer is
cancelled by dropping/replacing its `JoinHandle` (via `tokio::task::Abort`).

**The tracker is driven by explicit method calls, not channels.** The
terminal adapter and hook handlers are synchronous callbacks in the Swift
reference; exposure as `tracker.on_terminal_activity()`,
`tracker.on_user_input(key)`, `tracker.on_process_exit(code)`,
`tracker.apply_hook_status(...)`, `tracker.check_injections()` keeps the crate
runtime-agnostic and trivially unit-testable with `tokio::test` (pausing time
via `tokio::time::advance`). The tracker receives an `&str` agent type and
derives its own tracking preset and long-startup behavior, matching the
Swift lookup tables.

**State lives on the tracker, not on `Agent`.** The spec's presentation of
agent status (`AgentState`) already exists in `skwad-agents`; the tracker
emits `AgentState` transitions plus the timestamp through a callback. A later
integration change writes those into the `AgentStore`. Duplicating state on
`Agent` now would create two sources of truth.

**Registration gating mirrors the Swift conditions but is simplified at the
library boundary.** The Swift controller resolves `needsLongStartup` from a
global `availableAgents` list; the Rust tracker takes
`registration: Option<RegistrationConfig { first_idle_delay_short,
first_idle_delay_long, subsequent_delay, is_long_startup }>` and injects the
prompt via a callback (`FnOnce(String)`), so the prompt text comes from
`skwad-agent-launch`'s `registration_prompt(agent_id)` without the tracker
depending on registration-prompt string assembly.

**Input is keycode-agnostic for the state transitions, keycode-aware for
Exit.** The Swift code only inspects the macOS keycode (36=Return, 53=Escape)
to leave Awaiting-input. The Rust API takes a `KeyEvent` enum (`Return`,
`Escape`, `Other`) so the port doesn't leak macOS keycodes into a cross
platform crate.

## Risks / Trade-offs

- [Timing-sensitive tests could flake without timer control] -> all
  idle/guard/registration timing tests run on a paused tokio clock so
  behaviors are deterministic; no real sleeps in tests.
- [Hook and terminal sources can race on status] -> status writes are
  serialized through the tracker state; hook writes cancel the guard per the
  spec, terminal writes only when the tracking bitfield allows. A single
  `&mut self` API makes ordering explicit.
- [The spec references `AgentState::Input` by a Swift raw string that
  `skwad-agents` already serializes] -> reuse `skwad_agents::AgentState` and
  its existing serde names; presentation color mapping for the card UI stays
  a later concern.

## Migration Plan

New crate, additive workspace member. No existing code changes, no rollback
concern beyond removing the crate.