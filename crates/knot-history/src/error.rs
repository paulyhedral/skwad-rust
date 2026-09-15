use thiserror::Error;

pub type Result<T, E = HistoryError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum HistoryError {
    #[error("reading history file failed: {0}")]
    Io(#[from] std::io::Error),

    #[error("parsing history JSON failed: {0}")]
    Json(#[from] serde_json::Error),

    #[error("querying history database failed: {0}")]
    Sqlite(#[from] rusqlite::Error),
}

#[cfg(test)]
mod tests {
    use super::HistoryError;

    #[test]
    fn io_display_wraps_source() {
        let io_err = std::io::Error::new(std::io::ErrorKind::NotFound, "missing");
        let err: HistoryError = io_err.into();

        assert!(err.to_string().contains("missing"));
    }
}
