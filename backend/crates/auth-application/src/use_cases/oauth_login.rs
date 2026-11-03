use crate::auth_runtime::{AuthRuntimePort, OAuthAuthorizationCryptoPort};
use crate::auth_transactions::AuthTransactionsPort;
use crate::dto::auth_dto::{AuthResponseDto, OAuthAuthorizeResponseDto};
use crate::errors::AuthError;
use crate::errors::AuthResult as Result;
use crate::ports::{
    AuthAuditRepositoryPort, OAuthAuthorizationStorePort, OAuthProviderMetricsPort,
    OAuthProviderPort, TokenServicePort, UserRepositoryPort,
};
use auth_domain::oauth_authorization::OAuthAuthorizationTransaction;
use auth_domain::session::{AuthAuditEvent, AuthTokenPair, LoginAudit, RefreshTokenRecord};
use auth_domain::user::{IdentityProvider, User, UserStatus};
use base64ct::{Base64, Encoding};
use chrono::Duration;
use serde_json::json;
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct OAuthLoginUseCase {
    user_repository: Arc<dyn UserRepositoryPort>,
    transactions: Arc<dyn AuthTransactionsPort>,
    auth_audit_repository: Arc<dyn AuthAuditRepositoryPort>,
    token_service: Arc<dyn TokenServicePort>,
    provider_client: Arc<dyn OAuthProviderPort>,
    authorization_store: Arc<dyn OAuthAuthorizationStorePort>,
    metrics: Arc<dyn OAuthProviderMetricsPort>,
    runtime: Arc<dyn AuthRuntimePort>,
    authorization_crypto: Arc<dyn OAuthAuthorizationCryptoPort>,
    google_redirect_uri: String,
    apple_redirect_uri: String,
    x_redirect_uri: String,
}

