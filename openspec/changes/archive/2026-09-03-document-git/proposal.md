## Why

Skwad shows per-agent git status, drives stage/commit from the UI, discovers
repositories and worktrees under a source folder, creates worktrees for new
agents, and auto-refreshes on filesystem changes. The Rust port needs the git
behavior stated: which commands, how output is parsed, timeout handling, and
the discovery/watching rules.

## What Changes

- Add `git-operations` spec: the command runner (path, timeout, error model),
  porcelain-v2 status parsing, unified-diff parsing, numstat line counts,
  stage/unstage/discard/commit, branch and ahead/behind queries.
- Add `worktree-management` spec: worktree detection, creation on a new branch,
  suggested path derivation.
- Add `repo-discovery` spec: the pure filesystem scan of the source folder that
  groups clones and linked worktrees without invoking git, and its debounce.
- Add `file-watching` spec: the FSEvents watch with debounce, pause/resume, and
  the path filter that decides when a change is relevant.
- No implementation code.

## Capabilities

### New Capabilities

- `git-operations`: run git, parse status/diff/numstat, staging and commit,
  branch info.
- `worktree-management`: detect and create git worktrees.
- `repo-discovery`: filesystem scan that maps a source folder to repos and
  worktrees.
- `file-watching`: debounced FSEvents monitoring with a relevance filter.

### Modified Capabilities

None.

## Impact

- New specs under `openspec/specs/git-operations/`,
  `openspec/specs/worktree-management/`, `openspec/specs/repo-discovery/`,
  `openspec/specs/file-watching/`.
- `repo-discovery` and `worktree-management` back the `list-repos`,
  `list-worktrees`, `create-worktree`, and `create-agent` MCP tools.
- Non-goals: the diff/commit UI, syntax highlighting, git operations beyond
  status/diff/stage/commit/branch (no rebase, merge, push, stash).
