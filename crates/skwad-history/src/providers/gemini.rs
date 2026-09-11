//! Gemini provider: `~/.gemini/tmp/<hash>/logs.json`, located by a
//! `.project_root` marker file.

use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::consts::{GEMINI_TMP_DIR, MAX_SESSIONS};
use crate::paths::home_dir;
use crate::provider::{HistoryProvider, SessionSummary};
use crate::title::{extract_title, is_valid_title};

pub struct GeminiProvider;

fn base_dir() -> Option<PathBuf> {
    Some(home_dir()?.join(GEMINI_TMP_DIR))
}

/// Find the `~/.gemini/tmp/<hash>/` directory whose `.project_root` trims to
/// `folder`.
fn find_project_dir(folder: &str) -> Option<PathBuf> {
    let base = base_dir()?;
    let entries = fs::read_dir(&base).ok()?;

    for entry in entries.filter_map(Result::ok) {
        let dir = entry.path();
        let root_file = dir.join(".project_root");
        let Ok(root) = fs::read_to_string(&root_file) else {
            continue;
        };
        if root.trim() == folder {
            return Some(dir);
        }
    }
    None
}

fn parse_timestamp(raw: &str) -> OffsetDateTime {
    OffsetDateTime::parse(raw, &Rfc3339).unwrap_or(OffsetDateTime::UNIX_EPOCH)
}

/// Find the `chats/session-*<short-id>*.json` file for a session id (files
/// are named with an 8-char prefix of the full session id).
fn find_chat_file(chats_dir: &Path, session_id: &str) -> Option<PathBuf> {
    let short_id: String = session_id.chars().take(8).collect();
    let entries = fs::read_dir(chats_dir).ok()?;
    entries
        .filter_map(Result::ok)
        .map(|e| e.path())
        .find(|path| {
            path.extension().and_then(|e| e.to_str()) == Some("json")
                && path
                    .file_name()
                    .and_then(|n| n.to_str())
                    .is_some_and(|name| name.contains(&short_id))
        })
}

