use super::support::{instant, stored_event, uuid, Harness};
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use post_service::adapters::r#in::messaging::external_event_forwarder::ExternalEventForwarder;
use post_service::adapters::r#in::messaging::timeline_fanout_worker::TimelineFanoutWorker;
use post_service::application::dto::timeline_fanout_dto::{
    TimelineFanoutClaimDto, TimelineFanoutClaimRequestDto, TimelineFanoutFailureDto,
    TimelineFanoutOutcomeDto, TimelineFanoutPageDto,
};
use post_service::application::ports::{FeedFeatureRepositoryPort, TimelineFanoutRepositoryPort};
use post_service::application::use_cases::process_timeline_fanout::{
    ProcessTimelineFanoutUseCase, TimelineFanoutConfig,
};
use post_service::config::PostMetrics;
use post_service::domain::post::OutboxMetricsSnapshot;
use post_service::domain::ranking::{AuthorDeliveryMode, FeedEngagementAction};
use post_service::domain::recommendation::TimelineFanoutJob;
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
struct EnqueueCall {
    event_id: Uuid,
    post_id: Uuid,
    author_id: Uuid,
    follower_count: i64,
    threshold: i64,
}

#[derive(Debug, Clone, PartialEq)]
struct EngagementCall {
    event_id: Uuid,
    post_id: Uuid,
    actor_id: Uuid,
    author_id: Uuid,
    action: FeedEngagementAction,
}

#[derive(Default)]
struct FakeFeedRankingRepository {
    enqueues: Mutex<Vec<EnqueueCall>>,
    claimed: Mutex<Vec<TimelineFanoutClaimDto>>,
    inserted: Mutex<Vec<(Uuid, Vec<Uuid>)>>,
    advanced: Mutex<Vec<(Uuid, Option<String>, bool)>>,
    removals: Mutex<Vec<Uuid>>,
    engagements: Mutex<Vec<EngagementCall>>,
    claim_error: Mutex<Option<String>>,
    failures: Mutex<usize>,
    expiries: Mutex<Vec<DateTime<Utc>>>,
}

#[async_trait]
impl TimelineFanoutRepositoryPort for FakeFeedRankingRepository {
    async fn enqueue_post(
        &self,
        event_id: Uuid,
        post_id: Uuid,
        author_id: Uuid,
        _post_created_at: DateTime<Utc>,
        follower_count: i64,
        high_fanout_threshold: i64,
        _now: DateTime<Utc>,
    ) -> Result<AuthorDeliveryMode> {
        self.enqueues.lock().unwrap().push(EnqueueCall {
            event_id,
            post_id,
            author_id,
            follower_count,
            threshold: high_fanout_threshold,
        });
        Ok(if follower_count >= high_fanout_threshold {
            AuthorDeliveryMode::HighFanout
        } else {
            AuthorDeliveryMode::Normal
        })
    }

    async fn claim_jobs(
        &self,
        _request: &TimelineFanoutClaimRequestDto,
    ) -> Result<Vec<TimelineFanoutClaimDto>> {
        if let Some(error) = self.claim_error.lock().unwrap().clone() {
            anyhow::bail!(error);
        }
        Ok(std::mem::take(&mut *self.claimed.lock().unwrap()))
    }

    async fn apply_page(
        &self,
        claim: &TimelineFanoutClaimDto,
        page: &TimelineFanoutPageDto,
    ) -> Result<TimelineFanoutOutcomeDto> {
        self.inserted
            .lock()
            .unwrap()
            .push((claim.job.job_id, page.follower_ids.clone()));
        self.expiries.lock().unwrap().push(page.expires_at);
        self.advanced.lock().unwrap().push((
            claim.job.job_id,
            page.next_cursor.clone(),
            page.next_cursor.is_none(),
        ));
        Ok(TimelineFanoutOutcomeDto::Applied)
    }
    async fn record_job_failure(
        &self,
        _claim: &TimelineFanoutClaimDto,
        _failure: &TimelineFanoutFailureDto,
    ) -> Result<TimelineFanoutOutcomeDto> {
        *self.failures.lock().unwrap() += 1;
        Ok(TimelineFanoutOutcomeDto::Applied)
    }

    async fn remove_post(&self, post_id: Uuid) -> Result<()> {
        self.removals.lock().unwrap().push(post_id);
        Ok(())
    }
}

