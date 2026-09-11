//! Reads a coding agent's past sessions for a given project folder:
//! per-agent providers (claude, codex, gemini, copilot), a uniform
//! `SessionSummary` shape, and a folder-scoped cache with explicit
//! refresh / invalidate and delete-then-backfill.
//!
//! Contract: `openspec/specs/conversation-history/spec.md`.
//!
//! Runtime-agnostic: no async runtime; wrap calls in `spawn_blocking` at an
//! async boundary. Providers swallow their own I/O errors and degrade to an
//! empty result rather than surfacing them.

pub mod cache;
pub mod consts;
pub mod error;
pub mod paths;
pub mod provider;
pub mod providers;
pub mod title;

pub use cache::HistoryCache;
pub use error::{HistoryError, Result};
pub use provider::{HistoryProvider, SessionSummary, provider, supports_history};
