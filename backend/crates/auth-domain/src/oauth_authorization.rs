use crate::user::IdentityProvider;
use chrono::{DateTime, Utc};

#[derive(Clone, PartialEq, Eq)]
pub struct OAuthAuthorizationTransaction {
    pub provider: IdentityProvider,
    pub state: String,
    pub redirect_uri: String,
    pub code_verifier: String,
    pub code_challenge: String,
    pub nonce: String,
    pub expires_at: DateTime<Utc>,
}

impl std::fmt::Debug for OAuthAuthorizationTransaction {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OAuthAuthorizationTransaction")
            .field("provider", &self.provider)
            .field("state", &"[REDACTED]")
            .field("redirect_uri", &"[REDACTED]")
            .field("code_verifier", &"[REDACTED]")
            .field("code_challenge", &"[REDACTED]")
            .field("nonce", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct ConsumedOAuthAuthorization {
    pub provider: IdentityProvider,
    pub state: String,
    pub redirect_uri: String,
    pub code_verifier: String,
    pub nonce: String,
}

impl std::fmt::Debug for ConsumedOAuthAuthorization {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ConsumedOAuthAuthorization")
            .field("provider", &self.provider)
            .field("state", &"[REDACTED]")
            .field("redirect_uri", &"[REDACTED]")
            .field("code_verifier", &"[REDACTED]")
            .field("nonce", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod secret_debug_tests {
    use super::{ConsumedOAuthAuthorization, OAuthAuthorizationTransaction};
    use crate::user::IdentityProvider;
    use chrono::Utc;

    #[test]
    fn oauth_authorization_debug_does_not_render_state_or_pkce_material() {
        let authorization = OAuthAuthorizationTransaction {
            provider: IdentityProvider::Google,
            state: "oauth-state-secret".into(),
            redirect_uri: "https://app.example.test/callback".into(),
            code_verifier: "pkce-verifier-secret".into(),
            code_challenge: "pkce-challenge-secret".into(),
            nonce: "oidc-nonce-secret".into(),
            expires_at: Utc::now(),
        };
        let consumed = ConsumedOAuthAuthorization {
            provider: IdentityProvider::Google,
            state: "consumed-state-secret".into(),
            redirect_uri: "https://app.example.test/callback".into(),
            code_verifier: "consumed-verifier-secret".into(),
            nonce: "consumed-nonce-secret".into(),
        };

        let rendered = format!("{authorization:?} {consumed:?}");

        for secret in [
            "oauth-state-secret",
            "pkce-verifier-secret",
            "pkce-challenge-secret",
            "oidc-nonce-secret",
            "consumed-state-secret",
            "consumed-verifier-secret",
            "consumed-nonce-secret",
        ] {
            assert!(
                !rendered.contains(secret),
                "Debug leaked {secret}: {rendered}"
            );
        }
        assert!(rendered.contains("[REDACTED]"));
    }
}
