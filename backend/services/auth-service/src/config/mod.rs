use crate::adapters::out::cache::session_store::PgRefreshTokenStore;
use crate::adapters::out::grpc::user_deletion_dispatcher::GrpcUserDeletionDispatcher;
use crate::adapters::out::mail::reset_mailer::{
    ConfiguredResetMailer, HttpResetMailer, LoggingResetMailer,
};
use crate::adapters::out::oauth::provider_client::{
    HttpOAuthProviderClient, OAuthHttpProviderSettings, OAuthTokenAuthMethod, OAuthUserInfoFormat,
    ParityOAuthProviderClient,
};
use crate::adapters::out::persistence::auth_lifecycle_cleanup_repository::PgAuthLifecycleCleanupRepository;
use crate::adapters::out::persistence::oauth_authorization_store::PgOAuthAuthorizationStore;
use crate::adapters::out::persistence::user_deletion_saga_repository::PgUserDeletionSagaRepository;
use crate::adapters::out::persistence::user_repository::{
    PgAuthAuditRepository, PgLoginAuditRepository, PgUserRepository,
};
use crate::adapters::out::runtime::SystemAuthRuntime;
use crate::adapters::out::security::admitted_argon2::{hash_password, AdmittedArgon2PasswordWork};
use crate::adapters::out::security::apple_client_secret::AppleClientSecretGenerator;
use crate::adapters::out::security::oauth_authorization::SecureOAuthAuthorization;
use crate::adapters::r#in::messaging::lifecycle_cleanup_worker::AuthLifecycleCleanupWorker;
use crate::adapters::r#in::messaging::user_deletion_worker::{
    UserDeletionWorker, UserDeletionWorkerPolicy,
};
use crate::application::auth_runtime::AuthRuntimePort;
use crate::application::auth_transactions::AuthTransactionsPort;
use crate::application::password_work::{PasswordWork, PasswordWorkKind, PasswordWorkLimits};
use crate::application::ports::{OAuthProviderMetricsPort, OAuthProviderPort, TokenServicePort};
use crate::application::use_cases::delete_user::DeleteUserUseCase;
use crate::application::use_cases::get_user_deletion::GetUserDeletionUseCase;
use crate::application::use_cases::lifecycle_cleanup::AuthLifecycleCleanupUseCase;
use crate::application::use_cases::list_dead_lettered_user_deletions::ListDeadLetteredUserDeletionsUseCase;
use crate::application::use_cases::login::{LoginLockoutPolicy, LoginUseCase, LoginUseCaseDeps};
use crate::application::use_cases::oauth_link::OAuthLinkUseCase;
use crate::application::use_cases::oauth_login::OAuthLoginUseCase;
use crate::application::use_cases::password_reset::{
    PasswordResetLinkBaseUrl, PasswordResetUseCase,
};
use crate::application::use_cases::retry_user_deletion::RetryUserDeletionUseCase;
use crate::application::use_cases::signup::SignupUseCase;
use crate::domain::session::{AuthTokenPair, DecodedAccessToken, DecodedRefreshToken};
use crate::domain::user::IdentityProvider;
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, DecodingKey, EncodingKey, Header, Validation};
use platform::config::{
    env_or, parse_bool_setting, parse_i32_setting, parse_i64_setting, parse_u64_setting,
    require_non_empty_setting, require_production_disabled, require_production_https_url,
    require_production_postgres_url, require_production_safe_socket_addr,
    require_production_safe_url,
};
use platform::edge_metrics::EdgeDeadlineMetrics;
use rsa::pkcs8::{DecodePrivateKey, EncodePrivateKey, EncodePublicKey};
use rsa::{RsaPrivateKey, RsaPublicKey};
use serde::{Deserialize, Serialize};
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

mod signing_keys;

pub use signing_keys::{MAX_ROTATION_KEYS, MIN_RSA_MODULUS_BITS};

pub const DEFAULT_METRICS_ADDR: &str = "127.0.0.1:9081";

#[derive(Clone)]
pub struct AppConfig {
    pub http_addr: SocketAddr,
    pub metrics_addr: SocketAddr,
    pub grpc_addr: SocketAddr,
    pub database_url: String,
    pub database_migration_mode: migration_runtime::MigrationMode,
    /// Largest number of pooled database connections this service opens.
    ///
    /// A single hardcoded value forced every service to share one concurrency
    /// ceiling. That is wrong in both directions: a read-heavy service that
    /// fans out is throttled to the same limit as an edge that holds a
    /// connection per in-flight request, and raising it everywhere to suit one
    /// service multiplies the load on PostgreSQL instead. The bound is
    /// configuration so a deployment can size each service to its own work.
    pub database_pool_max_connections: u32,
    pub seed_enabled: bool,
    /// Whether to serve the duplicate public auth application routes.
    ///
    /// The Gateway is the public entry point and applies rate limiting, client
    /// identity normalization, a durable request journal, and OAuth state-cookie
    /// binding. Serving login/signup/reset/OAuth directly from this service
    /// bypasses all of it, so the surface is opt-in for local development and
    /// disabled in production.
    pub public_application_routes_enabled: bool,
    pub runtime_mode: RuntimeMode,
    pub access_ttl_seconds: i64,
    pub refresh_ttl_seconds: i64,
    pub issuer: String,
    pub access_audience: String,
    pub google_redirect_uri: String,
    pub apple_redirect_uri: String,
    pub x_redirect_uri: String,
    pub google_authorize_base_uri: String,
    pub apple_authorize_base_uri: String,
    pub x_authorize_base_uri: String,
    pub google_client_id: Option<String>,
    pub google_client_secret: Option<String>,
    pub google_token_uri: Option<String>,
    pub google_userinfo_uri: Option<String>,
    pub google_issuer: String,
    pub google_jwks_json: Option<String>,
    pub google_jwks_uri: Option<String>,
    pub apple_client_id: Option<String>,
    /// A precomputed client secret, used only when generation is not configured.
    pub apple_client_secret: Option<String>,
    /// Apple team id, the issuer of the generated client secret.
    pub apple_team_id: Option<String>,
    /// Key id naming the Apple private key that signs the client secret.
    pub apple_key_id: Option<String>,
    /// ES256 private key used to sign the client secret.
    pub apple_private_key_pem: Option<String>,
    /// Lifetime of a generated client secret, in seconds.
    pub apple_client_secret_ttl_seconds: i64,
    pub apple_token_uri: Option<String>,
    pub apple_userinfo_uri: Option<String>,
    pub apple_issuer: String,
    pub apple_jwks_json: Option<String>,
    pub apple_jwks_uri: Option<String>,
    pub x_client_id: Option<String>,
    pub x_client_secret: Option<String>,
    pub x_token_uri: Option<String>,
    pub x_userinfo_uri: Option<String>,
    pub jwt_private_key_pem: Option<String>,
    pub jwt_key_id: Option<String>,
    /// Public keys published and accepted beside the signing key during a
    /// rotation (`AUTH_JWT_ROTATION_JWKS_JSON`).
    ///
    /// A JWK Set that holds the next key before the switch and the previous key
    /// until the tokens it signed have expired. It never holds a private key.
    /// See `docs/operations/signing-key-rotation.md`.
    pub jwt_rotation_jwks_json: Option<String>,
    /// Callers this service accepts, each with its own credential.
    pub caller_registry: platform::grpc_auth::CallerRegistry,
    /// Verifies Gateway actor delegations on actor-bound methods.
    pub actor_delegation: platform::actor_delegation::ActorDelegationVerifier,
    /// Credential this service presents when dispatching deletion work.
    pub outbound_identity: String,
    /// Mutual TLS material for internal gRPC, resolved at load time.
    ///
    /// `None` means the documented plaintext development path. Production cannot
    /// reach a running service with `None` because `load` fails first.
    pub tls_material: Option<platform::grpc_tls::TlsMaterial>,
    pub login_lockout_max_failures: u32,
    pub login_lockout_window_seconds: i64,
    /// Worker and queue bounds for password hashing; see `password_work`.
    pub password_work: PasswordWorkLimits,
    pub password_reset_base_url: String,
    pub password_reset_mailer_mode: PasswordResetMailerMode,
    pub password_reset_mailer_endpoint: Option<String>,
    pub password_reset_mailer_bearer_token: Option<String>,
    pub lifecycle_cleanup_worker_enabled: bool,
    pub lifecycle_cleanup_worker_interval_ms: u64,
    pub profile_grpc_endpoint: String,
    pub friend_grpc_endpoint: String,
    pub post_grpc_endpoint: String,
    pub notification_grpc_endpoint: String,
    pub user_deletion_worker_enabled: bool,
    pub user_deletion_worker_interval_ms: u64,
    pub user_deletion_worker_batch_size: i32,
    pub user_deletion_worker_max_attempts: i32,
    pub user_deletion_worker_lease_timeout_ms: u64,
    pub user_deletion_worker_base_retry_ms: u64,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let runtime_mode = RuntimeMode::parse(&env_or("AUTH_RUNTIME_MODE", "production"))?;
        let is_production = matches!(runtime_mode, RuntimeMode::Production);
        // Only the edge may drive user-facing auth RPCs. No internal service
        // calls auth except the edge, so the accepted set stays minimal.
        let caller_registry = platform::config::build_caller_registry(
            is_production,
            &[platform::grpc_auth::Workload::Gateway],
            &[platform::grpc_auth::Workload::Gateway],
        )?;
        let outbound_identity = platform::config::resolve_outbound_identity(
            is_production,
            platform::grpc_auth::Workload::Auth,
        )?;
        let tls_material = platform::config::resolve_tls_material(is_production, "auth")?;

