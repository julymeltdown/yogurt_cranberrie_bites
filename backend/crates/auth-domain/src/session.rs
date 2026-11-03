use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub struct AuthTokenPair {
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in_seconds: i64,
    pub refresh_jti: String,
    pub refresh_expires_at: DateTime<Utc>,
    /// The session family these tokens belong to, carried in the access token as
    /// `sid`. A new login starts a family whose id is its first refresh jti; a
    /// refresh keeps the family of the token it replaces.
    pub session_id: Uuid,
}

impl std::fmt::Debug for AuthTokenPair {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthTokenPair")
            .field("access_token", &"[REDACTED]")
            .field("refresh_token", &"[REDACTED]")
            .field("token_type", &self.token_type)
            .field("expires_in_seconds", &self.expires_in_seconds)
            .field("refresh_jti", &"[REDACTED]")
            .field("refresh_expires_at", &self.refresh_expires_at)
            .field("session_id", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthSession {
    pub user_id: Uuid,
    pub tokens: AuthTokenPair,
}

impl std::fmt::Debug for AuthSession {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthSession")
            .field("user_id", &"[REDACTED]")
            .field("tokens", &self.tokens)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct RefreshTokenRecord {
    pub jti: String,
    pub token_hash: String,
    pub user_id: Uuid,
    /// Lineage of one login session. Every token descended from a single login
    /// shares this value, so reuse of a superseded ancestor can be attributed to
    /// one device instead of to every session the user owns.
    pub family_id: Uuid,
    pub expires_at: DateTime<Utc>,
}

impl std::fmt::Debug for RefreshTokenRecord {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RefreshTokenRecord")
            .field("jti", &"[REDACTED]")
            .field("token_hash", &"[REDACTED]")
            .field("user_id", &self.user_id)
            .field("family_id", &"[REDACTED]")
            .field("expires_at", &self.expires_at)
            .finish()
    }
}

impl RefreshTokenRecord {
    /// Begin a new session family. The first token is its own family root.
    pub fn new_family(
        family_id: Uuid,
        jti: String,
        token_hash: String,
        user_id: Uuid,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            jti,
            token_hash,
            user_id,
            family_id,
            expires_at,
        }
    }

    /// Continue an existing family with a freshly issued token.
    pub fn successor_of(
        family_id: Uuid,
        jti: String,
        token_hash: String,
        user_id: Uuid,
        expires_at: DateTime<Utc>,
    ) -> Self {
        Self {
            jti,
            token_hash,
            user_id,
            family_id,
            expires_at,
        }
    }
}

/// Lifecycle state of a refresh token that is no longer active.
///
/// The distinction matters because the three cases call for three different
/// responses. Before this existed they collapsed into "not found", which the
/// refresh path answered by revoking every session the user owned.
#[derive(Clone, PartialEq, Eq)]
pub enum RefreshTokenState {
    /// Superseded by a rotation. A concurrent refresh inside the grace window is
    /// benign; the same token arriving later is reuse and revokes the family.
    Rotated {
        successor_jti: String,
        rotated_at: DateTime<Utc>,
        family_id: Uuid,
        user_id: Uuid,
    },
    /// Ended deliberately by logout. Presenting it is an ordinary stale client,
    /// not an attack, and must not touch other devices.
    RevokedByLogout,
    /// Revoked by a credential change, role change, or operator action.
    Revoked,
}

impl std::fmt::Debug for RefreshTokenState {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rotated { rotated_at, .. } => formatter
                .debug_struct("RefreshTokenState::Rotated")
                .field("successor_jti", &"[REDACTED]")
                .field("rotated_at", rotated_at)
                .field("family_id", &"[REDACTED]")
                .field("user_id", &"[REDACTED]")
                .finish(),
            Self::RevokedByLogout => formatter.write_str("RefreshTokenState::RevokedByLogout"),
            Self::Revoked => formatter.write_str("RefreshTokenState::Revoked"),
        }
    }
}

/// Result of attempting to rotate a refresh token.
#[derive(Clone, PartialEq, Eq)]
pub enum RotationOutcome {
    /// This token was live and is now superseded by the freshly stored one.
    Rotated,
    /// This token had already been superseded. The caller decides whether this is
    /// a concurrent client or reuse, using rotated_at and the caller's grace
    /// window.
    AlreadyRotated {
        successor_jti: String,
        rotated_at: DateTime<Utc>,
    },
    /// The token is not rotatable for a reason other than a prior rotation:
    /// unknown, expired, hash mismatch, or its session was ended.
    NotRotatable,
}

