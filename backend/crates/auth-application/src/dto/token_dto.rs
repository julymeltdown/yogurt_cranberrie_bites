//! Concrete token-service transfers; credential serialization belongs to inbound adapters.

use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub struct AuthTokenPairDto {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in_seconds: i64,
    pub refresh_jti: String,
    pub refresh_expires_at: DateTime<Utc>,
    pub session_id: Uuid,
}
impl From<auth_domain::session::AuthTokenPair> for AuthTokenPairDto {
    fn from(value: auth_domain::session::AuthTokenPair) -> Self {
        Self {
            access_token: value.access_token,
            refresh_token: value.refresh_token,
            token_type: value.token_type,
            expires_in_seconds: value.expires_in_seconds,
            refresh_jti: value.refresh_jti,
            refresh_expires_at: value.refresh_expires_at,
            session_id: value.session_id,
        }
    }
}
impl From<AuthTokenPairDto> for auth_domain::session::AuthTokenPair {
    fn from(value: AuthTokenPairDto) -> Self {
        Self {
            access_token: value.access_token,
            refresh_token: value.refresh_token,
            token_type: value.token_type,
            expires_in_seconds: value.expires_in_seconds,
            refresh_jti: value.refresh_jti,
            refresh_expires_at: value.refresh_expires_at,
            session_id: value.session_id,
        }
    }
}
impl std::fmt::Debug for AuthTokenPairDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthTokenPairDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthDecodedAccessTokenDto {
    pub subject: Uuid,
    pub roles: Vec<String>,
    pub session_id: Option<Uuid>,
}
impl From<auth_domain::session::DecodedAccessToken> for AuthDecodedAccessTokenDto {
    fn from(value: auth_domain::session::DecodedAccessToken) -> Self {
        Self {
            subject: value.subject,
            roles: value.roles,
            session_id: value.session_id,
        }
    }
}
impl From<AuthDecodedAccessTokenDto> for auth_domain::session::DecodedAccessToken {
    fn from(value: AuthDecodedAccessTokenDto) -> Self {
        Self {
            subject: value.subject,
            roles: value.roles,
            session_id: value.session_id,
        }
    }
}
impl std::fmt::Debug for AuthDecodedAccessTokenDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthDecodedAccessTokenDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthDecodedRefreshTokenDto {
    pub subject: Uuid,
    pub jti: String,
}
impl From<auth_domain::session::DecodedRefreshToken> for AuthDecodedRefreshTokenDto {
    fn from(value: auth_domain::session::DecodedRefreshToken) -> Self {
        Self {
            subject: value.subject,
            jti: value.jti,
        }
    }
}
impl From<AuthDecodedRefreshTokenDto> for auth_domain::session::DecodedRefreshToken {
    fn from(value: AuthDecodedRefreshTokenDto) -> Self {
        Self {
            subject: value.subject,
            jti: value.jti,
        }
    }
}
impl std::fmt::Debug for AuthDecodedRefreshTokenDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthDecodedRefreshTokenDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

/// Only the public fields of an issued RSA signing key cross this boundary.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthPublicJwkDto {
    pub kty: String,
    pub key_use: String,
    pub alg: String,
    pub kid: String,
    pub n: String,
    pub e: String,
}

/// The first key signs new tokens; subsequent keys retain configured rotation order.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthJwkSetDto {
    pub keys: Vec<AuthPublicJwkDto>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use auth_domain::session::{AuthTokenPair, DecodedAccessToken, DecodedRefreshToken};

    #[test]
    fn transfers_preserve_credentials_expiry_family_optional_session_and_role_order() {
        let pair = AuthTokenPair {
            access_token: "Exact.Access.Credential".into(),
            refresh_token: "Exact.Refresh.Credential".into(),
            token_type: "Bearer".into(),
            expires_in_seconds: 901,
            refresh_jti: "CaseSensitive-JTI".into(),
            refresh_expires_at: DateTime::from_timestamp(1_700_000_000, 123_456_789).unwrap(),
            session_id: Uuid::from_u128(17),
        };
        assert_eq!(
            AuthTokenPair::from(AuthTokenPairDto::from(pair.clone())),
            pair
        );
        for session_id in [None, Some(pair.session_id)] {
            let access = DecodedAccessToken {
                subject: Uuid::from_u128(23),
                roles: vec![
                    "ROLE_SECOND".into(),
                    "ROLE_FIRST".into(),
                    "ROLE_SECOND".into(),
                ],
                session_id,
            };
            assert_eq!(
                DecodedAccessToken::from(AuthDecodedAccessTokenDto::from(access.clone())),
                access
            );
        }
        let refresh = DecodedRefreshToken {
            subject: Uuid::from_u128(31),
            jti: "CaseSensitive-Refresh-JTI".into(),
        };
        assert_eq!(
            DecodedRefreshToken::from(AuthDecodedRefreshTokenDto::from(refresh.clone())),
            refresh
        );
    }

    #[test]
    fn debug_redacts_every_credential_claim_and_family_identifier() {
        let pair = AuthTokenPairDto {
            access_token: "private-access".into(),
            refresh_token: "private-refresh".into(),
            token_type: "private-type".into(),
            expires_in_seconds: 900,
            refresh_jti: "private-jti".into(),
            refresh_expires_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            session_id: Uuid::from_u128(17),
        };
        let access = AuthDecodedAccessTokenDto {
            subject: Uuid::from_u128(23),
            roles: vec!["private-role".into()],
            session_id: Some(pair.session_id),
        };
        let refresh = AuthDecodedRefreshTokenDto {
            subject: access.subject,
            jti: "private-refresh-jti".into(),
        };
        assert_eq!(
            format!("{pair:?}"),
            "AuthTokenPairDto { fields: \"[REDACTED]\" }"
        );
        assert_eq!(
            format!("{access:?}"),
            "AuthDecodedAccessTokenDto { fields: \"[REDACTED]\" }"
        );
        assert_eq!(
            format!("{refresh:?}"),
            "AuthDecodedRefreshTokenDto { fields: \"[REDACTED]\" }"
        );
    }
}
