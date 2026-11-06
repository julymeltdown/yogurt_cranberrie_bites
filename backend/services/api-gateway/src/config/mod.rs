use anyhow::Context;
mod rate_limit;

pub use rate_limit::{InMemoryRateLimiter, STORE_BUCKET_CAPACITY};

use crate::adapters::out::auth_client::{verification::GrpcVerificationClient, GrpcAuthClient};
use crate::adapters::out::cache::response_cache::InMemoryResponseCache;
use crate::adapters::out::friend_client::GrpcFriendClient;
use crate::adapters::out::jwt::JwtVerifier;
use crate::adapters::out::moderation_runtime::SystemModerationRuntime;
use crate::adapters::out::notification_client::GrpcNotificationClient;
use crate::adapters::out::post_client::GrpcPostClient;
use crate::adapters::out::profile_client::GrpcProfileClient;
use crate::adapters::out::repository::aggregation_recipe_repository::PgAggregationRecipeRepository;
use crate::adapters::out::repository::async_operation_queue::PayloadCipher;
use crate::adapters::out::repository::async_operation_queue::PgAsyncOperationQueueRepository;
use crate::adapters::out::repository::client_profile_repository::PgClientProfileRepository;
use crate::adapters::out::repository::journal_maintenance::PgJournalMaintenanceRepository;
use crate::adapters::out::repository::journal_write_repository::{
    PgJournalDispatchRepository, PgJournalRetentionRepository,
};
use crate::adapters::out::repository::legacy_audit_repository::PgLegacyAuditRepository;
use crate::adapters::out::repository::moderation_repository::PgModerationRepository;
use crate::adapters::out::repository::operation_journal_repository::PgOperationJournalRepository;
use crate::adapters::out::repository::owner_receipt_journal_repository::PgOwnerReceiptJournalRepository;
use crate::adapters::out::repository::policy_repository::{
    PgPolicyRepository, PgRolloutRepository,
};
use crate::adapters::out::repository::route_contract_repository::PgRouteContractRepository;
use crate::adapters::out::repository::telemetry_repository::PgTelemetryRepository;
use crate::adapters::out::runtime::SystemGatewayRuntime;
use crate::adapters::out::signing_keys::{RefreshPolicy, SigningKeyRefresher};
use crate::adapters::out::telemetry_queue::QueuedTelemetry;
use crate::application::ports::{
    AuthClientPort, FriendClientPort, ModerationRepositoryPort, NotificationClientPort,
    PostClientPort, ProfileClientPort, SigningKeySourcePort,
};
use crate::application::search_cursor::SearchCursorCodec;
use crate::application::service::{
    AggregationService, AuthGatewayService, ClientProfileService, FriendGatewayService,
    HighRiskSessionGuard, ModerationService, NotificationGatewayService, OperationJournalService,
    PolicyService, PostGatewayService, ProfileGatewayService, RouteContractService,
};
use crate::domain::admission::{AdmissionMode, AdmissionPolicies, AdmissionScope, RouteClass};
use axum::http::header::{ACCEPT, AUTHORIZATION, CONTENT_TYPE};
use axum::http::{HeaderName, HeaderValue, Method};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use platform::config::{
    env_or, parse_bool_setting, parse_i64_setting, parse_u32_setting, parse_u64_setting,
    require_non_empty_setting, require_production_disabled, require_production_postgres_url,
    require_production_safe_socket_addr, require_production_safe_url, require_production_safe_urls,
};
use platform::edge_metrics::EdgeDeadlineMetrics;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::ops::{Deref, DerefMut};
use std::path::PathBuf;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower_http::cors::CorsLayer;

pub const DEFAULT_METRICS_ADDR: &str = "127.0.0.1:9080";
/// Loopback default for the internal exchange listener outside production.
pub const DEFAULT_GRPC_ADDR: &str = "127.0.0.1:50050";

/// Build the child-authority issuer from the Gateway's own keys.
///
/// The parent verifier always trusts the current signing key, so a Gateway
/// with no rotation list can verify the parents it signed. When
/// ACTOR_DELEGATION_VERIFYING_KEYS is set it is used instead, which lets a
/// rotation accept parents signed by the previous key; the issuer then refuses
/// a list that omits the current key. Resource evidence keys must differ from
/// every trusted user-authority key, including one retained for rotation.
pub fn authority_exchange(
    is_production: bool,
    signer: &platform::actor_delegation::ActorDelegationSigner,
    rotation_keys_configured: bool,
    resources: platform::actor_delegation::exchange::ResourceEvidenceVerifier,
) -> anyhow::Result<platform::actor_delegation::exchange::DelegationExchange> {
    use platform::actor_delegation::ActorDelegationVerifier;
    use platform::grpc_auth::Workload;
    let parents = if rotation_keys_configured {
        ActorDelegationVerifier::from_env(is_production, Workload::Post)?
    } else {
        ActorDelegationVerifier::new(Workload::Post, &[signer.public_key()])?
    };
    authority_exchange_with_parents(signer, parents, resources)
}

/// [authority_exchange] with an explicit parent verifier.
pub fn authority_exchange_with_parents(
    signer: &platform::actor_delegation::ActorDelegationSigner,
    parents: platform::actor_delegation::ActorDelegationVerifier,
    resources: platform::actor_delegation::exchange::ResourceEvidenceVerifier,
) -> anyhow::Result<platform::actor_delegation::exchange::DelegationExchange> {
    anyhow::ensure!(
        !resources.trusts(&signer.public_key()) && !resources.shares_key_with(&parents),
        "{} must not include the Gateway actor delegation key or any key in {}",
        platform::actor_delegation::exchange::RESOURCE_EVIDENCE_VERIFYING_KEYS_ENV,
        platform::actor_delegation::VERIFYING_KEYS_ENV
    );
    platform::actor_delegation::exchange::DelegationExchange::new(
        signer.clone(),
        parents,
        resources,
    )
}

/// Shortest allowed interval between scheduled signing-key refreshes, in seconds.
pub const MIN_JWT_KEYS_REFRESH_SECONDS: u64 = 5;

/// Time reserved for atomically accepting an expired journal operation into
/// the durable async queue (`GATEWAY_ASYNC_OPERATION_KEYRING`).
pub const ASYNC_OPERATION_ENQUEUE_TIMEOUT_MS: u64 = 250;
const DEFAULT_PARITY_SEARCH_CURSOR_HMAC_KEY: &str =
    "parity-only-search-cursor-key-change-before-production";
const MIN_SEARCH_CURSOR_TTL_SECONDS: u64 = 60;
const MAX_SEARCH_CURSOR_TTL_SECONDS: u64 = 3_600;
const MAX_SEARCH_CURSOR_LEGACY_COMPATIBILITY_DAYS: i64 = 7;

/// Optional links advertised by the Gateway's `/docs` page.
///
/// An unset override uses the local-tools default in parity mode and stays
/// hidden in production. A blank override explicitly hides that link.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeveloperPortalLinkSettings {
    pub swagger: Option<String>,
    pub grafana: Option<String>,
    pub profiles: Option<String>,
    pub prometheus: Option<String>,
    pub pyroscope: Option<String>,
}

/// Validated deployment links shown on the API Gateway developer portal.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct DeveloperPortalLinks {
    swagger: Option<String>,
    grafana: Option<String>,
    profiles: Option<String>,
    prometheus: Option<String>,
    pyroscope: Option<String>,
}

impl DeveloperPortalLinks {
    pub fn from_env(mode: RuntimeMode) -> anyhow::Result<Self> {
        Self::from_settings(
            mode,
            DeveloperPortalLinkSettings {
                swagger: optional_env_setting("GATEWAY_DOCS_SWAGGER_URL")?,
                grafana: optional_env_setting("GATEWAY_DOCS_GRAFANA_URL")?,
                profiles: optional_env_setting("GATEWAY_DOCS_PROFILES_URL")?,
                prometheus: optional_env_setting("GATEWAY_DOCS_PROMETHEUS_URL")?,
                pyroscope: optional_env_setting("GATEWAY_DOCS_PYROSCOPE_URL")?,
            },
        )
    }

    pub fn from_settings(
        mode: RuntimeMode,
        settings: DeveloperPortalLinkSettings,
    ) -> anyhow::Result<Self> {
        let production = matches!(mode, RuntimeMode::Production);
        Ok(Self {
            swagger: resolve_portal_link(
                "GATEWAY_DOCS_SWAGGER_URL",
                settings.swagger,
                (!production).then_some("http://localhost:8090"),
                production,
            )?,
            grafana: resolve_portal_link(
                "GATEWAY_DOCS_GRAFANA_URL",
                settings.grafana,
                (!production).then_some("http://localhost:3000"),
                production,
            )?,
            profiles: resolve_portal_link(
                "GATEWAY_DOCS_PROFILES_URL",
                settings.profiles,
                (!production).then_some("http://localhost:3000/a/grafana-pyroscope-app/explore"),
                production,
            )?,
            prometheus: resolve_portal_link(
                "GATEWAY_DOCS_PROMETHEUS_URL",
                settings.prometheus,
                (!production).then_some("http://localhost:9090"),
                production,
            )?,
            pyroscope: resolve_portal_link(
                "GATEWAY_DOCS_PYROSCOPE_URL",
                settings.pyroscope,
                (!production).then_some("http://localhost:4040"),
                production,
            )?,
        })
    }

    pub fn defaults(mode: RuntimeMode) -> Self {
        Self::from_settings(mode, DeveloperPortalLinkSettings::default())
            .expect("built-in developer portal links are valid")
    }

    pub fn swagger(&self) -> Option<&str> {
        self.swagger.as_deref()
    }

    pub fn grafana(&self) -> Option<&str> {
        self.grafana.as_deref()
    }

    pub fn profiles(&self) -> Option<&str> {
        self.profiles.as_deref()
    }

    pub fn prometheus(&self) -> Option<&str> {
        self.prometheus.as_deref()
    }

    pub fn pyroscope(&self) -> Option<&str> {
        self.pyroscope.as_deref()
    }
}

fn optional_env_setting(name: &str) -> anyhow::Result<Option<String>> {
    match std::env::var(name) {
        Ok(value) => Ok(Some(value)),
        Err(std::env::VarError::NotPresent) => Ok(None),
        Err(std::env::VarError::NotUnicode(_)) => {
            anyhow::bail!("{name} must be valid UTF-8")
        }
    }
}

fn resolve_portal_link(
    name: &str,
    configured: Option<String>,
    default: Option<&str>,
    production: bool,
) -> anyhow::Result<Option<String>> {
    let Some(configured) = configured else {
        return default
            .map(|url| validate_portal_url(name, url, production))
            .transpose();
    };
    if configured.trim().is_empty() {
        return Ok(None);
    }
    validate_portal_url(name, &configured, production).map(Some)
}

fn validate_portal_url(name: &str, value: &str, production: bool) -> anyhow::Result<String> {
    use http::Uri;

    let value = value.trim();
    if value.len() > 2_048 {
        anyhow::bail!("{name} must not exceed 2048 bytes");
    }
    let uri = value
        .parse::<Uri>()
        .map_err(|_| anyhow::anyhow!("{name} must be an absolute HTTP(S) URL"))?;
    let scheme = uri.scheme_str().unwrap_or_default();
    if !(scheme.eq_ignore_ascii_case("http") || scheme.eq_ignore_ascii_case("https")) {
        anyhow::bail!("{name} must use http or https");
    }
    let authority = uri
        .authority()
        .ok_or_else(|| anyhow::anyhow!("{name} must include a host"))?;
    if authority.host().is_empty() || authority.as_str().contains('@') {
        anyhow::bail!("{name} must include a host and must not contain user information");
    }
    if production {
        platform::config::require_production_https_url(true, name, value)?;
    }
    Ok(uri.to_string())
}

