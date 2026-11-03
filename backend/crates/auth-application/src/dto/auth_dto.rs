use auth_domain::user::IdentityProvider;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
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

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignupRequestDto {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct VerifyEmailRequestDto {
    pub email: String,
    pub code: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmailCheckRequestDto {
    pub email: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LoginRequestDto {
    pub email: String,
    pub password: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RefreshRequestDto {
    pub refresh_token: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LogoutRequestDto {
    pub refresh_token: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PasswordResetRequestDto {
    pub email: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PasswordResetConfirmRequestDto {
    pub token: String,
    pub new_password: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OAuthLinkRequestDto {
    pub code: String,
    pub state: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct SignupResponseDto {
    pub user_id: Uuid,
    pub status: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EmailAvailabilityResponseDto {
    pub email: String,
    pub available: bool,
    pub status: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct AuthResponseDto {
    pub user_id: Uuid,
    pub access_token: String,
    pub refresh_token: String,
    pub token_type: String,
    pub expires_in_seconds: i64,
    pub linked_providers: Vec<IdentityProvider>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct PasswordResetResponseDto {
    pub status: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct OAuthAuthorizeResponseDto {
    pub authorization_uri: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct LinkedProvidersResponseDto {
    pub user_id: Uuid,
    #[serde(alias = "linkedProviders")]
    pub providers: Vec<IdentityProvider>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteUserDto {
    pub user_id: String,
    pub actor_id: String,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeleteUserResponseDto {
    pub user_id: String,
    pub deleted: bool,
    pub saga_id: String,
    pub saga: UserDeletionSagaDto,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RetryUserDeletionDto {
    pub saga_id: String,
    pub resolution_id: String,
    pub actor_id: String,
    pub reason: String,
    pub evidence_reference: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct RetryUserDeletionResponseDto {
    pub resolution_id: String,
    pub accepted: bool,
    pub saga: UserDeletionSagaDto,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserDeletionStepDto {
    pub target: String,
    pub status: String,
    pub attempt_count: i32,
    pub next_attempt_at: DateTime<Utc>,
    pub last_error: Option<String>,
    pub completed_at: Option<DateTime<Utc>>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct UserDeletionSagaDto {
    pub id: String,
    pub user_id: String,
    pub status: String,
    pub retention_until: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub completed_at: Option<DateTime<Utc>>,
    pub steps: Vec<UserDeletionStepDto>,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeadLetteredUserDeletionCursorDto {
    pub updated_at: DateTime<Utc>,
    pub saga_id: String,
}

#[derive(Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct DeadLetteredUserDeletionListDto {
    pub sagas: Vec<UserDeletionSagaDto>,
    pub next_cursor: Option<DeadLetteredUserDeletionCursorDto>,
}

impl From<auth_domain::user_deletion::UserDeletionSaga> for UserDeletionSagaDto {
    fn from(value: auth_domain::user_deletion::UserDeletionSaga) -> Self {
        Self {
            id: value.id.to_string(),
            user_id: value.user_id.to_string(),
            status: value.status.as_str().to_owned(),
            retention_until: value.retention_until,
            created_at: value.created_at,
            updated_at: value.updated_at,
            completed_at: value.completed_at,
            steps: value
                .steps
                .into_iter()
                .map(|step| UserDeletionStepDto {
                    target: step.target.as_str().to_owned(),
                    status: step.status.as_str().to_owned(),
                    attempt_count: step.attempt_count,
                    next_attempt_at: step.next_attempt_at,
                    last_error: step.last_error,
                    completed_at: step.completed_at,
                })
                .collect(),
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AuthLifecycleCleanupResult {
    pub deleted_email_verifications: i64,
    pub deleted_password_reset_tokens: i64,
    pub deleted_refresh_tokens: i64,
    pub deleted_oauth_authorizations: i64,
    pub deleted_users: i64,
}

redacted_debug! {
    SignupRequestDto { email, password },
    VerifyEmailRequestDto { email, code },
    EmailCheckRequestDto { email },
    LoginRequestDto { email, password },
    RefreshRequestDto { refresh_token },
    LogoutRequestDto { refresh_token },
    PasswordResetRequestDto { email },
    PasswordResetConfirmRequestDto { token, new_password },
    OAuthLinkRequestDto { code, state },
    EmailAvailabilityResponseDto { email },
    AuthResponseDto { access_token, refresh_token },
    OAuthAuthorizeResponseDto { authorization_uri },
    SignupResponseDto { user_id, status },
    LinkedProvidersResponseDto { user_id, providers },
    DeleteUserDto { user_id, actor_id, request_id, ip_address, user_agent },
    DeleteUserResponseDto { user_id, saga_id, saga },
    RetryUserDeletionDto { saga_id, resolution_id, actor_id, reason, evidence_reference },
    RetryUserDeletionResponseDto { resolution_id, saga },
    UserDeletionStepDto { target, status, attempt_count, next_attempt_at, last_error, completed_at },
    UserDeletionSagaDto { id, user_id, status, retention_until, created_at, updated_at, completed_at, steps },
    DeadLetteredUserDeletionCursorDto { updated_at, saga_id },
    DeadLetteredUserDeletionListDto { sagas, next_cursor },
}

#[cfg(test)]
mod secret_debug_tests {
    use super::{
        AuthResponseDto, EmailAvailabilityResponseDto, EmailCheckRequestDto, LoginRequestDto,
        LogoutRequestDto, OAuthAuthorizeResponseDto, OAuthLinkRequestDto,
        PasswordResetConfirmRequestDto, PasswordResetRequestDto, RefreshRequestDto,
        SignupRequestDto, VerifyEmailRequestDto,
    };
    use auth_domain::user::IdentityProvider;
    use uuid::Uuid;

    #[test]
    fn auth_dto_debug_does_not_render_credentials_or_oauth_state() {
        let rendered = [
            format!(
                "{:?}",
                SignupRequestDto {
                    email: "private-auth-email@example.test".into(),
                    password: "signup-password-secret".into(),
                }
            ),
            format!(
                "{:?}",
                VerifyEmailRequestDto {
                    email: "private-auth-email@example.test".into(),
                    code: "verification-code-secret".into(),
                }
            ),
            format!(
                "{:?}",
                EmailCheckRequestDto {
                    email: "private-auth-email@example.test".into(),
                }
            ),
            format!(
                "{:?}",
                EmailAvailabilityResponseDto {
                    email: "private-auth-email@example.test".into(),
                    available: false,
                    status: "UNAVAILABLE".into(),
                }
            ),
            format!(
                "{:?}",
                LoginRequestDto {
                    email: "private-auth-email@example.test".into(),
                    password: "login-password-secret".into(),
                }
            ),
            format!(
                "{:?}",
                RefreshRequestDto {
                    refresh_token: "refresh-token-secret".into(),
                }
            ),
            format!(
                "{:?}",
                LogoutRequestDto {
                    refresh_token: "logout-token-secret".into(),
                }
            ),
            format!(
                "{:?}",
                PasswordResetConfirmRequestDto {
                    token: "password-reset-token-secret".into(),
                    new_password: "new-password-secret".into(),
                }
            ),
            format!(
                "{:?}",
                PasswordResetRequestDto {
                    email: "private-auth-email@example.test".into(),
                }
            ),
            format!(
                "{:?}",
                OAuthLinkRequestDto {
                    code: "oauth-code-secret".into(),
                    state: "oauth-state-secret".into(),
                }
            ),
            format!(
                "{:?}",
                AuthResponseDto {
                    user_id: Uuid::new_v4(),
                    access_token: "issued-access-token-secret".into(),
                    refresh_token: "issued-refresh-token-secret".into(),
                    token_type: "Bearer".into(),
                    expires_in_seconds: 900,
                    linked_providers: vec![IdentityProvider::Google],
                }
            ),
            format!(
                "{:?}",
                OAuthAuthorizeResponseDto {
                    authorization_uri:
                        "https://idp.example.test/authorize?state=authorize-state-secret".into(),
                }
            ),
        ];

        for value in &rendered {
            for secret in [
                "private-auth-email@example.test",
                "signup-password-secret",
                "verification-code-secret",
                "login-password-secret",
                "refresh-token-secret",
                "logout-token-secret",
                "password-reset-token-secret",
                "new-password-secret",
                "oauth-code-secret",
                "oauth-state-secret",
                "issued-access-token-secret",
                "issued-refresh-token-secret",
                "authorize-state-secret",
            ] {
                assert!(!value.contains(secret), "Debug leaked {secret}: {value}");
            }
            assert!(value.contains("[REDACTED]"), "{value}");
        }
    }
}

#[cfg(test)]
mod user_deletion_debug_tests {
    use super::*;

    #[test]
    fn user_deletion_dto_debug_redacts_identity_reasons_and_worker_errors() {
        let now = Utc::now();
        let signup_user_id = Uuid::from_u128(0xabababababab4bab8bab000000000001);
        let signup_user_id_text = signup_user_id.to_string();
        let saga = UserDeletionSagaDto {
            id: "private-saga-id-cc73".into(),
            user_id: "private-user-id-07ab".into(),
            status: "DEAD_LETTERED".into(),
            retention_until: now,
            created_at: now,
            updated_at: now,
            completed_at: None,
            steps: vec![UserDeletionStepDto {
                target: "PROFILE".into(),
                status: "DEAD_LETTERED".into(),
                attempt_count: 8,
                next_attempt_at: now,
                last_error: Some("private-worker-error-835a".into()),
                completed_at: None,
            }],
        };
        let values = [
            format!(
                "{:?}",
                SignupResponseDto {
                    user_id: signup_user_id,
                    status: "private-signup-status".into(),
                }
            ),
            format!(
                "{:?}",
                LinkedProvidersResponseDto {
                    user_id: signup_user_id,
                    providers: vec![IdentityProvider::Google],
                }
            ),
            format!(
                "{:?}",
                DeleteUserDto {
                    user_id: "private-request-user-id".into(),
                    actor_id: "private-request-actor-id".into(),
                    request_id: Some("private-request-id".into()),
                    ip_address: Some("203.0.113.19".into()),
                    user_agent: Some("private-request-agent".into()),
                }
            ),
            format!(
                "{:?}",
                RetryUserDeletionDto {
                    saga_id: "private-retry-saga-id".into(),
                    resolution_id: "private-resolution-id".into(),
                    actor_id: "private-retry-actor-id".into(),
                    reason: "private-operator-reason".into(),
                    evidence_reference: "private-protected-evidence".into(),
                }
            ),
            format!(
                "{:?}",
                DeleteUserResponseDto {
                    user_id: "private-response-user-id".into(),
                    deleted: true,
                    saga_id: "private-response-saga-id".into(),
                    saga: saga.clone(),
                }
            ),
            format!(
                "{:?}",
                RetryUserDeletionResponseDto {
                    resolution_id: "private-response-resolution-id".into(),
                    accepted: true,
                    saga: saga.clone(),
                }
            ),
            format!(
                "{:?}",
                DeadLetteredUserDeletionListDto {
                    sagas: vec![saga.clone()],
                    next_cursor: Some(DeadLetteredUserDeletionCursorDto {
                        updated_at: now,
                        saga_id: "private-page-cursor-saga".into(),
                    }),
                }
            ),
        ];

        for rendered in values {
            for secret in [
                "private-",
                "203.0.113.19",
                "private-request-agent",
                signup_user_id_text.as_str(),
            ] {
                assert!(
                    !rendered.contains(secret),
                    "Debug leaked {secret}: {rendered}"
                );
            }
            assert!(rendered.contains("[REDACTED]"));
        }

        let serialized = serde_json::to_value(saga).unwrap();
        assert_eq!(serialized["userId"], "private-user-id-07ab");
        assert_eq!(
            serialized["steps"][0]["lastError"],
            "private-worker-error-835a"
        );
    }
}
