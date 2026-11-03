#[path = "../support/query_witness.rs"]
mod query_witness;
use chrono::{DateTime, TimeZone, Utc};
use post_service::adapters::out::persistence::content_moderation_repository::PgContentModerationRepository;
use post_service::adapters::out::persistence::feed_recommendation_repository::PgFeedRecommendationRepository;
use post_service::adapters::out::persistence::user_lifecycle_repository::PgUserLifecycleRepository;
use post_service::adapters::r#in::messaging::external_event_forwarder::ExternalEventForwarder;
use post_service::adapters::r#in::messaging::outbox_worker::PostOutboxWorker;
use post_service::adapters::r#in::messaging::recommendation_retention_worker::FeedRecommendationRetentionWorker;
use post_service::adapters::r#in::messaging::retention_purge_worker::PostRetentionPurgeWorker;
use post_service::adapters::r#in::messaging::timeline_fanout_worker::TimelineFanoutWorker;
use post_service::application::use_cases::{
    apply_content_moderation::ApplyContentModerationUseCase,
    apply_user_deletion::ApplyUserDeletionUseCase,
    create_comment::CreateCommentUseCase,
    create_post::CreatePostUseCase,
    delete_comment::DeleteCommentUseCase,
    delete_post::DeletePostUseCase,
    get_author_post_counts::GetAuthorPostCountsUseCase,
    get_author_posts::GetAuthorPostsUseCase,
    get_feed::GetFeedUseCase,
    get_mutation_receipt_status::GetMutationReceiptStatusUseCase,
    get_post_detail::GetPostDetailUseCase,
    get_trending::GetTrendingUseCase,
    like_post::LikePostUseCase,
    process_timeline_fanout::{ProcessTimelineFanoutUseCase, TimelineFanoutConfig},
    purge_expired_posts::PurgeExpiredPostsUseCase,
    record_feed_recommendation_event::RecordFeedRecommendationEventUseCase,
    replay_events::ReplayEventsUseCase,
    resolve_dead_letter_replay::ResolveDeadLetterReplayUseCase,
    search_posts::SearchPostsUseCase,
    unlike_post::UnlikePostUseCase,
    PostApplicationService,
};
use post_service::config::{AppState, PostMetrics};
use post_service::domain::comment::Comment;
use post_service::domain::post::{Post, StoredEvent};
pub use query_witness::UnforwardedQueryFixture;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use uuid::Uuid;

#[path = "support_adapters.rs"]
mod support_adapters;
#[path = "support_likes.rs"]
mod support_likes;
#[path = "support_repositories.rs"]
mod support_repositories;
pub use support_adapters::*;
pub use support_likes::*;
pub use support_repositories::*;

pub const ACTOR_ID: &str = "99999999-9999-9999-9999-999999999999";

pub fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("valid uuid")
}

pub fn instant(seconds: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(seconds, 0)
        .single()
        .expect("valid instant")
}

pub fn post(id: Uuid, author_id: Uuid, content: &str) -> Post {
    Post {
        id,
        author_id,
        content: content.to_owned(),
        visibility: "PUBLIC".to_owned(),
        created_at: instant(1_700_000_000),
        comment_count: 0,
        like_count: 0,
    }
}

pub fn comment(id: Uuid, post_id: Uuid, author_id: Uuid, content: &str) -> Comment {
    Comment {
        id,
        post_id,
        author_id,
        content: content.to_owned(),
        created_at: instant(1_700_000_100),
    }
}

pub fn stored_event(event_id: Uuid, event_type: &str, payload: String) -> StoredEvent {
    let value: serde_json::Value = serde_json::from_str(&payload).unwrap_or_default();
    let field = |name: &str| {
        value
            .get(name)
            .and_then(serde_json::Value::as_str)
            .and_then(|v| Uuid::parse_str(v).ok())
    };
    StoredEvent {
        event_id,
        event_type: event_type.to_owned(),
        aggregate_id: field("postId")
            .unwrap_or_else(|| uuid("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa")),
        aggregate_version: None,
        origin_traceparent: None,
        actor_id: field("actorId")
            .or_else(|| field("authorId"))
            .or_else(|| Some(uuid("bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"))),
        attributes: vec!["unit".to_owned()],
        payload,
        occurred_at: instant(1_700_000_200),
        created_at: instant(1_700_000_200),
        published: false,
        published_at: None,
        replay_resolution_id: None,
    }
}