/// OAuth transport is off unless a complete, explicitly private fixture is
/// configured. Public exposure remains closed until joint browser qualification.
#[derive(Clone, Default)]
pub struct OAuthExposurePolicy {
    providers: Vec<String>,
    signing_key: Option<Arc<[u8]>>,
    browser_origin: Option<String>,
}

impl std::fmt::Debug for OAuthExposurePolicy {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthExposurePolicy")
            .field("controlled_provider_count", &self.providers.len())
            .field("public_handoff_qualified", &false)
            .finish_non_exhaustive()
    }
}

impl OAuthExposurePolicy {
    fn from_env(mode: RuntimeMode) -> anyhow::Result<Self> {
        let public = env_or("GATEWAY_OAUTH_ENABLED_PROVIDERS", "");
        let controlled = env_or("GATEWAY_OAUTH_CONTROLLED_PROVIDERS", "");
        let key = std::env::var("GATEWAY_OAUTH_HANDOFF_HMAC_KEY").ok();
        let origin = std::env::var("GATEWAY_OAUTH_BROWSER_ORIGIN").ok();
        Self::from_settings(mode, &public, &controlled, key, origin)
    }

    /// Parse the same bounded settings used by startup. This never enables a
    /// public provider or permits controlled transport in production.
    pub fn from_settings(mode: RuntimeMode, public: &str, controlled: &str,
        key: Option<String>, origin: Option<String>) -> anyhow::Result<Self> {
        // A provider secret, requested provider name or claimed profile is not
        // evidence that the matching frontend handoff has been qualified.
        if !public.trim().is_empty() {
            anyhow::bail!("Public OAuth HANDOFF_NOT_QUALIFIED");
        }
        if controlled.trim().is_empty() {
            if key.is_some() || origin.is_some() {
                anyhow::bail!("Incomplete OAuth transport configuration");
            }
            return Ok(Self::default());
        }
        if mode != RuntimeMode::Parity {
            anyhow::bail!("Controlled OAuth transport is forbidden in production");
        }
        let mut providers = Vec::new();
        for provider in controlled.split(',') {
            let provider = provider.trim().to_ascii_lowercase();
            if !matches!(provider.as_str(), "google" | "apple" | "x")
                || providers.contains(&provider)
            {
                anyhow::bail!("Unknown or duplicate controlled OAuth provider");
            }
            providers.push(provider);
        }
        let key = key.filter(|value| value.len() >= 32 && value.len() <= 4096)
            .ok_or_else(|| anyhow::anyhow!("OAuth signing key must contain 32 to 4096 bytes"))?;
        let origin = origin.ok_or_else(|| anyhow::anyhow!("OAuth browser origin required"))?;
        let parsed: axum::http::Uri = origin.parse()
            .map_err(|_| anyhow::anyhow!("Invalid OAuth browser origin"))?;
        let loopback = matches!(parsed.host(), Some("localhost" | "127.0.0.1" | "[::1]" | "::1"));
        if parsed.authority().is_none() || origin.contains('@') || origin.ends_with('/')
            || parsed.query().is_some() || parsed.path() != "/"
            || !(parsed.scheme_str() == Some("https")
                || (parsed.scheme_str() == Some("http") && loopback))
        {
            anyhow::bail!("OAuth origin must be an exact HTTPS origin or controlled loopback origin");
        }
        Ok(Self { providers, signing_key: Some(Arc::from(key.into_bytes())), browser_origin: Some(origin) })
    }

    pub fn has_transport(&self) -> bool { !self.providers.is_empty() }

    pub fn transport_enabled(&self, provider: &str) -> bool {
        self.providers.iter().any(|value| value == provider)
    }

    pub fn public_enabled(&self, _provider: &str) -> bool {
        // Unfinished joint FE/backend qualification is an explicit release
        // prerequisite. There is deliberately no operator boolean bypass.
        false
    }

    pub(crate) fn signing_key(&self) -> Option<&[u8]> { self.signing_key.as_deref() }
    pub(crate) fn browser_origin(&self) -> Option<&str> { self.browser_origin.as_deref() }

