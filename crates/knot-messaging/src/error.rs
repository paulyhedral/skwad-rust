/// Rejection reason for `send`. `Display` produces the exact text
/// `openspec/specs/mcp-messaging/spec.md` pins as the wire-visible rejection
/// message, so `mcp-tools` can surface `.to_string()` verbatim while callers
/// that care about *why* can match the variant instead of comparing strings.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum SendError {
    #[error("Sender not registered")]
    SenderNotRegistered,
    #[error("Recipient not found")]
    RecipientNotFound,
    #[error("Cannot send messages to shell agents")]
    ShellRecipient,
    #[error("Only the owner can send messages to a companion agent")]
    NotCompanionOwner,
    #[error("Companion agents can only send messages to their owner")]
    CompanionNotOwner,
}

pub type Result<T> = std::result::Result<T, SendError>;
