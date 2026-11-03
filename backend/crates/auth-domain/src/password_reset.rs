use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub struct PasswordResetToken {
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
}

impl std::fmt::Debug for PasswordResetToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PasswordResetToken")
            .field("user_id", &self.user_id)
            .field("token_hash", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[cfg(test)]
mod secret_debug_tests {
    use super::PasswordResetToken;
    use uuid::Uuid;

    #[test]
    fn password_reset_token_debug_redacts_token_hash() {
        let token = PasswordResetToken {
            user_id: Uuid::new_v4(),
            token_hash: "password-reset-hash-secret".into(),
            expires_at: chrono::Utc::now(),
        };

        let rendered = format!("{token:?}");

        assert!(
            !rendered.contains("password-reset-hash-secret"),
            "{rendered}"
        );
        assert!(rendered.contains("[REDACTED]"));
    }
}
