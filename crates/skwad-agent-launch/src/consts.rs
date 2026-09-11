//! Fixed strings the launch-command builder assembles into agent commands:
//! registration prompts, skwad system instructions, and Copilot's allow-list
//! of messaging tools. Kept in one place per the workspace's constants
//! convention.

/// Sent to Claude (and combined into other types' registration prompt) on
/// first launch, to trigger the agent list table and status set.
pub const REGISTRATION_USER_PROMPT: &str = "List other agents names and project (no ID) in a table based on context then set your status to indicate you are ready to get going. If you don't see yourself in the table, register with the skwad.";

/// Messaging tools Copilot's `--allow-tool` flags are generated for.
pub const COPILOT_ALLOWED_TOOLS: &[&str] = &[
    "register-agent",
    "list-agents",
    "send-message",
    "check-messages",
    "broadcast-message",
];
