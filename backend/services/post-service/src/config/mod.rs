use crate::adapters::out::cache::feed_cache::RedisFeedCache;
use crate::adapters::out::cache::post_cache::RedisPostCache;
use crate::adapters::out::grpc::friend_client::GrpcFriendClient;
use crate::adapters::out::grpc::notification_client::GrpcNotificationClient;
use crate::adapters::out::grpc::profile_client::GrpcProfileClient;
use crate::adapters::out::messaging::kafka_publisher::{
    DisabledEventPublisher, KafkaEventPublisher,
};
use crate::adapters::out::persistence::content_moderation_repository::PgContentModerationRepository;
use crate::adapters::out::persistence::delivery_schedule::PgDeliverySchedule;
use crate::adapters::out::persistence::event_outbox::PgEventOutbox;
use crate::adapters::out::persistence::feed_recommendation_repository::PgFeedRecommendationRepository;
use crate::adapters::out::persistence::post_repository::{PgPostLikeRepository, PgPostRepository};
use crate::adapters::out::persistence::user_lifecycle_repository::PgUserLifecycleRepository;
use crate::adapters::out::recommendation::feed_candidate_pipeline::{
    DeterministicFeedReranker, FeedCandidatePipeline, LinearFeedRanker,
};
use crate::adapters::r#in::messaging::external_event_forwarder::ExternalEventForwarder;
use crate::adapters::r#in::messaging::internal_notification_listener::InternalNotificationListener;
use crate::adapters::r#in::messaging::outbox_worker::PostOutboxWorker;
use crate::adapters::r#in::messaging::recommendation_retention_worker::FeedRecommendationRetentionWorker;
use crate::adapters::r#in::messaging::retention_purge_worker::PostRetentionPurgeWorker;
use crate::adapters::r#in::messaging::timeline_fanout_worker::TimelineFanoutWorker;
use crate::application::dto::delivery_schedule_dto::{
    DeliveryExecutionConfigDto, DeliveryScheduleConfigDto,
};
use crate::application::ports::{
    ContentModerationRepositoryPort, EventDeliveryMetricsPort, EventOutboxPort,
    ExternalEventPublisherPort, FeedCachePort, FeedCandidateCollectorPort,
    FeedCandidateSourceRepositoryPort, FeedListRerankerPort, FeedRankerPort,
    FeedRecommendationRepositoryPort, FriendClientPort, InternalEventDispatcherPort,
    NotificationClientPort, PostCachePort, PostLikeRepositoryPort, PostRepositoryPort,
    ProfileClientPort, TimelineFanoutRepositoryPort,
};
use crate::application::use_cases::apply_content_moderation::ApplyContentModerationUseCase;
use crate::application::use_cases::apply_user_deletion::ApplyUserDeletionUseCase;
use crate::application::use_cases::authorize_notification_posts::AuthorizeNotificationPostsUseCase;
use crate::application::use_cases::create_comment::CreateCommentUseCase;
use crate::application::use_cases::create_post::CreatePostUseCase;
use crate::application::use_cases::delete_comment::DeleteCommentUseCase;
use crate::application::use_cases::delete_post::DeletePostUseCase;
use crate::application::use_cases::get_author_post_counts::GetAuthorPostCountsUseCase;
use crate::application::use_cases::get_author_posts::GetAuthorPostsUseCase;
use crate::application::use_cases::get_feed::GetFeedUseCase;
use crate::application::use_cases::get_mutation_receipt_status::GetMutationReceiptStatusUseCase;
use crate::application::use_cases::get_post_detail::GetPostDetailUseCase;
use crate::application::use_cases::get_ranked_feed::{RankedFeedConfig, RankedFeedService};
use crate::application::use_cases::get_trending::GetTrendingUseCase;
use crate::application::use_cases::like_post::LikePostUseCase;
use crate::application::use_cases::process_timeline_fanout::{
    ProcessTimelineFanoutUseCase, TimelineFanoutConfig,
};
use crate::application::use_cases::purge_expired_posts::PurgeExpiredPostsUseCase;
use crate::application::use_cases::record_feed_recommendation_event::RecordFeedRecommendationEventUseCase;
use crate::application::use_cases::replay_events::ReplayEventsUseCase;
use crate::application::use_cases::resolve_dead_letter_replay::ResolveDeadLetterReplayUseCase;
use crate::application::use_cases::search_posts::SearchPostsUseCase;
use crate::application::use_cases::unlike_post::UnlikePostUseCase;
use crate::application::use_cases::PostApplicationService;
use crate::domain::post::{DeliveryTarget, OutboxMetricsSnapshot};
use crate::domain::recommendation::FeedRankWeights;
use platform::config::{
    build_caller_registry, env_or, parse_bool_setting, parse_i32_setting, parse_i64_setting,
    parse_u32_setting, parse_u64_setting, require_production_disabled,
    require_production_postgres_url, require_production_safe_socket_addr,
    require_production_safe_url, resolve_outbound_identity,
};
use platform::edge_metrics::EdgeDeadlineMetrics;
use platform::grpc_auth::{CallerRegistry, Workload};
use platform::recommendation_metrics::{
    RecommendationEntity, RecommendationMetricOutcome, RecommendationMetricStage,
    RecommendationMetricSurface, RecommendationMetrics,
};
use post_application::query_witness::OwnedQueryWitnessProducerPort;
use redis::Client as RedisClient;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

const LOCAL_RETENTION_PURGE_ACTOR_ID: &str = "99999999-9999-9999-9999-999999999999";

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
    /// Maximum time a request waits to acquire a PostgreSQL pool connection.
    pub database_pool_acquire_timeout_ms: u64,
    pub redis_url: String,
    /// Feed-cache namespace. Bump this when Redis is reset or cache state is
    /// otherwise invalidated while old requests may still be in flight.
    pub feed_cache_epoch: u64,
    pub friend_grpc_endpoint: String,
    pub profile_grpc_endpoint: String,
    pub notification_grpc_endpoint: String,
    /// Gateway internal listener that exchanges an admitted request's authority
    /// for a one-hop child to Friend or Profile.
    pub gateway_grpc_endpoint: String,
    /// Signs the Post-owned resource evidence the Gateway exchange fetches.
    pub resource_evidence_signer: platform::actor_delegation::exchange::ResourceEvidenceSigner,
    pub seed_enabled: bool,
    pub runtime_mode: RuntimeMode,
    /// Callers this service accepts, each with its own credential.
    pub caller_registry: CallerRegistry,
    /// Verifies Gateway actor delegations on actor-bound methods.
    pub actor_delegation: platform::actor_delegation::ActorDelegationVerifier,
    /// Credential this service presents on outbound internal calls.
    pub outbound_identity: String,
    /// Mutual TLS material for internal gRPC, resolved at load time.
    ///
    /// `None` means the documented plaintext development path. Production cannot
    /// reach a running service with `None` because `load` fails first.
    pub tls_material: Option<platform::grpc_tls::TlsMaterial>,
    pub outbox_worker_enabled: bool,
    pub outbox_worker_interval_ms: u64,
    pub outbox_worker_batch_size: i32,
    pub outbox_worker_min_event_age_ms: u64,
    pub outbox_delivery: DeliveryExecutionConfigDto,
    /// Which external publisher the running deployment expects to deliver.
    pub external_publisher: ExternalPublisherMode,
    /// Kafka bootstrap brokers, used only when the mode is `Kafka`.
    pub kafka_bootstrap_brokers: Vec<String>,
    /// Topic that receives external events.
    pub kafka_topic: String,
    /// Partitions the publisher spreads aggregates across.
    pub kafka_partition_count: i32,
    /// Time to wait for one broker acknowledgement.
    pub kafka_publish_timeout_ms: u64,
    /// Time to wait for the broker to answer the initial connection.
    pub kafka_connect_timeout_ms: u64,
    /// Whether the service creates the topic when it does not exist.
    pub kafka_create_topic: bool,
    pub retention_purge_worker_enabled: bool,
    pub retention_purge_worker_interval_ms: u64,
    pub retention_purge_actor_id: String,
    pub timeline_fanout_worker_enabled: bool,
    pub timeline_fanout_worker_interval_ms: u64,
    pub timeline_fanout_claim_limit: i32,
    pub timeline_follower_page_size: i32,
    pub timeline_fanout_lease_timeout_ms: u64,
    pub timeline_fanout_max_attempts: i32,
    pub timeline_fanout_base_retry_ms: u64,
    pub timeline_entry_ttl_seconds: u64,
    pub high_fanout_follower_threshold: i64,
    pub feed_candidate_source_timeout_ms: u64,
    pub feed_candidate_per_source_limit: i32,
    pub feed_following_author_limit: i32,
    pub feed_snapshot_candidate_limit: i32,
    pub feed_snapshot_ttl_seconds: u64,
    pub feed_rank_version: String,
    pub feed_ranker_adapter: String,
    pub feed_embedding_source_enabled: bool,
    pub feed_recent_sequence_enabled: bool,
    pub feed_recent_sequence_limit: i32,
    pub feed_retention_worker_enabled: bool,
    pub feed_retention_worker_interval_ms: u64,
    pub feed_retention_batch_size: i32,
    pub feed_event_retention_days: i64,
}