struct ClaimedReader {
    calls: Mutex<Vec<post_service::application::fanout_authority::FanoutPageLookupDto>>,
    mode: u8,
}
#[async_trait]
impl post_service::application::fanout_authority::ClaimedFanoutReadPort for ClaimedReader {
    async fn fetch_claimed_page(
        &self,
        request: &post_service::application::fanout_authority::FanoutPageLookupDto,
    ) -> Result<post_service::application::fanout_authority::ClaimedFanoutPageDto> {
        use post_service::application::fanout_authority::{
            ClaimedFanoutPageDto, FanoutPageLookupDto,
        };
        self.calls.lock().unwrap().push(request.clone());
        let author = request.claim().job.author_id;
        let id = Uuid::from_u128(99);
        match self.mode {
            0 => ClaimedFanoutPageDto::new(
                request,
                author,
                vec![id],
                None,
                (request).deadline().expires_at(),
            ),
            1 => anyhow::bail!("owner denied"),
            2 => ClaimedFanoutPageDto::new(
                request,
                author,
                vec![id, id],
                None,
                (request).deadline().expires_at(),
            ),
            3 => ClaimedFanoutPageDto::new(
                request,
                author,
                vec![Uuid::nil()],
                None,
                (request).deadline().expires_at(),
            ),
            4 => ClaimedFanoutPageDto::new(
                request,
                author,
                vec![author],
                None,
                (request).deadline().expires_at(),
            ),
            5 => ClaimedFanoutPageDto::new(
                request,
                author,
                vec![id],
                Some(format!("2026-10-14T00:00:00Z|{}", Uuid::new_v4())),
                (request).deadline().expires_at(),
            ),
            7 => ClaimedFanoutPageDto::new(
                request,
                author,
                vec![id, Uuid::from_u128(100), Uuid::from_u128(101)],
                None,
                (request).deadline().expires_at(),
            ),
            8 => ClaimedFanoutPageDto::new(
                request,
                author,
                vec![id],
                request.claim().job.cursor.clone(),
                (request).deadline().expires_at(),
            ),
            9 => ClaimedFanoutPageDto::new(
                request,
                author,
                vec![id],
                Some(format!(" 2026-10-14T00:00:00Z|{id} ")),
                (request).deadline().expires_at(),
            ),
            10 => ClaimedFanoutPageDto::new(
                request,
                author,
                vec![id],
                Some(format!("2026-10-14T00:00:00.{}Z|{id}", "0".repeat(100))),
                (request).deadline().expires_at(),
            ),
            _ => {
                let mut claim = request.claim().clone();
                claim.job.author_id = Uuid::new_v4();
                let other = FanoutPageLookupDto::new(claim, request.page_limit())?;
                ClaimedFanoutPageDto::new(
                    &other,
                    other.claim().job.author_id,
                    vec![id],
                    None,
                    other.deadline().expires_at(),
                )
            }
        }
    }
}
#[tokio::test]
async fn explicit_claimed_worker_has_no_fallback_and_keeps_entry_retention_separate() {
    use post_service::application::fanout_authority::ClaimedFanoutReadPort;
    for mode in 0..11 {
        let ranking = Arc::new(FakeFeedRankingRepository::default());
        let claim = TimelineFanoutClaimDto {
            job: TimelineFanoutJob {
                job_id: Uuid::new_v4(),
                event_id: Uuid::new_v4(),
                post_id: Uuid::new_v4(),
                author_id: Uuid::new_v4(),
                post_created_at: instant(1_700_000_000),
                cursor: (mode == 8)
                    .then(|| format!("2026-10-14T00:00:00Z|{}", Uuid::from_u128(99))),
                attempt_count: 4,
            },
            lease_token: Uuid::new_v4(),
            lease_expires_at: Utc::now() + chrono::Duration::seconds(60),
            page_attempt_count: 2,
            max_page_attempts: 2,
        };
        ranking.claimed.lock().unwrap().push(claim.clone());
        let reader = Arc::new(ClaimedReader {
            calls: Mutex::new(vec![]),
            mode,
        });
        let config = TimelineFanoutConfig {
            claim_limit: 1,
            follower_page_size: 2,
            entry_ttl: std::time::Duration::from_secs(3600),
            ..TimelineFanoutConfig::default()
        };
        // No general Friend dependency is provided or reachable.
        let worker = ProcessTimelineFanoutUseCase::new_claimed(
            ranking.clone(),
            reader.clone() as Arc<dyn ClaimedFanoutReadPort>,
            ranking.clone(),
            config,
        )
        .unwrap();
        assert_eq!(
            worker.execute(instant(1_700_000_300)).await.unwrap(),
            i64::from(mode == 0)
        );
        let calls = reader.calls.lock().unwrap();
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].claim().lease_token, claim.lease_token);
        assert_eq!(calls[0].claim().job.attempt_count, 4);
        assert_eq!(calls[0].claim().page_attempt_count, 2);
        assert_eq!(calls[0].page_limit(), 2);
        assert_eq!(
            ranking.inserted.lock().unwrap().len(),
            usize::from(mode == 0)
        );
        assert_eq!(*ranking.failures.lock().unwrap(), usize::from(mode != 0));
        if mode == 0 {
            assert_eq!(
                ranking.expiries.lock().unwrap().as_slice(),
                &[instant(1_700_003_600)]
            );
        }
    }
}
#[test]
fn claimed_worker_rejects_invalid_configuration_instead_of_clamping_signed_inputs() {
    let ranking = Arc::new(FakeFeedRankingRepository::default());
    let reader = Arc::new(ClaimedReader {
        calls: Mutex::new(vec![]),
        mode: 0,
    });
    for case in 0..13 {
        let mut config = TimelineFanoutConfig::default();
        match case {
            0 => config.claim_limit = 0,
            1 => config.claim_limit = 101,
            2 => config.follower_page_size = 0,
            3 => config.follower_page_size = 101,
            4 => config.max_attempts = 0,
            5 => config.lease_timeout = std::time::Duration::ZERO,
            6 => config.base_retry = std::time::Duration::ZERO,
            7 => config.entry_ttl = std::time::Duration::ZERO,
            8 => config.lease_timeout = std::time::Duration::from_micros(999),
            9 => config.base_retry = std::time::Duration::from_millis(50),
            10 => config.lease_timeout = std::time::Duration::from_millis(86_400_001),
            11 => config.base_retry = std::time::Duration::from_millis(3_600_001),
            _ => config.lease_timeout = std::time::Duration::MAX,
        }
        assert!(ProcessTimelineFanoutUseCase::new_claimed(
            ranking.clone(),
            reader.clone(),
            ranking.clone(),
            config
        )
        .is_err());
    }
    assert!(reader.calls.lock().unwrap().is_empty());
}

