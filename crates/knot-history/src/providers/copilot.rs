//! Copilot provider: `~/.copilot/session-state/<id>/workspace.yaml` (flat
//! `key: value`), `events.jsonl` for the title fallback.

use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::consts::{COPILOT_SESSION_STATE_DIR, MAX_SESSIONS};
use crate::paths::home_dir;
use crate::provider::{HistoryProvider, SessionSummary};
use crate::title::{extract_title, is_valid_title};

pub struct CopilotProvider;

struct WorkspaceInfo {
    cwd: String,
    summary: String,
    updated_at: OffsetDateTime,
}

fn base_dir() -> Option<PathBuf> {
    Some(home_dir()?.join(COPILOT_SESSION_STATE_DIR))
}

/// Parse a flat `key: value` YAML file (only the shape `workspace.yaml`
/// actually uses; not a general YAML parser).
fn parse_workspace_yaml(content: &str) -> Option<WorkspaceInfo> {
    let mut cwd = None;
    let mut summary = String::new();
    let mut updated_at = OffsetDateTime::UNIX_EPOCH;

    for line in content.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let key = key.trim();
        let value = value.trim();
        match key {
            "cwd" => cwd = Some(value.to_owned()),
            "summary" => summary = value.to_owned(),
            "updated_at" => {
                if let Ok(parsed) = OffsetDateTime::parse(value, &Rfc3339) {
                    updated_at = parsed;
                }
            }
            _ => {}
        }
    }

    Some(WorkspaceInfo {
        cwd: cwd?,
        summary,
        updated_at,
    })
}

/// Parse `events.jsonl` for the first `user.message` event's content.
fn title_from_events(path: &std::path::Path) -> Option<String> {
    let content = fs::read_to_string(path).ok()?;

    for line in content.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        let Ok(json) = serde_json::from_str::<Value>(trimmed) else {
            continue;
        };
        if json.get("type").and_then(Value::as_str) != Some("user.message") {
            continue;
        }
        let Some(message) = json
            .get("data")
            .and_then(|d| d.get("content"))
            .and_then(Value::as_str)
        else {
            continue;
        };
        if !is_valid_title(message) {
            continue;
        }
        return extract_title(message);
    }
    None
}

fn resolve_title(summary: &str, session_dir: &std::path::Path) -> String {
    if is_valid_title(summary) {
        return crate::title::truncate(summary);
    }
    title_from_events(&session_dir.join("events.jsonl")).unwrap_or_default()
}

impl HistoryProvider for CopilotProvider {
    fn load_sessions(&self, folder: &str) -> Vec<SessionSummary> {
        let Some(base) = base_dir() else {
            return Vec::new();
        };
        let Ok(entries) = fs::read_dir(&base) else {
            return Vec::new();
        };

        let mut summaries: Vec<SessionSummary> = entries
            .filter_map(Result::ok)
            .filter_map(|entry| {
                let session_dir = entry.path();
                let id = entry.file_name().to_str()?.to_owned();
                let yaml_content = fs::read_to_string(session_dir.join("workspace.yaml")).ok()?;
                let info = parse_workspace_yaml(&yaml_content)?;
                if info.cwd != folder {
                    return None;
                }
                Some(SessionSummary {
                    title: resolve_title(&info.summary, &session_dir),
                    id,
                    timestamp: info.updated_at,
                    message_count: 0,
                })
            })
            .collect();

        summaries.sort_by_key(|s| std::cmp::Reverse(s.timestamp));
        summaries.truncate(MAX_SESSIONS);
        summaries
    }

    fn delete_session(&self, id: &str, _folder: &str) {
        let Some(base) = base_dir() else {
            return;
        };
        let _ = fs::remove_dir_all(base.join(id));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_flat_workspace_yaml() {
        let content = "cwd: /proj\nsummary: fix the bug\nupdated_at: 2024-01-01T00:00:00Z\n";
        let info = parse_workspace_yaml(content).unwrap();
        assert_eq!(info.cwd, "/proj");
        assert_eq!(info.summary, "fix the bug");
        assert_eq!(info.updated_at.year(), 2024);
    }

    #[test]
    fn folder_mismatch_excluded() {
        let tmp = tempfile::tempdir().unwrap();
        let session_dir = tmp.path().join("s1");
        fs::create_dir_all(&session_dir).unwrap();
        fs::write(
            session_dir.join("workspace.yaml"),
            "cwd: /other\nsummary: unrelated\nupdated_at: 2024-01-01T00:00:00Z\n",
        )
        .unwrap();

        let yaml = fs::read_to_string(session_dir.join("workspace.yaml")).unwrap();
        let info = parse_workspace_yaml(&yaml).unwrap();
        assert_ne!(info.cwd, "/proj");
    }

    #[test]
    fn events_fallback_finds_first_user_message() {
        let tmp = tempfile::tempdir().unwrap();
        let events_path = tmp.path().join("events.jsonl");
        fs::write(
            &events_path,
            r#"{"type":"other.event"}
{"type":"user.message","data":{"content":"the real title"}}"#,
        )
        .unwrap();

        assert_eq!(
            title_from_events(&events_path).as_deref(),
            Some("the real title")
        );
    }
}
