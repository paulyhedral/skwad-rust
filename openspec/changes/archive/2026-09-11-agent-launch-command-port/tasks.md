## 1. Crate scaffold

- [x] 1.1 Create `crates/skwad-agent-launch` (`Cargo.toml` depending on
      `skwad-core`, `uuid`), add it to the root workspace `[workspace.members]`,
      and verify `cargo build -p skwad-agent-launch` succeeds with an empty
      `lib.rs`.
- [x] 1.2 Add `consts.rs` for the fixed strings (registration user prompt,
      skwad system instructions template, MCP arg fragments, messaging tool
      names for Copilot's allow-list) and verify the crate still builds.

## 2. Agent-type capability lookups

- [x] 2.1 Implement `can_resume`, `can_fork`, `supports_system_prompt`,
      `supports_inline_registration` (one `&str -> bool` function each,
      matching the spec's per-type tables) and verify unit tests cover every
      named type (`claude`, `codex`, `opencode`, `gemini`, `copilot`, `shell`)
      plus one unknown type.

## 3. Shell escaping and persona text

- [x] 3.1 Implement `shell_escape` (backslash, double-quote, `$`, backtick,
      `!`) and verify a unit test round-trips each escaped character.
- [x] 3.2 Implement `persona_prompt` (returns `None` for an empty-instructions
      persona) and verify a unit test for both the populated and empty cases.

## 4. Registration and MCP argument builders

- [x] 4.1 Implement the registration prompt strings (`registration_user_prompt`,
      `registration_prompt(agent_id)`, `skwad_instructions(agent_id)`) and
      verify a unit test checks the agent id is embedded verbatim.
- [x] 4.2 Implement `mcp_arguments(agent_type, mcp_url, plugin_root)` per the
      spec's four agent types (claude/codex/gemini/copilot) and verify unit
      tests for: each type's argument text, plugin-dir-present vs. absent for
      claude/codex, and the empty string for every other type.
- [x] 4.3 Implement `inline_registration_arguments(agent_type, agent_id,
      is_resume, persona)` per the spec's per-type table, including the
      resume-drops-user-prompt and persona-appended-to-system-prompt rules,
      and verify unit tests for each scenario named in
      `openspec/specs/agent-launch-command/spec.md` (Claude resume/fork,
      persona reaches Claude, persona dropped for Gemini, Gemini resume adds
      nothing).

## 5. Command assembly and initialization wrapper

- [x] 5.1 Define `LaunchRequest` (agent type, agent id, shell command, resume
      session id, fork flag, persona, plugin root) and verify it derives
      `Clone`/`Debug`.
- [x] 5.2 Implement `build_agent_command(settings, request)` wiring base
      command lookup, resume/fork args, user options, and MCP/registration
      injection in spec order, and verify unit tests for: empty configured
      command -> empty result, shell agent -> custom command or empty, Codex
      resume/fork subcommand vs. other types' flags, MCP-disabled omits all
      MCP/registration args.
- [x] 5.3 Implement `build_initialization_command(folder, agent_command,
      agent_id)` (leading-space wrapper, `SKWAD_AGENT_ID` prefix, no-env-var
      shell case) and verify unit tests for both the non-empty and empty
      `agent_command` cases.

## 6. Verification

- [x] 6.1 Run `cargo +nightly fmt`, `cargo clippy -p skwad-agent-launch
      --all-targets -- -D warnings`, and `cargo test -p skwad-agent-launch`,
      and verify all three succeed with zero warnings/failures.
- [x] 6.2 Cross-check every scenario in
      `openspec/specs/agent-launch-command/spec.md` against a test name in
      the crate and verify none are missing.