    fn validate_settings(&self, secure: bool, domain: &str, origins: &[String], admission_key: &str) -> anyhow::Result<()> {
        if self.has_transport() {
            if !secure || !domain.is_empty() || !origins.iter().any(|origin| Some(origin.as_str()) == self.browser_origin()) {
                anyhow::bail!("OAuth requires Secure cookies and its exact configured browser origin");
            }
            if self.signing_key() == Some(admission_key.as_bytes()) {
                anyhow::bail!("OAuth signing key must be independent of the admission key");
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod oauth_exposure_tests {
    use super::{OAuthExposurePolicy, RuntimeMode};

    fn controlled(mode: RuntimeMode, providers: &str) -> anyhow::Result<OAuthExposurePolicy> {
        OAuthExposurePolicy::from_settings(mode, "", providers,
            Some("unit-only-independent-handoff-key-32bytes".into()),
            Some("https://mcr.example.test".into()))
    }

    #[test]
    fn absent_settings_keep_every_transport_and_capability_off() {
        let policy = OAuthExposurePolicy::from_settings(RuntimeMode::Production, "", "", None, None).unwrap();
        assert!(!policy.has_transport());
        for provider in ["google", "apple", "x"] {
            assert!(!policy.transport_enabled(provider));
            assert!(!policy.public_enabled(provider));
        }
    }

    #[test]
    fn public_or_incomplete_or_production_fixture_settings_fail_closed() {
        assert!(OAuthExposurePolicy::from_settings(RuntimeMode::Production, "google", "", None, None).is_err());
        assert!(controlled(RuntimeMode::Production, "google").is_err());
        assert!(controlled(RuntimeMode::Parity, "google,google").is_err());
        assert!(controlled(RuntimeMode::Parity, "naver").is_err());
        assert!(OAuthExposurePolicy::from_settings(RuntimeMode::Parity, "", "google", None, None).is_err());
        assert!(OAuthExposurePolicy::from_settings(RuntimeMode::Parity, "", "", Some("unused".into()), None).is_err());
    }

    #[test]
    fn controlled_transport_never_advertises_public_readiness_and_requires_cookie_origin_policy() {
        let policy = controlled(RuntimeMode::Parity, "google,apple").unwrap();
        assert!(policy.transport_enabled("google"));
        assert!(!policy.transport_enabled("x"));
        assert!(!policy.public_enabled("google"));
        let origins = vec!["https://mcr.example.test".to_owned()];
        assert!(policy.validate_settings(true, "", &origins, "different-admission-key").is_ok());
        assert!(policy.validate_settings(false, "", &origins, "different").is_err());
        assert!(policy.validate_settings(true, ".example.test", &origins, "different").is_err());
        assert!(policy.validate_settings(true, "", &[], "different").is_err());
        assert!(policy.validate_settings(true, "", &origins, "unit-only-independent-handoff-key-32bytes").is_err());
        assert!(!format!("{policy:?}").contains("unit-only-independent"));
    }
}

#[derive(Clone)]
pub struct AppConfig {
    pub http_addr: SocketAddr,
    pub metrics_addr: SocketAddr,
    /// Internal mutually authenticated gRPC listener. It serves only the
    /// authorization exchange, and only Post may call it.
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
    /// Connections reserved for request-path journal work: durable intents,
    /// completions, durable queue acceptance, owner replay and operation
    /// lookups (`GATEWAY_JOURNAL_DB_POOL_MAX_CONNECTIONS`).
    pub journal_pool_max_connections: u32,
    /// Connections for background journal work: the durable-operation worker
    /// and stale-operation maintenance
    /// (`GATEWAY_BACKGROUND_DB_POOL_MAX_CONNECTIONS`). Background polling runs
    /// on its own pool so it can never take the connection a request needs to
    /// record its outcome.
    pub background_pool_max_connections: u32,
    pub seed_enabled: bool,
    pub auth_grpc_endpoint: String,
    /// Explicit opt-in; absent Concert configuration keeps bootstrap unavailable.
    pub concert_grpc_endpoint: Option<String>,
    /// Operator-selected digest of the same validated release served by Concert.
    pub concert_release_digest: Option<String>,
    pub profile_grpc_endpoint: String,
    pub friend_grpc_endpoint: String,
    pub notification_grpc_endpoint: String,
    pub post_grpc_endpoint: String,
    pub refresh_cookie_domain: String,
    pub oauth_exposure: OAuthExposurePolicy,
    pub refresh_cookie_secure: bool,
    pub refresh_cookie_max_age_seconds: i64,
    pub aggregation_cache_ttl_seconds: u64,
    pub cors_allowed_origins: Vec<String>,
    pub jwt_issuer: String,
    pub jwt_audience: String,
    /// Signing keys given in configuration (`GATEWAY_JWT_JWKS_JSON`).
    ///
    /// With key refresh on, this is an optional starting set that lets the
    /// Gateway verify tokens before the issuer first answers; the first
    /// successful refresh replaces it. With refresh off, it is the only key
    /// source and is required.
    pub jwt_jwks_json: String,
    /// Seconds between refreshes of the issuer's signing keys over the auth
    /// channel (`GATEWAY_JWT_KEYS_REFRESH_SECONDS`). Zero turns refresh off.
    ///
    /// This bounds how long a newly published key takes to reach this Gateway
    /// and how long a withdrawn key keeps verifying here.
    pub jwt_keys_refresh_seconds: u64,
    /// Least seconds between refreshes that tokens naming an unknown key may
    /// start (`GATEWAY_JWT_KEYS_MIN_REFRESH_SECONDS`). A flood of made-up key
    /// ids therefore costs at most one fetch per this interval.
    pub jwt_keys_min_refresh_seconds: u64,
    /// Seconds a high-risk session confirmation from the issuer is reused
    /// (`GATEWAY_HIGH_RISK_SESSION_FRESHNESS_SECONDS`, 0 to 30). Zero asks the
    /// issuer on every high-risk request.
    ///
    /// This is the documented bound on how long a logged-out or demoted session
    /// can keep performing administration, account deletion, or provider
    /// linking after the issuer last confirmed it.
    pub high_risk_session_freshness_seconds: u64,
    pub runtime_mode: RuntimeMode,
    /// Optional external links shown by the public API docs page.
    pub developer_portal_links: DeveloperPortalLinks,
    /// Read budget per client network per minute
    /// (`GATEWAY_RATE_LIMIT_PER_MINUTE`). Other classes are in `admission`.
    pub rate_limit_per_minute: u32,
    /// Budgets for the other route classes (`GATEWAY_ADMISSION_BUDGETS`), the
    /// anonymous identity budgets (`GATEWAY_ADMISSION_IDENTITY_BUDGETS`),
    /// authenticated principal budgets (`GATEWAY_ADMISSION_PRINCIPAL_BUDGETS`),
    /// overflow factor (`GATEWAY_ADMISSION_OVERFLOW_FACTOR`), the per-instance
    /// read allowance during a store outage
    /// (`GATEWAY_ADMISSION_FALLBACK_PER_SECOND`), and the IPv6 prefix clients
    /// are keyed by (`GATEWAY_RATE_LIMIT_V6_PREFIX_LENGTH`), plus the
    /// enforce/shadow rollout mode (`GATEWAY_ADMISSION_MODE`).
    pub admission: AdmissionPolicies,
    /// Shared secret for keyed anonymous-identity and authenticated-principal
    /// budgets (`GATEWAY_ADMISSION_HMAC_KEY`).
    pub admission_hmac_key: String,
    /// Shared key used to sign public Post/Profile search cursors.
    /// (`GATEWAY_SEARCH_CURSOR_HMAC_KEY`).
    pub search_cursor_hmac_key: String,
    /// Lifetime for newly issued public search cursors
    /// (`GATEWAY_SEARCH_CURSOR_TTL_SECONDS`).
    pub search_cursor_ttl_seconds: u64,
    /// Absolute end of the temporary raw-cursor compatibility window
    /// (`GATEWAY_SEARCH_CURSOR_LEGACY_ACCEPT_UNTIL`).
    pub search_cursor_legacy_accept_until: Option<DateTime<Utc>>,
    pub request_timeout_ms: u64,
    /// How long the journal lets a caller wait before answering with the operation id.
    ///
    /// This must be strictly shorter than `request_timeout_ms`. The outer timeout
    /// middleware returns a bare 504 when it fires, so a journal window at or above
    /// it would never be reached and a timed-out caller would lose the operation id
    /// it needs to check the outcome it just gave up on.
    pub journal_client_timeout_ms: u64,
    /// Optional encryption keyring enabling durable Gateway command acceptance
    /// (`GATEWAY_ASYNC_OPERATION_KEYRING`). The worker and 202 path remain off
    /// when this is absent.
    pub async_operation_keyring: Option<String>,
    /// Days of telemetry to keep before the maintenance purge removes it.
    pub telemetry_retention_days: i64,
    /// Records allowed to wait for the telemetry writer before one is shed.
    pub telemetry_queue_capacity: u32,
    /// Days of audit evidence to keep. Reconciliation-required rows are retained
    /// regardless of age.
    pub audit_retention_days: i64,
    /// Callers this service accepts on its internal listener. Only Post calls
    /// the Gateway, to exchange its admitted authority for a one-hop child.
    pub caller_registry: platform::grpc_auth::CallerRegistry,
    /// Signs actor delegations on Gateway-forwarded owner calls.
    pub actor_delegation_signer: platform::actor_delegation::ActorDelegationSigner,
    /// Child-authority issuer served on the internal listener. It verifies the
    /// Post parent with the current signing key (plus, during rotation, the
    /// keys in ACTOR_DELEGATION_VERIFYING_KEYS) and Post resource evidence with
    /// RESOURCE_EVIDENCE_VERIFYING_KEYS.
    pub authority_exchange: platform::actor_delegation::exchange::DelegationExchange,
    /// Credential the edge presents on outbound internal calls.
    pub outbound_identity: String,
    /// Mutual TLS material for internal gRPC, resolved at load time.
    ///
    /// `None` means the documented plaintext development path. Production cannot
    /// reach a running service with `None` because `load` fails first.
    pub tls_material: Option<platform::grpc_tls::TlsMaterial>,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let runtime_mode = RuntimeMode::parse(&env_or("GATEWAY_RUNTIME_MODE", "production"))?;
        let developer_portal_links = DeveloperPortalLinks::from_env(runtime_mode)?;
        let search_cursor_legacy_accept_until =
            match std::env::var("GATEWAY_SEARCH_CURSOR_LEGACY_ACCEPT_UNTIL") {
                Ok(value) if !value.trim().is_empty() => Some(
                    DateTime::parse_from_rfc3339(value.trim())
                        .map_err(|_| {
                            anyhow::anyhow!(
                            "GATEWAY_SEARCH_CURSOR_LEGACY_ACCEPT_UNTIL must be an RFC3339 timestamp"
                        )
                        })?
                        .with_timezone(&Utc),
                ),
                _ => None,
            };
        let is_production = matches!(runtime_mode, RuntimeMode::Production);
        let caller_registry = platform::config::build_caller_registry(
            is_production,
            &[platform::grpc_auth::Workload::Post],
            &[platform::grpc_auth::Workload::Post],
        )?;
        let outbound_identity = platform::config::resolve_outbound_identity(
            is_production,
            platform::grpc_auth::Workload::Gateway,
        )?;
        let tls_material = platform::config::resolve_tls_material(is_production, "gateway")?;

        let actor_delegation_signer = platform::actor_delegation::ActorDelegationSigner::from_env(
            is_production,
            platform::grpc_auth::Workload::Gateway,
        )?;
        let authority_exchange = authority_exchange(
            is_production,
            &actor_delegation_signer,
            platform::config::optional_env(platform::actor_delegation::VERIFYING_KEYS_ENV)
                .is_some(),
            platform::actor_delegation::exchange::ResourceEvidenceVerifier::from_env(
                is_production,
            )?,
        )?;
        let config = Self {
            database_migration_mode: migration_runtime::MigrationMode::from_env_value(
                std::env::var("GATEWAY_DATABASE_MIGRATION_MODE"),
            )?,
            http_addr: env_or("GATEWAY_HTTP_ADDR", "127.0.0.1:8080").parse()?,
            metrics_addr: env_or("GATEWAY_METRICS_ADDR", DEFAULT_METRICS_ADDR).parse()?,
            grpc_addr: env_or("GATEWAY_GRPC_ADDR", DEFAULT_GRPC_ADDR).parse()?,
            database_url: env_or(
                "GATEWAY_DATABASE_URL",
                "postgres://postgres:postgres@localhost:5432/gateway_db",
            ),
            database_pool_max_connections: parse_u32_setting(
                "GATEWAY_DB_POOL_MAX_CONNECTIONS",
                &env_or("GATEWAY_DB_POOL_MAX_CONNECTIONS", "5"),
                1,
                512,
            )?,
            journal_pool_max_connections: parse_u32_setting(
                "GATEWAY_JOURNAL_DB_POOL_MAX_CONNECTIONS",
                &env_or("GATEWAY_JOURNAL_DB_POOL_MAX_CONNECTIONS", "4"),
                2,
                64,
            )?,
            background_pool_max_connections: parse_u32_setting(
                "GATEWAY_BACKGROUND_DB_POOL_MAX_CONNECTIONS",
                &env_or("GATEWAY_BACKGROUND_DB_POOL_MAX_CONNECTIONS", "3"),
                1,
                32,
            )?,
            seed_enabled: parse_bool_setting(
                "GATEWAY_SEED_ENABLED",
                &env_or("GATEWAY_SEED_ENABLED", "false"),
            )?,
            auth_grpc_endpoint: env_or("GATEWAY_AUTH_GRPC_ENDPOINT", "http://127.0.0.1:50051"),
            concert_grpc_endpoint: platform::config::optional_env("GATEWAY_CONCERT_GRPC_ENDPOINT"),
            concert_release_digest: platform::config::optional_env(
                "GATEWAY_CONCERT_RELEASE_DIGEST",
            ),
            profile_grpc_endpoint: env_or(
                "GATEWAY_PROFILE_GRPC_ENDPOINT",
                "http://127.0.0.1:50052",
            ),
            friend_grpc_endpoint: env_or("GATEWAY_FRIEND_GRPC_ENDPOINT", "http://127.0.0.1:50053"),
            notification_grpc_endpoint: env_or(
                "GATEWAY_NOTIFICATION_GRPC_ENDPOINT",
                "http://127.0.0.1:50054",
            ),
            post_grpc_endpoint: env_or("GATEWAY_POST_GRPC_ENDPOINT", "http://127.0.0.1:50055"),
            refresh_cookie_domain: env_or("GATEWAY_REFRESH_COOKIE_DOMAIN", ""),
            oauth_exposure: OAuthExposurePolicy::from_env(runtime_mode)?,
            refresh_cookie_secure: parse_bool_setting(
                "GATEWAY_REFRESH_COOKIE_SECURE",
                &env_or("GATEWAY_REFRESH_COOKIE_SECURE", "false"),
            )?,
            refresh_cookie_max_age_seconds: parse_i64_setting(
                "GATEWAY_REFRESH_COOKIE_MAX_AGE_SECONDS",
                &env_or("GATEWAY_REFRESH_COOKIE_MAX_AGE_SECONDS", "1209600"),
                60,
                31_536_000,
            )?,
            aggregation_cache_ttl_seconds: parse_u64_setting(
                "GATEWAY_AGGREGATION_CACHE_TTL_SECONDS",
                &env_or("GATEWAY_AGGREGATION_CACHE_TTL_SECONDS", "30"),
                0,
                86_400,
            )?,
            cors_allowed_origins: env_or(
                "GATEWAY_CORS_ALLOWED_ORIGINS",
                "http://localhost:3000,http://localhost:5173,http://localhost:8090",
            )
            .split(',')
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(ToOwned::to_owned)
            .collect(),
            jwt_issuer: env_or("GATEWAY_JWT_ISSUER", "auth-service"),
            jwt_audience: require_non_empty_setting(
                "GATEWAY_JWT_AUDIENCE",
                env_or("GATEWAY_JWT_AUDIENCE", "port-rust-api"),
            )?,
            jwt_jwks_json: env_or("GATEWAY_JWT_JWKS_JSON", ""),
            jwt_keys_refresh_seconds: parse_u64_setting(
                "GATEWAY_JWT_KEYS_REFRESH_SECONDS",
                &env_or("GATEWAY_JWT_KEYS_REFRESH_SECONDS", "60"),
                0,
                3_600,
            )?,
            jwt_keys_min_refresh_seconds: parse_u64_setting(
                "GATEWAY_JWT_KEYS_MIN_REFRESH_SECONDS",
                &env_or("GATEWAY_JWT_KEYS_MIN_REFRESH_SECONDS", "10"),
                1,
                3_600,
            )?,
            high_risk_session_freshness_seconds: parse_u64_setting(
                "GATEWAY_HIGH_RISK_SESSION_FRESHNESS_SECONDS",
                &env_or("GATEWAY_HIGH_RISK_SESSION_FRESHNESS_SECONDS", "5"),
                0,
                crate::application::service::high_risk_session_service::MAX_SESSION_FRESHNESS
                    .as_secs(),
            )?,
            runtime_mode,
            developer_portal_links,
            rate_limit_per_minute: parse_u32_setting(
                "GATEWAY_RATE_LIMIT_PER_MINUTE",
                &env_or("GATEWAY_RATE_LIMIT_PER_MINUTE", "600"),
                1,
                60_000,
            )?,
            admission: admission_from_env()?,
            admission_hmac_key: env_or("GATEWAY_ADMISSION_HMAC_KEY", ""),
            search_cursor_hmac_key: env_or(
                "GATEWAY_SEARCH_CURSOR_HMAC_KEY",
                DEFAULT_PARITY_SEARCH_CURSOR_HMAC_KEY,
            ),
            search_cursor_ttl_seconds: parse_u64_setting(
                "GATEWAY_SEARCH_CURSOR_TTL_SECONDS",
                &env_or("GATEWAY_SEARCH_CURSOR_TTL_SECONDS", "900"),
                MIN_SEARCH_CURSOR_TTL_SECONDS,
                MAX_SEARCH_CURSOR_TTL_SECONDS,
            )?,
            search_cursor_legacy_accept_until,
            request_timeout_ms: parse_u64_setting(
                "GATEWAY_REQUEST_TIMEOUT_MS",
                &env_or("GATEWAY_REQUEST_TIMEOUT_MS", "1000"),
                50,
                60_000,
            )?,
            journal_client_timeout_ms: parse_u64_setting(
                "GATEWAY_JOURNAL_CLIENT_TIMEOUT_MS",
                &env_or("GATEWAY_JOURNAL_CLIENT_TIMEOUT_MS", "900"),
                10,
                59_000,
            )?,
            async_operation_keyring: std::env::var("GATEWAY_ASYNC_OPERATION_KEYRING")
                .ok()
                .filter(|value| !value.trim().is_empty()),
            telemetry_retention_days: platform::config::parse_i64_setting(
                "GATEWAY_TELEMETRY_RETENTION_DAYS",
                &env_or("GATEWAY_TELEMETRY_RETENTION_DAYS", "30"),
                1,
                3_650,
            )?,
            telemetry_queue_capacity: platform::config::parse_u32_setting(
                "GATEWAY_TELEMETRY_QUEUE_CAPACITY",
                &env_or("GATEWAY_TELEMETRY_QUEUE_CAPACITY", "4096"),
                1,
                1_048_576,
            )?,
            audit_retention_days: platform::config::parse_i64_setting(
                "GATEWAY_AUDIT_RETENTION_DAYS",
                &env_or("GATEWAY_AUDIT_RETENTION_DAYS", "365"),
                1,
                3_650,
            )?,
            caller_registry,
            actor_delegation_signer,
            authority_exchange,
            outbound_identity,
            tls_material,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        self.oauth_exposure.validate_settings(self.refresh_cookie_secure, &self.refresh_cookie_domain,
            &self.cors_allowed_origins, &self.admission_hmac_key)?;
        let production = matches!(self.runtime_mode, RuntimeMode::Production);
        if !self.admission_hmac_key.is_empty()
            && !(32..=4096).contains(&self.admission_hmac_key.len())
        {
            anyhow::bail!("GATEWAY_ADMISSION_HMAC_KEY must contain between 32 and 4096 bytes");
        }
        if (self.admission.has_identity_budgets() || self.admission.has_principal_budgets())
            && self.admission_hmac_key.is_empty()
        {
            anyhow::bail!(
                "GATEWAY_ADMISSION_HMAC_KEY is required when GATEWAY_ADMISSION_IDENTITY_BUDGETS or GATEWAY_ADMISSION_PRINCIPAL_BUDGETS is configured"
            );
        }
        if !(32..=4096).contains(&self.search_cursor_hmac_key.len()) {
            anyhow::bail!("GATEWAY_SEARCH_CURSOR_HMAC_KEY must contain between 32 and 4096 bytes");
        }
        if production && self.search_cursor_hmac_key == DEFAULT_PARITY_SEARCH_CURSOR_HMAC_KEY {
            anyhow::bail!(
                "GATEWAY_SEARCH_CURSOR_HMAC_KEY must be set to a shared secret in production"
            );
        }
        if !(MIN_SEARCH_CURSOR_TTL_SECONDS..=MAX_SEARCH_CURSOR_TTL_SECONDS)
            .contains(&self.search_cursor_ttl_seconds)
        {
            anyhow::bail!("GATEWAY_SEARCH_CURSOR_TTL_SECONDS must be between 60 and 3600");
        }
        if self
            .search_cursor_legacy_accept_until
            .is_some_and(|deadline| {
                deadline
                    > Utc::now() + ChronoDuration::days(MAX_SEARCH_CURSOR_LEGACY_COMPATIBILITY_DAYS)
            })
        {
            anyhow::bail!(
                "GATEWAY_SEARCH_CURSOR_LEGACY_ACCEPT_UNTIL may be at most seven days in the future"
            );
        }
        require_production_safe_socket_addr(production, "GATEWAY_HTTP_ADDR", self.http_addr)?;
        require_production_safe_socket_addr(production, "GATEWAY_GRPC_ADDR", self.grpc_addr)?;
        require_production_postgres_url(production, "GATEWAY_DATABASE_URL", &self.database_url)?;
        require_production_disabled(
            production,
            "GATEWAY_SEED_ENABLED",
            self.seed_enabled,
            "demo seed data must not run in production",
        )?;

        // The journal's client window must leave room for the outer timeout to fire
        // after it. If the outer middleware fired first it would return a bare 504,
        // and the caller would lose the operation id needed to check the outcome.
        if self.journal_client_timeout_ms >= self.request_timeout_ms {
            anyhow::bail!(
                "GATEWAY_JOURNAL_CLIENT_TIMEOUT_MS must be lower than GATEWAY_REQUEST_TIMEOUT_MS so a timed-out caller still receives the operation id"
            );
        }
        if let Some(keyring) = self.async_operation_keyring.as_deref() {
            PayloadCipher::from_hex_keyring(keyring)
                .map_err(|_| anyhow::anyhow!("GATEWAY_ASYNC_OPERATION_KEYRING is invalid"))?;
            const ACCEPTANCE_RESPONSE_MARGIN_MS: u64 = 20;
            let acceptance_deadline = self
                .journal_client_timeout_ms
                .checked_add(ASYNC_OPERATION_ENQUEUE_TIMEOUT_MS)
                .and_then(|deadline| deadline.checked_add(ACCEPTANCE_RESPONSE_MARGIN_MS));
            if acceptance_deadline.is_none_or(|deadline| deadline >= self.request_timeout_ms) {
                anyhow::bail!(
                    "GATEWAY_JOURNAL_CLIENT_TIMEOUT_MS must leave time for durable queue acceptance before GATEWAY_REQUEST_TIMEOUT_MS"
                );
            }
        }

        for (name, endpoint) in [
            ("GATEWAY_AUTH_GRPC_ENDPOINT", &self.auth_grpc_endpoint),
            ("GATEWAY_PROFILE_GRPC_ENDPOINT", &self.profile_grpc_endpoint),
            ("GATEWAY_FRIEND_GRPC_ENDPOINT", &self.friend_grpc_endpoint),
            (
                "GATEWAY_NOTIFICATION_GRPC_ENDPOINT",
                &self.notification_grpc_endpoint,
            ),
            ("GATEWAY_POST_GRPC_ENDPOINT", &self.post_grpc_endpoint),
        ] {
            require_production_safe_url(production, name, endpoint)?;
        }
        if let Some(endpoint) = self.concert_grpc_endpoint.as_ref() {
            let uri = endpoint.parse::<axum::http::Uri>().map_err(|_| {
                anyhow::anyhow!("GATEWAY_CONCERT_GRPC_ENDPOINT must be an HTTP(S) endpoint")
            })?;
            anyhow::ensure!(
                matches!(uri.scheme_str(), Some("http" | "https"))
                    && uri.authority().is_some()
                    && uri.host().is_some_and(|host| !host.is_empty())
                    && uri.query().is_none()
                    && matches!(uri.path(), "" | "/")
                    && !endpoint.contains(['@', '#']),
                "GATEWAY_CONCERT_GRPC_ENDPOINT must be an HTTP(S) endpoint without credentials, path or query"
            );
            require_production_safe_url(production, "GATEWAY_CONCERT_GRPC_ENDPOINT", endpoint)?;
        }

        if let Some(digest) = &self.concert_release_digest {
            anyhow::ensure!(
                self.concert_grpc_endpoint.is_some()
                    && digest.len() == 64
                    && digest
                        .bytes()
                        .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
                    && digest.bytes().any(|b| b != b'0'),
                "GATEWAY_CONCERT_RELEASE_DIGEST requires Concert endpoint and exact release digest"
            );
        }
        require_production_safe_urls(
            production,
            "GATEWAY_CORS_ALLOWED_ORIGINS",
            &self.cors_allowed_origins,
        )?;
        if production && self.cors_allowed_origins.is_empty() {
            anyhow::bail!("GATEWAY_CORS_ALLOWED_ORIGINS must not be empty in production");
        }
        for origin in &self.cors_allowed_origins {
            HeaderValue::from_str(origin).map_err(|error| {
                anyhow::anyhow!("GATEWAY_CORS_ALLOWED_ORIGINS contains an invalid origin: {error}")
            })?;
            let uri: http::Uri = origin.parse().map_err(|error| {
                anyhow::anyhow!("GATEWAY_CORS_ALLOWED_ORIGINS contains an invalid URI: {error}")
            })?;
            if !matches!(uri.scheme_str(), Some("http" | "https"))
                || origin.contains('*')
                || uri.authority().is_none()
                || uri
                    .authority()
                    .is_some_and(|authority| authority.as_str().contains('@'))
                || uri
                    .path_and_query()
                    .is_some_and(|path| path.as_str() != "/")
                || origin
                    != &format!(
                        "{}://{}",
                        uri.scheme_str().unwrap_or(""),
                        uri.authority()
                            .map(|authority| authority.as_str())
                            .unwrap_or("")
                    )
            {
                anyhow::bail!("GATEWAY_CORS_ALLOWED_ORIGINS must contain explicit HTTP(S) origins");
            }
        }

        if production && !self.refresh_cookie_secure {
            anyhow::bail!("GATEWAY_REFRESH_COOKIE_SECURE must be true in production");
        }
        if production && self.refresh_cookie_domain.trim().is_empty() {
            anyhow::bail!("GATEWAY_REFRESH_COOKIE_DOMAIN is required in production");
        }
        require_non_empty_setting("GATEWAY_JWT_ISSUER", self.jwt_issuer.clone())?;
        require_non_empty_setting("GATEWAY_JWT_AUDIENCE", self.jwt_audience.clone())?;
        self.validate_signing_key_sources()?;
        Ok(())
    }

    /// Keys come from configuration, from the issuer, or both; at least one
    /// source must be able to supply them.
    fn validate_signing_key_sources(&self) -> anyhow::Result<()> {
        if self.jwt_keys_refresh_seconds == 0 {
            // Configuration is then the only source. An empty set would start a
            // Gateway that can never accept a token.
            require_non_empty_setting("GATEWAY_JWT_JWKS_JSON", self.jwt_jwks_json.clone())
                .map_err(|_| {
                    anyhow::anyhow!(
                        "GATEWAY_JWT_JWKS_JSON is required when GATEWAY_JWT_KEYS_REFRESH_SECONDS is 0"
                    )
                })?;
            return Ok(());
        }
        if self.jwt_keys_refresh_seconds < MIN_JWT_KEYS_REFRESH_SECONDS {
            anyhow::bail!(
                "GATEWAY_JWT_KEYS_REFRESH_SECONDS must be 0 or at least {MIN_JWT_KEYS_REFRESH_SECONDS}"
            );
        }
        if self.jwt_keys_min_refresh_seconds > self.jwt_keys_refresh_seconds {
            anyhow::bail!(
                "GATEWAY_JWT_KEYS_MIN_REFRESH_SECONDS must not exceed GATEWAY_JWT_KEYS_REFRESH_SECONDS"
            );
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
            _ => anyhow::bail!("GATEWAY_RUNTIME_MODE must be production or parity"),
        }
    }

    pub fn allows_parity_routes(self) -> bool {
        matches!(self, Self::Parity)
    }
}

#[derive(Clone)]
pub struct AppState {
    /// Fresh signed owner reads for Concert. Test-double compositions leave this
    /// absent; only the configured concrete clients populate it.
    pub concert_identity: Option<crate::adapters::out::concert_identity::ConcertIdentityReader>,
    pub concert_client: Option<crate::adapters::out::concert_client::GrpcConcertClient>,
    pub concert_product_client:
        Option<crate::adapters::out::concert_product::GrpcConcertProductClient>,
    pub concert_world_client: Option<crate::adapters::out::concert_world::GrpcConcertWorldClient>,
    pub concert_participant_client:
        Option<Arc<crate::adapters::out::concert_participant::GrpcConcertParticipantClient>>,
    pub auth_service: AuthGatewayService,
    pub oauth_exposure: OAuthExposurePolicy,
    pub enrollment_policy: Arc<dyn crate::application::enrollment_policy::EnrollmentPolicyPort>,
    pub profile_service: ProfileGatewayService,
    pub friend_service: FriendGatewayService,
    pub moderation_service: ModerationService,
    pub operation_journal_service: OperationJournalService,
    pub journal_dispatch_repository:
        Arc<dyn crate::application::ports::JournalDispatchRepositoryPort>,
    pub notification_service: NotificationGatewayService,
    pub post_service: PostGatewayService,
    pub search_cursor_codec: SearchCursorCodec,
    pub aggregation_service: AggregationService,
    pub route_contract_service: RouteContractService,
    pub policy_service: PolicyService,
    pub client_profile_service: ClientProfileService,
    /// Accepts telemetry records without waiting for the write to complete.
    pub telemetry_service: QueuedTelemetry,
    pub refresh_cookie_domain: String,
    pub refresh_cookie_secure: bool,
    pub refresh_cookie_max_age_seconds: i64,
    pub jwt_verifier: JwtVerifier,
    /// Exact browser mutation allowlist, shared with explicitly configured CORS origins.
    pub browser_auth_origins: Vec<String>,
    /// Keeps `jwt_verifier` current with the issuer. `None` when signing keys
    /// come only from configuration.
    pub signing_keys: Option<SigningKeyRefresher>,
    /// Confirms administration, account deletion, and provider linking with
    /// the issuer, so they stop within a bounded window of logout or demotion
    /// rather than when the access token expires.
    pub high_risk_sessions: HighRiskSessionGuard,
    pub runtime_mode: RuntimeMode,
    pub developer_portal_links: DeveloperPortalLinks,
    pub rate_limiter: InMemoryRateLimiter,
    /// HMAC key used to pseudonymize anonymous identifiers and verified token
    /// subjects before they are used as admission keys.
    pub admission_hmac_key: Arc<[u8]>,
    pub request_timeout: Duration,
    /// How long the journal lets a caller wait before answering with the operation id.
    pub journal_client_timeout: Duration,
    /// Present only when the encrypted durable command worker is configured.
    pub async_operation_cipher: Option<Arc<dyn crate::application::ports::QueuePayloadPort>>,
    pub async_operation_queue:
        Arc<dyn crate::application::ports::AsyncOperationQueueRepositoryPort>,
    /// Request-path journal capabilities on the journal pool. Inbound code
    /// receives only these ports; no connection pool is reachable from AppState.
    pub journal: crate::application::journal_runtime_ports::JournalRuntimePorts,
    pub legacy_audit_repository: Arc<dyn crate::application::ports::LegacyAuditRepositoryPort>,
    pub metrics: GatewayMetrics,
}

#[derive(Clone, Default)]
pub struct GatewayMetrics {
    counters: Arc<Mutex<BTreeMap<String, u64>>>,
    gauges: Arc<Mutex<BTreeMap<String, i64>>>,
    admission_duration:
        Arc<Mutex<BTreeMap<(RouteClass, AdmissionScope), AdmissionDurationHistogram>>>,
    admission_lock_wait:
        Arc<Mutex<BTreeMap<(RouteClass, AdmissionScope), AdmissionDurationHistogram>>>,
    edge_deadlines: EdgeDeadlineMetrics,
    database_pools: Arc<Mutex<BTreeMap<DatabasePoolRole, PgPool>>>,
    database_acquire_duration:
        Arc<Mutex<BTreeMap<(DatabasePoolRole, DatabasePoolCaller), AdmissionDurationHistogram>>>,
}

/// Closed labels keep pool observability independent of user/configuration data.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DatabasePoolRole {
    Main,
    Journal,
    Background,
    Quota,
}

impl DatabasePoolRole {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Main => "main",
            Self::Journal => "journal",
            Self::Background => "background",
            Self::Quota => "quota",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum DatabasePoolCaller {
    Intent,
    Completion,
    OwnerReplay,
    WorkerClaim,
}

impl DatabasePoolCaller {
    fn as_str(self) -> &'static str {
        match self {
            Self::Intent => "intent",
            Self::Completion => "completion",
            Self::OwnerReplay => "owner_replay",
            Self::WorkerClaim => "worker_claim",
        }
    }
}

pub struct ObservedDatabaseConnection {
    connection: sqlx::pool::PoolConnection<sqlx::Postgres>,
    metrics: GatewayMetrics,
    role: DatabasePoolRole,
    caller: DatabasePoolCaller,
    acquired_at: tokio::time::Instant,
}

impl Deref for ObservedDatabaseConnection {
    type Target = sqlx::PgConnection;
    fn deref(&self) -> &Self::Target {
        &self.connection
    }
}

impl DerefMut for ObservedDatabaseConnection {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.connection
    }
}

impl Drop for ObservedDatabaseConnection {
    fn drop(&mut self) {
        let labels = &[
            ("pool", self.role.as_str()),
            ("caller", self.caller.as_str()),
        ];
        self.metrics.add(
            "gateway_db_pool_hold_microseconds_total",
            labels,
            self.acquired_at.elapsed().as_micros().min(u64::MAX as u128) as u64,
        );
        self.metrics
            .adjust_gauge("gateway_db_pool_observed_holders", labels, -1);
    }
}

const ADMISSION_DURATION_BUCKETS_SECONDS: [f64; 7] = [0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 1.0];
const ADMISSION_DURATION_BUCKET_LABELS: [&str; 8] = [
    "0.005", "0.01", "0.025", "0.05", "0.1", "0.25", "1.0", "+Inf",
];

#[derive(Clone, Default)]
struct AdmissionDurationHistogram {
    buckets: [u64; 8],
    count: u64,
    sum_seconds: f64,
}

impl GatewayMetrics {
    const MAX_SERIES: usize = 4096;

    pub fn register_database_pool(&self, role: DatabasePoolRole, pool: PgPool) {
        self.database_pools
            .lock()
            .expect("gateway database pools")
            .insert(role, pool);
    }

    pub fn observe_database_acquire(
        &self,
        role: DatabasePoolRole,
        caller: DatabasePoolCaller,
        elapsed: Duration,
        timed_out: bool,
    ) {
        let labels = &[("pool", role.as_str()), ("caller", caller.as_str())];
        self.inc("gateway_db_pool_acquire_total", labels);
        self.add(
            "gateway_db_pool_acquire_wait_microseconds_total",
            labels,
            elapsed.as_micros().min(u64::MAX as u128) as u64,
        );
        if timed_out {
            self.inc("gateway_db_pool_acquire_timeout_total", labels);
        }
        let mut histograms = self
            .database_acquire_duration
            .lock()
            .expect("gateway database acquire durations");
        let histogram = histograms.entry((role, caller)).or_default();
        let seconds = elapsed.as_secs_f64();
        histogram.count = histogram.count.saturating_add(1);
        histogram.sum_seconds += seconds;
        for (index, bound) in ADMISSION_DURATION_BUCKETS_SECONDS.iter().enumerate() {
            if seconds <= *bound {
                histogram.buckets[index] = histogram.buckets[index].saturating_add(1);
            }
        }
        histogram.buckets[7] = histogram.buckets[7].saturating_add(1);
    }

    pub async fn acquire_database_connection(
        &self,
        role: DatabasePoolRole,
        caller: DatabasePoolCaller,
        pool: &PgPool,
    ) -> Result<ObservedDatabaseConnection, sqlx::Error> {
        let started = tokio::time::Instant::now();
        let size_before = pool.size();
        let idle_before = pool.num_idle();
        let result = pool.acquire().await;
        if started.elapsed() >= Duration::from_millis(100) {
            tracing::warn!(pool = role.as_str(), caller = caller.as_str(), elapsed_us = %started.elapsed().as_micros(), size_before, idle_before, size_after = pool.size(), idle_after = pool.num_idle(), code = "DATABASE_ACQUIRE_SLOW", "database pool acquisition was slow");
        }
        self.observe_database_acquire(
            role,
            caller,
            started.elapsed(),
            matches!(&result, Err(sqlx::Error::PoolTimedOut)),
        );
        let connection = result?;
        self.adjust_gauge(
            "gateway_db_pool_observed_holders",
            &[("pool", role.as_str()), ("caller", caller.as_str())],
            1,
        );
        Ok(ObservedDatabaseConnection {
            connection,
            metrics: self.clone(),
            role,
            caller,
            acquired_at: tokio::time::Instant::now(),
        })
    }

    pub fn inc_rate_limit_rejection(&self, route: &str) {
        self.inc("gateway_rate_limit_rejections_total", &[("route", route)]);
    }

    /// Count a request refused immediately because its route class is at its
    /// per-process concurrency ceiling.
    pub fn inc_admission_in_flight_rejection(&self, class: RouteClass) {
        self.inc(
            "gateway_admission_in_flight_rejections_total",
            &[("class", class.as_str())],
        );
    }

    /// Count one admission decision by route class and outcome.
    ///
    /// Outcomes separate a spent client budget (`rejected_quota`) from full
    /// capacity (`admitted_overflow`, `rejected_cardinality`) and from a
    /// store failure (`admitted_fallback`, `rejected_fallback_exhausted`,
    /// `rejected_store_unavailable`).
    pub fn inc_admission_decision(&self, class: &str, scope: AdmissionScope, outcome: &str) {
        self.inc(
            "gateway_admission_decisions_total",
            &[
                ("class", class),
                ("scope", scope.as_str()),
                ("outcome", outcome),
            ],
        );
    }

    /// Count a quota/cardinality decision that was observed but not enforced.
    pub fn inc_admission_shadow_pass(&self, class: &str, scope: AdmissionScope, decision: &str) {
        self.inc(
            "gateway_admission_shadow_passes_total",
            &[
                ("class", class),
                ("scope", scope.as_str()),
                ("decision", decision),
            ],
        );
    }

    /// Observe total time spent deciding whether a request may proceed.
    ///
    /// The bounded route-class label makes this safe to expose on every request;
    /// the observation includes shared-store pool wait and database lock/query
    /// time when the PostgreSQL adapter is active.
    pub fn observe_admission_duration(
        &self,
        class: RouteClass,
        scope: AdmissionScope,
        duration: Duration,
    ) {
        let seconds = duration.as_secs_f64();
        let mut histograms = self
            .admission_duration
            .lock()
            .expect("gateway admission duration metrics");
        let histogram = histograms.entry((class, scope)).or_default();
        for (index, bound) in ADMISSION_DURATION_BUCKETS_SECONDS.iter().enumerate() {
            if seconds <= *bound {
                histogram.buckets[index] = histogram.buckets[index].saturating_add(1);
            }
        }
        histogram.buckets[7] = histogram.buckets[7].saturating_add(1);
        histogram.count = histogram.count.saturating_add(1);
        histogram.sum_seconds += seconds;
    }

    /// Observe only PostgreSQL advisory-lock wait returned by the durable
    /// admission function; local admission and store errors have no sample.
    pub fn observe_admission_lock_wait(
        &self,
        class: RouteClass,
        scope: AdmissionScope,
        duration: Duration,
    ) {
        let seconds = duration.as_secs_f64();
        let mut histograms = self
            .admission_lock_wait
            .lock()
            .expect("gateway admission lock-wait metrics");
        let histogram = histograms.entry((class, scope)).or_default();
        for (index, bound) in ADMISSION_DURATION_BUCKETS_SECONDS.iter().enumerate() {
            if seconds <= *bound {
                histogram.buckets[index] = histogram.buckets[index].saturating_add(1);
            }
        }
        histogram.buckets[7] = histogram.buckets[7].saturating_add(1);
        histogram.count = histogram.count.saturating_add(1);
        histogram.sum_seconds += seconds;
    }

    pub fn inc_request_timeout(&self, route: &str, downstream: &str) {
        self.inc(
            "gateway_request_timeouts_total",
            &[("route", route), ("downstream", downstream)],
        );
    }

    pub(crate) fn edge_deadline_metrics(&self) -> EdgeDeadlineMetrics {
        self.edge_deadlines.clone()
    }

    pub fn inc_jwt_verification_failure(&self, reason: &str) {
        self.inc(
            "gateway_jwt_verification_failures_total",
            &[("reason", reason)],
        );
    }

    /// Count a signing-key refresh by what started it and how it ended.
    pub fn inc_signing_key_refresh(&self, trigger: &str, outcome: &str) {
        self.inc(
            "gateway_jwt_key_refreshes_total",
            &[("trigger", trigger), ("outcome", outcome)],
        );
    }

    /// Record a successful key refresh: how many keys are installed, and when.
    ///
    /// The time is a gauge rather than a counter so staleness can be alerted on
    /// directly: the age is `time()` minus this value. It keeps growing while
    /// refreshes fail and also when no refresh runs at all, which a failure
    /// counter cannot show.
    pub fn set_signing_keys_installed(&self, key_count: usize, refreshed_at_unix_seconds: i64) {
        self.set_gauge(
            "gateway_jwt_verification_keys",
            &[],
            i64::try_from(key_count).unwrap_or(i64::MAX),
        );
        self.set_gauge(
            "gateway_jwt_keys_last_refresh_timestamp_seconds",
            &[],
            refreshed_at_unix_seconds,
        );
    }

    /// Record the configured refresh interval.
    ///
    /// Exported so an alert can judge staleness relative to the interval this
    /// Gateway actually uses, and can tell a Gateway with refresh on that has
    /// never refreshed from one with refresh off.
    pub fn set_signing_key_refresh_interval(&self, interval_seconds: u64) {
        self.set_gauge(
            "gateway_jwt_keys_refresh_interval_seconds",
            &[],
            i64::try_from(interval_seconds).unwrap_or(i64::MAX),
        );
    }

    pub fn inc_admin_audit_event(&self, action: &str, outcome: &str) {
        self.inc(
            "admin_audit_events_total",
            &[("action", action), ("outcome", outcome)],
        );
    }

    /// Count one high-risk session check by action and outcome.
    ///
    /// Outcomes: `allowed`, `allowed_cached`, `session_ended`,
    /// `role_revoked`, `no_session_claim`, and `unavailable` (the issuer
    /// could not answer, so the request was refused).
    pub fn inc_high_risk_session_check(&self, action: &str, outcome: &str) {
        self.inc(
            "gateway_high_risk_session_checks_total",
            &[("action", action), ("outcome", outcome)],
        );
    }

    /// Records a telemetry record shed by the background writer's queue.
    ///
    /// Exported because a drop is otherwise invisible: the request still
    /// succeeds, so nothing in a response or a log distinguishes a queue that is
    /// keeping up from one that is discarding records.
    pub fn inc_telemetry_dropped(&self) {
        self.inc("gateway_telemetry_records_dropped_total", &[]);
    }

    /// Count one failed or timed-out telemetry/audit/rate-limit retention pass.
    pub fn inc_retention_purge_failure(&self) {
        self.inc("gateway_retention_purge_failures_total", &[]);
    }

    /// Set the reconciliation backlog gauges.
    ///
    /// A gauge rather than a counter: the backlog is a level that rises and
    /// falls, so incrementing would report the number of times it was observed
    /// instead of how much work is outstanding.
    pub fn set_reconciliation_backlog(&self, outstanding: i64, oldest_age_seconds: i64) {
        self.set_gauge("gateway_reconciliation_backlog", &[], outstanding);
        self.set_gauge(
            "gateway_reconciliation_oldest_age_seconds",
            &[],
            oldest_age_seconds,
        );
    }

    /// Set aggregate content moderation outbox health without report labels.
    pub fn set_content_moderation_backlog(
        &self,
        pending: i64,
        oldest_age_seconds: i64,
        target_unavailable: i64,
        retry_exhausted: i64,
        last_success_timestamp_seconds: i64,
    ) {
        self.set_gauge(
            "gateway_moderation_decision_outbox_pending",
            &[],
            pending.max(0),
        );
        self.set_gauge(
            "gateway_moderation_decision_outbox_oldest_age_seconds",
            &[],
            oldest_age_seconds.max(0),
        );
        self.set_gauge(
            "gateway_moderation_decision_outbox_target_unavailable",
            &[],
            target_unavailable.max(0),
        );
        self.set_gauge(
            "gateway_moderation_decision_outbox_retry_exhausted",
            &[],
            retry_exhausted.max(0),
        );
        self.set_gauge(
            "gateway_moderation_decision_outbox_metrics_last_success_timestamp_seconds",
            &[],
            last_success_timestamp_seconds.max(0),
        );
    }

    pub fn render_prometheus(&self) -> String {
        let counters = self.counters.lock().expect("gateway metrics");
        let mut output = String::new();
        for (sample, value) in counters.iter() {
            output.push_str(sample);
            output.push(' ');
            output.push_str(&value.to_string());
            output.push('\n');
        }
        drop(counters);
        // Gauges are rendered after the counters and carry no `_total` suffix, so
        // a reader can tell a level from a running count without external context.
        let gauges = self.gauges.lock().expect("gateway gauges");
        for (sample, value) in gauges.iter() {
            output.push_str(sample);
            output.push(' ');
            output.push_str(&value.to_string());
            output.push('\n');
        }
        drop(gauges);
        for (role, pool) in self
            .database_pools
            .lock()
            .expect("gateway database pools")
            .iter()
        {
            let size = pool.size();
            let idle = pool.num_idle();
            output.push_str(&format!(
                "gateway_db_pool_connections{{pool=\"{}\"}} {}\ngateway_db_pool_idle_connections{{pool=\"{}\"}} {}\n",
                role.as_str(), size, role.as_str(), idle
            ));
        }
        for ((role, caller), histogram) in self
            .database_acquire_duration
            .lock()
            .expect("gateway database acquire durations")
            .iter()
        {
            for (index, bound) in ADMISSION_DURATION_BUCKET_LABELS.iter().enumerate() {
                output.push_str(&format!("gateway_db_pool_acquire_wait_seconds_bucket{{pool=\"{}\",caller=\"{}\",le=\"{}\"}} {}\n", role.as_str(), caller.as_str(), bound, histogram.buckets[index]));
            }
            output.push_str(&format!("gateway_db_pool_acquire_wait_seconds_sum{{pool=\"{}\",caller=\"{}\"}} {:.9}\ngateway_db_pool_acquire_wait_seconds_count{{pool=\"{}\",caller=\"{}\"}} {}\n", role.as_str(), caller.as_str(), histogram.sum_seconds, role.as_str(), caller.as_str(), histogram.count));
        }
        let admission_duration = self
            .admission_duration
            .lock()
            .expect("gateway admission duration metrics");
        for ((class, scope), histogram) in admission_duration.iter() {
            for (index, bound) in ADMISSION_DURATION_BUCKET_LABELS.iter().enumerate() {
                output.push_str(&format!(
                    "gateway_admission_duration_seconds_bucket{{class=\"{}\",scope=\"{}\",le=\"{}\"}} {}\n",
                    class.as_str(),
                    scope.as_str(),
                    bound,
                    histogram.buckets[index]
                ));
            }
            output.push_str(&format!(
                "gateway_admission_duration_seconds_sum{{class=\"{}\",scope=\"{}\"}} {:.9}\n",
                class.as_str(),
                scope.as_str(),
                histogram.sum_seconds
            ));
            output.push_str(&format!(
                "gateway_admission_duration_seconds_count{{class=\"{}\",scope=\"{}\"}} {}\n",
                class.as_str(),
                scope.as_str(),
                histogram.count
            ));
        }
        drop(admission_duration);
        let admission_lock_wait = self
            .admission_lock_wait
            .lock()
            .expect("gateway admission lock-wait metrics");
        for ((class, scope), histogram) in admission_lock_wait.iter() {
            for (index, bound) in ADMISSION_DURATION_BUCKET_LABELS.iter().enumerate() {
                output.push_str(&format!(
                    "gateway_admission_lock_wait_seconds_bucket{{class=\"{}\",scope=\"{}\",le=\"{}\"}} {}\n",
                    class.as_str(),
                    scope.as_str(),
                    bound,
                    histogram.buckets[index]
                ));
            }
            output.push_str(&format!(
                "gateway_admission_lock_wait_seconds_sum{{class=\"{}\",scope=\"{}\"}} {:.9}\n",
                class.as_str(),
                scope.as_str(),
                histogram.sum_seconds
            ));
            output.push_str(&format!(
                "gateway_admission_lock_wait_seconds_count{{class=\"{}\",scope=\"{}\"}} {}\n",
                class.as_str(),
                scope.as_str(),
                histogram.count
            ));
        }
        drop(admission_lock_wait);
        output.push_str(&self.edge_deadlines.render_prometheus());
        output
    }

    fn inc(&self, name: &str, labels: &[(&str, &str)]) {
        self.add(name, labels, 1);
    }

    fn add(&self, name: &str, labels: &[(&str, &str)], amount: u64) {
        let sample = format!("{name}{{{}}}", label_set(labels));
        let mut counters = self.counters.lock().expect("gateway metrics");
        // Reserve one bounded series for reporting dropped label combinations.
        let sample = if counters.len() >= Self::MAX_SERIES - 1 && !counters.contains_key(&sample) {
            "gateway_metrics_dropped_series_total{}".to_owned()
        } else {
            sample
        };
        let count = counters.entry(sample).or_insert(0);
        *count = count.saturating_add(amount);
    }

    /// Add `delta` to a gauge. The value is deliberately not clamped: a
    /// negative holder count exposes a double release instead of hiding it.
    fn adjust_gauge(&self, name: &str, labels: &[(&str, &str)], delta: i64) {
        let sample = format!("{name}{{{}}}", label_set(labels));
        let mut gauges = self.gauges.lock().expect("gateway gauges");
        let count = gauges.entry(sample).or_default();
        *count = count.saturating_add(delta);
    }

    /// Set a gauge, replacing any previous value for the same sample.
    fn set_gauge(&self, name: &str, labels: &[(&str, &str)], value: i64) {
        let sample = format!("{name}{{{}}}", label_set(labels));
        let mut gauges = self.gauges.lock().expect("gateway gauges");
        // The gauge series are bounded independently from counters. Report an
        // overflow through the counter map so the `_total` metric remains a
        // counter and is never emitted twice as both a counter and a gauge.
        if gauges.len() >= Self::MAX_SERIES && !gauges.contains_key(&sample) {
            drop(gauges);
            self.inc("gateway_metrics_dropped_series_total", &[]);
            return;
        }
        gauges.insert(sample, value);
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

/// Admission settings other than the read budget, which is
/// `GATEWAY_RATE_LIMIT_PER_MINUTE`.
fn admission_from_env() -> anyhow::Result<AdmissionPolicies> {
    let mut admission = AdmissionPolicies::default()
        .with_overrides(&env_or("GATEWAY_ADMISSION_BUDGETS", ""))
        .map_err(|error| anyhow::anyhow!("GATEWAY_ADMISSION_BUDGETS: {error}"))?;
    admission = admission
        .with_identity_overrides(&env_or("GATEWAY_ADMISSION_IDENTITY_BUDGETS", ""))
        .map_err(|error| anyhow::anyhow!("GATEWAY_ADMISSION_IDENTITY_BUDGETS: {error}"))?;
    admission = admission
        .with_principal_overrides(&env_or("GATEWAY_ADMISSION_PRINCIPAL_BUDGETS", ""))
        .map_err(|error| anyhow::anyhow!("GATEWAY_ADMISSION_PRINCIPAL_BUDGETS: {error}"))?;
    admission = admission
        .with_max_in_flight_overrides(&env_or("GATEWAY_ADMISSION_MAX_IN_FLIGHT", ""))
        .map_err(|error| anyhow::anyhow!("GATEWAY_ADMISSION_MAX_IN_FLIGHT: {error}"))?;
    admission.mode = AdmissionMode::parse(&env_or("GATEWAY_ADMISSION_MODE", "enforce"))
        .ok_or_else(|| {
            anyhow::anyhow!("GATEWAY_ADMISSION_MODE must be either 'enforce' or 'shadow'")
        })?;
    admission.overflow_factor = parse_u32_setting(
        "GATEWAY_ADMISSION_OVERFLOW_FACTOR",
        &env_or(
            "GATEWAY_ADMISSION_OVERFLOW_FACTOR",
            &crate::domain::admission::DEFAULT_OVERFLOW_FACTOR.to_string(),
        ),
        0,
        100,
    )?;
    admission.fallback_per_second = parse_u32_setting(
        "GATEWAY_ADMISSION_FALLBACK_PER_SECOND",
        &env_or(
            "GATEWAY_ADMISSION_FALLBACK_PER_SECOND",
            &crate::domain::admission::DEFAULT_FALLBACK_PER_SECOND.to_string(),
        ),
        0,
        10_000,
    )?;
    // Narrower than /32 would merge unrelated providers into one client.
    admission.v6_prefix_length = u8::try_from(parse_u32_setting(
        "GATEWAY_RATE_LIMIT_V6_PREFIX_LENGTH",
        &env_or(
            "GATEWAY_RATE_LIMIT_V6_PREFIX_LENGTH",
            &crate::domain::admission::DEFAULT_V6_PREFIX_LENGTH.to_string(),
        ),
        32,
        128,
    )?)?;
    Ok(admission)
}

pub async fn connect_pool(config: &AppConfig) -> anyhow::Result<PgPool> {
    let options = PgConnectOptions::from_str(&config.database_url)
        .context("Gateway startup: parse main database options")?
        .application_name("gateway-main");
    let options = migration_runtime::serving_options(options, config.database_migration_mode);
    let pool = PgPoolOptions::new()
        .max_connections(config.database_pool_max_connections)
        .connect_with(options)
        .await
        .context("Gateway startup: connect main database pool")?;

    let migrations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    migration_runtime::prepare_database(
        &pool,
        migrations.as_path(),
        config.database_migration_mode,
        &[migration_runtime::SchemaProbe {
            name: "GATEWAY required schema objects",
            sql: include_str!("schema-probe.sql"),
        }],
    )
    .await
    .context("Gateway startup: prepare main database schema")?;
    Ok(pool)
}

/// Connect the dedicated production admission pool after main-schema preparation.
/// This preserves serving-mode schema selection and the existing bounded store
/// budget. It does not apply migrations or replace the main pool's verification.
pub async fn connect_quota_pool(config: &AppConfig) -> anyhow::Result<PgPool> {
    let options = PgConnectOptions::from_str(&config.database_url)
        .context("Gateway startup: parse quota database options")?
        .application_name("gateway-quota")
        .options([("statement_timeout", "200"), ("lock_timeout", "150")]);
    let options = migration_runtime::serving_options(options, config.database_migration_mode);
    PgPoolOptions::new()
        .max_connections(4)
        .acquire_timeout(Duration::from_millis(200))
        .connect_with(options)
        .await
        .context("Gateway startup: connect quota database pool")
}

/// The guarded internal exchange service for the Gateway's gRPC listener.
///
/// Owner evidence is fetched independently from Post over the Gateway to Post
/// edge, so the presenter never supplies the rows the issuer trusts. Only Post
/// may call the service, and its methods act for no user directly, so it is
/// registered without delegation verification.
pub fn authority_exchange_service(
    config: &AppConfig,
    metrics: &GatewayMetrics,
) -> anyhow::Result<
    platform::grpc_method_policy::GuardedService<
        crate::adapters::r#in::grpc::authority_exchange::proto::authorization_exchange_service_server::AuthorizationExchangeServiceServer<
            crate::adapters::r#in::grpc::authority_exchange::GatewayAuthorizationExchangeGrpcServer,
        >,
    >,
>{
    use crate::adapters::r#in::grpc::authority_exchange::{
        proto::authorization_exchange_service_server::AuthorizationExchangeServiceServer,
        GatewayAuthorizationExchangeGrpcServer, AUTHORIZATION_EXCHANGE_POLICY,
    };
    let owner =
        crate::adapters::out::owner_resource_client::GrpcPostResourceAuthorityClient::connect(
            &config.post_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
            metrics.edge_deadline_metrics(),
        )?;
    let runtime = crate::adapters::out::authority_exchange::DelegationExchangeRuntime::new(
        config.authority_exchange.clone(),
        Arc::new(owner),
    );
    platform::grpc_method_policy::guard_without_delegation(
        &AUTHORIZATION_EXCHANGE_POLICY,
        AuthorizationExchangeServiceServer::new(GatewayAuthorizationExchangeGrpcServer::new(
            Arc::new(runtime),
        )),
        config.caller_registry.clone(),
    )
}

pub async fn build_state(config: &AppConfig, pool: PgPool) -> anyhow::Result<AppState> {
    let metrics = GatewayMetrics::default();
    let auth_client = Arc::new(
        GrpcAuthClient::connect_with_metrics(
            config.auth_grpc_endpoint.clone(),
            platform::grpc_auth::Workload::Gateway,
            &config.outbound_identity,
            config.tls_material.as_ref(),
            config.actor_delegation_signer.clone(),
            metrics.clone(),
        )
        .await?,
    );
    let verification_client = Arc::new(GrpcVerificationClient::new(
        auth_client.as_ref().clone(),
        Duration::from_millis(config.request_timeout_ms),
    )?);
    let profile_client = Arc::new(
        GrpcProfileClient::connect_with_metrics(
            config.profile_grpc_endpoint.clone(),
            platform::grpc_auth::Workload::Gateway,
            &config.outbound_identity,
            config.tls_material.as_ref(),
            config.actor_delegation_signer.clone(),
            metrics.clone(),
        )
        .await?,
    );
    let concert_identity = crate::adapters::out::concert_identity::ConcertIdentityReader::new(
        auth_client.as_ref().clone(),
        profile_client.as_ref().clone(),
    );
    let friend_client = Arc::new(
        GrpcFriendClient::connect_with_metrics(
            config.friend_grpc_endpoint.clone(),
            platform::grpc_auth::Workload::Gateway,
            &config.outbound_identity,
            config.tls_material.as_ref(),
            config.actor_delegation_signer.clone(),
            metrics.clone(),
        )
        .await?,
    ) as Arc<dyn FriendClientPort>;
    let notification_client = Arc::new(
        GrpcNotificationClient::connect_with_metrics(
            config.notification_grpc_endpoint.clone(),
            platform::grpc_auth::Workload::Gateway,
            &config.outbound_identity,
            config.tls_material.as_ref(),
            config.actor_delegation_signer.clone(),
            metrics.clone(),
        )
        .await?,
    ) as Arc<dyn NotificationClientPort>;
    let post_client = Arc::new(
        GrpcPostClient::connect_with_metrics(
            config.post_grpc_endpoint.clone(),
            platform::grpc_auth::Workload::Gateway,
            &config.outbound_identity,
            config.tls_material.as_ref(),
            config.actor_delegation_signer.clone(),
            metrics.clone(),
        )
        .await?,
    ) as Arc<dyn PostClientPort>;

    let moderation_repository: Arc<dyn ModerationRepositoryPort> =
        Arc::new(PgModerationRepository::new(pool.clone()));
    let mut state = build_state_with_clients_and_moderation_with_metrics(
        config,
        pool,
        auth_client.clone(),
        profile_client,
        friend_client,
        notification_client,
        post_client,
        moderation_repository,
        metrics,
    )
    .await?;
    state.concert_identity = Some(concert_identity);
    state.concert_client = config
        .concert_grpc_endpoint
        .as_ref()
        .map(|endpoint| {
            crate::adapters::out::concert_client::GrpcConcertClient::connect(
                endpoint,
                &config.outbound_identity,
                config.tls_material.as_ref(),
                config.actor_delegation_signer.clone(),
            )
        })
        .transpose()?;
    state.concert_world_client = config
        .concert_grpc_endpoint
        .as_ref()
        .map(|endpoint| {
            crate::adapters::out::concert_world::GrpcConcertWorldClient::connect(
                endpoint,
                &config.outbound_identity,
                config.tls_material.as_ref(),
                config.actor_delegation_signer.clone(),
            )
        })
        .transpose()?;
    state.concert_product_client = config
        .concert_grpc_endpoint
        .as_ref()
        .map(|endpoint| {
            crate::adapters::out::concert_product::GrpcConcertProductClient::connect(
                endpoint,
                &config.outbound_identity,
                config.tls_material.as_ref(),
                config.actor_delegation_signer.clone(),
            )
        })
        .transpose()?;
    state.concert_participant_client = config
        .concert_release_digest
        .as_ref()
        .map(|digest| {
            crate::adapters::out::concert_participant::GrpcConcertParticipantClient::connect(
                config
                    .concert_grpc_endpoint
                    .as_deref()
                    .ok_or_else(|| anyhow::anyhow!("Concert endpoint required"))?,
                &config.outbound_identity,
                config.tls_material.as_ref(),
                config.actor_delegation_signer.clone(),
                digest,
            )
            .map(Arc::new)
        })
        .transpose()?;
    state.auth_service = state
        .auth_service
        .with_verification_client(verification_client);
    if config.jwt_keys_refresh_seconds > 0 {
        // Keys come from the issuer over the same authenticated channel as the
        // auth calls. The loop that refreshes them on a schedule is started by
        // the executable; request handling may ask for an early refresh.
        let source: Arc<dyn SigningKeySourcePort> = auth_client;
        state.signing_keys = Some(SigningKeyRefresher::new(
            state.jwt_verifier.clone(),
            source,
            RefreshPolicy {
                interval: Duration::from_secs(config.jwt_keys_refresh_seconds),
                unknown_kid_min_interval: Duration::from_secs(config.jwt_keys_min_refresh_seconds),
            },
            Some(state.metrics.clone()),
        ));
    }
    Ok(state)
}

pub async fn build_state_with_clients(
    config: &AppConfig,
    pool: PgPool,
    auth_client: Arc<dyn AuthClientPort>,
    profile_client: Arc<dyn ProfileClientPort>,
    friend_client: Arc<dyn FriendClientPort>,
    notification_client: Arc<dyn NotificationClientPort>,
    post_client: Arc<dyn PostClientPort>,
) -> anyhow::Result<AppState> {
    let moderation_repository: Arc<dyn ModerationRepositoryPort> =
        Arc::new(PgModerationRepository::new(pool.clone()));
    build_state_with_clients_and_moderation(
        config,
        pool,
        auth_client,
        profile_client,
        friend_client,
        notification_client,
        post_client,
        moderation_repository,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn build_state_with_clients_and_moderation(
    config: &AppConfig,
    pool: PgPool,
    auth_client: Arc<dyn AuthClientPort>,
    profile_client: Arc<dyn ProfileClientPort>,
    friend_client: Arc<dyn FriendClientPort>,
    notification_client: Arc<dyn NotificationClientPort>,
    post_client: Arc<dyn PostClientPort>,
    moderation_repository: Arc<dyn ModerationRepositoryPort>,
) -> anyhow::Result<AppState> {
    build_state_with_clients_and_moderation_with_metrics(
        config,
        pool,
        auth_client,
        profile_client,
        friend_client,
        notification_client,
        post_client,
        moderation_repository,
        GatewayMetrics::default(),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
async fn build_state_with_clients_and_moderation_with_metrics(
    config: &AppConfig,
    pool: PgPool,
    auth_client: Arc<dyn AuthClientPort>,
    profile_client: Arc<dyn ProfileClientPort>,
    friend_client: Arc<dyn FriendClientPort>,
    notification_client: Arc<dyn NotificationClientPort>,
    post_client: Arc<dyn PostClientPort>,
    moderation_repository: Arc<dyn ModerationRepositoryPort>,
    metrics: GatewayMetrics,
) -> anyhow::Result<AppState> {
    if config.seed_enabled {
        seed_governance(&pool).await?;
    }

    let route_contract_repository = Arc::new(PgRouteContractRepository::new(pool.clone()));
    let policy_repository = Arc::new(PgPolicyRepository::new(pool.clone()));
    let rollout_repository = Arc::new(PgRolloutRepository::new(pool.clone()));
    let client_profile_repository = Arc::new(PgClientProfileRepository::new(pool.clone()));
    let aggregation_recipe_repository = Arc::new(PgAggregationRecipeRepository::new(pool.clone()));
    let audit_pool = pool.clone();
    let telemetry_repository = Arc::new(PgTelemetryRepository::new(pool));
    let response_cache = Arc::new(InMemoryResponseCache::new());
    let jwt_verifier = if config.jwt_jwks_json.trim().is_empty() {
        // The keys arrive from the issuer after startup. Until they do, every
        // token names an unknown key and readiness reports the missing keys.
        JwtVerifier::without_keys(&config.jwt_issuer, &config.jwt_audience)?
    } else {
        JwtVerifier::from_jwks_json(
            &config.jwt_issuer,
            &config.jwt_audience,
            &config.jwt_jwks_json,
        )
        .map_err(|error| {
            anyhow::anyhow!("GATEWAY_JWT_JWKS_JSON is not a usable key set: {error}")
        })?
    };
    let async_operation_cipher = config
        .async_operation_keyring
        .as_deref()
        .map(PayloadCipher::from_hex_keyring)
        .transpose()
        .map_err(|_| anyhow::anyhow!("GATEWAY_ASYNC_OPERATION_KEYRING is invalid"))?;
    let runtime = Arc::new(SystemGatewayRuntime);
    let search_cursor_codec = SearchCursorCodec::new(
        Arc::from(config.search_cursor_hmac_key.as_bytes()),
        config.search_cursor_ttl_seconds,
        config.search_cursor_legacy_accept_until,
        runtime.clone(),
    )?;

    Ok(AppState {
        concert_identity: None,
        concert_client: None,
        concert_product_client: None,
        concert_world_client: None,
        concert_participant_client: None,
        high_risk_sessions: HighRiskSessionGuard::over_auth_client(
            auth_client.clone(),
            Duration::from_secs(config.high_risk_session_freshness_seconds),
            runtime.clone(),
        ),
        auth_service: AuthGatewayService::new(auth_client),
        oauth_exposure: config.oauth_exposure.clone(),
        enrollment_policy: Arc::new(crate::adapters::out::enrollment_policy::GrpcEnrollmentClient::connect(
            &config.auth_grpc_endpoint, &config.outbound_identity, config.tls_material.as_ref(),
            config.actor_delegation_signer.clone(), metrics.clone(),
        )?),
        profile_service: ProfileGatewayService::new(
            profile_client,
            friend_client.clone(),
            post_client.clone(),
            search_cursor_codec.clone(),
        ),
        friend_service: FriendGatewayService::new(friend_client),
        moderation_service: ModerationService::new(
            moderation_repository,
            post_client.clone(),
            Arc::new(SystemModerationRuntime),
        ),
        notification_service: NotificationGatewayService::new(
            notification_client,
            post_client.clone(),
            runtime.clone(),
        )
        .with_projection_budget(Duration::from_millis(
            config.request_timeout_ms.saturating_sub(100).max(1),
        )),
        post_service: PostGatewayService::new(post_client, search_cursor_codec.clone()),
        search_cursor_codec,
        aggregation_service: AggregationService::new(
            aggregation_recipe_repository,
            response_cache,
            Duration::from_secs(config.aggregation_cache_ttl_seconds),
        ),
        route_contract_service: RouteContractService::new(route_contract_repository),
        policy_service: PolicyService::new(policy_repository, rollout_repository, runtime.clone()),
        client_profile_service: ClientProfileService::new(client_profile_repository),
        // The request path hands records to a bounded queue; a background writer
        // performs the insert. See `telemetry_queue` for why the write does not run
        // inline, and for how shedding is reported.
        telemetry_service: QueuedTelemetry::with_capacity(
            telemetry_repository,
            config.telemetry_queue_capacity as usize,
        ),
        refresh_cookie_domain: config.refresh_cookie_domain.clone(),
        refresh_cookie_secure: config.refresh_cookie_secure,
        refresh_cookie_max_age_seconds: config.refresh_cookie_max_age_seconds,
        jwt_verifier,
        browser_auth_origins: config.cors_allowed_origins.clone(),
        signing_keys: None,
        runtime_mode: config.runtime_mode,
        developer_portal_links: config.developer_portal_links.clone(),
        rate_limiter: InMemoryRateLimiter::production(
            config
                .admission
                .clone()
                .with_budget(RouteClass::Read, config.rate_limit_per_minute),
        ),
        admission_hmac_key: Arc::from(config.admission_hmac_key.as_bytes()),
        request_timeout: Duration::from_millis(config.request_timeout_ms),
        journal_client_timeout: Duration::from_millis(config.journal_client_timeout_ms),
        async_operation_cipher: async_operation_cipher
            .map(|cipher| Arc::new(cipher) as Arc<dyn crate::application::ports::QueuePayloadPort>),
        async_operation_queue: async_operation_queue_repository(&audit_pool, None),
        operation_journal_service: operation_journal_service(&audit_pool),
        journal_dispatch_repository: journal_dispatch_repository(
            &audit_pool,
            Some(metrics.clone()),
        ),
        legacy_audit_repository: Arc::new(PgLegacyAuditRepository::new(audit_pool.clone())),
        journal: journal_runtime_ports(&audit_pool, &metrics),
        metrics,
    })
}

pub fn build_cors_layer(config: &AppConfig) -> CorsLayer {
    let origins = config
        .cors_allowed_origins
        .iter()
        .filter_map(|value| HeaderValue::from_str(value).ok())
        .collect::<Vec<_>>();
    // Credentialed CORS cannot use wildcard headers or origins. An empty list
    // deliberately denies cross-origin access instead of falling back to '*'.
    CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::OPTIONS,
        ])
        .allow_headers([
            ACCEPT,
            AUTHORIZATION,
            CONTENT_TYPE,
            HeaderName::from_static("x-client-id"),
            HeaderName::from_static("x-request-id"),
            HeaderName::from_static("x-correlation-id"),
            HeaderName::from_static("idempotency-key"),
        ])
        .expose_headers([HeaderName::from_static("x-correlation-id")])
        .allow_credentials(true)
        .allow_origin(origins)
}

async fn seed_governance(pool: &PgPool) -> anyhow::Result<()> {
    sqlx::query(
        r#"
        insert into policy_sets (
            id, name, rate_limit, quota, timeout_ms, retry_policy, circuit_breaker_policy, cache_policy
        )
        values ('default', 'default', '600/min', null, 1000, 'retry-2', 'enabled', 'default')
        on conflict (id) do nothing
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        insert into route_contracts (id, route_key, owner, sla_target, status, description)
        values (
            'feed-summary-contract',
            'feed-summary',
            'gateway-platform',
            'p95<500ms',
            'active',
            'Frontend summary aggregation contract'
        )
        on conflict (id) do nothing
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        insert into route_contract_versions (
            contract_id, version, status, deprecation_date, response_schema_ref
        )
        values ('feed-summary-contract', 'v1', 'active', null, '#/components/schemas/AggregationResponse')
        on conflict (contract_id, version) do nothing
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        insert into aggregation_recipes (
            id, route_key, downstream_calls, join_strategy, timeout_ms, cache_policy
        )
        values (
            'feed-summary',
            'feed-summary',
            '[{"service":"post","path":"/grpc/post","fields":["id","content"]}]',
            'merge',
            1000,
            'default'
        )
        on conflict (id) do nothing
        "#,
    )
    .execute(pool)
    .await?;

    sqlx::query(
        r#"
        insert into client_profiles (id, client_type, allowed_contracts, policy_set_id)
        values ('web-default', 'web', '["feed-summary"]', 'default')
        on conflict (id) do nothing
        "#,
    )
    .execute(pool)
    .await?;

    Ok(())
}

/// Explicit repository composition shared by background and compatibility consumers.
pub fn operation_journal_service(pool: &PgPool) -> OperationJournalService {
    OperationJournalService::new(Arc::new(PgOperationJournalRepository::new(pool.clone())))
}

/// Preserve the caller's pool and optional observed acquisition lifecycle.
pub fn journal_dispatch_repository(
    pool: &PgPool,
    metrics: Option<GatewayMetrics>,
) -> Arc<dyn crate::application::ports::JournalDispatchRepositoryPort> {
    Arc::new(PgJournalDispatchRepository::new(pool.clone(), metrics))
}
pub fn journal_retention_repository(
    pool: &PgPool,
) -> Arc<dyn crate::application::ports::JournalRetentionRepositoryPort> {
    Arc::new(PgJournalRetentionRepository::new(pool.clone()))
}
pub fn owner_receipt_journal_repository(
    pool: &PgPool,
) -> Arc<dyn crate::application::ports::OwnerReceiptJournalRepositoryPort> {
    Arc::new(PgOwnerReceiptJournalRepository::new(pool.clone()))
}

/// Queue claim sessions retain the supplied background pool and optional metrics.
pub fn async_operation_queue_repository(
    pool: &PgPool,
    metrics: Option<GatewayMetrics>,
) -> Arc<dyn crate::application::ports::AsyncOperationQueueRepositoryPort> {
    Arc::new(PgAsyncOperationQueueRepository::new(pool.clone(), metrics))
}
pub fn journal_maintenance_repository(
    pool: &PgPool,
) -> Arc<dyn crate::application::ports::JournalMaintenanceRepositoryPort> {
    Arc::new(PgJournalMaintenanceRepository::new(pool.clone()))
}

/// Bind every journal, receipt and queue capability to one pool. The observed
/// ports record connection acquisition in `metrics`; the others do not, which
/// matches the callers that previously composed them per call.
pub fn journal_runtime_ports(
    pool: &PgPool,
    metrics: &GatewayMetrics,
) -> crate::application::journal_runtime_ports::JournalRuntimePorts {
    crate::application::journal_runtime_ports::JournalRuntimePorts {
        dispatch: journal_dispatch_repository(pool, None),
        observed_dispatch: journal_dispatch_repository(pool, Some(metrics.clone())),
        retention: journal_retention_repository(pool),
        owner_receipts: owner_receipt_journal_repository(pool),
        maintenance: journal_maintenance_repository(pool),
        queue: async_operation_queue_repository(pool, None),
        observed_queue: async_operation_queue_repository(pool, Some(metrics.clone())),
        operations: operation_journal_service(pool),
    }
}

#[cfg(test)]
mod hardening_tests;
