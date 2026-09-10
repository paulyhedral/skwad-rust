## Purpose

Defines the in-process agent-to-agent message queue: send, broadcast, check,
unread tracking, the workspace and companion routing rules that constrain who
may message whom, the idle-time delivery nudge, and retention cleanup. Messages
are runtime state; the intended Rust behavior matches the Swift reference,
including that messages do not survive an app restart.

## ADDED Requirements

### Requirement: Message model

A message SHALL carry a unique id, sender id, recipient id (both stored as
agent uuids), content, a timestamp, and a read flag defaulting to false.
Messages SHALL be held in memory only and SHALL NOT persist across app
restarts.

#### Scenario: New message is unread

- **WHEN** a message is created
- **THEN** its read flag is false and it has a fresh id and timestamp

### Requirement: Sender must be registered

Send and broadcast SHALL be rejected unless the sender resolves to a known
agent that is registered with MCP. Rejection SHALL return an explanatory
string; broadcast SHALL return a recipient count of zero.

#### Scenario: Unregistered sender

- **WHEN** an unregistered agent calls send-message
- **THEN** no message is stored and the caller receives "Sender not registered"

### Requirement: Recipients are workspace-scoped

A message SHALL be delivered only to a recipient in the same workspace as the
sender. A recipient outside the sender's workspace SHALL be treated as not
found.

#### Scenario: Cross-workspace send fails

- **WHEN** the sender targets an agent in another workspace
- **THEN** the result is "Recipient not found" and nothing is stored

### Requirement: Shell agents cannot receive messages

Send and broadcast SHALL skip shell agents. A direct send to a shell agent
SHALL return "Cannot send messages to shell agents".

#### Scenario: Direct send to shell agent

- **WHEN** an agent sends to a shell agent
- **THEN** the send is rejected with the shell-agent message

### Requirement: Companion routing rules

A companion agent SHALL only send to, and only receive from, its owner. A
non-owner sending to a companion SHALL be rejected with "Only the owner can
send messages to a companion agent"; a companion sending to anyone other than
its owner SHALL be rejected with "Companion agents can only send messages to
their owner". Broadcast SHALL apply the same filter per recipient.

#### Scenario: Owner messages its companion

- **WHEN** owner `A` sends to companion `C` where `C.created_by == A`
- **THEN** the message is stored

#### Scenario: Third party messages a companion

- **WHEN** agent `B` (not the owner) sends to companion `C`
- **THEN** the send is rejected

### Requirement: Idle-time delivery nudge

When a message is stored (whether from a direct send or a broadcast) and the
recipient is currently Idle, the system SHALL inject a short "check your inbox"
prompt into the recipient's terminal, subject to the input-protection guard.
When the recipient is not Idle, the message waits in the queue and is surfaced
on the recipient's next transition to Idle.

Divergence from the Swift reference: the Swift app idle-gates the nudge for
direct sends but nudges every broadcast recipient unconditionally. The Rust
port SHALL idle-gate both paths identically.

#### Scenario: Recipient idle at send time

- **WHEN** a message arrives for an Idle recipient with the guard inactive
- **THEN** the inbox prompt is injected into that recipient's terminal

#### Scenario: Recipient busy at send time

- **WHEN** a message arrives for a Working recipient
- **THEN** nothing is injected and the message stays unread until the recipient
  is Idle

#### Scenario: Broadcast to a busy recipient

- **WHEN** a broadcast reaches an eligible recipient that is currently Working
- **THEN** the message is stored unread and no prompt is injected until that
  recipient next becomes Idle

### Requirement: Check and mark-read

Check-messages SHALL return the caller's unread messages and, by default, mark
them read. A caller MAY request a non-destructive read that leaves the flags
unchanged. Unread queries SHALL report whether any unread message exists and
SHALL be able to return the most recent unread message id.

#### Scenario: Check clears unread

- **WHEN** an agent checks messages with default options
- **THEN** it receives its unread messages and a subsequent unread query
  reports none

### Requirement: Broadcast fan-out

Broadcast SHALL create one message per eligible recipient in the sender's
workspace (excluding the sender, unregistered agents, shell agents, and
companion-rule violations). Each stored message SHALL follow the same
idle-gated delivery nudge as a direct send (see "Idle-time delivery nudge").
The return value SHALL be the number of messages created.

#### Scenario: Broadcast to a mixed workspace

- **WHEN** a registered agent broadcasts in a workspace with two other
  registered non-shell agents and one shell agent
- **THEN** two messages are created and the count is 2

### Requirement: Retention cleanup

The system SHALL cap stored read messages, discarding the oldest read messages
beyond a fixed retention (100). Unread messages SHALL never be discarded by
cleanup.

#### Scenario: Old read messages pruned

- **WHEN** more than 100 read messages accumulate
- **THEN** the oldest read messages are removed down to 100 and no unread
  message is touched
