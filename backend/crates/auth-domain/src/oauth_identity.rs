use crate::user::IdentityProvider;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthIdentity {
    pub user_id: Uuid,
    pub provider: IdentityProvider,
    pub subject: String,
    pub email: String,
    pub email_verified: bool,
}

impl std::fmt::Debug for OAuthIdentity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OAuthIdentity")
            .field("user_id", &self.user_id)
            .field("provider", &self.provider)
            .field("subject", &"[REDACTED]")
            .field("email", &"[REDACTED]")
            .field("email_verified", &self.email_verified)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OAuthUserInfo {
    pub provider: IdentityProvider,
    pub subject: String,
    pub email: String,
    pub email_verified: bool,
}

impl std::fmt::Debug for OAuthUserInfo {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OAuthUserInfo")
            .field("provider", &self.provider)
            .field("subject", &"[REDACTED]")
            .field("email", &"[REDACTED]")
            .field("email_verified", &self.email_verified)
            .finish()
    }
}

#[cfg(test)]
mod secret_debug_tests {
    use super::{OAuthIdentity, OAuthUserInfo};
    use crate::user::IdentityProvider;
    use uuid::Uuid;

    #[test]
    fn oauth_identity_debug_does_not_render_provider_subject_or_email() {
        let identity = OAuthIdentity {
            user_id: Uuid::new_v4(),
            provider: IdentityProvider::Google,
            subject: "provider-subject-secret".into(),
            email: "private-oauth-email@example.test".into(),
            email_verified: true,
        };
        let user_info = OAuthUserInfo {
            provider: IdentityProvider::Google,
            subject: "provider-userinfo-subject-secret".into(),
            email: "private-userinfo-email@example.test".into(),
            email_verified: true,
        };

        let rendered = format!("{identity:?} {user_info:?}");

        for private_value in [
            "provider-subject-secret",
            "private-oauth-email@example.test",
            "provider-userinfo-subject-secret",
            "private-userinfo-email@example.test",
        ] {
            assert!(
                !rendered.contains(private_value),
                "Debug leaked {private_value}: {rendered}"
            );
        }
        assert!(rendered.contains("[REDACTED]"));
    }
}