#[derive(Debug, Clone)]
pub struct RecommendationRuntimeConfig {
    pub source_timeout: std::time::Duration,
    pub per_source_limit: i32,
    pub following_author_limit: usize,
    pub snapshot_candidate_limit: usize,
    pub snapshot_ttl_seconds: i64,
    pub rank_version: String,
    pub recent_sequence_enabled: bool,
    pub recent_sequence_limit: i32,
    pub high_fanout_threshold: i64,
    pub retention_worker_enabled: bool,
    pub retention_worker_interval_ms: u64,
    pub retention_batch_size: i32,
    pub event_retention_days: i64,
    pub fanout: TimelineFanoutConfig,
}

impl RecommendationRuntimeConfig {
    pub fn from_app(config: &AppConfig) -> anyhow::Result<Self> {
        Ok(Self {
            source_timeout: std::time::Duration::from_millis(
                config.feed_candidate_source_timeout_ms,
            ),
            per_source_limit: config.feed_candidate_per_source_limit,
            following_author_limit: usize::try_from(config.feed_following_author_limit)?,
            snapshot_candidate_limit: usize::try_from(config.feed_snapshot_candidate_limit)?,
            snapshot_ttl_seconds: i64::try_from(config.feed_snapshot_ttl_seconds)?,
            rank_version: config.feed_rank_version.clone(),
            recent_sequence_enabled: config.feed_recent_sequence_enabled,
            recent_sequence_limit: config.feed_recent_sequence_limit,
            high_fanout_threshold: config.high_fanout_follower_threshold,
            retention_worker_enabled: config.feed_retention_worker_enabled,
            retention_worker_interval_ms: config.feed_retention_worker_interval_ms,
            retention_batch_size: config.feed_retention_batch_size,
            event_retention_days: config.feed_event_retention_days,
            fanout: TimelineFanoutConfig {
                claim_limit: config.timeline_fanout_claim_limit,
                follower_page_size: config.timeline_follower_page_size,
                lease_timeout: std::time::Duration::from_millis(
                    config.timeline_fanout_lease_timeout_ms,
                ),
                max_attempts: config.timeline_fanout_max_attempts,
                base_retry: std::time::Duration::from_millis(config.timeline_fanout_base_retry_ms),
                entry_ttl: std::time::Duration::from_secs(config.timeline_entry_ttl_seconds),
            },
        })
    }
}

