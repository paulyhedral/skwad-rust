## Context

See proposal.md - Why. `skwad-git` already has `Repository` (a working
directory + `Runner`), a typed `GitError`, and one consts module. The
`worktree-management` spec defines three behaviors in terms of a `.git` entry
check and one `git worktree add -b` invocation. This design fits them into the
existing crate with no new dependencies.

## Goals / Non-Goals

**Goals:**

- Keep `skwad-git` runtime-agnostic: detection is `std::fs`, creation is one
  `Runner::run` call.
- `suggest_worktree_path` is a pure function, testable without a repo.
- Detection recognizes a linked worktree, where `.git` is a file, not a dir.

**Non-Goals:**

- Parsing `git worktree list` or discovering existing worktrees.
- Validating the branch name or destination beyond what `git` itself rejects.
- Creating the destination's parent directory.

## Decisions

### `is_working_tree` checks `.git` existence, does not shell out

`path.join(".git").exists()` (via `std::path::Path::exists`, which follows the
metadata and returns true for both a file and a directory). Matches the Swift
reference (`FileManager.fileExists`) and the spec scenario exactly. Rejected:
`git rev-parse --is-inside-work-tree` — a process spawn for a filesystem stat,
and it answers a subtly different question (it is true from any subdirectory,
not just the tree root).

### `create_worktree` is a `Repository` method, not a free function

`Repository` already carries the source repo path in its `Runner`. The method
is `create_worktree(&self, branch: &str, destination: &Path) -> Result<()>`,
running `["worktree", "add", "-b", branch, destination]` and returning
`Ok(())` on success. Runner errors propagate unchanged, so a pre-existing
branch surfaces as `GitError::Command` with the git stderr - the spec's
"Branch already exists" scenario needs no special handling.

### `suggest_worktree_path` mirrors the Swift sanitizer

`suggest_worktree_path(repo: &Path, branch: &str) -> PathBuf`:
`repo.parent()` joined with `format!("{repo_name}-{sanitized}")`, where
`sanitized` replaces `/` and ` ` with `-`. `repo_name` is `repo.file_name()`.
Only the two characters the spec names are sanitized; no attempt at general
path-safety (out of scope, and would diverge from the contract).

### consts

Add `WORKTREE_ADD: &[&str] = &["worktree", "add", "-b"]` to
`skwad-git/src/consts.rs`; `create_worktree` appends `branch` and
`destination`. Matches the existing argv-array convention.

## Risks / Trade-offs

- [A dangling symlink at `<path>/.git` makes `exists()` return false, so a
  broken worktree reads as "not a working tree"] → Acceptable: a broken `.git`
  link is not a usable working tree, and the spec only requires detecting a
  valid file or directory entry.
- [`repo.parent()` is `None` for a root path like `/`] → `suggest_worktree_path`
  falls back to a relative sibling (`format!(...)` with no parent); a repo at
  the filesystem root is not a real case. Documented, not guarded.
- [`destination` with non-UTF-8 bytes] → passed straight to `Command` as an
  `OsStr`; no lossy conversion. No risk.

## Migration Plan

Purely additive within `skwad-git`. Rollback = delete `worktree.rs` /
`tests/worktree.rs` and revert the `consts.rs` / `lib.rs` hunks.
