use crate::application::dto::auth_dto::{DeleteUserDto, RetryUserDeletionDto};
use crate::config::AppState;
use std::sync::Arc;
use tonic::{Request, Response, Status};

pub mod proto {
    tonic::include_proto!("auth.v1");
}

#[derive(Clone)]
pub struct AuthGrpcServer {
    state: Arc<AppState>,
}

impl AuthGrpcServer {
    pub fn new(state: Arc<AppState>) -> Self {
        Self { state }
    }
}

#[tonic::async_trait]
impl proto::auth_service_server::AuthService for AuthGrpcServer {
    async fn signup_email(
        &self,
        request: Request<proto::SignupEmailRequest>,
    ) -> Result<Response<proto::SignupResponse>, Status> {
        let request = request.into_inner();
        let response = self
            .state
            .signup
            .signup_email(&request.email, &request.password)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::SignupResponse {
            user_id: response.user_id.to_string(),
            status: response.status,
        }))
    }

    async fn verify_email(
        &self,
        request: Request<proto::VerifyEmailRequest>,
    ) -> Result<Response<proto::SignupResponse>, Status> {
        let request = request.into_inner();
        let response = self
            .state
            .signup
            .verify_email(&request.email, &request.code)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::SignupResponse {
            user_id: response.user_id.to_string(),
            status: response.status,
        }))
    }

    async fn resend_verification(
        &self,
        request: Request<proto::ResendVerificationRequest>,
    ) -> Result<Response<proto::ResendVerificationResponse>, Status> {
        super::verification::resend(&self.state, request.into_inner()).await
    }

    async fn check_email(
        &self,
        request: Request<proto::CheckEmailRequest>,
    ) -> Result<Response<proto::EmailAvailabilityResponse>, Status> {
        let request = request.into_inner();
        let response = self
            .state
            .signup
            .check_email(&request.email)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::EmailAvailabilityResponse {
            email: response.email,
            available: response.available,
            status: response.status,
        }))
    }

    async fn login_email(
        &self,
        request: Request<proto::LoginEmailRequest>,
    ) -> Result<Response<proto::AuthResponse>, Status> {
        let request = request.into_inner();
        let response = self
            .state
            .login
            .login(
                &request.email,
                &request.password,
                empty_to_none(request.ip_address),
                empty_to_none(request.user_agent),
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(to_proto_auth(response)))
    }

    async fn refresh_tokens(
        &self,
        request: Request<proto::RefreshTokensRequest>,
    ) -> Result<Response<proto::AuthResponse>, Status> {
        let request = request.into_inner();
        let response = self
            .state
            .login
            .refresh(&request.refresh_token)
            .await
            .map_err(to_status)?;
        Ok(Response::new(to_proto_auth(response)))
    }

    async fn logout(
        &self,
        request: Request<proto::LogoutRequest>,
    ) -> Result<Response<proto::PasswordResetResponse>, Status> {
        self.state
            .login
            .logout(&request.into_inner().refresh_token)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::PasswordResetResponse {
            status: "LOGGED_OUT".into(),
        }))
    }

    async fn request_password_reset(
        &self,
        request: Request<proto::RequestPasswordResetRequest>,
    ) -> Result<Response<proto::PasswordResetResponse>, Status> {
        let response = self
            .state
            .password_reset
            .request_reset(&request.into_inner().email)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::PasswordResetResponse {
            status: response.status,
        }))
    }

    async fn confirm_password_reset(
        &self,
        request: Request<proto::ConfirmPasswordResetRequest>,
    ) -> Result<Response<proto::PasswordResetResponse>, Status> {
        let request = request.into_inner();
        let response = self
            .state
            .password_reset
            .confirm_reset(&request.token, &request.new_password)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::PasswordResetResponse {
            status: response.status,
        }))
    }

    async fn authorize_o_auth(
        &self,
        request: Request<proto::AuthorizeOAuthRequest>,
    ) -> Result<Response<proto::OAuthAuthorizeResponse>, Status> {
        let provider = parse_provider(request.into_inner().provider)?;
        let response = self
            .state
            .oauth_login
            .authorize(provider)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::OAuthAuthorizeResponse {
            authorization_uri: response.authorization_uri,
        }))
    }

    async fn o_auth_callback(
        &self,
        request: Request<proto::OAuthCallbackRequest>,
    ) -> Result<Response<proto::AuthResponse>, Status> {
        let request = request.into_inner();
        let response = self
            .state
            .oauth_login
            .oauth_callback(
                parse_provider(request.provider)?,
                &request.code,
                &request.state,
                None,
                empty_to_none(request.ip_address),
                empty_to_none(request.user_agent),
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(to_proto_auth(response)))
    }

    async fn link_o_auth_provider(
        &self,
        request: Request<proto::LinkOAuthProviderRequest>,
    ) -> Result<Response<proto::LinkedProvidersResponse>, Status> {
        let request = request.into_inner();
        let user_id = request
            .user_id
            .parse()
            .map_err(|_| Status::invalid_argument("User ID must be a UUID"))?;
        let response = self
            .state
            .oauth_link
            .link_provider(
                user_id,
                parse_provider(request.provider)?,
                &request.code,
                &request.state,
                None,
                None,
                None,
            )
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::LinkedProvidersResponse {
            user_id: response.user_id.to_string(),
            providers: response
                .providers
                .into_iter()
                .map(to_proto_provider)
                .collect(),
        }))
    }

    async fn delete_user(
        &self,
        request: Request<proto::DeleteUserRequest>,
    ) -> Result<Response<proto::DeleteUserResponse>, Status> {
        // A deletion is *requested* by the edge on the user's behalf; the
        // coordinator then fans cleanup out to the other services. Only the edge
        // may request it, and the use case additionally requires actor == subject.
        platform::grpc_auth::authorize_call(
            &request,
            platform::grpc_auth::RpcAction::UserMutation,
        )?;
        let request = request.into_inner();
        let response = self
            .state
            .delete_user
            .execute(DeleteUserDto {
                user_id: request.user_id,
                actor_id: request.actor_id,
                request_id: empty_to_none(request.request_id),
                ip_address: empty_to_none(request.ip_address),
                user_agent: empty_to_none(request.user_agent),
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::DeleteUserResponse {
            user_id: response.user_id,
            deleted: response.deleted,
            saga_id: response.saga_id,
            saga: Some(to_proto_deletion_saga(response.saga)),
        }))
    }

    async fn get_user_deletion(
        &self,
        request: Request<proto::GetUserDeletionRequest>,
    ) -> Result<Response<proto::GetUserDeletionResponse>, Status> {
        let user_id = request
            .into_inner()
            .user_id
            .parse()
            .map_err(|_| Status::invalid_argument("User ID must be a UUID"))?;
        let saga = self
            .state
            .get_user_deletion
            .execute(user_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::GetUserDeletionResponse {
            found: saga.is_some(),
            saga: saga.map(to_proto_deletion_saga),
        }))
    }

    async fn get_user_deletion_by_saga_id(
        &self,
        request: Request<proto::GetUserDeletionBySagaIdRequest>,
    ) -> Result<Response<proto::GetUserDeletionBySagaIdResponse>, Status> {
        platform::grpc_auth::authorize_call(
            &request,
            platform::grpc_auth::RpcAction::InspectUserDeletions,
        )?;
        let saga_id = request
            .into_inner()
            .saga_id
            .parse()
            .map_err(|_| Status::invalid_argument("Saga ID must be a UUID"))?;
        let saga = self
            .state
            .get_user_deletion
            .execute_by_saga_id(saga_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::GetUserDeletionBySagaIdResponse {
            found: saga.is_some(),
            saga: saga.map(to_proto_deletion_saga),
        }))
    }

    async fn list_dead_lettered_user_deletions(
        &self,
        request: Request<proto::ListDeadLetteredUserDeletionsRequest>,
    ) -> Result<Response<proto::ListDeadLetteredUserDeletionsResponse>, Status> {
        platform::grpc_auth::authorize_call(
            &request,
            platform::grpc_auth::RpcAction::InspectUserDeletions,
        )?;
        let request = request.into_inner();
        let after = parse_deletion_cursor(&request.after_updated_at, &request.after_saga_id)?;
        let page = self
            .state
            .list_dead_lettered_user_deletions
            .execute(request.limit, after)
            .await
            .map_err(to_status)?;
        let next = page.next_cursor;
        Ok(Response::new(
            proto::ListDeadLetteredUserDeletionsResponse {
                sagas: page.sagas.into_iter().map(to_proto_deletion_saga).collect(),
                has_more: next.is_some(),
                next_updated_at: next
                    .as_ref()
                    .map(|cursor| cursor.updated_at.to_rfc3339())
                    .unwrap_or_default(),
                next_saga_id: next.map(|cursor| cursor.saga_id).unwrap_or_default(),
            },
        ))
    }

    async fn retry_user_deletion(
        &self,
        request: Request<proto::RetryUserDeletionRequest>,
    ) -> Result<Response<proto::RetryUserDeletionResponse>, Status> {
        platform::grpc_auth::authorize_call(
            &request,
            platform::grpc_auth::RpcAction::RetryUserDeletion,
        )?;
        let request = request.into_inner();
        let response = self
            .state
            .retry_user_deletion
            .execute(RetryUserDeletionDto {
                saga_id: request.saga_id,
                actor_id: request.actor_id,
                resolution_id: request.resolution_id,
                reason: request.reason,
                evidence_reference: request.evidence_reference,
            })
            .await
            .map_err(to_status)?;
        Ok(Response::new(proto::RetryUserDeletionResponse {
            saga: Some(to_proto_deletion_saga(response.saga)),
            resolution_id: response.resolution_id,
            accepted: response.accepted,
        }))
    }

    async fn get_signing_keys(
        &self,
        request: Request<proto::GetSigningKeysRequest>,
    ) -> Result<Response<proto::GetSigningKeysResponse>, Status> {
        // The keys are public, but a verifier must be able to trust where they
        // came from. Serving them only to registered callers over the internal
        // channel means a network position alone cannot substitute a key.
        platform::grpc_auth::authorize_call(&request, platform::grpc_auth::RpcAction::Read)?;
        Ok(Response::new(proto::GetSigningKeysResponse {
            jwks_json: crate::adapters::r#in::jwks::public_jwks(&self.state.login.jwk_set())
                .to_string(),
        }))
    }

    async fn check_session(
        &self,
        request: Request<proto::CheckSessionRequest>,
    ) -> Result<Response<proto::CheckSessionResponse>, Status> {
        // Only registered callers may learn whether a session is live; the
        // answer is what lets them refuse a revoked session's high-risk work.
        platform::grpc_auth::authorize_call(&request, platform::grpc_auth::RpcAction::Read)?;
        let request = request.into_inner();
        let user_id = request
            .user_id
            .parse()
            .map_err(|_| Status::invalid_argument("User ID must be a UUID"))?;
        let session_id = request
            .session_id
            .parse()
            .map_err(|_| Status::invalid_argument("Session ID must be a UUID"))?;
        let status = self
            .state
            .login
            .session_status(user_id, session_id)
            .await
            .map_err(to_status)?;
        Ok(Response::new(match status {
            crate::domain::session::SessionStatus::Active { roles } => {
                proto::CheckSessionResponse {
                    active: true,
                    roles,
                }
            }
            crate::domain::session::SessionStatus::Ended => proto::CheckSessionResponse {
                active: false,
                roles: Vec::new(),
            },
        }))
    }
}

