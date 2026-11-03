#![allow(dead_code)]
mod query_witness;
use post_service::application::query_witness::PublishedWitnessStageDto;
use query_witness::UnforwardedQueryFixture;

use anyhow::Result;
use async_trait::async_trait;
use axum::body::Body;
use axum::http::{Method, Request};
use http_body_util::BodyExt;
use post_service::adapters::out::cache::feed_cache::{
    feed_cache_generation_key, feed_cache_key, RedisFeedCache,
};
use post_service::adapters::out::cache::post_cache::RedisPostCache;
use post_service::adapters::out::persistence::event_outbox::PgEventOutbox;
use post_service::adapters::out::persistence::post_repository::{
    PgPostLikeRepository, PgPostRepository,
};
use post_service::adapters::r#in::grpc::server::PostGrpcServer;
use post_service::adapters::r#in::http;
use post_service::adapters::r#in::messaging::external_event_forwarder::ExternalEventForwarder;
use post_service::adapters::r#in::messaging::internal_notification_listener::InternalNotificationListener;
use post_service::application::ports::{
    EventOutboxPort, ExternalEventPublisherPort, FeedCachePort, FriendClientPort,
    InternalEventDispatcherPort, NotificationClientPort, PostCachePort, PostLikeRepositoryPort,
    PostRepositoryPort, ProfileClientPort,
};
use post_service::application::use_cases::get_feed::GetFeedUseCase;
use post_service::application::use_cases::process_timeline_fanout::TimelineFanoutConfig;
use post_service::application::use_cases::PostApplicationService;
use post_service::config::{
    build_state_with_dependencies, AppConfig, AppState, RecommendationRuntimeConfig, RuntimeMode,
};
use post_service::domain::post::{NotificationRequest, RelationshipPolicySnapshot, StoredEvent};
use redis::AsyncCommands;
use redis::Client as RedisClient;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use test_support::postgres::PostgresContainer;
pub(crate) mod native_kafka;
pub(crate) mod native_postgres;
pub(crate) mod native_redis;
use native_redis::RedisFixture;
use tokio::sync::Mutex;
use tower::ServiceExt;
use uuid::Uuid;

const TEST_FEED_CANDIDATE_SOURCE_TIMEOUT_MS: u64 = 2_000;

/// Callers accepted by the test server. Both the edge and the auth deletion
/// coordinator call this service, so the fixture registers both workloads.
fn test_caller_registry() -> platform::grpc_auth::CallerRegistry {
    platform::grpc_auth::CallerRegistry::new()
        .register(
            platform::grpc_auth::Workload::Gateway,
            "test-gateway-credential",
        )
        .expect("gateway caller")
        .register(platform::grpc_auth::Workload::Auth, "test-auth-credential")
        .expect("auth caller")
}

pub struct TestApp {
    pub _postgres: Option<PostgresContainer>,
    pub _redis: Option<RedisFixture>,
    pub pool: PgPool,
    pub state: Arc<AppState>,
    pub chronological_feed: GetFeedUseCase,
    feed_service: Arc<PostApplicationService>,
    notifications: Arc<RecordingNotificationClient>,
    redis_client: RedisClient,
    outbox: Arc<dyn EventOutboxPort>,
    publisher: Arc<dyn ExternalEventPublisherPort>,
    dispatcher: Arc<dyn InternalEventDispatcherPort>,
    feed_cache: Arc<dyn FeedCachePort>,
    post_cache: Arc<dyn PostCachePort>,
    friend_client: Arc<dyn FriendClientPort>,
    static_friend_client: Arc<StaticFriendClient>,
    recommendation_config: RecommendationRuntimeConfig,
}

impl TestApp {
    pub fn claimed_cache_test_client(&self) -> RedisClient {
        self.redis_client.clone()
    }

    /// Required deadline-aware adapter, in the same epoch as this fixture's config.
    pub async fn claimed_cache_effects(
        &self,
    ) -> Result<Arc<dyn post_service::application::cache_authority::ClaimedCacheEffectsPort>> {
        Ok(Arc::new(
            post_service::adapters::out::cache::claimed_effects::RedisClaimedCacheEffects::new(
                self.redis_client.clone(),
                1,
            )
            .await?,
        ))
    }

    /// Real fixture adapters for explicitly composed cache delivery acceptance.
    pub fn claimed_cache_effect_ports(&self) -> (Arc<dyn FeedCachePort>, Arc<dyn PostCachePort>) {
        (self.feed_cache.clone(), self.post_cache.clone())
    }

    /// Rebuild the actual production application composition with explicit query
    /// authority/Friend dependencies; retains real candidate/snapshot/impression
    /// repositories, ranker, reranker and the fixture's original runtime config.
    pub async fn state_with_query_witnesses(
        &self,
        producer: Arc<dyn post_service::application::query_witness::OwnedQueryWitnessProducerPort>,
        friend: Arc<dyn FriendClientPort>,
    ) -> Result<Arc<AppState>> {
        Ok(Arc::new(
            build_state_with_dependencies(
                self.pool.clone(),
                false,
                "99999999-9999-9999-9999-999999999999".into(),
                self.feed_service.posts.clone(),
                producer,
                self.feed_service.likes.clone(),
                self.feed_cache.clone(),
                self.post_cache.clone(),
                friend,
                self.feed_service.profile_client.clone(),
                self.outbox.clone(),
                self.publisher.clone(),
                self.dispatcher.clone(),
                self.recommendation_config.clone(),
            )
            .await?,
        ))
    }

