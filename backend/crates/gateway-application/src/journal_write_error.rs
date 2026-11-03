//! Retry classification without exposing persistence diagnostics.
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalWriteError {
    Transient,
    Permanent,
}
pub type JournalWriteResult<T> = Result<T, JournalWriteError>;
impl fmt::Display for JournalWriteError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Transient => "Operation journal temporarily unavailable",
            Self::Permanent => "Operation journal write failed",
        })
    }
}
impl std::error::Error for JournalWriteError {}
