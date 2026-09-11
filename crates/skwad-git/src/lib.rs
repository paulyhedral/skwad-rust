//! Runs `git` and turns its output into structured data: command runner with
//! timeout, porcelain v2 status parsing, unified-diff parsing, numstat stats,
//! staging/commit operations, branch / ahead-behind queries, and worktree
//! detection / creation.
//!
//! Contract: `openspec/specs/git-operations/spec.md` and
//! `openspec/specs/worktree-management/spec.md`.
//!
//! Requires a `git` binary on `PATH` (>= 2.30; see `README.md`).
//! Runtime-agnostic: no async runtime; wrap calls in `spawn_blocking` at an
//! async boundary.

pub mod consts;
pub mod diff;
pub mod error;
pub mod repository;
pub mod runner;
pub mod stats;
pub mod status;
pub mod worktree;

pub use diff::{DiffLine, FileDiff, Hunk, LineKind};
pub use error::{GitError, Result};
pub use repository::Repository;
pub use runner::Runner;
pub use stats::{DiffStats, parse_numstat};
pub use status::{ChangeType, FileEntry, RepoStatus};
pub use worktree::{is_working_tree, suggest_worktree_path};