        let actor_delegation = platform::actor_delegation::ActorDelegationVerifier::from_env(
            is_production,
            platform::grpc_auth::Workload::Auth,
        )?;
        let config = Self {
            database_migration_mode: migration_runtime::MigrationMode::from_env_value(
                std::env::var("AUTH_DATABASE_MIGRATION_MODE"),
            )?,
            http_addr: env_or("AUTH_HTTP_ADDR", "127.0.0.1:8081").parse()?,
            metrics_addr: env_or("AUTH_METRICS_ADDR", DEFAULT_METRICS_ADDR).parse()?,
            grpc_addr: env_or("AUTH_GRPC_ADDR", "127.0.0.1:50051").parse()?,
            database_url: env_or(
                "AUTH_DATABASE_URL",
                "postgres://postgres:postgres@localhost:5432/auth_db",
            ),
            database_pool_max_connections: platform::config::parse_u32_setting(
                "AUTH_DB_POOL_MAX_CONNECTIONS",
                &env_or("AUTH_DB_POOL_MAX_CONNECTIONS", "5"),
                1,
                512,
            )?,
            seed_enabled: parse_bool_setting(
                "AUTH_SEED_ENABLED",
                &env_or("AUTH_SEED_ENABLED", "false"),
            )?,
            public_application_routes_enabled: parse_bool_setting(
                "AUTH_PUBLIC_APPLICATION_ROUTES_ENABLED",
                &env_or("AUTH_PUBLIC_APPLICATION_ROUTES_ENABLED", "false"),
            )?,
            runtime_mode,
            access_ttl_seconds: parse_i64_setting(
                "AUTH_ACCESS_TTL_SECONDS",
                &env_or("AUTH_ACCESS_TTL_SECONDS", "900"),
                60,
                86_400,
            )?,
            refresh_ttl_seconds: parse_i64_setting(
                "AUTH_REFRESH_TTL_SECONDS",
                &env_or("AUTH_REFRESH_TTL_SECONDS", "1209600"),
                300,
                31_536_000,
            )?,
            issuer: env_or("AUTH_ISSUER", "auth-service"),
            access_audience: require_non_empty_setting(
                "AUTH_ACCESS_AUDIENCE",
                env_or("AUTH_ACCESS_AUDIENCE", "port-rust-api"),
            )?,
            google_redirect_uri: env_or(
                "AUTH_GOOGLE_REDIRECT_URI",
                "http://127.0.0.1:8080/auth/oauth/google/callback",
            ),
            apple_redirect_uri: env_or(
                "AUTH_APPLE_REDIRECT_URI",
                "http://127.0.0.1:8080/auth/oauth/apple/callback",
            ),
            x_redirect_uri: env_or(
                "AUTH_X_REDIRECT_URI",
                "http://127.0.0.1:8080/auth/oauth/x/callback",
            ),
            google_authorize_base_uri: env_or(
                "AUTH_GOOGLE_AUTHORIZE_BASE_URI",
                "https://accounts.google.test/o/oauth2/v2/auth",
            ),
            apple_authorize_base_uri: env_or(
                "AUTH_APPLE_AUTHORIZE_BASE_URI",
                "https://appleid.apple.test/auth/authorize",
            ),
            x_authorize_base_uri: env_or(
                "AUTH_X_AUTHORIZE_BASE_URI",
                "https://x.test/i/oauth2/authorize",
            ),
            google_client_id: optional_env("AUTH_GOOGLE_CLIENT_ID"),
            google_client_secret: optional_env("AUTH_GOOGLE_CLIENT_SECRET"),
            google_token_uri: optional_env("AUTH_GOOGLE_TOKEN_URI"),
            google_userinfo_uri: optional_env("AUTH_GOOGLE_USERINFO_URI"),
            google_issuer: env_or("AUTH_GOOGLE_ISSUER", "https://accounts.google.com"),
            google_jwks_json: optional_env("AUTH_GOOGLE_JWKS_JSON"),
            google_jwks_uri: optional_env("AUTH_GOOGLE_JWKS_URI"),
            apple_client_id: optional_env("AUTH_APPLE_CLIENT_ID"),
            apple_client_secret: optional_env("AUTH_APPLE_CLIENT_SECRET"),
            apple_team_id: optional_env("AUTH_APPLE_TEAM_ID"),
            apple_key_id: optional_env("AUTH_APPLE_KEY_ID"),
            apple_private_key_pem: optional_env("AUTH_APPLE_PRIVATE_KEY_PEM"),
            apple_client_secret_ttl_seconds: parse_i64_setting(
                "AUTH_APPLE_CLIENT_SECRET_TTL_SECONDS",
                // Six months is Apple's stated maximum. A shorter default keeps
                // rotation frequent without a deployment having to decide.
                &env_or("AUTH_APPLE_CLIENT_SECRET_TTL_SECONDS", "86400"),
                1,
                15_777_000,
            )?,
            apple_token_uri: optional_env("AUTH_APPLE_TOKEN_URI"),
            apple_userinfo_uri: optional_env("AUTH_APPLE_USERINFO_URI"),
            apple_issuer: env_or("AUTH_APPLE_ISSUER", "https://appleid.apple.com"),
            apple_jwks_json: optional_env("AUTH_APPLE_JWKS_JSON"),
            apple_jwks_uri: optional_env("AUTH_APPLE_JWKS_URI"),
            x_client_id: optional_env("AUTH_X_CLIENT_ID"),
            x_client_secret: optional_env("AUTH_X_CLIENT_SECRET"),
            x_token_uri: optional_env("AUTH_X_TOKEN_URI"),
            x_userinfo_uri: optional_env("AUTH_X_USERINFO_URI"),
            jwt_private_key_pem: std::env::var("AUTH_JWT_PRIVATE_KEY_PEM")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            jwt_key_id: std::env::var("AUTH_JWT_KEY_ID")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            jwt_rotation_jwks_json: optional_env("AUTH_JWT_ROTATION_JWKS_JSON"),
            caller_registry,
            actor_delegation,
            outbound_identity,
            tls_material,
            login_lockout_max_failures: platform::config::parse_u32_setting(
                "AUTH_LOGIN_LOCKOUT_MAX_FAILURES",
                &env_or("AUTH_LOGIN_LOCKOUT_MAX_FAILURES", "5"),
                0,
                100,
            )?,
            login_lockout_window_seconds: parse_i64_setting(
                "AUTH_LOGIN_LOCKOUT_WINDOW_SECONDS",
                &env_or("AUTH_LOGIN_LOCKOUT_WINDOW_SECONDS", "900"),
                60,
                86_400,
            )?,
            password_work: password_work_limits_from_env()?,
            password_reset_base_url: env_or(
                "AUTH_PASSWORD_RESET_BASE_URL",
                "http://localhost:5174/auth/reset",
            ),
            password_reset_mailer_mode: PasswordResetMailerMode::parse(&env_or(
                "AUTH_PASSWORD_RESET_MAILER_MODE",
                "logging",
            ))?,
            password_reset_mailer_endpoint: optional_env("AUTH_PASSWORD_RESET_MAILER_ENDPOINT"),
            password_reset_mailer_bearer_token: optional_env(
                "AUTH_PASSWORD_RESET_MAILER_BEARER_TOKEN",
            ),
            lifecycle_cleanup_worker_enabled: parse_bool_setting(
                "AUTH_LIFECYCLE_CLEANUP_WORKER_ENABLED",
                &env_or("AUTH_LIFECYCLE_CLEANUP_WORKER_ENABLED", "true"),
            )?,
            lifecycle_cleanup_worker_interval_ms: parse_u64_setting(
                "AUTH_LIFECYCLE_CLEANUP_WORKER_INTERVAL_MS",
                &env_or("AUTH_LIFECYCLE_CLEANUP_WORKER_INTERVAL_MS", "3600000"),
                1_000,
                86_400_000,
            )?,
            profile_grpc_endpoint: env_or("AUTH_PROFILE_GRPC_ENDPOINT", "http://127.0.0.1:50052"),
            friend_grpc_endpoint: env_or("AUTH_FRIEND_GRPC_ENDPOINT", "http://127.0.0.1:50053"),
            notification_grpc_endpoint: env_or(
                "AUTH_NOTIFICATION_GRPC_ENDPOINT",
                "http://127.0.0.1:50054",
            ),
            post_grpc_endpoint: env_or("AUTH_POST_GRPC_ENDPOINT", "http://127.0.0.1:50055"),
            user_deletion_worker_enabled: parse_bool_setting(
                "AUTH_USER_DELETION_WORKER_ENABLED",
                &env_or("AUTH_USER_DELETION_WORKER_ENABLED", "true"),
            )?,
            user_deletion_worker_interval_ms: parse_u64_setting(
                "AUTH_USER_DELETION_WORKER_INTERVAL_MS",
                &env_or("AUTH_USER_DELETION_WORKER_INTERVAL_MS", "1000"),
                100,
                86_400_000,
            )?,
            user_deletion_worker_batch_size: parse_i32_setting(
                "AUTH_USER_DELETION_WORKER_BATCH_SIZE",
                &env_or("AUTH_USER_DELETION_WORKER_BATCH_SIZE", "50"),
                1,
                100,
            )?,
            user_deletion_worker_max_attempts: parse_i32_setting(
                "AUTH_USER_DELETION_WORKER_MAX_ATTEMPTS",
                &env_or("AUTH_USER_DELETION_WORKER_MAX_ATTEMPTS", "8"),
                1,
                100,
            )?,
            user_deletion_worker_lease_timeout_ms: parse_u64_setting(
                "AUTH_USER_DELETION_WORKER_LEASE_TIMEOUT_MS",
                &env_or("AUTH_USER_DELETION_WORKER_LEASE_TIMEOUT_MS", "300000"),
                1_000,
                86_400_000,
            )?,
            user_deletion_worker_base_retry_ms: parse_u64_setting(
                "AUTH_USER_DELETION_WORKER_BASE_RETRY_MS",
                &env_or("AUTH_USER_DELETION_WORKER_BASE_RETRY_MS", "1000"),
                100,
                86_400_000,
            )?,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let production = matches!(self.runtime_mode, RuntimeMode::Production);
        require_production_safe_socket_addr(production, "AUTH_HTTP_ADDR", self.http_addr)?;
        require_production_safe_socket_addr(production, "AUTH_GRPC_ADDR", self.grpc_addr)?;
        require_production_postgres_url(production, "AUTH_DATABASE_URL", &self.database_url)?;
        require_production_disabled(
            production,
            "AUTH_SEED_ENABLED",
            self.seed_enabled,
            "demo seed data must not run in production",
        )?;
        // The Gateway owns the public auth surface. Serving the duplicate
        // application routes here would skip Gateway rate limiting, client
        // identity normalization, the request journal, and OAuth state-cookie
        // binding, so production must never enable it.
        require_production_disabled(
            production,
            "AUTH_PUBLIC_APPLICATION_ROUTES_ENABLED",
            self.public_application_routes_enabled,
            "the duplicate public auth application routes bypass Gateway controls",
        )?;

        require_non_empty_setting("AUTH_ISSUER", self.issuer.clone())?;
        require_non_empty_setting("AUTH_ACCESS_AUDIENCE", self.access_audience.clone())?;
        validate_password_reset_base_url(production, &self.password_reset_base_url)?;
        validate_password_reset_mailer(production, self)?;

        if production {
            require_non_empty_setting(
                "AUTH_JWT_PRIVATE_KEY_PEM",
                self.jwt_private_key_pem.clone().unwrap_or_default(),
            )?;
            require_non_empty_setting(
                "AUTH_JWT_KEY_ID",
                self.jwt_key_id.clone().unwrap_or_default(),
            )?;
        }

        for (name, uri) in [
            ("AUTH_GOOGLE_REDIRECT_URI", &self.google_redirect_uri),
            ("AUTH_APPLE_REDIRECT_URI", &self.apple_redirect_uri),
            ("AUTH_X_REDIRECT_URI", &self.x_redirect_uri),
            (
                "AUTH_GOOGLE_AUTHORIZE_BASE_URI",
                &self.google_authorize_base_uri,
            ),
            (
                "AUTH_APPLE_AUTHORIZE_BASE_URI",
                &self.apple_authorize_base_uri,
            ),
            ("AUTH_X_AUTHORIZE_BASE_URI", &self.x_authorize_base_uri),
        ] {
            require_production_https_url(production, name, uri)?;
        }

        for (name, uri) in [
            ("AUTH_GOOGLE_TOKEN_URI", &self.google_token_uri),
            ("AUTH_GOOGLE_USERINFO_URI", &self.google_userinfo_uri),
            ("AUTH_GOOGLE_JWKS_URI", &self.google_jwks_uri),
            ("AUTH_APPLE_TOKEN_URI", &self.apple_token_uri),
            ("AUTH_APPLE_USERINFO_URI", &self.apple_userinfo_uri),
            ("AUTH_APPLE_JWKS_URI", &self.apple_jwks_uri),
            ("AUTH_X_TOKEN_URI", &self.x_token_uri),
            ("AUTH_X_USERINFO_URI", &self.x_userinfo_uri),
        ] {
            require_production_https_url(production, name, uri.as_deref().unwrap_or_default())?;
        }
        for (name, endpoint) in [
            ("AUTH_PROFILE_GRPC_ENDPOINT", &self.profile_grpc_endpoint),
            ("AUTH_FRIEND_GRPC_ENDPOINT", &self.friend_grpc_endpoint),
            ("AUTH_POST_GRPC_ENDPOINT", &self.post_grpc_endpoint),
            (
                "AUTH_NOTIFICATION_GRPC_ENDPOINT",
                &self.notification_grpc_endpoint,
            ),
        ] {
            require_production_safe_url(production, name, endpoint)?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RuntimeMode {
    Production,
    Parity,
}

impl RuntimeMode {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "production" | "prod" => Ok(Self::Production),
            "parity" | "demo" | "development" | "dev" => Ok(Self::Parity),
            _ => anyhow::bail!("AUTH_RUNTIME_MODE must be production or parity"),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PasswordResetMailerMode {
    Logging,
    Http,
}

impl PasswordResetMailerMode {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "logging" | "log" => Ok(Self::Logging),
            "http" | "webhook" => Ok(Self::Http),
            _ => anyhow::bail!("AUTH_PASSWORD_RESET_MAILER_MODE must be logging or http"),
        }
    }
}

#[derive(Clone)]
pub struct AppState {
    pub signup: SignupUseCase<Arc<PgUserRepository>>,
    pub login: LoginUseCase<
        Arc<PgUserRepository>,
        Arc<PgLoginAuditRepository>,
        Arc<PgRefreshTokenStore>,
        Arc<JwtTokenService>,
    >,
    pub oauth_login: OAuthLoginUseCase,
    pub oauth_link: OAuthLinkUseCase,
    pub password_reset: PasswordResetUseCase<Arc<PgUserRepository>>,
    pub delete_user: DeleteUserUseCase<
        Arc<PgUserRepository>,
        Arc<PgAuthAuditRepository>,
        Arc<PgUserDeletionSagaRepository>,
    >,
    pub get_user_deletion: GetUserDeletionUseCase<Arc<PgUserDeletionSagaRepository>>,
    pub list_dead_lettered_user_deletions:
        ListDeadLetteredUserDeletionsUseCase<Arc<PgUserDeletionSagaRepository>>,
    pub retry_user_deletion: RetryUserDeletionUseCase<Arc<PgUserDeletionSagaRepository>>,
    pub user_deletion_worker:
        UserDeletionWorker<Arc<PgUserDeletionSagaRepository>, GrpcUserDeletionDispatcher>,
    pub lifecycle_cleanup_worker: AuthLifecycleCleanupWorker<Arc<PgAuthLifecycleCleanupRepository>>,
    pub metrics: Arc<AuthMetrics>,
    /// Whether the duplicate public application routes are served. False in
    /// production, where the Gateway owns that surface.
    public_application_routes_enabled: bool,
}

impl AppState {
    pub fn public_application_routes_enabled(&self) -> bool {
        self.public_application_routes_enabled
    }
}

#[derive(Clone, Default)]
pub struct AuthMetrics {
    counters: Arc<Mutex<BTreeMap<String, u64>>>,
    password_work: Option<PasswordWork>,
    edge_deadlines: EdgeDeadlineMetrics,
}

impl AuthMetrics {
    pub fn edge_deadline_metrics(&self) -> EdgeDeadlineMetrics {
        self.edge_deadlines.clone()
    }

    /// Also export admission counts for this password work.
    #[must_use]
    pub fn with_password_work(mut self, password_work: PasswordWork) -> Self {
        self.password_work = Some(password_work);
        self
    }

    pub fn render_prometheus(&self) -> String {
        let counters = self.counters.lock().expect("auth metrics");
        let mut output = String::new();
        for (sample, value) in counters.iter() {
            output.push_str(sample);
            output.push(' ');
            output.push_str(&value.to_string());
            output.push('\n');
        }
        if let Some(password_work) = &self.password_work {
            // Rejections are the saturation signal: a request refused as busy
            // instead of being queued without bound.
            for kind in PasswordWorkKind::ALL {
                let stats = password_work.stats(kind);
                let labels = label_set(&[("class", kind.label())]);
                output.push_str(&format!(
                    "auth_password_work_admitted_total{{{labels}}} {}\n",
                    stats.admitted
                ));
                output.push_str(&format!(
                    "auth_password_work_rejected_total{{{labels}}} {}\n",
                    stats.rejected
                ));
            }
        }
        output.push_str(&self.edge_deadlines.render_prometheus());
        output
    }

    fn inc(&self, name: &str, labels: &[(&str, &str)]) {
        self.add(name, labels, 1);
    }

    fn add(&self, name: &str, labels: &[(&str, &str)], amount: u64) {
        let sample = format!("{name}{{{}}}", label_set(labels));
        let mut counters = self.counters.lock().expect("auth metrics");
        *counters.entry(sample).or_insert(0) += amount;
    }

    pub fn record_lifecycle_cleanup_run(&self, outcome: &str) {
        self.inc("auth_lifecycle_cleanup_runs_total", &[("outcome", outcome)]);
    }

    pub fn record_lifecycle_cleanup_deleted_records(
        &self,
        record_type: &str,
        outcome: &str,
        count: i64,
    ) {
        self.add(
            "auth_lifecycle_cleanup_deleted_records_total",
            &[("record_type", record_type), ("outcome", outcome)],
            count.max(0) as u64,
        );
    }
}

impl OAuthProviderMetricsPort for AuthMetrics {
    fn record_provider_exchange(
        &self,
        provider: crate::application::dto::repository_dto::AuthIdentityProviderDto,
        outcome: &str,
    ) {
        let provider = crate::domain::user::IdentityProvider::from(provider);

        self.inc(
            "auth_oauth_provider_exchanges_total",
            &[("provider", provider_name(provider)), ("outcome", outcome)],
        );
    }
}

fn provider_name(provider: IdentityProvider) -> &'static str {
    match provider {
        IdentityProvider::Email => "email",
        IdentityProvider::Google => "google",
        IdentityProvider::Apple => "apple",
        IdentityProvider::X => "x",
    }
}

fn label_set(labels: &[(&str, &str)]) -> String {
    labels
        .iter()
        .map(|(name, value)| format!("{name}=\"{}\"", escape_label_value(value)))
        .collect::<Vec<_>>()
        .join(",")
}

fn escape_label_value(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
}

pub async fn connect_pool(config: &AppConfig) -> anyhow::Result<PgPool> {
    let options = migration_runtime::serving_options(
        config.database_url.parse()?,
        config.database_migration_mode,
    );
    let pool = PgPoolOptions::new()
        .max_connections(config.database_pool_max_connections)
        .connect_with(options)
        .await?;

    let migrations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    migration_runtime::prepare_database(
        &pool,
        migrations.as_path(),
        config.database_migration_mode,
        &[migration_runtime::SchemaProbe {
            name: "AUTH required schema objects",
            sql: include_str!("schema-probe.sql"),
        }],
    )
    .await?;
    Ok(pool)
}

pub async fn build_state(config: &AppConfig, pool: PgPool) -> anyhow::Result<AppState> {
    let password_work: PasswordWork =
        Arc::new(AdmittedArgon2PasswordWork::new(config.password_work)?);
    build_state_with_password_work(config, pool, password_work).await
}

/// Build the service with an explicit password-work instance.
///
/// Every hashing use case shares this one instance, so its classes and their
/// reserved login capacity cover the whole process.
pub async fn build_state_with_password_work(
    config: &AppConfig,
    pool: PgPool,
    password_work: PasswordWork,
) -> anyhow::Result<AppState> {
    build_state_with_auth_capabilities(config, pool, password_work, Arc::new(SystemAuthRuntime))
        .await
}

/// Compose Auth with mandatory shared runtime and admitted password capabilities.
/// Production selects SystemAuthRuntime; controlled acceptance tests select a clock.
pub async fn build_state_with_auth_capabilities(
    config: &AppConfig,
    pool: PgPool,
    password_work: PasswordWork,
    auth_runtime: Arc<dyn AuthRuntimePort>,
) -> anyhow::Result<AppState> {
    if config.seed_enabled {
        seed_users(&pool).await?;
    }
    let user_repository = Arc::new(PgUserRepository::new(pool.clone(), auth_runtime.clone()));
    let auth_transactions: Arc<dyn AuthTransactionsPort> = user_repository.clone();
    let login_audit_repository = Arc::new(PgLoginAuditRepository::new(pool.clone()));
    let auth_audit_repository = Arc::new(PgAuthAuditRepository::new(pool.clone()));
    let oauth_authorization_store = Arc::new(PgOAuthAuthorizationStore::new(pool.clone()));
    let refresh_token_store =
        Arc::new(PgRefreshTokenStore::new(pool.clone(), auth_runtime.clone()));
    let lifecycle_cleanup_repository =
        Arc::new(PgAuthLifecycleCleanupRepository::new(pool.clone()));
    let user_deletion_repository = Arc::new(PgUserDeletionSagaRepository::new(pool.clone()));
    let metrics = Arc::new(AuthMetrics::default().with_password_work(password_work.clone()));
    let token_service = match config.jwt_private_key_pem.as_deref() {
        Some(private_key_pem) => JwtTokenService::from_private_key_pem(
            &config.issuer,
            &config.access_audience,
            config.access_ttl_seconds,
            config.refresh_ttl_seconds,
            private_key_pem,
            config
                .jwt_key_id
                .as_deref()
                .unwrap_or("auth-service-current"),
        )?,
        None => JwtTokenService::new(
            &config.issuer,
            &config.access_audience,
            config.access_ttl_seconds,
            config.refresh_ttl_seconds,
        )?,
    };
    let token_service = Arc::new(match config.jwt_rotation_jwks_json.as_deref() {
        Some(rotation_jwks) => token_service.with_rotation_keys(rotation_jwks)?,
        None => token_service,
    });
    let oauth_provider = build_oauth_provider(config)?;
    // Retain startup validation/construction of configured delivery even
    // though the use case now reaches it only through the atomic outbox.
    let _configured_reset_mailer = build_reset_mailer(config)?;
    let reset_link_base_url =
        PasswordResetLinkBaseUrl::new(config.password_reset_base_url.clone())?;
    let lifecycle_cleanup_worker = AuthLifecycleCleanupWorker::new(
        AuthLifecycleCleanupUseCase::new(lifecycle_cleanup_repository, auth_runtime.clone()),
        metrics.clone(),
    );
    let user_deletion_dispatcher = GrpcUserDeletionDispatcher::connect_with_metrics(
        config.profile_grpc_endpoint.clone(),
        config.friend_grpc_endpoint.clone(),
        config.post_grpc_endpoint.clone(),
        config.notification_grpc_endpoint.clone(),
        &config.outbound_identity,
        config.tls_material.as_ref(),
        metrics.edge_deadline_metrics(),
    )?;
    let user_deletion_worker = UserDeletionWorker::new(
        user_deletion_repository.clone(),
        user_deletion_dispatcher,
        UserDeletionWorkerPolicy {
            batch_size: config.user_deletion_worker_batch_size,
            max_attempts: config.user_deletion_worker_max_attempts,
            lease_timeout: Duration::milliseconds(
                i64::try_from(config.user_deletion_worker_lease_timeout_ms)
                    .map_err(|_| anyhow::anyhow!("user deletion lease timeout is too large"))?,
            ),
            base_retry: Duration::milliseconds(
                i64::try_from(config.user_deletion_worker_base_retry_ms)
                    .map_err(|_| anyhow::anyhow!("user deletion retry interval is too large"))?,
            ),
        },
    );

    Ok(AppState {
        signup: SignupUseCase::new(
            user_repository.clone(),
            auth_transactions.clone(),
            password_work.clone(),
            auth_runtime.clone(),
            Arc::new(crate::adapters::out::security::email_verification::SecureEmailVerification),
        ),
        login: LoginUseCase::new_with_lockout_policy(
            LoginUseCaseDeps {
                user_repository: user_repository.clone(),
                login_audit_repository: login_audit_repository.clone(),
                refresh_token_store: refresh_token_store.clone(),
                token_service: token_service.clone(),
                transactions: auth_transactions.clone(),
                password_work: password_work.clone(),
                runtime: auth_runtime.clone(),
            },
            LoginLockoutPolicy {
                max_failures: config.login_lockout_max_failures,
                window: Duration::seconds(config.login_lockout_window_seconds),
            },
        ),
        oauth_login: OAuthLoginUseCase::new(
            user_repository.clone(),
            auth_transactions.clone(),
            auth_audit_repository.clone(),
            token_service.clone(),
            oauth_provider.clone(),
            oauth_authorization_store.clone(),
            metrics.clone(),
            auth_runtime.clone(),
            Arc::new(SecureOAuthAuthorization),
            config.google_redirect_uri.clone(),
            config.apple_redirect_uri.clone(),
            config.x_redirect_uri.clone(),
        ),
        oauth_link: OAuthLinkUseCase::new(
            user_repository.clone(),
            user_repository.clone(),
            auth_audit_repository.clone(),
            oauth_provider,
            oauth_authorization_store,
            metrics.clone(),
            token_service,
            auth_runtime.clone(),
        ),
        password_reset: PasswordResetUseCase::new(
            user_repository.clone(),
            auth_transactions,
            password_work,
            reset_link_base_url,
            auth_runtime.clone(),
            Arc::new(crate::adapters::out::security::password_reset::SecurePasswordReset),
        ),
        delete_user: DeleteUserUseCase::new(
            user_repository.clone(),
            auth_audit_repository,
            user_deletion_repository.clone(),
            auth_runtime.clone(),
        ),
        get_user_deletion: GetUserDeletionUseCase::new(user_deletion_repository.clone()),
        list_dead_lettered_user_deletions: ListDeadLetteredUserDeletionsUseCase::new(
            user_deletion_repository.clone(),
        ),
        retry_user_deletion: RetryUserDeletionUseCase::new(
            user_deletion_repository,
            auth_runtime.clone(),
        ),
        user_deletion_worker,
        lifecycle_cleanup_worker,
        metrics,
        public_application_routes_enabled: config.public_application_routes_enabled,
    })
}

fn validate_password_reset_base_url(production: bool, value: &str) -> anyhow::Result<()> {
    require_non_empty_setting("AUTH_PASSWORD_RESET_BASE_URL", value.to_owned())?;
    PasswordResetLinkBaseUrl::new(value.to_owned())?;
    let uri: http::Uri = value.parse().map_err(|_| {
        anyhow::anyhow!("AUTH_PASSWORD_RESET_BASE_URL must be an absolute HTTP(S) URL")
    })?;
    anyhow::ensure!(
        matches!(uri.scheme_str(), Some("http" | "https"))
            && uri.authority().is_some_and(|authority| !authority.as_str().contains('@'))
            && uri.query().is_none()
            && !value.contains('#')
            && !value.contains('\\'),
        "AUTH_PASSWORD_RESET_BASE_URL must be an absolute HTTP(S) URL without credentials, query or fragment"
    );
    require_production_https_url(production, "AUTH_PASSWORD_RESET_BASE_URL", value)?;
    Ok(())
}

fn validate_password_reset_mailer(production: bool, config: &AppConfig) -> anyhow::Result<()> {
    match config.password_reset_mailer_mode {
        PasswordResetMailerMode::Logging => {
            require_production_disabled(
                production,
                "AUTH_PASSWORD_RESET_MAILER_MODE",
                true,
                "logging reset links is parity-only; configure http mail delivery",
            )?;
        }
        PasswordResetMailerMode::Http => {
            require_non_empty_setting(
                "AUTH_PASSWORD_RESET_MAILER_ENDPOINT",
                config
                    .password_reset_mailer_endpoint
                    .clone()
                    .unwrap_or_default(),
            )?;
            require_non_empty_setting(
                "AUTH_PASSWORD_RESET_MAILER_BEARER_TOKEN",
                config
                    .password_reset_mailer_bearer_token
                    .clone()
                    .unwrap_or_default(),
            )?;
            require_production_https_url(
                production,
                "AUTH_PASSWORD_RESET_MAILER_ENDPOINT",
                config
                    .password_reset_mailer_endpoint
                    .as_deref()
                    .unwrap_or_default(),
            )?;
        }
    }
    Ok(())
}

fn build_reset_mailer(config: &AppConfig) -> anyhow::Result<ConfiguredResetMailer> {
    match config.password_reset_mailer_mode {
        PasswordResetMailerMode::Logging => Ok(ConfiguredResetMailer::Logging(LoggingResetMailer)),
        PasswordResetMailerMode::Http => Ok(ConfiguredResetMailer::Http(HttpResetMailer::new(
            required_reset_mailer(
                &config.password_reset_mailer_endpoint,
                "AUTH_PASSWORD_RESET_MAILER_ENDPOINT",
            )?,
            required_reset_mailer(
                &config.password_reset_mailer_bearer_token,
                "AUTH_PASSWORD_RESET_MAILER_BEARER_TOKEN",
            )?,
        )?)),
    }
}

fn required_reset_mailer(value: &Option<String>, name: &str) -> anyhow::Result<String> {
    value
        .as_ref()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{name} is required for HTTP password reset mailer"))
}

pub fn build_oauth_provider(config: &AppConfig) -> anyhow::Result<Arc<dyn OAuthProviderPort>> {
    match config.runtime_mode {
        RuntimeMode::Parity => Ok(Arc::new(ParityOAuthProviderClient::new(
            config.google_authorize_base_uri.clone(),
            config.apple_authorize_base_uri.clone(),
            config.x_authorize_base_uri.clone(),
        ))),
        RuntimeMode::Production => Ok(Arc::new(HttpOAuthProviderClient::new(
            OAuthHttpProviderSettings {
                authorize_base_uri: config.google_authorize_base_uri.clone(),
                client_id: required_oauth(&config.google_client_id, "AUTH_GOOGLE_CLIENT_ID")?,
                client_secret: required_oauth(
                    &config.google_client_secret,
                    "AUTH_GOOGLE_CLIENT_SECRET",
                )?,
                client_secret_generator: None,
                token_uri: required_oauth(&config.google_token_uri, "AUTH_GOOGLE_TOKEN_URI")?,
                userinfo_uri: required_oauth(
                    &config.google_userinfo_uri,
                    "AUTH_GOOGLE_USERINFO_URI",
                )?,
                issuer: config.google_issuer.clone(),
                jwks_json: required_oauth(&config.google_jwks_json, "AUTH_GOOGLE_JWKS_JSON")?,
                jwks_uri: Some(required_oauth(
                    &config.google_jwks_uri,
                    "AUTH_GOOGLE_JWKS_URI",
                )?),
                scopes: vec!["openid".into(), "email".into(), "profile".into()],
                token_auth_method: OAuthTokenAuthMethod::ClientSecretPost,
                userinfo_format: OAuthUserInfoFormat::Oidc,
            },
            OAuthHttpProviderSettings {
                authorize_base_uri: config.apple_authorize_base_uri.clone(),
                client_id: required_oauth(&config.apple_client_id, "AUTH_APPLE_CLIENT_ID")?,
                // The secret is minted per token request when a key is
                // configured, because a generated secret is an ES256 JWT that
                // expires and a process outliving that lifetime would present a
                // stale one. Otherwise the static value is used as-is.
                client_secret: config.apple_client_secret.clone().unwrap_or_default(),
                client_secret_generator: resolve_apple_client_secret(config)?.map(Arc::new),
                token_uri: required_oauth(&config.apple_token_uri, "AUTH_APPLE_TOKEN_URI")?,
                userinfo_uri: required_oauth(
                    &config.apple_userinfo_uri,
                    "AUTH_APPLE_USERINFO_URI",
                )?,
                issuer: config.apple_issuer.clone(),
                jwks_json: required_oauth(&config.apple_jwks_json, "AUTH_APPLE_JWKS_JSON")?,
                jwks_uri: Some(required_oauth(
                    &config.apple_jwks_uri,
                    "AUTH_APPLE_JWKS_URI",
                )?),
                scopes: vec!["name".into(), "email".into()],
                token_auth_method: OAuthTokenAuthMethod::ClientSecretPost,
                userinfo_format: OAuthUserInfoFormat::Oidc,
            },
            OAuthHttpProviderSettings {
                authorize_base_uri: config.x_authorize_base_uri.clone(),
                client_id: required_oauth(&config.x_client_id, "AUTH_X_CLIENT_ID")?,
                client_secret: required_oauth(&config.x_client_secret, "AUTH_X_CLIENT_SECRET")?,
                client_secret_generator: None,
                token_uri: required_oauth(&config.x_token_uri, "AUTH_X_TOKEN_URI")?,
                userinfo_uri: required_oauth(&config.x_userinfo_uri, "AUTH_X_USERINFO_URI")?,
                issuer: String::new(),
                jwks_json: String::new(),
                jwks_uri: None,
                scopes: vec!["users.read".into(), "users.email".into()],
                token_auth_method: OAuthTokenAuthMethod::BasicAuth,
                userinfo_format: OAuthUserInfoFormat::XApiV2,
            },
        )?)),
    }
}

fn optional_env(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

/// Read password hashing bounds, failing at startup on an invalid split.
fn password_work_limits_from_env() -> anyhow::Result<PasswordWorkLimits> {
    let defaults = PasswordWorkLimits::default();
    let count = |name: &str, default: usize, min: u32, max: u32| -> anyhow::Result<usize> {
        let value = platform::config::parse_u32_setting(
            name,
            &env_or(name, &default.to_string()),
            min,
            max,
        )?;
        Ok(usize::try_from(value)?)
    };
    let limits = PasswordWorkLimits {
        workers: count("AUTH_PASSWORD_WORKERS", defaults.workers, 1, 64)?,
        login_queue: count("AUTH_PASSWORD_LOGIN_QUEUE", defaults.login_queue, 0, 1024)?,
        signup_workers: count(
            "AUTH_PASSWORD_SIGNUP_WORKERS",
            defaults.signup_workers,
            1,
            63,
        )?,
        signup_queue: count("AUTH_PASSWORD_SIGNUP_QUEUE", defaults.signup_queue, 0, 1024)?,
        reset_workers: count("AUTH_PASSWORD_RESET_WORKERS", defaults.reset_workers, 1, 63)?,
        reset_queue: count("AUTH_PASSWORD_RESET_QUEUE", defaults.reset_queue, 0, 1024)?,
    };
    limits.validate()?;
    Ok(limits)
}

/// Resolve how the Apple client secret is produced for this deployment.
///
/// Apple does not issue a static client secret. It requires an ES256 JWT signed
/// with a key from the developer account, so the supported path is to build a
/// generator from the key pair and mint the secret per token request.
///
/// A configured static secret is still accepted, so a deployment that has one is
/// not broken by this change. It is warned about rather than silently preferred,
/// because a static secret cannot be rotated by restarting and cannot be minted
/// again once it expires.
fn resolve_apple_client_secret(
    config: &AppConfig,
) -> anyhow::Result<Option<AppleClientSecretGenerator>> {
    let has_key_material = config
        .apple_private_key_pem
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty());
    if has_key_material {
        let team_id = required_oauth_setting(&config.apple_team_id, "AUTH_APPLE_TEAM_ID")?;
        let key_id = required_oauth_setting(&config.apple_key_id, "AUTH_APPLE_KEY_ID")?;
        let client_id = required_oauth_setting(&config.apple_client_id, "AUTH_APPLE_CLIENT_ID")?;
        let private_key = config.apple_private_key_pem.clone().unwrap_or_default();
        let generator = AppleClientSecretGenerator::new(
            &team_id,
            &key_id,
            &client_id,
            &private_key,
            config.apple_client_secret_ttl_seconds,
        )?;
        tracing::info!(
            ttl_seconds = generator.ttl_seconds(),
            "the Apple client secret is generated as an ES256 JWT per token request"
        );
        return Ok(Some(generator));
    }

    if config
        .apple_client_secret
        .as_deref()
        .map(str::trim)
        .is_some_and(|value| !value.is_empty())
    {
        tracing::warn!(
            "using the static Apple client secret; configure AUTH_APPLE_PRIVATE_KEY_PEM with AUTH_APPLE_TEAM_ID and AUTH_APPLE_KEY_ID so the secret is generated and rotated"
        );
        return Ok(None);
    }

    // Neither path configured. The token endpoint would reject the request, and
    // `validate_settings` reports the missing credential at startup, so this is
    // deliberately not an error here as well.
    Ok(None)
}

fn required_oauth_setting(value: &Option<String>, name: &str) -> anyhow::Result<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .ok_or_else(|| anyhow::anyhow!("{name} is required to generate the Apple client secret"))
}
fn required_oauth(value: &Option<String>, name: &str) -> anyhow::Result<String> {
    value
        .as_ref()
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{name} is required in production OAuth mode"))
}

