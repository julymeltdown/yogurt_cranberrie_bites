//! Typed, transport-free failures. No database, codec or CPU error crosses a port.
use crate::dto::mutation_dto::MutationError;
use profile_domain::avatar::{MAX_AVATAR_BYTES, MAX_AVATAR_PIXELS};

pub type ProfileResult<T> = Result<T, ProfileError>;

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ProfileError {
    InvalidInput(&'static str),
    ProfileNotFound,
    Forbidden,
    AccountDeleted,
    Busy,
    Persistence,
    ReceiptUnavailable,
    Mutation(MutationError),
    AvatarUploadTooLarge,
    AvatarContentTooLarge { received: usize },
    AvatarPixelsExceeded { width: u32, height: u32 },
}

impl From<MutationError> for ProfileError {
    fn from(value: MutationError) -> Self {
        Self::Mutation(value)
    }
}

impl std::fmt::Display for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidInput(message) => f.write_str(message),
            Self::ProfileNotFound => f.write_str("Profile not found"),
            Self::Forbidden => f.write_str("Forbidden"),
            Self::AccountDeleted => f.write_str("Account is deleted"),
            Self::Busy => f.write_str("PROFILE_BUSY"),
            Self::Persistence => f.write_str("Profile persistence operation failed"),
            Self::ReceiptUnavailable => f.write_str("Owner operation outcome unavailable"),
            Self::Mutation(error) => error.fmt(f),
            Self::AvatarUploadTooLarge => {
                write!(f, "Avatar image must be <= {MAX_AVATAR_BYTES} bytes")
            }
            Self::AvatarContentTooLarge { received } => write!(
                f,
                "Avatar exceeds the {MAX_AVATAR_BYTES} byte limit; received {received} bytes"
            ),
            Self::AvatarPixelsExceeded { width, height } => write!(
                f,
                "Avatar is {width}x{height} which exceeds the {MAX_AVATAR_PIXELS} pixel limit"
            ),
        }
    }
}

impl std::fmt::Debug for ProfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProfileError")
            .field("message", &self.to_string())
            .finish()
    }
}

impl std::error::Error for ProfileError {}
