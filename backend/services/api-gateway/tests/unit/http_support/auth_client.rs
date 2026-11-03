use super::{DeleteAccountCall, LoginCall, SessionAnswer, TestClients, USER_ID};
use api_gateway::application::dto::auth_dto::{
    AuthResponse, DeadLetteredUserDeletionListResponse, DeleteAccountResponse,
    EmailAvailabilityResponse, EmailCheckRequest, EmailSignupRequest, EmailVerifyRequest,
    LinkedProvidersResponse, LoginRequest, OAuthLinkRequest, PasswordResetConfirmRequest,
    PasswordResetRequest, PasswordResetResponse, RetryUserDeletionRequestDto, SignupResponse,
    UserDeletionSagaCursorResponse, UserDeletionSagaResponse, UserDeletionStepResponse,
};
use api_gateway::application::dto::issuer_dto::{
    SessionAuthorityRequestDto, SessionStateDto as SessionState,
};
use api_gateway::application::ports::AuthClientPort;
use api_gateway::domain::IdentityProvider;
use async_trait::async_trait;
use uuid::Uuid;

impl TestClients {
    async fn signup_auth_client_port_inner(
        &self,
        _request: EmailSignupRequest,
    ) -> anyhow::Result<SignupResponse> {
        Ok(SignupResponse {
            user_id: Uuid::parse_str(USER_ID)?,
            status: "PENDING_VERIFICATION".into(),
        })
    }

    async fn verify_auth_client_port_inner(
        &self,
        _request: EmailVerifyRequest,
    ) -> anyhow::Result<SignupResponse> {
        Ok(SignupResponse {
            user_id: Uuid::parse_str(USER_ID)?,
            status: "ACTIVE".into(),
        })
    }

    async fn check_email_auth_client_port_inner(
        &self,
        request: EmailCheckRequest,
    ) -> anyhow::Result<EmailAvailabilityResponse> {
        Ok(EmailAvailabilityResponse {
            email: request.email,
            available: true,
            status: "AVAILABLE".into(),
        })
    }

    async fn request_password_reset_auth_client_port_inner(
        &self,
        _request: PasswordResetRequest,
    ) -> anyhow::Result<PasswordResetResponse> {
        Ok(PasswordResetResponse {
            status: "RESET_REQUESTED".into(),
        })
    }

    async fn confirm_password_reset_auth_client_port_inner(
        &self,
        _request: PasswordResetConfirmRequest,
    ) -> anyhow::Result<PasswordResetResponse> {
        Ok(PasswordResetResponse {
            status: "RESET_CONFIRMED".into(),
        })
    }