fn to_proto_deletion_saga(
    saga: crate::application::dto::auth_dto::UserDeletionSagaDto,
) -> proto::UserDeletionSaga {
    proto::UserDeletionSaga {
        id: saga.id,
        user_id: saga.user_id,
        status: saga.status,
        retention_until: saga.retention_until.to_rfc3339(),
        created_at: saga.created_at.to_rfc3339(),
        updated_at: saga.updated_at.to_rfc3339(),
        completed_at: saga
            .completed_at
            .map(|value| value.to_rfc3339())
            .unwrap_or_default(),
        steps: saga
            .steps
            .into_iter()
            .map(|step| proto::UserDeletionStep {
                target: step.target,
                status: step.status,
                attempt_count: step.attempt_count,
                next_attempt_at: step.next_attempt_at.to_rfc3339(),
                last_error: step.last_error.unwrap_or_default(),
                completed_at: step
                    .completed_at
                    .map(|value| value.to_rfc3339())
                    .unwrap_or_default(),
            })
            .collect(),
    }
}

fn parse_deletion_cursor(
    created_at: &str,
    saga_id: &str,
) -> Result<Option<(chrono::DateTime<chrono::Utc>, uuid::Uuid)>, Status> {
    match (created_at.trim().is_empty(), saga_id.trim().is_empty()) {
        (true, true) => Ok(None),
        (false, false) => {
            let created_at = chrono::DateTime::parse_from_rfc3339(created_at)
                .map_err(|_| Status::invalid_argument("Cursor timestamp must be RFC3339"))?
                .with_timezone(&chrono::Utc);
            let saga_id = saga_id
                .parse()
                .map_err(|_| Status::invalid_argument("Cursor saga ID must be a UUID"))?;
            Ok(Some((created_at, saga_id)))
        }
        _ => Err(Status::invalid_argument(
            "Cursor timestamp and saga ID must be provided together",
        )),
    }
}

