## Context

See proposal.md - Why. The repo has `openspec/specs/` and the Swift reference
app but no Rust. The port context (stack mapping) already commits the toolkit:
gpui, libghostty FFI, axum, tokio, serde. This change lays the workspace and
implements `openspec/specs/git-operations/spec.md`, which defines behavior in
terms of `git` CLI invocations and their parsed output.

## Goals / Non-Goals

**Goals:**

- A Cargo workspace that builds on macOS and Linux CI, additive to the Swift tree.
- Crate boundaries that keep `skwad-git` free of GUI and async-runtime deps.
- `skwad-git` passing every scenario in the git-operations spec, with parsers
  unit-tested off fixture strings and operations tested against temp repos.
- The gpui binary compiles and opens a window, proving the toolkit choice early.

**Non-Goals (design-level):**

- Any shared runtime wiring between `skwad` and `skwad-git` (git stays sync).
- A real localization backend — `skwad-core` ships a lookup shim only.
- Deciding the module port order beyond git-operations (later changes).

## Decisions

### Workspace layout: `crates/` at repo root

Virtual manifest `Cargo.toml` with `members = ["crates/*"]`. `target/` added to
`.gitignore`. Swift files untouched. Rejected: nesting Rust under a subdir like
`rust/` — needless path depth, and the Rust app is the repo's future primary
artifact.

### Three crates now

- `skwad-git` — git-operations capability. No dep on tokio, gpui, or axum.
- `skwad-core` — error/`Result` aliases, shared newtypes, `consts`, and a
  `t(key)` localization shim (static map, fluent later). No GUI dep.
- `skwad` — the gpui binary. Depends on `skwad-core`; will depend on
  `skwad-git` when a git panel exists (not this change).

Rationale: `skwad-git` must be callable from tests, a CLI, or the GUI without
dragging a runtime. Keeping it runtime-agnostic is the main structural bet.

### `skwad-git` shells out to `git`, does not use `gix`

The spec is written as specific `git` invocations (`status --porcelain=v2
--branch`, `diff --numstat`, `add -A`, `commit -m`, `rev-list --left-right`).
Shelling out matches the contract exactly. `gix` would mean reimplementing
porcelain/diff semantics and risking divergence from the spec's scenarios for
no gain here. Trade-off: a `git` binary must be on PATH — acceptable, it already
is for the Swift app.

### Command runner: sync, thread-based timeout

`std::process::Command` spawns the child; output is read on a worker thread that
sends to a `mpsc::sync_channel`. The caller does `recv_timeout(Duration)`; on
elapse it calls `child.kill()` and returns `GitError::Timeout { command }`.
Non-zero exit -> `GitError::Command { command, stderr_or_stdout, code }`. Stdout
returned `.trim()`-ed. Rejected: `tokio::process` + `tokio::time::timeout` —
pulls tokio into `skwad-git`, violating the runtime-agnostic boundary.

### Parsers are pure functions over `&str`

`parse_status(&str) -> RepoStatus`, `parse_diff(&str) -> Vec<FileDiff>`,
`parse_numstat(&str) -> Vec<(u64, u64, PathBuf)>`. No process calls inside them,
so they test off captured fixture strings with `insta` snapshots. Operations
(`stage`, `commit`, ...) are thin wrappers over the runner and get integration
tests that build a temp repo with `tempfile` + the runner itself.

### Error model: one `thiserror` enum per crate

`skwad_git::GitError`, `skwad_core::Error`. No shared god-error. `skwad-git`
errors are developer-facing (Rust convention, and the spec calls for errors
carrying command/stderr/code) so they skip the localization shim; user-facing
strings in the GUI layer go through `skwad_core::t`.

### Constants in one module per crate

`skwad-git/src/consts.rs` holds the default 30s timeout and the argv arrays for
each git subcommand. Matches the repo convention.

### gpui via GPUI Kit (crates.io), not a pinned git dependency

Depend on [`gpui-kit`](https://github.com/longbridge/gpui-kit) (Apache-2.0,
crates.io, currently 0.6.x), which vends a maintained `gpui` fork
(`gpui-pre`) plus `gpui-component` as one versioned dependency. This replaces
the originally-planned pinned git dependency on `zed-industries/zed`: normal
semver instead of a hand-chosen commit, and `cargo update` moves it forward
like any other dependency. Still isolated to the `skwad` crate so its build
cost never touches `skwad-git` or CI's fast test path.

### CI / Makefile

Add `rust-fmt` (`cargo +nightly fmt --check`), `rust-lint` (`cargo clippy
--all-targets -- -D warnings`), `rust-test` (`cargo test --workspace`),
`rust-build` targets. A GitHub Actions job runs them on macOS + Linux. The
existing Swift `make test` job is untouched.

## Risks / Trade-offs

- [`gpui-kit`'s large default feature set (icon assets, optional tree-sitter
  grammars) slows the `skwad` build] → Confine it to the `skwad` crate; CI's
  fmt/clippy/test steps for `skwad-git` never pull it in. Trim default
  features (`default-features = false`, opt back into `component`/`assets`)
  if build time becomes a problem.
- [`git` output drifts across versions] → Use `--porcelain=v2` (documented
  stable format) and `--no-color`; pin a `git` version in CI and note the
  minimum in the crate README.
- [Timeout `child.kill()` leaves grandchild processes] → The git subcommands
  used spawn no long-lived children; document the assumption. Revisit with
  process-group kill if a future subcommand needs it.
- [Runtime-agnostic `skwad-git` proves inconvenient once the GUI needs async
  git] → Wrap calls in `spawn_blocking` at the GUI boundary; the crate stays
  clean. Cheap to revisit, expensive to undo if we bake tokio in now.
- [Windows] → Explicitly unsupported; CI is macOS + Linux only.

## Migration Plan

Purely additive. No deploy step. Rollback = delete `crates/` and the workspace
`Cargo.toml`, revert the `Makefile`/CI/`.gitignore` hunks; the Swift build never
depended on any of it.

## Open Questions

- Whether `skwad-core::t` later uses `fluent-rs` or a lighter table — deferred;
  the shim's call signature (`t("key") -> String`) is what callers depend on.