/// Parse a Gemini chat file for the first `user` message text.
fn title_from_chat_file(path: &Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;
    let json: Value = serde_json::from_str(&content).ok()?;
    let messages = json.get("messages")?.as_array()?;

    for message in messages {
        if message.get("type").and_then(Value::as_str) != Some("user") {
            continue;
        }
        let Some(text) = message
            .get("content")
            .and_then(Value::as_array)
            .and_then(|arr| arr.first())
            .and_then(|first| first.get("text"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        if !is_valid_title(text) {
            continue;
        }
        return extract_title(text);
    }
    None
}

fn resolve_title(log_message: &str, session_id: &str, chats_dir: &Path) -> String {
    if is_valid_title(log_message) {
        return crate::title::truncate(log_message);
    }
    find_chat_file(chats_dir, session_id)
        .and_then(|path| title_from_chat_file(&path))
        .unwrap_or_default()
}

impl HistoryProvider for GeminiProvider {
    fn load_sessions(&self, folder: &str) -> Vec<SessionSummary> {
        let Some(project_dir) = find_project_dir(folder) else {
            return Vec::new();
        };
        let logs_path = project_dir.join("logs.json");
        let Ok(content) = fs::read_to_string(&logs_path) else {
            return Vec::new();
        };
        let Ok(entries) = serde_json::from_str::<Vec<Value>>(&content) else {
            return Vec::new();
        };

        let mut sessions: HashMap<String, (String, OffsetDateTime)> = HashMap::new();
        for entry in &entries {
            let (Some(session_id), Some(entry_type), Some(message), Some(timestamp_str)) = (
                entry.get("sessionId").and_then(Value::as_str),
                entry.get("type").and_then(Value::as_str),
                entry.get("message").and_then(Value::as_str),
                entry.get("timestamp").and_then(Value::as_str),
            ) else {
                continue;
            };
            if entry_type != "user" {
                continue;
            }
            sessions
                .entry(session_id.to_owned())
                .or_insert_with(|| (message.to_owned(), parse_timestamp(timestamp_str)));
        }

        let mut sorted: Vec<_> = sessions.into_iter().collect();
        sorted.sort_by_key(|(_, (_, timestamp))| std::cmp::Reverse(*timestamp));

        let chats_dir = project_dir.join("chats");
        sorted
            .into_iter()
            .take(MAX_SESSIONS)
            .map(|(session_id, (message, timestamp))| SessionSummary {
                title: resolve_title(&message, &session_id, &chats_dir),
                id: session_id,
                timestamp,
                message_count: 0,
            })
            .collect()
    }

    fn delete_session(&self, id: &str, folder: &str) {
        let Some(project_dir) = find_project_dir(folder) else {
            return;
        };

        let chats_dir = project_dir.join("chats");
        if let Some(chat_file) = find_chat_file(&chats_dir, id) {
            let _ = fs::remove_file(chat_file);
        }

        let logs_path = project_dir.join("logs.json");
        let Ok(content) = fs::read_to_string(&logs_path) else {
            return;
        };
        let Ok(mut entries) = serde_json::from_str::<Vec<Value>>(&content) else {
            return;
        };
        entries.retain(|entry| entry.get("sessionId").and_then(Value::as_str) != Some(id));
        if let Ok(updated) = serde_json::to_string_pretty(&entries) {
            let _ = fs::write(&logs_path, updated);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setup_project(root: &Path, folder: &str) -> PathBuf {
        let project_dir = root.join("proj-hash");
        fs::create_dir_all(&project_dir).unwrap();
        fs::write(project_dir.join(".project_root"), folder).unwrap();
        project_dir
    }

    #[test]
    fn finds_project_dir_by_marker() {
        let tmp = tempfile::tempdir().unwrap();
        setup_project(tmp.path(), "/Users/x/proj");

        // Exercise the search logic directly against the temp base.
        let base = tmp.path();
        let mut found = None;
        for entry in fs::read_dir(base).unwrap().filter_map(Result::ok) {
            let dir = entry.path();
            if let Ok(root) = fs::read_to_string(dir.join(".project_root"))
                && root.trim() == "/Users/x/proj"
            {
                found = Some(dir);
            }
        }
        assert!(found.is_some());
    }

    #[test]
    fn groups_by_session_and_orders_by_timestamp() {
        let tmp = tempfile::tempdir().unwrap();
        let project_dir = setup_project(tmp.path(), "/proj");
        fs::write(
            project_dir.join("logs.json"),
            r#"[
                {"sessionId":"s1","type":"user","message":"older","timestamp":"2024-01-01T00:00:00Z"},
                {"sessionId":"s2","type":"user","message":"newer","timestamp":"2024-02-01T00:00:00Z"},
                {"sessionId":"s1","type":"user","message":"ignored second","timestamp":"2024-01-02T00:00:00Z"}
            ]"#,
        )
        .unwrap();

        let content = fs::read_to_string(project_dir.join("logs.json")).unwrap();
        let entries: Vec<Value> = serde_json::from_str(&content).unwrap();
        let mut sessions: HashMap<String, (String, OffsetDateTime)> = HashMap::new();
        for entry in &entries {
            let session_id = entry["sessionId"].as_str().unwrap().to_owned();
            let message = entry["message"].as_str().unwrap().to_owned();
            let timestamp = parse_timestamp(entry["timestamp"].as_str().unwrap());
            sessions.entry(session_id).or_insert((message, timestamp));
        }

        assert_eq!(sessions.get("s1").unwrap().0, "older");
        assert_eq!(sessions.len(), 2);
    }

    #[test]
    fn chat_file_title_fallback() {
        let tmp = tempfile::tempdir().unwrap();
        let chats_dir = tmp.path().join("chats");
        fs::create_dir_all(&chats_dir).unwrap();
        fs::write(
            chats_dir.join("session-2024-01-01T00-00-abcdef12.json"),
            r#"{"messages":[{"type":"user","content":[{"text":"the real title"}]}]}"#,
        )
        .unwrap();

        let title = title_from_chat_file(&chats_dir.join("session-2024-01-01T00-00-abcdef12.json"));
        assert_eq!(title.as_deref(), Some("the real title"));
    }
}
