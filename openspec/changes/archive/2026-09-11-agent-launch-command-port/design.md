## Context

`openspec/specs/agent-launch-command/spec.md` already pins the target
behavior (ported from `Skwad/Services/TerminalCommandBuilder.swift`). No
crate builds it yet. Consumers of this crate (the terminal host in `skwad`)
don't exist yet either, so this change has no wiring step - it's a pure
library the binary crate will call into once terminal integration lands.

Inputs the builder needs already exist:
- `skwad_core::Settings` - `agent_commands` / `agent_options` (`BTreeMap
  <String, String>`), `mcp_server_enabled`, `mcp_server_port`.
- `skwad_core::Persona` - `instructions` field.
- The activity-hook plugin bundle lives at repo-root `plugin/<agent-type>/`
  (currently `plugin/claude/`, `plugin/codex/`) - same layout
  `TerminalCommandBuilder.resolvePluginPath` assumes for its dev-tree
  fallback.

## Goals / Non-Goals

**Goals:**
- One pure function, `build_agent_command`, plus the initialization-wrapper
  function, both string-in/string-out with no I/O beyond the plugin
  directory existence check.
- Byte-for-byte argument text compatible with the Swift reference (flag
  names, quoting, JSON literal spelling) since agent CLIs parse this text
  literally.
- Cover every scenario in the spec with a unit test.

**Non-Goals:**
- Actually spawning the process or wiring a terminal - that's a later
  terminal-integration change.
- Resolving an app-bundled plugin path (macOS `.app` resource lookup) - only
  the dev-tree-relative lookup the spec's scenarios exercise is in scope;
  bundled-resource resolution is a `skwad` binary-crate concern when
  packaging is designed.
- A generic "agent type" enum shared across crates - `agent_type` stays a
  `&str` here, matching `skwad-agents`' and `skwad-core`'s existing
  string-typed representation (`SavedAgent::agent_type`, `Settings`'
  `BTreeMap<String, String>` keys).

## Decisions

**New crate `skwad-agent-launch`, not a module in `skwad-core` or
`skwad-agents`.** The builder depends on `Settings` and `Persona` (from
`skwad-core`) but is a distinct concern - command-line text assembly, not
configuration storage or agent lifecycle. Keeping it separate avoids
`skwad-core` growing an agent-CLI-specific dependency surface, and matches
the existing pattern of one crate per bounded capability
(`skwad-messaging`, `skwad-discovery`, `skwad-mcp-tools`).

**Group the builder's inputs into one `LaunchRequest` struct.** The Swift
function already takes seven parameters (`agentType`, `settings`, `agentId`,
`shellCommand`, `resumeSessionId`, `forkSession`, `persona`). Per the
project's <=5-6-arg convention, `build_agent_command(settings: &Settings,
req: &LaunchRequest) -> String` groups the per-launch fields into one struct
instead of exceeding the arg limit or introducing a builder pattern nobody
asked for.

**Plugin path resolution takes an injected base dir, not
`env::current_exe()`.** The Swift version's `Bundle.main` / `#filePath`
dance is inherently platform- and packaging-specific. This crate instead
takes a `plugin_root: Option<&Path>` on `LaunchRequest` (or a builder field)
- the caller (eventually `skwad`, which knows whether it's a dev checkout or
an installed bundle) resolves the base directory once; this crate only
joins `<plugin_root>/<agent_type>` and checks existence. Keeps the crate
free of `std::env` / bundle-resolution logic and trivially testable with a
tempdir.

**Shell escaping is a small hand-rolled function, not a crate dependency.**
The Swift version escapes five characters (`\`, `"`, `$`, backtick, `!`) for
embedding inside a double-quoted argument - not general POSIX shell
quoting (no single-quote escaping needed since these strings are always
wrapped in double quotes). A `shell_words`-style crate solves a different
problem (building an argv array); porting the five-character replace
matches the spec's shown examples exactly and needs no new dependency.

## Risks / Trade-offs

- [Byte-for-byte flag/quoting drift from the Swift reference would silently
  break an agent CLI's argument parsing] → every scenario in the spec
  becomes a unit test asserting the exact built string, not just
  substring/contains checks.
- [`agent_type` as a bare `&str` allows typos/unknown types to fall through
  every `match` to a default arm] → matches the spec's own framing (unknown
  types get no resume/MCP/registration args, same as `shell`'s non-launch
  types today); no validation layer is specified, so none is added here.

## Migration Plan

New crate, additive `Cargo.toml` workspace member. No existing code changes.
No rollback concerns beyond removing the crate.
