#[path = "http_support/auth_client.rs"]
pub mod auth_client;
#[path = "http_support/friend_notification_clients.rs"]
pub mod friend_notification_clients;
#[path = "http_support/moderation_repository.rs"]
pub mod moderation_repository;
#[path = "http_support/post_client.rs"]
pub mod post_client;
#[path = "http_support/profile_client.rs"]
pub mod profile_client;

use api_gateway::application::dto::auth_dto::RetryUserDeletionRequestDto;
use api_gateway::application::dto::post_dto::ResolveDeadLetterReplayRequestDto;
use api_gateway::application::ports::{
    AuthClientPort, FriendClientPort, ModerationRepositoryPort, NotificationClientPort,
    PostClientPort, ProfileClientPort,
};
use api_gateway::config::{
    build_state_with_clients_and_moderation, AppConfig, AppState, RuntimeMode,
};
use api_gateway::domain::admission::AdmissionPolicies;
use axum::body::{to_bytes, Body};
use axum::http::Response;
use base64ct::{Base64UrlUnpadded, Encoding};
use chrono::Utc;
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use rand::rngs::OsRng;
use rsa::pkcs8::EncodePrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::Serialize;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use std::sync::{Arc, Mutex, OnceLock};

type DeletionListCall = (i32, Option<chrono::DateTime<Utc>>, Option<uuid::Uuid>);

const ISSUER: &str = "auth-service-test";
const AUDIENCE: &str = "port-rust-api-test";
const KID: &str = "api-gateway-route-test-key";

pub const USER_ID: &str = "11111111-1111-1111-1111-111111111111";
pub const OTHER_USER_ID: &str = "22222222-2222-2222-2222-222222222222";
/// Session family named by every token the harness mints, unless a test asks
/// for a token without one.
pub const SESSION_ID: &str = "33333333-3333-3333-3333-333333333333";

pub struct RouteHarness {
    pub state: Arc<AppState>,
    pub clients: Arc<TestClients>,
    pub moderation_repository: Arc<moderation_repository::TestModerationRepository>,
    /// The lazy pool behind every persistence port in `state`. Only some test
    /// targets that share this helper close it.
    #[allow(dead_code)]
    pub pool: sqlx::PgPool,
    jwt_key: &'static TestJwtKey,
}

impl RouteHarness {
    pub fn transport_router(&self) -> axum::Router {
        use api_gateway::adapters::r#in::http::{self, client_identity::TrustedProxies, server};
        server::with_transport_layers(
            http::router(self.state.clone()),
            &test_config(&self.jwt_key.jwks_json),
            TrustedProxies::parse("").expect("empty trusted-proxy set"),
        )
    }

    pub async fn new() -> Self {
        Self::with_configured_keys(shared_test_jwt_key().jwks_json.as_str())
            .await
            .expect("state")
    }

    /// A harness whose Gateway starts with `jwks_json` as its configured key
    /// set. An empty set means the keys are expected from the issuer.
    pub async fn with_configured_keys(jwks_json: &str) -> anyhow::Result<Self> {
        let config = test_config(jwks_json);
        Self::with_config(config).await
    }

    pub async fn with_admission(policies: AdmissionPolicies) -> anyhow::Result<Self> {
        let mut config = test_config(shared_test_jwt_key().jwks_json.as_str());
        config.admission = policies;
        Self::with_config(config).await
    }

    async fn with_config(config: AppConfig) -> anyhow::Result<Self> {
        // No database exists; a short acquire timeout lets best-effort writes
        // such as audit records fail within the request deadline.
        let pool = PgPoolOptions::new()
            .acquire_timeout(std::time::Duration::from_millis(100))
            .connect_lazy(&config.database_url)
            .expect("lazy pool");
        let clients = Arc::new(TestClients::default());
        let auth_client: Arc<dyn AuthClientPort> = clients.clone();
        Self::with_parts(config, pool, clients, auth_client).await
    }