#[derive(Clone)]
pub struct JwtTokenService {
    issuer: String,
    access_audience: String,
    access_ttl: Duration,
    refresh_ttl: Duration,
    encoding: EncodingKey,
    decoding: DecodingKey,
    public_key: RsaPublicKey,
    kid: String,
    /// Keys published and accepted beside the signing key during a rotation.
    /// None of them signs.
    rotation_keys: Vec<signing_keys::RotationKey>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Claims {
    sub: String,
    iss: String,
    iat: i64,
    exp: i64,
    jti: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    aud: Option<String>,
    token_type: String,
    roles: Vec<String>,
    /// Session family of an access token. High-risk actions ask the issuer
    /// whether this family is still live instead of trusting the token alone.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    sid: Option<String>,
}

impl JwtTokenService {
    pub fn new(
        issuer: &str,
        access_audience: &str,
        access_ttl_seconds: i64,
        refresh_ttl_seconds: i64,
    ) -> anyhow::Result<Self> {
        validate_jwt_identity_settings(issuer, access_audience)?;
        let private_key = RsaPrivateKey::new(&mut rand::thread_rng(), 2048)?;
        let public_key = RsaPublicKey::from(&private_key);
        let private_pem = private_key.to_pkcs8_pem(Default::default())?;
        let public_pem = public_key.to_public_key_pem(Default::default())?;

        Ok(Self {
            issuer: issuer.to_owned(),
            access_audience: access_audience.to_owned(),
            access_ttl: Duration::seconds(access_ttl_seconds),
            refresh_ttl: Duration::seconds(refresh_ttl_seconds),
            encoding: EncodingKey::from_rsa_pem(private_pem.as_bytes())?,
            decoding: DecodingKey::from_rsa_pem(public_pem.as_bytes())?,
            public_key,
            kid: Uuid::new_v4().to_string(),
            rotation_keys: Vec::new(),
        })
    }

