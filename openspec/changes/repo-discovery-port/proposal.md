## Why

`skwad-git` covers `git-operations` and `worktree-management`. `repo-discovery`
is the next slice: a self-contained capability with a complete spec, no GUI and
no MCP. It backs `list-repos` and the repo picker, so it is a prerequisite for
the agent-launch and MCP-tools work later.

## What Changes

- Add a new crate `skwad-discovery` implementing
  `openspec/specs/repo-discovery/spec.md`:
  - `scan(base: &Path) -> Vec<RepoInfo>` — pure `std::fs` scan of the base
    folder's immediate children. A child with a `.git` directory is a
    repository (branch from `.git/HEAD` `ref:` line, else folder name); a child
    with a `.git` file is resolved via its `gitdir: .../.git/worktrees/...`
    pointer and attached as a worktree of the owning repo. No `git` process.
  - `RepoInfo { name, worktrees }` and `WorktreeInfo { name, path }`, with the
    spec's naming (linked-worktree `<repo>-` prefix stripped) and ordering
    (repos sorted case-insensitively; primary clone ahead of linked worktrees).
  - `Discovery` — an async coordinator owning the current source folder, a
    `tokio::sync::watch` channel of the latest `Vec<RepoInfo>`, and a directory
    watch on the base. `set_source_folder(path)` cancels any pending rescan,
    resets to empty for a missing / non-directory path, otherwise scans once and
    starts watching. Filesystem changes trigger a rescan after a ~1s debounce,
    coalescing bursts to a single run against the latest source folder.
- Add `notify` (FS watching) and `tokio` to workspace dependencies; the new
  crate depends on both plus `thiserror`.
- Constants (debounce duration, `.git` / `HEAD` / `gitdir:` / `ref:` literals)
  in `skwad-discovery/src/consts.rs`.
- Unit tests for `scan` against temp trees (clone + linked worktree grouping,
  detached HEAD, prefix stripping, ordering); integration tests for the
  debounced coordinator (missing path yields empty and no watch; rapid
  `set_source_folder` calls coalesce to one scan against the last value).

Non-goals:

- The general `file-watching` capability (pause/resume around the app's own git
  writes, the shared recursive-watch abstraction, the full relevance filter).
  This change carries only the minimal base-folder watch its own spec requires;
  `file-watching` later generalizes it.
- Any GUI, MCP tool, or `list-repos` wiring — separate changes consume this
  crate's `watch::Receiver`.
- Reading git metadata beyond `.git/HEAD` and the worktree `.git` pointer file.

## Capabilities

### New Capabilities

None. This change implements the existing `repo-discovery` spec without changing
its requirements.

### Modified Capabilities

None. `openspec/specs/repo-discovery/spec.md` is the unchanged contract; this
change adds the implementation. `skip_specs: true`.

## Impact

- New: `crates/skwad-discovery/` (`src/lib.rs`, `src/consts.rs`, `src/error.rs`,
  `src/scan.rs`, `src/discovery.rs`, `tests/`).
- Modified: root `Cargo.toml` (workspace members + `notify`, `tokio` deps),
  `Cargo.lock`.
- Dependencies added: `notify`, `tokio` (workspace).
- `skwad-git`, `skwad-core`, `skwad`, and the Swift build are unaffected.
