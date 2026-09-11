//! Per `(agent type, folder)` cache of session lists, with explicit
//! refresh / invalidate and delete-then-backfill.

use std::collections::HashMap;
use std::sync::Mutex;

use crate::provider::{SessionSummary, provider};

type CacheKey = (String, String);

#[derive(Default)]
pub struct HistoryCache {
    entries: Mutex<HashMap<CacheKey, Vec<SessionSummary>>>,
}

impl HistoryCache {
    pub fn new() -> Self {
        Self::default()
    }

    fn key(agent_type: &str, folder: &str) -> CacheKey {
        (agent_type.to_owned(), folder.to_owned())
    }

    /// The cached list for `(agent_type, folder)`, empty when absent. Never
    /// touches disk.
    pub fn get(&self, agent_type: &str, folder: &str) -> Vec<SessionSummary> {
        let key = Self::key(agent_type, folder);
        self.entries
            .lock()
            .unwrap()
            .get(&key)
            .cloned()
            .unwrap_or_default()
    }

    /// Reload from disk and replace the entry. No-op for an unsupported
    /// agent type.
    pub fn refresh(&self, agent_type: &str, folder: &str) {
        let Some(p) = provider(agent_type) else {
            return;
        };
        let sessions = p.load_sessions(folder);
        let key = Self::key(agent_type, folder);
        self.entries.lock().unwrap().insert(key, sessions);
    }

    /// Drop the entry without reloading.
    pub fn invalidate(&self, agent_type: &str, folder: &str) {
        let key = Self::key(agent_type, folder);
        self.entries.lock().unwrap().remove(&key);
    }

    /// Delete a session via its provider, then refresh the entry so the
    /// list reflects the removal. No-op for an unsupported agent type.
    pub fn delete_session(&self, agent_type: &str, id: &str, folder: &str) {
        let Some(p) = provider(agent_type) else {
            return;
        };
        p.delete_session(id, folder);
        self.refresh(agent_type, folder);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn read_without_refresh_is_empty_and_reads_no_disk() {
        let cache = HistoryCache::new();
        assert_eq!(cache.get("claude", "/nonexistent/folder"), Vec::new());
    }

    #[test]
    fn invalidate_drops_entry() {
        let cache = HistoryCache::new();
        // Seed an entry directly, bypassing disk I/O, to test invalidate in isolation.
        cache
            .entries
            .lock()
            .unwrap()
            .insert(("claude".to_owned(), "/proj".to_owned()), vec![]);
        cache.invalidate("claude", "/proj");
        assert!(
            !cache
                .entries
                .lock()
                .unwrap()
                .contains_key(&("claude".to_owned(), "/proj".to_owned()))
        );
    }

    #[test]
    fn refresh_unsupported_agent_type_is_noop() {
        let cache = HistoryCache::new();
        cache.refresh("shell", "/proj");
        assert_eq!(cache.get("shell", "/proj"), Vec::new());
    }

    #[test]
    fn delete_session_unsupported_agent_type_is_noop() {
        let cache = HistoryCache::new();
        cache.delete_session("shell", "id", "/proj");
        assert_eq!(cache.get("shell", "/proj"), Vec::new());
    }
}