    pub fn from_private_key_pem(
        issuer: &str,
        access_audience: &str,
        access_ttl_seconds: i64,
        refresh_ttl_seconds: i64,
        private_key_pem: &str,
        kid: &str,
    ) -> anyhow::Result<Self> {
        validate_jwt_identity_settings(issuer, access_audience)?;
        if kid.trim().is_empty() {
            anyhow::bail!("JWT key id is required");
        }
        let private_key = RsaPrivateKey::from_pkcs8_pem(private_key_pem)?;
        let public_key = RsaPublicKey::from(&private_key);
        let public_pem = public_key.to_public_key_pem(Default::default())?;

        Ok(Self {
            issuer: issuer.to_owned(),
            access_audience: access_audience.to_owned(),
            access_ttl: Duration::seconds(access_ttl_seconds),
            refresh_ttl: Duration::seconds(refresh_ttl_seconds),
            encoding: EncodingKey::from_rsa_pem(private_key_pem.as_bytes())?,
            decoding: DecodingKey::from_rsa_pem(public_pem.as_bytes())?,
            public_key,
            kid: kid.to_owned(),
            rotation_keys: Vec::new(),
        })
    }

    /// Publish and accept the keys of a rotation beside the signing key.
    ///
    /// `jwks_json` is a public JWK Set holding the next key before a switch, or
    /// the previous key until the tokens it signed have expired. Tokens are
    /// still signed only with the configured private key.
    pub fn with_rotation_keys(mut self, jwks_json: &str) -> anyhow::Result<Self> {
        self.rotation_keys = signing_keys::parse_rotation_keys(jwks_json, &self.kid)?;
        Ok(self)
    }

