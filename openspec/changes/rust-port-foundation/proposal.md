## Why

`skwad-rust` has baseline specs but no Rust code — the repo is still the Swift
app plus `openspec/specs/`. The port needs a foundation: a Cargo workspace, the
committed toolkit choices wired to a running shell, and one real module ported
end to end to prove the structure. `git-operations` is the lowest-dependency
module (no GUI, no async runtime, no MCP) and already has a complete spec, so it
is the first slice.

## What Changes

- Add a Cargo workspace at the repo root alongside the existing Swift tree
  (Swift app stays as the reference implementation, untouched).
- Crates:
  - `skwad-git` — the `git-operations` capability (this change implements it).
  - `skwad-core` — shared types, error model, constants, localization lookup
    (skeleton only this change).
  - `skwad` — the gpui binary; opens an empty window and exits cleanly
    (skeleton only this change).
- Commit the stack choices from the port context as actual dependencies:
  gpui (GUI), tokio (async runtime), axum (HTTP, not wired yet), serde +
  `directories` (config). libghostty FFI and the MCP server are out of scope
  here.
- Implement `skwad-git` fully against `openspec/specs/git-operations/spec.md`:
  command runner with timeout, porcelain v2 status parsing, unified-diff
  parsing, numstat combined stats, staging/commit operations, branch and
  ahead/behind queries. Unit tests + `insta` snapshots for the parsers.
- Extend the `Makefile` (or add `justfile`) with `fmt`, `lint` (clippy),
  `test`, `build` targets for the Rust workspace; wire the same into CI.

Non-goals (each its own later change):

- Porting any GUI view, the terminal embedding, or libghostty FFI.
- The MCP HTTP server, MCP tools, agent-to-agent messaging.
- Agent lifecycle, worktree management, file watching, hooks, personas,
  settings persistence, repo discovery, conversation history.
- Auto-update, packaging, notarization.
- Removing or modifying the Swift app.

## Capabilities

### New Capabilities

None. This change scaffolds the workspace and implements the existing
`git-operations` spec without changing its requirements.

### Modified Capabilities

None. `openspec/specs/git-operations/spec.md` is the unchanged contract for
`skwad-git`; this change adds the implementation, not new behavior.

## Impact

- New: `Cargo.toml` (workspace), `crates/skwad-git/`, `crates/skwad-core/`,
  `crates/skwad/`, `Cargo.lock`, `rust-toolchain.toml`, `.cargo/` if needed.
- Modified: `Makefile`, `.github/workflows/`, `.gitignore` (add `target/`).
- Dependencies added: gpui, tokio, axum, serde, serde_json, thiserror,
  directories, insta (dev). Exact versions resolved at implementation time
  against latest stable.
- The Swift build (`Package.swift`, `Skwad.xcodeproj`, `make test`) is
  unaffected; Rust targets are additive.
- `skip_specs: true` — scaffolding plus implementing an existing spec; no
  spec-level behavior changes.
