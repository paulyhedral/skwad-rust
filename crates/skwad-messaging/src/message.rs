use std::time::SystemTime;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// An agent-to-agent message. Held in memory only; does not persist across
/// an app restart.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Message {
    pub id: Uuid,
    pub from: Uuid,
    pub to: Uuid,
    pub content: String,
    pub timestamp: SystemTime,
    pub is_read: bool,
}

impl Message {
    /// Mints a fresh unread message.
    pub fn new(from: Uuid, to: Uuid, content: impl Into<String>) -> Self {
        Self {
            id: Uuid::new_v4(),
            from,
            to,
            content: content.into(),
            timestamp: SystemTime::now(),
            is_read: false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_message_is_unread() {
        let from = Uuid::new_v4();
        let to = Uuid::new_v4();

        let message = Message::new(from, to, "hello");

        assert!(!message.is_read);
        assert_eq!(message.from, from);
        assert_eq!(message.to, to);
        assert_eq!(message.content, "hello");
    }

    #[test]
    fn each_message_gets_a_fresh_id() {
        let a = Message::new(Uuid::new_v4(), Uuid::new_v4(), "a");
        let b = Message::new(Uuid::new_v4(), Uuid::new_v4(), "b");

        assert_ne!(a.id, b.id);
    }
}
