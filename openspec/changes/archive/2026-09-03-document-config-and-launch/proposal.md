## Why

Skwad keeps all its configuration and durable objects (agents, workspaces,
personas, bench templates, recent repos) in one settings store, and builds each
agent's terminal launch command from that config plus per-agent-type rules. The
Rust port needs the persisted shape and the launch-command construction stated,
independent of `@AppStorage`.

## What Changes

- Add `settings-persistence` spec: the settings surface (scalars plus JSON blobs
  for savedAgents, savedWorkspaces, personas, benchAgents, recentRepos), the
  durable `SavedAgent` field set, first-launch source-folder detection, recent-
  repos MRU, and decode-tolerant migration.
- Add `personas` spec: the persona model (system vs user, enabled/disabled/
  deleted), the shipped defaults with install-on-startup and restore-defaults,
  soft-delete for system personas and hard-delete for user personas.
- Add `agent-launch-command` spec: how the shell command that starts an agent
  is assembled — resume/fork arguments, user options, MCP config and plugin/
  notify hook injection per agent type, inline registration arguments, the
  `cd && clear && SKWAD_AGENT_ID=... <cmd>` wrapper, leading-space history
  suppression, shell agents.
- No implementation code.

## Capabilities

### New Capabilities

- `settings-persistence`: the single configuration + durable-object store and
  its migration behavior.
- `personas`: reusable system-prompt personas, their lifecycle, and shipped
  defaults.
- `agent-launch-command`: constructing the per-agent terminal launch command
  from settings and agent-type rules.

### Modified Capabilities

None.

## Impact

- New specs under `openspec/specs/settings-persistence/`,
  `openspec/specs/personas/`, `openspec/specs/agent-launch-command/`.
- `agent-launch-command` is consumed by `agent-lifecycle` (start/restart) and
  `activity-detection` (whether an agent registers inline vs by prompt).
- Non-goals: the Settings UI, the `terminalEngine` toggle (Rust port is Ghostty
  only), voice and updater settings (separate later capabilities).
