use thiserror::Error;
use uuid::Uuid;

pub type Result<T, E = AgentError> = std::result::Result<T, E>;

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AgentError {
    #[error("agent {0} not found")]
    NotFound(Uuid),

    #[error("companion agent {0} cannot own companions")]
    CompanionCannotOwn(Uuid),
}
