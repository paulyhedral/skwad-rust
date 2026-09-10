use thiserror::Error;

pub type Result<T, E = GitError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum GitError {
    #[error("git {command} timed out")]
    Timeout { command: String },

    #[error("git {command} failed (exit {code}): {output}")]
    Command {
        command: String,
        output: String,
        code: i32,
    },

    #[error("running git failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("parsing git output failed: {0}")]
    Parse(String),
}

#[cfg(test)]
mod tests {
    use super::GitError;

    #[test]
    fn timeout_display_names_command() {
        let err = GitError::Timeout {
            command: "status --porcelain=v2 --branch".to_owned(),
        };

        assert!(err.to_string().contains("status --porcelain=v2 --branch"));
    }

    #[test]
    fn command_display_carries_command_code_and_output() {
        let err = GitError::Command {
            command: "commit -m msg".to_owned(),
            output: "nothing to commit".to_owned(),
            code: 1,
        };

        let text = err.to_string();

        assert!(text.contains("commit -m msg"));
        assert!(text.contains("exit 1"));
        assert!(text.contains("nothing to commit"));
    }
}
