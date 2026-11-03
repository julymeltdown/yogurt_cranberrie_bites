#![allow(dead_code)]

pub mod controlled_runtime;
pub mod owned_database;
pub mod security;

use anyhow::Result;
use async_trait::async_trait;
use auth_service::adapters::out::security::admitted_argon2::{
    hash_password, AdmittedArgon2PasswordWork,
};
use auth_service::adapters::r#in::grpc::server::AuthGrpcServer;
use auth_service::adapters::r#in::http;
use auth_service::application::password_work::{PasswordWork, PasswordWorkLimits};
use auth_service::application::ports::{
    AuthAuditRepositoryPort, LoginAuditRepositoryPort, OAuthIdentityRepositoryPort,
    PasswordResetStorePort, RefreshTokenStorePort, ResetMailerPort, TokenServicePort,
    UserRepositoryPort, VerificationRepositoryPort,
};
use auth_service::config::{
    build_state, build_state_with_password_work, connect_pool, AppConfig, AppState,
    JwtTokenService, PasswordResetMailerMode, RuntimeMode,
};
use auth_service::domain::oauth_identity::OAuthIdentity;
use auth_service::domain::password_reset::PasswordResetToken;
use auth_service::domain::session::{
    AuthAuditEvent, LoginAudit, RefreshTokenRecord, RefreshTokenState, RotationOutcome,
};
use auth_service::domain::user::{IdentityProvider, User, UserStatus};
use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use axum::Router;
use chrono::{DateTime, Utc};
use http_body_util::BodyExt;
use rand::rngs::StdRng;
use rand::SeedableRng;
use rsa::pkcs8::EncodePrivateKey;
use rsa::RsaPrivateKey;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use testcontainers::ContainerAsync;
use testcontainers_modules::postgres::Postgres;
use tokio::sync::Mutex;
use tower::util::ServiceExt;
use uuid::Uuid;

pub fn shared_test_rsa_private_key() -> &'static RsaPrivateKey {
    static KEY: OnceLock<RsaPrivateKey> = OnceLock::new();
    KEY.get_or_init(|| {
        let mut rng = StdRng::seed_from_u64(0x504F_5254_5255_5354);
        RsaPrivateKey::new(&mut rng, 2048).expect("deterministic test RSA key")
    })
}

pub fn shared_test_rsa_private_key_pem() -> &'static str {
    static PEM: OnceLock<String> = OnceLock::new();
    PEM.get_or_init(|| {
        shared_test_rsa_private_key()
            .to_pkcs8_pem(Default::default())
            .expect("test private key PEM")
            .to_string()
    })
}

/// A second deterministic key, for tests that rotate from one signing key to
/// another. Its seed differs from the shared key's, so the two never collide.
pub fn rotation_test_rsa_private_key_pem() -> &'static str {
    static PEM: OnceLock<String> = OnceLock::new();
    PEM.get_or_init(|| {
        let mut rng = StdRng::seed_from_u64(0x524F_5441_5445_4B32);
        RsaPrivateKey::new(&mut rng, 2048)
            .expect("deterministic rotation RSA key")
            .to_pkcs8_pem(Default::default())
            .expect("rotation private key PEM")
            .to_string()
    })
}

pub fn test_jwt_service() -> JwtTokenService {
    JwtTokenService::from_private_key_pem(
        "unit-test",
        "port-rust-api-test",
        900,
        1_209_600,
        shared_test_rsa_private_key_pem(),
        "unit-test-key",
    )
    .expect("test token service")
}

/// One occupied slot of a password-work class.
///
/// The computation blocks until the slot is released. Dropping the slot without
/// releasing it also ends the computation, because the blocked receiver then sees
/// a closed channel, so a failed test cannot leave a worker thread behind.
pub struct OccupiedSlot {
    release: std::sync::mpsc::Sender<()>,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
}

impl OccupiedSlot {
    pub async fn release(self) {
        let _ = self.release.send(());
        self.task
            .await
            .expect("occupying task")
            .expect("occupying computation");
    }
}