impl OAuthLoginUseCase {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        user_repository: Arc<dyn UserRepositoryPort>,
        transactions: Arc<dyn AuthTransactionsPort>,
        auth_audit_repository: Arc<dyn AuthAuditRepositoryPort>,
        token_service: Arc<dyn TokenServicePort>,
        provider_client: Arc<dyn OAuthProviderPort>,
        authorization_store: Arc<dyn OAuthAuthorizationStorePort>,
        metrics: Arc<dyn OAuthProviderMetricsPort>,
        runtime: Arc<dyn AuthRuntimePort>,
        authorization_crypto: Arc<dyn OAuthAuthorizationCryptoPort>,
        google_redirect_uri: String,
        apple_redirect_uri: String,
        x_redirect_uri: String,
    ) -> Self {
        Self {
            user_repository,
            transactions,
            auth_audit_repository,
            token_service,
            provider_client,
            authorization_store,
            metrics,
            runtime,
            authorization_crypto,
            google_redirect_uri,
            apple_redirect_uri,
            x_redirect_uri,
        }
    }

    pub async fn authorize(&self, provider: IdentityProvider) -> Result<OAuthAuthorizeResponseDto> {
        let provider = require_oauth(provider)?;
        let redirect_uri = self.redirect_uri(provider)?.to_owned();
        let material = self.authorization_crypto.generate()?;
        let transaction = OAuthAuthorizationTransaction {
            provider,
            state: material.state,
            redirect_uri,
            code_verifier: material.code_verifier,
            code_challenge: material.code_challenge,
            nonce: material.nonce,
            expires_at: self.runtime.now() + Duration::minutes(10),
        };
        self.authorization_store
            .save(&(&transaction).into())
            .await?;
        let authorization_uri = self.provider_client.build_authorization_uri(
            (provider).into(),
            &transaction.redirect_uri,
            &transaction.state,
            &transaction.code_challenge,
            &transaction.nonce,
        )?;
        Ok(OAuthAuthorizeResponseDto { authorization_uri })
    }

    pub async fn oauth_callback(
        &self,
        provider: IdentityProvider,
        code: &str,
        state: &str,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
    ) -> Result<AuthResponseDto> {
        let provider = require_oauth(provider)?;
        if code.trim().is_empty() {
            return Err(AuthError::InvalidInput("OAuth code is required"));
        }
        if state.trim().is_empty() {
            return Err(AuthError::InvalidInput("OAuth state is required"));
        }
        let authorization = self
            .authorization_store
            .consume(provider.into(), state.trim(), self.runtime.now())
            .await?
            .ok_or(AuthError::InvalidInput(
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
            provider,
            "callback",
            outcome,
            request_id,
            ip_address.clone(),
            user_agent.clone(),
            error_code,
        )
        .await?;
        let user_info = auth_domain::oauth_identity::OAuthUserInfo::from(user_info?);
        if user_info.provider != provider {
            return Err(AuthError::InvalidInput("OAuth provider mismatch"));
        }

        let expected: User = self
            .transactions
            .resolve_oauth_user(&(&user_info).into())
            .await?
            .into();
        validate_active(&expected)?;
        let user_id = expected.id;
        let roles = self.transactions.roles(user_id).await?;
        let linked_providers = self
            .user_repository
            .linked_providers(user_id)
            .await?
            .into_iter()
            .map(Into::into)
            .collect();
        let tokens = self
            .token_service
            .issue_token_pair(user_id, roles.clone())
            .map(AuthTokenPair::from)?;
        let refresh = RefreshTokenRecord::new_family(
            uuid::Uuid::parse_str(&tokens.refresh_jti).unwrap_or_else(|_| self.runtime.new_id()),
            tokens.refresh_jti.clone(),
            hash_token(&tokens.refresh_token),
            user_id,
            tokens.refresh_expires_at,
        );
        let audit = LoginAudit::success(
            self.runtime.new_id(),
            user_id,
            ip_address,
            user_agent,
            self.runtime.now(),
        );
        if !self
            .transactions
            .begin_session(
                &(&expected).into(),
                &(&refresh).into(),
                &roles,
                &(&audit).into(),
            )
            .await?
        {
            return Err(AuthError::RefreshConflict);
        }
        Ok(to_auth_response(user_id, tokens, linked_providers))
    }

    #[allow(clippy::too_many_arguments)]
    async fn record_provider_exchange_audit(
        &self,
        provider: IdentityProvider,
        flow: &str,
        outcome: &str,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        error_code: Option<&'static str>,
    ) -> Result<()> {
        let metadata_json = match error_code {
            Some(error) => {
                json!({"provider": provider_audit_name(provider), "flow": flow, "error": error})
            }
            None => json!({"provider": provider_audit_name(provider), "flow": flow}),
        }
        .to_string();
        self.auth_audit_repository
            .record(
                &(&AuthAuditEvent {
                    id: self.runtime.new_id(),
                    actor_id: None,
                    subject_user_id: None,
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

    fn redirect_uri(&self, provider: IdentityProvider) -> Result<&str> {
        match provider {
            IdentityProvider::Google if !self.google_redirect_uri.is_empty() => {
                Ok(&self.google_redirect_uri)
            }
            IdentityProvider::Apple if !self.apple_redirect_uri.is_empty() => {
                Ok(&self.apple_redirect_uri)
            }
            IdentityProvider::X if !self.x_redirect_uri.is_empty() => Ok(&self.x_redirect_uri),
            IdentityProvider::Google | IdentityProvider::Apple | IdentityProvider::X => {
                Err(AuthError::Invariant("OAuth redirect URI is not configured"))
            }
            IdentityProvider::Email => Err(AuthError::InvalidInput("OAuth provider required")),
        }
    }
}

fn require_oauth(provider: IdentityProvider) -> Result<IdentityProvider> {
    if provider == IdentityProvider::Email {
        return Err(AuthError::InvalidInput("OAuth provider required"));
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

fn validate_active(user: &User) -> Result<()> {
    if user.status != UserStatus::Active {
        return Err(AuthError::InvalidCredentials);
    }
    Ok(())
}

fn hash_token(token: &str) -> String {
    Base64::encode_string(Sha256::digest(token.as_bytes()).as_slice())
}

fn to_auth_response(
    user_id: Uuid,
    tokens: AuthTokenPair,
    linked_providers: Vec<IdentityProvider>,
) -> AuthResponseDto {
    AuthResponseDto {
        user_id,
        access_token: tokens.access_token,
        refresh_token: tokens.refresh_token,
        token_type: tokens.token_type,
        expires_in_seconds: tokens.expires_in_seconds,
        linked_providers,
    }
}