/// Map an application failure to a gRPC status with a fixed public message.
///
/// Nothing derived from internal error text is returned or logged here. The
/// stable failure code is recorded at the edge. Unknown
/// failures become `Internal` rather than a client error, which also keeps
/// outage telemetry from counting them as caller mistakes.
fn to_status(error: crate::application::errors::AuthError) -> Status {
    use crate::application::presentation::{present_auth_error, AuthFailureKind};
    let projection = present_auth_error(Some(&error));
    match projection.kind {
        AuthFailureKind::Credential => {
            tracing::info!(code = projection.code, "credential check failed");
            Status::unauthenticated(projection.message)
        }
        AuthFailureKind::InvalidInput => {
            tracing::info!(code = projection.code, "auth input rejected");
            Status::invalid_argument(projection.message)
        }
        AuthFailureKind::Denied => {
            tracing::warn!(code = projection.code, "auth action forbidden");
            Status::permission_denied(projection.message)
        }
        AuthFailureKind::Missing => Status::not_found(projection.message),
        AuthFailureKind::Conflict => Status::already_exists(projection.message),
        AuthFailureKind::RefreshConflict => Status::aborted(projection.message),
        AuthFailureKind::RateLimited => Status::resource_exhausted(projection.message),
        AuthFailureKind::Busy => Status::unavailable(projection.message),
        AuthFailureKind::DependencyUnavailable => {
            tracing::error!(code = projection.code, "auth dependency unavailable");
            Status::unavailable(projection.message)
        }
        AuthFailureKind::Internal | AuthFailureKind::ProviderFailure => {
            tracing::error!(code = projection.code, "auth request failed");
            Status::internal(projection.message)
        }
    }
}

