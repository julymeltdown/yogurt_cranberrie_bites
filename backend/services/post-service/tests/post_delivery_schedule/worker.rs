use super::Fixture;
use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;
use post_service::adapters::out::persistence::delivery_schedule::PgDeliverySchedule;
use post_service::adapters::r#in::messaging::external_event_forwarder::ExternalEventForwarder;
use post_service::adapters::r#in::messaging::outbox_worker::PostOutboxWorker;
use post_service::application::delivery_effect::DeliveryEffectFailure;
use post_service::application::dto::delivery_schedule_dto::DeliveryExecutionConfigDto;
use post_service::application::dto::delivery_schedule_dto::{
    DeliveryLeaseOutcomeDto, DeliveryLeaseStatusDto, DeliveryScheduleConfigDto,
    TargetDeliveryBatchDto, TargetDeliveryClaimDto,
};
use post_service::application::ports::DeliverySchedulePort;
use post_service::application::ports::{ExternalEventPublisherPort, InternalEventDispatcherPort};
use post_service::config::PostMetrics;
use post_service::domain::post::{DeliveryTarget, StoredEvent};
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use std::time::Duration;
use tokio::sync::Notify;
use uuid::Uuid;

use super::ports;

#[derive(Default)]
struct Publisher {
    calls: AtomicUsize,
    entered: Notify,
    release: Notify,
    block: bool,
    failure: Option<Failure>,
    accepted: AtomicUsize,
}

#[derive(Clone, Copy)]
enum Failure {
    InnerTimeout,
    LostAcknowledgement,
    Confirmed,
    Unclassified,
}

#[async_trait]
impl ExternalEventPublisherPort for Publisher {
    async fn publish(&self, _event: &StoredEvent) -> Result<()> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.entered.notify_one();
        if let Some(failure) = self.failure {
            let error = anyhow::anyhow!("injected adapter result");
            return Err(match failure {
                Failure::InnerTimeout => {
                    tokio::time::sleep(Duration::from_millis(20)).await;
                    DeliveryEffectFailure::uncertain(error)
                }
                Failure::LostAcknowledgement => {
                    self.accepted.fetch_add(1, Ordering::SeqCst);
                    DeliveryEffectFailure::uncertain(error)
                }
                Failure::Confirmed => DeliveryEffectFailure::confirmed(error),
                Failure::Unclassified => error,
            });
        }
        if self.block {
            self.release.notified().await;
        }
        Ok(())
    }
}

#[derive(Default)]
struct Notifications {
    events: Mutex<Vec<Uuid>>,
}

#[async_trait]
impl InternalEventDispatcherPort for Notifications {
    async fn dispatch(&self, event: &StoredEvent) -> Result<()> {
        self.events.lock().unwrap().push(event.event_id);
        Ok(())
    }
}

fn forwarder(
    f: &Fixture,
    publisher: Arc<Publisher>,
    notifications: Arc<Notifications>,
    config: DeliveryExecutionConfigDto,
) -> Result<ExternalEventForwarder> {
    let ports = ports::Harness::new();
    ExternalEventForwarder::new(
        Arc::new(f.schedule()),
        publisher,
        notifications,
        ports.feed_cache,
        ports.post_cache,
        ports.friend,
        Arc::new(ports::RecordingTimelineProjection::default()),
    )
    .with_execution_config(config)
}

async fn event(f: &Fixture, aggregate: Uuid, version: i64, age: i64) -> Result<Uuid> {
    let id = f.save(aggregate, Some(version), age).await?;
    // Unsupported fixture event has no cache/projection effect. The real
    // scheduler and worker still execute its dispatcher/publisher boundaries.
    sqlx::query(
        "update post_event_outbox set event_type='WORKER_FIXTURE',payload='{}' where event_id=$1",
    )
    .bind(id)
    .execute(&f.pool)
    .await?;
    Ok(id)
}