/// Hold one slot of `class` with a blocking computation until released.
pub async fn occupy(class: &platform::cpu::AdmissionClass) -> OccupiedSlot {
    let (started_tx, started_rx) = tokio::sync::oneshot::channel();
    let (release, release_rx) = std::sync::mpsc::channel::<()>();
    let class = class.clone();
    let task = tokio::spawn(async move {
        class
            .run(move || {
                let _ = started_tx.send(());
                release_rx.recv()?;
                Ok(())
            })
            .await
    });
    started_rx.await.expect("occupying computation started");
    OccupiedSlot { release, task }
}

/// Small password-work limits for saturation tests: three workers, one each
/// for signup and reset, so exactly one stays reserved for login.
pub fn small_password_work(login_queue: usize) -> Arc<AdmittedArgon2PasswordWork> {
    Arc::new(
        AdmittedArgon2PasswordWork::new(PasswordWorkLimits {
            workers: 3,
            login_queue,
            signup_workers: 1,
            signup_queue: 0,
            reset_workers: 1,
            reset_queue: 0,
        })
        .expect("valid small password work limits"),
    )
}

pub struct TestApp {
    pub _postgres: Option<ContainerAsync<Postgres>>,
    pub pool: sqlx::PgPool,
    pub state: Arc<AppState>,
    pub router: Router,
    /// The configuration the app was built from, so a test can build further
    /// instances against the same database with changed settings.
    pub config: AppConfig,
}

pub async fn start_postgres_app(seed_enabled: bool) -> Result<TestApp> {
    start_postgres_app_with(seed_enabled, None).await
}

/// Start the app with explicit password-work limits, so a test can hold a class
/// busy and observe the transport's answer.
pub async fn start_postgres_app_with_password_work(
    seed_enabled: bool,
    password_work: PasswordWork,
) -> Result<TestApp> {
    start_postgres_app_with(seed_enabled, Some(password_work)).await
}