    /// Owning real PG and actual Auth transport for composed OAuth acceptance.
    #[allow(dead_code)] // Used by the Auth integration binary sharing this harness.
    pub async fn with_pool_and_auth(
        pool: sqlx::PgPool,
        database_url: String,
        auth_client: Arc<dyn AuthClientPort>,
    ) -> anyhow::Result<Self> {
        let mut config = test_config(&shared_test_jwt_key().jwks_json);
        config.database_url = database_url;
        config.admission = config
            .admission
            .with_budget(api_gateway::domain::admission::RouteClass::Login, 128);
        Self::with_parts(config, pool, Arc::new(TestClients::default()), auth_client).await
    }

    async fn with_parts(
        config: AppConfig,
        pool: sqlx::PgPool,
        clients: Arc<TestClients>,
        auth_client: Arc<dyn AuthClientPort>,
    ) -> anyhow::Result<Self> {
        let jwt_key = shared_test_jwt_key();
        let profile_client: Arc<dyn ProfileClientPort> = clients.clone();
        let friend_client: Arc<dyn FriendClientPort> = clients.clone();
        let notification_client: Arc<dyn NotificationClientPort> = clients.clone();
        let post_client: Arc<dyn PostClientPort> = clients.clone();
        let moderation_repository =
            Arc::new(moderation_repository::TestModerationRepository::default());
        let moderation_port: Arc<dyn ModerationRepositoryPort> = moderation_repository.clone();
        let state = build_state_with_clients_and_moderation(
            &config,
            pool.clone(),
            auth_client,
            profile_client,
            friend_client,
            notification_client,
            post_client,
            moderation_port,
        )
        .await?;

        Ok(Self {
            state: Arc::new(state),
            clients,
            moderation_repository,
            pool,
            jwt_key,
        })
    }

    pub fn bearer_token(&self, user_id: &str) -> String {
        self.bearer_token_with_roles(user_id, vec!["ROLE_USER".into()])
    }

    pub fn bearer_token_with_roles(&self, user_id: &str, roles: Vec<String>) -> String {
        format!(
            "Bearer {}",
            self.jwt_key.access_token(user_id, roles, Some(SESSION_ID))
        )
    }