#[tokio::test]
async fn blocked_external_target_does_not_block_notification_successors() -> Result<()> {
    let f = Fixture::new(22).await?;
    let aggregate = Uuid::new_v4();
    let first = event(&f, aggregate, 1, 20).await?;
    let second = event(&f, aggregate, 2, 10).await?;
    let publisher = Arc::new(Publisher {
        block: true,
        ..Default::default()
    });
    let notifications = Arc::new(Notifications::default());
    let forwarder = forwarder(
        &f,
        publisher.clone(),
        notifications.clone(),
        Default::default(),
    )?;
    let worker = PostOutboxWorker::new(forwarder, Arc::new(PostMetrics::default()));
    let external = worker.clone();
    let task = tokio::spawn(async move {
        external
            .tick_target(DeliveryTarget::External, Utc::now(), 1)
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), publisher.entered.notified()).await?;
    let internal = tokio::time::timeout(
        Duration::from_secs(2),
        worker.tick_target(DeliveryTarget::Internal, Utc::now(), 10),
    )
    .await??;
    assert_eq!(internal, 0, "parent still awaits external/cache/timeline");
    assert_eq!(*notifications.events.lock().unwrap(), vec![first, second]);
    assert!(
        !task.is_finished(),
        "external effect is still blocked while notifications progress"
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_jobs where target='INTERNAL' and state='SUCCEEDED'").fetch_one(&f.pool).await?,2);
    publisher.release.notify_one();
    task.await??;
    f.shutdown().await
}

#[tokio::test]
async fn just_in_time_claim_does_not_lease_later_jobs_while_first_effect_waits() -> Result<()> {
    let f = Fixture::new(22).await?;
    event(&f, Uuid::new_v4(), 1, 20).await?;
    let later = event(&f, Uuid::new_v4(), 1, 10).await?;
    let publisher = Arc::new(Publisher {
        block: true,
        ..Default::default()
    });
    let forwarder = forwarder(
        &f,
        publisher.clone(),
        Arc::new(Notifications::default()),
        Default::default(),
    )?;
    let task = tokio::spawn(async move {
        forwarder
            .republish_target(DeliveryTarget::External, Utc::now(), 2)
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), publisher.entered.notified()).await?;
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='EXTERNAL'"
        )
        .bind(later)
        .fetch_one(&f.pool)
        .await?,
        "PENDING"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where event_id=$1"
        )
        .bind(later)
        .fetch_one(&f.pool)
        .await?,
        0
    );
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    f.shutdown().await
}

#[tokio::test]
async fn timed_out_effect_preserves_uncertain_started_and_prevents_immediate_retry() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1, 1).await?;
    let publisher = Arc::new(Publisher {
        block: true,
        ..Default::default()
    });
    let config = DeliveryExecutionConfigDto {
        effect_timeout_ms: 40,
        completion_reserve_ms: 100,
        ..Default::default()
    };
    let forwarder = forwarder(
        &f,
        publisher.clone(),
        Arc::new(Notifications::default()),
        config,
    )?;
    assert_eq!(
        tokio::time::timeout(
            Duration::from_secs(2),
            forwarder.republish_target(DeliveryTarget::External, Utc::now(), 10)
        )
        .await??,
        0
    );
    assert_eq!(publisher.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='EXTERNAL'"
        )
        .bind(id)
        .fetch_one(&f.pool)
        .await?,
        "LEASED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase<>'STARTED'"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    assert_eq!(
        forwarder
            .republish_target(DeliveryTarget::External, Utc::now(), 10)
            .await?,
        0
    );
    assert_eq!(publisher.calls.load(Ordering::SeqCst), 1);
    f.shutdown().await
}

