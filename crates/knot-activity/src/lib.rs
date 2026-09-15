//! Drives an agent's automatic status from terminal output, user keystrokes,
//! hook events, and process exit; the input-protection guard that blocks
//! automatic text injection while a user is typing; the hook-driven
//! awaiting-input state; and the gating of the deferred MCP registration
//! prompt.
//!
//! Contract: `openspec/specs/activity-detection/spec.md`.
//!
//! The status state machine lives in [`state::ActivityState`] as a pure,
//! clock-injected model over `tokio::time::Instant`. [`Tracker`] wraps it in a
//! small tokio actor that owns the idle, input-protection, and registration
//! timers and dispatches effects to an [`EventSink`].

mod consts;
mod events;
mod state;
mod tracker;
mod tracking;

pub use events::{ActivitySource, Effect, EventSink, KeyEvent, StatusEvent};
pub use state::{ActivityState, TrackerConfig};
pub use tracker::Tracker;
pub use tracking::{ActivityTracking, tracking_for};
