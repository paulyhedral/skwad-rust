# skwad-git

Runs `git` and parses its output into structured data for the Skwad Rust port:
command runner with timeout, porcelain v2 status, unified-diff parsing, numstat
stats, staging/commit operations, and branch / ahead-behind queries.

Behavior contract: `openspec/specs/git-operations/spec.md`.

## Requirements

- A `git` binary on `PATH`. **Minimum 2.30**; CI and the test suite run against
  2.55. The crate relies on `git status --porcelain=v2 --branch` and
  `git init -b <name>` (2.28+), both stable since well before 2.30.

## Design notes

- Runtime-agnostic: no async runtime. Wrap calls in `spawn_blocking` at an
  async boundary.
- The command runner drains stdout/stderr on worker threads and kills the child
  on timeout.
