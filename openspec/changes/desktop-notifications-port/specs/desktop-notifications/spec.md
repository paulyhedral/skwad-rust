## Purpose

Delivers macOS desktop notifications (via `UNUserNotificationCenter`) when an
agent needs the user's attention while Skwad is backgrounded or hidden to the
menu bar, and routes a click on that notification back to the agent.

## ADDED Requirements

### Requirement: Authorization requested once at startup

The system SHALL request notification authorization (alert and sound) once
during application startup, regardless of the `desktop_notifications_enabled`
setting's value at that time. A denied or not-yet-answered authorization
SHALL NOT block startup or raise an error.

#### Scenario: Authorization requested on launch

- **WHEN** the application starts
- **THEN** a notification authorization request is issued exactly once

### Requirement: Awaiting-input raises a desktop notification

When an agent enters Awaiting input and `desktop_notifications_enabled` is
on, the system SHALL raise a desktop notification titled with the agent's
name and bodied with the hook-supplied message, or "Needs your attention"
when no message is present. The notification SHALL be delivered immediately
and carry the agent's id so a click can route back to it.

#### Scenario: Notification raised with hook message

- **WHEN** agent "auth-service" enters Awaiting input with hook message
  "Grant filesystem access?" and the setting is on
- **THEN** a desktop notification titled "Skwad - auth-service" with body
  "Grant filesystem access?" is raised

#### Scenario: Notification uses default body without a message

- **WHEN** an agent enters Awaiting input with no hook-supplied message and
  the setting is on
- **THEN** the notification body is "Needs your attention"

#### Scenario: Setting off suppresses the notification

- **WHEN** an agent enters Awaiting input and `desktop_notifications_enabled`
  is off
- **THEN** no desktop notification is raised

### Requirement: Repeat and visible-agent suppression

The system SHALL NOT raise a duplicate notification for an agent already in
Awaiting input (a second hook event for the same prompt), and SHALL NOT raise
a notification for an agent that is currently the selected/visible agent in
the app's active workspace.

#### Scenario: Second hook event for the same prompt is suppressed

- **WHEN** an agent already in Awaiting input receives another hook event
  reporting Awaiting input
- **THEN** no additional notification is raised

#### Scenario: Visible agent is suppressed

- **WHEN** the agent entering Awaiting input is the currently selected agent
  in the active workspace
- **THEN** no desktop notification is raised

### Requirement: Clicking a notification navigates to its agent

The system SHALL respond to a notification click by selecting the
notification's agent and bringing the application window to the front. A
click for an agent id that no longer exists SHALL be a no-op.

#### Scenario: Click selects the agent and raises the window

- **WHEN** the user clicks a delivered notification for agent `X`
- **THEN** agent `X` becomes the selected agent and the app window is raised

#### Scenario: Click for a removed agent is a no-op

- **WHEN** the user clicks a notification whose agent id no longer exists in
  the agent store
- **THEN** nothing changes and no error is raised
