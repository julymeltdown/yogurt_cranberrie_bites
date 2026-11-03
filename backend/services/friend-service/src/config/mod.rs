use crate::adapters::out::clock::SystemPeopleRecommendationClock;
use crate::adapters::out::grpc::profile_privacy_client::GrpcProfilePrivacyClient;
use crate::adapters::out::metrics::recommendation_observer::RecommendationMetricsObserver;
use crate::adapters::out::persistence::follow_repository::PgFollowRepository;
use crate::adapters::out::persistence::people_recommendation_repository::PgPeopleRecommendationRepository;
use crate::adapters::r#in::messaging::mutation_receipt_retention_worker::MutationReceiptRetentionWorker;
use crate::adapters::r#in::messaging::recommendation_refresh_worker::RecommendationRefreshWorker;
use crate::adapters::r#in::messaging::recommendation_retention_worker::PeopleRecommendationRetentionWorker;
use crate::application::ports::{ProfileDiscoveryPort, ProfilePrivacyPort};
use crate::application::ranking::LinearPeopleRanker;
use crate::application::use_cases::accept_follow_request::AcceptFollowRequestUseCase;
use crate::application::use_cases::apply_user_deletion::ApplyUserDeletionUseCase;
use crate::application::use_cases::block_user::BlockUserUseCase;
use crate::application::use_cases::cancel_follow_request::CancelFollowRequestUseCase;
use crate::application::use_cases::check_follow::CheckFollowUseCase;
use crate::application::use_cases::follow::FollowUseCase;
use crate::application::use_cases::get_counts::GetCountsUseCase;
use crate::application::use_cases::get_people_recommendations::{
    GetPeopleRecommendationsUseCase, PeopleSequenceConfig,
};
use crate::application::use_cases::get_relationship_policy::GetRelationshipPolicyUseCase;
use crate::application::use_cases::get_stats_batch::GetStatsBatchUseCase;
use crate::application::use_cases::list_follow_requests::ListFollowRequestsUseCase;
use crate::application::use_cases::list_followers::ListFollowersUseCase;
use crate::application::use_cases::list_following::ListFollowingUseCase;
use crate::application::use_cases::mute_user::MuteUserUseCase;
use crate::application::use_cases::purge_expired_mutation_outcomes::PurgeExpiredMutationOutcomesUseCase;
use crate::application::use_cases::record_recommendation_event::RecordRecommendationEventUseCase;
use crate::application::use_cases::reject_follow_request::RejectFollowRequestUseCase;
use crate::application::use_cases::unblock_user::UnblockUserUseCase;
use crate::application::use_cases::unfollow::UnfollowUseCase;
use crate::application::use_cases::unmute_user::UnmuteUserUseCase;
use crate::domain::recommendation::PeopleRankWeights;
use chrono::{Duration, Utc};
use platform::config::{
    env_or, parse_bool_setting, parse_i32_setting, parse_i64_setting, parse_u32_setting,
    parse_u64_setting, require_non_empty_setting, require_production_disabled,
    require_production_postgres_url, require_production_safe_socket_addr,
    require_production_safe_url, runtime_mode_is_production,
};
use platform::edge_metrics::EdgeDeadlineMetrics;
use platform::recommendation_metrics::RecommendationMetrics;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::Arc;
use uuid::Uuid;