    /// Replace only the Profile existence dependency for native child-authority
    /// acceptance while retaining the real Post transaction/outbox repository.
    pub fn application_with_profile_author(
        &self,
        profile: Arc<dyn ProfileClientPort>,
    ) -> Arc<PostApplicationService> {
        let mut service = (*self.feed_service).clone();
        service.profile_client = profile;
        Arc::new(service)
    }

    /// Native producer acceptance retains the owned PostgreSQL/Redis backends,
    /// replaces only query-authority composition and the relationship recorder.
    pub fn application_with_query_witnesses(
        &self,
        producer: Arc<dyn post_service::application::query_witness::OwnedQueryWitnessProducerPort>,
        friend: Arc<dyn FriendClientPort>,
    ) -> Arc<PostApplicationService> {
        let mut service = (*self.feed_service).clone();
        service.query_witnesses = producer;
        service.friend_client = friend;
        Arc::new(service)
    }

    /// Build a feed use case that pauses once immediately before writing a
    /// computed page to Redis. This lets integration tests commit a mutation
    /// in the exact read-fill window without adding hooks to production code.
    pub fn chronological_feed_with_write_barrier(
        &self,
    ) -> (GetFeedUseCase, Arc<FeedCacheWriteBarrier>) {
        let barrier = Arc::new(FeedCacheWriteBarrier::default());
        let feed_cache: Arc<dyn FeedCachePort> = Arc::new(BarrierFeedCache {
            inner: self.feed_cache.clone(),
            barrier: barrier.clone(),
            block_next_write: AtomicBool::new(true),
        });
        let mut service = (*self.feed_service).clone();
        service.feed_cache = feed_cache;
        (GetFeedUseCase::new(Arc::new(service)), barrier)
    }

    /// Build a feed use case that pauses after resolving a warm cache entry and
    /// before asking the relationship service for current authorization policy.
    pub fn chronological_feed_with_relationship_policy_barrier(
        &self,
    ) -> (GetFeedUseCase, Arc<FeedRelationshipPolicyReadBarrier>) {
        let barrier = Arc::new(FeedRelationshipPolicyReadBarrier::default());
        let friend_client: Arc<dyn FriendClientPort> = Arc::new(BarrierFriendClient {
            inner: self.friend_client.clone(),
            barrier: barrier.clone(),
            block_next_read: AtomicBool::new(true),
        });
        let mut service = (*self.feed_service).clone();
        service.friend_client = friend_client;
        (GetFeedUseCase::new(Arc::new(service)), barrier)
    }

    pub fn set_relationship_policy(
        &self,
        viewer_id: Uuid,
        target_id: Uuid,
        policy: RelationshipPolicySnapshot,
    ) {
        self.static_friend_client
            .set_relationship_policy(viewer_id, target_id, policy);
    }

    pub async fn notification_requests(&self) -> Vec<NotificationRequest> {
        self.notifications.snapshot().await
    }

    pub fn set_notification_service_available(&self, available: bool) {
        self.notifications
            .fail_requests
            .store(!available, Ordering::SeqCst);
    }

    pub fn outbox_forwarder(&self) -> ExternalEventForwarder {
        self.outbox_forwarder_with_publisher(self.publisher.clone())
    }

