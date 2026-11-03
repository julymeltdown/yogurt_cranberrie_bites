use async_trait::async_trait;
use auth_service::application::errors::AuthError;
use auth_service::application::ports::{
    OAuthAuthorizationStorePort, OAuthIdentityRepositoryPort, OAuthProviderMetricsPort,
    OAuthProviderPort, RefreshTokenStorePort, TokenServicePort, UserRepositoryPort,
};
use auth_service::application::use_cases::oauth_link::OAuthLinkUseCase;
use auth_service::application::use_cases::oauth_login::OAuthLoginUseCase;
use auth_service::domain::oauth_authorization::{
    ConsumedOAuthAuthorization, OAuthAuthorizationTransaction,
};
use auth_service::domain::oauth_failure::OAuthProviderFailure;
use auth_service::domain::oauth_identity::OAuthUserInfo;
use auth_service::domain::user::{IdentityProvider, UserStatus};
use chrono::{DateTime, Duration, Utc};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use tokio::sync::Mutex as AsyncMutex;
use uuid::Uuid;

#[derive(Clone, Default)]
struct FakeOAuthAuthorizationStore {
    transactions:
        Arc<AsyncMutex<HashMap<(IdentityProvider, String), OAuthAuthorizationTransaction>>>,
}

impl FakeOAuthAuthorizationStore {
    async fn insert(
        &self,
        provider: IdentityProvider,
        state: &str,
        redirect_uri: &str,
        code_verifier: &str,
        nonce: &str,
    ) {
        self.transactions.lock().await.insert(
            (provider, state.to_owned()),
            OAuthAuthorizationTransaction {
                provider,
                state: state.to_owned(),
                redirect_uri: redirect_uri.to_owned(),
                code_challenge: "challenge".into(),
                code_verifier: code_verifier.to_owned(),
                nonce: nonce.to_owned(),
                expires_at: Utc::now() + Duration::minutes(10),
            },
        );
    }

    async fn saved_transactions(&self) -> Vec<OAuthAuthorizationTransaction> {
        self.transactions.lock().await.values().cloned().collect()
    }
}

