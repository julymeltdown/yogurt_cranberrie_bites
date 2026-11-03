//! Controlled journal failures. Database diagnostics remain in the adapter.
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JournalRepositoryError {
    Storage,
    Unexpected,
}

pub type JournalRepositoryResult<T> = Result<T, JournalRepositoryError>;

impl fmt::Display for JournalRepositoryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::Storage => "Operation journal storage failed",
            Self::Unexpected => "Operation journal data is invalid",
        })
    }
}

impl std::error::Error for JournalRepositoryError {}