impl AppConfig {
    pub fn load() -> anyhow::Result<Self> {
        let runtime_mode = RuntimeMode::parse(&env_or("POST_RUNTIME_MODE", "production"))?;
        let is_production = matches!(runtime_mode, RuntimeMode::Production);
        // The edge owns user-facing writes and moderation; no other workload may
        // drive them. Account deletion is the auth coordinator's job, so it is
        // accepted here as well but never from a sibling service.
        let caller_registry = build_caller_registry(
            is_production,
            &[Workload::Gateway, Workload::Auth],
            &[Workload::Gateway],
        )?;
        let outbound_identity = resolve_outbound_identity(is_production, Workload::Post)?;
        let tls_material = platform::config::resolve_tls_material(is_production, "post")?;
        let retention_purge_actor_id = resolve_retention_purge_actor_id(
            runtime_mode,
            platform::config::optional_env("POST_RETENTION_PURGE_ACTOR_ID"),
        )?;

        let actor_delegation = platform::actor_delegation::ActorDelegationVerifier::from_env(
            is_production,
            platform::grpc_auth::Workload::Post,
        )?;
        let config = Self {
            database_migration_mode: migration_runtime::MigrationMode::from_env_value(
                std::env::var("POST_DATABASE_MIGRATION_MODE"),
            )?,
            http_addr: env_or("POST_HTTP_ADDR", "127.0.0.1:8085").parse()?,
            metrics_addr: env_or("POST_METRICS_ADDR", "127.0.0.1:9085").parse()?,
            grpc_addr: env_or("POST_GRPC_ADDR", "127.0.0.1:50055").parse()?,
            database_url: env_or(
                "POST_DATABASE_URL",
                "postgres://postgres:postgres@localhost:5432/post_db",
            ),
            database_pool_max_connections: parse_u32_setting(
                "POST_DB_POOL_MAX_CONNECTIONS",
                &env_or("POST_DB_POOL_MAX_CONNECTIONS", "5"),
                1,
                512,
            )?,
            database_pool_acquire_timeout_ms: parse_u64_setting(
                "POST_DB_POOL_ACQUIRE_TIMEOUT_MS",
                &env_or("POST_DB_POOL_ACQUIRE_TIMEOUT_MS", "30000"),
                1,
                300_000,
            )?,
            redis_url: env_or("POST_REDIS_URL", "redis://127.0.0.1:6379/"),
            feed_cache_epoch: parse_u64_setting(
                "POST_FEED_CACHE_EPOCH",
                &env_or("POST_FEED_CACHE_EPOCH", "1"),
                1,
                u64::MAX,
            )?,
            friend_grpc_endpoint: env_or("POST_FRIEND_GRPC_ENDPOINT", "http://127.0.0.1:50053"),
            profile_grpc_endpoint: env_or("POST_PROFILE_GRPC_ENDPOINT", "http://127.0.0.1:50052"),
            notification_grpc_endpoint: env_or(
                "POST_NOTIFICATION_GRPC_ENDPOINT",
                "http://127.0.0.1:50054",
            ),
            gateway_grpc_endpoint: env_or("POST_GATEWAY_GRPC_ENDPOINT", "http://127.0.0.1:50050"),
            resource_evidence_signer:
                platform::actor_delegation::exchange::ResourceEvidenceSigner::from_env(
                    is_production,
                )?,
            seed_enabled: parse_bool_setting(
                "POST_SEED_ENABLED",
                &env_or("POST_SEED_ENABLED", "false"),
            )?,
            runtime_mode,
            caller_registry,
            actor_delegation,
            outbound_identity,
            tls_material,
            outbox_worker_enabled: parse_bool_setting(
                "POST_OUTBOX_WORKER_ENABLED",
                &env_or("POST_OUTBOX_WORKER_ENABLED", "true"),
            )?,
            outbox_worker_interval_ms: parse_u64_setting(
                "POST_OUTBOX_WORKER_INTERVAL_MS",
                &env_or("POST_OUTBOX_WORKER_INTERVAL_MS", "1000"),
                100,
                86_400_000,
            )?,
            outbox_worker_batch_size: parse_i32_setting(
                "POST_OUTBOX_WORKER_BATCH_SIZE",
                &env_or("POST_OUTBOX_WORKER_BATCH_SIZE", "100"),
                1,
                10_000,
            )?,
            outbox_worker_min_event_age_ms: parse_u64_setting(
                "POST_OUTBOX_WORKER_MIN_EVENT_AGE_MS",
                &env_or("POST_OUTBOX_WORKER_MIN_EVENT_AGE_MS", "0"),
                0,
                3_600_000,
            )?,
            outbox_delivery: load_delivery_execution_config()?,
            external_publisher: ExternalPublisherMode::parse(&env_or(
                "POST_EXTERNAL_PUBLISHER",
                "disabled",
            ))?,
            kafka_bootstrap_brokers: parse_kafka_brokers(&env_or(
                "POST_KAFKA_BOOTSTRAP_BROKERS",
                "",
            )),
            kafka_topic: env_or("POST_KAFKA_TOPIC", "post-events"),
            kafka_partition_count: parse_i32_setting(
                "POST_KAFKA_PARTITION_COUNT",
                &env_or("POST_KAFKA_PARTITION_COUNT", "6"),
                1,
                512,
            )?,
            kafka_publish_timeout_ms: parse_u64_setting(
                "POST_KAFKA_PUBLISH_TIMEOUT_MS",
                &env_or("POST_KAFKA_PUBLISH_TIMEOUT_MS", "5000"),
                100,
                300_000,
            )?,
            kafka_create_topic: parse_bool_setting(
                "POST_KAFKA_CREATE_TOPIC",
                &env_or("POST_KAFKA_CREATE_TOPIC", "false"),
            )?,
            kafka_connect_timeout_ms: parse_u64_setting(
                "POST_KAFKA_CONNECT_TIMEOUT_MS",
                &env_or("POST_KAFKA_CONNECT_TIMEOUT_MS", "10000"),
                100,
                300_000,
            )?,
            retention_purge_worker_enabled: parse_bool_setting(
                "POST_RETENTION_PURGE_WORKER_ENABLED",
                &env_or("POST_RETENTION_PURGE_WORKER_ENABLED", "true"),
            )?,
            retention_purge_worker_interval_ms: parse_u64_setting(
                "POST_RETENTION_PURGE_WORKER_INTERVAL_MS",
                &env_or("POST_RETENTION_PURGE_WORKER_INTERVAL_MS", "3600000"),
                1_000,
                86_400_000,
            )?,
            retention_purge_actor_id,
            timeline_fanout_worker_enabled: parse_bool_setting(
                "POST_TIMELINE_FANOUT_WORKER_ENABLED",
                &env_or("POST_TIMELINE_FANOUT_WORKER_ENABLED", "true"),
            )?,
            timeline_fanout_worker_interval_ms: parse_u64_setting(
                "POST_TIMELINE_FANOUT_WORKER_INTERVAL_MS",
                &env_or("POST_TIMELINE_FANOUT_WORKER_INTERVAL_MS", "500"),
                100,
                86_400_000,
            )?,
            timeline_fanout_claim_limit: parse_i32_setting(
                "POST_TIMELINE_FANOUT_CLAIM_LIMIT",
                &env_or("POST_TIMELINE_FANOUT_CLAIM_LIMIT", "20"),
                1,
                100,
            )?,
            timeline_follower_page_size: parse_i32_setting(
                "POST_TIMELINE_FOLLOWER_PAGE_SIZE",
                &env_or("POST_TIMELINE_FOLLOWER_PAGE_SIZE", "100"),
                1,
                100,
            )?,
            timeline_fanout_lease_timeout_ms: parse_u64_setting(
                "POST_TIMELINE_FANOUT_LEASE_TIMEOUT_MS",
                &env_or("POST_TIMELINE_FANOUT_LEASE_TIMEOUT_MS", "60000"),
                1_000,
                3_600_000,
            )?,
            timeline_fanout_max_attempts: parse_i32_setting(
                "POST_TIMELINE_FANOUT_MAX_ATTEMPTS",
                &env_or("POST_TIMELINE_FANOUT_MAX_ATTEMPTS", "8"),
                1,
                100,
            )?,
            timeline_fanout_base_retry_ms: parse_u64_setting(
                "POST_TIMELINE_FANOUT_BASE_RETRY_MS",
                &env_or("POST_TIMELINE_FANOUT_BASE_RETRY_MS", "2000"),
                100,
                3_600_000,
            )?,
            timeline_entry_ttl_seconds: parse_u64_setting(
                "POST_TIMELINE_ENTRY_TTL_SECONDS",
                &env_or("POST_TIMELINE_ENTRY_TTL_SECONDS", "1209600"),
                3_600,
                31_536_000,
            )?,
            high_fanout_follower_threshold: parse_i64_setting(
                "POST_HIGH_FANOUT_FOLLOWER_THRESHOLD",
                &env_or("POST_HIGH_FANOUT_FOLLOWER_THRESHOLD", "10000"),
                100,
                1_000_000_000,
            )?,
            feed_candidate_source_timeout_ms: parse_u64_setting(
                "POST_FEED_CANDIDATE_SOURCE_TIMEOUT_MS",
                &env_or("POST_FEED_CANDIDATE_SOURCE_TIMEOUT_MS", "40"),
                5,
                5_000,
            )?,
            feed_candidate_per_source_limit: parse_i32_setting(
                "POST_FEED_CANDIDATE_PER_SOURCE_LIMIT",
                &env_or("POST_FEED_CANDIDATE_PER_SOURCE_LIMIT", "200"),
                10,
                500,
            )?,
            feed_following_author_limit: parse_i32_setting(
                "POST_FEED_FOLLOWING_AUTHOR_LIMIT",
                &env_or("POST_FEED_FOLLOWING_AUTHOR_LIMIT", "5000"),
                100,
                10_000,
            )?,
            feed_snapshot_candidate_limit: parse_i32_setting(
                "POST_FEED_SNAPSHOT_CANDIDATE_LIMIT",
                &env_or("POST_FEED_SNAPSHOT_CANDIDATE_LIMIT", "500"),
                20,
                2_000,
            )?,
            feed_snapshot_ttl_seconds: parse_u64_setting(
                "POST_FEED_SNAPSHOT_TTL_SECONDS",
                &env_or("POST_FEED_SNAPSHOT_TTL_SECONDS", "900"),
                60,
                86_400,
            )?,
            feed_rank_version: env_or("POST_FEED_RANK_VERSION", "feed-linear-v1"),
            feed_ranker_adapter: env_or("POST_FEED_RANKER_ADAPTER", "LINEAR")
                .trim()
                .to_ascii_uppercase(),
            feed_embedding_source_enabled: parse_bool_setting(
                "POST_FEED_EMBEDDING_SOURCE_ENABLED",
                &env_or("POST_FEED_EMBEDDING_SOURCE_ENABLED", "false"),
            )?,
            feed_recent_sequence_enabled: parse_bool_setting(
                "POST_FEED_RECENT_SEQUENCE_ENABLED",
                &env_or("POST_FEED_RECENT_SEQUENCE_ENABLED", "false"),
            )?,
            feed_recent_sequence_limit: parse_i32_setting(
                "POST_FEED_RECENT_SEQUENCE_LIMIT",
                &env_or("POST_FEED_RECENT_SEQUENCE_LIMIT", "20"),
                1,
                100,
            )?,
            feed_retention_worker_enabled: parse_bool_setting(
                "POST_FEED_RETENTION_WORKER_ENABLED",
                &env_or("POST_FEED_RETENTION_WORKER_ENABLED", "true"),
            )?,
            feed_retention_worker_interval_ms: parse_u64_setting(
                "POST_FEED_RETENTION_WORKER_INTERVAL_MS",
                &env_or("POST_FEED_RETENTION_WORKER_INTERVAL_MS", "3600000"),
                1_000,
                86_400_000,
            )?,
            feed_retention_batch_size: parse_i32_setting(
                "POST_FEED_RETENTION_BATCH_SIZE",
                &env_or("POST_FEED_RETENTION_BATCH_SIZE", "500"),
                1,
                5_000,
            )?,
            feed_event_retention_days: parse_i64_setting(
                "POST_FEED_EVENT_RETENTION_DAYS",
                &env_or("POST_FEED_EVENT_RETENTION_DAYS", "90"),
                1,
                3_650,
            )?,
        };
        config.validate()?;
        Ok(config)
    }