#[async_trait]
impl OAuthAuthorizationStorePort for FakeOAuthAuthorizationStore {
    async fn save(
        &self,
        transaction: &auth_service::application::dto::repository_dto::AuthOAuthAuthorizationRecordDto,
    ) -> auth_service::application::errors::AuthResult<()> {
        let transaction =
            &auth_service::domain::oauth_authorization::OAuthAuthorizationTransaction::from(
                transaction.clone(),
            );

        let result: anyhow::Result<_> = async {
            self.transactions.lock().await.insert(
                (transaction.provider, transaction.state.clone()),
                transaction.clone(),
            );
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn consume(
        &self,
        provider: auth_service::application::dto::repository_dto::AuthIdentityProviderDto,
        state: &str,
        now: DateTime<Utc>,
    ) -> auth_service::application::errors::AuthResult<
        Option<auth_service::application::dto::repository_dto::AuthConsumedOAuthAuthorizationDto>,
    > {
        let provider = auth_service::domain::user::IdentityProvider::from(provider);
        let result: anyhow::Result<_> = async {
            let transaction = self
                .transactions
                .lock()
                .await
                .remove(&(provider, state.to_owned()));
            Ok(transaction.and_then(|transaction| {
                (transaction.expires_at > now).then_some(ConsumedOAuthAuthorization {
                    provider: transaction.provider,
                    state: transaction.state,
                    redirect_uri: transaction.redirect_uri,
                    code_verifier: transaction.code_verifier,
                    nonce: transaction.nonce,
                })
            }))
        }
        .await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }
}

#[derive(Debug, Clone)]
struct AuthorizationCall {
    provider: IdentityProvider,
    redirect_uri: String,
    state: String,
    code_challenge: String,
    nonce: String,
}

#[derive(Debug, Clone)]
struct FetchCall {
    provider: IdentityProvider,
    code: String,
    redirect_uri: String,
    code_verifier: Option<String>,
    expected_nonce: String,
}

#[derive(Debug, Clone)]
enum ProviderResponse {
    Success(OAuthUserInfo),
    Failure(OAuthProviderFailure),
}

#[derive(Clone)]
struct FakeOAuthProvider {
    response: Arc<Mutex<ProviderResponse>>,
    authorization_calls: Arc<Mutex<Vec<AuthorizationCall>>>,
    fetch_calls: Arc<Mutex<Vec<FetchCall>>>,
}

impl FakeOAuthProvider {
    fn success(provider: IdentityProvider, subject: &str, email: &str, verified: bool) -> Self {
        Self {
            response: Arc::new(Mutex::new(ProviderResponse::Success(OAuthUserInfo {
                provider,
                subject: subject.to_owned(),
                email: email.to_owned(),
                email_verified: verified,
            }))),
            authorization_calls: Arc::new(Mutex::new(Vec::new())),
            fetch_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn failure(failure: OAuthProviderFailure) -> Self {
        Self {
            response: Arc::new(Mutex::new(ProviderResponse::Failure(failure))),
            authorization_calls: Arc::new(Mutex::new(Vec::new())),
            fetch_calls: Arc::new(Mutex::new(Vec::new())),
        }
    }

    fn authorization_calls(&self) -> Vec<AuthorizationCall> {
        self.authorization_calls
            .lock()
            .expect("authorization calls")
            .clone()
    }

    fn fetch_calls(&self) -> Vec<FetchCall> {
        self.fetch_calls.lock().expect("fetch calls").clone()
    }
}

#[async_trait]
impl OAuthProviderPort for FakeOAuthProvider {
    fn build_authorization_uri(
        &self,
        provider: auth_service::application::dto::repository_dto::AuthIdentityProviderDto,
        redirect_uri: &str,
        state: &str,
        code_challenge: &str,
        nonce: &str,
    ) -> auth_service::application::errors::AuthResult<String> {
        let provider = auth_service::domain::user::IdentityProvider::from(provider);

        let result: anyhow::Result<_> = {
            self.authorization_calls
                .lock()
                .expect("authorization calls")
                .push(AuthorizationCall {
                    provider,
                    redirect_uri: redirect_uri.to_owned(),
                    state: state.to_owned(),
                    code_challenge: code_challenge.to_owned(),
                    nonce: nonce.to_owned(),
                });
            Ok(format!(
            "https://oauth.test/{provider:?}?redirect_uri={redirect_uri}&state={state}&challenge={code_challenge}&nonce={nonce}"
        ))
        };
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn fetch_user_info(
        &self,
        provider: auth_service::application::dto::repository_dto::AuthIdentityProviderDto,
        code: &str,
        redirect_uri: &str,
        code_verifier: Option<&str>,
        expected_nonce: &str,
    ) -> auth_service::application::errors::AuthResult<
        auth_service::application::dto::repository_dto::AuthOAuthProofDto,
    > {
        let provider = auth_service::domain::user::IdentityProvider::from(provider);
        let result: anyhow::Result<_> = async {
            self.fetch_calls
                .lock()
                .expect("fetch calls")
                .push(FetchCall {
                    provider,
                    code: code.to_owned(),
                    redirect_uri: redirect_uri.to_owned(),
                    code_verifier: code_verifier.map(str::to_owned),
                    expected_nonce: expected_nonce.to_owned(),
                });
            match self.response.lock().expect("provider response").clone() {
                ProviderResponse::Success(user_info) => Ok(user_info),
                ProviderResponse::Failure(failure) => Err(anyhow::Error::new(failure)),
            }
        }
        .await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value| value.into())
    }
}

#[derive(Clone, Default)]
struct FakeOAuthMetrics {
    exchanges: Arc<Mutex<Vec<(IdentityProvider, String)>>>,
}

impl FakeOAuthMetrics {
    fn exchanges(&self) -> Vec<(IdentityProvider, String)> {
        self.exchanges.lock().expect("metrics").clone()
    }
}

impl OAuthProviderMetricsPort for FakeOAuthMetrics {
    fn record_provider_exchange(
        &self,
        provider: auth_service::application::dto::repository_dto::AuthIdentityProviderDto,
        outcome: &str,
    ) {
        let provider = auth_service::domain::user::IdentityProvider::from(provider);

        self.exchanges
            .lock()
            .expect("metrics")
            .push((provider, outcome.to_owned()));
    }
}

fn oauth_login_use_case(
    users: &crate::support::InMemoryUserRepository,
    login_audits: &crate::support::InMemoryAuditRepository,
    auth_audits: &crate::support::InMemoryAuthAuditRepository,
    refresh_tokens: &crate::support::InMemoryRefreshTokenStore,
    tokens: &crate::support::TestTokenService,
    provider: &FakeOAuthProvider,
    authorizations: &FakeOAuthAuthorizationStore,
    metrics: &FakeOAuthMetrics,
) -> OAuthLoginUseCase {
    oauth_login_with_capabilities(
        users,
        login_audits,
        auth_audits,
        refresh_tokens,
        tokens,
        provider,
        authorizations,
        metrics,
        Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        Arc::new(
            auth_service::adapters::out::security::oauth_authorization::SecureOAuthAuthorization,
        ),
    )
}

#[allow(clippy::too_many_arguments)]
fn oauth_login_with_capabilities(
    users: &crate::support::InMemoryUserRepository,
    login_audits: &crate::support::InMemoryAuditRepository,
    auth_audits: &crate::support::InMemoryAuthAuditRepository,
    refresh_tokens: &crate::support::InMemoryRefreshTokenStore,
    tokens: &crate::support::TestTokenService,
    provider: &FakeOAuthProvider,
    authorizations: &FakeOAuthAuthorizationStore,
    metrics: &FakeOAuthMetrics,
    runtime: Arc<dyn auth_service::application::auth_runtime::AuthRuntimePort>,
    crypto: Arc<dyn auth_service::application::auth_runtime::OAuthAuthorizationCryptoPort>,
) -> OAuthLoginUseCase {
    let transactions = crate::support::security::InMemoryAuthTransactions::new(users.clone())
        .with_audits(login_audits.clone())
        .with_refresh_tokens(refresh_tokens.clone());
    OAuthLoginUseCase::new(
        Arc::new(users.clone()),
        crate::support::security::transaction_port(transactions),
        Arc::new(auth_audits.clone()),
        Arc::new(tokens.clone()),
        Arc::new(provider.clone()),
        Arc::new(authorizations.clone()),
        Arc::new(metrics.clone()),
        runtime,
        crypto,
        "http://gateway.test/oauth/google/callback".into(),
        "http://gateway.test/oauth/apple/callback".into(),
        "http://gateway.test/oauth/x/callback".into(),
    )
}

fn oauth_link_use_case(
    users: &crate::support::InMemoryUserRepository,
    auth_audits: &crate::support::InMemoryAuthAuditRepository,
    tokens: &crate::support::TestTokenService,
    provider: &FakeOAuthProvider,
    authorizations: &FakeOAuthAuthorizationStore,
    metrics: &FakeOAuthMetrics,
) -> OAuthLinkUseCase {
    OAuthLinkUseCase::new(
        Arc::new(users.clone()),
        Arc::new(users.clone()),
        Arc::new(auth_audits.clone()),
        Arc::new(provider.clone()),
        Arc::new(authorizations.clone()),
        Arc::new(metrics.clone()),
        Arc::new(tokens.clone()),
        Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
    )
}

#[tokio::test]
async fn authorize_google_saves_pkce_state_and_builds_provider_uri() {
    let users = crate::support::InMemoryUserRepository::default();
    let login_audits = crate::support::InMemoryAuditRepository::default();
    let auth_audits = crate::support::InMemoryAuthAuditRepository::default();
    let refresh_tokens = crate::support::InMemoryRefreshTokenStore::default();
    let tokens = crate::support::TestTokenService::default();
    let provider = FakeOAuthProvider::success(
        IdentityProvider::Google,
        "google-sub",
        "hana@example.com",
        true,
    );
    let authorizations = FakeOAuthAuthorizationStore::default();
    let metrics = FakeOAuthMetrics::default();
    let use_case = oauth_login_use_case(
        &users,
        &login_audits,
        &auth_audits,
        &refresh_tokens,
        &tokens,
        &provider,
        &authorizations,
        &metrics,
    );

    let response = use_case
        .authorize(IdentityProvider::Google)
        .await
        .expect("authorize");

    let saved = authorizations.saved_transactions().await;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].provider, IdentityProvider::Google);
    assert_eq!(
        saved[0].redirect_uri,
        "http://gateway.test/oauth/google/callback"
    );
    assert_ne!(saved[0].code_challenge, saved[0].code_verifier);
    assert!(response.authorization_uri.contains(&saved[0].state));
    let calls = provider.authorization_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].provider, IdentityProvider::Google);
    assert_eq!(
        calls[0].redirect_uri,
        "http://gateway.test/oauth/google/callback"
    );
    assert_eq!(calls[0].state, saved[0].state);
    assert_eq!(calls[0].code_challenge, saved[0].code_challenge);
    assert_eq!(calls[0].nonce, saved[0].nonce);
}

