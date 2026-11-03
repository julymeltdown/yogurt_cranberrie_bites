use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum UserStatus {
    PendingVerification,
    Active,
    Deleted,
}

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IdentityProvider {
    Email,
    Google,
    Apple,
    X,
}

#[derive(Clone, PartialEq, Eq)]
pub struct User {
    pub id: Uuid,
    pub primary_email: String,
    pub password_hash: String,
    pub status: UserStatus,
    pub last_login_at: Option<DateTime<Utc>>,
}

impl std::fmt::Debug for User {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("User")
            .field("id", &self.id)
            .field("primary_email", &"[REDACTED]")
            .field("password_hash", &"[REDACTED]")
            .field("status", &self.status)
            .field("last_login_at", &self.last_login_at)
            .finish()
    }
}

impl User {
    pub fn normalized_email(email: &str) -> String {
        email.trim().to_ascii_lowercase()
    }

    pub fn with_status(&self, status: UserStatus) -> Self {
        Self {
            status,
            ..self.clone()
        }
    }

    pub fn with_last_login(&self, last_login_at: DateTime<Utc>) -> Self {
        Self {
            last_login_at: Some(last_login_at),
            ..self.clone()
        }
    }
}

#[cfg(test)]
mod secret_debug_tests {
    use super::{User, UserStatus};
    use uuid::Uuid;

    #[test]
    fn user_debug_does_not_render_email_or_password_hash() {
        let user = User {
            id: Uuid::new_v4(),
            primary_email: "private-user@example.test".into(),
            password_hash: "argon2-password-hash-secret".into(),
            status: UserStatus::Active,
            last_login_at: None,
        };

        let rendered = format!("{user:?}");

        assert!(
            !rendered.contains("private-user@example.test"),
            "{rendered}"
        );
        assert!(
            !rendered.contains("argon2-password-hash-secret"),
            "{rendered}"
        );
        assert!(rendered.contains("[REDACTED]"));
    }
}
