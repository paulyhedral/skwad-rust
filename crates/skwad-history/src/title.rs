//! Title extraction shared by every provider: reject Skwad-injected prompts
//! and non-conversational commands, take the first line, truncate to a
//! display length.

use crate::consts::{
    CLEAR_COMMAND, COMMAND_ARGS_CLOSE, COMMAND_ARGS_OPEN, COMMAND_NAME_CLOSE, COMMAND_NAME_OPEN,
    LOCAL_COMMAND_PREFIX, REGISTRATION_PROMPT_NEEDLES, TITLE_MAX_LEN, TITLE_TRUNCATE_LEN,
};

/// Whether `text` looks like a Skwad-injected agent-registration prompt.
pub fn is_registration_prompt(text: &str) -> bool {
    let lower = text.to_lowercase();
    REGISTRATION_PROMPT_NEEDLES
        .iter()
        .any(|needle| lower.contains(needle))
}

/// Whether `text` is a usable title candidate.
pub fn is_valid_title(text: &str) -> bool {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return false;
    }
    if is_registration_prompt(trimmed) {
        return false;
    }
    if trimmed.starts_with(LOCAL_COMMAND_PREFIX) {
        return false;
    }
    if trimmed == CLEAR_COMMAND {
        return false;
    }
    true
}

/// Truncate `text`'s first line to `TITLE_MAX_LEN` chars (display length),
/// keeping `TITLE_TRUNCATE_LEN` chars plus an ellipsis when it overflows.
pub fn truncate(text: &str) -> String {
    let first_line = text.lines().next().unwrap_or(text);
    if first_line.chars().count() > TITLE_MAX_LEN {
        let head: String = first_line.chars().take(TITLE_TRUNCATE_LEN).collect();
        format!("{head}...")
    } else {
        first_line.to_owned()
    }
}

/// Extract a display title from raw text: first line, trimmed, truncated.
/// `None` when the trimmed first line is empty.
pub fn extract_title(text: &str) -> Option<String> {
    let trimmed = text.trim();
    if trimmed.is_empty() {
        return None;
    }
    let first_line = trimmed.lines().next().unwrap_or(trimmed);
    if first_line.is_empty() {
        return None;
    }
    Some(truncate(first_line))
}

/// Expand a Claude slash-command wrapper like
/// `<command-name>/review</command-name><command-args>text</command-args>`
/// into `"/review text"`. `None` when the tags don't parse.
pub fn format_command_message(content: &str) -> Option<String> {
    let name_start = content.find(COMMAND_NAME_OPEN)? + COMMAND_NAME_OPEN.len();
    let name_end = content[name_start..].find(COMMAND_NAME_CLOSE)? + name_start;
    let command_name = content[name_start..name_end].trim();

    let args = content
        .find(COMMAND_ARGS_OPEN)
        .zip(content.find(COMMAND_ARGS_CLOSE))
        .and_then(|(args_start, args_end)| {
            let start = args_start + COMMAND_ARGS_OPEN.len();
            content.get(start..args_end).map(str::trim)
        })
        .unwrap_or("");

    if args.is_empty() {
        Some(command_name.to_owned())
    } else {
        Some(format!("{command_name} {args}"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_registration_prompts() {
        assert!(!is_valid_title(""));
        assert!(!is_valid_title("   "));
        assert!(!is_valid_title(
            "You are part of a team of agents working together."
        ));
        assert!(!is_valid_title(
            "<local-command-stdout>x</local-command-stdout>"
        ));
        assert!(!is_valid_title("/clear"));
    }

    #[test]
    fn accepts_normal_title() {
        assert!(is_valid_title("fix the login bug"));
    }

    #[test]
    fn truncate_boundary_at_80_chars() {
        let exactly_80 = "a".repeat(80);
        assert_eq!(truncate(&exactly_80), exactly_80);

        let over_80 = "a".repeat(81);
        let truncated = truncate(&over_80);
        assert_eq!(truncated.chars().count(), 80);
        assert!(truncated.ends_with("..."));
        assert_eq!(&truncated[..77], "a".repeat(77));
    }

    #[test]
    fn extract_title_takes_first_line() {
        assert_eq!(
            extract_title("first line\nsecond line").as_deref(),
            Some("first line")
        );
        assert_eq!(extract_title("   \n  "), None);
    }

    #[test]
    fn format_command_message_name_only() {
        let content = "<command-name>/review</command-name><command-args></command-args>";
        assert_eq!(format_command_message(content).as_deref(), Some("/review"));
    }

    #[test]
    fn format_command_message_name_and_args() {
        let content =
            "<command-name>/review</command-name><command-args>please check</command-args>";
        assert_eq!(
            format_command_message(content).as_deref(),
            Some("/review please check")
        );
    }

    #[test]
    fn format_command_message_malformed_is_none() {
        assert_eq!(format_command_message("no tags here"), None);
        assert_eq!(format_command_message("<command-name>unterminated"), None);
    }
}
