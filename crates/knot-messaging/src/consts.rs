//! Crate-wide constants.

/// Read messages beyond this count are pruned oldest-first by
/// [`crate::MessageStore::cleanup`]. Unread messages are never pruned.
pub const READ_RETENTION_LIMIT: usize = 100;
