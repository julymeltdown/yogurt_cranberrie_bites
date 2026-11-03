//! Strip implementation details before failures cross application capabilities.
use crate::application::errors::NotificationError;

pub fn boundary_error(error: anyhow::Error) -> NotificationError {
    if let Some(known) = error.downcast_ref::<NotificationError>() {
        return *known;
    }
    if let Some(database) = error.downcast_ref::<sqlx::Error>() {
        if matches!(
            database,
            sqlx::Error::PoolClosed
                | sqlx::Error::PoolTimedOut
                | sqlx::Error::Io(_)
                | sqlx::Error::Tls(_)
        ) {
            return NotificationError::DependencyUnavailable;
        }
    }
    NotificationError::Unexpected
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::application::errors::NotificationInvalidInput;
    use std::error::Error;

    #[test]
    fn arbitrary_messages_and_sql_diagnostics_are_opaque_even_when_they_spoof_policy_text() {
        for diagnostic in [
            "PRIVATE_NOTIFICATION_DIAGNOSTIC_CANARY",
            "Forbidden",
            "Notification not found",
            "Invalid cursor format",
        ] {
            for cause in [
                anyhow::anyhow!(diagnostic),
                anyhow::Error::new(sqlx::Error::Protocol(diagnostic.into())),
            ] {
                let error = boundary_error(cause.context("private caller/payload/password"));
                assert_eq!(error, NotificationError::Unexpected);
                assert_eq!(error.to_string(), "Internal error");
                assert!(error.source().is_none());
                assert!(!format!("{error:?}").contains("private"));
            }
        }
    }

    #[test]
    fn typed_policy_and_availability_survive_without_private_context() {
        let known =
            NotificationError::InvalidInput(NotificationInvalidInput::LegacyCursorUnavailable);
        assert_eq!(
            boundary_error(anyhow::Error::new(known).context("private anchor/caller")),
            known
        );
        for database in [sqlx::Error::PoolClosed, sqlx::Error::PoolTimedOut] {
            let error =
                boundary_error(anyhow::Error::new(database).context("private endpoint/password"));
            assert_eq!(error, NotificationError::DependencyUnavailable);
            assert!(error.source().is_none());
        }
    }
}