pub mod retention_metrics;

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
    /// Maximum time a request waits for one of this service's pooled connections.
    pub database_pool_acquire_timeout_ms: u64,
    pub profile_grpc_endpoint: String,
    pub seed_enabled: bool,
    /// Callers this service accepts, each with its own credential.
    pub caller_registry: platform::grpc_auth::CallerRegistry,
    /// Verifies Gateway actor delegations on actor-bound methods.
    pub actor_delegation: platform::actor_delegation::ActorDelegationVerifier,
    /// Credential this service presents on outbound internal calls.
    pub outbound_identity: String,
    /// Mutual TLS material for internal gRPC, resolved at load time.
    ///
    /// `None` means the documented plaintext development path. Production cannot
    /// reach a running service with `None` because `load` fails first.
    pub tls_material: Option<platform::grpc_tls::TlsMaterial>,
    pub recommendation_worker_enabled: bool,
    pub recommendation_worker_interval_ms: u64,
    pub recommendation_worker_batch_size: i32,
    pub recommendation_worker_lease_timeout_ms: i64,
    pub recommendation_worker_max_attempts: i32,
    pub recommendation_worker_base_retry_ms: i64,
    pub recommendation_retention_worker_enabled: bool,
    pub recommendation_retention_worker_interval_ms: u64,
    pub recommendation_retention_batch_size: i32,
    pub recommendation_event_retention_days: i64,
    pub mutation_receipt_retention_worker_enabled: bool,
    pub mutation_receipt_retention_worker_interval_ms: u64,
    pub mutation_receipt_retention_batch_size: i32,
    pub people_rank_version: String,
    pub people_ranker_adapter: String,
    pub people_embedding_source_enabled: bool,
    pub people_recent_sequence_enabled: bool,
    pub people_recent_sequence_limit: i32,
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let production_mode = runtime_mode_is_production(
            &env_or("FRIEND_RUNTIME_MODE", "production"),
            "FRIEND_RUNTIME_MODE",
        )?;
        // The edge owns user-facing follow operations, post-service reads
        // relationship policies for feeds, and the auth coordinator alone may
        // apply account deletion.
        let caller_registry = platform::config::build_caller_registry(
            production_mode,
            &[
                platform::grpc_auth::Workload::Concert,
                platform::grpc_auth::Workload::Gateway,
                platform::grpc_auth::Workload::Post,
                platform::grpc_auth::Workload::Auth,
            ],
            &[platform::grpc_auth::Workload::Gateway],
        )?;
        let outbound_identity = platform::config::resolve_outbound_identity(
            production_mode,
            platform::grpc_auth::Workload::Friend,
        )?;
        let tls_material = platform::config::resolve_tls_material(production_mode, "friend")?;

        let actor_delegation = platform::actor_delegation::ActorDelegationVerifier::from_env(
            production_mode,
            platform::grpc_auth::Workload::Friend,
        )?;
        let config = Self {
            database_migration_mode: migration_runtime::MigrationMode::from_env_value(
                std::env::var("FRIEND_DATABASE_MIGRATION_MODE"),
            )?,
            http_addr: env_or("FRIEND_HTTP_ADDR", "127.0.0.1:8083").parse()?,
            metrics_addr: env_or("FRIEND_METRICS_ADDR", "127.0.0.1:9083").parse()?,
            grpc_addr: env_or("FRIEND_GRPC_ADDR", "127.0.0.1:50053").parse()?,
            database_url: env_or(
                "FRIEND_DATABASE_URL",
                "postgres://postgres:postgres@localhost:5432/friend_db",
            ),
            database_pool_max_connections: parse_u32_setting(
                "FRIEND_DB_POOL_MAX_CONNECTIONS",
                &env_or("FRIEND_DB_POOL_MAX_CONNECTIONS", "5"),
                1,
                512,
            )?,
            database_pool_acquire_timeout_ms: parse_u64_setting(
                "FRIEND_DB_POOL_ACQUIRE_TIMEOUT_MS",
                &env_or("FRIEND_DB_POOL_ACQUIRE_TIMEOUT_MS", "30000"),
                1,
                300_000,
            )?,
            profile_grpc_endpoint: env_or("FRIEND_PROFILE_GRPC_ENDPOINT", "http://127.0.0.1:50052"),
            seed_enabled: parse_bool_setting(
                "FRIEND_SEED_ENABLED",
                &env_or("FRIEND_SEED_ENABLED", "false"),
            )?,
            caller_registry,
            actor_delegation,
            outbound_identity,
            tls_material,
            recommendation_worker_enabled: parse_bool_setting(
                "FRIEND_RECOMMENDATION_WORKER_ENABLED",
                &env_or("FRIEND_RECOMMENDATION_WORKER_ENABLED", "true"),
            )?,
            recommendation_worker_interval_ms: parse_u64_setting(
                "FRIEND_RECOMMENDATION_WORKER_INTERVAL_MS",
                &env_or("FRIEND_RECOMMENDATION_WORKER_INTERVAL_MS", "5000"),
                100,
                3_600_000,
            )?,
            recommendation_worker_batch_size: parse_i32_setting(
                "FRIEND_RECOMMENDATION_WORKER_BATCH_SIZE",
                &env_or("FRIEND_RECOMMENDATION_WORKER_BATCH_SIZE", "100"),
                1,
                500,
            )?,
            recommendation_worker_lease_timeout_ms: parse_i64_setting(
                "FRIEND_RECOMMENDATION_WORKER_LEASE_TIMEOUT_MS",
                &env_or("FRIEND_RECOMMENDATION_WORKER_LEASE_TIMEOUT_MS", "300000"),
                1000,
                3_600_000,
            )?,
            recommendation_worker_max_attempts: parse_i32_setting(
                "FRIEND_RECOMMENDATION_WORKER_MAX_ATTEMPTS",
                &env_or("FRIEND_RECOMMENDATION_WORKER_MAX_ATTEMPTS", "5"),
                1,
                20,
            )?,
            recommendation_worker_base_retry_ms: parse_i64_setting(
                "FRIEND_RECOMMENDATION_WORKER_BASE_RETRY_MS",
                &env_or("FRIEND_RECOMMENDATION_WORKER_BASE_RETRY_MS", "1000"),
                100,
                3_600_000,
            )?,
            recommendation_retention_worker_enabled: parse_bool_setting(
                "FRIEND_RECOMMENDATION_RETENTION_WORKER_ENABLED",
                &env_or("FRIEND_RECOMMENDATION_RETENTION_WORKER_ENABLED", "true"),
            )?,
            recommendation_retention_worker_interval_ms: parse_u64_setting(
                "FRIEND_RECOMMENDATION_RETENTION_WORKER_INTERVAL_MS",
                &env_or(
                    "FRIEND_RECOMMENDATION_RETENTION_WORKER_INTERVAL_MS",
                    "3600000",
                ),
                1_000,
                86_400_000,
            )?,
            recommendation_retention_batch_size: parse_i32_setting(
                "FRIEND_RECOMMENDATION_RETENTION_BATCH_SIZE",
                &env_or("FRIEND_RECOMMENDATION_RETENTION_BATCH_SIZE", "500"),
                1,
                5_000,
            )?,
            recommendation_event_retention_days: parse_i64_setting(
                "FRIEND_RECOMMENDATION_EVENT_RETENTION_DAYS",
                &env_or("FRIEND_RECOMMENDATION_EVENT_RETENTION_DAYS", "90"),
                1,
                3_650,
            )?,
            mutation_receipt_retention_worker_enabled: parse_bool_setting(
                "FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_ENABLED",
                &env_or("FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_ENABLED", "true"),
            )?,
            mutation_receipt_retention_worker_interval_ms: parse_u64_setting(
                "FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_INTERVAL_MS",
                &env_or(
                    "FRIEND_MUTATION_RECEIPT_RETENTION_WORKER_INTERVAL_MS",
                    "3600000",
                ),
                1_000,
                86_400_000,
            )?,
            mutation_receipt_retention_batch_size: parse_i32_setting(
                "FRIEND_MUTATION_RECEIPT_RETENTION_BATCH_SIZE",
                &env_or("FRIEND_MUTATION_RECEIPT_RETENTION_BATCH_SIZE", "1000"),
                1,
                5_000,
            )?,
            people_rank_version: env_or("FRIEND_PEOPLE_RANK_VERSION", "people-linear-v1"),
            people_ranker_adapter: env_or("FRIEND_PEOPLE_RANKER_ADAPTER", "LINEAR")
                .trim()
                .to_ascii_uppercase(),
            people_embedding_source_enabled: parse_bool_setting(
                "FRIEND_PEOPLE_EMBEDDING_SOURCE_ENABLED",
                &env_or("FRIEND_PEOPLE_EMBEDDING_SOURCE_ENABLED", "false"),
            )?,
            people_recent_sequence_enabled: parse_bool_setting(
                "FRIEND_PEOPLE_RECENT_SEQUENCE_ENABLED",
                &env_or("FRIEND_PEOPLE_RECENT_SEQUENCE_ENABLED", "false"),
            )?,
            people_recent_sequence_limit: parse_i32_setting(
                "FRIEND_PEOPLE_RECENT_SEQUENCE_LIMIT",
                &env_or("FRIEND_PEOPLE_RECENT_SEQUENCE_LIMIT", "20"),
                1,
                100,
            )?,
        };
        validate_production_config(production_mode, &config)?;
        Ok(config)
    }
}