#[async_trait]
impl FeedFeatureRepositoryPort for FakeFeedRankingRepository {
    async fn record_engagement(
        &self,
        event_id: Uuid,
        post_id: Uuid,
        actor_id: Uuid,
        author_id: Uuid,
        action: FeedEngagementAction,
        _now: DateTime<Utc>,
    ) -> Result<()> {
        self.engagements.lock().unwrap().push(EngagementCall {
            event_id,
            post_id,
            actor_id,
            author_id,
            action,
        });
        Ok(())
    }

    async fn cleanup_user(&self, _user_id: Uuid) -> Result<()> {
        Ok(())
    }
}

#[tokio::test]
async fn post_created_enqueues_timeline_projection_and_checkpoints_it() {
    let harness = Harness::new();
    let ranking = Arc::new(FakeFeedRankingRepository::default());
    let author_id = uuid("40000000-0000-0000-0000-000000000001");
    let post_id = uuid("40000000-0000-0000-0000-000000000002");
    *harness.friend.follower_ids.lock().unwrap() = vec![Uuid::new_v4(), Uuid::new_v4()];
    let event = stored_event(
        uuid("40000000-0000-0000-0000-000000000003"),
        "POST_CREATED",
        serde_json::json!({"postId": post_id, "authorId": author_id}).to_string(),
    );
    *harness.outbox.unpublished.lock().unwrap() = vec![event.clone()];

    let forwarded = ExternalEventForwarder::new(
        harness.outbox.clone(),
        harness.publisher.clone(),
        harness.internal.clone(),
        harness.feed_cache.clone(),
        harness.post_cache.clone(),
        harness.friend.clone(),
        ranking.clone(),
    )
    .with_high_fanout_threshold(10_000)
    .republish_unpublished(instant(1_700_000_300), 10)
    .await
    .expect("forward");

    assert_eq!(forwarded, 1);
    assert_eq!(ranking.enqueues.lock().unwrap().len(), 1);
    assert_eq!(ranking.enqueues.lock().unwrap()[0].post_id, post_id);
    assert!(harness
        .outbox
        .completed
        .lock()
        .unwrap()
        .contains(&(event.event_id, "TIMELINE".to_owned())));
}

