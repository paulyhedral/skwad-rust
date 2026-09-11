use thiserror::Error;

pub type Result<T, E = WatchError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum WatchError {
    #[error("watching the path failed: {0}")]
    Watch(#[from] notify::Error),
}