fn validate_production_config(production_mode: bool, config: &AppConfig) -> anyhow::Result<()> {
    require_production_safe_socket_addr(production_mode, "FRIEND_HTTP_ADDR", config.http_addr)?;
    require_production_safe_socket_addr(
        production_mode,
        "FRIEND_METRICS_ADDR",
        config.metrics_addr,
    )?;
    require_production_safe_socket_addr(production_mode, "FRIEND_GRPC_ADDR", config.grpc_addr)?;
    require_production_postgres_url(production_mode, "FRIEND_DATABASE_URL", &config.database_url)?;
    require_production_safe_url(
        production_mode,
        "FRIEND_PROFILE_GRPC_ENDPOINT",
        &config.profile_grpc_endpoint,
    )?;
    require_production_disabled(
        production_mode,
        "FRIEND_SEED_ENABLED",
        config.seed_enabled,
        "demo seed data must not run in production",
    )?;
    require_non_empty_setting("FRIEND_PEOPLE_RANK_VERSION", &config.people_rank_version)?;
    if !matches!(
        config.people_ranker_adapter.as_str(),
        "LINEAR" | "DETERMINISTIC"
    ) {
        anyhow::bail!("FRIEND_PEOPLE_RANKER_ADAPTER requires a configured learned ranker adapter");
    }
    if config.people_embedding_source_enabled {
        anyhow::bail!(
            "FRIEND_PEOPLE_EMBEDDING_SOURCE_ENABLED requires a configured embedding retrieval adapter"
        );
    }
    if !(1..=100).contains(&config.people_recent_sequence_limit) {
        anyhow::bail!("FRIEND_PEOPLE_RECENT_SEQUENCE_LIMIT must be between 1 and 100");
    }
    Ok(())
}