#[tokio::test]
async fn aborted_worker_recovers_expired_claim_without_losing_event_or_history() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1, 1).await?;
    let publisher = Arc::new(Publisher {
        block: true,
        ..Default::default()
    });
    let initial = forwarder(
        &f,
        publisher.clone(),
        Arc::new(Notifications::default()),
        Default::default(),
    )?;
    let task = tokio::spawn(async move {
        initial
            .republish_target(DeliveryTarget::External, Utc::now(), 1)
            .await
    });
    tokio::time::timeout(Duration::from_secs(2), publisher.entered.notified()).await?;
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='EXTERNAL'").bind(id).execute(&f.pool).await?;
    let recovered = forwarder(
        &f,
        Arc::new(Publisher::default()),
        Arc::new(Notifications::default()),
        Default::default(),
    )?;
    recovered.republish_unpublished(Utc::now(), 10).await?;
    assert!(
        sqlx::query_scalar::<_, bool>("select published from post_event_outbox where event_id=$1")
            .bind(id)
            .fetch_one(&f.pool)
            .await?
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where target='EXTERNAL' and phase='STARTED'").fetch_one(&f.pool).await?,2);
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where target='EXTERNAL' and phase='SUCCEEDED'").fetch_one(&f.pool).await?,1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase='FAILED'"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn completed_parent_is_not_redelivered_by_worker() -> Result<()> {
    let f = Fixture::new(22).await?;
    event(&f, Uuid::new_v4(), 1, 1).await?;
    let publisher = Arc::new(Publisher::default());
    let notifications = Arc::new(Notifications::default());
    let forwarder = forwarder(
        &f,
        publisher.clone(),
        notifications.clone(),
        Default::default(),
    )?;
    // Repeated diagnostic drains must preserve completed target checkpoints.
    for _ in 0..4 {
        forwarder.republish_unpublished(Utc::now(), 10).await?;
    }
    assert_eq!(publisher.calls.load(Ordering::SeqCst), 1);
    assert_eq!(notifications.events.lock().unwrap().len(), 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from event_publications")
            .fetch_one(&f.pool)
            .await?,
        4
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from post_event_delivery_attempts")
            .fetch_one(&f.pool)
            .await?,
        8
    );
    f.shutdown().await
}

async fn uncertain_adapter_result(failure: Failure) -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1, 1).await?;
    let publisher = Arc::new(Publisher {
        failure: Some(failure),
        ..Default::default()
    });
    let worker = forwarder(
        &f,
        publisher.clone(),
        Arc::new(Notifications::default()),
        DeliveryExecutionConfigDto {
            effect_timeout_ms: 1000,
            ..Default::default()
        },
    )?;
    worker
        .republish_target(DeliveryTarget::External, Utc::now(), 10)
        .await?;
    worker
        .republish_target(DeliveryTarget::External, Utc::now(), 10)
        .await?;
    assert_eq!(publisher.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        publisher.accepted.load(Ordering::SeqCst),
        usize::from(matches!(failure, Failure::LostAcknowledgement))
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='EXTERNAL'"
        )
        .bind(id)
        .fetch_one(&f.pool)
        .await?,
        "LEASED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase='STARTED'"
        )
        .fetch_one(&f.pool)
        .await?,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase<>'STARTED'"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from event_publications")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn adapter_inner_timeout_is_uncertain_before_worker_timeout() -> Result<()> {
    uncertain_adapter_result(Failure::InnerTimeout).await
}

#[tokio::test]
async fn committed_effect_with_lost_acknowledgement_keeps_uncertain_history() -> Result<()> {
    uncertain_adapter_result(Failure::LostAcknowledgement).await
}

#[tokio::test]
async fn unclassified_adapter_error_cannot_prove_no_effect() -> Result<()> {
    uncertain_adapter_result(Failure::Unclassified).await
}

#[tokio::test]
async fn confirmed_failure_records_failed_and_schedules_target_retry() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1, 1).await?;
    let publisher = Arc::new(Publisher {
        failure: Some(Failure::Confirmed),
        ..Default::default()
    });
    let worker = forwarder(
        &f,
        publisher.clone(),
        Arc::new(Notifications::default()),
        Default::default(),
    )?;
    worker
        .republish_target(DeliveryTarget::External, Utc::now(), 1)
        .await?;
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='EXTERNAL'"
        )
        .bind(id)
        .fetch_one(&f.pool)
        .await?,
        "RETRY"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase='FAILED'"
        )
        .fetch_one(&f.pool)
        .await?,
        1
    );
    f.shutdown().await
}