#[tokio::test]
async fn authorize_x_saves_pkce_state_and_uses_x_redirect_uri() {
    let users = crate::support::InMemoryUserRepository::default();
    let login_audits = crate::support::InMemoryAuditRepository::default();
    let auth_audits = crate::support::InMemoryAuthAuditRepository::default();
    let refresh_tokens = crate::support::InMemoryRefreshTokenStore::default();
    let tokens = crate::support::TestTokenService::default();
    let provider = FakeOAuthProvider::success(IdentityProvider::X, "x-sub", "x@example.com", true);
    let authorizations = FakeOAuthAuthorizationStore::default();
    let metrics = FakeOAuthMetrics::default();
    let use_case = oauth_login_use_case(
        &users,
        &login_audits,
        &auth_audits,
        &refresh_tokens,
        &tokens,
        &provider,
        &authorizations,
        &metrics,
    );

    let response = use_case
        .authorize(IdentityProvider::X)
        .await
        .expect("authorize");

    let saved = authorizations.saved_transactions().await;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].provider, IdentityProvider::X);
    assert_eq!(
        saved[0].redirect_uri,
        "http://gateway.test/oauth/x/callback"
    );
    assert!(response.authorization_uri.contains(&saved[0].state));
    let calls = provider.authorization_calls();
    assert_eq!(calls.len(), 1);
    assert_eq!(calls[0].provider, IdentityProvider::X);
    assert_eq!(
        calls[0].redirect_uri,
        "http://gateway.test/oauth/x/callback"
    );
}

