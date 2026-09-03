---
name: repo-setup-standard
description: Scaffold a new or under-scaffolded repo with CI, dependabot, branch protection, community docs, and AGENTS.md/CLAUDE.md robot setup. Use when the user asks to "set up the repo", "add repo scaffolding", or "bootstrap CI".
metadata:
  type: engineering
---

# Repo setup standard

Bring a repo up to a working baseline: CI, dependabot, branch protection, community docs, and
robot guidance. This is infrastructure work, separate from implementing whatever the repo does -
do it first, commit it on its own, then move to feature work.

Discovery-first: detect what the repo actually is, then scaffold to match. Don't copy another
repo's CI file blind.

## Steps

### 1. Detect the stack

Signals: `Cargo.toml` (Rust), `Package.swift` / `*.xcodeproj` (Swift), `pyproject.toml` /
`requirements.txt` (Python), `package.json` (TS/JS), `go.mod` (Go). A repo can be mid-migration
and carry more than one - scaffold for the stack that CI should actually build. If none exist
(a brand-new empty repo), ask the user what the repo will be.

Resolve `owner/repo` from `git remote -v` (or `gh repo view --json owner,name`). The default
branch is `main` unless the repo says otherwise.

### 2. What to add

1. **CI workflow(s)** (`.github/workflows/`) - build, lint, and test for the detected stack:
   - Rust: `cargo build`, `cargo clippy -- -D warnings`, `cargo test`, `cargo +nightly fmt --check`.
   - Swift: `xcodebuild build`/`test` or `swift build`/`swift test` as appropriate.
   Trigger on `push` to the default branch and on `pull_request`. Include
   `.github/workflows/**` in any push-path filter, or a CI-only commit ships unverified.
2. **Dependabot** (`.github/dependabot.yaml`) - ecosystem(s) matching the stack (`cargo`,
   `github-actions`, `swift`, ...), weekly, targeting the default branch.
3. **Community docs** - `README.md`, `CONTRIBUTING.md`, `SECURITY.md`, `CODE_OF_CONDUCT.md`.
   Adapt names, URLs, and what the repo does. Seed `CHANGELOG.md` with an `[Unreleased]`
   section noting the scaffolding work if the repo uses a changelog.
4. **Robot guidance** - `AGENTS.md` covering: About This Project, dependency relationships,
   Committing Code (Conventional Commits), Branches and Workflow, and Running Checks Locally
   with the actual detected-stack commands. Symlink `CLAUDE.md` to it - a real symlink, not a
   copy: `ln -s AGENTS.md CLAUDE.md`.
5. **Branch protection** on the default branch - require PRs and passing CI. Match the
   required status-check names to this repo's own CI job names (from step 2.1), and disable
   squash merges if the repo's release tooling relies on Conventional Commits prefixes
   surviving the merge (`git-cliff`, `git-cliff`-style changelog generation). Prefer
   `["merge", "rebase"]` in that case.
6. **Repository setting: allow auto-merge** - `gh repo edit <owner>/<repo> --enable-auto-merge`.
   Off by default on new repos; without it `gh pr merge --auto` fails. Verify with
   `gh api repos/<owner>/<repo> --jq '.allow_auto_merge'`.
7. **`release` label** - green, for tagging release PRs/issues:
   `gh label create release --repo <owner>/<repo> --color 00FF00 --description "Release" --force`.
8. **Architecture Decision Records** - a `docs/adr/` directory with `0000-template.md` and a
   short `README.md` index, plus the `/adr` command (`.claude/commands/adr.md`). Add a
   `## Architecture Decisions` section to `AGENTS.md` pointing at `docs/adr/README.md` and
   noting that `/adr "<title>"` scaffolds a new record.

### Optional, only if the repo needs it

- **Code coverage** - a coverage step, a warn-only threshold check
  (`continue-on-error: true`, never blocks a merge), an uploaded HTML report artifact, and a
  shields.io endpoint badge published to GitHub Pages. Rust: `cargo tarpaulin --engine llvm
  --out Html --out Xml`. Swift: `swift test --enable-code-coverage` + `llvm-cov export
  -format=lcov`. Enable Pages explicitly - a `gh-pages` branch existing is not the same as
  Pages serving it (`gh api repos/<owner>/<repo>/pages`).
- **Multi-arch Docker** - if the repo ships a container, `docker/build-push-action` builds
  the runner's native arch only unless `platforms: linux/amd64,linux/arm64` is passed
  explicitly. QEMU/Buildx setup steps alone are not enough. Verify with
  `docker buildx imagetools inspect <image>:latest`.
- **Release workflow** - if releases run through CI rather than a local `make` target, add a
  `workflow_dispatch` release workflow that bumps the version, updates `CHANGELOG.md`, and
  publishes. See the `release-repo` skill for this repo's actual release flow before adding
  a second mechanism.

## Gotchas

- **Required checks that "never ran"**: GitHub lets you register a required status-check
  context before it first reports - expected on a fresh repo, not an error. It starts
  enforcing on the first PR that runs CI.
- **Empty-diff PRs get rejected**: if opening a PR right after scaffolding with no other
  commits, `gh pr create` fails with "No commits between X and Y". `git commit --allow-empty`
  unblocks it only if a PR is genuinely needed before real work lands.
- **`git add`**: stage new files explicitly by path, never `git add -A`/`.`.
- **`gh` TLS/certificate error**: sandboxed-network restriction, not a GitHub outage - retry
  outside the sandbox.
