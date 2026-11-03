#[derive(Debug, thiserror::Error)]
pub enum FriendError {
    #[error("{0}")]
    InvalidInput(String),
    #[error("{0}")]
    NotFound(String),
    #[error("{0}")]
    PermissionDenied(String),
    #[error("unexpected friend failure: {0}")]
    Unexpected(#[from] anyhow::Error),
}
