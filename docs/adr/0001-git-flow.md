---
id: ADR-0001
title: Git-flow branching model
status: accepted
date: 2026-09-10
tags: [process, ci]
---

# ADR-0001: Git-flow branching model

## Context

Through `worktree-management-port`, every change PR'd straight into `main` and
CI ran only on `main`. That's fine for a young repo with one active line of
work, but the Rust port and the Swift reference app now both take PRs, and a
release needs to be cuttable from a known-good point without freezing
in-flight port work. There was no branch that represented "next release" as
distinct from "currently released."

## Decision

We use git-flow: `develop` is the integration branch every feature branches
from and PRs back into; `main` holds only released code, advanced exclusively
by `release/x.y.z` and `hotfix/x.y.z` branches. Rulesets on both `main` and
`develop` require the Rust CI matrix (`workspace (ubuntu-latest)` and
`workspace (macos-latest)`) and a PR - no direct pushes. Merges use a merge
commit or rebase, never squash, so Conventional Commit prefixes survive in
history.

## Options considered

- **Trunk-based (status quo)** - every branch PRs into `main`, CI gates
  merges. Simplest, but conflates "integrated" with "released": a release
  cut is just whatever `main` happens to be at that moment, and there's no
  place to stabilize a release candidate while port work keeps landing.
- **Git-flow** - adds `develop` as the integration branch and
  `release/*`/`hotfix/*` as the only paths onto `main`. One more branch to
  keep in sync, but gives releases a clean cut point and a place for
  release-only fixes (`hotfix/*`) that don't have to wait on in-flight
  `develop` work. Chosen.
- **GitHub Flow + release branches ad hoc** - close to git-flow without the
  naming convention or the rule that `main` only moves via release/hotfix.
  Rejected for being the same shape with less enforcement.

## Consequences

- Every feature PR now targets `develop`, not `main`; `project-start-change`
  and `project-finish-change` branch from and merge into `develop`.
- `develop` needs to be merged forward from `main` after every release
  (release and hotfix branches land on `main` first) - see the
  `prepare-release` / `tag-release` / `release` workflows, which include a
  merge-back-to-`develop` step for exactly this.
- Two long-lived branches now need their CI and branch-protection rules kept
  in sync (`rust.yml` runs on both; both carry the same ruleset).