impl std::fmt::Debug for RotationOutcome {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Rotated => formatter.write_str("RotationOutcome::Rotated"),
            Self::AlreadyRotated { rotated_at, .. } => formatter
                .debug_struct("RotationOutcome::AlreadyRotated")
                .field("successor_jti", &"[REDACTED]")
                .field("rotated_at", rotated_at)
                .finish(),
            Self::NotRotatable => formatter.write_str("RotationOutcome::NotRotatable"),
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct LoginAudit {
    pub id: Uuid,
    pub user_id: Uuid,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub success: bool,
    /// False when the attempt was refused *because* the account was already
    /// locked. Such attempts must not extend the lockout, otherwise an attacker
    /// who knows an address can keep an account locked forever by sending a
    /// request every few minutes.
    pub credential_attempt: bool,
    pub occurred_at: DateTime<Utc>,
}

impl std::fmt::Debug for LoginAudit {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("LoginAudit")
            .field("id", &"[REDACTED]")
            .field("user_id", &"[REDACTED]")
            .field("ip_address", &"[REDACTED]")
            .field("user_agent", &"[REDACTED]")
            .field("success", &self.success)
            .field("credential_attempt", &self.credential_attempt)
            .field("occurred_at", &self.occurred_at)
            .finish()
    }
}

