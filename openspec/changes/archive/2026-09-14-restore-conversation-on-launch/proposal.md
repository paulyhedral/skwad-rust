## Why

Today, agent load persists only durable fields (settings-persistence spec:
"Loading SHALL reconstruct agents with all runtime fields at defaults") — so
quitting and relaunching Knot, or a layout restore after an OS restart,
gives back the right named panes in the right folders but each one starts a
brand-new CLI conversation, even though the prior transcript still exists on
disk. The plumbing to avoid this already exists (resume-session id flows into
`agent-launch-command`'s resume arguments; `knot-history` already resolves
the most recent session per `(folder, agent type)`) — it's just not wired
together at load time. (GitHub #60)

A folder-based "most recent session" lookup is a heuristic: it can guess
wrong when a folder has more than one agent working in it, or when the most
recent session on disk for that folder wasn't actually this agent's. The
precise fix is to remember the exact session id each agent was last attached
to and restore that directly, falling back to the heuristic lookup only when
no exact id was recorded.

## What Changes

- Add an opt-in scalar setting `restore-conversation-on-launch` (default off),
  alongside the existing `restore-layout-on-launch`.
- When enabled, persisting an agent SHALL additionally record its current
  session id (a new optional durable field on `SavedAgent`). When disabled,
  no session id is persisted, matching today's behavior.
- When enabled, on layout restore at agent load (cold app launch, not manual
  "Restart"), for each restored agent:
  1. If a persisted session id is present, set that agent's resume-session id
     to it directly — an exact restore of the session it was last attached
     to, before its terminal launches.
  2. Otherwise, fall back to looking up the most recent `SessionSummary` for
     the agent's `(folder, agent_type)` via the `knot-history` provider
     registry, and use that session's id if found.
  Either way, session id itself (not resume-session id) is left unset by this
  step; it is set by the normal resume flow once the terminal actually
  resumes, so `agent-launch-command`'s existing resume-arg logic picks it up
  automatically.
- If neither an exact persisted session id nor a history-lookup match exists,
  fall back to today's fresh-launch behavior — no error, no user-visible
  difference.
- Manual "Restart" from the UI is unaffected: it continues to always clear
  session id and resume-session id per the existing agent-lifecycle
  "Restart" requirement. Persisted session id (the new durable field) is
  likewise untouched by restart — it only ever changes on the next persist
  after a real session id is set.

## Capabilities

### New Capabilities

(none)

### Modified Capabilities

- `agent-lifecycle`: loading agents at layout restore SHALL, when
  `restore-conversation-on-launch` is enabled, populate resume-session id
  from the agent's exact persisted session id when present, else from
  history, before launch — instead of always leaving it unset.
- `settings-persistence`: the scalar settings surface gains
  `restore-conversation-on-launch`, and the durable `SavedAgent` field set
  gains an optional session id, persisted only while the setting is enabled.

## Impact

- `knot-core` / settings model: new scalar setting field, default off,
  decode-tolerant (missing field defaults to off, matching existing
  tolerant-decode conventions). `SavedAgent` gains an optional `session_id`
  field, decode-tolerant (absent defaults to `None`).
- Agent persist path: when the setting is on, include the agent's current
  session id in the `SavedAgent` record produced for saving; when off, omit
  it (`None`), so turning the setting off and back on doesn't resurrect a
  stale id from before it was disabled.
- Agent load path (wherever `restore-layout-on-launch` is currently
  consumed): prefers the persisted exact session id, falls back to a
  `knot-history` provider-registry lookup per restored agent, and sets
  resume-session id before the terminal is spawned.
- `knot-history`: no behavior change; consumed as a read-only fallback
  dependency.
- `agent-launch-command`: no behavior change; already supports resume-session
  id when present.
- UI: one new settings toggle next to "Restore layout on launch".