#[derive(Clone)]
pub struct AppState {
    pub apply_user_deletion: ApplyUserDeletionUseCase<Arc<PgFollowRepository>>,
    pub follow: FollowUseCase<Arc<PgFollowRepository>, Arc<dyn ProfilePrivacyPort>>,
    pub unfollow: UnfollowUseCase<Arc<PgFollowRepository>>,
    pub list_followers: ListFollowersUseCase<Arc<PgFollowRepository>>,
    pub list_following: ListFollowingUseCase<Arc<PgFollowRepository>>,
    pub get_counts: GetCountsUseCase<Arc<PgFollowRepository>>,
    pub get_stats_batch: GetStatsBatchUseCase<Arc<PgFollowRepository>>,
    pub check_follow: CheckFollowUseCase<Arc<PgFollowRepository>>,
    pub accept_follow_request: AcceptFollowRequestUseCase<Arc<PgFollowRepository>>,
    pub reject_follow_request: RejectFollowRequestUseCase<Arc<PgFollowRepository>>,
    pub cancel_follow_request: CancelFollowRequestUseCase<Arc<PgFollowRepository>>,
    pub list_follow_requests: ListFollowRequestsUseCase<Arc<PgFollowRepository>>,
    pub block_user: BlockUserUseCase<Arc<PgFollowRepository>>,
    pub unblock_user: UnblockUserUseCase<Arc<PgFollowRepository>>,
    pub mute_user: MuteUserUseCase<Arc<PgFollowRepository>>,
    pub unmute_user: UnmuteUserUseCase<Arc<PgFollowRepository>>,
    pub get_relationship: GetRelationshipPolicyUseCase<Arc<PgFollowRepository>>,
    pub relationship_repository: Arc<PgFollowRepository>,
    pub get_people_recommendations: GetPeopleRecommendationsUseCase<
        Arc<PgPeopleRecommendationRepository>,
        Arc<dyn ProfileDiscoveryPort>,
        Arc<PgFollowRepository>,
    >,
    pub record_recommendation_event:
        RecordRecommendationEventUseCase<Arc<PgPeopleRecommendationRepository>>,
    pub recommendation_refresh_worker:
        RecommendationRefreshWorker<Arc<PgPeopleRecommendationRepository>>,
    pub recommendation_retention_worker:
        PeopleRecommendationRetentionWorker<Arc<PgPeopleRecommendationRepository>>,
    pub recommendation_metrics: Arc<RecommendationMetrics>,
    pub mutation_receipt_retention_worker: MutationReceiptRetentionWorker<Arc<PgFollowRepository>>,
    pub mutation_receipt_retention_metrics: Arc<retention_metrics::MutationReceiptRetentionMetrics>,
    pub edge_deadline_metrics: EdgeDeadlineMetrics,
}