async fn start_postgres_app_with(
    seed_enabled: bool,
    password_work: Option<PasswordWork>,
) -> Result<TestApp> {
    let (postgres, database_url) = if let Some(url) = owned_database::native_database_url().await? {
        (None, url)
    } else {
        let (postgres, database_url) = test_support::postgres::start_postgres_with_url().await?;
        (Some(postgres), database_url)
    };
    let config = AppConfig {
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        http_addr: "127.0.0.1:18081".parse()?,
        metrics_addr: "127.0.0.1:0".parse()?,
        grpc_addr: "127.0.0.1:15051".parse()?,
        database_pool_max_connections: 5,
        database_url,
        seed_enabled,
        // Tests exercise the application routes directly, without the Gateway.
        public_application_routes_enabled: true,
        runtime_mode: RuntimeMode::Parity,
        access_ttl_seconds: 900,
        refresh_ttl_seconds: 1209600,
        issuer: "auth-service-test".into(),
        access_audience: "port-rust-api-test".into(),
        google_redirect_uri: "http://127.0.0.1:8080/auth/oauth/google/callback".into(),
        apple_redirect_uri: "http://127.0.0.1:8080/auth/oauth/apple/callback".into(),
        x_redirect_uri: "http://127.0.0.1:8080/auth/oauth/x/callback".into(),
        google_authorize_base_uri: "https://accounts.google.test/o/oauth2/v2/auth".into(),
        apple_authorize_base_uri: "https://appleid.apple.test/auth/authorize".into(),
        x_authorize_base_uri: "https://x.test/i/oauth2/authorize".into(),
        google_client_id: None,
        google_client_secret: None,
        google_token_uri: None,
        google_userinfo_uri: None,
        google_issuer: "https://accounts.google.test".into(),
        google_jwks_json: None,
        google_jwks_uri: None,
        apple_client_id: None,
        apple_client_secret: None,
        apple_team_id: None,
        apple_key_id: None,
        apple_private_key_pem: None,
        apple_client_secret_ttl_seconds: 86_400,
        apple_token_uri: None,
        apple_userinfo_uri: None,
        apple_issuer: "https://appleid.apple.test".into(),
        apple_jwks_json: None,
        apple_jwks_uri: None,
        x_client_id: None,
        x_client_secret: None,
        x_token_uri: None,
        x_userinfo_uri: None,
        jwt_private_key_pem: None,
        jwt_key_id: None,
        jwt_rotation_jwks_json: None,
        actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
            platform::grpc_auth::Workload::Auth,
        ),
        caller_registry: platform::grpc_auth::CallerRegistry::new()
            .register(
                platform::grpc_auth::Workload::Gateway,
                "test-gateway-credential",
            )
            .expect("gateway caller"),
        outbound_identity: "test-internal-service-token".into(),
        tls_material: None,
        login_lockout_max_failures: 5,
        login_lockout_window_seconds: 900,
        password_work: auth_service::application::password_work::PasswordWorkLimits::default(),
        password_reset_base_url: "http://localhost:5174/auth/reset".into(),
        password_reset_mailer_mode: PasswordResetMailerMode::Logging,
        password_reset_mailer_endpoint: None,
        password_reset_mailer_bearer_token: None,
        lifecycle_cleanup_worker_enabled: true,
        lifecycle_cleanup_worker_interval_ms: 1000,
        profile_grpc_endpoint: "http://127.0.0.1:50052".into(),
        friend_grpc_endpoint: "http://127.0.0.1:50053".into(),
        notification_grpc_endpoint: "http://127.0.0.1:50054".into(),
        post_grpc_endpoint: "http://127.0.0.1:50055".into(),
        user_deletion_worker_enabled: true,
        user_deletion_worker_interval_ms: 1_000,
        user_deletion_worker_batch_size: 10,
        user_deletion_worker_max_attempts: 3,
        user_deletion_worker_lease_timeout_ms: 300_000,
        user_deletion_worker_base_retry_ms: 1_000,
    };
    let pool = connect_pool(&config).await?;
    let state = Arc::new(match password_work {
        Some(password_work) => {
            build_state_with_password_work(&config, pool.clone(), password_work).await?
        }
        None => build_state(&config, pool.clone()).await?,
    });
    let router = http::router(state.clone());
    Ok(TestApp {
        _postgres: postgres,
        pool,
        state,
        router,
        config,
    })
}

impl TestApp {
    pub async fn metrics(&self) -> Result<String> {
        let response = http::metrics_router(self.state.metrics.clone())
            .oneshot(
                Request::builder()
                    .method(Method::GET)
                    .uri("/metrics")
                    .body(Body::empty())?,
            )
            .await?;
        Ok(String::from_utf8(
            response.into_body().collect().await?.to_bytes().to_vec(),
        )?)
    }
}

pub async fn json_response(
    router: Router,
    request: Request<Body>,
) -> Result<(StatusCode, serde_json::Value, axum::http::HeaderMap)> {
    let response = router.oneshot(request).await?;
    let status = response.status();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await?.to_bytes();
    let json = if body.is_empty() {
        serde_json::Value::Null
    } else {
        serde_json::from_slice(&body)?
    };
    Ok((status, json, headers))
}

#[derive(Clone, Default)]
pub struct InMemoryUserRepository {
    users_by_email: Arc<Mutex<HashMap<String, User>>>,
    users_by_id: Arc<Mutex<HashMap<Uuid, User>>>,
    identities: Arc<Mutex<Vec<OAuthIdentity>>>,
}

impl InMemoryUserRepository {
    pub async fn remove_by_id(&self, user_id: Uuid) -> Option<User> {
        let removed = self.users_by_id.lock().await.remove(&user_id);
        if let Some(user) = &removed {
            self.users_by_email.lock().await.remove(&user.primary_email);
        }
        removed
    }
}

