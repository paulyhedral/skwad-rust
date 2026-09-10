//! Maps a source folder to the repositories it holds, each with its linked
//! worktrees, using only filesystem reads (no `git` process), and keeps the
//! list fresh with a debounced watch on the folder.
//!
//! Contract: `openspec/specs/repo-discovery/spec.md`.
//!
//! [`scan()`] is a pure function. [`Discovery`] wraps it in a `tokio`-driven
//! coordinator that publishes results on a `watch` channel.

pub mod consts;
pub mod discovery;
pub mod error;
pub mod scan;

pub use discovery::Discovery;
pub use error::{DiscoveryError, Result};
pub use scan::{scan, RepoInfo, WorktreeInfo};