#[tokio::test]
async fn authorize_rejects_email_provider_before_saving_state() {
    let users = crate::support::InMemoryUserRepository::default();
    let login_audits = crate::support::InMemoryAuditRepository::default();
    let auth_audits = crate::support::InMemoryAuthAuditRepository::default();
    let refresh_tokens = crate::support::InMemoryRefreshTokenStore::default();
    let tokens = crate::support::TestTokenService::default();
    let provider = FakeOAuthProvider::success(
        IdentityProvider::Google,
        "google-sub",
        "hana@example.com",
        true,
    );
    let authorizations = FakeOAuthAuthorizationStore::default();
    let metrics = FakeOAuthMetrics::default();
    let use_case = oauth_login_use_case(
        &users,
        &login_audits,
        &auth_audits,
        &refresh_tokens,
        &tokens,
        &provider,
        &authorizations,
        &metrics,
    );

    let error = use_case
        .authorize(IdentityProvider::Email)
        .await
        .expect_err("email provider should be rejected");

    assert!(error.to_string().contains("OAuth provider required"));
    assert!(authorizations.saved_transactions().await.is_empty());
    assert!(provider.authorization_calls().is_empty());
}

#[tokio::test]
async fn callback_creates_user_links_identity_and_records_success_side_effects() {
    let users = crate::support::InMemoryUserRepository::default();
    let login_audits = crate::support::InMemoryAuditRepository::default();
    let auth_audits = crate::support::InMemoryAuthAuditRepository::default();
    let refresh_tokens = crate::support::InMemoryRefreshTokenStore::default();
    let tokens = crate::support::TestTokenService::default();
    let provider = FakeOAuthProvider::success(
        IdentityProvider::Google,
        "google-sub",
        "HANA@Example.COM",
        true,
    );
    let authorizations = FakeOAuthAuthorizationStore::default();
    authorizations
        .insert(
            IdentityProvider::Google,
            "state-1",
            "http://gateway.test/oauth/google/callback",
            "verifier-1",
            "nonce-1",
        )
        .await;
    let metrics = FakeOAuthMetrics::default();
    let use_case = oauth_login_use_case(
        &users,
        &login_audits,
        &auth_audits,
        &refresh_tokens,
        &tokens,
        &provider,
        &authorizations,
        &metrics,
    );

    let response = use_case
        .oauth_callback(
            IdentityProvider::Google,
            " auth-code ",
            " state-1 ",
            Some("request-1".into()),
            Some("127.0.0.1".into()),
            Some("unit-test".into()),
        )
        .await
        .expect("callback");

    let user = users
        .find_by_id(response.user_id)
        .await
        .expect("user lookup")
        .map(auth_service::domain::user::User::from)
        .expect("created user");
    assert_eq!(user.primary_email, "hana@example.com");
    assert_eq!(user.status, UserStatus::Active);
    let identity = users
        .find_by_provider_subject((IdentityProvider::Google).into(), "google-sub")
        .await
        .expect("identity lookup")
        .expect("linked identity");
    assert_eq!(identity.user_id, response.user_id);
    assert_eq!(response.linked_providers, vec![IdentityProvider::Google]);
    let decoded_refresh = tokens
        .decode_refresh_token(&response.refresh_token)
        .expect("refresh token");
    assert!(refresh_tokens
        .find(&decoded_refresh.jti)
        .await
        .expect("refresh store")
        .is_some());
    assert_eq!(login_audits.audits.lock().await.len(), 1);
    let auth_events = auth_audits.audits.lock().await;
    assert_eq!(auth_events.len(), 1);
    assert_eq!(auth_events[0].outcome, "SUCCESS");
    assert_eq!(auth_events[0].request_id.as_deref(), Some("request-1"));
    assert!(auth_events[0]
        .metadata_json
        .contains("\"provider\":\"GOOGLE\""));
    assert_eq!(
        metrics.exchanges(),
        vec![(IdentityProvider::Google, "SUCCESS".into())]
    );
    let fetch_calls = provider.fetch_calls();
    assert_eq!(fetch_calls.len(), 1);
    assert_eq!(fetch_calls[0].provider, IdentityProvider::Google);
    assert_eq!(fetch_calls[0].code, "auth-code");
    assert_eq!(fetch_calls[0].code_verifier.as_deref(), Some("verifier-1"));
    assert_eq!(fetch_calls[0].expected_nonce, "nonce-1");
}