#[tokio::test]
async fn fanout_worker_pages_followers_and_advances_cursor() {
    let harness = Harness::new();
    let ranking = Arc::new(FakeFeedRankingRepository::default());
    let job_id = uuid("41000000-0000-0000-0000-000000000001");
    let author_id = uuid("41000000-0000-0000-0000-000000000002");
    *ranking.claimed.lock().unwrap() = vec![TimelineFanoutClaimDto {
        page_attempt_count: 1,
        max_page_attempts: 8,
        job: TimelineFanoutJob {
            job_id,
            event_id: uuid("41000000-0000-0000-0000-000000000003"),
            post_id: uuid("41000000-0000-0000-0000-000000000004"),
            author_id,
            post_created_at: instant(1_700_000_000),
            cursor: None,
            attempt_count: 0,
        },
        lease_token: Uuid::new_v4(),
        lease_expires_at: instant(1_700_000_900),
    }];
    *harness.friend.follower_ids.lock().unwrap() = (0..150)
        .map(|index| Uuid::from_u128(0x41000000000000000000000000001000 + index))
        .collect();
    let worker = ProcessTimelineFanoutUseCase::new(
        ranking.clone(),
        harness.friend.clone(),
        TimelineFanoutConfig {
            claim_limit: 25,
            follower_page_size: 100,
            lease_timeout: std::time::Duration::from_secs(300),
            max_attempts: 5,
            base_retry: std::time::Duration::from_secs(1),
            entry_ttl: std::time::Duration::from_secs(14 * 24 * 60 * 60),
        },
    );

    assert_eq!(worker.execute(instant(1_700_000_300)).await.unwrap(), 1);
    let inserted = ranking.inserted.lock().unwrap();
    assert_eq!(inserted.len(), 1);
    assert_eq!(inserted[0].1.len(), 100);
    assert_ne!(inserted[0].1[0], author_id);
    assert_eq!(
        ranking.advanced.lock().unwrap().as_slice(),
        &[(job_id, Some("offset:100".to_owned()), false)]
    );
}

#[tokio::test]
async fn timeline_fanout_worker_tick_records_failure_metrics_when_claim_fails() {
    let harness = Harness::new();
    let ranking = Arc::new(FakeFeedRankingRepository::default());
    *ranking.claim_error.lock().unwrap() = Some("timeline claim unavailable".into());
    let metrics = Arc::new(PostMetrics::default());
    let worker = TimelineFanoutWorker::new(
        ProcessTimelineFanoutUseCase::new(
            ranking,
            harness.friend.clone(),
            TimelineFanoutConfig {
                claim_limit: 25,
                follower_page_size: 100,
                lease_timeout: std::time::Duration::from_secs(300),
                max_attempts: 5,
                base_retry: std::time::Duration::from_secs(1),
                entry_ttl: std::time::Duration::from_secs(14 * 24 * 60 * 60),
            },
        ),
        metrics.clone(),
    );

    let error = worker.tick().await.expect_err("timeline claim must fail");

    assert!(error.to_string().contains("timeline claim unavailable"));
    let rendered = metrics.render_prometheus(&OutboxMetricsSnapshot::default());
    assert!(rendered.contains("post_timeline_fanout_runs_total{outcome=\"failure\"} 1\n"));
    assert!(rendered.contains("post_timeline_fanout_pages_total{outcome=\"failure\"} 0\n"));
}

#[tokio::test]
async fn engagement_events_update_velocity_and_affinity_with_post_author() {
    let harness = Harness::new();
    let ranking = Arc::new(FakeFeedRankingRepository::default());
    let actor_id = uuid("42000000-0000-0000-0000-000000000001");
    let post_author_id = uuid("42000000-0000-0000-0000-000000000002");
    let post_id = uuid("42000000-0000-0000-0000-000000000003");
    let mut event = stored_event(
        uuid("42000000-0000-0000-0000-000000000004"),
        "POST_LIKED",
        serde_json::json!({
            "postId": post_id,
            "actorId": actor_id,
            "authorId": post_author_id,
            "notifyUserId": post_author_id
        })
        .to_string(),
    );
    event.actor_id = Some(actor_id);
    let event_id = event.event_id;
    *harness.outbox.unpublished.lock().unwrap() = vec![event.clone()];

    ExternalEventForwarder::new(
        harness.outbox.clone(),
        harness.publisher.clone(),
        harness.internal.clone(),
        harness.feed_cache.clone(),
        harness.post_cache.clone(),
        harness.friend.clone(),
        ranking.clone(),
    )
    .with_high_fanout_threshold(10_000)
    .republish_unpublished(instant(1_700_000_300), 10)
    .await
    .expect("forward");

    assert_eq!(
        ranking.engagements.lock().unwrap().as_slice(),
        &[EngagementCall {
            event_id,
            post_id,
            actor_id,
            author_id: post_author_id,
            action: FeedEngagementAction::Like,
        }]
    );
}