    async fn login_auth_client_port_inner(
        &self,
        request: LoginRequest,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> anyhow::Result<AuthResponse> {
        self.login_calls
            .lock()
            .expect("login calls")
            .push(LoginCall {
                email: request.email,
                ip_address,
                user_agent,
            });
        Ok(auth_response("refresh-from-login"))
    }

    async fn refresh_auth_client_port_inner(
        &self,
        refresh_token: String,
    ) -> anyhow::Result<AuthResponse> {
        self.refresh_calls
            .lock()
            .expect("refresh calls")
            .push(refresh_token);
        Ok(auth_response("refresh-from-refresh"))
    }

    async fn logout_auth_client_port_inner(&self, refresh_token: String) -> anyhow::Result<()> {
        self.logout_calls
            .lock()
            .expect("logout calls")
            .push(refresh_token);
        Ok(())
    }

    async fn authorize_auth_client_port_inner(
        &self,
        provider: IdentityProvider,
    ) -> anyhow::Result<api_gateway::application::dto::auth_dto::OAuthAuthorizationDto> {
        let state = match provider {
            IdentityProvider::Email => "email-state",
            IdentityProvider::Google => "google-state",
            IdentityProvider::Apple => "apple-state",
            IdentityProvider::X => "x-state",
        };
        Ok(
            api_gateway::application::dto::auth_dto::OAuthAuthorizationDto {
                authorization_uri: format!("https://accounts.example.test/oauth?state={state}")
                    .parse::<::http::Uri>()?
                    .to_string(),
            },
        )
    }

    async fn oauth_callback_auth_client_port_inner(
        &self,
        provider: IdentityProvider,
        _code: String,
        _state: String,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> anyhow::Result<AuthResponse> {
        self.oauth_callback_calls
            .lock()
            .expect("oauth callback calls")
            .push(provider);
        Ok(auth_response("refresh-from-oauth"))
    }

    async fn link_provider_auth_client_port_inner(
        &self,
        provider: IdentityProvider,
        _request: OAuthLinkRequest,
        _user_id: String,
    ) -> anyhow::Result<LinkedProvidersResponse> {
        Ok(LinkedProvidersResponse {
            user_id: Uuid::parse_str(USER_ID)?,
            providers: vec![IdentityProvider::Email, provider],
        })
    }

    async fn delete_account_auth_client_port_inner(
        &self,
        user_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> anyhow::Result<DeleteAccountResponse> {
        self.delete_account_calls
            .lock()
            .expect("delete account calls")
            .push(DeleteAccountCall {
                user_id: user_id.clone(),
                request_id,
                ip_address,
                user_agent,
            });
        Ok(DeleteAccountResponse {
            user_id: user_id.clone(),
            deleted: true,
            saga_id: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa".into(),
            saga: deletion_saga(user_id),
        })
    }

    async fn get_user_deletion_auth_client_port_inner(
        &self,
        user_id: String,
    ) -> anyhow::Result<Option<UserDeletionSagaResponse>> {
        self.get_deletion_calls
            .lock()
            .expect("get deletion calls")
            .push(user_id.clone());
        Ok(Some(deletion_saga(user_id)))
    }

    async fn get_user_deletion_by_saga_id_auth_client_port_inner(
        &self,
        saga_id: String,
    ) -> anyhow::Result<Option<UserDeletionSagaResponse>> {
        self.get_deletion_by_saga_id_calls
            .lock()
            .expect("get deletion by saga id calls")
            .push(saga_id.clone());
        let mut saga = deletion_saga(USER_ID.to_owned());
        saga.id = saga_id;
        Ok(Some(saga))
    }

    async fn list_dead_lettered_user_deletions_auth_client_port_inner(
        &self,
        limit: i32,
        after_updated_at: Option<chrono::DateTime<chrono::Utc>>,
        after_saga_id: Option<Uuid>,
    ) -> anyhow::Result<DeadLetteredUserDeletionListResponse> {
        self.list_dead_lettered_deletion_calls
            .lock()
            .expect("list dead-lettered deletion calls")
            .push((limit, after_updated_at, after_saga_id));
        let mut saga = deletion_saga(USER_ID.to_owned());
        saga.status = "DEAD_LETTERED".into();
        let updated_at =
            chrono::DateTime::parse_from_rfc3339(&saga.updated_at)?.with_timezone(&chrono::Utc);
        Ok(DeadLetteredUserDeletionListResponse {
            sagas: vec![saga],
            next_cursor: Some(UserDeletionSagaCursorResponse {
                updated_at,
                saga_id: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa".into(),
            }),
        })
    }

    async fn retry_user_deletion_auth_client_port_inner(
        &self,
        request: RetryUserDeletionRequestDto,
    ) -> anyhow::Result<UserDeletionSagaResponse> {
        self.retry_user_deletion_calls
            .lock()
            .expect("retry user deletion calls")
            .push(request.clone());
        let mut saga = deletion_saga(USER_ID.to_owned());
        saga.id = request.saga_id;
        Ok(saga)
    }

    async fn session_state_auth_client_port_inner(
        &self,
        request: &SessionAuthorityRequestDto,
    ) -> anyhow::Result<SessionState> {
        self.session_checks
            .lock()
            .expect("session checks")
            .push((request.user_id.clone(), request.session_id.clone()));
        match self.session_answer() {
            SessionAnswer::Active(roles) => Ok(SessionState::Active { roles }),
            SessionAnswer::Ended => Ok(SessionState::Ended),
            SessionAnswer::Unavailable => anyhow::bail!("issuer unavailable"),
        }
    }
}

#[async_trait]
impl AuthClientPort for TestClients {
    async fn signup(
        &self,
        _request: EmailSignupRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<SignupResponse> {
        self.signup_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn verify(
        &self,
        _request: EmailVerifyRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<SignupResponse> {
        self.verify_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn check_email(
        &self,
        request: EmailCheckRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<EmailAvailabilityResponse>
    {
        self.check_email_auth_client_port_inner(request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn request_password_reset(
        &self,
        _request: PasswordResetRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<PasswordResetResponse> {
        self.request_password_reset_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn confirm_password_reset(
        &self,
        _request: PasswordResetConfirmRequest,
    ) -> api_gateway::application::client_error::GatewayClientResult<PasswordResetResponse> {
        self.confirm_password_reset_auth_client_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn login(
        &self,
        request: LoginRequest,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<AuthResponse> {
        self.login_auth_client_port_inner(request, ip_address, user_agent)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn refresh(
        &self,
        refresh_token: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<AuthResponse> {
        self.refresh_auth_client_port_inner(refresh_token)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn logout(
        &self,
        refresh_token: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<()> {
        self.logout_auth_client_port_inner(refresh_token)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn authorize(
        &self,
        provider: api_gateway::application::dto::facade_dto::IdentityProviderDto,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        api_gateway::application::dto::auth_dto::OAuthAuthorizationDto,
    > {
        self.authorize_auth_client_port_inner(provider.into())
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn oauth_callback(
        &self,
        provider: api_gateway::application::dto::facade_dto::IdentityProviderDto,
        _code: String,
        _state: String,
        _ip_address: Option<String>,
        _user_agent: Option<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<AuthResponse> {
        self.oauth_callback_auth_client_port_inner(
            provider.into(),
            _code,
            _state,
            _ip_address,
            _user_agent,
        )
        .await
        .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn link_provider(
        &self,
        provider: api_gateway::application::dto::facade_dto::IdentityProviderDto,
        _request: OAuthLinkRequest,
        _user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<LinkedProvidersResponse> {
        self.link_provider_auth_client_port_inner(provider.into(), _request, _user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn delete_account(
        &self,
        user_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> api_gateway::application::client_error::GatewayClientResult<DeleteAccountResponse> {
        self.delete_account_auth_client_port_inner(user_id, request_id, ip_address, user_agent)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_user_deletion(
        &self,
        user_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<Option<UserDeletionSagaResponse>>
    {
        self.get_user_deletion_auth_client_port_inner(user_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn get_user_deletion_by_saga_id(
        &self,
        saga_id: String,
    ) -> api_gateway::application::client_error::GatewayClientResult<Option<UserDeletionSagaResponse>>
    {
        self.get_user_deletion_by_saga_id_auth_client_port_inner(saga_id)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn list_dead_lettered_user_deletions(
        &self,
        limit: i32,
        after_updated_at: Option<chrono::DateTime<chrono::Utc>>,
        after_saga_id: Option<Uuid>,
    ) -> api_gateway::application::client_error::GatewayClientResult<
        DeadLetteredUserDeletionListResponse,
    > {
        self.list_dead_lettered_user_deletions_auth_client_port_inner(
            limit,
            after_updated_at,
            after_saga_id,
        )
        .await
        .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn retry_user_deletion(
        &self,
        request: RetryUserDeletionRequestDto,
    ) -> api_gateway::application::client_error::GatewayClientResult<UserDeletionSagaResponse> {
        self.retry_user_deletion_auth_client_port_inner(request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
    async fn session_state(
        &self,
        request: &SessionAuthorityRequestDto,
    ) -> api_gateway::application::client_error::GatewayClientResult<SessionState> {
        self.session_state_auth_client_port_inner(request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
}

fn deletion_saga(user_id: String) -> UserDeletionSagaResponse {
    UserDeletionSagaResponse {
        id: "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa".into(),
        user_id,
        status: "IN_PROGRESS".into(),
        retention_until: "2026-09-03T12:00:00+00:00".into(),
        created_at: "2026-08-04T12:00:00+00:00".into(),
        updated_at: "2026-08-04T12:00:00+00:00".into(),
        completed_at: None,
        steps: vec![UserDeletionStepResponse {
            target: "PROFILE".into(),
            status: "PENDING".into(),
            attempt_count: 0,
            next_attempt_at: "2026-08-04T12:00:00+00:00".into(),
            last_error: None,
            completed_at: None,
        }],
    }
}

fn auth_response(refresh_token: &str) -> AuthResponse {
    AuthResponse {
        user_id: Uuid::parse_str(USER_ID).expect("user id"),
        access_token: "access-token".into(),
        refresh_token: refresh_token.into(),
        token_type: "Bearer".into(),
        expires_in: 900,
        linked_providers: vec![IdentityProvider::Email],
    }
}