#[tokio::test]
async fn callback_existing_identity_updates_last_login_without_relinking() {
    let users = crate::support::InMemoryUserRepository::default();
    let user_id = crate::support::seed_active_user(&users, "existing@example.com")
        .await
        .expect("seed user");
    users
        .link_oauth_identity(
            user_id,
            (IdentityProvider::Google).into(),
            "existing-sub",
            "existing@example.com",
            true,
        )
        .await
        .expect("link identity");
    let login_audits = crate::support::InMemoryAuditRepository::default();
    let auth_audits = crate::support::InMemoryAuthAuditRepository::default();
    let refresh_tokens = crate::support::InMemoryRefreshTokenStore::default();
    let tokens = crate::support::TestTokenService::default();
    let provider = FakeOAuthProvider::success(
        IdentityProvider::Google,
        "existing-sub",
        "existing@example.com",
        true,
    );
    let authorizations = FakeOAuthAuthorizationStore::default();
    authorizations
        .insert(
            IdentityProvider::Google,
            "state-existing",
            "http://gateway.test/oauth/google/callback",
            "verifier-existing",
            "nonce-existing",
        )
        .await;
    let metrics = FakeOAuthMetrics::default();
    let use_case = oauth_login_use_case(
        &users,
        &login_audits,
        &auth_audits,
        &refresh_tokens,
        &tokens,
        &provider,
        &authorizations,
        &metrics,
    );

    let response = use_case
        .oauth_callback(
            IdentityProvider::Google,
            "code-existing",
            "state-existing",
            None,
            None,
            None,
        )
        .await
        .expect("callback");

    assert_eq!(response.user_id, user_id);
    let updated = users
        .find_by_id(user_id)
        .await
        .expect("user lookup")
        .map(auth_service::domain::user::User::from)
        .expect("user");
    assert!(updated.last_login_at.is_some());
    assert_eq!(
        users
            .linked_providers(user_id)
            .await
            .expect("providers")
            .len(),
        1
    );
    assert_eq!(login_audits.audits.lock().await.len(), 1);
}

#[tokio::test]
async fn callback_provider_failure_records_failure_audit_and_metric() {
    let users = crate::support::InMemoryUserRepository::default();
    let login_audits = crate::support::InMemoryAuditRepository::default();
    let auth_audits = crate::support::InMemoryAuthAuditRepository::default();
    let refresh_tokens = crate::support::InMemoryRefreshTokenStore::default();
    let tokens = crate::support::TestTokenService::default();
    let provider = FakeOAuthProvider::failure(OAuthProviderFailure::code_invalid());
    let authorizations = FakeOAuthAuthorizationStore::default();
    authorizations
        .insert(
            IdentityProvider::Google,
            "state-failure",
            "http://gateway.test/oauth/google/callback",
            "verifier-failure",
            "nonce-failure",
        )
        .await;
    let metrics = FakeOAuthMetrics::default();
    let use_case = oauth_login_use_case(
        &users,
        &login_audits,
        &auth_audits,
        &refresh_tokens,
        &tokens,
        &provider,
        &authorizations,
        &metrics,
    );

    let error = use_case
        .oauth_callback(
            IdentityProvider::Google,
            "bad-code",
            "state-failure",
            Some("request-failure".into()),
            None,
            None,
        )
        .await
        .expect_err("provider failure should surface");

    assert!(error.to_string().contains("OAuth code is invalid"));
    assert!(login_audits.audits.lock().await.is_empty());
    let auth_events = auth_audits.audits.lock().await;
    assert_eq!(auth_events.len(), 1);
    assert_eq!(auth_events[0].outcome, "FAILURE");
    assert_eq!(
        auth_events[0].request_id.as_deref(),
        Some("request-failure")
    );
    assert!(auth_events[0]
        .metadata_json
        .contains("PROVIDER_CODE_INVALID"));
    assert_eq!(
        metrics.exchanges(),
        vec![(IdentityProvider::Google, "FAILURE".into())]
    );
}

