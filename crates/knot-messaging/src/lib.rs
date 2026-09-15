//! In-process agent-to-agent message queue: send, broadcast, check, unread
//! tracking, workspace/companion/shell routing rules, and retention
//! cleanup. Messages are runtime state only - they do not persist across an
//! app restart.
//!
//! Contract: `openspec/specs/mcp-messaging/spec.md`.

mod consts;
mod error;
mod message;
mod notify;
mod routing;
mod store;

pub use consts::READ_RETENTION_LIMIT;
pub use error::{Result, SendError};
pub use message::Message;
pub use notify::{
    DeliveryEvent, DeliveryNotifier, NoopNotifier, QueuedNotifier, RecordingNotifier,
};
pub use routing::{broadcast, check, send};
pub use store::MessageStore;