fn empty_to_none(value: String) -> Option<String> {
    if value.is_empty() {
        None
    } else {
        Some(value)
    }
}

#[allow(clippy::result_large_err)]
fn parse_provider(value: i32) -> Result<crate::domain::user::IdentityProvider, Status> {
    match proto::IdentityProvider::try_from(value).ok() {
        Some(proto::IdentityProvider::Google) => Ok(crate::domain::user::IdentityProvider::Google),
        Some(proto::IdentityProvider::Apple) => Ok(crate::domain::user::IdentityProvider::Apple),
        Some(proto::IdentityProvider::X) => Ok(crate::domain::user::IdentityProvider::X),
        Some(proto::IdentityProvider::Email) => {
            Err(Status::invalid_argument("OAuth provider required"))
        }
        _ => Err(Status::invalid_argument("Identity provider is required")),
    }
}

fn to_proto_provider(provider: crate::domain::user::IdentityProvider) -> i32 {
    match provider {
        crate::domain::user::IdentityProvider::Email => proto::IdentityProvider::Email as i32,
        crate::domain::user::IdentityProvider::Google => proto::IdentityProvider::Google as i32,
        crate::domain::user::IdentityProvider::Apple => proto::IdentityProvider::Apple as i32,
        crate::domain::user::IdentityProvider::X => proto::IdentityProvider::X as i32,
    }
}

