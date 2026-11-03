//! Controlled shared client boundary failures. Transport causes stay outside the core.
use std::fmt;

pub const MAX_GATEWAY_CLIENT_MESSAGE_CHARS: usize = 256;

/// Only a deliberate client-facing invalid-argument message is retained.
/// Normalize it before it crosses the application port; Debug never prints it.
#[derive(Clone, PartialEq, Eq)]
pub struct GatewayClientMessage(String);

impl GatewayClientMessage {
    pub fn new(message: &str) -> Self {
        let cleaned: String = message
            .chars()
            .filter(|character| !character.is_control())
            .take(MAX_GATEWAY_CLIENT_MESSAGE_CHARS)
            .collect();
        let cleaned = cleaned.trim();
        Self(if cleaned.is_empty() {
            "Invalid request".to_owned()
        } else {
            cleaned.to_owned()
        })
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for GatewayClientMessage {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("GatewayClientMessage(<redacted>)")
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GatewayClientError {
    InvalidArgument(GatewayClientMessage),
    Unauthenticated,
    PermissionDenied,
    NotFound,
    AlreadyExists,
    OperationOutcomeUnknown,
    OperationReplayExpired,
    OperationConflict,
    Aborted,
    RateLimited,
    FailedPrecondition,
    Busy,
    Unavailable,
    DeadlineExceeded,
    Unexpected,
}

pub type GatewayClientResult<T> = Result<T, GatewayClientError>;

impl fmt::Display for GatewayClientError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::InvalidArgument(message) => message.as_str(),
            Self::Unauthenticated => "Invalid credentials",
            Self::PermissionDenied => "Forbidden",
            Self::NotFound => "Not found",
            Self::AlreadyExists => "Resource already exists",
            Self::OperationOutcomeUnknown => {
                "Operation outcome is unknown; reconcile using the original operation ID and key"
            }
            Self::OperationReplayExpired => {
                "Operation replay window expired; reconcile before issuing another command"
            }
            Self::OperationConflict => {
                "Operation identity or request conflicts with its durable receipt"
            }
            Self::Aborted => "Operation conflict; retry",
            Self::RateLimited => "Too many requests",
            Self::FailedPrecondition => "Request is not valid in the current state",
            Self::Busy => "Service busy; retry later",
            Self::Unavailable => "Service unavailable",
            Self::DeadlineExceeded => "Downstream request timed out",
            Self::Unexpected => "Internal error",
        })
    }
}

impl std::error::Error for GatewayClientError {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::error::Error;

    #[test]
    fn public_message_is_normalized_bounded_and_redacted_in_debug() {
        let message = GatewayClientMessage::new(&format!("  public\n{}  ", "가".repeat(400)));
        assert_eq!(message.as_str(), format!("public{}", "가".repeat(248)));
        assert_eq!(message.as_str().chars().count(), 254);
        assert_eq!(
            GatewayClientMessage::new("\n\r\t  ").as_str(),
            "Invalid request"
        );
        assert_eq!(GatewayClientMessage::new("  input  ").as_str(), "input");
        assert!(!format!("{message:?}").contains("public"));
        assert!(!format!("{message:?}").contains('가'));
    }

    #[test]
    fn controlled_failures_have_no_library_source_or_private_diagnostic() {
        let failure =
            GatewayClientError::InvalidArgument(GatewayClientMessage::new("Public validation"));
        assert!(failure.source().is_none());
        assert_eq!(failure.to_string(), "Public validation");
        assert!(!format!("{failure:?}").contains("Public validation"));
        let unexpected = GatewayClientError::Unexpected;
        assert!(unexpected.source().is_none());
        assert_eq!(unexpected.to_string(), "Internal error");
    }
}