pub async fn connect_pool(config: &AppConfig) -> anyhow::Result<PgPool> {
    let options = migration_runtime::serving_options(
        config.database_url.parse()?,
        config.database_migration_mode,
    );
    let pool = PgPoolOptions::new()
        .max_connections(config.database_pool_max_connections)
        .acquire_timeout(std::time::Duration::from_millis(
            config.database_pool_acquire_timeout_ms,
        ))
        .connect_with(options)
        .await?;
    let migrations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations");
    migration_runtime::prepare_database(
        &pool,
        migrations.as_path(),
        config.database_migration_mode,
        &[migration_runtime::SchemaProbe {
            name: "FRIEND required schema objects",
            sql: include_str!("schema-probe.sql"),
        }],
    )
    .await?;
    Ok(pool)
}

pub async fn build_state(config: &AppConfig, pool: PgPool) -> anyhow::Result<AppState> {
    let edge_deadline_metrics = EdgeDeadlineMetrics::default();
    let profile_client = Arc::new(GrpcProfilePrivacyClient::connect_lazy_with_metrics(
        config.profile_grpc_endpoint.clone(),
        platform::grpc_auth::Workload::Friend,
        &config.outbound_identity,
        config.tls_material.as_ref(),
        edge_deadline_metrics.clone(),
    )?);
    let privacy: Arc<dyn ProfilePrivacyPort> = profile_client.clone();
    let discovery: Arc<dyn ProfileDiscoveryPort> = profile_client;
    build_state_with_dependencies_and_metrics(
        config,
        pool,
        privacy,
        discovery,
        edge_deadline_metrics,
    )
    .await
}

pub async fn build_state_with_dependencies(
    config: &AppConfig,
    pool: PgPool,
    privacy: Arc<dyn ProfilePrivacyPort>,
    discovery: Arc<dyn ProfileDiscoveryPort>,
) -> anyhow::Result<AppState> {
    build_state_with_dependencies_and_metrics(
        config,
        pool,
        privacy,
        discovery,
        EdgeDeadlineMetrics::default(),
    )
    .await
}

