use crate::errors::NotificationError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NotificationFailureKind {
    InvalidArgument,
    NotFound,
    PermissionDenied,
    Unavailable,
    Internal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NotificationFailure {
    pub kind: NotificationFailureKind,
    pub message: &'static str,
}

pub fn present(error: NotificationError) -> NotificationFailure {
    use NotificationFailureKind as Kind;
    let (kind, message) = match error {
        NotificationError::InvalidInput(value) => (Kind::InvalidArgument, value.message()),
        NotificationError::NotFound => (Kind::NotFound, "Notification not found"),
        NotificationError::Forbidden => (Kind::PermissionDenied, "Forbidden"),
        NotificationError::AdmissionExpired => {
            (Kind::PermissionDenied, "Notification admission expired")
        }
        NotificationError::DependencyUnavailable => (Kind::Unavailable, "Service unavailable"),
        NotificationError::Unexpected => (Kind::Internal, "Internal error"),
    };
    NotificationFailure { kind, message }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::errors::NotificationInvalidInput as Input;

    #[test]
    fn controlled_validation_messages_preserve_the_existing_contract() {
        let cases = [
            (Input::UserIdRequired, "User ID is required"),
            (Input::NotificationIdRequired, "Notification ID is required"),
            (Input::ActorIdRequired, "Actor ID is required"),
            (Input::SagaIdRequired, "Saga ID is required"),
            (
                Input::RetentionDeadlineInvalid,
                "Retention deadline is invalid",
            ),
            (Input::InvalidEventId, "Invalid event ID"),
            (Input::InvalidActorId, "Invalid actor ID"),
            (Input::InvalidOccurredAt, "Invalid occurredAt"),
            (Input::TypeRequired, "Type is required"),
            (Input::UnsupportedType, "Unsupported notification type"),
            (Input::InvalidCursor, "Invalid cursor format"),
            (Input::CursorRecipientMismatch, "Cursor recipient mismatch"),
            (
                Input::LegacyCursorUnavailable,
                "Legacy cursor anchor is unavailable",
            ),
            (
                Input::LegacyCursorMismatch,
                "Legacy cursor anchor does not match its timestamp",
            ),
            (
                Input::DeletionSagaInputMismatch,
                "Notification deletion saga id was reused with different inputs",
            ),
        ];
        for (input, expected) in cases {
            let error = NotificationError::from(input);
            let failure = present(error);
            assert_eq!(failure.kind, NotificationFailureKind::InvalidArgument);
            assert_eq!(failure.message, expected);
            assert_eq!(error.to_string(), expected);
            assert!(std::error::Error::source(&error).is_none());
        }
    }

    #[test]
    fn policy_and_dependency_failures_have_fixed_safe_presentations() {
        for (error, kind, message) in [
            (
                NotificationError::NotFound,
                NotificationFailureKind::NotFound,
                "Notification not found",
            ),
            (
                NotificationError::Forbidden,
                NotificationFailureKind::PermissionDenied,
                "Forbidden",
            ),
            (
                NotificationError::DependencyUnavailable,
                NotificationFailureKind::Unavailable,
                "Service unavailable",
            ),
            (
                NotificationError::Unexpected,
                NotificationFailureKind::Internal,
                "Internal error",
            ),
        ] {
            assert_eq!(present(error), NotificationFailure { kind, message });
            assert!(std::error::Error::source(&error).is_none());
        }
    }
}
