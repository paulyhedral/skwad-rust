use thiserror::Error;

pub type Result<T, E = DiscoveryError> = std::result::Result<T, E>;

#[derive(Debug, Error)]
pub enum DiscoveryError {
    #[error("watching the source folder failed: {0}")]
    Watch(#[from] notify::Error),
}