pub async fn build_state_with_dependencies_and_metrics(
    config: &AppConfig,
    pool: PgPool,
    privacy: Arc<dyn ProfilePrivacyPort>,
    discovery: Arc<dyn ProfileDiscoveryPort>,
    edge_deadline_metrics: EdgeDeadlineMetrics,
) -> anyhow::Result<AppState> {
    if config.seed_enabled {
        seed_follows(&pool).await?;
    }
    let repository = Arc::new(PgFollowRepository::new(pool));
    let recommendation_repository = Arc::new(PgPeopleRecommendationRepository::new(
        repository.pool().clone(),
    ));
    let recommendation_metrics = Arc::new(RecommendationMetrics::default());
    let mutation_receipt_retention_metrics =
        Arc::new(retention_metrics::MutationReceiptRetentionMetrics::default());
    let weights = PeopleRankWeights {
        version: config.people_rank_version.clone(),
        ..PeopleRankWeights::default()
    };
    Ok(AppState {
        apply_user_deletion: ApplyUserDeletionUseCase::new(repository.clone()),
        follow: FollowUseCase::new(repository.clone(), privacy),
        unfollow: UnfollowUseCase::new(repository.clone()),
        list_followers: ListFollowersUseCase::new(repository.clone()),
        list_following: ListFollowingUseCase::new(repository.clone()),
        get_counts: GetCountsUseCase::new(repository.clone()),
        get_stats_batch: GetStatsBatchUseCase::new(repository.clone()),
        check_follow: CheckFollowUseCase::new(repository.clone()),
        accept_follow_request: AcceptFollowRequestUseCase::new(repository.clone()),
        reject_follow_request: RejectFollowRequestUseCase::new(repository.clone()),
        cancel_follow_request: CancelFollowRequestUseCase::new(repository.clone()),
        list_follow_requests: ListFollowRequestsUseCase::new(repository.clone()),
        block_user: BlockUserUseCase::new(repository.clone()),
        unblock_user: UnblockUserUseCase::new(repository.clone()),
        mute_user: MuteUserUseCase::new(repository.clone()),
        unmute_user: UnmuteUserUseCase::new(repository.clone()),
        get_relationship: GetRelationshipPolicyUseCase::new(repository.clone()),
        relationship_repository: repository.clone(),
        get_people_recommendations: GetPeopleRecommendationsUseCase::new(
            recommendation_repository.clone(),
            discovery,
            repository.clone(),
            Arc::new(LinearPeopleRanker::new(weights)),
            Arc::new(SystemPeopleRecommendationClock),
            Arc::new(RecommendationMetricsObserver::new(
                recommendation_metrics.clone(),
            )),
        )
        .with_sequence_config(PeopleSequenceConfig {
            enabled: config.people_recent_sequence_enabled,
            limit: config.people_recent_sequence_limit,
        }),
        record_recommendation_event: RecordRecommendationEventUseCase::new(
            recommendation_repository.clone(),
        ),
        recommendation_refresh_worker: RecommendationRefreshWorker::new(
            recommendation_repository.clone(),
            config.people_rank_version.clone(),
            Duration::milliseconds(config.recommendation_worker_lease_timeout_ms),
            config.recommendation_worker_batch_size,
            config.recommendation_worker_max_attempts,
            Duration::milliseconds(config.recommendation_worker_base_retry_ms),
        )
        .with_metrics(recommendation_metrics.clone()),
        recommendation_retention_worker: PeopleRecommendationRetentionWorker::new(
            recommendation_repository,
            recommendation_metrics.clone(),
            Duration::days(config.recommendation_event_retention_days),
            config.recommendation_retention_batch_size,
        ),
        mutation_receipt_retention_worker: MutationReceiptRetentionWorker::new(
            PurgeExpiredMutationOutcomesUseCase::new(repository),
            mutation_receipt_retention_metrics.clone(),
            config.mutation_receipt_retention_batch_size,
        ),
        mutation_receipt_retention_metrics,
        edge_deadline_metrics,
        recommendation_metrics,
    })
}

async fn seed_follows(pool: &PgPool) -> anyhow::Result<()> {
    for (follower_id, followee_id) in seed_pairs() {
        sqlx::query(
            "insert into follows (follower_id, followee_id, created_at) values ($1, $2, $3) on conflict (follower_id, followee_id) do nothing",
        )
        .bind(Uuid::parse_str(follower_id)?)
        .bind(Uuid::parse_str(followee_id)?)
        .bind(Utc::now())
        .execute(pool)
        .await?;
    }
    Ok(())
}

