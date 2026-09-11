use serde_json::Value;
use skwad_mcp::ToolCallResult;

fn missing(key: &str) -> ToolCallResult {
    ToolCallResult::error(format!("Missing required parameter: {key}"))
}

/// A required string argument. Missing or wrong-typed both report the same
/// "Missing required parameter" text - the spec only distinguishes present
/// vs absent, not present-but-wrong-type.
pub fn require_str<'a>(args: &'a Value, key: &str) -> Result<&'a str, ToolCallResult> {
    args.get(key)
        .and_then(Value::as_str)
        .ok_or_else(|| missing(key))
}

pub fn optional_str<'a>(args: &'a Value, key: &str) -> Option<&'a str> {
    args.get(key).and_then(Value::as_str)
}

pub fn optional_bool(args: &Value, key: &str) -> Option<bool> {
    args.get(key).and_then(Value::as_bool)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn require_str_returns_value_when_present() {
        let args = json!({"agentId": "abc"});
        assert_eq!(require_str(&args, "agentId").unwrap(), "abc");
    }

    #[test]
    fn require_str_errors_when_missing() {
        let args = json!({});
        let err = require_str(&args, "agentId").unwrap_err();
        assert_eq!(err.is_error, Some(true));
        assert!(err.content[0].text.contains("agentId"));
    }

    #[test]
    fn require_str_errors_when_wrong_type() {
        let args = json!({"agentId": 42});
        let err = require_str(&args, "agentId").unwrap_err();
        assert_eq!(err.is_error, Some(true));
        assert!(err.content[0].text.contains("agentId"));
    }

    #[test]
    fn optional_str_returns_none_when_absent() {
        let args = json!({});
        assert_eq!(optional_str(&args, "sessionId"), None);
    }

    #[test]
    fn optional_bool_returns_value_when_present() {
        let args = json!({"markAsRead": false});
        assert_eq!(optional_bool(&args, "markAsRead"), Some(false));
    }

    #[test]
    fn optional_bool_returns_none_when_wrong_type() {
        let args = json!({"markAsRead": "false"});
        assert_eq!(optional_bool(&args, "markAsRead"), None);
    }
}
