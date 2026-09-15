## Why

Today, agent load persists only durable fields (settings-persistence spec:
"Loading SHALL reconstruct agents with all runtime fields at defaults") — so
quitting and relaunching Skwad, or a layout restore after an OS restart,
gives back the right named panes in the right folders but each one starts a
brand-new CLI conversation, even though the prior transcript still exists on
disk. The plumbing to avoid this already exists (resume-session id flows into
`agent-launch-command`'s resume arguments; `skwad-history` already resolves
the most recent session per `(folder, agent type)`) — it's just not wired
together at load time. (GitHub #60)

## What Changes

- Add an opt-in scalar setting `restore-conversation-on-launch` (default off),
  alongside the existing `restore-layout-on-launch`.
- When enabled, on layout restore at agent load (cold app launch, not manual
  "Restart"), look up the most recent `SessionSummary` for each restored
  agent's `(folder, agent type)` via the `skwad-history` provider registry.
  If found, set that agent's resume-session id (not session id) before its
  terminal launches, so the existing `agent-launch-command` resume-arg logic
  picks it up automatically.
- If no history provider exists for the agent type, or no session is found,
  fall back to today's fresh-launch behavior — no error, no user-visible
  difference.
- Manual "Restart" from the UI is unaffected: it continues to always clear
  session id and resume-session id per the existing agent-lifecycle
  "Restart" requirement.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `agent-lifecycle`: loading agents at layout restore SHALL, when
  `restore-conversation-on-launch` is enabled, populate resume-session id
  from history before launch instead of always leaving it unset.
- `settings-persistence`: the scalar settings surface gains
  `restore-conversation-on-launch`.

## Impact

- `skwad-core` / settings model: new scalar setting field, default off,
  decode-tolerant (missing field defaults to off, matching existing
  tolerant-decode conventions).
- Agent load path (wherever `restore-layout-on-launch` is currently
  consumed): gains a lookup into `skwad-history`'s provider registry per
  restored agent, and sets resume-session id before the terminal is spawned.
- `skwad-history`: no behavior change; consumed as a read-only dependency.
- `agent-launch-command`: no behavior change; already supports resume-session
  id when present.
- UI: one new settings toggle next to "Restore layout on launch".
