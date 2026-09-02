## Purpose

Defines detecting whether a path is a git working tree and creating a new
worktree on a new branch, including how the default destination path is
derived. This backs the `create-worktree` and `create-agent` MCP tools and the
new-worktree UI.

## ADDED Requirements

### Requirement: Working-tree detection

The system SHALL treat a path as a git working tree when it contains a `.git`
entry, whether that entry is a directory (a primary clone) or a file (a linked
worktree).

#### Scenario: Linked worktree is detected

- **WHEN** a directory contains a `.git` file pointing at a worktrees gitdir
- **THEN** it is reported as a git working tree

### Requirement: Create worktree on a new branch

Creating a worktree SHALL run `git worktree add -b <branch> <destination>` from
the source repository path and SHALL propagate the command runner's error on
failure. The branch name SHALL be created as part of the operation.

#### Scenario: Branch already exists

- **WHEN** the requested branch name already exists
- **THEN** `git worktree add -b` fails and that error is returned; no worktree
  is created

### Requirement: Suggested destination path

Given a repository path and a branch name, the system SHALL suggest a sibling
directory named `<repo-name>-<sanitized-branch>`, where sanitizing replaces `/`
and space with `-`.

#### Scenario: Slash in branch name

- **WHEN** the repo is `/src/app` and the branch is `feat/login`
- **THEN** the suggested path is `/src/app-feat-login`