pub struct Harness {
    pub service: Arc<PostApplicationService>,
    pub posts: Arc<FakePostRepository>,
    pub likes: Arc<FakeLikeRepository>,
    pub feed_cache: Arc<FakeFeedCache>,
    pub post_cache: Arc<FakePostCache>,
    pub friend: Arc<FakeFriendClient>,
    pub profile: Arc<FakeProfileClient>,
    pub outbox: Arc<FakeOutbox>,
    pub publisher: Arc<SpyPublisher>,
    pub internal: Arc<SpyInternalDispatcher>,
    pub metrics: Arc<PostMetrics>,
}

impl Harness {
    pub fn new() -> Self {
        let posts = Arc::new(FakePostRepository::default());
        let likes = Arc::new(FakeLikeRepository::default());
        let feed_cache = Arc::new(FakeFeedCache::default());
        let post_cache = Arc::new(FakePostCache::default());
        let friend = Arc::new(FakeFriendClient::default());
        let profile = Arc::new(FakeProfileClient::default());
        let outbox = Arc::new(FakeOutbox::default());
        let publisher = Arc::new(SpyPublisher::default());
        let internal = Arc::new(SpyInternalDispatcher::default());
        let metrics = Arc::new(PostMetrics::default());
        let service = Arc::new(PostApplicationService {
            posts: posts.clone(),
            query_witnesses: Arc::new(UnforwardedQueryFixture),
            likes: likes.clone(),
            feed_cache: feed_cache.clone(),
            post_cache: post_cache.clone(),
            friend_client: friend.clone(),
            profile_client: profile.clone(),
            outbox: outbox.clone(),
            replay_delivery: Arc::new(RecordingReplayDelivery {
                publisher: publisher.clone(),
                internal: internal.clone(),
            }),
            publisher: publisher.clone(),
            internal_dispatcher: internal.clone(),
        });
        Self {
            service,
            posts,
            likes,
            feed_cache,
            post_cache,
            friend,
            profile,
            outbox,
            publisher,
            internal,
            metrics,
        }
    }

    pub fn state(&self) -> Arc<AppState> {
        let metrics = self.metrics.clone();
        let purge = PurgeExpiredPostsUseCase::new(self.service.clone());
        let forwarder = ExternalEventForwarder::new(
            self.outbox.clone(),
            self.publisher.clone(),
            self.internal.clone(),
            self.feed_cache.clone(),
            self.post_cache.clone(),
            self.friend.clone(),
            Arc::new(RecordingTimelineProjection::default()),
        )
        .with_delivery_metrics(metrics.clone());
        let lifecycle_pool = PgPoolOptions::new()
            .connect_lazy("postgres://postgres:postgres@localhost:5432/post_unit")
            .expect("lazy lifecycle pool");
        let feed_repository = Arc::new(PgFeedRecommendationRepository::new(lifecycle_pool.clone()));
        let timeline_fanout_worker = TimelineFanoutWorker::new(
            ProcessTimelineFanoutUseCase::new(
                feed_repository.clone(),
                self.friend.clone(),
                TimelineFanoutConfig {
                    claim_limit: 100,
                    follower_page_size: 100,
                    lease_timeout: std::time::Duration::from_secs(30),
                    max_attempts: 5,
                    base_retry: std::time::Duration::from_secs(1),
                    entry_ttl: std::time::Duration::from_secs(2_592_000),
                },
            ),
            metrics.clone(),
        );
        Arc::new(AppState {
            get_author_post_counts: GetAuthorPostCountsUseCase::new(self.service.clone()),
            apply_content_moderation: ApplyContentModerationUseCase::new(
                Arc::new(PgContentModerationRepository::new(lifecycle_pool.clone())),
                self.post_cache.clone(),
            ),
            apply_user_deletion: ApplyUserDeletionUseCase::new(
                Arc::new(PgUserLifecycleRepository::new(lifecycle_pool)),
                self.post_cache.clone(),
            ),
            create_post: CreatePostUseCase::new(self.service.clone()),
            create_comment: CreateCommentUseCase::new(self.service.clone()),
            delete_comment: DeleteCommentUseCase::new(self.service.clone()),
            delete_post: DeletePostUseCase::new(self.service.clone()),
            get_feed: GetFeedUseCase::new(self.service.clone()),
            get_mutation_receipt_status: GetMutationReceiptStatusUseCase::new(
                self.service.posts.clone(),
            ),
            get_author_posts: GetAuthorPostsUseCase::new(self.service.clone()),
            search_posts: SearchPostsUseCase::new(self.service.clone()),
            get_post_detail: GetPostDetailUseCase::new(self.service.clone()),
            authorize_notification_posts: post_service::application::use_cases::authorize_notification_posts::AuthorizeNotificationPostsUseCase::new(self.service.clone()),
            like_post: LikePostUseCase::new(self.service.clone()),
            unlike_post: UnlikePostUseCase::new(self.service.clone()),
            get_trending: GetTrendingUseCase::new(self.service.clone()),
            replay_events: ReplayEventsUseCase::new(self.outbox.clone(), self.service.replay_delivery.clone()),
            resolve_dead_letter_replay: ResolveDeadLetterReplayUseCase::new(self.outbox.clone()),
            record_feed_recommendation_event: RecordFeedRecommendationEventUseCase::new(
                feed_repository.clone(),
            ),
            purge_expired_posts: purge.clone(),
            outbox: self.outbox.clone(),
            outbox_worker: PostOutboxWorker::new(forwarder, metrics.clone()),
            retention_purge_worker: PostRetentionPurgeWorker::new(
                purge,
                metrics.clone(),
                ACTOR_ID.to_owned(),
            ),
            timeline_fanout_worker,
            recommendation_retention_worker: FeedRecommendationRetentionWorker::new(
                feed_repository,
                metrics.clone(),
                chrono::Duration::days(90),
                500,
            ),
            metrics,
        })
    }
}

