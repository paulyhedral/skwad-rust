## Context

See proposal.md - Why. The load path today (wherever `restore-layout-on-launch`
is consumed to reconstruct agents from `SavedAgent` records) resets every
runtime field to its default, including session id and resume-session id, and
launches each agent's terminal fresh. `skwad-history`'s provider registry
(`conversation-history` spec) already resolves the most recent session for a
`(folder, agent type)` pair from disk. `agent-launch-command` already honors
resume-session id when present. This change only adds a lookup step between
"reset runtime fields" and "launch terminal" in the load path.

## Goals / Non-Goals

**Goals:**
- Wire the existing history lookup into agent load, gated by a new opt-in
  setting.
- Keep the change confined to the load/layout-restore path; no changes to
  `agent-launch-command` or to manual restart/resume behavior.

**Non-Goals:**
- Resuming on manual "Restart" — explicitly out of scope per the issue's own
  resolution of its open question (restart clearing state is intentional).
- Any UI for picking *which* session to resume — always the most recent, same
  as what history providers already surface as position 0.
- Retrying or waiting on a slow/unavailable history provider — the lookup is a
  best-effort, synchronous-or-fast local read (file mtimes, a local sqlite
  file, etc., per `conversation-history`); if it fails or returns nothing, the
  agent launches fresh exactly like today.

## Decisions

- **Gate with a new setting, not folding into `restore-layout-on-launch`.**
  Resuming a conversation is a materially different (and slightly riskier —
  it changes what the CLI process does, not just where panes sit) behavior
  than restoring pane geometry. A separate opt-in lets a user restore layout
  without resuming conversations, which the issue anticipates by naming the
  new setting explicitly. Alternative considered: a sub-option nested under
  `restore-layout-on-launch`; rejected as unnecessary indirection for one
  boolean.

- **Look up resume-session id after runtime-field reset, before terminal
  spawn, per agent, at load time — not lazily on first focus.** The history
  provider read is local-disk and already used synchronously elsewhere
  (conversation-history spec's cache-or-refresh model). Doing it eagerly at
  load keeps the "does this agent try to resume" decision in one place and
  matches how `resume-session id` already flows into `agent-launch-command`
  for the explicit user-triggered resume action.

- **Only the most recent session, no cross-checking that the transcript is
  still resumable.** `agent-launch-command`'s resume args are passed straight
  to the CLI; if the CLI itself rejects a stale/corrupt session id, that
  surfaces the same way an explicit user-triggered resume failure would
  today. Adding validation here would duplicate the CLI's own handling.

## Risks / Trade-offs

- [Resuming an agent's most recent session may not be what the user wants
  after a longer gap, e.g. a session from a stale task] → Mitigation: opt-in
  setting, off by default; the existing manual resume UI remains available to
  target an older session instead.
- [History provider lookups run for every restored agent at once on cold
  launch, adding load-path latency proportional to agent count] → Mitigation:
  each lookup is a local file/sqlite read already bounded by
  `conversation-history`'s "at most 20 sessions" cap; no network I/O. If this
  proves measurably slow in practice, lookups can be parallelized per agent —
  not needed for the initial implementation.
- [A history provider's on-disk format changes underneath us and lookups
  start silently returning nothing] → Mitigation: already the existing
  fallback path (`conversation-history` spec's "no session found" case); this
  change adds no new failure mode beyond what manual resume already tolerates.
