# Skwad (Rust port) - Agent Development Guide

Context for AI agents working on this codebase.

## About This Project

Skwad is a macOS app that runs a team of AI coding agents, each in an embedded
terminal, and lets them coordinate over MCP. It was built in Swift/SwiftUI; this
repo ports it to Rust.

Two trees live here:

- `crates/` - the Rust workspace (the port, in progress).
- `Skwad/`, `SkwadTests/`, `Package.swift`, `Skwad.xcodeproj` - the original
  Swift app. It is the **behavioral reference**: when porting a subsystem, its
  Swift implementation is the spec for what the Rust code must do. Do not add
  features to the Swift app here.

The port proceeds one subsystem at a time. Each is driven by an OpenSpec change
under `openspec/changes/` and pinned to a contract under `openspec/specs/`, which
the crate's module docs link back to.

## Workspace Layout

```
crates/
├── skwad-core/        Shared types, error model, constants, localization (l10n::t)
├── skwad-git/         Runs `git`, parses porcelain v2 status, unified diffs,
│                      numstat; staging/commit, branch + ahead/behind, worktrees.
│                      Runtime-agnostic (no async runtime; wrap in spawn_blocking).
├── skwad-discovery/   Maps a source folder to repos + linked worktrees via
│                      filesystem reads only (no `git` process); tokio-driven
│                      debounced folder watch. `scan()` is a pure function.
└── skwad/             Binary. GPUI Kit window shell (gpui-kit crate).
```

### Dependency relationships

- `skwad` depends on `skwad-core` and `gpui-kit` (external, the UI toolkit).
- `skwad-git` and `skwad-discovery` are standalone: they depend on `thiserror`
  (via workspace) and, for discovery, `notify` + `tokio`.
- Shared dep versions are pinned in the root `Cargo.toml` `[workspace.dependencies]`.
  Add new shared deps there, reference with `dep.workspace = true`.
- Edition 2024, `rust-version = "1.98"`, toolchain pinned by `rust-toolchain.toml`.

### Contracts (OpenSpec)

- `openspec/specs/repo-discovery/spec.md` - `skwad-discovery`
- `openspec/specs/git-operations/spec.md` - `skwad-git`
- `openspec/specs/worktree-management/spec.md` - `skwad-git` worktree module

In-progress and archived changes are under `openspec/changes/`. Use the
`opsx:*` / OpenSpec skills to propose, apply, and archive changes.

## Committing Code

[Conventional Commits](https://www.conventionalcommits.org/). Scope is the crate
name (or `openspec`, `rust`, `ci`):

```
feat(skwad-git): parse ahead/behind from status
fix(skwad-discovery): debounce watch events per folder
docs(openspec): archive worktree-management-port change
build(rust): add dependabot config
```

PRs merge with a **merge commit** so the prefixes survive in history. Do not
squash.

## Branches and Workflow

git-flow. `develop` is the integration branch; `main` is release-only.

1. Branch from `develop`: `feature/<change>` (or `release/x.y.z`, `hotfix/x.y.z`).
2. Implement against the OpenSpec change / spec contract.
3. Open a PR to `develop` (`release/*` and `hotfix/*` PR to `main`). Rulesets on
   both branches require the Rust CI matrix (`workspace (ubuntu-latest)` and
   `workspace (macos-latest)` from `.github/workflows/rust.yml`) to pass, and a
   PR to merge. Merge with a merge commit or rebase, never squash.
4. `dependabot` opens weekly grouped PRs against `develop` for `cargo` and
   `github-actions`.

## Running Checks Locally

```bash
make rust          # fmt (nightly) + clippy + test + build, whole workspace

make rust-fmt      # cargo +nightly fmt --check
make rust-lint     # cargo clippy --workspace --all-targets -- -D warnings
make rust-test     # cargo test --workspace
make rust-build    # cargo build --workspace
```

Run `cargo +nightly fmt` before committing (needs `rustup toolchain install
nightly`). `skwad-git` tests need `git` >= 2.30 on `PATH` for porcelain v2.

The Swift app has its own targets in the same `Makefile` (`make build`,
`make test`, `make notarize`) and its own CI (`tests.yml`, `build.yml`); those
are unrelated to port work.

## Architecture Decisions

Non-trivial design choices are recorded as ADRs under `docs/adr/`. Index and
process: `docs/adr/README.md`. `/adr "<title>"` scaffolds a new record from
`docs/adr/0000-template.md`.

## Conventions

- Constants live in a single `consts.rs` per crate.
- Errors: `thiserror` enums per crate (`GitError`, `DiscoveryError`, core
  `Error`), re-exported with a crate `Result` alias.
- User-facing text goes through `skwad_core::l10n::t`.
- Keep functions to <= 5-6 args; group related args in a struct.
- No statement-hugging brace style; format with nightly `rustfmt`.
