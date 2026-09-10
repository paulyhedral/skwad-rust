## Why

`skwad-git` now covers the `git-operations` spec but not `worktree-management`,
the next lowest-dependency slice of the port (no GUI, no async, no MCP; extends
the same crate and test pattern). The spec is small and complete, so it is a
clean follow-on to the foundation change.

## What Changes

- Add a `worktree` module to `skwad-git` implementing
  `openspec/specs/worktree-management/spec.md`:
  - `is_working_tree(path)` — true when the path holds a `.git` entry, whether
    that entry is a directory (primary clone) or a file (linked worktree).
  - `Repository::create_worktree(branch, destination)` — runs
    `git worktree add -b <branch> <destination>` from the repository path and
    propagates the runner's error unchanged (branch-already-exists surfaces as
    `GitError::Command`).
  - `suggest_worktree_path(repo, branch)` — pure function returning the sibling
    directory `<repo-name>-<sanitized-branch>`, sanitizing `/` and space to `-`.
- Add the `worktree add -b` argv to `skwad-git/src/consts.rs`.
- Export the new items from `skwad-git/src/lib.rs`; extend the crate doc.
- Unit tests for `suggest_worktree_path`; integration tests against temp repos
  for detection (including a real linked worktree) and the create/branch-exists
  paths.

Non-goals:

- Listing or discovering existing worktrees (`repo-discovery` spec, later).
- Removing or pruning worktrees (not in the spec).
- Any GUI, MCP tool, or agent wiring.

## Capabilities

### New Capabilities

None. This change implements the existing `worktree-management` spec without
changing its requirements.

### Modified Capabilities

None. `openspec/specs/worktree-management/spec.md` is the unchanged contract;
this change adds the implementation. `skip_specs: true`.

## Impact

- New: `crates/skwad-git/src/worktree.rs`,
  `crates/skwad-git/tests/worktree.rs`.
- Modified: `crates/skwad-git/src/consts.rs`, `crates/skwad-git/src/lib.rs`,
  `crates/skwad-git/README.md` (if the tested git version note needs it).
- No new dependencies. `skwad-core`, `skwad`, and the Swift build are
  unaffected.