#[tokio::test]
async fn link_provider_adds_identity_and_records_actor_audit() {
    let users = crate::support::InMemoryUserRepository::default();
    let user_id = crate::support::seed_active_user(&users, "link-me@example.com")
        .await
        .expect("seed user");
    let auth_audits = crate::support::InMemoryAuthAuditRepository::default();
    let tokens = crate::support::TestTokenService::default();
    let provider = FakeOAuthProvider::success(
        IdentityProvider::Apple,
        "apple-sub",
        "link-me@example.com",
        true,
    );
    let authorizations = FakeOAuthAuthorizationStore::default();
    authorizations
        .insert(
            IdentityProvider::Apple,
            "state-link",
            "http://gateway.test/oauth/apple/callback",
            "verifier-link",
            "nonce-link",
        )
        .await;
    let metrics = FakeOAuthMetrics::default();
    let use_case = oauth_link_use_case(
        &users,
        &auth_audits,
        &tokens,
        &provider,
        &authorizations,
        &metrics,
    );

    let response = use_case
        .link_provider(
            user_id,
            IdentityProvider::Apple,
            "link-code",
            "state-link",
            Some("request-link".into()),
            Some("10.0.0.1".into()),
            Some("link-test".into()),
        )
        .await
        .expect("link provider");

    assert_eq!(response.user_id, user_id);
    assert_eq!(response.providers, vec![IdentityProvider::Apple]);
    let auth_events = auth_audits.audits.lock().await;
    assert_eq!(auth_events.len(), 1);
    assert_eq!(auth_events[0].actor_id, Some(user_id));
    assert_eq!(auth_events[0].subject_user_id, Some(user_id));
    assert_eq!(auth_events[0].outcome, "SUCCESS");
    assert!(auth_events[0].metadata_json.contains("\"flow\":\"link\""));
    assert_eq!(
        metrics.exchanges(),
        vec![(IdentityProvider::Apple, "SUCCESS".into())]
    );
    let fetch_calls = provider.fetch_calls();
    assert_eq!(fetch_calls[0].provider, IdentityProvider::Apple);
    assert_eq!(
        fetch_calls[0].redirect_uri,
        "http://gateway.test/oauth/apple/callback"
    );
    assert_eq!(
        fetch_calls[0].code_verifier.as_deref(),
        Some("verifier-link")
    );
}

#[tokio::test]
async fn link_provider_rejects_identity_owned_by_another_user_after_auditing_exchange() {
    let users = crate::support::InMemoryUserRepository::default();
    let current_user_id = crate::support::seed_active_user(&users, "current@example.com")
        .await
        .expect("seed current user");
    let other_user_id = crate::support::seed_active_user(&users, "other@example.com")
        .await
        .expect("seed other user");
    users
        .link_oauth_identity(
            other_user_id,
            (IdentityProvider::Google).into(),
            "shared-sub",
            "other@example.com",
            true,
        )
        .await
        .expect("link other identity");
    let auth_audits = crate::support::InMemoryAuthAuditRepository::default();
    let tokens = crate::support::TestTokenService::default();
    let provider = FakeOAuthProvider::success(
        IdentityProvider::Google,
        "shared-sub",
        "current@example.com",
        true,
    );
    let authorizations = FakeOAuthAuthorizationStore::default();
    authorizations
        .insert(
            IdentityProvider::Google,
            "state-conflict",
            "http://gateway.test/oauth/google/callback",
            "verifier-conflict",
            "nonce-conflict",
        )
        .await;
    let metrics = FakeOAuthMetrics::default();
    let use_case = oauth_link_use_case(
        &users,
        &auth_audits,
        &tokens,
        &provider,
        &authorizations,
        &metrics,
    );

    let error = use_case
        .link_provider(
            current_user_id,
            IdentityProvider::Google,
            "conflict-code",
            "state-conflict",
            None,
            None,
            None,
        )
        .await
        .expect_err("identity owned by another user should be rejected");

    assert!(error
        .to_string()
        .contains("Provider already linked to another user"));
    assert!(users
        .linked_providers(current_user_id)
        .await
        .expect("current providers")
        .is_empty());
    let auth_events = auth_audits.audits.lock().await;
    assert_eq!(auth_events.len(), 1);
    assert_eq!(auth_events[0].actor_id, Some(current_user_id));
    assert_eq!(auth_events[0].outcome, "SUCCESS");
}

#[tokio::test]
async fn link_use_case_decodes_access_token_subject() {
    let users = crate::support::InMemoryUserRepository::default();
    let auth_audits = crate::support::InMemoryAuthAuditRepository::default();
    let tokens = crate::support::TestTokenService::default();
    let provider = FakeOAuthProvider::success(
        IdentityProvider::Google,
        "google-sub",
        "hana@example.com",
        true,
    );
    let authorizations = FakeOAuthAuthorizationStore::default();
    let metrics = FakeOAuthMetrics::default();
    let use_case = oauth_link_use_case(
        &users,
        &auth_audits,
        &tokens,
        &provider,
        &authorizations,
        &metrics,
    );
    let user_id = Uuid::new_v4();
    let token_pair = tokens
        .issue_token_pair(user_id, vec!["ROLE_USER".into()])
        .expect("issue token");

    let decoded = use_case
        .decode_access_token(&token_pair.access_token)
        .expect("decode access token");

    assert_eq!(decoded, user_id);
}