    pub fn validate(&self) -> anyhow::Result<()> {
        let production = matches!(self.runtime_mode, RuntimeMode::Production);
        // The resource signer must not hold a key owners accept as Gateway user
        // authority, or Post could mint delegations for any user.
        anyhow::ensure!(
            !self
                .actor_delegation
                .trusts(&self.resource_evidence_signer.public_key()),
            "{} must not match a key in {}",
            platform::actor_delegation::exchange::RESOURCE_EVIDENCE_SIGNING_SEED_ENV,
            platform::actor_delegation::VERIFYING_KEYS_ENV
        );
        require_production_safe_socket_addr(production, "POST_HTTP_ADDR", self.http_addr)?;
        require_production_safe_socket_addr(production, "POST_METRICS_ADDR", self.metrics_addr)?;
        require_production_safe_socket_addr(production, "POST_GRPC_ADDR", self.grpc_addr)?;
        if self.runtime_mode.allows_admin_http_routes() && !self.http_addr.ip().is_loopback() {
            anyhow::bail!("POST_HTTP_ADDR must be loopback when parity replay routes are enabled");
        }
        require_production_postgres_url(production, "POST_DATABASE_URL", &self.database_url)?;
        require_production_safe_url(production, "POST_REDIS_URL", &self.redis_url)?;
        require_production_disabled(
            production,
            "POST_SEED_ENABLED",
            self.seed_enabled,
            "demo seed data must not run in production",
        )?;
        // The disabled mode is valid on its own; the Kafka mode needs a broker
        // list, because a producer with no bootstrap brokers cannot deliver and
        // would otherwise fail on the first event instead of at startup.
        if matches!(self.external_publisher, ExternalPublisherMode::Kafka) {
            if self.kafka_bootstrap_brokers.is_empty() {
                anyhow::bail!(
                    "POST_KAFKA_BOOTSTRAP_BROKERS must list at least one broker when POST_EXTERNAL_PUBLISHER is `kafka`"
                );
            }
            // A broker is `host:port`, not a URL, so the check is per broker on
            // the host alone. This rejects a loopback or placeholder broker in
            // production for the same reason the other endpoints are checked:
            // a deployment pointed at a local stand-in must not start.
            for broker in &self.kafka_bootstrap_brokers {
                require_production_safe_url(production, "POST_KAFKA_BOOTSTRAP_BROKERS", broker)?;
            }
        }
        if self.kafka_topic.len() > 249 || self.kafka_topic.trim().is_empty() {
            // 249 is the Kafka topic length limit.
            anyhow::bail!("POST_KAFKA_TOPIC must be between 1 and 249 characters");
        }
        if !(1..=512).contains(&self.kafka_partition_count) {
            anyhow::bail!("POST_KAFKA_PARTITION_COUNT must be between 1 and 512");
        }
        if !(100..=300_000).contains(&self.kafka_publish_timeout_ms) {
            anyhow::bail!("POST_KAFKA_PUBLISH_TIMEOUT_MS must be between 100 and 300000");
        }
        if !(100..=300_000).contains(&self.kafka_connect_timeout_ms) {
            anyhow::bail!("POST_KAFKA_CONNECT_TIMEOUT_MS must be between 100 and 300000");
        }

        if self.feed_rank_version.len() > 64 || self.feed_rank_version.trim().is_empty() {
            anyhow::bail!("POST_FEED_RANK_VERSION is invalid");
        }
        if self.feed_ranker_adapter != "LINEAR" {
            anyhow::bail!("POST_FEED_RANKER_ADAPTER requires a configured learned ranker adapter");
        }
        if self.feed_embedding_source_enabled {
            anyhow::bail!(
                "POST_FEED_EMBEDDING_SOURCE_ENABLED requires a configured embedding retrieval adapter"
            );
        }
        if !(1..=100).contains(&self.feed_recent_sequence_limit) {
            anyhow::bail!("POST_FEED_RECENT_SEQUENCE_LIMIT must be between 1 and 100");
        }
        if !(1..=5_000).contains(&self.feed_retention_batch_size) {
            anyhow::bail!("POST_FEED_RETENTION_BATCH_SIZE must be between 1 and 5000");
        }
        if !(1..=3_650).contains(&self.feed_event_retention_days) {
            anyhow::bail!("POST_FEED_EVENT_RETENTION_DAYS must be between 1 and 3650");
        }
        for (name, endpoint) in [
            ("POST_FRIEND_GRPC_ENDPOINT", &self.friend_grpc_endpoint),
            ("POST_PROFILE_GRPC_ENDPOINT", &self.profile_grpc_endpoint),
            (
                "POST_NOTIFICATION_GRPC_ENDPOINT",
                &self.notification_grpc_endpoint,
            ),
            ("POST_GATEWAY_GRPC_ENDPOINT", &self.gateway_grpc_endpoint),
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
            _ => anyhow::bail!("POST_RUNTIME_MODE must be production or parity"),
        }
    }

    pub fn allows_admin_http_routes(self) -> bool {
        matches!(self, Self::Parity)
    }
}

/// Which external publisher a deployment expects to deliver with.
///
/// The mode is configuration rather than a fallback because the two states are
/// operationally different. `Disabled` publishes nothing and says so, leaving
/// events retryable; `Kafka` must actually reach a broker. A deployment that did
/// not choose either is the case this type removes: it would previously look
/// healthy while publishing nothing.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExternalPublisherMode {
    /// External delivery is switched off. The `EXTERNAL` target is skipped and
    /// events stay outstanding so they can be delivered once configured.
    Disabled,
    /// External delivery goes to a Kafka topic.
    Kafka,
}

impl ExternalPublisherMode {
    pub fn parse(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "kafka" => Ok(Self::Kafka),
            "disabled" | "off" | "none" => Ok(Self::Disabled),
            _ => anyhow::bail!("POST_EXTERNAL_PUBLISHER must be `kafka` or `disabled`"),
        }
    }

    pub fn as_str(self) -> &'static str {
        match self {
            Self::Disabled => "disabled",
            Self::Kafka => "kafka",
        }
    }
}

/// Split the bootstrap broker list, ignoring surrounding whitespace and empty
/// entries so a trailing separator in a deployment template is harmless.
///
/// Whether the list may be empty is a question for `validate`, which knows the
/// selected publisher mode. Parsing a list and deciding whether a list is
/// required are different questions, and answering both here would make the
/// requirement invisible at the point where the mode is known.
fn parse_kafka_brokers(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|broker| !broker.is_empty())
        .map(ToOwned::to_owned)
        .collect()
}

fn resolve_retention_purge_actor_id(
    runtime_mode: RuntimeMode,
    configured: Option<String>,
) -> anyhow::Result<String> {
    let actor_id = match configured
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
    {
        Some(value) => value,
        None if matches!(runtime_mode, RuntimeMode::Parity) => {
            LOCAL_RETENTION_PURGE_ACTOR_ID.into()
        }
        None => anyhow::bail!("POST_RETENTION_PURGE_ACTOR_ID is required in production"),
    };
    uuid::Uuid::parse_str(&actor_id)
        .map_err(|_| anyhow::anyhow!("POST_RETENTION_PURGE_ACTOR_ID must be a UUID"))?;
    Ok(actor_id)
}

#[derive(Clone)]
pub struct AppState {
    pub apply_content_moderation: ApplyContentModerationUseCase,
    pub apply_user_deletion:
        ApplyUserDeletionUseCase<Arc<PgUserLifecycleRepository>, Arc<dyn PostCachePort>>,
    pub create_post: CreatePostUseCase,
    pub create_comment: CreateCommentUseCase,
    pub delete_comment: DeleteCommentUseCase,
    pub delete_post: DeletePostUseCase,
    pub get_feed: GetFeedUseCase,
    pub get_mutation_receipt_status: GetMutationReceiptStatusUseCase,
    pub get_author_posts: GetAuthorPostsUseCase,
    pub get_author_post_counts: GetAuthorPostCountsUseCase,
    pub search_posts: SearchPostsUseCase,
    pub get_post_detail: GetPostDetailUseCase,
    pub authorize_notification_posts: AuthorizeNotificationPostsUseCase,
    pub like_post: LikePostUseCase,
    pub unlike_post: UnlikePostUseCase,
    pub get_trending: GetTrendingUseCase,
    pub replay_events: ReplayEventsUseCase,
    pub resolve_dead_letter_replay: ResolveDeadLetterReplayUseCase,
    pub record_feed_recommendation_event: RecordFeedRecommendationEventUseCase,
    pub purge_expired_posts: PurgeExpiredPostsUseCase,
    pub outbox: Arc<dyn EventOutboxPort>,
    pub outbox_worker: PostOutboxWorker,
    pub retention_purge_worker: PostRetentionPurgeWorker,
    pub timeline_fanout_worker: TimelineFanoutWorker,
    pub recommendation_retention_worker:
        FeedRecommendationRetentionWorker<Arc<PgFeedRecommendationRepository>>,
    pub metrics: Arc<PostMetrics>,
}

#[derive(Clone, Default)]
pub struct PostMetrics {
    counters: Arc<Mutex<BTreeMap<String, i64>>>,
    recommendation: Arc<RecommendationMetrics>,
    edge_deadlines: EdgeDeadlineMetrics,
}

impl PostMetrics {
    pub fn edge_deadline_metrics(&self) -> EdgeDeadlineMetrics {
        self.edge_deadlines.clone()
    }

    pub fn record_recommendation_request(&self, outcome: RecommendationMetricOutcome) {
        self.recommendation
            .record_request(RecommendationMetricSurface::HomeFeed, outcome);
    }

    pub fn record_recommendation_retention_run(&self, outcome: RecommendationMetricOutcome) {
        self.recommendation
            .record_retention_run(RecommendationMetricSurface::HomeFeed, outcome);
    }