struct ObservedSchedule {
    schedule: PgDeliverySchedule,
    pool: sqlx::PgPool,
    lose: bool,
}

#[async_trait]
impl DeliverySchedulePort for ObservedSchedule {
    async fn claim_target(
        &self,
        target: DeliveryTarget,
        cutoff: chrono::DateTime<Utc>,
        limit: i32,
        config: &DeliveryScheduleConfigDto,
    ) -> Result<TargetDeliveryBatchDto> {
        self.schedule
            .claim_target(target, cutoff, limit, config)
            .await
    }
    async fn lease_status(&self, claim: &TargetDeliveryClaimDto) -> Result<DeliveryLeaseStatusDto> {
        if self.lose {
            sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target=$2")
                .bind(claim.event.event_id).bind(claim.target.as_str()).execute(&self.pool).await?;
            self.schedule.lease_status(claim).await
        } else {
            // Inject an observation with less time than the completion reserve.
            Ok(DeliveryLeaseStatusDto::Owned { remaining_ms: 1 })
        }
    }
    async fn complete_target(
        &self,
        claim: &TargetDeliveryClaimDto,
    ) -> Result<DeliveryLeaseOutcomeDto> {
        self.schedule.complete_target(claim).await
    }
    async fn fail_target(&self, claim: &TargetDeliveryClaimDto) -> Result<DeliveryLeaseOutcomeDto> {
        self.schedule.fail_target(claim).await
    }
}

async fn refuse_dispatch(lose: bool) -> Result<()> {
    let f = Fixture::new(22).await?;
    event(&f, Uuid::new_v4(), 1, 1).await?;
    let publisher = Arc::new(Publisher::default());
    let notifications = Arc::new(Notifications::default());
    let ports = ports::Harness::new();
    let worker = ExternalEventForwarder::new(
        Arc::new(ObservedSchedule {
            schedule: f.schedule(),
            pool: f.pool.clone(),
            lose,
        }),
        publisher.clone(),
        notifications.clone(),
        ports.feed_cache,
        ports.post_cache,
        ports.friend,
        Arc::new(ports::RecordingTimelineProjection::default()),
    );
    worker
        .republish_target(DeliveryTarget::External, Utc::now(), 1)
        .await?;
    assert_eq!(publisher.calls.load(Ordering::SeqCst), 0);
    assert!(notifications.events.lock().unwrap().is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase<>'STARTED'"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn lost_database_ownership_prevents_effect_dispatch() -> Result<()> {
    refuse_dispatch(true).await
}

#[tokio::test]
async fn insufficient_observed_budget_prevents_effect_dispatch() -> Result<()> {
    refuse_dispatch(false).await
}

#[tokio::test]
async fn dead_external_head_does_not_stop_notification_worker() -> Result<()> {
    let f = Fixture::new(22).await?;
    let aggregate = Uuid::new_v4();
    let first = event(&f, aggregate, 1, 20).await?;
    let second = event(&f, aggregate, 2, 10).await?;
    let publisher = Arc::new(Publisher {
        failure: Some(Failure::Confirmed),
        ..Default::default()
    });
    let notifications = Arc::new(Notifications::default());
    let worker = forwarder(
        &f,
        publisher.clone(),
        notifications.clone(),
        DeliveryExecutionConfigDto {
            schedule: DeliveryScheduleConfigDto {
                max_attempts: 1,
                ..Default::default()
            },
            ..Default::default()
        },
    )?;
    worker
        .republish_target(DeliveryTarget::External, Utc::now(), 10)
        .await?;
    assert_eq!(publisher.calls.load(Ordering::SeqCst), 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='EXTERNAL'"
        )
        .bind(first)
        .fetch_one(&f.pool)
        .await?,
        "DEAD"
    );
    worker
        .republish_target(DeliveryTarget::Internal, Utc::now(), 10)
        .await?;
    assert_eq!(*notifications.events.lock().unwrap(), vec![first, second]);
    f.shutdown().await
}