#[tokio::test]
async fn callback_does_not_auto_link_unverified_email_to_existing_account() {
    let users = crate::support::InMemoryUserRepository::default();
    let user_id = crate::support::seed_active_user(&users, "victim@example.com")
        .await
        .expect("seed user");
    users
        .link_oauth_identity(
            user_id,
            (IdentityProvider::Email).into(),
            "victim-email",
            "victim@example.com",
            true,
        )
        .await
        .expect("email identity");
    let login_audits = crate::support::InMemoryAuditRepository::default();
    let auth_audits = crate::support::InMemoryAuthAuditRepository::default();
    let refresh_tokens = crate::support::InMemoryRefreshTokenStore::default();
    let tokens = crate::support::TestTokenService::default();
    let provider = FakeOAuthProvider::success(
        IdentityProvider::Google,
        "attacker-google-sub",
        "victim@example.com",
        false,
    );
    let authorizations = FakeOAuthAuthorizationStore::default();
    authorizations
        .insert(
            IdentityProvider::Google,
            "state-unverified-existing",
            "http://gateway.test/oauth/google/callback",
            "verifier-unverified-existing",
            "nonce-unverified-existing",
        )
        .await;
    let metrics = FakeOAuthMetrics::default();
    let use_case = oauth_login_use_case(
        &users,
        &login_audits,
        &auth_audits,
        &refresh_tokens,
        &tokens,
        &provider,
        &authorizations,
        &metrics,
    );

    let error = use_case
        .oauth_callback(
            IdentityProvider::Google,
            "unverified-code",
            "state-unverified-existing",
            None,
            None,
            None,
        )
        .await
        .expect_err("unverified email must not auto-link an existing account");

    assert!(matches!(
        Some(&error),
        Some(AuthError::InvalidInput(
            "Verified provider email is required to link an existing account"
        ))
    ));
    assert!(users
        .find_by_provider_subject((IdentityProvider::Google).into(), "attacker-google-sub")
        .await
        .expect("identity lookup")
        .is_none());
    assert_eq!(refresh_tokens.stored_count().await, 0);
    assert!(login_audits.audits.lock().await.is_empty());
}

#[tokio::test]
async fn callback_creates_pending_user_but_issues_no_tokens_for_unverified_email() {
    let users = crate::support::InMemoryUserRepository::default();
    let login_audits = crate::support::InMemoryAuditRepository::default();
    let auth_audits = crate::support::InMemoryAuthAuditRepository::default();
    let refresh_tokens = crate::support::InMemoryRefreshTokenStore::default();
    let tokens = crate::support::TestTokenService::default();
    let provider = FakeOAuthProvider::success(
        IdentityProvider::Google,
        "pending-google-sub",
        "pending@example.com",
        false,
    );
    let authorizations = FakeOAuthAuthorizationStore::default();
    authorizations
        .insert(
            IdentityProvider::Google,
            "state-unverified-new",
            "http://gateway.test/oauth/google/callback",
            "verifier-unverified-new",
            "nonce-unverified-new",
        )
        .await;
    let metrics = FakeOAuthMetrics::default();
    let use_case = oauth_login_use_case(
        &users,
        &login_audits,
        &auth_audits,
        &refresh_tokens,
        &tokens,
        &provider,
        &authorizations,
        &metrics,
    );

    let error = use_case
        .oauth_callback(
            IdentityProvider::Google,
            "unverified-code",
            "state-unverified-new",
            None,
            None,
            None,
        )
        .await
        .expect_err("pending user must not receive normal tokens");

    assert!(matches!(
        Some(&error),
        Some(AuthError::InvalidInput("Email verification required"))
    ));
    let user = UserRepositoryPort::find_by_email(&users, "pending@example.com")
        .await
        .expect("user lookup")
        .map(auth_service::domain::user::User::from)
        .expect("pending user");
    assert_eq!(user.status, UserStatus::PendingVerification);
    let identity = users
        .find_by_provider_subject((IdentityProvider::Google).into(), "pending-google-sub")
        .await
        .expect("identity lookup")
        .expect("pending identity");
    assert!(!identity.email_verified);
    assert_eq!(refresh_tokens.stored_count().await, 0);
    assert!(login_audits.audits.lock().await.is_empty());
}

