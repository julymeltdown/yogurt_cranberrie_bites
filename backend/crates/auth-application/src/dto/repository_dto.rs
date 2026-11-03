//! Application-owned account and atomic authentication transfer snapshots.
//! These types carry no database rows, transport types or domain entities.
//! Conversions preserve credential/status/role-admission evidence verbatim.
use auth_domain::oauth_authorization::{ConsumedOAuthAuthorization, OAuthAuthorizationTransaction};
use auth_domain::oauth_identity::{OAuthIdentity, OAuthUserInfo};
use auth_domain::password_reset::PasswordResetToken;
use auth_domain::session::{LoginAudit, RefreshTokenRecord, RotationOutcome};
use auth_domain::user::{IdentityProvider, User, UserStatus};
use chrono::{DateTime, Utc};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthUserStatusDto {
    PendingVerification,
    Active,
    Deleted,
}
impl From<UserStatus> for AuthUserStatusDto {
    fn from(value: UserStatus) -> Self {
        match value {
            UserStatus::PendingVerification => Self::PendingVerification,
            UserStatus::Active => Self::Active,
            UserStatus::Deleted => Self::Deleted,
        }
    }
}
impl From<AuthUserStatusDto> for UserStatus {
    fn from(value: AuthUserStatusDto) -> Self {
        match value {
            AuthUserStatusDto::PendingVerification => Self::PendingVerification,
            AuthUserStatusDto::Active => Self::Active,
            AuthUserStatusDto::Deleted => Self::Deleted,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthIdentityProviderDto {
    Email,
    Google,
    Apple,
    X,
}
impl From<IdentityProvider> for AuthIdentityProviderDto {
    fn from(value: IdentityProvider) -> Self {
        match value {
            IdentityProvider::Email => Self::Email,
            IdentityProvider::Google => Self::Google,
            IdentityProvider::Apple => Self::Apple,
            IdentityProvider::X => Self::X,
        }
    }
}
impl From<AuthIdentityProviderDto> for IdentityProvider {
    fn from(value: AuthIdentityProviderDto) -> Self {
        match value {
            AuthIdentityProviderDto::Email => Self::Email,
            AuthIdentityProviderDto::Google => Self::Google,
            AuthIdentityProviderDto::Apple => Self::Apple,
            AuthIdentityProviderDto::X => Self::X,
        }
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthUserRecordDto {
    pub id: Uuid,
    pub primary_email: String,
    pub password_hash: String,
    pub status: AuthUserStatusDto,
    pub last_login_at: Option<DateTime<Utc>>,
}
impl From<User> for AuthUserRecordDto {
    fn from(value: User) -> Self {
        Self {
            id: value.id,
            primary_email: value.primary_email,
            password_hash: value.password_hash,
            status: value.status.into(),
            last_login_at: value.last_login_at,
        }
    }
}
impl From<AuthUserRecordDto> for User {
    fn from(value: AuthUserRecordDto) -> Self {
        Self {
            id: value.id,
            primary_email: value.primary_email,
            password_hash: value.password_hash,
            status: value.status.into(),
            last_login_at: value.last_login_at,
        }
    }
}
impl From<&User> for AuthUserRecordDto {
    fn from(value: &User) -> Self {
        value.clone().into()
    }
}
impl std::fmt::Debug for AuthUserRecordDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthUserRecordDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthOAuthProofDto {
    pub provider: AuthIdentityProviderDto,
    pub subject: String,
    pub email: String,
    pub email_verified: bool,
}
impl From<OAuthUserInfo> for AuthOAuthProofDto {
    fn from(value: OAuthUserInfo) -> Self {
        Self {
            provider: value.provider.into(),
            subject: value.subject,
            email: value.email,
            email_verified: value.email_verified,
        }
    }
}
impl From<AuthOAuthProofDto> for OAuthUserInfo {
    fn from(value: AuthOAuthProofDto) -> Self {
        Self {
            provider: value.provider.into(),
            subject: value.subject,
            email: value.email,
            email_verified: value.email_verified,
        }
    }
}
impl From<&OAuthUserInfo> for AuthOAuthProofDto {
    fn from(value: &OAuthUserInfo) -> Self {
        value.clone().into()
    }
}
impl std::fmt::Debug for AuthOAuthProofDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthOAuthProofDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthPasswordResetRecordDto {
    pub user_id: Uuid,
    pub token_hash: String,
    pub expires_at: DateTime<Utc>,
}
impl From<PasswordResetToken> for AuthPasswordResetRecordDto {
    fn from(value: PasswordResetToken) -> Self {
        Self {
            user_id: value.user_id,
            token_hash: value.token_hash,
            expires_at: value.expires_at,
        }
    }
}
impl From<AuthPasswordResetRecordDto> for PasswordResetToken {
    fn from(value: AuthPasswordResetRecordDto) -> Self {
        Self {
            user_id: value.user_id,
            token_hash: value.token_hash,
            expires_at: value.expires_at,
        }
    }
}
impl From<&PasswordResetToken> for AuthPasswordResetRecordDto {
    fn from(value: &PasswordResetToken) -> Self {
        value.clone().into()
    }
}
impl std::fmt::Debug for AuthPasswordResetRecordDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthPasswordResetRecordDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthRefreshRecordDto {
    pub jti: String,
    pub token_hash: String,
    pub user_id: Uuid,
    pub family_id: Uuid,
    pub expires_at: DateTime<Utc>,
}
impl From<RefreshTokenRecord> for AuthRefreshRecordDto {
    fn from(value: RefreshTokenRecord) -> Self {
        Self {
            jti: value.jti,
            token_hash: value.token_hash,
            user_id: value.user_id,
            family_id: value.family_id,
            expires_at: value.expires_at,
        }
    }
}
impl From<AuthRefreshRecordDto> for RefreshTokenRecord {
    fn from(value: AuthRefreshRecordDto) -> Self {
        Self {
            jti: value.jti,
            token_hash: value.token_hash,
            user_id: value.user_id,
            family_id: value.family_id,
            expires_at: value.expires_at,
        }
    }
}
impl From<&RefreshTokenRecord> for AuthRefreshRecordDto {
    fn from(value: &RefreshTokenRecord) -> Self {
        value.clone().into()
    }
}
impl std::fmt::Debug for AuthRefreshRecordDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthRefreshRecordDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthLoginAuditRecordDto {
    pub id: Uuid,
    pub user_id: Uuid,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub success: bool,
    pub credential_attempt: bool,
    pub occurred_at: DateTime<Utc>,
}
impl From<LoginAudit> for AuthLoginAuditRecordDto {
    fn from(value: LoginAudit) -> Self {
        Self {
            id: value.id,
            user_id: value.user_id,
            ip_address: value.ip_address,
            user_agent: value.user_agent,
            success: value.success,
            credential_attempt: value.credential_attempt,
            occurred_at: value.occurred_at,
        }
    }
}
impl From<AuthLoginAuditRecordDto> for LoginAudit {
    fn from(value: AuthLoginAuditRecordDto) -> Self {
        Self {
            id: value.id,
            user_id: value.user_id,
            ip_address: value.ip_address,
            user_agent: value.user_agent,
            success: value.success,
            credential_attempt: value.credential_attempt,
            occurred_at: value.occurred_at,
        }
    }
}
impl From<&LoginAudit> for AuthLoginAuditRecordDto {
    fn from(value: &LoginAudit) -> Self {
        value.clone().into()
    }
}
impl std::fmt::Debug for AuthLoginAuditRecordDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthLoginAuditRecordDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub enum AuthRotationOutcomeDto {
    Rotated,
    AlreadyRotated {
        successor_jti: String,
        rotated_at: DateTime<Utc>,
    },
    NotRotatable,
}
impl From<RotationOutcome> for AuthRotationOutcomeDto {
    fn from(value: RotationOutcome) -> Self {
        match value {
            RotationOutcome::Rotated => Self::Rotated,
            RotationOutcome::NotRotatable => Self::NotRotatable,
            RotationOutcome::AlreadyRotated {
                successor_jti,
                rotated_at,
            } => Self::AlreadyRotated {
                successor_jti,
                rotated_at,
            },
        }
    }
}
impl From<AuthRotationOutcomeDto> for RotationOutcome {
    fn from(value: AuthRotationOutcomeDto) -> Self {
        match value {
            AuthRotationOutcomeDto::Rotated => Self::Rotated,
            AuthRotationOutcomeDto::NotRotatable => Self::NotRotatable,
            AuthRotationOutcomeDto::AlreadyRotated {
                successor_jti,
                rotated_at,
            } => Self::AlreadyRotated {
                successor_jti,
                rotated_at,
            },
        }
    }
}
impl std::fmt::Debug for AuthRotationOutcomeDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthRotationOutcomeDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

// OAuth persistence transfers preserve consumed proof and single-use material.

#[derive(Clone, PartialEq, Eq)]
pub struct AuthOAuthIdentityRecordDto {
    pub user_id: Uuid,
    pub provider: AuthIdentityProviderDto,
    pub subject: String,
    pub email: String,
    pub email_verified: bool,
}
impl From<OAuthIdentity> for AuthOAuthIdentityRecordDto {
    fn from(value: OAuthIdentity) -> Self {
        Self {
            user_id: value.user_id,
            provider: value.provider.into(),
            subject: value.subject,
            email: value.email,
            email_verified: value.email_verified,
        }
    }
}
impl From<AuthOAuthIdentityRecordDto> for OAuthIdentity {
    fn from(value: AuthOAuthIdentityRecordDto) -> Self {
        Self {
            user_id: value.user_id,
            provider: value.provider.into(),
            subject: value.subject,
            email: value.email,
            email_verified: value.email_verified,
        }
    }
}
impl From<&OAuthIdentity> for AuthOAuthIdentityRecordDto {
    fn from(value: &OAuthIdentity) -> Self {
        value.clone().into()
    }
}
impl std::fmt::Debug for AuthOAuthIdentityRecordDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthOAuthIdentityRecordDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthOAuthAuthorizationRecordDto {
    pub provider: AuthIdentityProviderDto,
    pub state: String,
    pub redirect_uri: String,
    pub code_verifier: String,
    pub code_challenge: String,
    pub nonce: String,
    pub expires_at: DateTime<Utc>,
}
impl From<OAuthAuthorizationTransaction> for AuthOAuthAuthorizationRecordDto {
    fn from(value: OAuthAuthorizationTransaction) -> Self {
        Self {
            provider: value.provider.into(),
            state: value.state,
            redirect_uri: value.redirect_uri,
            code_verifier: value.code_verifier,
            code_challenge: value.code_challenge,
            nonce: value.nonce,
            expires_at: value.expires_at,
        }
    }
}
impl From<AuthOAuthAuthorizationRecordDto> for OAuthAuthorizationTransaction {
    fn from(value: AuthOAuthAuthorizationRecordDto) -> Self {
        Self {
            provider: value.provider.into(),
            state: value.state,
            redirect_uri: value.redirect_uri,
            code_verifier: value.code_verifier,
            code_challenge: value.code_challenge,
            nonce: value.nonce,
            expires_at: value.expires_at,
        }
    }
}
impl From<&OAuthAuthorizationTransaction> for AuthOAuthAuthorizationRecordDto {
    fn from(value: &OAuthAuthorizationTransaction) -> Self {
        value.clone().into()
    }
}
impl std::fmt::Debug for AuthOAuthAuthorizationRecordDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthOAuthAuthorizationRecordDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct AuthConsumedOAuthAuthorizationDto {
    pub provider: AuthIdentityProviderDto,
    pub state: String,
    pub redirect_uri: String,
    pub code_verifier: String,
    pub nonce: String,
}
impl From<ConsumedOAuthAuthorization> for AuthConsumedOAuthAuthorizationDto {
    fn from(value: ConsumedOAuthAuthorization) -> Self {
        Self {
            provider: value.provider.into(),
            state: value.state,
            redirect_uri: value.redirect_uri,
            code_verifier: value.code_verifier,
            nonce: value.nonce,
        }
    }
}
impl From<AuthConsumedOAuthAuthorizationDto> for ConsumedOAuthAuthorization {
    fn from(value: AuthConsumedOAuthAuthorizationDto) -> Self {
        Self {
            provider: value.provider.into(),
            state: value.state,
            redirect_uri: value.redirect_uri,
            code_verifier: value.code_verifier,
            nonce: value.nonce,
        }
    }
}
impl From<&ConsumedOAuthAuthorization> for AuthConsumedOAuthAuthorizationDto {
    fn from(value: &ConsumedOAuthAuthorization) -> Self {
        value.clone().into()
    }
}
impl std::fmt::Debug for AuthConsumedOAuthAuthorizationDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthConsumedOAuthAuthorizationDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

/// Lifecycle transfer preserves the distinction between rotation, logout and revocation.
#[derive(Clone, PartialEq, Eq)]
pub enum AuthRefreshStateDto {
    Rotated {
        successor_jti: String,
        rotated_at: DateTime<Utc>,
        family_id: Uuid,
        user_id: Uuid,
    },
    RevokedByLogout,
    Revoked,
}
impl From<auth_domain::session::RefreshTokenState> for AuthRefreshStateDto {
    fn from(value: auth_domain::session::RefreshTokenState) -> Self {
        match value {
            auth_domain::session::RefreshTokenState::Rotated {
                successor_jti,
                rotated_at,
                family_id,
                user_id,
            } => Self::Rotated {
                successor_jti,
                rotated_at,
                family_id,
                user_id,
            },
            auth_domain::session::RefreshTokenState::RevokedByLogout => Self::RevokedByLogout,
            auth_domain::session::RefreshTokenState::Revoked => Self::Revoked,
        }
    }
}
impl From<AuthRefreshStateDto> for auth_domain::session::RefreshTokenState {
    fn from(value: AuthRefreshStateDto) -> Self {
        match value {
            AuthRefreshStateDto::Rotated {
                successor_jti,
                rotated_at,
                family_id,
                user_id,
            } => Self::Rotated {
                successor_jti,
                rotated_at,
                family_id,
                user_id,
            },
            AuthRefreshStateDto::RevokedByLogout => Self::RevokedByLogout,
            AuthRefreshStateDto::Revoked => Self::Revoked,
        }
    }
}
impl std::fmt::Debug for AuthRefreshStateDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthRefreshStateDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}
#[derive(Clone, PartialEq, Eq)]
pub struct AuthAuditEventRecordDto {
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
impl From<auth_domain::session::AuthAuditEvent> for AuthAuditEventRecordDto {
    fn from(value: auth_domain::session::AuthAuditEvent) -> Self {
        Self {
            id: value.id,
            actor_id: value.actor_id,
            subject_user_id: value.subject_user_id,
            action: value.action,
            outcome: value.outcome,
            request_id: value.request_id,
            ip_address: value.ip_address,
            user_agent: value.user_agent,
            metadata_json: value.metadata_json,
            occurred_at: value.occurred_at,
        }
    }
}
impl From<AuthAuditEventRecordDto> for auth_domain::session::AuthAuditEvent {
    fn from(value: AuthAuditEventRecordDto) -> Self {
        Self {
            id: value.id,
            actor_id: value.actor_id,
            subject_user_id: value.subject_user_id,
            action: value.action,
            outcome: value.outcome,
            request_id: value.request_id,
            ip_address: value.ip_address,
            user_agent: value.user_agent,
            metadata_json: value.metadata_json,
            occurred_at: value.occurred_at,
        }
    }
}
impl From<&auth_domain::session::AuthAuditEvent> for AuthAuditEventRecordDto {
    fn from(value: &auth_domain::session::AuthAuditEvent) -> Self {
        value.clone().into()
    }
}
impl std::fmt::Debug for AuthAuditEventRecordDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthAuditEventRecordDto")
            .field("fields", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn instant() -> DateTime<Utc> {
        DateTime::from_timestamp(1_700_000_000, 987_654_321).unwrap()
    }

    #[test]
    fn account_and_provider_snapshots_preserve_admission_fields_without_normalization() {
        for status in [
            UserStatus::PendingVerification,
            UserStatus::Active,
            UserStatus::Deleted,
        ] {
            let original = User {
                id: Uuid::from_u128(1),
                primary_email: " MixedCase@Example.test ".into(),
                password_hash: "private-credential-fingerprint".into(),
                status,
                last_login_at: Some(instant()),
            };
            let dto = AuthUserRecordDto::from(&original);
            assert_eq!(User::from(dto), original);
        }
        for provider in [
            IdentityProvider::Email,
            IdentityProvider::Google,
            IdentityProvider::Apple,
            IdentityProvider::X,
        ] {
            for email_verified in [false, true] {
                let proof = OAuthUserInfo {
                    provider,
                    subject: "private-exact-provider-subject".into(),
                    email: " CaseSensitive@Example.test ".into(),
                    email_verified,
                };
                assert_eq!(OAuthUserInfo::from(AuthOAuthProofDto::from(&proof)), proof);
            }
        }
    }

    #[test]
    fn atomic_transfer_preserves_family_expiry_audit_credit_and_rotation_race_evidence() {
        let refresh = RefreshTokenRecord {
            jti: "private-predecessor-jti".into(),
            token_hash: "private-token-fingerprint".into(),
            user_id: Uuid::from_u128(2),
            family_id: Uuid::from_u128(3),
            expires_at: instant(),
        };
        assert_eq!(
            RefreshTokenRecord::from(AuthRefreshRecordDto::from(&refresh)),
            refresh
        );
        let reset = PasswordResetToken {
            user_id: refresh.user_id,
            token_hash: "private-reset-fingerprint".into(),
            expires_at: instant(),
        };
        assert_eq!(
            PasswordResetToken::from(AuthPasswordResetRecordDto::from(&reset)),
            reset
        );
        for success in [false, true] {
            for credential_attempt in [false, true] {
                let audit = LoginAudit {
                    id: Uuid::from_u128(4),
                    user_id: refresh.user_id,
                    ip_address: Some("private-address".into()),
                    user_agent: Some("private-agent".into()),
                    success,
                    credential_attempt,
                    occurred_at: instant(),
                };
                assert_eq!(
                    LoginAudit::from(AuthLoginAuditRecordDto::from(&audit)),
                    audit
                );
            }
        }
        for outcome in [
            RotationOutcome::Rotated,
            RotationOutcome::AlreadyRotated {
                successor_jti: "private-successor-jti".into(),
                rotated_at: instant(),
            },
            RotationOutcome::NotRotatable,
        ] {
            assert_eq!(
                RotationOutcome::from(AuthRotationOutcomeDto::from(outcome.clone())),
                outcome
            );
        }
    }

    #[test]
    fn transfer_debug_never_renders_credentials_personal_data_or_rotation_identifiers() {
        let user = User {
            id: Uuid::from_u128(1),
            primary_email: "private-email".into(),
            password_hash: "private-password".into(),
            status: UserStatus::Active,
            last_login_at: None,
        };
        let proof = OAuthUserInfo {
            provider: IdentityProvider::Google,
            subject: "private-subject".into(),
            email: "private-email".into(),
            email_verified: true,
        };
        let refresh = RefreshTokenRecord {
            jti: "private-jti".into(),
            token_hash: "private-token".into(),
            user_id: user.id,
            family_id: Uuid::from_u128(2),
            expires_at: instant(),
        };
        let reset = PasswordResetToken {
            user_id: user.id,
            token_hash: "private-reset".into(),
            expires_at: instant(),
        };
        let audit = LoginAudit {
            id: Uuid::from_u128(3),
            user_id: user.id,
            ip_address: Some("private-ip".into()),
            user_agent: Some("private-agent".into()),
            success: false,
            credential_attempt: false,
            occurred_at: instant(),
        };
        let outcome = AuthRotationOutcomeDto::AlreadyRotated {
            successor_jti: "private-successor".into(),
            rotated_at: instant(),
        };
        let rendered = format!(
            "{:?} {:?} {:?} {:?} {:?} {:?}",
            AuthUserRecordDto::from(&user),
            AuthOAuthProofDto::from(&proof),
            AuthRefreshRecordDto::from(&refresh),
            AuthPasswordResetRecordDto::from(&reset),
            AuthLoginAuditRecordDto::from(&audit),
            outcome
        );
        assert!(!rendered.contains("private-"), "{rendered}");
        assert_eq!(rendered.matches("[REDACTED]").count(), 6);
    }
}

#[cfg(test)]
mod oauth_transfer_tests {
    use super::*;

    #[test]
    fn oauth_transfer_preserves_identity_binding_single_use_material_and_exact_expiry() {
        let expires_at = DateTime::from_timestamp(1_700_000_000, 123_456_789).unwrap();
        for provider in [
            IdentityProvider::Email,
            IdentityProvider::Google,
            IdentityProvider::Apple,
            IdentityProvider::X,
        ] {
            for email_verified in [false, true] {
                let identity = OAuthIdentity {
                    user_id: Uuid::from_u128(17),
                    provider,
                    subject: "CaseSensitive Provider Subject".into(),
                    email: " Unnormalized@Example.test ".into(),
                    email_verified,
                };
                assert_eq!(
                    OAuthIdentity::from(AuthOAuthIdentityRecordDto::from(&identity)),
                    identity
                );
            }
            let authorization = OAuthAuthorizationTransaction {
                provider,
                state: "CaseSensitive-State".into(),
                redirect_uri: "https://client.example.test/callback?opaque=%2F".into(),
                code_verifier: "Exact-Code-Verifier_~".into(),
                code_challenge: "Exact-Code-Challenge-".into(),
                nonce: "CaseSensitive-Nonce".into(),
                expires_at,
            };
            assert_eq!(
                OAuthAuthorizationTransaction::from(AuthOAuthAuthorizationRecordDto::from(
                    &authorization
                )),
                authorization
            );
            let consumed = ConsumedOAuthAuthorization {
                provider,
                state: authorization.state,
                redirect_uri: authorization.redirect_uri,
                code_verifier: authorization.code_verifier,
                nonce: authorization.nonce,
            };
            assert_eq!(
                ConsumedOAuthAuthorization::from(AuthConsumedOAuthAuthorizationDto::from(
                    &consumed
                )),
                consumed
            );
        }
    }

    #[test]
    fn oauth_transfer_debug_redacts_subject_email_state_redirect_pkce_and_nonce() {
        let identity = OAuthIdentity {
            user_id: Uuid::from_u128(17),
            provider: IdentityProvider::Google,
            subject: "private-subject".into(),
            email: "private-email".into(),
            email_verified: true,
        };
        let authorization = OAuthAuthorizationTransaction {
            provider: identity.provider,
            state: "private-state".into(),
            redirect_uri: "private-redirect".into(),
            code_verifier: "private-verifier".into(),
            code_challenge: "private-challenge".into(),
            nonce: "private-nonce".into(),
            expires_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
        };
        let consumed = ConsumedOAuthAuthorization {
            provider: identity.provider,
            state: authorization.state.clone(),
            redirect_uri: authorization.redirect_uri.clone(),
            code_verifier: authorization.code_verifier.clone(),
            nonce: authorization.nonce.clone(),
        };
        let rendered = format!(
            "{:?} {:?} {:?}",
            AuthOAuthIdentityRecordDto::from(&identity),
            AuthOAuthAuthorizationRecordDto::from(&authorization),
            AuthConsumedOAuthAuthorizationDto::from(&consumed)
        );
        assert!(!rendered.contains("private-"), "{rendered}");
        assert_eq!(rendered.matches("[REDACTED]").count(), 3);
    }
}

#[cfg(test)]
mod session_audit_transfer_tests {
    use super::*;
    use auth_domain::session::{AuthAuditEvent, RefreshTokenState};

    #[test]
    fn refresh_state_preserves_rotation_identity_time_and_revocation_reason() {
        for state in [
            RefreshTokenState::Rotated {
                successor_jti: "CaseSensitive-Successor_~".into(),
                rotated_at: DateTime::from_timestamp(1_700_000_000, 987_654_321).unwrap(),
                family_id: Uuid::from_u128(17),
                user_id: Uuid::from_u128(23),
            },
            RefreshTokenState::RevokedByLogout,
            RefreshTokenState::Revoked,
        ] {
            assert_eq!(
                RefreshTokenState::from(AuthRefreshStateDto::from(state.clone())),
                state
            );
        }
    }

    #[test]
    fn audit_transfer_preserves_absent_context_and_exact_metadata_without_parsing() {
        // Each optional field varies independently; metadata remains an opaque
        // string, including whitespace and malformed input for the repository.
        for flags in 0..32 {
            for metadata in [" {\"opaque\": \"%2F\", \"n\": 1} ", "", "not-json"] {
                let event = AuthAuditEvent {
                    id: Uuid::from_u128(31),
                    actor_id: (flags & 1 != 0).then_some(Uuid::from_u128(37)),
                    subject_user_id: (flags & 2 != 0).then_some(Uuid::from_u128(41)),
                    action: "CaseSensitive Action".into(),
                    outcome: "CaseSensitive Outcome".into(),
                    request_id: (flags & 4 != 0).then(|| " Exact-Request ".into()),
                    ip_address: (flags & 8 != 0).then(|| " Exact-Address ".into()),
                    user_agent: (flags & 16 != 0).then(|| " Exact-Agent ".into()),
                    metadata_json: metadata.into(),
                    occurred_at: DateTime::from_timestamp(1_700_000_000, 123_456_789).unwrap(),
                };
                assert_eq!(
                    AuthAuditEvent::from(AuthAuditEventRecordDto::from(&event)),
                    event
                );
            }
        }
    }

    #[test]
    fn session_and_audit_debug_redacts_identifiers_metadata_and_context() {
        let state = AuthRefreshStateDto::Rotated {
            successor_jti: "private-successor".into(),
            rotated_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
            family_id: Uuid::from_u128(17),
            user_id: Uuid::from_u128(23),
        };
        let event = AuthAuditEventRecordDto {
            id: Uuid::from_u128(31),
            actor_id: Some(Uuid::from_u128(37)),
            subject_user_id: Some(Uuid::from_u128(41)),
            action: "private-action".into(),
            outcome: "private-outcome".into(),
            request_id: Some("private-request".into()),
            ip_address: Some("private-address".into()),
            user_agent: Some("private-agent".into()),
            metadata_json: "private-metadata".into(),
            occurred_at: DateTime::from_timestamp(1_700_000_000, 0).unwrap(),
        };
        assert_eq!(
            format!("{state:?}"),
            "AuthRefreshStateDto { fields: \"[REDACTED]\" }"
        );
        assert_eq!(
            format!("{event:?}"),
            "AuthAuditEventRecordDto { fields: \"[REDACTED]\" }"
        );
    }
}