    /// The key that verifies `token`, chosen from the keys this issuer accepts.
    ///
    /// The token's `kid` only selects among configured keys; nothing in a token
    /// can introduce a key. A token that names no key is verified with the
    /// signing key, which is how every token was verified before rotation keys
    /// existed; it cannot select any other key.
    fn decoding_key_for(&self, token: &str) -> anyhow::Result<&DecodingKey> {
        let header = jsonwebtoken::decode_header(token)?;
        match header.kid.as_deref() {
            None => Ok(&self.decoding),
            Some(kid) if kid == self.kid => Ok(&self.decoding),
            Some(kid) => self
                .rotation_keys
                .iter()
                .find(|key| key.kid == kid)
                .map(|key| &key.decoding)
                .ok_or_else(|| anyhow::anyhow!("Token signing key is not accepted")),
        }
    }
}

fn validate_jwt_identity_settings(issuer: &str, access_audience: &str) -> anyhow::Result<()> {
    if issuer.trim().is_empty() {
        anyhow::bail!("JWT issuer is required");
    }
    if access_audience.trim().is_empty() {
        anyhow::bail!("JWT access audience is required");
    }
    Ok(())
}

impl JwtTokenService {
    /// Issue an access/refresh pair in `session_id`, or in a new family whose id
    /// is the refresh token's jti, matching `RefreshTokenRecord::new_family`.
    fn issue_in_session(
        &self,
        subject: Uuid,
        roles: Vec<String>,
        session_id: Option<Uuid>,
    ) -> anyhow::Result<AuthTokenPair> {
        let now = Utc::now();
        let refresh_jti = Uuid::new_v4();
        let session_id = session_id.unwrap_or(refresh_jti);
        let access_claims = Claims {
            sub: subject.to_string(),
            iss: self.issuer.clone(),
            iat: now.timestamp(),
            exp: (now + self.access_ttl).timestamp(),
            jti: Uuid::new_v4().to_string(),
            aud: Some(self.access_audience.clone()),
            token_type: "access".into(),
            roles: roles.clone(),
            sid: Some(session_id.to_string()),
        };
        let refresh_claims = Claims {
            sub: subject.to_string(),
            iss: self.issuer.clone(),
            iat: now.timestamp(),
            exp: (now + self.refresh_ttl).timestamp(),
            jti: refresh_jti.to_string(),
            aud: None,
            token_type: "refresh".into(),
            roles,
            sid: None,
        };
        let header = Header {
            alg: Algorithm::RS256,
            kid: Some(self.kid.clone()),
            ..Header::default()
        };

        Ok(AuthTokenPair {
            access_token: jsonwebtoken::encode(&header, &access_claims, &self.encoding)?,
            refresh_token: jsonwebtoken::encode(&header, &refresh_claims, &self.encoding)?,
            token_type: "Bearer".into(),
            expires_in_seconds: self.access_ttl.num_seconds(),
            refresh_jti: refresh_claims.jti.clone(),
            refresh_expires_at: now + self.refresh_ttl,
            session_id,
        })
    }
}

impl TokenServicePort for JwtTokenService {
    fn issue_token_pair(
        &self,
        subject: Uuid,
        roles: Vec<String>,
    ) -> crate::application::errors::AuthResult<crate::application::dto::token_dto::AuthTokenPairDto>
    {
        let result: anyhow::Result<_> = self.issue_in_session(subject, roles, None);
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(Into::into)
    }

