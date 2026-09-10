## Context

See proposal.md - Why. The `repo-discovery` spec has two halves: a pure
filesystem scan and a debounced rescan driven by changes to the configured
source folder. The scan needs no runtime. The rescan needs a directory watch,
an async timer, and cancellation - none of which fit `skwad-git`, whose lib doc
promises "no async runtime". So this lands in a new crate.

The Swift reference is `RepoDiscoveryService.scanRepos` (pure scan) plus its
FSEvents stream with a 1s latency and a 1s debounce `Task`, gated by a
`shouldRefresh` relevance filter. The Rust port keeps the scan behavior exactly,
keeps the debounce, and carries only the slice of the relevance filter the
`repo-discovery` spec's own "Debounced refresh" scenario requires.

## Goals / Non-Goals

**Goals:**

- `scan` is a pure function over a `&Path`, fully testable with `tempfile`, no
  runtime and no `git`.
- The coordinator exposes results as a `tokio::sync::watch::Receiver<Vec<RepoInfo>>`
  so a future GUI / `list-repos` consumer just subscribes.
- Rescan bursts coalesce to one run against the latest source folder; a missing
  or non-directory path yields an empty result and no watch.

**Non-Goals:**

- A reusable watch abstraction, pause/resume, or the second-level `.git`
  relevance filtering - all `file-watching` spec territory.
- Detecting *which* child changed to do an incremental update; every relevant
  event triggers a full rescan (the scan is cheap - a single `read_dir` plus one
  small file read per child).
- Non-UTF-8 path handling beyond `to_string_lossy` for display names.

## Decisions

### New crate `skwad-discovery`, not a module in `skwad-git`

`skwad-git` is runtime-agnostic by contract. Discovery needs `tokio` and
`notify`. A new crate keeps that boundary clean and matches the
one-crate-per-capability layout (`skwad-git` already carries two closely related
git specs; discovery is unrelated - it never runs `git`). Rejected: putting
`scan` in `skwad-core` and the coordinator elsewhere - splits one small
capability across two crates for no gain.

### `scan(base: &Path) -> Vec<RepoInfo>` - pure, infallible

Mirrors `RepoDiscoveryService.scanRepos`. A `read_dir` failure (missing dir,
permissions) returns `Vec::new()`, matching the Swift `try?` and the spec's
"missing or not a directory SHALL yield an empty result". Per child:

- `child/.git` is a directory -> repository. Branch = first line of
  `child/.git/HEAD` when it is `ref: refs/heads/<name>`, else the folder name.
  The primary `WorktreeInfo` is inserted at index 0 of that repo's worktree
  list, so it always sorts ahead of linked worktrees (spec: "Result ordering").
- `child/.git` is a file -> read it, take the `gitdir: ` value, cut at
  `/.git/worktrees/` to get the owning repo path. Worktree name = folder name
  with a leading `<owning-repo-name>-` stripped when present (spec: "Worktree
  naming").
- `child/.git` missing or unreadable -> skipped.

Owning-repo paths from `.git` files are matched to discovered repositories by
path. A linked worktree whose owning repo is outside the scanned base folder is
dropped (no repo to attach it to) - same as the Swift dictionary lookup.

Final list: repos sorted case-insensitively by name (`spec: Result ordering`).

### Coordinator holds a `watch` channel, spawns one task per source folder

```
Discovery::new() -> (Discovery, watch::Receiver<Vec<RepoInfo>>)
Discovery::set_source_folder(&self, path: Option<PathBuf>)
```

The base folder is `canonicalize`d before it is watched, scanned, and passed to
the task: on macOS FSEvents reports symlink-resolved absolute paths, so the
relevance check's `strip_prefix(base)` only lines up when `base` is resolved the
same way.

`set_source_folder` runs on the caller's runtime. It:

1. Aborts the current watcher task (drops the `notify` watcher and the debounce
   timer with it) and its `JoinHandle`.
2. If the path is `None`, empty, missing, or not a directory: sends
   `Vec::new()` on the channel, returns. No watch.
3. Otherwise: `scan` once synchronously via `spawn_blocking`, send the result,
   then spawn the watcher task.

The watcher task creates a `notify::recommended_watcher` on the base folder,
non-recursive, forwarding events into a `tokio::sync::mpsc`. Its loop:

- on an event that passes the relevance check, (re)arm a
  `tokio::time::Sleep` of `DEBOUNCE` (~1s);
- when the sleep fires, `spawn_blocking(scan)` and send the result;
- a new relevant event before the sleep fires resets it - this is the
  coalescing (spec: "Rapid source-folder changes coalesce").

Rejected: a `notify` debouncer crate (`notify-debouncer-full`) - pulls another
dependency and its own file-cache for a single `Sleep` reset we already need to
own for the `set_source_folder` race.

### Relevance check: within base, depth 1 or 2

Port of `shouldRefresh`, trimmed to what the spec needs: an event path is
relevant when it is under the base and touches the base directly, a direct child
(repo folder added/removed), or `<child>/.git`. Deeper paths are ignored so
working-tree edits inside a repo do not cause rescans. The Swift "only if the
repo is new" refinement on `.git` events is dropped - a rescan is idempotent and
cheap, and keeping it would need the filter to read the last result. This is a
deliberate simplification, noted for `file-watching` to revisit.

### Constants

`skwad-discovery/src/consts.rs`: `DEBOUNCE: Duration` (from
`TimingConstants.repoDiscoveryDebounce` = 1s), and the parse literals
`GIT_DIR = ".git"`, `HEAD = "HEAD"`, `HEAD_REF_PREFIX = "ref: refs/heads/"`,
`GITDIR_PREFIX = "gitdir: "`, `WORKTREES_MARKER = "/.git/worktrees/"`.

### Error model

`scan` is infallible. The coordinator's only fallible step is creating the
`notify` watcher; that surfaces as `DiscoveryError::Watch(#[from] notify::Error)`
returned from `set_source_folder`. Watcher-task internal errors (an event
channel closing) end the task quietly - the last sent result stays valid.

## Risks / Trade-offs

- [Every relevant FS event triggers a full rescan, including repeated `.git`
  writes from an active repo directly under the base] → The debounce collapses
  bursts; a full scan is one `read_dir` + one tiny read per child. Acceptable at
  the expected scale (tens of repos). `file-watching` can add the "new repo
  only" filter later.
- [`notify` non-recursive watch semantics differ by platform for the "child dir
  created" event] → On macOS (FSEvents, the only target) directory creation
  under a watched path is reported. Integration tests assert the observed
  behavior on the build platform; no cross-platform guarantee is claimed.
- [A linked worktree whose owning clone is not under the base folder is
  silently dropped] → Matches the Swift reference and the spec's grouping model
  (worktrees attach to a discovered repo). Documented, not fixed here.
- [`set_source_folder` called concurrently from two tasks] → The method takes
  `&self` and mutates shared state under a `Mutex`; last call wins, consistent
  with the spec's "cancelling any pending rescan".

## Migration Plan

Purely additive: a new crate and two new workspace dependencies. Nothing
consumes `skwad-discovery` yet. Rollback = remove the crate from the workspace
members and drop the `notify` / `tokio` workspace entries if unused elsewhere.
