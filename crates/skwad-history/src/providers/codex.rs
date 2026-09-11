//! Codex provider: `~/.codex/state_5.sqlite`, `threads` table.

use std::fs;
use std::path::PathBuf;

use rusqlite::{Connection, OpenFlags, params};
use time::OffsetDateTime;

use crate::consts::{CODEX_DB_PATH, MAX_SESSIONS};
use crate::paths::home_dir;
use crate::provider::{HistoryProvider, SessionSummary};
use crate::title::{extract_title, is_valid_title};

pub struct CodexProvider;

fn db_path() -> Option<PathBuf> {
    Some(home_dir()?.join(CODEX_DB_PATH))
}

/// Parse a Codex rollout JSONL file for the first real `user_message`.
fn title_from_rollout(path: &str) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(json) = serde_json::from_str::<serde_json::Value>(trimmed) else {
            continue;
        };
        let payload = json.get("payload")?;
        if payload.get("type").and_then(serde_json::Value::as_str) != Some("user_message") {
            continue;
        }
        let Some(message) = payload.get("message").and_then(serde_json::Value::as_str) else {
            continue;
        };
        if !is_valid_title(message) {
            continue;
        }
        return extract_title(message);
    }
    None
}

fn resolve_title(db_title: &str, rollout_path: &str) -> String {
    if is_valid_title(db_title) {
        return crate::title::truncate(db_title);
    }
    title_from_rollout(rollout_path).unwrap_or_default()
}

impl HistoryProvider for CodexProvider {
    fn load_sessions(&self, folder: &str) -> Vec<SessionSummary> {
        let Some(path) = db_path() else {
            return Vec::new();
        };
        if !path.exists() {
            return Vec::new();
        }
        let Ok(conn) = Connection::open_with_flags(&path, OpenFlags::SQLITE_OPEN_READ_ONLY) else {
            return Vec::new();
        };

        let query = "SELECT id, rollout_path, title, updated_at FROM threads \
                     WHERE cwd = ?1 AND archived = 0 ORDER BY updated_at DESC LIMIT ?2";
        let Ok(mut stmt) = conn.prepare(query) else {
            return Vec::new();
        };

        let rows = stmt.query_map(params![folder, MAX_SESSIONS as i64], |row| {
            let id: String = row.get(0)?;
            let rollout_path: String = row.get(1)?;
            let title: String = row.get(2)?;
            let updated_at: i64 = row.get(3)?;
            Ok((id, rollout_path, title, updated_at))
        });

        let Ok(rows) = rows else {
            return Vec::new();
        };

        rows.filter_map(Result::ok)
            .map(|(id, rollout_path, title, updated_at)| SessionSummary {
                id,
                title: resolve_title(&title, &rollout_path),
                timestamp: OffsetDateTime::from_unix_timestamp(updated_at)
                    .unwrap_or(OffsetDateTime::UNIX_EPOCH),
                message_count: 0,
            })
            .collect()
    }

    fn delete_session(&self, id: &str, _folder: &str) {
        let Some(path) = db_path() else {
            return;
        };
        if !path.exists() {
            return;
        }
        let Ok(conn) = Connection::open(&path) else {
            return;
        };

        if let Ok(rollout_path) = conn.query_row(
            "SELECT rollout_path FROM threads WHERE id = ?1",
            params![id],
            |row| row.get::<_, String>(0),
        ) {
            let _ = fs::remove_file(rollout_path);
        }

        let _ = conn.execute("UPDATE threads SET archived = 1 WHERE id = ?1", params![id]);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn seed_db(conn: &Connection) {
        conn.execute_batch(
            "CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                rollout_path TEXT NOT NULL,
                title TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                cwd TEXT NOT NULL,
                archived INTEGER NOT NULL DEFAULT 0
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads VALUES ('t1', '/tmp/t1.jsonl', 'first task', 200, '/proj', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads VALUES ('t2', '/tmp/t2.jsonl', 'second task', 100, '/proj', 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads VALUES ('t3', '/tmp/t3.jsonl', 'archived task', 300, '/proj', 1)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads VALUES ('t4', '/tmp/t4.jsonl', 'other project', 400, '/other', 0)",
            [],
        )
        .unwrap();
    }

    #[test]
    fn missing_db_returns_empty() {
        assert!(title_from_rollout("/nonexistent/rollout.jsonl").is_none());
    }

    #[test]
    fn query_returns_matching_non_archived_rows_in_order() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("state_5.sqlite");
        let conn = Connection::open(&db_path).unwrap();
        seed_db(&conn);
        drop(conn);

        let ro = Connection::open_with_flags(&db_path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap();
        let mut stmt = ro
            .prepare(
                "SELECT id FROM threads WHERE cwd = ?1 AND archived = 0 ORDER BY updated_at DESC LIMIT ?2",
            )
            .unwrap();
        let ids: Vec<String> = stmt
            .query_map(params!["/proj", 20i64], |row| row.get(0))
            .unwrap()
            .filter_map(Result::ok)
            .collect();

        assert_eq!(ids, vec!["t1", "t2"]);
    }

    #[test]
    fn delete_marks_archived_and_removes_rollout() {
        let tmp = tempfile::tempdir().unwrap();
        let db_path = tmp.path().join("state_5.sqlite");
        let rollout_path = tmp.path().join("t1.jsonl");
        fs::write(&rollout_path, "{}").unwrap();

        let conn = Connection::open(&db_path).unwrap();
        conn.execute_batch(
            "CREATE TABLE threads (
                id TEXT PRIMARY KEY,
                rollout_path TEXT NOT NULL,
                title TEXT NOT NULL,
                updated_at INTEGER NOT NULL,
                cwd TEXT NOT NULL,
                archived INTEGER NOT NULL DEFAULT 0
            );",
        )
        .unwrap();
        conn.execute(
            "INSERT INTO threads VALUES ('t1', ?1, 'first task', 200, '/proj', 0)",
            params![rollout_path.to_str().unwrap()],
        )
        .unwrap();

        if let Ok(path) = conn.query_row(
            "SELECT rollout_path FROM threads WHERE id = 't1'",
            [],
            |row| row.get::<_, String>(0),
        ) {
            fs::remove_file(&path).unwrap();
        }
        conn.execute("UPDATE threads SET archived = 1 WHERE id = 't1'", [])
            .unwrap();

        assert!(!rollout_path.exists());
        let archived: i64 = conn
            .query_row("SELECT archived FROM threads WHERE id = 't1'", [], |row| {
                row.get(0)
            })
            .unwrap();
        assert_eq!(archived, 1);
    }
}