fn seed_pairs() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "11111111-1111-1111-1111-111111111111",
            "44444444-4444-4444-4444-444444444444",
        ),
        (
            "44444444-4444-4444-4444-444444444444",
            "77777777-7777-7777-7777-777777777777",
        ),
        (
            "77777777-7777-7777-7777-777777777777",
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        ),
        (
            "11111111-1111-1111-1111-111111111111",
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
        ),
        (
            "11111111-1111-1111-1111-111111111111",
            "77777777-7777-7777-7777-777777777777",
        ),
        (
            "22222222-2222-2222-2222-222222222222",
            "55555555-5555-5555-5555-555555555555",
        ),
        (
            "55555555-5555-5555-5555-555555555555",
            "88888888-8888-8888-8888-888888888888",
        ),
        (
            "22222222-2222-2222-2222-222222222222",
            "88888888-8888-8888-8888-888888888888",
        ),
        (
            "33333333-3333-3333-3333-333333333333",
            "66666666-6666-6666-6666-666666666666",
        ),
        (
            "66666666-6666-6666-6666-666666666666",
            "99999999-9999-9999-9999-999999999999",
        ),
        (
            "33333333-3333-3333-3333-333333333333",
            "99999999-9999-9999-9999-999999999999",
        ),
        (
            "11111111-1111-1111-1111-111111111111",
            "22222222-2222-2222-2222-222222222222",
        ),
        (
            "44444444-4444-4444-4444-444444444444",
            "33333333-3333-3333-3333-333333333333",
        ),
        (
            "77777777-7777-7777-7777-777777777777",
            "66666666-6666-6666-6666-666666666666",
        ),
        (
            "aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
            "88888888-8888-8888-8888-888888888888",
        ),
        (
            "55555555-5555-5555-5555-555555555555",
            "99999999-9999-9999-9999-999999999999",
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_config_rejects_unsafe_friend_defaults() {
        let mut config = production_config();
        config.grpc_addr = "127.0.0.1:50053".parse().expect("addr");
        assert!(validate_production_config(true, &config)
            .expect_err("loopback gRPC bind must fail")
            .to_string()
            .contains("FRIEND_GRPC_ADDR"));

        let mut config = production_config();
        config.database_url = "postgres://postgres:postgres@localhost:5432/friend_db".into();
        assert!(validate_production_config(true, &config)
            .expect_err("local database must fail")
            .to_string()
            .contains("FRIEND_DATABASE_URL"));

        let mut config = production_config();
        config.profile_grpc_endpoint = "http://127.0.0.1:50052".into();
        assert!(validate_production_config(true, &config)
            .expect_err("local profile service must fail")
            .to_string()
            .contains("FRIEND_PROFILE_GRPC_ENDPOINT"));

        let mut config = production_config();
        config.seed_enabled = true;
        assert!(validate_production_config(true, &config)
            .expect_err("seed must fail")
            .to_string()
            .contains("FRIEND_SEED_ENABLED"));
    }

    #[test]
    fn production_config_accepts_explicit_remote_friend_settings() {
        validate_production_config(true, &production_config()).expect("production config");
    }

    #[test]
    fn production_config_rejects_out_of_range_recent_sequence_limit() {
        let mut config = production_config();
        config.people_recent_sequence_limit = 101;

        let error = validate_production_config(true, &config)
            .expect_err("oversized recent sequence must fail");

        assert!(error
            .to_string()
            .contains("FRIEND_PEOPLE_RECENT_SEQUENCE_LIMIT"));
    }

    #[test]
    fn parity_config_allows_local_friend_defaults() {
        let mut config = production_config();
        config.grpc_addr = "127.0.0.1:50053".parse().expect("addr");
        config.database_url = "postgres://postgres:postgres@localhost:5432/friend_db".into();
        config.profile_grpc_endpoint = "http://127.0.0.1:50052".into();
        config.seed_enabled = true;
        validate_production_config(false, &config).expect("parity config");
    }

    fn production_config() -> AppConfig {
        AppConfig {
            database_migration_mode: migration_runtime::MigrationMode::Startup,
            http_addr: "0.0.0.0:8083".parse().expect("addr"),
            metrics_addr: "0.0.0.0:9083".parse().expect("addr"),
            grpc_addr: "0.0.0.0:50053".parse().expect("addr"),
            database_pool_max_connections: 5,
            database_pool_acquire_timeout_ms: 30_000,
            database_url: "postgres://friend:secret@postgres.internal:5432/friend_db".into(),
            profile_grpc_endpoint: "http://profile.internal:50052".into(),
            seed_enabled: false,
            actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
                platform::grpc_auth::Workload::Friend,
            ),
            caller_registry: platform::grpc_auth::CallerRegistry::new()
                .register(
                    platform::grpc_auth::Workload::Gateway,
                    "production-service-token",
                )
                .expect("production caller registry"),
            outbound_identity: "production-service-token".into(),
            tls_material: None,
            recommendation_worker_enabled: true,
            recommendation_worker_interval_ms: 5_000,
            recommendation_worker_batch_size: 100,
            recommendation_worker_lease_timeout_ms: 300_000,
            recommendation_worker_max_attempts: 5,
            recommendation_worker_base_retry_ms: 1_000,
            recommendation_retention_worker_enabled: true,
            recommendation_retention_worker_interval_ms: 3_600_000,
            recommendation_retention_batch_size: 500,
            recommendation_event_retention_days: 90,
            mutation_receipt_retention_worker_enabled: true,
            mutation_receipt_retention_worker_interval_ms: 3_600_000,
            mutation_receipt_retention_batch_size: 1_000,
            people_rank_version: "people-linear-v1".into(),
            people_ranker_adapter: "LINEAR".into(),
            people_embedding_source_enabled: false,
            people_recent_sequence_enabled: false,
            people_recent_sequence_limit: 20,
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
            "follows",
            "follower_id",
            "follows_user_deletion_fence",
        )
        .await
    }
}