fn to_proto_auth(
    response: crate::application::dto::auth_dto::AuthResponseDto,
) -> proto::AuthResponse {
    proto::AuthResponse {
        user_id: response.user_id.to_string(),
        access_token: response.access_token,
        refresh_token: response.refresh_token,
        token_type: response.token_type,
        expires_in_seconds: response.expires_in_seconds,
        linked_providers: response
            .linked_providers
            .into_iter()
            .map(to_proto_provider)
            .collect(),
    }
}

#[cfg(test)]
mod presentation_tests {
    use super::to_status;
    use crate::application::errors::AuthError;
    use tonic::Code;

    #[test]
    fn auth_wire_matrix_keeps_codes_messages_and_unknown_cause_redaction() {
        for (error, code, message) in [
            (
                AuthError::InvalidCredentials,
                Code::Unauthenticated,
                "AUTH_INVALID_CREDENTIALS",
            ),
            (
                AuthError::InvalidInput("Invalid email"),
                Code::InvalidArgument,
                "Invalid email",
            ),
            (
                AuthError::Forbidden,
                Code::PermissionDenied,
                "AUTH_FORBIDDEN",
            ),
            (AuthError::NotFound, Code::NotFound, "AUTH_NOT_FOUND"),
            (
                AuthError::Invariant("fixed invariant"),
                Code::Internal,
                "AUTH_INTERNAL",
            ),
            (
                AuthError::Conflict("Email already registered"),
                Code::AlreadyExists,
                "Email already registered",
            ),
            (
                AuthError::RefreshConflict,
                Code::Aborted,
                "AUTH_REFRESH_CONFLICT",
            ),
            (
                AuthError::CredentialRejected,
                Code::Unauthenticated,
                "AUTH_CREDENTIAL_REJECTED",
            ),
            (
                AuthError::RateLimited,
                Code::ResourceExhausted,
                "AUTH_RATE_LIMITED",
            ),
            (AuthError::Busy, Code::Unavailable, "AUTH_BUSY"),
            (
                AuthError::DependencyUnavailable,
                Code::Unavailable,
                "AUTH_DEPENDENCY_UNAVAILABLE",
            ),
            (AuthError::Unexpected, Code::Internal, "AUTH_INTERNAL"),
        ] {
            let status = to_status(error);
            assert_eq!(status.code(), code);
            assert_eq!(status.message(), message);
            assert!(!format!("{status:?}").contains("private"));
        }
        let provider = to_status(AuthError::ProviderFailure(
            auth_domain::oauth_failure::OAuthProviderFailure::code_invalid(),
        ));
        assert_eq!(provider.code(), Code::Internal);
        assert_eq!(provider.message(), "AUTH_INTERNAL");
        let unknown = to_status(AuthError::Unexpected);
        assert_eq!(unknown.code(), Code::Internal);
        assert_eq!(unknown.message(), "AUTH_INTERNAL");
        let dependency = to_status(AuthError::DependencyUnavailable);
        assert_eq!(dependency.code(), Code::Unavailable);
        assert_eq!(dependency.message(), "AUTH_DEPENDENCY_UNAVAILABLE");
    }
}
