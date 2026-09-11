use thiserror::Error;

#[derive(Debug, Error)]
pub enum McpError {
    #[error("failed to bind {0}: {1}")]
    Bind(std::net::SocketAddr, std::io::Error),
    #[error("server error: {0}")]
    Serve(std::io::Error),
}

pub type Result<T, E = McpError> = std::result::Result<T, E>;
