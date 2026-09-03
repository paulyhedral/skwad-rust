//! Runs `git` and turns its output into structured data: command runner with
//! timeout, porcelain v2 status parsing, unified-diff parsing, numstat stats,
//! staging/commit operations, and branch / ahead-behind queries.
//!
//! Contract: `openspec/specs/git-operations/spec.md`.
//!
//! Requires a `git` binary on `PATH`. Runtime-agnostic: no async runtime; wrap
//! calls in `spawn_blocking` at an async boundary.

pub mod consts;
pub mod error;
pub mod runner;

pub use error::{GitError, Result};
pub use runner::Runner;
