use chrono::{DateTime, Utc};
use gateway_domain::IdentityProvider;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

macro_rules! redacted_debug {
    ($($type:ident { $($field:ident),+ $(,)? }),+ $(,)?) => {
        $(
            impl std::fmt::Debug for $type {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    let mut debug = formatter.debug_struct(stringify!($type));
                    $(debug.field(stringify!($field), &"[REDACTED]");)+
                    debug.finish()
                }
            }
        )+
    };
}

/// Neutral authorization result. URI parsing belongs to boundary adapters.
#[derive(Clone, PartialEq, Eq)]
pub struct OAuthAuthorizationDto {
    pub authorization_uri: String,
}

impl std::fmt::Debug for OAuthAuthorizationDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("OAuthAuthorizationDto")
            .field("authorization_uri", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmailSignupRequest {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmailVerifyRequest {
    pub email: String,
    pub code: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmailCheckRequest {
    pub email: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequest {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OAuthLinkRequest {
    pub code: String,
    pub state: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignupResponse {
    pub user_id: Uuid,
    pub status: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmailAvailabilityResponse {
    pub email: String,
    pub available: bool,
    pub status: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PasswordResetRequest {
    pub email: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PasswordResetConfirmRequest {
    pub token: String,
    pub new_password: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PasswordResetResponse {
    pub status: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponse {
    pub user_id: Uuid,
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    #[serde(rename = "expiresIn")]
    pub expires_in: i64,
    pub linked_providers: Vec<IdentityProvider>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LinkedProvidersResponse {
    pub user_id: Uuid,
    #[serde(alias = "linkedProviders")]
    pub providers: Vec<IdentityProvider>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserDeletionStepResponse {
    pub target: String,
    pub status: String,
    pub attempt_count: i32,
    pub next_attempt_at: String,
    pub last_error: Option<String>,
    pub completed_at: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserDeletionSagaResponse {
    pub id: String,
    pub user_id: String,
    pub status: String,
    pub retention_until: String,
    pub created_at: String,
    pub updated_at: String,
    pub completed_at: Option<String>,
    pub steps: Vec<UserDeletionStepResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserDeletionSagaCursorResponse {
    pub updated_at: DateTime<Utc>,
    pub saga_id: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeadLetteredUserDeletionListResponse {
    pub sagas: Vec<UserDeletionSagaResponse>,
    pub next_cursor: Option<UserDeletionSagaCursorResponse>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteAccountResponse {
    pub user_id: String,
    pub deleted: bool,
    pub saga_id: String,
    pub saga: UserDeletionSagaResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RetryUserDeletionRequest {
    pub resolution_id: Uuid,
    pub reason: String,
    pub evidence_reference: String,
}

impl RetryUserDeletionRequest {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.resolution_id.is_nil() {
            return Err("Resolution ID must not be nil");
        }
        let reason = self.reason.trim();
        if !(20..=1024).contains(&reason.chars().count()) {
            return Err("Reason must contain between 20 and 1024 characters");
        }
        let evidence = self.evidence_reference.trim();
        if !(3..=256).contains(&evidence.len())
            || !evidence.is_ascii()
            || !evidence.chars().all(|character| {
                character.is_ascii_alphanumeric()
                    || matches!(character, '.' | '_' | ':' | '/' | '#' | '-')
            })
        {
            return Err("Evidence reference must identify a protected record");
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RetryUserDeletionRequestDto {
    pub saga_id: String,
    pub resolution_id: String,
    pub actor_id: String,
    pub reason: String,
    pub evidence_reference: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RetryUserDeletionAcceptedResponse {
    pub resolution_id: String,
    pub accepted: bool,
    pub saga: UserDeletionSagaResponse,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct ErrorResponse {
    pub code: String,
    pub message: String,
    pub trace_id: Option<String>,
    pub timestamp: Option<DateTime<Utc>>,
    pub details: Option<Value>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ShadowComparisonResponse {
    pub scenario_id: String,
    pub diff_category: String,
    pub source_status: u16,
    pub target_status: u16,
    pub database_state_match: bool,
}

redacted_debug! {
    EmailSignupRequest { email, password },
    EmailVerifyRequest { email, code },
    EmailCheckRequest { email },
    LoginRequest { email, password },
    OAuthLinkRequest { code, state },
    EmailAvailabilityResponse { email },
    PasswordResetRequest { email },
    PasswordResetConfirmRequest { token, new_password },
    AuthResponse { access_token, refresh_token },
}

#[cfg(test)]
mod secret_debug_tests {
    use super::{
        AuthResponse, EmailAvailabilityResponse, EmailCheckRequest, EmailSignupRequest,
        EmailVerifyRequest, LoginRequest, OAuthLinkRequest, PasswordResetConfirmRequest,
        PasswordResetRequest,
    };
    use gateway_domain::IdentityProvider;
    use uuid::Uuid;

    #[test]
    fn gateway_auth_dto_debug_does_not_render_credentials_or_personal_identifiers() {
        let rendered = [
            format!(
                "{:?}",
                EmailSignupRequest {
                    email: "private-gateway-email@example.test".into(),
                    password: "signup-password-secret".into(),
                }
            ),
            format!(
                "{:?}",
                EmailVerifyRequest {
                    email: "private-gateway-email@example.test".into(),
                    code: "verification-code-secret".into(),
                }
            ),
            format!(
                "{:?}",
                EmailCheckRequest {
                    email: "private-gateway-email@example.test".into(),
                }
            ),
            format!(
                "{:?}",
                EmailAvailabilityResponse {
                    email: "private-gateway-email@example.test".into(),
                    available: false,
                    status: "UNAVAILABLE".into(),
                }
            ),
            format!(
                "{:?}",
                LoginRequest {
                    email: "private-gateway-email@example.test".into(),
                    password: "login-password-secret".into(),
                }
            ),
            format!(
                "{:?}",
                OAuthLinkRequest {
                    code: "oauth-code-secret".into(),
                    state: "oauth-state-secret".into(),
                }
            ),
            format!(
                "{:?}",
                PasswordResetRequest {
                    email: "private-gateway-email@example.test".into(),
                }
            ),
            format!(
                "{:?}",
                PasswordResetConfirmRequest {
                    token: "password-reset-token-secret".into(),
                    new_password: "new-password-secret".into(),
                }
            ),
            format!(
                "{:?}",
                AuthResponse {
                    user_id: Uuid::new_v4(),
                    access_token: "issued-access-token-secret".into(),
                    refresh_token: "issued-refresh-token-secret".into(),
                    token_type: "Bearer".into(),
                    expires_in: 900,
                    linked_providers: vec![IdentityProvider::Google],
                }
            ),
        ];

        for value in &rendered {
            for secret in [
                "private-gateway-email@example.test",
                "signup-password-secret",
                "verification-code-secret",
                "login-password-secret",
                "oauth-code-secret",
                "oauth-state-secret",
                "password-reset-token-secret",
                "new-password-secret",
                "issued-access-token-secret",
                "issued-refresh-token-secret",
            ] {
                assert!(!value.contains(secret), "Debug leaked {secret}: {value}");
            }
            assert!(value.contains("[REDACTED]"), "{value}");
        }
    }
}