#[derive(Clone)]
struct FixedAuthRuntime {
    now: Arc<Mutex<DateTime<Utc>>>,
}
impl auth_service::application::auth_runtime::AuthRuntimePort for FixedAuthRuntime {
    fn now(&self) -> DateTime<Utc> {
        *self.now.lock().expect("fixed clock")
    }
    fn new_id(&self) -> Uuid {
        Uuid::from_u128(42)
    }
}
struct RecordingOAuthCrypto {
    calls: std::sync::atomic::AtomicUsize,
    fail: bool,
}
impl auth_service::application::auth_runtime::OAuthAuthorizationCryptoPort
    for RecordingOAuthCrypto
{
    fn generate(
        &self,
    ) -> auth_service::application::errors::AuthResult<
        auth_service::application::dto::oauth_authorization_dto::OAuthAuthorizationMaterialDto,
    > {
        let result: anyhow::Result<_> = (|| {
            self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            anyhow::ensure!(!self.fail, "test credential generation unavailable");
            Ok(auth_service::application::dto::oauth_authorization_dto::OAuthAuthorizationMaterialDto {
            state: "fixed-state".into(), nonce: "fixed-nonce".into(), code_verifier: "fixed-verifier".into(), code_challenge: "fixed-challenge".into()
        })
        })();
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
}

#[tokio::test]
async fn oauth_authorization_uses_explicit_material_and_rejects_exact_expiry_before_exchange() {
    use chrono::TimeZone;
    let users = crate::support::InMemoryUserRepository::default();
    let login_audits = crate::support::InMemoryAuditRepository::default();
    let auth_audits = crate::support::InMemoryAuthAuditRepository::default();
    let refresh = crate::support::InMemoryRefreshTokenStore::default();
    let tokens = crate::support::TestTokenService::default();
    let provider = FakeOAuthProvider::success(
        IdentityProvider::Google,
        "fixture-subject",
        "fixture@example.com",
        true,
    );
    let authorizations = FakeOAuthAuthorizationStore::default();
    let metrics = FakeOAuthMetrics::default();
    let initial = Utc.timestamp_opt(1_700_000_000, 0).unwrap();
    let runtime = FixedAuthRuntime {
        now: Arc::new(Mutex::new(initial)),
    };
    let crypto = Arc::new(RecordingOAuthCrypto {
        calls: std::sync::atomic::AtomicUsize::new(0),
        fail: false,
    });
    let use_case = oauth_login_with_capabilities(
        &users,
        &login_audits,
        &auth_audits,
        &refresh,
        &tokens,
        &provider,
        &authorizations,
        &metrics,
        Arc::new(runtime.clone()),
        crypto.clone(),
    );
    use_case
        .authorize(IdentityProvider::Google)
        .await
        .expect("authorize");
    let saved = authorizations.saved_transactions().await;
    assert_eq!(saved.len(), 1);
    assert_eq!(saved[0].expires_at, initial + Duration::minutes(10));
    assert_eq!(saved[0].state, "fixed-state");
    assert_eq!(saved[0].nonce, "fixed-nonce");
    assert_eq!(saved[0].code_verifier, "fixed-verifier");
    assert_eq!(
        provider.authorization_calls()[0].code_challenge,
        "fixed-challenge"
    );
    assert_eq!(crypto.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    *runtime.now.lock().unwrap() = initial + Duration::minutes(10);
    let error = use_case
        .oauth_callback(
            IdentityProvider::Google,
            "code",
            "fixed-state",
            None,
            None,
            None,
        )
        .await
        .expect_err("exact expiry is rejected");
    assert_eq!(error.to_string(), "OAuth authorization state is invalid");
    assert!(provider.fetch_calls().is_empty());
    assert!(metrics.exchanges().is_empty());
    assert!(login_audits.audits.lock().await.is_empty());
}

#[tokio::test]
async fn failed_credential_generation_persists_nothing_and_never_builds_provider_uri() {
    let users = crate::support::InMemoryUserRepository::default();
    let login_audits = crate::support::InMemoryAuditRepository::default();
    let auth_audits = crate::support::InMemoryAuthAuditRepository::default();
    let refresh = crate::support::InMemoryRefreshTokenStore::default();
    let tokens = crate::support::TestTokenService::default();
    let provider = FakeOAuthProvider::success(
        IdentityProvider::Google,
        "fixture-subject",
        "fixture@example.com",
        true,
    );
    let authorizations = FakeOAuthAuthorizationStore::default();
    let metrics = FakeOAuthMetrics::default();
    let crypto = Arc::new(RecordingOAuthCrypto {
        calls: std::sync::atomic::AtomicUsize::new(0),
        fail: true,
    });
    let use_case = oauth_login_with_capabilities(
        &users,
        &login_audits,
        &auth_audits,
        &refresh,
        &tokens,
        &provider,
        &authorizations,
        &metrics,
        Arc::new(auth_service::adapters::out::runtime::SystemAuthRuntime),
        crypto.clone(),
    );
    use_case
        .authorize(IdentityProvider::Google)
        .await
        .expect_err("crypto failure");
    assert!(authorizations.saved_transactions().await.is_empty());
    assert!(provider.authorization_calls().is_empty());
    assert_eq!(crypto.calls.load(std::sync::atomic::Ordering::SeqCst), 1);
}
