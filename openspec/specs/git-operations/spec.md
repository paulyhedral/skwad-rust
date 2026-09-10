# git-operations Specification

## Purpose
Defines how Skwad invokes git and turns its output into structured data:
the command runner (working directory, timeout, error model), status parsing
from porcelain v2, unified-diff parsing into files/hunks/lines, numstat line
counts, the staging and commit operations, and branch / ahead-behind queries.
The Rust port MAY shell out to `git` or use a library; the parsed shapes and
the operation set are the contract.

## Requirements

### Requirement: Command runner

The system SHALL run git commands with an explicit working directory and a
timeout (default 30 seconds). On timeout the process SHALL be terminated and
the call SHALL fail with a timeout error naming the command. A non-zero exit
SHALL fail with an error carrying the command, the stderr (or stdout when
stderr is empty), and the exit code. Output SHALL be returned trimmed of
surrounding whitespace.

#### Scenario: Timeout terminates the process

- **WHEN** a git command runs longer than the timeout
- **THEN** the process is terminated and the result is a timeout error naming
  the command

#### Scenario: Non-zero exit surfaces stderr

- **WHEN** a git command exits non-zero with stderr text
- **THEN** the error carries that stderr and the exit code

### Requirement: Repository status

The system SHALL derive repository status from `git status --porcelain=v2
--branch`. It SHALL extract branch head, upstream, and ahead/behind counts from
the `# branch.*` lines, and a per-file status from `1`/`2`/`?`/`u` entries with
separate staged and unstaged change types drawn from
`{modified, added, deleted, renamed, copied, untracked, unmerged, ignored}`.
Renamed and copied entries SHALL retain the original path. The status SHALL
expose derived groupings: staged, modified (unstaged, tracked), untracked,
conflicted, and an is-clean flag.

#### Scenario: Mixed working tree

- **WHEN** the tree has one staged modification, one unstaged modification, and
  one untracked file
- **THEN** status reports one file in each of staged, modified, and untracked,
  and is-clean is false

#### Scenario: Rename keeps original path

- **WHEN** a file is staged as a rename
- **THEN** its entry has change type renamed and records the original path

### Requirement: Diff parsing

The system SHALL parse `git diff --no-color` (optionally `--staged` and/or a
single path) into a list of file diffs, each with its path, optional old path
for renames, a binary flag, and hunks. Each hunk SHALL carry its header, old
and new start/count parsed from `@@ -a,b +c,d @@` (counts default to 1 when
omitted), and lines classified as context, addition, deletion, header, or hunk
header, with old and new line numbers where applicable. Additions and deletions
per file SHALL be derivable by counting classified lines.

#### Scenario: Hunk header without counts

- **WHEN** a hunk header is `@@ -10 +10 @@`
- **THEN** the hunk's old and new counts are 1

#### Scenario: Binary file

- **WHEN** a file diff is binary
- **THEN** its binary flag is set and it has no hunks

### Requirement: Combined diff stats

The system SHALL compute combined insertions, deletions, and file count using
one `git diff --numstat` call plus one `git diff --staged --numstat` call, then
add untracked files by counting their lines directly (a binary or unreadable
untracked file counts as one file with no line delta).

#### Scenario: Untracked file contributes lines

- **WHEN** an untracked text file with 12 lines exists
- **THEN** combined stats add 12 insertions and 1 file for it

### Requirement: Staging and commit operations

The system SHALL provide: stage paths (`git add <paths>`), unstage paths
(`git restore --staged <paths>`), stage all (`git add -A`), unstage all
(`git reset HEAD`), discard working-tree changes for paths (`git restore
<paths>`), and commit with a message (`git commit -m <message>`). Each SHALL
propagate the command runner's error on failure; an empty path list for a
path-scoped operation SHALL be a no-op.

#### Scenario: Discard is path-scoped

- **WHEN** discard is called with an empty path list
- **THEN** no git command runs

#### Scenario: Commit failure propagates

- **WHEN** `git commit` exits non-zero
- **THEN** the operation returns that error

### Requirement: Branch and ahead/behind queries

The system SHALL report the current branch from `git branch --show-current`
(none when empty), whether unpushed commits exist from `git log @{u}..
--oneline`, and ahead/behind counts from `git rev-list --left-right --count
@{u}...HEAD`. When there is no upstream, unpushed SHALL be false and counts
SHALL be zero.

#### Scenario: No upstream

- **WHEN** the branch has no upstream
- **THEN** ahead and behind are both 0 and unpushed is false
