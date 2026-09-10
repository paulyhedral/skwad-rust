# repo-discovery Specification

## Purpose
Defines the background scan that maps a configured source folder to a list of
repositories, each with its worktrees, using only filesystem reads (no git
process). This backs `list-repos` and the repo picker, and refreshes on a
debounce when the source folder changes.

## Requirements

### Requirement: Filesystem-only scan

The system SHALL scan the immediate children of the expanded source folder. A
child whose `.git` is a directory SHALL be recorded as a repository; its branch
name SHALL come from `.git/HEAD` when it holds `ref: refs/heads/<name>`,
otherwise the folder name. A child whose `.git` is a file SHALL be resolved to
its owning repository by reading `gitdir: .../.git/worktrees/...` and attached
as a worktree of that repository. The scan SHALL NOT invoke git.

#### Scenario: Clone and its linked worktree group together

- **WHEN** the source folder holds `app/` (a clone) and `app-feat/` (a linked
  worktree of `app`)
- **THEN** the result has one repository `app` whose worktrees include both
  `app` and `app-feat`

#### Scenario: Detached HEAD falls back to folder name

- **WHEN** a clone's `.git/HEAD` is not a `ref:` line
- **THEN** its primary worktree name is the folder name

### Requirement: Worktree naming

A linked worktree's name SHALL be its folder name with a leading
`<repo-name>-` prefix stripped when present.

#### Scenario: Prefix stripped

- **WHEN** repo `app` has a worktree folder `app-hotfix`
- **THEN** that worktree's name is `hotfix`

### Requirement: Result ordering

Repositories SHALL be returned sorted case-insensitively by name. The primary
clone SHALL sort ahead of linked worktrees within a repository.

#### Scenario: Alphabetical repos

- **WHEN** the folder holds `Zebra/` and `apple/`
- **THEN** `apple` is listed before `Zebra`

### Requirement: Debounced refresh

A change to the configured source folder SHALL trigger a rescan after a short
debounce (about 1 second), cancelling any pending rescan. Setting the source
folder to a path that is missing or not a directory SHALL yield an empty
result and no watch.

#### Scenario: Rapid source-folder changes coalesce

- **WHEN** the source folder is changed three times within the debounce window
- **THEN** exactly one rescan runs, against the last value
