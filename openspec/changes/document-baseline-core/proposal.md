## Why

`skwad-rust` is a from-scratch Rust rewrite of the Swift Skwad app. The Rust
port needs a behavior contract to build against. The Swift codebase encodes that
behavior implicitly across ~25k LOC; nothing states it as requirements.

This change extracts the headless-core capabilities — the logic tiers that port
to Rust without waiting on UI-toolkit decisions — into main specs. Specs capture
INTENDED behavior: current Swift behavior with known limitations resolved and
the dual terminal engine dropped (Ghostty only).

This first batch ships two capabilities as a fidelity sample:
`agent-lifecycle` and `activity-detection`. Remaining headless-core capabilities
follow in sibling changes once the format is agreed.

## What Changes

- Add `agent-lifecycle` spec: agent CRUD, companions, restart/resume semantics,
  which fields persist, workspace placement.
- Add `activity-detection` spec: the agent status state machine, activity-source
  arbitration (terminal / user / hook), idle timers, input-protection guard,
  awaiting-input state, registration-prompt injection gating.
- No implementation code. No changes to the Swift reference app.

## Capabilities

### New Capabilities

- `agent-lifecycle`: creating, editing, restarting, removing agents and their
  shell companions; the durable-vs-runtime field split; placement in workspaces.
- `activity-detection`: deriving an agent's status (Idle / Working / Awaiting
  input / Error) from terminal output, user keystrokes, hook events, and process
  exit; the input-protection guard; gating deferred text injection.

### Modified Capabilities

None.

## Impact

- New files under `openspec/specs/agent-lifecycle/` and
  `openspec/specs/activity-detection/` after this change is synced/archived.
- Downstream: each Rust port change references one of these specs as its
  acceptance contract.
- Non-goals: UI capabilities (dashboard, sidebar, panels), terminal emulation
  internals, MCP tool surface, git operations — all deferred to later changes.
