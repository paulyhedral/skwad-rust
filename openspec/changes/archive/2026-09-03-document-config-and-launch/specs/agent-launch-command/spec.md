## Purpose

Defines how the shell command that launches an agent in its terminal is
assembled from settings and per-agent-type rules: the base command and user
options, resume/fork arguments, MCP configuration and hook plugin injection,
inline registration arguments, the working-directory wrapper, the
`SKWAD_AGENT_ID` environment variable, leading-space history suppression, and
the shell-agent path.

## ADDED Requirements

### Requirement: Base command and user options

For a non-shell agent, the system SHALL start from the configured command for
that agent type and append the configured user options for that type. An empty
configured command SHALL produce an empty agent command (nothing is launched).

#### Scenario: Missing command yields nothing

- **WHEN** the configured command for an agent type is empty
- **THEN** the built agent command is empty

### Requirement: Resume and fork arguments

When a resume-session id is present and the agent type supports resuming, the
system SHALL add resume arguments before user options. Codex SHALL use
subcommands (`resume <id>`, or `fork <id>` when forking is requested and
supported). Other types SHALL use flags (`--resume <id>`, plus `--fork-session`
when forking is requested and supported).

#### Scenario: Claude resume with fork

- **WHEN** a Claude agent has a resume-session id and fork is requested
- **THEN** the command includes `--resume <id> --fork-session`

#### Scenario: Codex fork uses a subcommand

- **WHEN** a Codex agent has a resume-session id and fork is requested
- **THEN** the command includes `fork <id>`, not `--resume`

### Requirement: MCP configuration injection

When the MCP server is enabled, the system SHALL append per-agent-type MCP
arguments, and SHALL inject the activity hook when a plugin directory is
resolved. The arguments differ by type:

- Claude: `--mcp-config` with the skwad HTTP server, `--allowed-tools
  'mcp__skwad__*'`, and `--plugin-dir <path>` when the plugin dir resolves.
- Codex: only `-c 'notify=["bash","<plugin>/scripts/notify.sh"]'` when the
  plugin dir resolves. This builder does not wire Codex's MCP server URL; that
  comes from the user options or the agent's own config.
- Gemini: only `--allowed-mcp-server-names skwad` (no URL; assumes the server
  is configured for the agent elsewhere).
- Copilot: `--additional-mcp-config` with the skwad HTTP server plus one
  `--allow-tool 'skwad(<tool>)'` flag per messaging tool.

When MCP is disabled, none of these — nor the inline registration arguments —
are added.

#### Scenario: MCP disabled omits all MCP args

- **WHEN** the MCP server is disabled
- **THEN** the built command contains no MCP config, allow-list, hook, or
  inline-registration arguments

#### Scenario: Claude gets plugin dir when resolvable

- **WHEN** the Claude plugin directory resolves
- **THEN** the command includes `--plugin-dir` with that path

#### Scenario: Codex gets only the notify hook

- **WHEN** a Codex agent is launched with MCP enabled and the plugin dir
  resolves
- **THEN** the command includes the `-c 'notify=[...]'` argument and no
  `--mcp-config`

### Requirement: Inline registration arguments

When MCP is enabled and the agent type supports inline registration
(`claude`, `codex`, `opencode`, `gemini`, `copilot`, `shell`), the system SHALL
append registration arguments carrying the agent id: for types that support a
system prompt, the skwad system instructions plus the registration user prompt;
for others, the combined registration prompt. Agents that do not support inline
registration SHALL instead be registered by the deferred prompt-injection path
(see `activity-detection`).

#### Scenario: Claude registers inline

- **WHEN** a Claude agent is launched with MCP enabled and an agent id
- **THEN** the command includes an appended system prompt and the registration
  user prompt

### Requirement: Persona injection

When a persona with non-empty instructions is supplied, the system SHALL append
its text — phrased as an instruction to impersonate that persona — to the
system prompt, shell-escaped, for system-prompt-capable agent types only
(`claude`, `codex`). For every other agent type the persona SHALL be ignored in
the launch command.

#### Scenario: Persona reaches a Claude agent

- **WHEN** a Claude agent is launched with a persona that has instructions
- **THEN** the appended system prompt includes the persona text

#### Scenario: Persona dropped for Gemini

- **WHEN** a Gemini agent is launched with a persona
- **THEN** the built command contains no persona text

### Requirement: Registration arguments on resume or fork

When the agent is resuming or forking a session, the registration user prompt
SHALL be omitted because the agent already has context. For `claude` and
`codex` the system prompt (with any persona) SHALL still be appended. For
`opencode`, `gemini`, and `copilot` no registration arguments SHALL be added at
all on resume or fork.

#### Scenario: Claude resume keeps system prompt only

- **WHEN** a Claude agent is launched with a resume-session id
- **THEN** the command appends the system prompt but not the registration user
  prompt

#### Scenario: Gemini resume adds no registration args

- **WHEN** a Gemini agent is launched with a resume-session id
- **THEN** the command contains no `--prompt-interactive` registration
  argument

### Requirement: Initialization wrapper

The final terminal command SHALL be
`<space>cd '<folder>' && clear && SKWAD_AGENT_ID=<id> <agent-command>`. The
leading space suppresses shell history (given `ignorespace` / zsh default).
When the agent command is empty (shell agent), the wrapper SHALL be
`<space>cd '<folder>' && clear` with no env prefix.

#### Scenario: Env var precedes the agent command

- **WHEN** a non-shell agent is launched
- **THEN** the command sets `SKWAD_AGENT_ID` to the agent's id immediately
  before the agent command

#### Scenario: Shell agent wrapper

- **WHEN** a shell agent with no custom command is launched
- **THEN** the command is `cd '<folder>' && clear` (leading space) and sets no
  `SKWAD_AGENT_ID`

### Requirement: Shell agent command

For a shell agent, the agent command SHALL be the agent's custom shell command
when set, otherwise empty (a plain interactive shell).

#### Scenario: Custom shell command

- **WHEN** a shell agent has a custom command `htop`
- **THEN** the built agent command is `htop`
