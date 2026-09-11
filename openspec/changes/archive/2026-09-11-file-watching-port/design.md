## Context

`skwad-discovery::Discovery` already implements a debounced, relevance-
filtered directory watch (`notify` + `tokio`) for the source-folder scan; see
`crates/skwad-discovery/src/discovery.rs`. It has no pause/resume - it
doesn't need it, since scanning is idempotent and doesn't write to the
watched tree.

The Swift `GitFileWatcher` (`Skwad/Git/GitFileWatcher.swift`) is a second,
independent FSEvents watch used for git-status auto-refresh. It adds
pause/resume so the app's own `git add`/`commit`/`checkout` writes (to
`.git/index`, `.git/HEAD`, `.git/refs/**`) don't self-trigger a refresh, plus
a path-relevance filter scoped to those same `.git` entries. No Rust
consumer of this exists yet (no git-status panel is ported).

## Goals / Non-Goals

**Goals:**
- A generic, reusable debounced watch primitive with pause/resume, usable by
  any future consumer (git-status refresh, single-file watch) without each
  one re-implementing FSEvents/notify plumbing and debounce timers.
- Match the two documented debounce values (~1s, ~0.3s) as caller-supplied
  configuration, not hardcoded per-purpose types.

**Non-Goals:**
- No git-status panel or artifact-file panel consumer in this change - both
  are unported GUI features. This change ships the primitive only.
- No refactor of `skwad-discovery::Discovery` onto this primitive. Its watch
  has no pause/resume need and already has its own tests; converging it is
  a separate, low-value change with regression risk for no behavior gain.

## Decisions

**New crate `skwad-watch` vs. adding to `skwad-discovery` or `skwad-git`.**
`skwad-git` is explicitly runtime-agnostic (no tokio) per its module docs, so
a `tokio`-driven watch can't live there. `skwad-discovery` is a plausible
home (it already depends on `notify` + `tokio`), but its name and module
docs commit it to "map a source folder to repos" - folding in an unrelated
generic watch primitive would blur that contract. A small standalone crate
keeps `skwad-discovery`'s scope intact and gives future consumers (a
git-status panel, an artifact-file panel) a dependency that doesn't pull in
repo-scanning code they don't need.

**API shape: single `Watch` type, relevance predicate injected per-call.**
Mirrors the Swift split (`GitFileWatcher` filters `.git/index|HEAD|refs`,
`FileWatcher` filters by filename) without two near-duplicate types: the
predicate is a `Fn(&Path) -> bool` closure the caller supplies, covering both
cases (and the discovery relevance filter, if it's ever migrated over later).

**Pause/resume: drop events while paused, require a settle delay after
resume before events are honored again.** Matches the spec's explicit
scenario (own commit does not self-trigger) and the Swift
`gitFileWatcherResume` (0.5s) constant. Implemented as a `resume_at: Instant`
gate checked alongside the paused flag, rather than tearing down and
recreating the underlying `notify` watcher - cheaper and avoids missing
events emitted in the gap between stop/recreate.

## Risks / Trade-offs

[Unconsumed crate ships with no caller] → Acceptable: this is a bottom-up
port following the spec contract; the git-status and artifact-file panels
that will consume it aren't proposed yet. Tests exercise the crate directly.

[Debounce/settle timing is fragile under `tokio::time::pause()` in tests] →
Use `tokio::time` test-util (already a dev-dependency pattern in
`skwad-discovery`) and advance virtual time explicitly rather than sleeping.

## Migration Plan

Additive only - new crate, added to the workspace `Cargo.toml` members list.
No existing crate changes.
