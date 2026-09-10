use std::time::Duration;

pub const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

pub const VERSION: &[&str] = &["--version"];

pub const STATUS: &[&str] = &["status", "--porcelain=v2", "--branch"];

pub const DIFF: &[&str] = &["diff", "--no-color"];
pub const DIFF_STAGED_FLAG: &str = "--staged";

pub const NUMSTAT: &[&str] = &["diff", "--numstat"];
pub const NUMSTAT_STAGED: &[&str] = &["diff", "--staged", "--numstat"];

pub const ADD: &[&str] = &["add"];
pub const ADD_ALL: &[&str] = &["add", "-A"];
pub const RESTORE_STAGED: &[&str] = &["restore", "--staged"];
pub const UNSTAGE_ALL: &[&str] = &["reset", "HEAD"];
pub const RESTORE: &[&str] = &["restore"];
pub const COMMIT: &[&str] = &["commit", "-m"];

pub const BRANCH_SHOW_CURRENT: &[&str] = &["branch", "--show-current"];
pub const LOG_UNPUSHED: &[&str] = &["log", "@{u}..", "--oneline"];
pub const AHEAD_BEHIND: &[&str] = &["rev-list", "--left-right", "--count", "@{u}...HEAD"];
