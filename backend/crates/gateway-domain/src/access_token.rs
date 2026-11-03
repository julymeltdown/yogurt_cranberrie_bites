//! The principal a verified access token names.
//!
//! This is the neutral result of access-token verification. How a token is
//! decoded and which keys verify it are adapter concerns
//! (`api_gateway::adapters::out::jwt`); core policy sees only these claims.

/// Why an access token was not accepted.
///
/// Core policy only needs to distinguish the one rejection a key refresh can
/// change, so this carries no library error and no claim from the token. The
/// decoding adapter keeps the diagnostic detail in its own error type.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TokenRejection {
    #[error("JWT could not be decoded")]
    Malformed,
    #[error("JWT header carries a key or a key location, which is never trusted")]
    EmbeddedKeyReference,
    #[error("JWT kid is required")]
    MissingKid,
    /// The token names a key that is not installed. This is the one rejection a
    /// key refresh can change.
    #[error("JWT kid is unknown")]
    UnknownKid,
    #[error("JWT is invalid")]
    Invalid,
    #[error("JWT audience is invalid")]
    WrongAudience,
    #[error("JWT token type is not access")]
    NotAccessToken,
}

impl TokenRejection {
    /// A bounded metrics label. The `kid` is never a label, because whoever
    /// sent the token chose it.
    pub fn reason(&self) -> &'static str {
        match self {
            Self::UnknownKid => "unknown_signing_key",
            Self::EmbeddedKeyReference => "embedded_key_reference",
            _ => "invalid_bearer_token",
        }
    }
}

#[derive(Clone)]
pub struct VerifiedAccessToken {
    pub subject: String,
    pub roles: Vec<String>,
    /// Session family (`sid`), used to confirm high-risk actions with the
    /// issuer. Absent in tokens issued before access tokens carried one.
    pub session_id: Option<String>,
}

impl std::fmt::Debug for VerifiedAccessToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VerifiedAccessToken")
            .field("subject", &"[REDACTED]")
            .field("roles", &"[REDACTED]")
            .field("session_id", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::VerifiedAccessToken;

    #[test]
    fn debug_output_never_contains_claims() {
        let token = VerifiedAccessToken {
            subject: "user-secret-subject".into(),
            roles: vec!["ROLE_ADMIN".into()],
            session_id: Some("session-secret".into()),
        };
        let rendered = format!("{token:?}");
        assert!(!rendered.contains("secret"));
        assert!(!rendered.contains("ROLE_ADMIN"));
    }

    #[test]
    fn only_an_unknown_key_is_a_refreshable_rejection() {
        use super::TokenRejection;
        assert_eq!(TokenRejection::UnknownKid.reason(), "unknown_signing_key");
        assert_eq!(
            TokenRejection::EmbeddedKeyReference.reason(),
            "embedded_key_reference"
        );
        for rejection in [
            TokenRejection::Malformed,
            TokenRejection::MissingKid,
            TokenRejection::Invalid,
            TokenRejection::WrongAudience,
            TokenRejection::NotAccessToken,
        ] {
            assert_eq!(rejection.reason(), "invalid_bearer_token");
        }
    }
}
