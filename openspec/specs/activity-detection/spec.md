# activity-detection Specification

## Purpose
Defines how an agent's status is derived from terminal output, user keystrokes,
hook events, and process exit; the input-protection guard that blocks automatic
text injection while a user is typing; the hook-driven awaiting-input state; and
the gating of the deferred MCP registration prompt. Behavior is the intended
target for the Rust port; timing values match the Swift reference unless noted.

## Requirements

### Requirement: Status states and presentation

The system SHALL represent agent status as exactly one of: Idle, Working,
Awaiting input, Error. Presentation colors SHALL be Idle green, Working orange,
Awaiting input red, Error red. Status changes SHALL record the time of the
change for dashboard sorting.

#### Scenario: Status maps to a single color

- **WHEN** an agent is Working
- **THEN** its indicator is orange and no other status is shown

### Requirement: Activity-tracking presets

The system SHALL assign each agent an activity-tracking set drawn from
`{user-input, terminal-output}`. Shell agents SHALL track neither and SHALL be
forced to Idle regardless of any status write. All other agents SHALL track
both. The tracking set MAY be downgraded at runtime (for example when hook-based
detection takes over); terminal-output callbacks remain wired but are ignored
when terminal-output is not in the set.

#### Scenario: Shell agent never leaves Idle

- **WHEN** a shell agent produces heavy terminal output
- **THEN** its status stays Idle

#### Scenario: Runtime downgrade stops terminal-output transitions

- **WHEN** an agent's tracking set is downgraded to `{user-input}`
- **THEN** subsequent terminal output does not move it to Working, but it still
  cancels a pending idle timer

### Requirement: Terminal output drives Working then Idle

When terminal-output activity is tracked and observed, the system SHALL set the
agent to Working and (re)arm a single idle timer. The default idle timeout is 3
seconds; hook-based agents use a longer terminal-output fallback timeout. When
the timer fires and no newer activity has occurred within the timeout window,
the agent becomes Idle; if newer activity exists, the timer reschedules for the
remaining interval.

#### Scenario: Goes Idle after quiet period

- **WHEN** terminal output stops and 3 seconds pass with no further activity
- **THEN** the agent becomes Idle

#### Scenario: Late activity defers Idle

- **WHEN** the idle timer fires but activity occurred 1 second ago
- **THEN** the agent stays Working and the timer reschedules for ~2 seconds

### Requirement: User input drives Working and arms the input-protection guard

When user-input activity is tracked and a keystroke occurs, the system SHALL
stamp the activity time, arm the input-protection guard for 10 seconds, and —
for non-hook agents — set the agent to Working with a 10-second idle timeout.
For hook-managed agents, a keystroke SHALL NOT by itself drive the state
machine (the awaiting-input exit in the next requirement is the only exception).

#### Scenario: Typing in a plain agent shows Working

- **WHEN** the user types in a non-hook agent's terminal
- **THEN** the agent is Working and returns to Idle 10 seconds after the last
  keystroke

#### Scenario: Typing in a hook agent does not flip status

- **WHEN** the user types in a hook-managed agent that is Idle
- **THEN** the agent stays Idle, but the input-protection guard is armed

### Requirement: Input-protection guard suppresses automatic injection

While the input-protection guard is active, the system SHALL NOT inject text
automatically into the terminal (queued MCP messages, the registration prompt,
or any other automated send). Suppressed messages remain queued and are
retried when the guard expires or at the next transition to Idle. A hook event
reporting Working or Idle SHALL cancel the guard immediately.

#### Scenario: Message held while user types

- **WHEN** an MCP message is delivered while the guard is active
- **THEN** the message is not injected and stays in the queue

#### Scenario: Guard expiry triggers a message check

- **WHEN** the guard expires with an unread message queued
- **THEN** the system re-checks messages and may now inject

### Requirement: Awaiting-input state is hook-entered and keystroke-exited

The system SHALL enter Awaiting input only when a hook reports that the agent
needs user attention (for example a permission prompt). On entering Awaiting
input, the system SHALL raise a desktop notification, including the hook-
supplied message when present. While in Awaiting input, idle timers SHALL NOT
move the agent. The state SHALL be left only by a keystroke: Return moves the
agent to Working, Escape moves it to Idle.

#### Scenario: Permission prompt raises attention

- **WHEN** a hook reports awaiting-input for an agent
- **THEN** the agent becomes Awaiting input and a desktop notification is shown

#### Scenario: Return answers the prompt

- **WHEN** the agent is Awaiting input and the user presses Return
- **THEN** the agent becomes Working

#### Scenario: Escape dismisses the prompt

- **WHEN** the agent is Awaiting input and the user presses Escape
- **THEN** the agent becomes Idle

### Requirement: Hook events are an authoritative status source

The system SHALL accept hook-reported statuses of Working, Idle, and Awaiting
input and apply them directly. Any hook status SHALL cancel the input-
protection guard. A Stop hook, when autopilot is enabled and an API key is
configured, MAY additionally trigger classification of the last assistant
message; classification failure SHALL NOT block the status update.

#### Scenario: Hook Idle overrides local Working

- **WHEN** a hook reports Idle while local detection has the agent Working
- **THEN** the agent becomes Idle and the guard is cancelled

### Requirement: Process exit sets terminal status

When the terminal process exits, the system SHALL cancel the idle timer and set
the agent to Error if the exit code is present and non-zero, otherwise Idle.

#### Scenario: Non-zero exit is an error

- **WHEN** the terminal process exits with code 1
- **THEN** the agent is Error

#### Scenario: Clean exit is Idle

- **WHEN** the terminal process exits with code 0 or no code
- **THEN** the agent is Idle

### Requirement: Deferred registration prompt gating

For agents that do not register via command-line arguments and when the MCP
server is enabled, the system SHALL schedule injection of a registration prompt.
Injection SHALL occur only once and only when both conditions hold: the
scheduled delay has elapsed AND the agent has become Idle at least once. The
first-idle delay SHALL be short for fast-starting agents and long for slow-
starting agents; subsequent idles use a short delay. Injection SHALL respect the
input-protection guard.

#### Scenario: Waits for first idle

- **WHEN** the scheduled delay elapses but the agent has never been Idle
- **THEN** the registration prompt is not injected yet

#### Scenario: Injected once after idle

- **WHEN** the agent has been Idle and the delay has elapsed
- **THEN** the registration prompt is injected exactly once, and later idles do
  not inject it again

### Requirement: Idle triggers an unread-message check

On each transition to Idle, the system SHALL check for unread MCP messages for
that agent and deliver them subject to the input-protection guard.

#### Scenario: Queued message delivered on idle

- **WHEN** an agent becomes Idle with an unread message queued and the guard
  inactive
- **THEN** the message is injected into the terminal