    pub fn record_recommendation_retention(
        &self,
        entity: RecommendationEntity,
        outcome: RecommendationMetricOutcome,
        count: i64,
    ) {
        self.recommendation.record_retention(
            RecommendationMetricSurface::HomeFeed,
            entity,
            outcome,
            count,
        );
    }

    pub fn observe_recommendation_stage_latency(
        &self,
        stage: RecommendationMetricStage,
        millis: u64,
    ) {
        self.recommendation.observe_stage_latency(
            RecommendationMetricSurface::HomeFeed,
            stage,
            millis,
        );
    }

    pub fn recommendation_metrics(&self) -> Arc<RecommendationMetrics> {
        self.recommendation.clone()
    }

    pub fn render_recommendation_prometheus(&self) -> String {
        self.recommendation.render_prometheus()
    }
    pub fn record_timeline_fanout_run(&self, outcome: &str) {
        self.inc(
            "post_timeline_fanout_runs_total",
            &[("outcome", outcome)],
            1,
        );
    }

    pub fn record_timeline_fanout_pages(&self, outcome: &str, count: i64) {
        self.inc(
            "post_timeline_fanout_pages_total",
            &[("outcome", outcome)],
            count,
        );
    }

    pub fn record_outbox_worker_run(&self, outcome: &str) {
        self.inc("post_outbox_worker_runs_total", &[("outcome", outcome)], 1);
    }

    pub fn record_outbox_worker_forwarded(&self, outcome: &str, count: i64) {
        self.inc(
            "post_outbox_worker_forwarded_events_total",
            &[("outcome", outcome)],
            count,
        );
    }

    pub fn record_retention_purge_run(&self, outcome: &str) {
        self.inc(
            "post_retention_purge_runs_total",
            &[("outcome", outcome)],
            1,
        );
    }

    pub fn record_retention_purged_posts(&self, outcome: &str, count: i64) {
        self.inc(
            "post_retention_purged_posts_total",
            &[("outcome", outcome)],
            count,
        );
    }

    pub fn record_retention_purged_comments(&self, outcome: &str, count: i64) {
        self.inc(
            "post_retention_purged_comments_total",
            &[("outcome", outcome)],
            count,
        );
    }

    pub fn render_prometheus(&self, snapshot: &OutboxMetricsSnapshot) -> String {
        let counters = self.counters.lock().expect("post metrics");
        let mut output = String::new();
        output.push_str(&format!(
            "post_outbox_unpublished_total {}\n",
            snapshot.unpublished_total
        ));
        output.push_str(&format!(
            "post_outbox_oldest_unpublished_age_seconds {}\n",
            snapshot.oldest_unpublished_age_seconds
        ));
        output.push_str(&format!(
            "post_outbox_dead_lettered_total {}\n",
            snapshot.dead_lettered_total
        ));
        for (sample, value) in counters.iter() {
            output.push_str(sample);
            output.push(' ');
            output.push_str(&value.to_string());
            output.push('\n');
        }
        output.push_str(&self.recommendation.render_prometheus());
        output.push_str(&self.edge_deadlines.render_prometheus());
        output
    }

    fn inc(&self, name: &str, labels: &[(&str, &str)], amount: i64) {
        let sample = format!("{name}{{{}}}", label_set(labels));
        let mut counters = self.counters.lock().expect("post metrics");
        *counters.entry(sample).or_insert(0) += amount;
    }
}

impl EventDeliveryMetricsPort for PostMetrics {
    fn record_target_failure(&self, target: DeliveryTarget) {
        self.inc(
            "post_outbox_worker_target_delivery_failures_total",
            &[("target", target.as_str())],
            1,
        );
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
        .acquire_timeout(Duration::from_millis(
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
            name: "POST required schema objects",
            sql: include_str!("schema-probe.sql"),
        }],
    )
    .await?;
    Ok(pool)
}