struct RecordingReplayDelivery {
    publisher: Arc<SpyPublisher>,
    internal: Arc<SpyInternalDispatcher>,
}
#[async_trait::async_trait]
impl post_service::application::ports::ReplayEventDeliveryPort for RecordingReplayDelivery {
    async fn replay_target(
        &self,
        event_id: Uuid,
        target: post_service::domain::post::DeliveryTarget,
    ) -> anyhow::Result<()> {
        match target {
            post_service::domain::post::DeliveryTarget::Internal => {
                self.internal.dispatched.lock().unwrap().push(event_id)
            }
            post_service::domain::post::DeliveryTarget::External => {
                self.publisher.published.lock().unwrap().push(event_id)
            }
            _ => anyhow::bail!("unsupported unit replay target"),
        }
        Ok(())
    }
}

/// Explicit recording boundary for tests of other delivery targets. These tests
/// do not exercise the TIMELINE database semantics; native projection probes do.
#[derive(Default)]
pub struct RecordingTimelineProjection {
    pub applied: std::sync::Mutex<
        Vec<post_service::application::dto::timeline_projection_dto::TimelineProjectionApplyDto>,
    >,
}
#[async_trait::async_trait]
impl post_service::application::ports::TimelineProjectionRepositoryPort
    for RecordingTimelineProjection
{
    async fn prepare(
        &self,
        event: &post_service::application::dto::timeline_projection_dto::TimelineProjectionEventDto,
        _limit: i32,
    ) -> anyhow::Result<
        post_service::application::dto::timeline_projection_dto::TimelineProjectionPlanDto,
    > {
        use post_service::application::dto::timeline_projection_dto::*;
        Ok(TimelineProjectionPlanDto::Events(vec![
            TimelineProjectionWorkDto {
                event: event.clone(),
                effect: TimelineProjectionEffectDto::Noop,
            },
        ]))
    }
    async fn apply(
        &self,
        request: &post_service::application::dto::timeline_projection_dto::TimelineProjectionApplyDto,
    ) -> anyhow::Result<
        post_service::application::dto::timeline_projection_dto::TimelineProjectionOutcomeDto,
    > {
        self.applied.lock().unwrap().push(request.clone());
        Ok(post_service::application::dto::timeline_projection_dto::TimelineProjectionOutcomeDto::Applied)
    }
}
