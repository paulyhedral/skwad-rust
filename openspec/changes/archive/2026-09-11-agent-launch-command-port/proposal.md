## Why

Every ported agent-lifecycle and MCP-tools crate can now create, register, and
message agents, but nothing yet builds the shell command that actually starts
an agent's process in its terminal. `skwad`'s terminal host needs a single
pure function - the Rust equivalent of Swift's `TerminalCommandBuilder` - that
assembles that command from settings, resume/fork state, MCP configuration,
persona text, and the working-directory wrapper, before terminal integration
work can proceed.

## What Changes

- Add a new crate `crates/skwad-agent-launch` implementing the command
  builder described in `openspec/specs/agent-launch-command/spec.md`:
  - Base command + user options lookup from `skwad_core::Settings`
    (`agent_commands`, `agent_options`), empty command short-circuits to an
    empty agent command.
  - Resume/fork argument assembly, per agent type (`claude`, `codex`,
    `opencode`, `gemini`, `copilot`, `shell`, plus unknown/custom types
    treated as non-resumable).
  - MCP argument injection per agent type when
    `Settings::mcp_server_enabled`, including plugin-dir resolution for the
    Claude/Codex activity hooks and inline registration arguments.
  - Persona instruction injection (shell-escaped) for system-prompt-capable
    types only.
  - The `<space>cd '<folder>' && clear && SKWAD_AGENT_ID=<id> <cmd>`
    initialization wrapper, including the no-env-var shell-agent case.
  - Shell escaping for double-quoted shell argument embedding, ported
    verbatim from `TerminalCommandBuilder.shellEscape`.
- No UI/terminal wiring in this change - this is the pure command-builder
  layer only, consumed later by the terminal host.

## Capabilities

### New Capabilities

None.

### Modified Capabilities

None. `openspec/specs/agent-launch-command/spec.md` is the unchanged
contract this change implements. `skip_specs: true`.

## Impact

- New crate `crates/skwad-agent-launch`, depending on `skwad-core`
  (`Settings`, `Persona`) and `uuid`.
- No changes to existing crates; `skwad-agents`, `skwad-mcp-tools`,
  `skwad-core` are read-only dependencies.
- Plugin bundle resolution (`plugin/<agent-type>/...`) needs a path relative
  to the running binary; exact resolution strategy (dev tree vs. bundled
  resource) is a design decision, not a spec change - the spec only requires
  that injection happens "when a plugin directory is resolved".