    fn continue_session(
        &self,
        subject: Uuid,
        roles: Vec<String>,
        session_id: Uuid,
    ) -> crate::application::errors::AuthResult<crate::application::dto::token_dto::AuthTokenPairDto>
    {
        let result: anyhow::Result<_> = self.issue_in_session(subject, roles, Some(session_id));
        result
            .map_err(crate::adapters::out::errors::boundary_error)
            .map(Into::into)
    }

    fn decode_access_token(
        &self,
        token: &str,
    ) -> crate::application::errors::AuthResult<
        crate::application::dto::token_dto::AuthDecodedAccessTokenDto,
    > {
        // Keys/configuration are qualified at construction. Decode failures are
        // untrusted credential failures, not infrastructure or signing defects.
        let result: anyhow::Result<_> = (|| {
            let mut validation = Validation::new(Algorithm::RS256);
            validation.validate_exp = true;
            validation.set_issuer(&[self.issuer.as_str()]);
            validation.set_audience(&[self.access_audience.as_str()]);
            validation.set_required_spec_claims(&["aud", "exp", "iss", "sub"]);
            let data =
                jsonwebtoken::decode::<Claims>(token, self.decoding_key_for(token)?, &validation)?;
            if data.claims.aud.as_deref() != Some(self.access_audience.as_str()) {
                return Err(crate::application::errors::AuthError::Invariant(
                    "Invalid token audience",
                )
                .into());
            }
            if data.claims.token_type != "access" {
                return Err(
                    crate::application::errors::AuthError::Invariant("Invalid token type").into(),
                );
            }
            Ok(DecodedAccessToken {
                subject: Uuid::parse_str(&data.claims.sub)?,
                roles: data.claims.roles,
                session_id: data
                    .claims
                    .sid
                    .as_deref()
                    .map(Uuid::parse_str)
                    .transpose()?,
            })
        })();
        result
            .map_err(|_| crate::application::errors::AuthError::CredentialRejected)
            .map(Into::into)
    }