    pub fn fenced_replay_delivery(
        &self,
        config: post_service::application::dto::delivery_schedule_dto::DeliveryExecutionConfigDto,
    ) -> Result<post_service::adapters::r#in::messaging::fenced_replay_delivery::FencedReplayDelivery>
    {
        post_service::adapters::r#in::messaging::fenced_replay_delivery::FencedReplayDelivery::new(
            Arc::new(post_service::adapters::out::persistence::delivery_schedule::PgDeliverySchedule::new(self.pool.clone())),
            self.publisher.clone(), self.dispatcher.clone(), config,
        )
    }

    pub fn outbox_forwarder_with_publisher(
        &self,
        publisher: Arc<dyn ExternalEventPublisherPort>,
    ) -> ExternalEventForwarder {
        ExternalEventForwarder::new(
            Arc::new(post_service::adapters::out::persistence::delivery_schedule::PgDeliverySchedule::new(self.pool.clone())),
            publisher,
            self.dispatcher.clone(),
            self.feed_cache.clone(),
            self.post_cache.clone(),
            self.friend_client.clone(),
        Arc::new(post_service::adapters::out::persistence::timeline_projection_repository::PgTimelineProjectionRepository::new(self.pool.clone())),
        )
    }

    pub async fn feed_cache_key_exists(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<bool> {
        let mut connection = self.redis_client.get_multiplexed_async_connection().await?;
        let generation: Option<u64> = connection.get(feed_cache_generation_key(user_id)).await?;
        let key = feed_cache_key(user_id, generation.unwrap_or(0), cursor, limit);
        let exists: bool = connection.exists(key).await?;
        Ok(exists)
    }

    pub async fn post_cache_key_exists(&self, post_id: Uuid) -> Result<bool> {
        self.redis_key_exists(&format!("post:{post_id}")).await
    }

    pub async fn metrics(&self) -> Result<String> {
        let response = http::metrics_router(self.state.clone())
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

    async fn redis_key_exists(&self, key: &str) -> Result<bool> {
        let mut connection = self.redis_client.get_multiplexed_async_connection().await?;
        let exists: bool = connection.exists(key).await?;
        Ok(exists)
    }
}

#[derive(Default)]
pub struct FeedCacheWriteBarrier {
    reached: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

#[derive(Default)]
pub struct FeedRelationshipPolicyReadBarrier {
    reached: tokio::sync::Notify,
    release: tokio::sync::Notify,
}

impl FeedRelationshipPolicyReadBarrier {
    pub async fn wait_until_reached(&self) {
        self.reached.notified().await;
    }

    pub fn release(&self) {
        self.release.notify_one();
    }
}

struct BarrierFriendClient {
    inner: Arc<dyn FriendClientPort>,
    barrier: Arc<FeedRelationshipPolicyReadBarrier>,
    block_next_read: AtomicBool,
}

#[async_trait]
impl FriendClientPort for BarrierFriendClient {
    async fn fetch_feed_authors(
        &self,
        viewer_id: Uuid,
    ) -> Result<post_service::application::feed_authors::FeedAuthorsDto> {
        self.inner.fetch_feed_authors(viewer_id).await
    }

    async fn fetch_following_ids(&self, user_id: Uuid) -> Result<Vec<Uuid>> {
        self.inner.fetch_following_ids(user_id).await
    }

    async fn fetch_follower_ids(&self, user_id: Uuid) -> Result<Vec<Uuid>> {
        self.inner.fetch_follower_ids(user_id).await
    }

    async fn is_following(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool> {
        self.inner.is_following(follower_id, followee_id).await
    }

    async fn relationship_policy(
        &self,
        viewer_id: Uuid,
        target_id: Uuid,
        _stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<RelationshipPolicySnapshot> {
        self.inner
            .relationship_policy(viewer_id, target_id, _stage)
            .await
    }

    async fn batch_relationship_policies(
        &self,
        viewer_id: Uuid,
        target_ids: &[Uuid],
        _stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<HashMap<Uuid, RelationshipPolicySnapshot>> {
        if self.block_next_read.swap(false, Ordering::SeqCst) {
            self.barrier.reached.notify_one();
            self.barrier.release.notified().await;
        }
        self.inner
            .batch_relationship_policies(viewer_id, target_ids, _stage)
            .await
    }
}

impl FeedCacheWriteBarrier {
    pub async fn wait_until_reached(&self) {
        self.reached.notified().await;
    }

    pub fn release(&self) {
        self.release.notify_one();
    }
}

struct BarrierFeedCache {
    inner: Arc<dyn FeedCachePort>,
    barrier: Arc<FeedCacheWriteBarrier>,
    block_next_write: AtomicBool,
}

#[async_trait]
impl FeedCachePort for BarrierFeedCache {
    async fn get_feed(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<Option<post_service::domain::feed::FeedCachePage>> {
        self.inner.get_feed(user_id, cursor, limit).await
    }

    async fn set_feed(
        &self,
        user_id: Uuid,
        cursor: Option<&str>,
        limit: i32,
        page: &post_service::domain::feed::FeedCachePage,
        generation: u64,
    ) -> Result<()> {
        if self.block_next_write.swap(false, Ordering::SeqCst) {
            self.barrier.reached.notify_one();
            self.barrier.release.notified().await;
        }
        self.inner
            .set_feed(user_id, cursor, limit, page, generation)
            .await
    }

    async fn current_generation(&self, user_id: Uuid) -> Result<u64> {
        self.inner.current_generation(user_id).await
    }

    async fn invalidate_users(&self, user_ids: &[Uuid]) -> Result<()> {
        self.inner.invalidate_users(user_ids).await
    }
}

/// Native override is restricted to the controller-owned loopback cluster and
/// a fresh Redis database. Docker fixtures remain the default.
async fn fixture_backends() -> Result<(
    Option<PostgresContainer>,
    String,
    Option<RedisFixture>,
    String,
)> {
    let Ok(parent_url) = std::env::var("POST_TEST_DATABASE_URL") else {
        let postgres = test_support::postgres::start_postgres().await?;
        let redis = RedisFixture::new().await?;
        let database_url = format!(
            "postgres://postgres:postgres@{}:{}/postgres",
            postgres.get_host().await?,
            postgres.get_host_port_ipv4(5432).await?
        );
        let redis_url = format!(
            "redis://{}:{}/",
            redis.get_host().await?,
            redis.get_host_port_ipv4(6379).await?
        );
        return Ok((Some(postgres), database_url, Some(redis), redis_url));
    };
    use sqlx::postgres::PgConnectOptions;
    use std::net::IpAddr;
    let options: PgConnectOptions = parent_url.parse()?;
    let host = options
        .get_host()
        .trim_start_matches('[')
        .trim_end_matches(']');
    anyhow::ensure!(
        host == "localhost" || host.parse::<IpAddr>().is_ok_and(|ip| ip.is_loopback()),
        "Post native fixture requires loopback"
    );
    anyhow::ensure!(
        options.get_socket().is_none()
            && options.get_options().is_none()
            && !parent_url.contains('?')
            && !parent_url.contains('#')
            && options
                .get_database()
                .is_some_and(|db| db.starts_with("port_rust_post_test_")),
        "owned Post parent database required without overrides"
    );
    let admin = PgPool::connect(&parent_url).await?;
    let lineage: String =
        sqlx::query_scalar("select system_identifier::text from pg_control_system()")
            .fetch_one(&admin)
            .await?;
    anyhow::ensure!(
        std::env::var("POST_TEST_INITDB_SYSTEM_IDENTIFIER").as_deref() == Ok(lineage.as_str()),
        "observed Post initdb lineage differs"
    );
    if std::env::var_os("POST_TEST_REDIS_FIXTURE_ROOT").is_some() {
        let redis = RedisFixture::new().await?;
        let redis_url = format!(
            "redis://{}:{}/0",
            redis.get_host().await?,
            redis.get_host_port_ipv4(6379).await?
        );
        let mut connection = RedisClient::open(redis_url.clone())?
            .get_multiplexed_async_connection()
            .await?;
        let keys: u64 = redis::cmd("DBSIZE").query_async(&mut connection).await?;
        anyhow::ensure!(keys == 0, "fresh owned Redis must be empty");
        let database = format!("port_rust_post_fixture_{}", Uuid::new_v4().simple());
        sqlx::query(&format!("create database {database} template template0"))
            .execute(&admin)
            .await?;
        admin.close().await;
        let database_url = format!(
            "{}/{}",
            parent_url
                .rsplit_once('/')
                .ok_or_else(|| anyhow::anyhow!("fixture URL"))?
                .0,
            database
        );
        let probe = PgPool::connect(&database_url).await?;
        let empty: bool = sqlx::query_scalar(
            "select not exists(select 1 from pg_tables where schemaname='public')",
        )
        .fetch_one(&probe)
        .await?;
        anyhow::ensure!(empty, "fresh Post database must be empty before migrations");
        probe.close().await;
        println!(
            "POST_FRESH_FIXTURE {}",
            serde_json::json!({"database":database,"systemIdentifier":lineage,"template":"template0","emptyBeforeMigrations":true,"redisEmptyBeforeUse":true,"redisMode":"owned-per-fixture"})
        );
        return Ok((None, database_url, Some(redis), redis_url));
    }
    let redis_url = std::env::var("POST_TEST_REDIS_URL")?;
    anyhow::ensure!(
        redis_url.starts_with("redis://127.0.0.1:")
            && redis_url.ends_with("/0")
            && !redis_url.contains('?')
            && !redis_url.contains('#')
            && !redis_url.contains('@'),
        "owned Post Redis loopback database zero required"
    );
    let mut connection = RedisClient::open(redis_url.clone())?
        .get_multiplexed_async_connection()
        .await?;
    let info: String = redis::cmd("INFO")
        .arg("server")
        .query_async(&mut connection)
        .await?;
    let run_id = info.lines().find_map(|line| line.strip_prefix("run_id:"));
    anyhow::ensure!(
        run_id.is_some() && std::env::var("POST_TEST_REDIS_RUN_ID").ok().as_deref() == run_id,
        "observed Post Redis identity differs"
    );
    let keys: u64 = redis::cmd("DBSIZE").query_async(&mut connection).await?;
    anyhow::ensure!(
        keys == 0,
        "Post native Redis fixture must be empty; existing keys are never flushed"
    );
    let database = format!("port_rust_post_fixture_{}", Uuid::new_v4().simple());
    sqlx::query(&format!("create database {database} template template0"))
        .execute(&admin)
        .await?;
    admin.close().await;
    let database_url = format!(
        "{}/{}",
        parent_url
            .rsplit_once('/')
            .ok_or_else(|| anyhow::anyhow!("fixture URL"))?
            .0,
        database
    );
    let probe = PgPool::connect(&database_url).await?;
    let empty: bool =
        sqlx::query_scalar("select not exists(select 1 from pg_tables where schemaname='public')")
            .fetch_one(&probe)
            .await?;
    anyhow::ensure!(empty, "Post fixture must be empty before migrations");
    probe.close().await;
    println!(
        "POST_FRESH_FIXTURE {}",
        serde_json::json!({"database":database,"systemIdentifier":lineage,"template":"template0","emptyBeforeMigrations":true,"redisRunId":run_id,"redisEmptyBeforeUse":true})
    );
    Ok((None, database_url, None, redis_url))
}

pub async fn start_postgres_redis_app(seed_enabled: bool) -> Result<TestApp> {
    start_postgres_redis_app_with_pool_settings(seed_enabled, 5, std::time::Duration::from_secs(30))
        .await
}

pub async fn start_postgres_redis_app_with_pool_settings(
    seed_enabled: bool,
    database_pool_max_connections: u32,
    database_acquire_timeout: std::time::Duration,
) -> Result<TestApp> {
    start_postgres_redis_app_with_legacy_events(
        seed_enabled,
        database_pool_max_connections,
        database_acquire_timeout,
        &[],
    )
    .await
}

/// Seed retained legacy publications at migration 16, then run the actual
/// migration chain. The boolean selects an inherited INTERNAL checkpoint.
pub async fn start_postgres_redis_app_with_legacy_events(
    seed_enabled: bool,
    database_pool_max_connections: u32,
    database_acquire_timeout: std::time::Duration,
    legacy: &[(StoredEvent, bool)],
) -> Result<TestApp> {
    let (postgres, database_url, redis, redis_url) = fixture_backends().await?;

    let config = AppConfig {
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        http_addr: "127.0.0.1:18085".parse()?,
        metrics_addr: "127.0.0.1:19085".parse()?,
        grpc_addr: "127.0.0.1:15055".parse()?,
        database_pool_max_connections,
        database_pool_acquire_timeout_ms: u64::try_from(database_acquire_timeout.as_millis())?,
        database_url,
        redis_url,
        feed_cache_epoch: 1,
        friend_grpc_endpoint: String::new(),
        profile_grpc_endpoint: String::new(),
        notification_grpc_endpoint: String::new(),
        gateway_grpc_endpoint: String::new(),
        resource_evidence_signer:
            platform::actor_delegation::exchange::ResourceEvidenceSigner::local_parity(),
        seed_enabled,
        runtime_mode: RuntimeMode::Production,
        actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
            platform::grpc_auth::Workload::Post,
        ),
        caller_registry: test_caller_registry(),
        outbound_identity: "test-internal-service-token".into(),
        tls_material: None,
        outbox_delivery: Default::default(),
        outbox_worker_enabled: true,
        outbox_worker_interval_ms: 1000,
        outbox_worker_batch_size: 100,
        outbox_worker_min_event_age_ms: 0,
        // External delivery is stubbed with a recorder elsewhere in this file,
        // so the configuration states the disabled mode and the recorder is
        // still exercised as an enabled publisher.
        external_publisher: post_service::config::ExternalPublisherMode::Disabled,
        kafka_bootstrap_brokers: Vec::new(),
        kafka_topic: "post-events".into(),
        kafka_partition_count: 6,
        kafka_publish_timeout_ms: 5_000,
        kafka_connect_timeout_ms: 10_000,
        kafka_create_topic: false,
        retention_purge_worker_enabled: true,
        retention_purge_worker_interval_ms: 1000,
        retention_purge_actor_id: "99999999-9999-9999-9999-999999999999".into(),
        timeline_fanout_worker_enabled: true,
        timeline_fanout_worker_interval_ms: 1000,
        timeline_fanout_claim_limit: 100,
        timeline_follower_page_size: 100,
        timeline_fanout_lease_timeout_ms: 30_000,
        timeline_fanout_max_attempts: 5,
        timeline_fanout_base_retry_ms: 1000,
        timeline_entry_ttl_seconds: 2_592_000,
        high_fanout_follower_threshold: 10_000,
        feed_candidate_source_timeout_ms: TEST_FEED_CANDIDATE_SOURCE_TIMEOUT_MS,
        feed_candidate_per_source_limit: 200,
        feed_following_author_limit: 5_000,
        feed_snapshot_candidate_limit: 500,
        feed_snapshot_ttl_seconds: 900,
        feed_rank_version: "feed-linear-v1".into(),
        feed_ranker_adapter: "LINEAR".into(),
        feed_embedding_source_enabled: false,
        feed_recent_sequence_enabled: false,
        feed_recent_sequence_limit: 20,
        feed_retention_worker_enabled: false,
        feed_retention_worker_interval_ms: 3_600_000,
        feed_retention_batch_size: 500,
        feed_event_retention_days: 90,
    };
    let pool = connect_pool_with_legacy_events(&config, database_acquire_timeout, legacy).await?;

    let notifications = Arc::new(RecordingNotificationClient::default());
    build_postgres_redis_app(config, postgres, redis, pool, notifications).await
}

pub async fn start_postgres_redis_app_with_notification_failures(
    seed_enabled: bool,
) -> Result<TestApp> {
    let (postgres, database_url, redis, redis_url) = fixture_backends().await?;

    let config = AppConfig {
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        http_addr: "127.0.0.1:18085".parse()?,
        metrics_addr: "127.0.0.1:19085".parse()?,
        grpc_addr: "127.0.0.1:15055".parse()?,
        database_pool_max_connections: 5,
        database_pool_acquire_timeout_ms: 30_000,
        database_url,
        redis_url,
        feed_cache_epoch: 1,
        friend_grpc_endpoint: String::new(),
        profile_grpc_endpoint: String::new(),
        notification_grpc_endpoint: String::new(),
        gateway_grpc_endpoint: String::new(),
        resource_evidence_signer:
            platform::actor_delegation::exchange::ResourceEvidenceSigner::local_parity(),
        seed_enabled,
        runtime_mode: RuntimeMode::Production,
        actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
            platform::grpc_auth::Workload::Post,
        ),
        caller_registry: test_caller_registry(),
        outbound_identity: "test-internal-service-token".into(),
        tls_material: None,
        outbox_delivery: Default::default(),
        outbox_worker_enabled: true,
        outbox_worker_interval_ms: 1000,
        outbox_worker_batch_size: 100,
        outbox_worker_min_event_age_ms: 0,
        // External delivery is stubbed with a recorder elsewhere in this file,
        // so the configuration states the disabled mode and the recorder is
        // still exercised as an enabled publisher.
        external_publisher: post_service::config::ExternalPublisherMode::Disabled,
        kafka_bootstrap_brokers: Vec::new(),
        kafka_topic: "post-events".into(),
        kafka_partition_count: 6,
        kafka_publish_timeout_ms: 5_000,
        kafka_connect_timeout_ms: 10_000,
        kafka_create_topic: false,
        retention_purge_worker_enabled: true,
        retention_purge_worker_interval_ms: 1000,
        retention_purge_actor_id: "99999999-9999-9999-9999-999999999999".into(),
        timeline_fanout_worker_enabled: true,
        timeline_fanout_worker_interval_ms: 1000,
        timeline_fanout_claim_limit: 100,
        timeline_follower_page_size: 100,
        timeline_fanout_lease_timeout_ms: 30_000,
        timeline_fanout_max_attempts: 5,
        timeline_fanout_base_retry_ms: 1000,
        timeline_entry_ttl_seconds: 2_592_000,
        high_fanout_follower_threshold: 10_000,
        feed_candidate_source_timeout_ms: TEST_FEED_CANDIDATE_SOURCE_TIMEOUT_MS,
        feed_candidate_per_source_limit: 200,
        feed_following_author_limit: 5_000,
        feed_snapshot_candidate_limit: 500,
        feed_snapshot_ttl_seconds: 900,
        feed_rank_version: "feed-linear-v1".into(),
        feed_ranker_adapter: "LINEAR".into(),
        feed_embedding_source_enabled: false,
        feed_recent_sequence_enabled: false,
        feed_recent_sequence_limit: 20,
        feed_retention_worker_enabled: false,
        feed_retention_worker_interval_ms: 3_600_000,
        feed_retention_batch_size: 500,
        feed_event_retention_days: 90,
    };
    let pool = connect_pool_with_timeout(&config, std::time::Duration::from_secs(30)).await?;
    let notifications = Arc::new(RecordingNotificationClient::failing());
    build_postgres_redis_app(config, postgres, redis, pool, notifications).await
}

async fn connect_pool_with_timeout(
    config: &AppConfig,
    acquire_timeout: std::time::Duration,
) -> Result<PgPool> {
    connect_pool_with_legacy_events(config, acquire_timeout, &[]).await
}

/// Establish fixture readiness without changing the pool's runtime deadline.
/// SQLx uses acquire_timeout for cold handshakes too; a saturation test's short
/// deadline must remain unchanged once setup completes. Only initial timed-out
/// acquisitions retry, within the same 30-second fixture startup window.
pub async fn connect_ready_fixture_pool(options: PgPoolOptions, url: &str) -> Result<PgPool> {
    let pool = options.connect_lazy(url)?;
    let startup_window = std::time::Duration::from_secs(30);
    let deadline = tokio::time::Instant::now() + startup_window;
    let mut timeouts = 0u32;
    loop {
        if tokio::time::Instant::now() >= deadline {
            pool.close().await;
            anyhow::bail!(
                "Post fixture pool was not ready within 30 seconds; timed_out_attempts={timeouts}"
            );
        }
        match tokio::time::timeout_at(deadline, pool.acquire()).await {
            Ok(Ok(connection)) => {
                drop(connection);
                // timeout_at polls the inner future first. A late executor poll
                // must not accept readiness after the setup deadline.
                if tokio::time::Instant::now() >= deadline {
                    pool.close().await;
                    anyhow::bail!("Post fixture pool became ready after the 30-second startup deadline; timed_out_attempts={timeouts}");
                }
                if timeouts > 0 {
                    eprintln!("POST_FIXTURE_POOL_READY timed_out_attempts={timeouts} acquire_timeout_ms={} max_connections={}",
                        pool.options().get_acquire_timeout().as_millis(), pool.options().get_max_connections());
                }
                return Ok(pool);
            }
            Ok(Err(sqlx::Error::PoolTimedOut)) => {
                timeouts += 1;
                eprintln!("POST_FIXTURE_POOL_STARTUP_RETRY timed_out_attempts={timeouts} acquire_timeout_ms={} max_connections={}",
                    pool.options().get_acquire_timeout().as_millis(), pool.options().get_max_connections());
            }
            Ok(Err(error)) => {
                pool.close().await;
                return Err(error.into());
            }
            Err(_) => {
                pool.close().await;
                anyhow::bail!("Post fixture pool was not ready within 30 seconds; timed_out_attempts={timeouts}");
            }
        }
    }
}

async fn connect_pool_with_legacy_events(
    config: &AppConfig,
    acquire_timeout: std::time::Duration,
    legacy: &[(StoredEvent, bool)],
) -> Result<PgPool> {
    let pool = connect_ready_fixture_pool(
        PgPoolOptions::new()
            .max_connections(config.database_pool_max_connections)
            .acquire_timeout(acquire_timeout),
        &config.database_url,
    )
    .await?;
    // This support module is also compiled into api-gateway's owner-replay
    // acceptance test, so resolve PostService's sibling manifest explicitly.
    let migrations = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../post-service/migrations");
    let mut migrator = sqlx::migrate::Migrator::new(migrations.as_path()).await?;
    if !legacy.is_empty() {
        let all = migrator.migrations.clone();
        migrator.migrations = all
            .iter()
            .filter(|m| m.version <= 16)
            .cloned()
            .collect::<Vec<_>>()
            .into();
        migrator.run(&pool).await?;
        for (event, checkpoint) in legacy {
            anyhow::ensure!(
                event.published && event.published_at.is_some(),
                "Expected retained legacy publication"
            );
            PgEventOutbox::new(pool.clone()).save(event).await?;
            if *checkpoint {
                sqlx::query("insert into event_publications(event_id,target,published_at,payload) values($1,'INTERNAL',clock_timestamp(),$2)")
                    .bind(event.event_id).bind(&event.payload).execute(&pool).await?;
            }
        }
        migrator.migrations = all;
    }
    migrator.run(&pool).await?;
    Ok(pool)
}

async fn build_postgres_redis_app(
    config: AppConfig,
    postgres: Option<PostgresContainer>,
    redis: Option<RedisFixture>,
    pool: PgPool,
    notifications: Arc<RecordingNotificationClient>,
) -> Result<TestApp> {
    let friend_client = Arc::new(StaticFriendClient::default());
    let friend_port: Arc<dyn FriendClientPort> = friend_client.clone();
    let profile_port: Arc<dyn ProfileClientPort> = Arc::new(StaticProfileClient);
    let notification_port: Arc<dyn NotificationClientPort> = notifications.clone();
    let outbox: Arc<dyn EventOutboxPort> = Arc::new(PgEventOutbox::new(pool.clone()));
    // The integration flows assert delivery ordering and checkpoint progress,
    // not broker transport, so they run against the recorder. External delivery
    // is declared enabled so the target is exercised like it is in production.
    let publisher: Arc<dyn ExternalEventPublisherPort> =
        Arc::new(RecordingEventPublisher::default());
    let dispatcher = Arc::new(InternalNotificationListener::new(
        Arc::new(PgPostRepository::new(pool.clone())),
        friend_port.clone(),
        notification_port,
    ));

    let redis_client = RedisClient::open(config.redis_url.clone())?;
    let cache_connection =
        post_service::adapters::out::cache::connect_manager(redis_client.clone()).await?;
    let feed_cache: Arc<dyn FeedCachePort> =
        Arc::new(RedisFeedCache::with_connection(cache_connection.clone()));
    let post_cache: Arc<dyn PostCachePort> =
        Arc::new(RedisPostCache::with_connection(cache_connection));
    let post_repository: Arc<dyn PostRepositoryPort> =
        Arc::new(PgPostRepository::new(pool.clone()));
    let like_repository: Arc<dyn PostLikeRepositoryPort> =
        Arc::new(PgPostLikeRepository::new(pool.clone()));
    let feed_service = Arc::new(PostApplicationService {
        posts: post_repository.clone(),
        query_witnesses: Arc::new(UnforwardedQueryFixture),
        likes: like_repository.clone(),
        feed_cache: feed_cache.clone(),
        post_cache: post_cache.clone(),
        friend_client: friend_port.clone(),
        profile_client: profile_port.clone(),
        outbox: outbox.clone(),
        replay_delivery: Arc::new(post_service::adapters::r#in::messaging::fenced_replay_delivery::FencedReplayDelivery::new(
            Arc::new(post_service::adapters::out::persistence::delivery_schedule::PgDeliverySchedule::new(pool.clone())),publisher.clone(),dispatcher.clone(),Default::default())?),
        publisher: publisher.clone(),
        internal_dispatcher: dispatcher.clone(),
    });
    let chronological_feed = GetFeedUseCase::new(feed_service.clone());
    let recommendation_config = RecommendationRuntimeConfig {
        source_timeout: std::time::Duration::from_millis(TEST_FEED_CANDIDATE_SOURCE_TIMEOUT_MS),
        per_source_limit: 200,
        following_author_limit: 5_000,
        snapshot_candidate_limit: 500,
        snapshot_ttl_seconds: 900,
        rank_version: "feed-linear-v1".into(),
        recent_sequence_enabled: false,
        recent_sequence_limit: 20,
        high_fanout_threshold: 10_000,
        retention_worker_enabled: false,
        retention_worker_interval_ms: 3_600_000,
        retention_batch_size: 500,
        event_retention_days: 90,
        fanout: TimelineFanoutConfig {
            claim_limit: 100,
            follower_page_size: 100,
            lease_timeout: std::time::Duration::from_secs(30),
            max_attempts: 5,
            base_retry: std::time::Duration::from_secs(1),
            entry_ttl: std::time::Duration::from_secs(2_592_000),
        },
    };
    let state = Arc::new(
        build_state_with_dependencies(
            pool.clone(),
            config.seed_enabled,
            config.retention_purge_actor_id.clone(),
            post_repository,
            Arc::new(UnforwardedQueryFixture),
            like_repository,
            feed_cache.clone(),
            post_cache.clone(),
            friend_port.clone(),
            profile_port,
            outbox.clone(),
            publisher.clone(),
            dispatcher.clone(),
            recommendation_config.clone(),
        )
        .await?,
    );

    Ok(TestApp {
        _postgres: postgres,
        _redis: redis,
        pool,
        state,
        chronological_feed,
        feed_service,
        notifications,
        redis_client,
        outbox,
        publisher,
        dispatcher,
        feed_cache,
        post_cache,
        friend_client: friend_port,
        static_friend_client: friend_client,
        recommendation_config,
    })
}

pub fn grpc_server(state: Arc<AppState>) -> PostGrpcServer {
    PostGrpcServer::new(state)
}

#[derive(Clone)]
struct StaticFriendClient {
    following: Arc<HashMap<Uuid, Vec<Uuid>>>,
    followers: Arc<HashMap<Uuid, Vec<Uuid>>>,
    policies: Arc<std::sync::Mutex<HashMap<(Uuid, Uuid), RelationshipPolicySnapshot>>>,
}

impl StaticFriendClient {
    fn set_relationship_policy(
        &self,
        viewer_id: Uuid,
        target_id: Uuid,
        policy: RelationshipPolicySnapshot,
    ) {
        self.policies
            .lock()
            .expect("relationship policies")
            .insert((viewer_id, target_id), policy);
    }
}

impl Default for StaticFriendClient {
    fn default() -> Self {
        let user_111 = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid");
        let user_222 = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");
        let user_333 = Uuid::parse_str("33333333-3333-3333-3333-333333333333").expect("uuid");
        let user_444 = Uuid::parse_str("44444444-4444-4444-4444-444444444444").expect("uuid");

        let following = HashMap::from([
            (user_111, vec![user_222, user_333]),
            (user_222, vec![user_333]),
            (user_333, vec![]),
            (user_444, vec![]),
        ]);
        let followers = HashMap::from([
            (user_222, vec![user_111]),
            (user_333, vec![user_111, user_222]),
            (user_111, vec![]),
            (user_444, vec![]),
        ]);

        Self {
            following: Arc::new(following),
            followers: Arc::new(followers),
            policies: Arc::new(std::sync::Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl FriendClientPort for StaticFriendClient {
    async fn fetch_feed_authors(
        &self,
        viewer_id: Uuid,
    ) -> Result<post_service::application::feed_authors::FeedAuthorsDto> {
        let ids = self.fetch_following_ids(viewer_id).await?;
        let mut eligible = Vec::new();
        for id in ids {
            let policy = self.relationship_policy(viewer_id, id, None).await?;
            if policy.following && !policy.is_blocked() && !policy.muted_by_viewer {
                eligible.push(id);
            }
        }
        post_service::application::feed_authors::FeedAuthorsDto::new(viewer_id, eligible)
    }

    async fn fetch_following_ids(&self, user_id: Uuid) -> Result<Vec<Uuid>> {
        Ok(self.following.get(&user_id).cloned().unwrap_or_default())
    }

    async fn fetch_follower_ids(&self, user_id: Uuid) -> Result<Vec<Uuid>> {
        Ok(self.followers.get(&user_id).cloned().unwrap_or_default())
    }

    async fn is_following(&self, follower_id: Uuid, followee_id: Uuid) -> Result<bool> {
        Ok(self
            .following
            .get(&follower_id)
            .map(|ids| ids.contains(&followee_id))
            .unwrap_or(false))
    }

    async fn relationship_policy(
        &self,
        viewer_id: Uuid,
        target_id: Uuid,
        _stage: Option<&PublishedWitnessStageDto>,
    ) -> Result<RelationshipPolicySnapshot> {
        if let Some(policy) = self
            .policies
            .lock()
            .expect("relationship policies")
            .get(&(viewer_id, target_id))
            .copied()
        {
            return Ok(policy);
        }
        Ok(RelationshipPolicySnapshot {
            following: self.is_following(viewer_id, target_id).await?,
            ..RelationshipPolicySnapshot::default()
        })
    }
}

#[derive(Clone, Default)]
struct StaticProfileClient;

#[async_trait]
impl ProfileClientPort for StaticProfileClient {
    async fn require_author(
        &self,
        user_id: Uuid,
    ) -> Result<post_service::application::profile_author::ExistingProfileAuthorDto> {
        post_service::application::profile_author::ExistingProfileAuthorDto::new(user_id)
    }
}

#[derive(Clone, Default)]
struct RecordingEventPublisher {
    published: Arc<Mutex<Vec<uuid::Uuid>>>,
}

#[async_trait]
impl ExternalEventPublisherPort for RecordingEventPublisher {
    async fn publish(&self, event: &StoredEvent) -> Result<()> {
        self.published.lock().await.push(event.event_id);
        Ok(())
    }
}

#[derive(Clone, Default)]
struct RecordingNotificationClient {
    requests: Arc<Mutex<Vec<NotificationRequest>>>,
    fail_requests: Arc<AtomicBool>,
}

impl RecordingNotificationClient {
    fn failing() -> Self {
        Self {
            requests: Arc::default(),
            fail_requests: Arc::new(AtomicBool::new(true)),
        }
    }

    async fn snapshot(&self) -> Vec<NotificationRequest> {
        self.requests.lock().await.clone()
    }
}

#[async_trait]
impl NotificationClientPort for RecordingNotificationClient {
    async fn create_notification(&self, request: NotificationRequest) -> Result<()> {
        if self.fail_requests.load(Ordering::SeqCst) {
            anyhow::bail!("notification service unavailable");
        }
        self.requests.lock().await.push(request);
        Ok(())
    }
}
