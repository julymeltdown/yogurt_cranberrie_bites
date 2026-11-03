use crate::dto::auth_dto::{
    AuthResponse, DeadLetteredUserDeletionListResponse, DeleteAccountResponse,
    EmailAvailabilityResponse, EmailCheckRequest, EmailSignupRequest, EmailVerifyRequest,
    LinkedProvidersResponse, LoginRequest, OAuthLinkRequest, PasswordResetConfirmRequest,
    PasswordResetRequest, PasswordResetResponse, RetryUserDeletionRequestDto, SignupResponse,
    UserDeletionSagaResponse,
};
use crate::dto::facade_dto::IdentityProviderDto;
use crate::dto::verification_dto::{ResendVerificationRequest, ResendVerificationResponse};
use crate::ports::AuthClientPort;
use crate::verification::{VerificationClientPort, VerificationError, VerificationGatewayService};
use std::sync::Arc;

#[derive(Clone)]
pub struct AuthGatewayService {
    client: Arc<dyn AuthClientPort>,
    verification: Option<VerificationGatewayService>,
}

impl AuthGatewayService {
    pub fn new(client: Arc<dyn AuthClientPort>) -> Self {
        Self {
            client,
            verification: None,
        }
    }

    /// Both the executable and HTTP regression tests inject the same typed port.
    /// Missing wiring fails closed rather than reporting a fabricated acceptance.
    pub fn with_verification_client(mut self, client: Arc<dyn VerificationClientPort>) -> Self {
        self.verification = Some(VerificationGatewayService::new(client));
        self
    }

    pub async fn resend_verification(
        &self,
        request: ResendVerificationRequest,
    ) -> Result<ResendVerificationResponse, VerificationError> {
        self.verification
            .as_ref()
            .ok_or(VerificationError::Unavailable)?
            .resend_verification(request)
            .await
    }

    pub async fn signup(
        &self,
        request: EmailSignupRequest,
    ) -> crate::service_error::GatewayServiceResult<SignupResponse> {
        self.client.signup(request).await.map_err(Into::into)
    }

    pub async fn verify(
        &self,
        request: EmailVerifyRequest,
    ) -> crate::service_error::GatewayServiceResult<SignupResponse> {
        self.client.verify(request).await.map_err(Into::into)
    }

    pub async fn check_email(
        &self,
        request: EmailCheckRequest,
    ) -> crate::service_error::GatewayServiceResult<EmailAvailabilityResponse> {
        self.client.check_email(request).await.map_err(Into::into)
    }

    pub async fn request_password_reset(
        &self,
        request: PasswordResetRequest,
    ) -> crate::service_error::GatewayServiceResult<PasswordResetResponse> {
        self.client
            .request_password_reset(request)
            .await
            .map_err(Into::into)
    }

    pub async fn confirm_password_reset(
        &self,
        request: PasswordResetConfirmRequest,
    ) -> crate::service_error::GatewayServiceResult<PasswordResetResponse> {
        self.client
            .confirm_password_reset(request)
            .await
            .map_err(Into::into)
    }

    pub async fn login(
        &self,
        request: LoginRequest,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> crate::service_error::GatewayServiceResult<AuthResponse> {
        self.client
            .login(request, ip_address, user_agent)
            .await
            .map_err(Into::into)
    }

    pub async fn refresh(
        &self,
        refresh_token: String,
    ) -> crate::service_error::GatewayServiceResult<AuthResponse> {
        self.client.refresh(refresh_token).await.map_err(Into::into)
    }

    pub async fn logout(
        &self,
        refresh_token: String,
    ) -> crate::service_error::GatewayServiceResult<()> {
        self.client.logout(refresh_token).await.map_err(Into::into)
    }

    pub async fn authorize(
        &self,
        provider: IdentityProviderDto,
    ) -> crate::service_error::GatewayServiceResult<crate::dto::auth_dto::OAuthAuthorizationDto>
    {
        self.client.authorize(provider).await.map_err(Into::into)
    }

    pub async fn oauth_callback(
        &self,
        provider: IdentityProviderDto,
        code: String,
        state: String,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> crate::service_error::GatewayServiceResult<AuthResponse> {
        self.client
            .oauth_callback(provider, code, state, ip_address, user_agent)
            .await
            .map_err(Into::into)
    }

    pub async fn link_provider(
        &self,
        provider: IdentityProviderDto,
        request: OAuthLinkRequest,
        user_id: String,
    ) -> crate::service_error::GatewayServiceResult<LinkedProvidersResponse> {
        self.client
            .link_provider(provider, request, user_id)
            .await
            .map_err(Into::into)
    }

    pub async fn delete_account(
        &self,
        user_id: String,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> crate::service_error::GatewayServiceResult<DeleteAccountResponse> {
        self.client
            .delete_account(user_id, request_id, ip_address, user_agent)
            .await
            .map_err(Into::into)
    }

    pub async fn get_user_deletion(
        &self,
        user_id: String,
    ) -> crate::service_error::GatewayServiceResult<Option<UserDeletionSagaResponse>> {
        self.client
            .get_user_deletion(user_id)
            .await
            .map_err(Into::into)
    }

    pub async fn get_user_deletion_by_saga_id(
        &self,
        saga_id: String,
    ) -> crate::service_error::GatewayServiceResult<
        Option<crate::dto::auth_dto::UserDeletionSagaResponse>,
    > {
        self.client
            .get_user_deletion_by_saga_id(saga_id)
            .await
            .map_err(Into::into)
    }

    pub async fn list_dead_lettered_user_deletions(
        &self,
        limit: i32,
        after_updated_at: Option<chrono::DateTime<chrono::Utc>>,
        after_saga_id: Option<uuid::Uuid>,
    ) -> crate::service_error::GatewayServiceResult<DeadLetteredUserDeletionListResponse> {
        self.client
            .list_dead_lettered_user_deletions(limit, after_updated_at, after_saga_id)
            .await
            .map_err(Into::into)
    }

    pub async fn retry_user_deletion(
        &self,
        request: RetryUserDeletionRequestDto,
    ) -> crate::service_error::GatewayServiceResult<UserDeletionSagaResponse> {
        self.client
            .retry_user_deletion(request)
            .await
            .map_err(Into::into)
    }
}
