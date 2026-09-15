## Why

The `file-watching` spec has three requirements. The relevance filter for the
discovery source-folder watch is already implemented in `knot-discovery`
(`Discovery::set_source_folder`, `is_relevant`). The debounced-watch-with-
pause/resume primitive is not ported: the Swift `GitFileWatcher` pauses a
directory watch around the app's own git writes (stage/commit/discard) so
those writes don't self-trigger a status refresh, and no Rust equivalent
exists yet. Git-status auto-refresh (and any future single-file watch such as
the artifact panel's `FileWatcher`) needs this primitive before it can be
built.

## What Changes

- Add a `knot-watch` crate providing a generic debounced directory watch:
  `start`/`stop` (idempotent), `pause`/`resume` with a settle delay after
  resume, and a caller-supplied relevance predicate per watch instance.
- Default debounce is configurable per watch; callers pass git-status (~1s)
  or generic-file (~0.3s) durations per `TimingConstants.swift`.
- No UI wiring in this change: no `knot` git-panel consumer exists yet in
  the Rust port. This change ships the primitive and its tests only.

## Capabilities

`openspec/specs/file-watching/spec.md` already documents the intended
behavior (written directly, not derived from a prior change). This proposal
implements it; no requirement text changes, so no spec delta
(`skip_specs: true`).

### New Capabilities
(none)

### Modified Capabilities
(none)

## Impact

- New crate `crates/knot-watch` (depends on `notify`, `tokio`).
- No changes to `knot-discovery`, `knot-git`, or the `knot` binary in this
  change - nothing currently consumes `knot-watch`. Wiring it into a
  git-status panel is future work once that panel exists.
