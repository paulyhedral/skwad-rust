# Contributing to Skwad (Rust port)

This repo holds the Rust port of Skwad alongside the original Swift/SwiftUI app.
The Swift app under `Skwad/` is the behavioral reference; the port moves it into
the `crates/` workspace one subsystem at a time, each driven by an OpenSpec
change and tied to a spec contract.

## Before you start

- Rust toolchain is pinned by `rust-toolchain.toml` (1.98.0, with `rustfmt` and
  `clippy`). `rustup` picks it up automatically.
- `git` >= 2.30 must be on `PATH` - `skwad-git` tests parse porcelain v2 output.
- Nightly `rustfmt` is used for formatting: `rustup toolchain install nightly`.

## Workflow

git-flow. `develop` is the integration branch; `main` is release-only.

1. Branch from `develop`: `feature/<change>` (e.g. `feature/git-operations-port`).
   Release and hotfix branches (`release/x.y.z`, `hotfix/x.y.z`) PR to `main`.
2. For a subsystem port, work through its OpenSpec change under `openspec/changes/`.
   The spec files in `openspec/specs/` are the contract - crate module docs link
   back to them.
3. Keep commits scoped and conventional (see below).
4. Open a PR against `develop` (release/hotfix against `main`). CI
   (`.github/workflows/rust.yml`) must pass: `cargo fmt --check`,
   `cargo clippy -D warnings`, `cargo test`, `cargo build` on Linux and macOS.
5. PRs merge with a merge commit or rebase - never squash - so Conventional
   Commit prefixes survive in history.

## Running checks locally

```bash
make rust          # fmt (nightly) + clippy + test + build for the whole workspace

# or individually
make rust-fmt
make rust-lint
make rust-test
make rust-build
```

`make rust-fmt` runs `cargo +nightly fmt --check`. Run `cargo +nightly fmt`
before committing.

## Commit messages

[Conventional Commits](https://www.conventionalcommits.org/). Use the crate name
as the scope:

```
feat(skwad-git): parse ahead/behind from status
fix(skwad-discovery): debounce watch events per folder
docs(openspec): archive worktree-management-port change
build(rust): add dependabot config
```

## Architecture decisions

Non-trivial design choices get an ADR under `docs/adr/`. Run `/adr "<title>"` or
copy `docs/adr/0000-template.md`. Index: `docs/adr/README.md`.

## Changelog

User-facing changes go under `## [Unreleased]` in `CHANGELOG.md` in the
Keep a Changelog format (Added / Changed / Fixed / Removed).

## License

By contributing you agree your work is licensed under AGPL-3.0-only, matching the
project.