#[async_trait]
impl UserRepositoryPort for InMemoryUserRepository {
    async fn find_by_email(
        &self,
        email: &str,
    ) -> auth_service::application::errors::AuthResult<
        Option<auth_service::application::dto::repository_dto::AuthUserRecordDto>,
    > {
        let result: anyhow::Result<_> =
            async { Ok(self.users_by_email.lock().await.get(email).cloned()) }.await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn find_by_id(
        &self,
        user_id: Uuid,
    ) -> auth_service::application::errors::AuthResult<
        Option<auth_service::application::dto::repository_dto::AuthUserRecordDto>,
    > {
        let result: anyhow::Result<_> =
            async { Ok(self.users_by_id.lock().await.get(&user_id).cloned()) }.await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn save(
        &self,
        user: &auth_service::application::dto::repository_dto::AuthUserRecordDto,
    ) -> auth_service::application::errors::AuthResult<()> {
        let user = &auth_service::domain::user::User::from(user.clone());

        let result: anyhow::Result<_> = async {
            self.users_by_email
                .lock()
                .await
                .insert(user.primary_email.clone(), user.clone());
            self.users_by_id.lock().await.insert(user.id, user.clone());
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn update_password_hash(
        &self,
        user_id: Uuid,
        password_hash: &str,
    ) -> auth_service::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
            if let Some(user) = self.users_by_id.lock().await.get_mut(&user_id) {
                user.password_hash = password_hash.to_owned();
                self.users_by_email
                    .lock()
                    .await
                    .insert(user.primary_email.clone(), user.clone());
            }
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn link_email_identity(
        &self,
        _user_id: Uuid,
        _email: &str,
        _verified: bool,
    ) -> auth_service::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async { Ok(()) }.await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn mark_email_verified(
        &self,
        _user_id: Uuid,
        _email: &str,
    ) -> auth_service::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async { Ok(()) }.await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn linked_providers(
        &self,
        _user_id: Uuid,
    ) -> auth_service::application::errors::AuthResult<
        Vec<auth_service::application::dto::repository_dto::AuthIdentityProviderDto>,
    > {
        let result: anyhow::Result<_> = async {
            let identities = self.identities.lock().await;
            Ok(identities
                .iter()
                .filter(|identity| identity.user_id == _user_id)
                .map(|identity| identity.provider)
                .collect())
        }
        .await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value: Vec<IdentityProvider>| value.into_iter().map(Into::into).collect())
    }

    async fn soft_delete_user(
        &self,
        user_id: Uuid,
        _actor_id: Uuid,
        _reason: &str,
        _retention_until: DateTime<Utc>,
    ) -> auth_service::application::errors::AuthResult<bool> {
        let result: anyhow::Result<_> = async {
            let mut users_by_id = self.users_by_id.lock().await;
            let Some(user) = users_by_id.get_mut(&user_id) else {
                return Ok(false);
            };
            user.status = UserStatus::Deleted;
            self.users_by_email
                .lock()
                .await
                .insert(user.primary_email.clone(), user.clone());
            Ok(true)
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
}

#[async_trait]
impl OAuthIdentityRepositoryPort for InMemoryUserRepository {
    async fn find_by_provider_subject(
        &self,
        provider: auth_service::application::dto::repository_dto::AuthIdentityProviderDto,
        subject: &str,
    ) -> auth_service::application::errors::AuthResult<
        Option<auth_service::application::dto::repository_dto::AuthOAuthIdentityRecordDto>,
    > {
        let provider = auth_service::domain::user::IdentityProvider::from(provider);
        let result: anyhow::Result<_> = async {
            Ok(self
                .identities
                .lock()
                .await
                .iter()
                .find(|identity| identity.provider == provider && identity.subject == subject)
                .cloned())
        }
        .await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value| value.map(Into::into))
    }

    async fn find_by_email(
        &self,
        email: &str,
    ) -> auth_service::application::errors::AuthResult<
        Vec<auth_service::application::dto::repository_dto::AuthOAuthIdentityRecordDto>,
    > {
        let result: anyhow::Result<_> = async {
            Ok(self
                .identities
                .lock()
                .await
                .iter()
                .filter(|identity| identity.email == email)
                .cloned()
                .collect())
        }
        .await;
        result
            .map_err(auth_service::adapters::out::errors::boundary_error)
            .map(|value: Vec<OAuthIdentity>| value.into_iter().map(Into::into).collect())
    }

    async fn link_oauth_identity(
        &self,
        user_id: Uuid,
        provider: auth_service::application::dto::repository_dto::AuthIdentityProviderDto,
        subject: &str,
        email: &str,
        email_verified: bool,
    ) -> auth_service::application::errors::AuthResult<()> {
        let provider = auth_service::domain::user::IdentityProvider::from(provider);

        let result: anyhow::Result<_> = async {
            let mut identities = self.identities.lock().await;
            if let Some(existing) = identities
                .iter_mut()
                .find(|identity| identity.provider == provider && identity.subject == subject)
            {
                existing.user_id = user_id;
                existing.email = email.to_owned();
                existing.email_verified = email_verified;
                return Ok(());
            }
            identities.push(OAuthIdentity {
                user_id,
                provider,
                subject: subject.to_owned(),
                email: email.to_owned(),
                email_verified,
            });
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
}

#[derive(Clone, Default)]
pub struct InMemoryVerificationRepository {
    codes: Arc<Mutex<HashMap<String, (Uuid, String)>>>,
}

impl InMemoryVerificationRepository {
    pub async fn stored_code(&self, email: &str) -> Option<String> {
        self.codes
            .lock()
            .await
            .get(email)
            .map(|(_, code)| code.clone())
    }
}

#[async_trait]
impl VerificationRepositoryPort for InMemoryVerificationRepository {
    async fn save_code(
        &self,
        user_id: Uuid,
        email: &str,
        code: &str,
    ) -> auth_service::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
            self.codes
                .lock()
                .await
                .insert(email.to_owned(), (user_id, code.to_owned()));
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn consume_code(
        &self,
        email: &str,
        code: &str,
    ) -> auth_service::application::errors::AuthResult<Option<Uuid>> {
        let result: anyhow::Result<_> = async {
            let mut codes = self.codes.lock().await;
            match codes.get(email) {
                Some((_user_id, stored)) if stored == code => {
                    Ok(codes.remove(email).map(|entry| entry.0))
                }
                _ => Ok(None),
            }
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
}

#[derive(Clone, Default)]
pub struct InMemoryRefreshTokenStore {
    entries: Arc<Mutex<HashMap<String, StoredRefreshToken>>>,
}

/// One stored token plus its lifecycle state.
///
/// The real store keeps revoked rows and marks them, rather than deleting them,
/// because a later presentation must be classifiable as a benign rotation race, a
/// stale logout, or reuse. A double that deleted on revoke could not represent
/// that distinction and would hide the difference between the two behaviours.
#[derive(Clone)]
struct StoredRefreshToken {
    record: RefreshTokenRecord,
    rotated_at: Option<DateTime<Utc>>,
    successor_jti: Option<String>,
    revoked_reason: Option<String>,
}

impl InMemoryRefreshTokenStore {
    /// Count tokens that are still usable, matching the old double's meaning.
    pub async fn stored_count(&self) -> usize {
        self.entries
            .lock()
            .await
            .values()
            .filter(|stored| stored.revoked_reason.is_none())
            .count()
    }
}

#[async_trait]
impl RefreshTokenStorePort for InMemoryRefreshTokenStore {
    async fn store(
        &self,
        record: &auth_service::application::dto::repository_dto::AuthRefreshRecordDto,
    ) -> auth_service::application::errors::AuthResult<()> {
        let record = &auth_service::domain::session::RefreshTokenRecord::from(record.clone());

        let result: anyhow::Result<_> = async {
            self.entries.lock().await.insert(
                record.jti.clone(),
                StoredRefreshToken {
                    record: record.clone(),
                    rotated_at: None,
                    successor_jti: None,
                    revoked_reason: None,
                },
            );
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn find(
        &self,
        jti: &str,
    ) -> auth_service::application::errors::AuthResult<
        Option<auth_service::application::dto::repository_dto::AuthRefreshRecordDto>,
    > {
        let result = async {
            let result: anyhow::Result<_> = async {
                Ok(self.entries.lock().await.get(jti).and_then(|stored| {
                    (stored.revoked_reason.is_none() && stored.record.expires_at > Utc::now())
                        .then(|| stored.record.clone())
                }))
            }
            .await;
            result.map_err(auth_service::adapters::out::errors::boundary_error)
        }
        .await;
        result.map(|value| value.map(Into::into))
    }

    async fn rotate(
        &self,
        current_jti: &str,
        current_hash: &str,
        next: &auth_service::application::dto::repository_dto::AuthRefreshRecordDto,
    ) -> auth_service::application::errors::AuthResult<
        auth_service::application::dto::repository_dto::AuthRotationOutcomeDto,
    > {
        let next = &auth_service::domain::session::RefreshTokenRecord::from(next.clone());
        let result = async {
            let result: anyhow::Result<_> = async {
                let mut entries = self.entries.lock().await;
                // Clone the entry out first so the map is free to mutate while deciding.
                let Some(current) = entries.get(current_jti).cloned() else {
                    return Ok(RotationOutcome::NotRotatable);
                };
                let hash_matches = current.record.token_hash == current_hash;
                if hash_matches && current.revoked_reason.is_none() {
                    let rotated_at = Utc::now();
                    entries.insert(
                        current_jti.to_owned(),
                        StoredRefreshToken {
                            rotated_at: Some(rotated_at),
                            successor_jti: Some(next.jti.clone()),
                            revoked_reason: Some("ROTATED".to_owned()),
                            ..current
                        },
                    );
                    entries.insert(
                        next.jti.clone(),
                        StoredRefreshToken {
                            record: next.clone(),
                            rotated_at: None,
                            successor_jti: None,
                            revoked_reason: None,
                        },
                    );
                    return Ok(RotationOutcome::Rotated);
                }
                match (current.rotated_at, current.successor_jti) {
                    (Some(rotated_at), Some(successor_jti)) => {
                        Ok(RotationOutcome::AlreadyRotated {
                            successor_jti,
                            rotated_at,
                        })
                    }
                    _ => Ok(RotationOutcome::NotRotatable),
                }
            }
            .await;
            result.map_err(auth_service::adapters::out::errors::boundary_error)
        }
        .await;
        result.map(|value| value.into())
    }

    async fn find_state(
        &self,
        jti: &str,
        now: DateTime<Utc>,
    ) -> auth_service::application::errors::AuthResult<
        Option<auth_service::application::dto::repository_dto::AuthRefreshStateDto>,
    > {
        let result = async {
            let result: anyhow::Result<_> = async {
                let entries = self.entries.lock().await;
                let Some(stored) = entries.get(jti) else {
                    return Ok(None);
                };
                if stored.record.expires_at <= now {
                    return Ok(None);
                }
                if let (Some(rotated_at), Some(successor_jti)) =
                    (stored.rotated_at, stored.successor_jti.clone())
                {
                    return Ok(Some(RefreshTokenState::Rotated {
                        successor_jti,
                        rotated_at,
                        family_id: stored.record.family_id,
                        user_id: stored.record.user_id,
                    }));
                }
                match stored.revoked_reason.as_deref() {
                    Some("LOGOUT") => Ok(Some(RefreshTokenState::RevokedByLogout)),
                    Some(_) => Ok(Some(RefreshTokenState::Revoked)),
                    None => Ok(Some(RefreshTokenState::Revoked)),
                }
            }
            .await;
            result.map_err(auth_service::adapters::out::errors::boundary_error)
        }
        .await;
        result.map(|value| value.map(Into::into))
    }

    async fn revoke(&self, jti: &str) -> auth_service::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
            let mut entries = self.entries.lock().await;
            let family = entries.get(jti).map(|stored| stored.record.family_id);
            if let Some(family) = family {
                for stored in entries.values_mut() {
                    if stored.record.family_id == family && stored.revoked_reason.is_none() {
                        stored.revoked_reason = Some("LOGOUT".to_owned());
                    }
                }
            }
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn revoke_family(
        &self,
        family_id: Uuid,
    ) -> auth_service::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
            for stored in self.entries.lock().await.values_mut() {
                if stored.record.family_id == family_id && stored.revoked_reason.is_none() {
                    stored.revoked_reason = Some("FAMILY_REUSE".to_owned());
                }
            }
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn revoke_all_for_user(
        &self,
        user_id: Uuid,
    ) -> auth_service::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
            for stored in self.entries.lock().await.values_mut() {
                if stored.record.user_id == user_id && stored.revoked_reason.is_none() {
                    stored.revoked_reason = Some("USER_REVOKED".to_owned());
                }
            }
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn session_is_live(
        &self,
        user_id: Uuid,
        family_id: Uuid,
        now: DateTime<Utc>,
    ) -> auth_service::application::errors::AuthResult<bool> {
        let result: anyhow::Result<_> = async {
            Ok(self.entries.lock().await.values().any(|stored| {
                stored.record.family_id == family_id
                    && stored.record.user_id == user_id
                    && stored.revoked_reason.is_none()
                    && stored.record.expires_at > now
            }))
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
}

#[derive(Clone, Default)]
pub struct InMemoryAuditRepository {
    pub audits: Arc<Mutex<Vec<LoginAudit>>>,
}

#[async_trait]
impl LoginAuditRepositoryPort for InMemoryAuditRepository {
    async fn record(
        &self,
        audit: &auth_service::application::dto::repository_dto::AuthLoginAuditRecordDto,
    ) -> auth_service::application::errors::AuthResult<()> {
        let audit = &auth_service::domain::session::LoginAudit::from(audit.clone());

        let result: anyhow::Result<_> = async {
            self.audits.lock().await.push(audit.clone());
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn count_recent_failures(
        &self,
        user_id: Uuid,
        since: DateTime<Utc>,
    ) -> auth_service::application::errors::AuthResult<i64> {
        let result: anyhow::Result<_> = async {
            Ok(self
                .audits
                .lock()
                .await
                .iter()
                .filter(|audit| {
                    // Mirror the SQL predicate exactly: only genuine credential
                    // failures count, so refusals during an existing lockout cannot
                    // extend that lockout. A test double that drifts from the real
                    // query would hide the production behaviour.
                    audit.user_id == user_id
                        && !audit.success
                        && audit.credential_attempt
                        && audit.occurred_at >= since
                })
                .count() as i64)
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
}

#[derive(Clone, Default)]
pub struct InMemoryAuthAuditRepository {
    pub audits: Arc<Mutex<Vec<AuthAuditEvent>>>,
}

#[async_trait]
impl AuthAuditRepositoryPort for InMemoryAuthAuditRepository {
    async fn record(
        &self,
        event: &auth_service::application::dto::repository_dto::AuthAuditEventRecordDto,
    ) -> auth_service::application::errors::AuthResult<()> {
        let event = &auth_service::domain::session::AuthAuditEvent::from(event.clone());

        let result: anyhow::Result<_> = async {
            self.audits.lock().await.push(event.clone());
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
}

#[derive(Clone, Default)]
pub struct InMemoryPasswordResetStore {
    entries: Arc<Mutex<HashMap<String, PasswordResetToken>>>,
}

#[async_trait]
impl PasswordResetStorePort for InMemoryPasswordResetStore {
    async fn save(
        &self,
        token: &auth_service::application::dto::repository_dto::AuthPasswordResetRecordDto,
    ) -> auth_service::application::errors::AuthResult<()> {
        let token = &auth_service::domain::password_reset::PasswordResetToken::from(token.clone());

        let result: anyhow::Result<_> = async {
            self.entries
                .lock()
                .await
                .insert(token.token_hash.clone(), token.clone());
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn exists(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> auth_service::application::errors::AuthResult<bool> {
        let result: anyhow::Result<_> = async {
            Ok(self
                .entries
                .lock()
                .await
                .get(token_hash)
                .is_some_and(|token| token.expires_at > now))
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }

    async fn consume(
        &self,
        token_hash: &str,
        now: DateTime<Utc>,
    ) -> auth_service::application::errors::AuthResult<
        Option<auth_service::application::dto::repository_dto::AuthPasswordResetRecordDto>,
    > {
        let result = async {
            let result: anyhow::Result<_> = async {
                let mut entries = self.entries.lock().await;
                if entries
                    .get(token_hash)
                    .is_some_and(|token| token.expires_at > now)
                {
                    Ok(entries.remove(token_hash))
                } else {
                    Ok(None)
                }
            }
            .await;
            result.map_err(auth_service::adapters::out::errors::boundary_error)
        }
        .await;
        result.map(|value| value.map(Into::into))
    }
}

#[derive(Clone, Default)]
pub struct RecordingResetMailer {
    pub sent: Arc<Mutex<Vec<(String, String)>>>,
}

#[async_trait]
impl ResetMailerPort for RecordingResetMailer {
    async fn send_reset_link(
        &self,
        email: &str,
        link: &str,
    ) -> auth_service::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
            self.sent
                .lock()
                .await
                .push((email.to_owned(), link.to_owned()));
            Ok(())
        }
        .await;
        result.map_err(auth_service::adapters::out::errors::boundary_error)
    }
}

#[derive(Clone)]
pub struct TestTokenService {
    jwt: JwtTokenService,
}

impl TestTokenService {
    pub fn new() -> Self {
        Self {
            jwt: test_jwt_service(),
        }
    }
}

impl Default for TestTokenService {
    fn default() -> Self {
        Self::new()
    }
}

impl TokenServicePort for TestTokenService {
    fn issue_token_pair(
        &self,
        subject: Uuid,
        roles: Vec<String>,
    ) -> auth_service::application::errors::AuthResult<
        auth_service::application::dto::token_dto::AuthTokenPairDto,
    > {
        self.jwt.issue_token_pair(subject, roles)
    }

    fn continue_session(
        &self,
        subject: Uuid,
        roles: Vec<String>,
        session_id: Uuid,
    ) -> auth_service::application::errors::AuthResult<
        auth_service::application::dto::token_dto::AuthTokenPairDto,
    > {
        self.jwt.continue_session(subject, roles, session_id)
    }

    fn decode_access_token(
        &self,
        token: &str,
    ) -> auth_service::application::errors::AuthResult<
        auth_service::application::dto::token_dto::AuthDecodedAccessTokenDto,
    > {
        self.jwt.decode_access_token(token)
    }

    fn decode_refresh_token(
        &self,
        token: &str,
    ) -> auth_service::application::errors::AuthResult<
        auth_service::application::dto::token_dto::AuthDecodedRefreshTokenDto,
    > {
        self.jwt.decode_refresh_token(token)
    }

    fn jwk_set(&self) -> auth_service::application::dto::token_dto::AuthJwkSetDto {
        self.jwt.jwk_set()
    }
}

pub async fn seed_active_user(repo: &InMemoryUserRepository, email: &str) -> Result<Uuid> {
    let id = Uuid::new_v4();
    repo.save(
        &(&User {
            id,
            primary_email: email.into(),
            password_hash: hash_password("Passw0rd!")?,
            status: UserStatus::Active,
            last_login_at: Some(Utc::now()),
        })
            .into(),
    )
    .await?;
    Ok(id)
}

pub fn grpc_server(state: Arc<AppState>) -> AuthGrpcServer {
    AuthGrpcServer::new(state)
}

#[cfg(test)]
mod rsa_key_tests {
    use super::*;

    #[test]
    fn shared_test_rsa_key_material_is_reused() {
        assert!(std::ptr::eq(
            shared_test_rsa_private_key(),
            shared_test_rsa_private_key(),
        ));
        assert_eq!(
            shared_test_rsa_private_key_pem(),
            shared_test_rsa_private_key_pem(),
        );
    }
}
