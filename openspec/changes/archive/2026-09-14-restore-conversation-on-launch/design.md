## Context

See proposal.md - Why. The load path today (wherever `restore-layout-on-launch`
is consumed to reconstruct agents from `SavedAgent` records) resets every
runtime field to its default, including session id and resume-session id, and
launches each agent's terminal fresh. `knot-history`'s provider registry
(`conversation-history` spec) already resolves the most recent session for a
`(folder, agent type)` pair from disk. `agent-launch-command` already honors
resume-session id when present. This change adds two things to the load/save
path: (1) an optional exact session id recorded on `SavedAgent` itself, and
(2) a history-lookup fallback when no exact id was recorded — both feeding
resume-session id before terminal launch.

## Goals / Non-Goals

**Goals:**
- Wire the existing history lookup into agent load, gated by a new opt-in
  setting, as a fallback.
- Persist the exact session id an agent was last attached to, when the
  setting is enabled, so restore is precise rather than a folder-based guess.
- Keep the change confined to the load/save/layout-restore path; no changes
  to `agent-launch-command` or to manual restart/resume behavior.

**Non-Goals:**
- Resuming on manual "Restart" — explicitly out of scope per the issue's own
  resolution of its open question (restart clearing state is intentional).
- Any UI for picking *which* session to resume — always the agent's own last
  session when known, else the most recent for its folder.
- Retrying or waiting on a slow/unavailable history provider for the fallback
  path — it's a best-effort, synchronous-or-fast local read (file mtimes, a
  local sqlite file, etc., per `conversation-history`); if it fails or
  returns nothing, the agent launches fresh exactly like today.
- Persisting session id when the setting is off — no change to today's
  behavior in that case; the durable field, if never written, stays `None`.

## Decisions

- **Gate with a new setting, not folding into `restore-layout-on-launch`.**
  Resuming a conversation is a materially different (and slightly riskier —
  it changes what the CLI process does, not just where panes sit) behavior
  than restoring pane geometry. A separate opt-in lets a user restore layout
  without resuming conversations, which the issue anticipates by naming the
  new setting explicitly. Alternative considered: a sub-option nested under
  `restore-layout-on-launch`; rejected as unnecessary indirection for one
  boolean.

- **Persist the exact session id on `SavedAgent`, used before the
  folder-based history lookup.** A folder can host more than one agent, or
  the most recent on-disk session for that folder may not be the one this
  particular agent was actually attached to — the history lookup is a
  best-effort guess, not a record of fact. Recording the session id the
  agent actually had at persist time and preferring it at load time gives an
  exact restore in the common case, with the existing history lookup
  retained as a fallback for agents that never got a chance to persist one
  (first launch after enabling the setting, or an agent that never attached
  to a session). Alternative considered: rely solely on the history lookup;
  rejected because it can silently attach an agent to the wrong session with
  no signal to the user, which is worse than the current no-restore
  behavior.

- **Only persist session id while the setting is enabled; write `None` when
  it's off.** Keeps SavedAgent's shape opt-in-coupled to the feature it
  serves and prevents a stale id from a much earlier enabled period being
  silently reused after the user disables and re-enables the setting later
  (by then the CLI's own session may be long gone or reassigned).

- **Look up resume-session id (exact id, or the history fallback) after
  runtime-field reset, before terminal spawn, per agent, at load time — not
  lazily on first focus.** Matches how resume-session id already flows into
  `agent-launch-command` for the explicit user-triggered resume action, and
  keeps "does this agent try to resume, and with which id" decided in one
  place.

- **No cross-checking that a persisted or looked-up session id is still
  resumable.** `agent-launch-command`'s resume args are passed straight to
  the CLI; if the CLI itself rejects a stale/corrupt session id, that
  surfaces the same way an explicit user-triggered resume failure would
  today. Adding validation here would duplicate the CLI's own handling.

## Risks / Trade-offs

- [A persisted session id can go stale — the CLI's own transcript store may
  prune or reject it later] → Mitigation: same fallback as today's manual
  resume: the CLI's own rejection surfaces as a launch error, not a crash;
  no new failure mode.
- [Persisting session id means Knot's settings store now retains a link to
  CLI conversation content location, where it previously retained none] →
  Mitigation: opt-in, off by default; only the session id string is stored,
  not transcript content; scoped to when the user has explicitly asked for
  conversation restoration.
- [History provider lookups (the fallback path) run for every restored agent
  without a persisted id, adding load-path latency proportional to agent
  count] → Mitigation: each lookup is a local file/sqlite read already
  bounded by `conversation-history`'s "at most 20 sessions" cap; no network
  I/O. Agents with an exact persisted id skip the lookup entirely. If this
  proves measurably slow in practice, lookups can be parallelized per agent —
  not needed for the initial implementation.
