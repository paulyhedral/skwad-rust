## Purpose

Defines filesystem monitoring used to auto-refresh git status and repository
discovery: a recursive watch on a directory, a debounce before firing, a
pause/resume control used around the app's own git writes, and a relevance
filter that suppresses irrelevant change bursts.

## ADDED Requirements

### Requirement: Debounced directory watch

The system SHALL watch a directory tree for filesystem changes and invoke a
single callback after changes settle, debouncing bursts (git status watch
about 1 second; generic file watch about 0.3 second). Starting an
already-running watch SHALL be a no-op; stopping SHALL cancel any pending
callback.

#### Scenario: Burst collapses to one callback

- **WHEN** twenty files change within the debounce window
- **THEN** the callback fires once after the window

### Requirement: Pause and resume

The system SHALL support pausing and resuming a watch so the app can suppress
self-inflicted events during its own git operations (stage, commit, discard).
Events delivered while paused SHALL NOT invoke the callback; a short settle
delay SHALL elapse after resume before events are honored again.

#### Scenario: Own commit does not self-trigger

- **WHEN** the watch is paused, a commit runs, and the watch resumes
- **THEN** the commit's filesystem changes do not fire the refresh callback

### Requirement: Relevance filter for discovery

For the source-folder watch, a change SHALL be considered relevant only when
its path is within the watched base and it touches the base directly or a
first- or second-level entry (a repository folder or its `.git`); deeper
working-tree noise SHALL be ignored.

#### Scenario: Deep edit is ignored

- **WHEN** a file changes several levels inside a repository's working tree
- **THEN** the discovery rescan is not triggered

#### Scenario: New repo folder is relevant

- **WHEN** a new folder appears directly under the source folder
- **THEN** the rescan is triggered
