use std::collections::BTreeMap;
use std::fs::File;
use std::io::{BufRead, BufReader};
use std::path::Path;

use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

const REGISTRATION_PROMPT: &str = "List other agents names and project (no ID) in a table based on context then set your status to indicate you are ready to get going. If you don't see yourself in the table, register with the skwad.";

#[derive(Debug, Clone, Deserialize)]
pub struct HookRequest {
    pub agent_id: String,
    #[serde(default = "default_agent")]
    pub agent: String,
    pub session_id: Option<String>,
    pub source: Option<String>,
    pub hook: Option<String>,
    pub status: Option<String>,
    #[serde(default)]
    pub payload: Value,
}

pub trait AgentHookHandler: Send + Sync {
    fn register(&self, request: &HookRequest) -> Result<Value, HookError>;

    fn status(&self, request: &HookRequest) -> Result<Value, HookError>;
}

impl HookRequest {
    pub fn agent_id(&self) -> Result<Uuid, HookError> {
        Uuid::parse_str(&self.agent_id).map_err(|_| HookError::InvalidAgentId)
    }

    pub fn validate_agent(&self) -> Result<(), HookError> {
        match self.agent.as_str() {
            "claude" | "codex" => Ok(()),
            agent => Err(HookError::UnknownAgent(agent.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HookError {
    InvalidAgentId,
    UnknownAgent(String),
    InvalidStatus(String),
    InvalidPayload(String),
}

impl std::fmt::Display for HookError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidAgentId => f.write_str("Invalid agent_id"),
            Self::UnknownAgent(agent) => write!(f, "Unknown agent type: {agent}"),
            Self::InvalidStatus(status) => write!(f, "Invalid status: {status}"),
            Self::InvalidPayload(message) => f.write_str(message),
        }
    }
}

impl std::error::Error for HookError {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HookStatus {
    Working,
    Idle,
    AwaitingInput,
}

pub fn claude_status(status: &str) -> Result<HookStatus, HookError> {
    match status {
        "running" => Ok(HookStatus::Working),
        "idle" => Ok(HookStatus::Idle),
        "input" => Ok(HookStatus::AwaitingInput),
        status => Err(HookError::InvalidStatus(status.to_string())),
    }
}

pub fn extract_metadata(agent: &str, payload: &Value) -> BTreeMap<String, String> {
    let keys: &[&str] = match agent {
        "codex" => &["cwd", "thread-id", "turn-id"],
        _ => &["transcript_path", "cwd", "model", "session_id"],
    };

    keys.iter()
        .filter_map(|key| {
            payload
                .get(*key)
                .and_then(Value::as_str)
                .filter(|value| !value.is_empty())
                .map(|value| ((*key).to_string(), value.to_string()))
        })
        .collect()
}

pub fn codex_turn_complete(payload: &Value) -> Result<Option<String>, HookError> {
    if payload.get("type").and_then(Value::as_str) != Some("agent-turn-complete") {
        return Err(HookError::InvalidPayload(
            "Unsupported Codex hook event".to_string(),
        ));
    }

    Ok(payload
        .get("thread-id")
        .and_then(Value::as_str)
        .filter(|thread_id| !thread_id.is_empty())
        .map(str::to_string))
}

pub fn last_assistant_message_from_transcript(path: impl AsRef<Path>) -> Option<String> {
    let file = File::open(path).ok()?;
    let lines: Vec<String> = BufReader::new(file).lines().map_while(Result::ok).collect();

    for (index, line) in lines.iter().enumerate().rev() {
        let Ok(entry) = serde_json::from_str::<Value>(line) else {
            continue;
        };
        if entry.get("type").and_then(Value::as_str) != Some("assistant") {
            continue;
        }

        let text = message_text(entry.get("message")?.get("content")?)?;
        if text.is_empty() {
            continue;
        }

        let previous_user = lines[..index].iter().rev().find_map(|line| {
            let entry: Value = serde_json::from_str(line).ok()?;
            (entry.get("type").and_then(Value::as_str) == Some("user"))
                .then(|| message_text(entry.get("message")?.get("content")?))?
        });
        if previous_user.as_deref() == Some(REGISTRATION_PROMPT) {
            return Some(String::new());
        }
        return Some(text);
    }
    None
}

fn message_text(content: &Value) -> Option<String> {
    if let Some(text) = content.as_str() {
        return Some(text.to_string());
    }

    let parts = content.as_array()?;
    let text = parts
        .iter()
        .filter_map(|part| part.get("text").and_then(Value::as_str))
        .collect::<String>();
    Some(text)
}

fn default_agent() -> String {
    "claude".to_string()
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use serde_json::json;

    #[test]
    fn request_defaults_to_claude_and_validates_uuid() {
        let id = Uuid::new_v4();
        let request: HookRequest = serde_json::from_value(json!({"agent_id": id})).unwrap();

        assert_eq!(request.agent, "claude");
        assert_eq!(request.agent_id().unwrap(), id);
        assert!(request.validate_agent().is_ok());
    }

    #[test]
    fn unknown_agent_is_rejected() {
        let request: HookRequest = serde_json::from_value(json!({
            "agent_id": Uuid::new_v4(),
            "agent": "gemini"
        }))
        .unwrap();

        assert_eq!(
            request.validate_agent(),
            Err(HookError::UnknownAgent("gemini".to_string()))
        );
    }

    #[test]
    fn status_and_metadata_follow_agent_contract() {
        assert_eq!(claude_status("running"), Ok(HookStatus::Working));
        assert_eq!(claude_status("input"), Ok(HookStatus::AwaitingInput));
        assert!(claude_status("done").is_err());

        let metadata = extract_metadata(
            "codex",
            &json!({"cwd": "/tmp/project", "thread-id": "thread-1", "model": "ignored", "turn-id": ""}),
        );
        assert_eq!(metadata.get("cwd"), Some(&"/tmp/project".to_string()));
        assert_eq!(metadata.get("thread-id"), Some(&"thread-1".to_string()));
        assert!(!metadata.contains_key("model"));
        assert!(!metadata.contains_key("turn-id"));

        assert_eq!(
            codex_turn_complete(&json!({"type": "agent-turn-complete", "thread-id": "t1"})),
            Ok(Some("t1".to_string()))
        );
        assert!(codex_turn_complete(&json!({"type": "notify"})).is_err());
    }

    #[test]
    fn transcript_returns_last_assistant_text_and_supports_parts() {
        let path = std::env::temp_dir().join(format!("skwad-transcript-{}.jsonl", Uuid::new_v4()));
        let content = [
            json!({"type":"user","message":{"content":"Do the work"}}),
            json!({"type":"assistant","message":{"content":[{"type":"text","text":"First"}]}}),
            json!({"type":"assistant","message":{"content":"Last"}}),
        ]
        .into_iter()
        .map(|entry| entry.to_string())
        .collect::<Vec<_>>()
        .join("\n");
        fs::write(&path, content).unwrap();

        assert_eq!(
            last_assistant_message_from_transcript(&path),
            Some("Last".to_string())
        );
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn transcript_suppresses_registration_reply() {
        let path = std::env::temp_dir().join(format!("skwad-transcript-{}.jsonl", Uuid::new_v4()));
        let content = format!(
            "{}\n{}",
            json!({"type":"user","message":{"content":REGISTRATION_PROMPT}}),
            json!({"type":"assistant","message":{"content":"Registered"}}),
        );
        fs::write(&path, content).unwrap();

        assert_eq!(
            last_assistant_message_from_transcript(&path),
            Some(String::new())
        );
        fs::remove_file(path).unwrap();
    }
}
