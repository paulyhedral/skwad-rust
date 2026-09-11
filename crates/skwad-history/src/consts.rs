pub const CLAUDE_PROJECTS_DIR: &str = ".claude/projects";
pub const CODEX_DB_PATH: &str = ".codex/state_5.sqlite";
pub const GEMINI_TMP_DIR: &str = ".gemini/tmp";
pub const COPILOT_SESSION_STATE_DIR: &str = ".copilot/session-state";

/// Recency cap applied by every provider.
pub const MAX_SESSIONS: usize = 20;

/// Title length limit: strings longer than this are truncated to
/// `TITLE_TRUNCATE_LEN` chars plus an ellipsis.
pub const TITLE_MAX_LEN: usize = 80;
pub const TITLE_TRUNCATE_LEN: usize = 77;

/// Substrings that mark a message as a Skwad-injected registration prompt,
/// never a real session title.
pub const REGISTRATION_PROMPT_NEEDLES: &[&str] = &[
    "you are part of a team of agents",
    "register with the skwad",
    "list other agents names and project",
    "check your inbox for messages",
];

pub const LOCAL_COMMAND_PREFIX: &str = "<local-command-";
pub const CLEAR_COMMAND: &str = "/clear";

pub const COMMAND_NAME_OPEN: &str = "<command-name>";
pub const COMMAND_NAME_CLOSE: &str = "</command-name>";
pub const COMMAND_ARGS_OPEN: &str = "<command-args>";
pub const COMMAND_ARGS_CLOSE: &str = "</command-args>";
