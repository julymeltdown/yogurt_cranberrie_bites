//! Controlled queue failures; keys, payloads and driver diagnostics stay private.
use std::fmt;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QueueOperationError {
    Storage,
    Rejected,
    PayloadUnavailable,
}
pub type QueueOperationResult<T> = Result<T, QueueOperationError>;
impl fmt::Display for QueueOperationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Storage => "Async operation storage failed",
            Self::Rejected => "Async operation input or record is invalid",
            Self::PayloadUnavailable => "Async operation payload unavailable",
        })
    }
}
impl std::error::Error for QueueOperationError {}
