//! Claude provider: `*.jsonl` transcripts under
//! `~/.claude/projects/<dashed-folder>`.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use serde_json::Value;
use time::OffsetDateTime;

use crate::consts::{CLAUDE_PROJECTS_DIR, MAX_SESSIONS};
use crate::paths::home_dir;
use crate::provider::{HistoryProvider, SessionSummary};
use crate::title::{extract_title, format_command_message, is_valid_title};

pub struct ClaudeProvider;

/// `~/.claude/projects/<folder with '/' -> '-'>`, e.g.
/// `/Users/x/src/app` -> `~/.claude/projects/-Users-x-src-app`.
fn dashed_dir(folder: &str) -> Option<PathBuf> {
    let dashed = folder.replace('/', "-");
    Some(home_dir()?.join(CLAUDE_PROJECTS_DIR).join(dashed))
}

fn system_time_to_offset(time: SystemTime) -> OffsetDateTime {
    OffsetDateTime::from(time)
}

struct ParsedSession {
    title: String,
    message_count: usize,
}

/// Parse one `.jsonl` transcript: count `user`/`assistant` entries and take
/// the title from the first non-meta `user` message.
fn parse_session_file(path: &Path) -> Option<ParsedSession> {
    let content = fs::read_to_string(path).ok()?;

    let mut title: Option<String> = None;
    let mut message_count = 0usize;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(json) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        let Some(entry_type) = json.get("type").and_then(Value::as_str) else {
            continue;
        };

        if entry_type == "user" || entry_type == "assistant" {
            message_count += 1;
        }

        if title.is_none() && entry_type == "user" {
            if json.get("isMeta").and_then(Value::as_bool) == Some(true) {
                continue;
            }
            let Some(message_content) = json
                .get("message")
                .and_then(|m| m.get("content"))
                .and_then(Value::as_str)
            else {
                continue;
            };

            let cleaned = if message_content.contains("<command-name>") {
                match format_command_message(message_content) {
                    Some(cleaned) if !cleaned.is_empty() => cleaned,
                    _ => continue,
                }
            } else {
                message_content.to_owned()
            };

            if !is_valid_title(&cleaned) {
                continue;
            }
            title = extract_title(&cleaned);
        }
    }

    let title = title?;
    if message_count == 0 {
        return None;
    }
    Some(ParsedSession {
        title,
        message_count,
    })
}

impl HistoryProvider for ClaudeProvider {
    fn load_sessions(&self, folder: &str) -> Vec<SessionSummary> {
        let Some(dir) = dashed_dir(folder) else {
            return Vec::new();
        };
        let Ok(entries) = fs::read_dir(&dir) else {
            return Vec::new();
        };

        let mut files: Vec<(PathBuf, String, SystemTime)> = entries
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                let path = entry.path();
                let stem = path.file_stem()?.to_str()?.to_owned();
                if path.extension().and_then(|ext| ext.to_str()) != Some("jsonl") {
                    return None;
                }
                let modified = entry.metadata().ok()?.modified().ok()?;
                Some((path, stem, modified))
            })
            .collect();
        files.sort_by_key(|(_, _, modified)| std::cmp::Reverse(*modified));

        let mut summaries = Vec::new();
        for (index, (path, session_id, modified)) in files.into_iter().enumerate() {
            if summaries.len() >= MAX_SESSIONS {
                break;
            }
            let timestamp = system_time_to_offset(modified);
            match parse_session_file(&path) {
                Some(parsed) => summaries.push(SessionSummary {
                    id: session_id,
                    title: parsed.title,
                    timestamp,
                    message_count: parsed.message_count,
                }),
                None if index == 0 => summaries.push(SessionSummary {
                    id: session_id,
                    title: String::new(),
                    timestamp,
                    message_count: 0,
                }),
                None => {}
            }
        }
        summaries
    }

    fn delete_session(&self, id: &str, folder: &str) {
        let Some(dir) = dashed_dir(folder) else {
            return;
        };
        let _ = fs::remove_file(dir.join(format!("{id}.jsonl")));
        let _ = fs::remove_file(dir.join(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dashed_dir_replaces_separators() {
        let home = home_dir().expect("home dir resolvable in test env");
        let dir = dashed_dir("/Users/x/src/app").expect("dir");
        assert_eq!(dir, home.join(CLAUDE_PROJECTS_DIR).join("-Users-x-src-app"));
    }

    fn write_session(dir: &Path, id: &str, lines: &[&str]) {
        fs::write(dir.join(format!("{id}.jsonl")), lines.join("\n")).unwrap();
    }

    #[test]
    fn parses_normal_session_title_and_count() {
        let tmp = tempfile::tempdir().unwrap();
        write_session(
            tmp.path(),
            "abc",
            &[
                r#"{"type":"user","message":{"content":"fix the login bug"}}"#,
                r#"{"type":"assistant","message":{"content":"done"}}"#,
            ],
        );

        let parsed = parse_session_file(&tmp.path().join("abc.jsonl")).unwrap();
        assert_eq!(parsed.title, "fix the login bug");
        assert_eq!(parsed.message_count, 2);
    }

    #[test]
    fn command_only_first_message_expands() {
        let tmp = tempfile::tempdir().unwrap();
        write_session(
            tmp.path(),
            "abc",
            &[
                r#"{"type":"user","message":{"content":"<command-name>/review</command-name><command-args>please check</command-args>"}}"#,
                r#"{"type":"assistant","message":{"content":"ok"}}"#,
            ],
        );

        let parsed = parse_session_file(&tmp.path().join("abc.jsonl")).unwrap();
        assert_eq!(parsed.title, "/review please check");
    }

    #[test]
    fn unparseable_session_returns_none() {
        let tmp = tempfile::tempdir().unwrap();
        write_session(tmp.path(), "abc", &["not json at all"]);

        assert!(parse_session_file(&tmp.path().join("abc.jsonl")).is_none());
    }
}