    fn decode_refresh_token(
        &self,
        token: &str,
    ) -> crate::application::errors::AuthResult<
        crate::application::dto::token_dto::AuthDecodedRefreshTokenDto,
    > {
        let result: anyhow::Result<_> = (|| {
            let mut validation = Validation::new(Algorithm::RS256);
            validation.validate_exp = true;
            validation.set_issuer(&[self.issuer.as_str()]);
            validation.validate_aud = false;
            validation.set_required_spec_claims(&["exp", "iss", "sub"]);
            let data =
                jsonwebtoken::decode::<Claims>(token, self.decoding_key_for(token)?, &validation)?;
            if data.claims.token_type != "refresh" {
                return Err(
                    crate::application::errors::AuthError::Invariant("Invalid token type").into(),
                );
            }
            Ok(DecodedRefreshToken {
                subject: Uuid::parse_str(&data.claims.sub)?,
                jti: data.claims.jti,
            })
        })();
        result
            .map_err(|_| crate::application::errors::AuthError::CredentialRejected)
            .map(Into::into)
    }

    fn jwk_set(&self) -> crate::application::dto::token_dto::AuthJwkSetDto {
        // The signing key comes first, so a reader of the first entry still finds
        // the key that signs new tokens.
        let mut keys = vec![signing_keys::public_jwk(&self.kid, &self.public_key)];
        keys.extend(
            self.rotation_keys
                .iter()
                .map(|key| signing_keys::public_jwk(&key.kid, &key.public_key)),
        );
        crate::application::dto::token_dto::AuthJwkSetDto { keys }
    }
}

async fn seed_users(pool: &PgPool) -> anyhow::Result<()> {
    let users = [
        ("11111111-1111-1111-1111-111111111111", "hana@example.com"),
        ("22222222-2222-2222-2222-222222222222", "jin@example.com"),
        ("33333333-3333-3333-3333-333333333333", "soyeon@example.com"),
        ("44444444-4444-4444-4444-444444444444", "mira@example.com"),
        ("55555555-5555-5555-5555-555555555555", "tae@example.com"),
        ("66666666-6666-6666-6666-666666666666", "minji@example.com"),
        ("77777777-7777-7777-7777-777777777777", "ryu@example.com"),
        ("88888888-8888-8888-8888-888888888888", "eun@example.com"),
        ("99999999-9999-9999-9999-999999999999", "lexi@example.com"),
        ("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", "noa@example.com"),
    ];

    let password_hash = hash_password("Passw0rd!")?;

    for (user_id, email) in users {
        sqlx::query(
            r#"
            insert into users (id, primary_email, password_hash, status, last_login_at)
            values ($1, $2, $3, 'ACTIVE', now())
            on conflict (id) do update
            set primary_email = excluded.primary_email,
                password_hash = excluded.password_hash,
                status = excluded.status,
                last_login_at = excluded.last_login_at
            "#,
        )
        .bind(Uuid::parse_str(user_id)?)
        .bind(email)
        .bind(&password_hash)
        .execute(pool)
        .await?;

        sqlx::query(
            r#"
            insert into user_identities (id, user_id, provider_type, email, email_verified, linked_at)
            values ($1, $2, 'EMAIL', $3, true, now())
            on conflict (user_id, provider_type, email) do update set email_verified = true
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(Uuid::parse_str(user_id)?)
        .bind(email)
        .execute(pool)
        .await?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn invalid_expired_wrong_type_and_unknown_key_credentials_are_rejected_safely() {
        use crate::application::errors::AuthError;
        let service = JwtTokenService::new("mcr-test", "mcr-browser", 3600, 86400).unwrap();
        let pair = service
            .issue_token_pair(Uuid::new_v4(), vec!["ROLE_USER".into()])
            .unwrap();
        for token in ["malformed", "a.b.c", "", &pair.access_token] {
            assert_eq!(
                service.decode_refresh_token(token).unwrap_err(),
                AuthError::CredentialRejected
            );
        }
        assert_eq!(
            service
                .decode_access_token(&pair.refresh_token)
                .unwrap_err(),
            AuthError::CredentialRejected
        );
        let expired = JwtTokenService::new("mcr-test", "mcr-browser", -3600, -3600).unwrap();
        let pair = expired.issue_token_pair(Uuid::new_v4(), vec![]).unwrap();
        assert_eq!(
            expired.decode_access_token(&pair.access_token).unwrap_err(),
            AuthError::CredentialRejected
        );
        assert_eq!(
            expired
                .decode_refresh_token(&pair.refresh_token)
                .unwrap_err(),
            AuthError::CredentialRejected
        );
        let mut claims = Claims {
            sub: Uuid::new_v4().to_string(),
            iss: "mcr-test".into(),
            iat: Utc::now().timestamp(),
            exp: Utc::now().timestamp() + 3600,
            jti: Uuid::new_v4().to_string(),
            aud: Some("mcr-browser".into()),
            token_type: "access".into(),
            roles: vec![],
            sid: None,
        };
        let mut header = Header {
            alg: Algorithm::RS256,
            kid: Some("unknown".into()),
            ..Header::default()
        };
        let unknown = jsonwebtoken::encode(&header, &claims, &service.encoding).unwrap();
        assert_eq!(
            service.decode_access_token(&unknown).unwrap_err(),
            AuthError::CredentialRejected
        );
        // Retain Auth's inherited no-kid decoding compatibility. The Gateway's
        // account-security test separately requires a canonical session ID.
        header.kid = None;
        let legacy = jsonwebtoken::encode(&header, &claims, &service.encoding).unwrap();
        assert!(service
            .decode_access_token(&legacy)
            .unwrap()
            .session_id
            .is_none());
        claims.sub = "not-a-uuid".into();
        let invalid_subject = jsonwebtoken::encode(&header, &claims, &service.encoding).unwrap();
        assert_eq!(
            service.decode_access_token(&invalid_subject).unwrap_err(),
            AuthError::CredentialRejected
        );
    }

    #[test]
    fn reset_targets_are_explicit_http_urls_without_redirect_credentials_or_token_parameters() {
        for value in [
            "javascript:alert(1)",
            "//hostile.test/reset",
            "https://user@hostile.test/reset",
            "https://app.example/reset?next=https://hostile.test",
            "https://app.example/reset#token=x",
            "https://app.example\\@hostile.test/reset",
        ] {
            assert!(
                validate_password_reset_base_url(false, value).is_err(),
                "{value}"
            );
        }
        assert!(
            validate_password_reset_base_url(false, "http://127.0.0.1:43210/reset-password")
                .is_ok()
        );
        assert!(
            validate_password_reset_base_url(true, "https://app.company.com/reset-password")
                .is_ok()
        );
        assert!(
            validate_password_reset_base_url(true, "http://app.company.com/reset-password")
                .is_err()
        );
    }

    #[test]
    fn production_config_rejects_unsafe_auth_defaults() {
        let mut config = production_config();
        config.http_addr = "127.0.0.1:8081".parse().expect("addr");
        assert!(config
            .validate()
            .expect_err("loopback bind must fail")
            .to_string()
            .contains("AUTH_HTTP_ADDR"));

        let mut config = production_config();
        config.database_url = "postgres://postgres:postgres@localhost:5432/auth_db".into();
        assert!(config
            .validate()
            .expect_err("local database must fail")
            .to_string()
            .contains("AUTH_DATABASE_URL"));

        let mut config = production_config();
        config.seed_enabled = true;
        assert!(config
            .validate()
            .expect_err("seed must fail")
            .to_string()
            .contains("AUTH_SEED_ENABLED"));

        let mut config = production_config();
        config.jwt_private_key_pem = None;
        assert!(config
            .validate()
            .expect_err("production key must fail")
            .to_string()
            .contains("AUTH_JWT_PRIVATE_KEY_PEM"));

        let mut config = production_config();
        config.google_authorize_base_uri = "https://accounts.google.test/o/oauth2/v2/auth".into();
        assert!(config
            .validate()
            .expect_err("placeholder OAuth host must fail")
            .to_string()
            .contains("AUTH_GOOGLE_AUTHORIZE_BASE_URI"));

        let mut config = production_config();
        config.x_authorize_base_uri = "https://x.test/i/oauth2/authorize".into();
        assert!(config
            .validate()
            .expect_err("placeholder X OAuth host must fail")
            .to_string()
            .contains("AUTH_X_AUTHORIZE_BASE_URI"));

        let mut config = production_config();
        config.password_reset_base_url = "http://localhost:5174/auth/reset".into();
        assert!(config
            .validate()
            .expect_err("local reset URL must fail")
            .to_string()
            .contains("AUTH_PASSWORD_RESET_BASE_URL"));

        let mut config = production_config();
        config.password_reset_mailer_mode = PasswordResetMailerMode::Logging;
        assert!(config
            .validate()
            .expect_err("logging reset mailer must fail")
            .to_string()
            .contains("AUTH_PASSWORD_RESET_MAILER_MODE"));

        let mut config = production_config();
        config.password_reset_mailer_endpoint = None;
        assert!(config
            .validate()
            .expect_err("HTTP reset mailer endpoint must fail")
            .to_string()
            .contains("AUTH_PASSWORD_RESET_MAILER_ENDPOINT"));
    }

    #[test]
    fn production_config_accepts_explicit_remote_auth_settings() {
        production_config().validate().expect("production config");
    }

    #[test]
    fn parity_config_allows_local_auth_defaults_and_ephemeral_key() {
        let mut config = production_config();
        config.runtime_mode = RuntimeMode::Parity;
        config.http_addr = "127.0.0.1:8081".parse().expect("addr");
        config.grpc_addr = "127.0.0.1:50051".parse().expect("addr");
        config.database_url = "postgres://postgres:postgres@localhost:5432/auth_db".into();
        config.seed_enabled = true;
        config.jwt_private_key_pem = None;
        config.jwt_key_id = None;
        config.google_redirect_uri = "http://127.0.0.1:8080/auth/oauth/google/callback".into();
        config.apple_redirect_uri = "http://127.0.0.1:8080/auth/oauth/apple/callback".into();
        config.x_redirect_uri = "http://127.0.0.1:8080/auth/oauth/x/callback".into();
        config.google_authorize_base_uri = "https://accounts.google.test/o/oauth2/v2/auth".into();
        config.apple_authorize_base_uri = "https://appleid.apple.test/auth/authorize".into();
        config.x_authorize_base_uri = "https://x.test/i/oauth2/authorize".into();
        config.google_token_uri = None;
        config.google_userinfo_uri = None;
        config.google_jwks_uri = None;
        config.apple_token_uri = None;
        config.apple_userinfo_uri = None;
        config.apple_jwks_uri = None;
        config.x_token_uri = None;
        config.x_userinfo_uri = None;
        config.password_reset_base_url = "http://localhost:5174/auth/reset".into();
        config.password_reset_mailer_mode = PasswordResetMailerMode::Logging;
        config.password_reset_mailer_endpoint = None;
        config.password_reset_mailer_bearer_token = None;
        config.validate().expect("parity config");
    }

    fn production_config() -> AppConfig {
        AppConfig {
            database_migration_mode: migration_runtime::MigrationMode::Startup,
            http_addr: "0.0.0.0:8081".parse().expect("addr"),
            metrics_addr: "127.0.0.1:9081".parse().expect("metrics addr"),
            grpc_addr: "0.0.0.0:50051".parse().expect("addr"),
            database_pool_max_connections: 5,
            database_url: "postgres://auth:secret@postgres.internal:5432/auth_db".into(),
            seed_enabled: false,
            public_application_routes_enabled: false,
            runtime_mode: RuntimeMode::Production,
            access_ttl_seconds: 900,
            refresh_ttl_seconds: 1_209_600,
            issuer: "auth-service".into(),
            access_audience: "port-rust-api".into(),
            google_redirect_uri: "https://app.company.com/auth/oauth/google/callback".into(),
            apple_redirect_uri: "https://app.company.com/auth/oauth/apple/callback".into(),
            x_redirect_uri: "https://app.company.com/auth/oauth/x/callback".into(),
            google_authorize_base_uri: "https://accounts.google.com/o/oauth2/v2/auth".into(),
            apple_authorize_base_uri: "https://appleid.apple.com/auth/authorize".into(),
            x_authorize_base_uri: "https://x.com/i/oauth2/authorize".into(),
            google_client_id: Some("google-client".into()),
            google_client_secret: Some("google-secret".into()),
            google_token_uri: Some("https://oauth2.googleapis.com/token".into()),
            google_userinfo_uri: Some("https://openidconnect.googleapis.com/v1/userinfo".into()),
            google_issuer: "https://accounts.google.com".into(),
            google_jwks_json: Some("{\"keys\":[]}".into()),
            google_jwks_uri: Some("https://www.googleapis.com/oauth2/v3/certs".into()),
            apple_client_id: Some("apple-client".into()),
            apple_client_secret: Some("apple-secret".into()),
            apple_team_id: None,
            apple_key_id: None,
            apple_private_key_pem: None,
            apple_client_secret_ttl_seconds: 86_400,
            apple_token_uri: Some("https://appleid.apple.com/auth/token".into()),
            apple_userinfo_uri: Some("https://appleid.apple.com/auth/userinfo".into()),
            apple_issuer: "https://appleid.apple.com".into(),
            apple_jwks_json: Some("{\"keys\":[]}".into()),
            apple_jwks_uri: Some("https://appleid.apple.com/auth/keys".into()),
            x_client_id: Some("x-client".into()),
            x_client_secret: Some("x-secret".into()),
            x_token_uri: Some("https://api.x.com/2/oauth2/token".into()),
            x_userinfo_uri: Some("https://api.x.com/2/users/me?user.fields=confirmed_email".into()),
            jwt_private_key_pem: Some("private-key-pem".into()),
            jwt_key_id: Some("auth-service-current".into()),
            jwt_rotation_jwks_json: None,
            actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
                platform::grpc_auth::Workload::Auth,
            ),
            caller_registry: platform::grpc_auth::CallerRegistry::new()
                .register(
                    platform::grpc_auth::Workload::Gateway,
                    "production-service-token",
                )
                .expect("production caller registry"),
            outbound_identity: "production-service-token".into(),
            tls_material: None,
            login_lockout_max_failures: 5,
            login_lockout_window_seconds: 900,
            password_work: PasswordWorkLimits::default(),
            password_reset_base_url: "https://app.company.com/auth/reset".into(),
            password_reset_mailer_mode: PasswordResetMailerMode::Http,
            password_reset_mailer_endpoint: Some(
                "https://mailer.company.com/password-reset".into(),
            ),
            password_reset_mailer_bearer_token: Some("mailer-token".into()),
            lifecycle_cleanup_worker_enabled: true,
            lifecycle_cleanup_worker_interval_ms: 3_600_000,
            profile_grpc_endpoint: "http://profile.internal:50052".into(),
            friend_grpc_endpoint: "http://friend.internal:50053".into(),
            notification_grpc_endpoint: "http://notification.internal:50054".into(),
            post_grpc_endpoint: "http://post.internal:50055".into(),
            user_deletion_worker_enabled: true,
            user_deletion_worker_interval_ms: 1_000,
            user_deletion_worker_batch_size: 50,
            user_deletion_worker_max_attempts: 8,
            user_deletion_worker_lease_timeout_ms: 300_000,
            user_deletion_worker_base_retry_ms: 1_000,
        }
    }

    mod external_pool_acceptance {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../crates/migration-runtime/tests/support/service_pool.rs"
        ));
    }

    #[tokio::test]
    async fn external_migration_mode_rejects_schema_damage_without_owner_privileges(
    ) -> anyhow::Result<()> {
        external_pool_acceptance::verify(
            |url, mode| async move {
                let mut config = production_config();
                config.database_url = url;
                config.database_migration_mode = mode;
                connect_pool(&config).await
            },
            "users",
            "id",
            "auth_retire_user_id",
        )
        .await
    }
}