impl LoginAudit {
    /// A genuine credential check that failed on its own merits.
    pub fn failure(
        id: Uuid,
        user_id: Uuid,
        ip_address: Option<String>,
        user_agent: Option<String>,
        occurred_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            user_id,
            ip_address,
            user_agent,
            success: false,
            credential_attempt: true,
            occurred_at,
        }
    }

    /// A refusal caused by an existing lockout. Recorded for operators, but not
    /// counted toward the failure threshold.
    pub fn lockout_refusal(
        id: Uuid,
        user_id: Uuid,
        ip_address: Option<String>,
        user_agent: Option<String>,
        occurred_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            user_id,
            ip_address,
            user_agent,
            success: false,
            credential_attempt: false,
            occurred_at,
        }
    }

    pub fn success(
        id: Uuid,
        user_id: Uuid,
        ip_address: Option<String>,
        user_agent: Option<String>,
        occurred_at: DateTime<Utc>,
    ) -> Self {
        Self {
            id,
            user_id,
            ip_address,
            user_agent,
            success: true,
            credential_attempt: true,
            occurred_at,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthAuditEvent {
    pub id: Uuid,
    pub actor_id: Option<Uuid>,
    pub subject_user_id: Option<Uuid>,
    pub action: String,
    pub outcome: String,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub metadata_json: String,
    pub occurred_at: DateTime<Utc>,
}

impl std::fmt::Debug for AuthAuditEvent {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("AuthAuditEvent")
            .field("id", &"[REDACTED]")
            .field("actor_id", &"[REDACTED]")
            .field("subject_user_id", &"[REDACTED]")
            .field("action", &self.action)
            .field("outcome", &self.outcome)
            .field("request_id", &"[REDACTED]")
            .field("ip_address", &"[REDACTED]")
            .field("user_agent", &"[REDACTED]")
            .field("metadata_json", &"[REDACTED]")
            .field("occurred_at", &self.occurred_at)
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DecodedRefreshToken {
    pub subject: Uuid,
    pub jti: String,
}

impl std::fmt::Debug for DecodedRefreshToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DecodedRefreshToken")
            .field("subject", &"[REDACTED]")
            .field("jti", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct DecodedAccessToken {
    pub subject: Uuid,
    pub roles: Vec<String>,
    /// The session family named by the token, absent in tokens issued before
    /// access tokens carried one.
    pub session_id: Option<Uuid>,
}

impl std::fmt::Debug for DecodedAccessToken {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DecodedAccessToken")
            .field("subject", &"[REDACTED]")
            .field("roles", &"[REDACTED]")
            .field("session_id", &"[REDACTED]")
            .finish()
    }
}

/// Whether a session may still perform a high-risk action, as the issuer sees it
/// now rather than as its access token described it when issued.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionStatus {
    /// The session family still holds a live refresh token and the account is
    /// active. `roles` are the account's current roles, which may be fewer
    /// than the access token claims.
    Active { roles: Vec<String> },
    /// Logged out, revoked, expired, or belonging to an inactive account.
    Ended,
}

#[cfg(test)]
mod secret_debug_tests {
    use super::{
        AuthAuditEvent, AuthSession, AuthTokenPair, DecodedAccessToken, DecodedRefreshToken,
        LoginAudit, RefreshTokenRecord, RefreshTokenState, RotationOutcome,
    };
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn auth_token_pair_debug_does_not_render_bearer_credentials() {
        let tokens = AuthTokenPair {
            access_token: "access-token-secret".into(),
            refresh_token: "refresh-token-secret".into(),
            token_type: "Bearer".into(),
            expires_in_seconds: 900,
            refresh_jti: "refresh-jti-secret".into(),
            refresh_expires_at: Utc::now(),
            session_id: Uuid::new_v4(),
        };

        let rendered = format!("{tokens:?}");

        for secret in [
            "access-token-secret",
            "refresh-token-secret",
            "refresh-jti-secret",
        ] {
            assert!(
                !rendered.contains(secret),
                "Debug leaked {secret}: {rendered}"
            );
        }
        assert!(rendered.contains("[REDACTED]"));
    }

    #[test]
    fn refresh_token_record_debug_does_not_render_token_material() {
        let record = RefreshTokenRecord {
            jti: "refresh-jti-secret".into(),
            token_hash: "refresh-token-hash-secret".into(),
            user_id: Uuid::new_v4(),
            family_id: Uuid::new_v4(),
            expires_at: Utc::now(),
        };

        let rendered = format!("{record:?}");

        for secret in ["refresh-jti-secret", "refresh-token-hash-secret"] {
            assert!(
                !rendered.contains(secret),
                "Debug leaked {secret}: {rendered}"
            );
        }
        assert!(rendered.contains("[REDACTED]"));
    }

    #[test]
    fn auth_session_debug_redacts_account_identity() {
        let user_id = Uuid::new_v4();
        let session = AuthSession {
            user_id,
            tokens: AuthTokenPair {
                access_token: "access-secret".into(),
                refresh_token: "refresh-secret".into(),
                token_type: "Bearer".into(),
                expires_in_seconds: 900,
                refresh_jti: "refresh-jti-secret".into(),
                refresh_expires_at: Utc::now(),
                session_id: Uuid::new_v4(),
            },
        };

        let rendered = format!("{session:?}");

        assert!(!rendered.contains(&user_id.to_string()), "{rendered}");
        assert!(!rendered.contains("access-secret"), "{rendered}");
        assert!(!rendered.contains("refresh-secret"), "{rendered}");
    }

    #[test]
    fn refresh_state_debug_redacts_successor_and_session_identifiers() {
        let family_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let state = RefreshTokenState::Rotated {
            successor_jti: "successor-jti-secret".into(),
            rotated_at: Utc::now(),
            family_id,
            user_id,
        };

        let rendered = format!("{state:?}");

        for sensitive in [
            "successor-jti-secret".to_owned(),
            family_id.to_string(),
            user_id.to_string(),
        ] {
            assert!(
                !rendered.contains(&sensitive),
                "Debug leaked {sensitive}: {rendered}"
            );
        }
    }

    #[test]
    fn rotation_outcome_debug_redacts_successor_jti() {
        let outcome = RotationOutcome::AlreadyRotated {
            successor_jti: "successor-jti-secret".into(),
            rotated_at: Utc::now(),
        };

        let rendered = format!("{outcome:?}");

        assert!(!rendered.contains("successor-jti-secret"), "{rendered}");
    }

    #[test]
    fn login_audit_debug_redacts_account_and_network_metadata() {
        let user_id = Uuid::new_v4();
        let audit = LoginAudit::failure(
            Uuid::new_v4(),
            user_id,
            Some("198.51.100.27".into()),
            Some("private-user-agent".into()),
            Utc::now(),
        );

        let rendered = format!("{audit:?}");

        for sensitive in [
            user_id.to_string(),
            "198.51.100.27".into(),
            "private-user-agent".into(),
        ] {
            assert!(
                !rendered.contains(&sensitive),
                "Debug leaked {sensitive}: {rendered}"
            );
        }
    }

    #[test]
    fn auth_audit_event_debug_redacts_identity_and_metadata() {
        let actor_id = Uuid::new_v4();
        let subject_user_id = Uuid::new_v4();
        let event = AuthAuditEvent {
            id: Uuid::new_v4(),
            actor_id: Some(actor_id),
            subject_user_id: Some(subject_user_id),
            action: "admin.change_role".into(),
            outcome: "success".into(),
            request_id: Some("private-request-id".into()),
            ip_address: Some("198.51.100.28".into()),
            user_agent: Some("private-user-agent".into()),
            metadata_json: r#"{"email":"private@example.test","token":"private-token"}"#.into(),
            occurred_at: Utc::now(),
        };

        let rendered = format!("{event:?}");

        for sensitive in [
            actor_id.to_string(),
            subject_user_id.to_string(),
            "private-request-id".to_owned(),
            "198.51.100.28".to_owned(),
            "private-user-agent".to_owned(),
            "private@example.test".to_owned(),
            "private-token".to_owned(),
        ] {
            assert!(
                !rendered.contains(&sensitive),
                "Debug leaked {sensitive}: {rendered}"
            );
        }
    }

    #[test]
    fn decoded_token_debug_redacts_identity_and_claims() {
        let refresh_user_id = Uuid::new_v4();
        let refresh = DecodedRefreshToken {
            subject: refresh_user_id,
            jti: "decoded-refresh-jti".into(),
        };
        let access_user_id = Uuid::new_v4();
        let session_id = Uuid::new_v4();
        let access = DecodedAccessToken {
            subject: access_user_id,
            roles: vec!["ROLE_PRIVATE".into()],
            session_id: Some(session_id),
        };

        let refresh_rendered = format!("{refresh:?}");
        let access_rendered = format!("{access:?}");

        for sensitive in [refresh_user_id.to_string(), "decoded-refresh-jti".into()] {
            assert!(
                !refresh_rendered.contains(&sensitive),
                "Debug leaked {sensitive}: {refresh_rendered}"
            );
        }
        for sensitive in [
            access_user_id.to_string(),
            session_id.to_string(),
            "ROLE_PRIVATE".into(),
        ] {
            assert!(
                !access_rendered.contains(&sensitive),
                "Debug leaked {sensitive}: {access_rendered}"
            );
        }
    }
}