pub async fn build_state(config: &AppConfig, pool: PgPool) -> anyhow::Result<AppState> {
    let metrics = Arc::new(PostMetrics::default());
    let redis = RedisClient::open(config.redis_url.clone())?;
    let post_repository: Arc<dyn PostRepositoryPort> =
        Arc::new(PgPostRepository::new(pool.clone()));
    let like_repository: Arc<dyn PostLikeRepositoryPort> =
        Arc::new(PgPostLikeRepository::new(pool.clone()));
    let outbox: Arc<dyn EventOutboxPort> = Arc::new(PgEventOutbox::new(pool.clone()));
    // One managed connection is shared by both caches. Each call previously
    // opened its own, so a page load that read the feed and then the posts paid
    // two connection handshakes for a dependency whose purpose is to be cheaper
    // than the database.
    let cache_connection = crate::adapters::out::cache::connect_manager(redis).await?;
    let feed_cache: Arc<dyn FeedCachePort> = Arc::new(RedisFeedCache::with_namespace_epoch(
        cache_connection.clone(),
        config.feed_cache_epoch,
    ));
    let post_cache: Arc<dyn PostCachePort> =
        Arc::new(RedisPostCache::with_connection(cache_connection));
    // Request handling acts for the admitted user: each Friend or Profile call
    // first exchanges that authority at the Gateway for a one-hop child bound to
    // the exact owner method and body. A call without admitted authority is
    // denied rather than sent with workload identity alone.
    let exchange = Arc::new(
        crate::adapters::out::grpc::authority_exchange_client::GrpcAuthorizationExchangeClient::connect(
            &config.gateway_grpc_endpoint,
            &config.outbound_identity,
            config.tls_material.as_ref(),
            authority_exchange_budget(),
        )?
        .with_deadline_metrics(metrics.edge_deadline_metrics()),
    );
    let friend_client: Arc<dyn FriendClientPort> = Arc::new(
        GrpcFriendClient::connect_with_metrics(
            config.friend_grpc_endpoint.clone(),
            Workload::Post,
            &config.outbound_identity,
            config.tls_material.as_ref(),
            metrics.edge_deadline_metrics(),
        )
        .await?
        .with_exchange(exchange.clone()),
    );
    // Delivery, fanout and notification workers act for no user. They keep the
    // workload-authenticated client and call only Friend methods that Friend
    // serves to Post without child proof.
    let background_friend_client: Arc<dyn FriendClientPort> = Arc::new(
        GrpcFriendClient::connect_with_metrics(
            config.friend_grpc_endpoint.clone(),
            Workload::Post,
            &config.outbound_identity,
            config.tls_material.as_ref(),
            metrics.edge_deadline_metrics(),
        )
        .await?,
    );
    let profile_client: Arc<dyn ProfileClientPort> = Arc::new(
        GrpcProfileClient::connect_with_metrics(
            config.profile_grpc_endpoint.clone(),
            Workload::Post,
            &config.outbound_identity,
            config.tls_material.as_ref(),
            metrics.edge_deadline_metrics(),
        )
        .await?
        .with_exchange(exchange),
    );
    let notification_client: Arc<dyn NotificationClientPort> = Arc::new(
        GrpcNotificationClient::connect_with_metrics(
            config.notification_grpc_endpoint.clone(),
            Workload::Post,
            &config.outbound_identity,
            config.tls_material.as_ref(),
            metrics.edge_deadline_metrics(),
        )
        .await?,
    );
    // The publisher is chosen from configuration rather than defaulted. A
    // deployment that asks for Kafka gets a producer that must reach a broker;
    // a deployment that asks for nothing gets a publisher that reports itself
    // disabled, so the two modes cannot be confused from the outside.
    let publisher: Arc<dyn ExternalEventPublisherPort> = match config.external_publisher {
        ExternalPublisherMode::Kafka => Arc::new(
            KafkaEventPublisher::connect(
                config.kafka_bootstrap_brokers.clone(),
                config.kafka_topic.clone(),
                config.kafka_partition_count,
                std::time::Duration::from_millis(config.kafka_connect_timeout_ms),
                std::time::Duration::from_millis(config.kafka_publish_timeout_ms),
                config.kafka_create_topic,
            )
            .await?,
        ),
        ExternalPublisherMode::Disabled => {
            // Stated at startup as well as per attempt. An operator reading logs
            // should not have to wait for the first event to learn the mode.
            tracing::warn!(
                "POST_EXTERNAL_PUBLISHER is `disabled`; events retain their registered destination requirements"
            );
            Arc::new(DisabledEventPublisher::new())
        }
    };
    let dispatcher: Arc<dyn InternalEventDispatcherPort> =
        Arc::new(InternalNotificationListener::new(
            post_repository.clone(),
            background_friend_client.clone(),
            notification_client.clone(),
        ));

    build_state_with_clients(
        pool.clone(),
        config.seed_enabled,
        config.retention_purge_actor_id.clone(),
        post_repository,
        Arc::new(crate::adapters::r#in::grpc::query_witness::AdmittedQueryWitnessProducer::new(
            Arc::new(crate::adapters::out::persistence::authority_witness_repository::PgAuthorityWitnessRepository::new(pool.clone())))),
        like_repository,
        feed_cache,
        post_cache,
        Clients {
            interactive_friend: friend_client,
            background_friend: background_friend_client,
        },
        profile_client,
        outbox,
        publisher,
        dispatcher,
        RecommendationRuntimeConfig::from_app(config)?,
        config.outbox_delivery.clone(),
        metrics,
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn build_state_with_dependencies(
    pool: PgPool,
    seed_enabled: bool,
    retention_purge_actor_id: String,
    post_repository: Arc<dyn PostRepositoryPort>,
    query_witnesses: Arc<dyn OwnedQueryWitnessProducerPort>,
    like_repository: Arc<dyn PostLikeRepositoryPort>,
    feed_cache: Arc<dyn FeedCachePort>,
    post_cache: Arc<dyn PostCachePort>,
    friend_client: Arc<dyn FriendClientPort>,
    profile_client: Arc<dyn ProfileClientPort>,
    outbox: Arc<dyn EventOutboxPort>,
    publisher: Arc<dyn ExternalEventPublisherPort>,
    dispatcher: Arc<dyn InternalEventDispatcherPort>,
    recommendation_config: RecommendationRuntimeConfig,
) -> anyhow::Result<AppState> {
    build_state_with_dependencies_and_metrics(
        pool,
        seed_enabled,
        retention_purge_actor_id,
        post_repository,
        query_witnesses,
        like_repository,
        feed_cache,
        post_cache,
        friend_client,
        profile_client,
        outbox,
        publisher,
        dispatcher,
        recommendation_config,
        DeliveryExecutionConfigDto::default(),
        Arc::new(PostMetrics::default()),
    )
    .await
}

#[allow(clippy::too_many_arguments)]
pub async fn build_state_with_dependencies_and_metrics(
    pool: PgPool,
    seed_enabled: bool,
    retention_purge_actor_id: String,
    post_repository: Arc<dyn PostRepositoryPort>,
    query_witnesses: Arc<dyn OwnedQueryWitnessProducerPort>,
    like_repository: Arc<dyn PostLikeRepositoryPort>,
    feed_cache: Arc<dyn FeedCachePort>,
    post_cache: Arc<dyn PostCachePort>,
    friend_client: Arc<dyn FriendClientPort>,
    profile_client: Arc<dyn ProfileClientPort>,
    outbox: Arc<dyn EventOutboxPort>,
    publisher: Arc<dyn ExternalEventPublisherPort>,
    dispatcher: Arc<dyn InternalEventDispatcherPort>,
    recommendation_config: RecommendationRuntimeConfig,
    execution_config: DeliveryExecutionConfigDto,
    metrics: Arc<PostMetrics>,
) -> anyhow::Result<AppState> {
    build_state_with_clients(
        pool,
        seed_enabled,
        retention_purge_actor_id,
        post_repository,
        query_witnesses,
        like_repository,
        feed_cache,
        post_cache,
        Clients {
            interactive_friend: friend_client.clone(),
            background_friend: friend_client,
        },
        profile_client,
        outbox,
        publisher,
        dispatcher,
        recommendation_config,
        execution_config,
        metrics,
    )
    .await
}

/// Friend clients split by whose authority a call carries.
struct Clients {
    /// Used by request handling for the admitted user.
    interactive_friend: Arc<dyn FriendClientPort>,
    /// Used by delivery and fanout workers, which act for no user.
    background_friend: Arc<dyn FriendClientPort>,
}

/// The Post to Gateway exchange edge, which carries the owner-approved Gateway
/// to Post budget values under its own name.
pub fn authority_exchange_budget() -> platform::edge::EdgeBudget {
    platform::edge::Edge::PostGateway.budget()
}

#[allow(clippy::too_many_arguments)]
async fn build_state_with_clients(
    pool: PgPool,
    seed_enabled: bool,
    retention_purge_actor_id: String,
    post_repository: Arc<dyn PostRepositoryPort>,
    query_witnesses: Arc<dyn OwnedQueryWitnessProducerPort>,
    like_repository: Arc<dyn PostLikeRepositoryPort>,
    feed_cache: Arc<dyn FeedCachePort>,
    post_cache: Arc<dyn PostCachePort>,
    clients: Clients,
    profile_client: Arc<dyn ProfileClientPort>,
    outbox: Arc<dyn EventOutboxPort>,
    publisher: Arc<dyn ExternalEventPublisherPort>,
    dispatcher: Arc<dyn InternalEventDispatcherPort>,
    recommendation_config: RecommendationRuntimeConfig,
    execution_config: DeliveryExecutionConfigDto,
    metrics: Arc<PostMetrics>,
) -> anyhow::Result<AppState> {
    let Clients {
        interactive_friend: friend_client,
        background_friend: background_friend_client,
    } = clients;
    if seed_enabled {
        super_seed(&pool).await?;
    }

    let content_moderation_repository: Arc<dyn ContentModerationRepositoryPort> =
        Arc::new(PgContentModerationRepository::new(pool.clone()));
    let user_lifecycle_repository = Arc::new(PgUserLifecycleRepository::new(pool.clone()));
    let feed_repository = Arc::new(PgFeedRecommendationRepository::new(pool.clone()));
    let candidate_source_repository: Arc<dyn FeedCandidateSourceRepositoryPort> =
        feed_repository.clone();
    let feed_recommendation_repository: Arc<dyn FeedRecommendationRepositoryPort> =
        feed_repository.clone();
    let timeline_repository: Arc<dyn TimelineFanoutRepositoryPort> = feed_repository.clone();
    if !feed_recommendation_repository
        .model_ready(&recommendation_config.rank_version)
        .await?
    {
        anyhow::bail!(
            "Configured feed rank version `{}` is not ready",
            recommendation_config.rank_version
        );
    }
    let service = Arc::new(PostApplicationService {
        posts: post_repository.clone(),
        query_witnesses,
        likes: like_repository,
        feed_cache: feed_cache.clone(),
        post_cache: post_cache.clone(),
        friend_client: friend_client.clone(),
        profile_client,
        outbox: outbox.clone(),
        replay_delivery: Arc::new(
            crate::adapters::r#in::messaging::fenced_replay_delivery::FencedReplayDelivery::new(
                Arc::new(PgDeliverySchedule::new(pool.clone())),
                publisher.clone(),
                dispatcher.clone(),
                execution_config.clone(),
            )?,
        ),
        publisher: publisher.clone(),
        internal_dispatcher: dispatcher.clone(),
    });
    let purge_expired_posts = PurgeExpiredPostsUseCase::new(service.clone());
    let collector: Arc<dyn FeedCandidateCollectorPort> = Arc::new(FeedCandidatePipeline::new(
        candidate_source_repository,
        recommendation_config.source_timeout,
    ));
    let weights = FeedRankWeights {
        version: recommendation_config.rank_version.clone(),
        ..FeedRankWeights::default()
    };
    let ranker: Arc<dyn FeedRankerPort> = Arc::new(LinearFeedRanker::new(weights));
    let reranker: Arc<dyn FeedListRerankerPort> = Arc::new(DeterministicFeedReranker::new(
        2,
        1,
        (recommendation_config.snapshot_candidate_limit / 10).max(1),
    ));
    let ranked_feed = Arc::new(
        RankedFeedService::new(
            service.clone(),
            collector,
            ranker,
            reranker,
            feed_recommendation_repository.clone(),
            RankedFeedConfig {
                per_source_limit: recommendation_config.per_source_limit,
                snapshot_candidate_limit: recommendation_config.snapshot_candidate_limit,
                snapshot_ttl_seconds: recommendation_config.snapshot_ttl_seconds,
                following_author_limit: recommendation_config.following_author_limit,
                recent_sequence_enabled: recommendation_config.recent_sequence_enabled,
                recent_sequence_limit: recommendation_config.recent_sequence_limit,
            },
        )
        .with_metrics(metrics.recommendation_metrics()),
    );
    let process_timeline_fanout = ProcessTimelineFanoutUseCase::new(
        timeline_repository.clone(),
        background_friend_client.clone(),
        recommendation_config.fanout.clone(),
    );
    let timeline_fanout_worker =
        TimelineFanoutWorker::new(process_timeline_fanout, metrics.clone());
    let recommendation_retention_worker = FeedRecommendationRetentionWorker::new(
        feed_repository.clone(),
        metrics.clone(),
        chrono::Duration::days(recommendation_config.event_retention_days),
        recommendation_config.retention_batch_size,
    );
    let outbox_worker = PostOutboxWorker::new(
        ExternalEventForwarder::new(
            Arc::new(PgDeliverySchedule::new(pool.clone())),
            publisher.clone(),
            dispatcher,
            feed_cache,
            post_cache.clone(),
            background_friend_client,
            Arc::new(crate::adapters::out::persistence::timeline_projection_repository::PgTimelineProjectionRepository::new(pool.clone())),
        )
        .with_execution_config(execution_config)?
        .with_delivery_metrics(metrics.clone())
        .with_high_fanout_threshold(recommendation_config.high_fanout_threshold),
        metrics.clone(),
    );
    let retention_purge_worker = PostRetentionPurgeWorker::new(
        purge_expired_posts.clone(),
        metrics.clone(),
        retention_purge_actor_id,
    );

    Ok(AppState {
        apply_content_moderation: ApplyContentModerationUseCase::new(
            content_moderation_repository,
            post_cache.clone(),
        ),
        apply_user_deletion: ApplyUserDeletionUseCase::new(
            user_lifecycle_repository,
            post_cache.clone(),
        ),
        create_post: CreatePostUseCase::new(service.clone()),
        create_comment: CreateCommentUseCase::new(service.clone()),
        delete_comment: DeleteCommentUseCase::new(service.clone()),
        delete_post: DeletePostUseCase::new(service.clone()),
        get_feed: GetFeedUseCase::with_ranked(service.clone(), ranked_feed),
        get_mutation_receipt_status: GetMutationReceiptStatusUseCase::new(post_repository),
        get_author_posts: GetAuthorPostsUseCase::new(service.clone()),
        get_author_post_counts: GetAuthorPostCountsUseCase::new(service.clone()),
        search_posts: SearchPostsUseCase::new(service.clone()),
        get_post_detail: GetPostDetailUseCase::new(service.clone()),
        authorize_notification_posts: AuthorizeNotificationPostsUseCase::new(service.clone()),
        like_post: LikePostUseCase::new(service.clone()),
        unlike_post: UnlikePostUseCase::new(service.clone()),
        get_trending: GetTrendingUseCase::new(service.clone()),
        replay_events: ReplayEventsUseCase::new(outbox.clone(), service.replay_delivery.clone()),
        resolve_dead_letter_replay: ResolveDeadLetterReplayUseCase::new(outbox.clone()),
        record_feed_recommendation_event: RecordFeedRecommendationEventUseCase::new(
            feed_recommendation_repository,
        ),
        purge_expired_posts,
        outbox,
        outbox_worker,
        retention_purge_worker,
        timeline_fanout_worker,
        recommendation_retention_worker,
        metrics,
    })
}