#[tokio::test]
async fn post_delete_removes_timeline_projection_once() {
    let harness = Harness::new();
    let ranking = Arc::new(FakeFeedRankingRepository::default());
    let post_id = uuid("43000000-0000-0000-0000-000000000001");
    let event = stored_event(
        uuid("43000000-0000-0000-0000-000000000002"),
        "POST_DELETED",
        serde_json::json!({
            "postId": post_id,
            "authorId": "43000000-0000-0000-0000-000000000003",
            "actorId": "43000000-0000-0000-0000-000000000003"
        })
        .to_string(),
    );
    *harness.outbox.unpublished.lock().unwrap() = vec![event.clone()];
    let forwarder = ExternalEventForwarder::new(
        harness.outbox.clone(),
        harness.publisher.clone(),
        harness.internal.clone(),
        harness.feed_cache.clone(),
        harness.post_cache.clone(),
        harness.friend.clone(),
        ranking.clone(),
    )
    .with_high_fanout_threshold(10_000);

    assert_eq!(
        forwarder
            .republish_unpublished(instant(1_700_000_300), 10)
            .await
            .unwrap(),
        1
    );
    // Re-present a completed event through the recording claim fixture. The
    // worker must preserve completed checkpoints without a new publication
    // transition or another projection effect.
    *harness.outbox.unpublished.lock().unwrap() = vec![event];
    assert_eq!(
        forwarder
            .republish_unpublished(instant(1_700_000_301), 10)
            .await
            .unwrap(),
        0
    );
    assert_eq!(ranking.removals.lock().unwrap().as_slice(), &[post_id]);
}

#[async_trait]
impl post_service::application::ports::TimelineProjectionRepositoryPort
    for FakeFeedRankingRepository
{
    async fn prepare(
        &self,
        event: &post_service::application::dto::timeline_projection_dto::TimelineProjectionEventDto,
        _limit: i32,
    ) -> Result<post_service::application::dto::timeline_projection_dto::TimelineProjectionPlanDto>
    {
        use post_service::application::dto::timeline_projection_dto::*;
        Ok(TimelineProjectionPlanDto::Events(vec![
            TimelineProjectionWorkDto {
                event: event.clone(),
                effect: event.effect()?,
            },
        ]))
    }
    async fn apply(
        &self,
        request: &post_service::application::dto::timeline_projection_dto::TimelineProjectionApplyDto,
    ) -> Result<post_service::application::dto::timeline_projection_dto::TimelineProjectionOutcomeDto>
    {
        use post_service::application::dto::timeline_projection_dto::*;
        let e = &request.work.event;
        match request.work.effect {
            TimelineProjectionEffectDto::Create { author_id } => {
                self.enqueue_post(
                    e.event_id,
                    e.aggregate_id,
                    author_id,
                    e.occurred_at,
                    request.follower_count.unwrap(),
                    request.high_fanout_threshold,
                    e.occurred_at,
                )
                .await?;
            }
            TimelineProjectionEffectDto::Delete { .. } => self.remove_post(e.aggregate_id).await?,
            TimelineProjectionEffectDto::Engagement {
                actor_id,
                author_id,
                action,
            } => {
                self.record_engagement(
                    e.event_id,
                    e.aggregate_id,
                    actor_id,
                    author_id,
                    action,
                    e.occurred_at,
                )
                .await?
            }
            TimelineProjectionEffectDto::StoredUnlike { .. } => {
                anyhow::bail!("Stored unlike owner resolution requires native PostgreSQL")
            }
            TimelineProjectionEffectDto::Noop => {}
        }
        Ok(TimelineProjectionOutcomeDto::Applied)
    }
}

#[async_trait]
impl post_service::application::fanout_authority::ClaimedFanoutApplyPort
    for FakeFeedRankingRepository
{
    async fn apply_claimed_page(
        &self,
        claim: &TimelineFanoutClaimDto,
        page: &TimelineFanoutPageDto,
        _deadline: post_service::application::delivery_effect::DeliveryEffectDeadlineDto,
    ) -> Result<TimelineFanoutOutcomeDto> {
        self.apply_page(claim, page).await
    }
}