#[tokio::test]
async fn successful_eviction_then_confirmed_feed_failure_is_partial_uncertainty() -> Result<()> {
    let f = Fixture::new(22).await?;
    let id = event(&f, Uuid::new_v4(), 1, 1).await?;
    let post = Uuid::new_v4();
    sqlx::query(
        "update post_event_outbox set event_type='POST_DELETED',payload=$2 where event_id=$1",
    )
    .bind(id)
    .bind(serde_json::json!({"postId":post,"actorId":Uuid::new_v4()}).to_string())
    .execute(&f.pool)
    .await?;
    let ports = ports::Harness::new();
    *ports.feed_cache.error.lock().unwrap() = Some("confirmed pre-invalidation failure".into());
    let worker = ExternalEventForwarder::new(
        Arc::new(f.schedule()),
        Arc::new(Publisher::default()),
        Arc::new(Notifications::default()),
        ports.feed_cache.clone(),
        ports.post_cache.clone(),
        ports.friend.clone(),
        Arc::new(ports::RecordingTimelineProjection::default()),
    );
    worker
        .republish_target(DeliveryTarget::Cache, Utc::now(), 1)
        .await?;
    assert_eq!(*ports.post_cache.evicted.lock().unwrap(), vec![post]);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='CACHE'"
        )
        .bind(id)
        .fetch_one(&f.pool)
        .await?,
        "LEASED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase<>'STARTED'"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from event_publications")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.shutdown().await
}

#[derive(Default)]
struct PartialNotifications {
    delivered: Mutex<Vec<Uuid>>,
    calls: AtomicUsize,
}

#[async_trait]
impl post_service::application::ports::NotificationClientPort for PartialNotifications {
    async fn create_notification(
        &self,
        request: post_service::domain::post::NotificationRequest,
    ) -> Result<()> {
        if self.calls.fetch_add(1, Ordering::SeqCst) == 1 {
            return Err(DeliveryEffectFailure::confirmed(anyhow::anyhow!(
                "second recipient rejected before write"
            )));
        }
        self.delivered.lock().unwrap().push(request.user_id);
        Ok(())
    }
}

#[tokio::test]
async fn partial_notification_fanout_cannot_be_a_confirmed_target_failure() -> Result<()> {
    let f = Fixture::new(22).await?;
    let aggregate = Uuid::new_v4();
    let id = event(&f, aggregate, 1, 1).await?;
    let author = Uuid::new_v4();
    sqlx::query("update post_event_outbox set event_type='POST_CREATED',actor_id=$2,payload=$3 where event_id=$1")
        .bind(id).bind(author).bind(serde_json::json!({"postId":aggregate,"authorId":author}).to_string()).execute(&f.pool).await?;
    let ports = ports::Harness::new();
    ports
        .posts
        .posts_by_id
        .lock()
        .unwrap()
        .insert(aggregate, ports::post(aggregate, author, "public fixture"));
    let first = Uuid::new_v4();
    *ports.friend.follower_ids.lock().unwrap() = vec![first, Uuid::new_v4()];
    let client = Arc::new(PartialNotifications::default());
    let dispatcher = post_service::adapters::r#in::messaging::internal_notification_listener::InternalNotificationListener::new(
        ports.posts.clone(), ports.friend.clone(), client.clone());
    let worker = ExternalEventForwarder::new(
        Arc::new(f.schedule()),
        Arc::new(Publisher::default()),
        Arc::new(dispatcher),
        ports.feed_cache,
        ports.post_cache,
        ports.friend,
        Arc::new(ports::RecordingTimelineProjection::default()),
    );
    worker
        .republish_target(DeliveryTarget::Internal, Utc::now(), 1)
        .await?;
    assert_eq!(*client.delivered.lock().unwrap(), vec![first]);
    assert_eq!(client.calls.load(Ordering::SeqCst), 2);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='INTERNAL'"
        )
        .bind(id)
        .fetch_one(&f.pool)
        .await?,
        "LEASED"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase<>'STARTED'"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    f.shutdown().await
}