async fn super_seed(_pool: &PgPool) -> anyhow::Result<()> {
    Ok(())
}

fn load_delivery_execution_config() -> anyhow::Result<DeliveryExecutionConfigDto> {
    let value = DeliveryExecutionConfigDto {
        schedule: DeliveryScheduleConfigDto {
            external_enabled: false,
            lease_duration_ms: parse_u64_setting(
                "POST_OUTBOX_DELIVERY_LEASE_MS",
                &env_or("POST_OUTBOX_DELIVERY_LEASE_MS", "300000"),
                1,
                86_400_000,
            )?,
            max_attempts: parse_i32_setting(
                "POST_OUTBOX_DELIVERY_MAX_ATTEMPTS",
                &env_or("POST_OUTBOX_DELIVERY_MAX_ATTEMPTS", "5"),
                1,
                1000,
            )?,
            retry_base_ms: parse_u64_setting(
                "POST_OUTBOX_DELIVERY_RETRY_BASE_MS",
                &env_or("POST_OUTBOX_DELIVERY_RETRY_BASE_MS", "1000"),
                1,
                86_400_000,
            )?,
            retry_cap_ms: parse_u64_setting(
                "POST_OUTBOX_DELIVERY_RETRY_CAP_MS",
                &env_or("POST_OUTBOX_DELIVERY_RETRY_CAP_MS", "300000"),
                1,
                86_400_000,
            )?,
        },
        effect_timeout_ms: parse_u64_setting(
            "POST_OUTBOX_DELIVERY_EFFECT_TIMEOUT_MS",
            &env_or("POST_OUTBOX_DELIVERY_EFFECT_TIMEOUT_MS", "25000"),
            1,
            86_400_000,
        )?,
        completion_reserve_ms: parse_u64_setting(
            "POST_OUTBOX_DELIVERY_COMPLETION_RESERVE_MS",
            &env_or("POST_OUTBOX_DELIVERY_COMPLETION_RESERVE_MS", "1000"),
            1,
            86_400_000,
        )?,
    };
    value.validate()?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn production_retention_purge_actor_id_must_be_explicit() {
        let error = resolve_retention_purge_actor_id(RuntimeMode::Production, None)
            .expect_err("production should require explicit actor id")
            .to_string();

        assert!(error.contains("POST_RETENTION_PURGE_ACTOR_ID"));
    }

    #[test]
    fn parity_retention_purge_actor_id_uses_local_default() {
        let actor_id = resolve_retention_purge_actor_id(RuntimeMode::Parity, None).expect("actor");

        assert_eq!(actor_id, LOCAL_RETENTION_PURGE_ACTOR_ID);
    }

    #[test]
    fn production_config_rejects_unsafe_post_defaults() {
        let mut config = production_config();
        config.redis_url = "redis://127.0.0.1:6379/".into();
        assert!(config
            .validate()
            .expect_err("local redis must fail")
            .to_string()
            .contains("POST_REDIS_URL"));

        let mut config = production_config();
        config.seed_enabled = true;
        assert!(config
            .validate()
            .expect_err("seed must fail")
            .to_string()
            .contains("POST_SEED_ENABLED"));

        let mut config = production_config();
        config.notification_grpc_endpoint = "http://127.0.0.1:50054".into();
        assert!(config
            .validate()
            .expect_err("local notification endpoint must fail")
            .to_string()
            .contains("POST_NOTIFICATION_GRPC_ENDPOINT"));
    }

    #[test]
    fn config_rejects_learned_ranker_without_a_configured_adapter() {
        let mut config = production_config();
        config.feed_ranker_adapter = "TWO_TOWER".into();

        let error = config
            .validate()
            .expect_err("learned ranker without adapter must fail");

        assert!(error.to_string().contains("POST_FEED_RANKER_ADAPTER"));
    }

    #[test]
    fn config_rejects_embedding_source_without_a_configured_adapter() {
        let mut config = production_config();
        config.feed_embedding_source_enabled = true;

        let error = config
            .validate()
            .expect_err("embedding source without adapter must fail");

        assert!(error
            .to_string()
            .contains("POST_FEED_EMBEDDING_SOURCE_ENABLED"));
    }

    #[test]
    fn config_rejects_oversized_whitespace_text_before_normalization() {
        let mut config = production_config();
        config.kafka_topic = " ".repeat(250);
        assert!(config
            .validate()
            .expect_err("oversized Kafka topic")
            .to_string()
            .contains("POST_KAFKA_TOPIC"));

        let mut config = production_config();
        config.feed_rank_version = " ".repeat(65);
        assert!(config
            .validate()
            .expect_err("oversized feed rank version")
            .to_string()
            .contains("POST_FEED_RANK_VERSION"));
    }

    #[test]
    fn config_rejects_out_of_range_recent_sequence_limit() {
        let mut config = production_config();
        config.feed_recent_sequence_limit = 101;

        let error = config
            .validate()
            .expect_err("oversized recent sequence must fail");

        assert!(error
            .to_string()
            .contains("POST_FEED_RECENT_SEQUENCE_LIMIT"));
    }

    #[test]
    fn config_rejects_unbounded_feed_retention_settings() {
        let mut config = production_config();
        config.feed_retention_batch_size = 5_001;
        assert!(config
            .validate()
            .expect_err("oversized feed retention batch must fail")
            .to_string()
            .contains("POST_FEED_RETENTION_BATCH_SIZE"));

        let mut config = production_config();
        config.feed_event_retention_days = 0;
        assert!(config
            .validate()
            .expect_err("zero feed event retention must fail")
            .to_string()
            .contains("POST_FEED_EVENT_RETENTION_DAYS"));
    }

    #[test]
    fn production_config_accepts_explicit_remote_post_settings() {
        production_config().validate().expect("production config");
    }

    #[test]
    fn resource_evidence_seed_must_not_match_a_trusted_actor_key() {
        let mut config = production_config();
        // production_config trusts the local parity actor key; reuse its seed.
        config.resource_evidence_signer =
            platform::actor_delegation::exchange::ResourceEvidenceSigner::from_seed(
                b"local-parity-actor-delegation-00",
            )
            .expect("seed");
        let error = config
            .validate()
            .expect_err("a resource key trusted for actor authority must fail");
        assert!(error
            .to_string()
            .contains("RESOURCE_EVIDENCE_SIGNING_SEED must not match"));
    }

    #[test]
    fn kafka_publisher_mode_requires_at_least_one_broker() {
        let mut config = production_config();
        config.external_publisher = ExternalPublisherMode::Kafka;
        config.kafka_bootstrap_brokers = Vec::new();

        let error = config
            .validate()
            .expect_err("kafka mode without a broker must not start");

        assert!(error.to_string().contains("POST_KAFKA_BOOTSTRAP_BROKERS"));
    }

    #[test]
    fn disabled_publisher_mode_does_not_require_a_broker() {
        let mut config = production_config();
        config.external_publisher = ExternalPublisherMode::Disabled;
        config.kafka_bootstrap_brokers = Vec::new();

        // The disabled mode publishes nothing on purpose, so a broker list is
        // not a requirement it can meet or miss.
        config.validate().expect("disabled publisher config");
    }

    #[test]
    fn kafka_publisher_mode_rejects_a_loopback_broker_in_production() {
        let mut config = production_config();
        config.external_publisher = ExternalPublisherMode::Kafka;
        config.kafka_bootstrap_brokers = vec!["127.0.0.1:9092".into()];

        let error = config
            .validate()
            .expect_err("a local broker must fail in production");

        assert!(error.to_string().contains("POST_KAFKA_BOOTSTRAP_BROKERS"));
    }

    #[test]
    fn kafka_publisher_mode_accepts_a_remote_broker_in_production() {
        let mut config = production_config();
        config.external_publisher = ExternalPublisherMode::Kafka;
        config.kafka_bootstrap_brokers = vec![
            "kafka-0.internal:9092".into(),
            "kafka-1.internal:9092".into(),
        ];

        config.validate().expect("remote brokers");
    }

    #[test]
    fn external_publisher_mode_parses_only_the_stated_values() {
        assert_eq!(
            ExternalPublisherMode::parse(" KAFKA ").expect("kafka"),
            ExternalPublisherMode::Kafka
        );
        assert_eq!(
            ExternalPublisherMode::parse("disabled").expect("disabled"),
            ExternalPublisherMode::Disabled
        );
        // A misspelling must not silently select the mode that publishes nothing,
        // because that is the state that looks healthy while losing events.
        let error = ExternalPublisherMode::parse("kakfa").expect_err("typo must fail");
        assert!(error.to_string().contains("POST_EXTERNAL_PUBLISHER"));
    }

    #[test]
    fn kafka_broker_list_ignores_separators_and_surrounding_whitespace() {
        assert_eq!(parse_kafka_brokers(" "), Vec::<String>::new());
        assert_eq!(
            parse_kafka_brokers("kafka-0.internal:9092 , kafka-1.internal:9092,"),
            vec!["kafka-0.internal:9092", "kafka-1.internal:9092"]
        );
    }

    #[test]
    fn parity_config_allows_local_post_defaults() {
        let mut config = production_config();
        config.runtime_mode = RuntimeMode::Parity;
        config.http_addr = "127.0.0.1:8085".parse().expect("addr");
        config.grpc_addr = "127.0.0.1:50055".parse().expect("addr");
        config.metrics_addr = "127.0.0.1:9085".parse().expect("addr");
        config.database_url = "postgres://postgres:postgres@localhost:5432/post_db".into();
        config.redis_url = "redis://127.0.0.1:6379/".into();
        config.friend_grpc_endpoint = "http://127.0.0.1:50053".into();
        config.profile_grpc_endpoint = "http://127.0.0.1:50052".into();
        config.notification_grpc_endpoint = "http://127.0.0.1:50054".into();
        config.seed_enabled = true;
        config.validate().expect("parity config");

        config.http_addr = "0.0.0.0:8085".parse().expect("wildcard addr");
        let error = config
            .validate()
            .expect_err("parity replay listener must remain loopback-bound");
        assert!(error.to_string().contains("POST_HTTP_ADDR"));
        assert!(error.to_string().contains("loopback"));
    }

    fn production_config() -> AppConfig {
        AppConfig {
            database_migration_mode: migration_runtime::MigrationMode::Startup,
            http_addr: "0.0.0.0:8085".parse().expect("addr"),
            metrics_addr: "0.0.0.0:9085".parse().expect("addr"),
            grpc_addr: "0.0.0.0:50055".parse().expect("addr"),
            database_pool_max_connections: 5,
            database_pool_acquire_timeout_ms: 30_000,
            database_url: "postgres://post:secret@postgres.internal:5432/post_db".into(),
            redis_url: "redis://redis.internal:6379/".into(),
            feed_cache_epoch: 1,
            friend_grpc_endpoint: "http://friend.internal:50053".into(),
            profile_grpc_endpoint: "http://profile.internal:50052".into(),
            notification_grpc_endpoint: "http://notification.internal:50054".into(),
            gateway_grpc_endpoint: "http://gateway.internal:50050".into(),
            resource_evidence_signer:
                platform::actor_delegation::exchange::ResourceEvidenceSigner::from_seed(&[7; 32])
                    .expect("resource evidence seed"),
            seed_enabled: false,
            runtime_mode: RuntimeMode::Production,
            actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
                platform::grpc_auth::Workload::Post,
            ),
            caller_registry: platform::grpc_auth::CallerRegistry::new()
                .register(
                    platform::grpc_auth::Workload::Gateway,
                    "production-service-token",
                )
                .expect("production caller registry"),
            outbound_identity: "production-service-token".into(),
            tls_material: None,
            outbox_worker_enabled: true,
            outbox_worker_interval_ms: 1000,
            outbox_worker_batch_size: 100,
            outbox_worker_min_event_age_ms: 0,
            outbox_delivery: DeliveryExecutionConfigDto::default(),
            external_publisher: ExternalPublisherMode::Disabled,
            kafka_bootstrap_brokers: Vec::new(),
            kafka_topic: "post-events".into(),
            kafka_partition_count: 6,
            kafka_publish_timeout_ms: 5_000,
            kafka_connect_timeout_ms: 10_000,
            kafka_create_topic: false,
            retention_purge_worker_enabled: true,
            retention_purge_worker_interval_ms: 3_600_000,
            retention_purge_actor_id: LOCAL_RETENTION_PURGE_ACTOR_ID.into(),
            timeline_fanout_worker_enabled: true,
            timeline_fanout_worker_interval_ms: 1_000,
            timeline_fanout_claim_limit: 100,
            timeline_follower_page_size: 100,
            timeline_fanout_lease_timeout_ms: 30_000,
            timeline_fanout_max_attempts: 5,
            timeline_fanout_base_retry_ms: 1_000,
            timeline_entry_ttl_seconds: 2_592_000,
            high_fanout_follower_threshold: 10_000,
            feed_candidate_source_timeout_ms: 40,
            feed_candidate_per_source_limit: 200,
            feed_following_author_limit: 5_000,
            feed_snapshot_candidate_limit: 500,
            feed_snapshot_ttl_seconds: 900,
            feed_rank_version: "feed-linear-v1".into(),
            feed_ranker_adapter: "LINEAR".into(),
            feed_embedding_source_enabled: false,
            feed_recent_sequence_enabled: false,
            feed_recent_sequence_limit: 20,
            feed_retention_worker_enabled: true,
            feed_retention_worker_interval_ms: 3_600_000,
            feed_retention_batch_size: 500,
            feed_event_retention_days: 90,
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
            "posts",
            "id",
            "posts_user_deletion_fence",
        )
        .await
    }
}
