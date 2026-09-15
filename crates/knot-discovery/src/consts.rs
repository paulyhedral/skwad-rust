use std::time::Duration;

pub const DEBOUNCE: Duration = Duration::from_secs(1);

pub const GIT_DIR: &str = ".git";
pub const HEAD: &str = "HEAD";
pub const HEAD_REF_PREFIX: &str = "ref: refs/heads/";
pub const GITDIR_PREFIX: &str = "gitdir: ";
pub const WORKTREES_MARKER: &str = "/.git/worktrees/";
