//! Runtime agent lifecycle: creation, removal, restart/resume, edit, bench
//! deployment, and ordering, layered over `skwad_core`'s durable settings
//! store.
//!
//! Contract: `openspec/specs/agent-lifecycle/spec.md`.

mod agent;
mod convert;
mod error;
mod store;

pub use agent::{Agent, AgentState};
pub use convert::{from_saved, to_saved};
pub use error::{AgentError, Result};
pub use store::{AgentStore, CreateOptions, EditRequest, RemovedAgent};