    /// A token as issued before access tokens named their session.
    pub fn bearer_token_without_session(&self, user_id: &str, roles: Vec<String>) -> String {
        format!("Bearer {}", self.jwt_key.access_token(user_id, roles, None))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LoginCall {
    pub email: String,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RelationshipCall {
    pub operation: String,
    pub viewer_id: String,
    pub target_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeletePostCall {
    pub user_id: String,
    pub post_id: String,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentModerationCall {
    pub report_id: String,
    pub target_type: String,
    pub target_id: String,
    pub actor_id: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DeleteAccountCall {
    pub user_id: String,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FeedRecommendationEventCall {
    pub viewer_id: String,
    pub request_id: String,
    pub post_id: String,
    pub impression_id: String,
    pub action: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeopleRecommendationCall {
    pub viewer_id: String,
    pub cursor: Option<String>,
    pub limit: i32,
    pub request_id: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PeopleRecommendationEventCall {
    pub viewer_id: String,
    pub request_id: String,
    pub candidate_user_id: String,
    pub action: String,
}

#[derive(Default)]
pub struct TestClients {
    pub auth_calls: Mutex<Vec<&'static str>>,
    pub notification_items:
        Mutex<Option<Vec<api_gateway::application::dto::notification_dto::NotificationResponse>>>,
    pub notification_post_access: Mutex<std::collections::HashMap<String, bool>>,
    pub notification_authorization_batches: std::sync::atomic::AtomicUsize,
    pub notification_authorization_budgets: Mutex<Vec<std::time::Duration>>,
    pub notification_authorization_delay: Mutex<std::time::Duration>,
    pub notification_post_unavailable: std::sync::atomic::AtomicBool,
    pub notification_post_checks: Mutex<Vec<(String, String)>>,
    pub notification_mark_read_calls: Mutex<Vec<(String, String)>>,
    pub notification_mark_read_delay: Mutex<std::time::Duration>,
    pub login_calls: Mutex<Vec<LoginCall>>,
    pub refresh_calls: Mutex<Vec<String>>,
    pub logout_calls: Mutex<Vec<String>>,
    pub oauth_callback_calls: Mutex<Vec<api_gateway::domain::IdentityProvider>>,
    pub create_post_calls: Mutex<Vec<String>>,
    pub delete_post_calls: Mutex<Vec<DeletePostCall>>,
    pub relationship_calls: Mutex<Vec<RelationshipCall>>,
    pub delete_account_calls: Mutex<Vec<DeleteAccountCall>>,
    pub get_deletion_calls: Mutex<Vec<String>>,
    pub get_deletion_by_saga_id_calls: Mutex<Vec<String>>,
    pub list_dead_lettered_deletion_calls: Mutex<Vec<DeletionListCall>>,
    pub retry_user_deletion_calls: Mutex<Vec<RetryUserDeletionRequestDto>>,
    pub content_moderation_calls: Mutex<Vec<ContentModerationCall>>,
    pub content_moderation_result: Mutex<Option<bool>>,
    pub dead_letter_replay_calls: Mutex<Vec<ResolveDeadLetterReplayRequestDto>>,
    pub feed_recommendation_event_calls: Mutex<Vec<FeedRecommendationEventCall>>,
    pub people_recommendation_calls: Mutex<Vec<PeopleRecommendationCall>>,
    pub people_recommendation_event_calls: Mutex<Vec<PeopleRecommendationEventCall>>,
    avatar_content_type: Mutex<String>,
    /// What the fake issuer answers to high-risk session checks.
    session_answer: Mutex<SessionAnswer>,
    /// Every session check as (user id, session id).
    pub session_checks: Mutex<Vec<(String, String)>>,
}

/// The fake issuer's answer to a session check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SessionAnswer {
    Active(Vec<String>),
    Ended,
    /// The check fails, as when the issuer cannot be reached.
    Unavailable,
}

impl Default for SessionAnswer {
    /// A live session holding both roles, so the claims in the token decide,
    /// as they did before sessions were checked.
    fn default() -> Self {
        Self::Active(vec!["ROLE_USER".into(), "ROLE_ADMIN".into()])
    }
}

impl TestClients {
    pub fn answer_sessions(&self, answer: SessionAnswer) {
        *self.session_answer.lock().expect("session answer") = answer;
    }

    pub fn session_answer(&self) -> SessionAnswer {
        self.session_answer.lock().expect("session answer").clone()
    }

    pub fn avatar_content_type(&self, content_type: &str) {
        *self
            .avatar_content_type
            .lock()
            .expect("avatar content type") = content_type.to_owned();
    }

    fn current_avatar_content_type(&self) -> String {
        let content_type = self
            .avatar_content_type
            .lock()
            .expect("avatar content type")
            .clone();
        if content_type.is_empty() {
            "image/png".to_owned()
        } else {
            content_type
        }
    }
}

pub async fn response_json(response: Response<Body>) -> Value {
    let bytes = to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("json body")
}

fn test_config(jwks_json: &str) -> AppConfig {
    AppConfig {
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        http_addr: "127.0.0.1:18080".parse().expect("addr"),
        metrics_addr: "127.0.0.1:0".parse().expect("metrics addr"),
        grpc_addr: "127.0.0.1:0".parse().expect("grpc addr"),
        database_pool_max_connections: 5,
        journal_pool_max_connections: 4,
        background_pool_max_connections: 3,
        // Route tests run without a database. Port 1 refuses connections, so a
        // write from an audit layer fails at once instead of reaching a
        // PostgreSQL a developer happens to run on the default port.
        database_url: "postgres://postgres:postgres@127.0.0.1:1/gateway_test".into(),
        seed_enabled: false,
        auth_grpc_endpoint: "http://127.0.0.1:50051".into(),
        concert_grpc_endpoint: None,
        concert_release_digest: None,
        profile_grpc_endpoint: "http://127.0.0.1:50052".into(),
        friend_grpc_endpoint: "http://127.0.0.1:50053".into(),
        notification_grpc_endpoint: "http://127.0.0.1:50054".into(),
        post_grpc_endpoint: "http://127.0.0.1:50055".into(),
        refresh_cookie_domain: String::new(),
        refresh_cookie_secure: false,
        refresh_cookie_max_age_seconds: 1209600,
        aggregation_cache_ttl_seconds: 30,
        cors_allowed_origins: vec!["http://localhost:3000".into()],
        jwt_issuer: ISSUER.into(),
        jwt_audience: AUDIENCE.into(),
        jwt_jwks_json: jwks_json.to_owned(),
        jwt_keys_refresh_seconds: 0,
        jwt_keys_min_refresh_seconds: 10,
        high_risk_session_freshness_seconds: 5,
        runtime_mode: RuntimeMode::Parity,
        developer_portal_links: api_gateway::config::DeveloperPortalLinks::defaults(
            RuntimeMode::Parity,
        ),
        rate_limit_per_minute: 600,
        admission: api_gateway::domain::admission::AdmissionPolicies::default(),
        admission_hmac_key: "test-admission-hmac-key-with-at-least-32-bytes".into(),
        search_cursor_hmac_key: "test-search-cursor-key-has-32-bytes-minimum".into(),
        search_cursor_ttl_seconds: 900,
        search_cursor_legacy_accept_until: None,
        request_timeout_ms: 1000,
        journal_client_timeout_ms: 900,
        async_operation_keyring: None,
        telemetry_retention_days: 30,
        telemetry_queue_capacity: 4096,
        audit_retention_days: 365,
        // The edge accepts no internal callers; it only sends them.
        actor_delegation_signer: platform::actor_delegation::ActorDelegationSigner::local_parity(
            platform::grpc_auth::Workload::Gateway,
        ),
        caller_registry: platform::grpc_auth::CallerRegistry::new(),
        authority_exchange: api_gateway::config::authority_exchange(
            false,
            &platform::actor_delegation::ActorDelegationSigner::local_parity(
                platform::grpc_auth::Workload::Gateway,
            ),
            false,
            platform::actor_delegation::exchange::ResourceEvidenceVerifier::local_parity(),
        )
        .expect("parity authority exchange"),
        outbound_identity: "unit-test-token".into(),
        tls_material: None,
    }
}

fn shared_test_jwt_key() -> &'static TestJwtKey {
    static KEY: OnceLock<TestJwtKey> = OnceLock::new();
    KEY.get_or_init(TestJwtKey::new)
}

struct TestJwtKey {
    encoding: EncodingKey,
    jwks_json: String,
}

impl TestJwtKey {
    fn new() -> Self {
        let private_key = RsaPrivateKey::new(&mut OsRng, 2048).expect("test RSA key");
        let public_key = RsaPublicKey::from(&private_key);
        let private_pem = private_key
            .to_pkcs8_pem(Default::default())
            .expect("private pem");
        let jwks_json = serde_json::json!({
            "keys": [{
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": KID,
                "n": Base64UrlUnpadded::encode_string(&public_key.n().to_bytes_be()),
                "e": Base64UrlUnpadded::encode_string(&public_key.e().to_bytes_be())
            }]
        })
        .to_string();
        Self {
            encoding: EncodingKey::from_rsa_pem(private_pem.as_bytes()).expect("encoding key"),
            jwks_json,
        }
    }

    fn access_token(&self, user_id: &str, roles: Vec<String>, sid: Option<&str>) -> String {
        let now = Utc::now().timestamp();
        let claims = TestAccessClaims {
            sub: user_id.to_owned(),
            iss: ISSUER,
            aud: AUDIENCE,
            iat: now,
            exp: now + 900,
            token_type: "access",
            roles,
            sid: sid.map(ToOwned::to_owned),
        };
        let header = Header {
            alg: Algorithm::RS256,
            kid: Some(KID.to_owned()),
            ..Header::default()
        };
        jsonwebtoken::encode(&header, &claims, &self.encoding).expect("signed token")
    }
}

#[derive(Serialize)]
struct TestAccessClaims<'a> {
    sub: String,
    iss: &'a str,
    aud: &'a str,
    iat: i64,
    exp: i64,
    token_type: &'a str,
    roles: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    sid: Option<String>,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn route_harness_reuses_single_jwt_key_material() {
        assert!(std::ptr::eq(shared_test_jwt_key(), shared_test_jwt_key()));
    }
}
