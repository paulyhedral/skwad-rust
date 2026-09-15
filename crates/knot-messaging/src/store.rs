use uuid::Uuid;

use crate::consts::READ_RETENTION_LIMIT;
use crate::message::Message;

/// In-memory message queue. No persistence: a fresh `MessageStore` is the
/// entire history on every app restart.
#[derive(Debug, Clone, Default)]
pub struct MessageStore {
    messages: Vec<Message>,
}

impl MessageStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add(&mut self, message: Message) {
        self.messages.push(message);
    }

    pub fn unread_for(&self, agent_id: Uuid) -> Vec<&Message> {
        self.messages
            .iter()
            .filter(|m| m.to == agent_id && !m.is_read)
            .collect()
    }

    pub fn unread_count(&self, agent_id: Uuid) -> usize {
        self.messages
            .iter()
            .filter(|message| message.to == agent_id && !message.is_read)
            .count()
    }

    pub fn mark_read(&mut self, agent_id: Uuid) {
        for m in self.messages.iter_mut().filter(|m| m.to == agent_id) {
            m.is_read = true;
        }
    }

    pub fn has_unread(&self, agent_id: Uuid) -> bool {
        self.messages.iter().any(|m| m.to == agent_id && !m.is_read)
    }

    pub fn latest_unread_id(&self, agent_id: Uuid) -> Option<Uuid> {
        self.messages
            .iter()
            .rev()
            .find(|m| m.to == agent_id && !m.is_read)
            .map(|m| m.id)
    }

    /// Prunes the oldest read messages beyond [`READ_RETENTION_LIMIT`].
    /// Unread messages are never touched.
    pub fn cleanup(&mut self) {
        let read_count = self.messages.iter().filter(|m| m.is_read).count();
        if read_count <= READ_RETENTION_LIMIT {
            return;
        }
        let mut to_remove = read_count - READ_RETENTION_LIMIT;
        self.messages.retain(|m| {
            if m.is_read && to_remove > 0 {
                to_remove -= 1;
                false
            } else {
                true
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn msg(to: Uuid) -> Message {
        Message::new(Uuid::new_v4(), to, "hi")
    }

    #[test]
    fn unread_filters_by_recipient_and_read_flag() {
        let mut store = MessageStore::new();
        let agent = Uuid::new_v4();
        let other = Uuid::new_v4();
        store.add(msg(agent));
        store.add(msg(other));

        let unread = store.unread_for(agent);

        assert_eq!(unread.len(), 1);
        assert_eq!(unread[0].to, agent);
    }

    #[test]
    fn mark_read_clears_has_unread() {
        let mut store = MessageStore::new();
        let agent = Uuid::new_v4();
        store.add(msg(agent));

        assert!(store.has_unread(agent));
        store.mark_read(agent);
        assert!(!store.has_unread(agent));
    }

    #[test]
    fn unread_count_tracks_only_unread_messages_for_recipient() {
        let mut store = MessageStore::new();
        let agent = Uuid::new_v4();
        let other = Uuid::new_v4();
        store.add(msg(agent));
        store.add(msg(agent));
        store.add(msg(other));

        assert_eq!(store.unread_count(agent), 2);
        store.mark_read(agent);
        assert_eq!(store.unread_count(agent), 0);
    }

    #[test]
    fn latest_unread_id_is_the_most_recent() {
        let mut store = MessageStore::new();
        let agent = Uuid::new_v4();
        store.add(msg(agent));
        let second = msg(agent);
        let second_id = second.id;
        store.add(second);

        assert_eq!(store.latest_unread_id(agent), Some(second_id));
    }

    #[test]
    fn cleanup_prunes_oldest_read_messages_keeping_unread() {
        let mut store = MessageStore::new();
        let agent = Uuid::new_v4();
        let mut unread_ids = Vec::new();
        for _ in 0..5 {
            let m = msg(agent);
            unread_ids.push(m.id);
            store.add(m);
        }
        for _ in 0..150 {
            let mut m = msg(agent);
            m.is_read = true;
            store.add(m);
        }

        store.cleanup();

        let read_count = store.messages.iter().filter(|m| m.is_read).count();
        assert_eq!(read_count, READ_RETENTION_LIMIT);
        for id in unread_ids {
            assert!(store.messages.iter().any(|m| m.id == id));
        }
    }
}
