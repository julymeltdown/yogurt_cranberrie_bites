use crate::auth_runtime::AuthRuntimePort;
use crate::dto::auth_dto::LinkedProvidersResponseDto;
use crate::errors::AuthResult as Result;
use crate::ports::{
    AuthAuditRepositoryPort, OAuthAuthorizationStorePort, OAuthIdentityRepositoryPort,
    OAuthProviderMetricsPort, OAuthProviderPort, TokenServicePort, UserRepositoryPort,
};
use auth_domain::session::AuthAuditEvent;
use auth_domain::user::{IdentityProvider, User};
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct OAuthLinkUseCase {
    user_repository: Arc<dyn UserRepositoryPort>,
    identity_repository: Arc<dyn OAuthIdentityRepositoryPort>,
    auth_audit_repository: Arc<dyn AuthAuditRepositoryPort>,
    provider_client: Arc<dyn OAuthProviderPort>,
    authorization_store: Arc<dyn OAuthAuthorizationStorePort>,
    metrics: Arc<dyn OAuthProviderMetricsPort>,
    token_service: Arc<dyn TokenServicePort>,
    runtime: Arc<dyn AuthRuntimePort>,
}

impl OAuthLinkUseCase {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user_repository: Arc<dyn UserRepositoryPort>,
        identity_repository: Arc<dyn OAuthIdentityRepositoryPort>,
        auth_audit_repository: Arc<dyn AuthAuditRepositoryPort>,
        provider_client: Arc<dyn OAuthProviderPort>,
        authorization_store: Arc<dyn OAuthAuthorizationStorePort>,
        metrics: Arc<dyn OAuthProviderMetricsPort>,
        token_service: Arc<dyn TokenServicePort>,
        runtime: Arc<dyn AuthRuntimePort>,
    ) -> Self {
        Self {
            user_repository,
            identity_repository,
            auth_audit_repository,
            provider_client,
            authorization_store,
            metrics,
            token_service,
            runtime,
        }
    }

    pub async fn link_provider(
        &self,
        user_id: Uuid,
        provider: IdentityProvider,
        code: &str,
        state: &str,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<LinkedProvidersResponseDto> {
        let provider = require_oauth(provider)?;
        if code.trim().is_empty() {
            return Err(crate::errors::AuthError::InvalidInput(
                "OAuth code is required",
            ));
        }
        if state.trim().is_empty() {
            return Err(crate::errors::AuthError::InvalidInput(
                "OAuth state is required",
            ));
        }
        self.user_repository
            .find_by_id(user_id)
            .await?
            .map(User::from)
            .ok_or(crate::errors::AuthError::NotFound)?;
        let authorization = self
            .authorization_store
            .consume(provider.into(), state.trim(), self.runtime.now())
            .await?
            .ok_or(crate::errors::AuthError::InvalidInput(
                "OAuth authorization state is invalid",
            ))?;

        let user_info = self
            .provider_client
            .fetch_user_info(
                (provider).into(),
                code.trim(),
                &authorization.redirect_uri,
                Some(&authorization.code_verifier),
                &authorization.nonce,
            )
            .await;
        let outcome = if user_info.is_ok() {
            "SUCCESS"
        } else {
            "FAILURE"
        };
        let error_code = user_info.as_ref().err().map(provider_exchange_failure_code);
        self.metrics
            .record_provider_exchange((provider).into(), outcome);
        self.record_provider_exchange_audit(
            provider, user_id, outcome, request_id, ip_address, user_agent, error_code,
        )
        .await?;
        let user_info = auth_domain::oauth_identity::OAuthUserInfo::from(user_info?);
        let normalized_email = if user_info.email.trim().is_empty() {
            format!("{}@oauth.local", user_info.subject)
        } else {
            auth_domain::user::User::normalized_email(&user_info.email)
        };

        if let Some(existing) = self
            .identity_repository
            .find_by_provider_subject((provider).into(), &user_info.subject)
            .await?
        {
            if existing.user_id != user_id {
                return Err(crate::errors::AuthError::Conflict(
                    "Provider already linked to another user",
                ));
            }
        }

        if !normalized_email.is_empty() {
            let identities = self
                .identity_repository
                .find_by_email(&normalized_email)
                .await?;
            if identities
                .iter()
                .any(|identity| identity.user_id != user_id)
            {
                return Err(crate::errors::AuthError::Conflict(
                    "Email already linked to another user",
                ));
            }
        }

        self.identity_repository
            .link_oauth_identity(
                user_id,
                (provider).into(),
                &user_info.subject,
                &normalized_email,
                user_info.email_verified,
            )
            .await?;

        Ok(LinkedProvidersResponseDto {
            user_id,
            providers: self
                .user_repository
                .linked_providers(user_id)
                .await?
                .into_iter()
                .map(Into::into)
                .collect(),
        })
    }

    pub fn decode_access_token(&self, token: &str) -> Result<Uuid> {
        Ok(self.token_service.decode_access_token(token)?.subject)
    }

    async fn record_provider_exchange_audit(
        &self,
        provider: IdentityProvider,
        user_id: Uuid,
        outcome: &str,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        error_code: Option<&'static str>,
    ) -> Result<()> {
        let metadata_json = match error_code {
            Some(error) => json!({
                "provider": provider_audit_name(provider),
                "flow": "link",
                "error": error,
            }),
            None => json!({
                "provider": provider_audit_name(provider),
                "flow": "link",
            }),
        }
        .to_string();
        self.auth_audit_repository
            .record(
                &(&AuthAuditEvent {
                    id: self.runtime.new_id(),
                    actor_id: Some(user_id),
                    subject_user_id: Some(user_id),
                    action: "OAUTH_PROVIDER_EXCHANGE".into(),
                    outcome: outcome.into(),
                    request_id,
                    ip_address,
                    user_agent,
                    metadata_json,
                    occurred_at: self.runtime.now(),
                })
                    .into(),
            )
            .await
    }
}

fn require_oauth(provider: IdentityProvider) -> Result<IdentityProvider> {
    if provider == IdentityProvider::Email {
        return Err(crate::errors::AuthError::InvalidInput(
            "OAuth provider required",
        ));
    }
    Ok(provider)
}

fn provider_exchange_failure_code(error: &crate::errors::AuthError) -> &'static str {
    match error {
        crate::errors::AuthError::ProviderFailure(failure) => failure.code(),
        _ => "PROVIDER_EXCHANGE_FAILED",
    }
}

fn provider_audit_name(provider: IdentityProvider) -> &'static str {
    match provider {
        IdentityProvider::Email => "EMAIL",
        IdentityProvider::Google => "GOOGLE",
        IdentityProvider::Apple => "APPLE",
        IdentityProvider::X => "X",
    }
}
