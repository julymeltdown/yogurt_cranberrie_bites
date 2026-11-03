//! Owner-backed PostgreSQL probes for the local TIMELINE consumer. Friend and
//! other targets are controlled doubles only in the forwarder composition probe.
use super::ports;
use super::Fixture;
use anyhow::Result;
use chrono::{Duration, Utc};
use post_service::adapters::out::persistence::event_outbox::PgEventOutbox;
use post_service::adapters::out::persistence::timeline_projection_repository::PgTimelineProjectionRepository;
use post_service::application::dto::timeline_projection_dto::*;
use post_service::application::ports::{EventOutboxPort, TimelineProjectionRepositoryPort};
use post_service::domain::post::StoredEvent;
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;

pub(super) async fn seed(f: &Fixture) -> Result<(Uuid, Uuid)> {
    let post = Uuid::new_v4();
    let author = Uuid::new_v4();
    sqlx::query("insert into posts(id,author_id,content,visibility,created_at) values($1,$2,'projection fixture','PUBLIC',now())")
        .bind(post).bind(author).execute(&f.pool).await?;
    Ok((post, author))
}
pub(super) async fn event(
    f: &Fixture,
    post: Uuid,
    author: Uuid,
    actor: Uuid,
    kind: &str,
) -> Result<TimelineProjectionEventDto> {
    let now = Utc::now() - Duration::seconds(1);
    let payload = match kind {
        "POST_COMMENTED" => {
            serde_json::json!({"postId":post,"authorId":actor,"notifyUserId":author})
        }
        "POST_COMMENT_DELETED" => {
            serde_json::json!({"postId":post,"authorId":actor,"postAuthorId":author,"actorId":actor,"commentId":Uuid::new_v4()})
        }
        _ => serde_json::json!({"postId":post,"authorId":author,"actorId":actor}),
    };
    let stored = PgEventOutbox::new(f.pool.clone())
        .save(&StoredEvent {
            event_id: Uuid::new_v4(),
            event_type: kind.into(),
            aggregate_id: post,
            aggregate_version: None,
            actor_id: Some(actor),
            attributes: vec!["projection-probe".into()],
            payload: payload.to_string(),
            occurred_at: now,
            created_at: now,
            published: false,
            published_at: None,
            replay_resolution_id: None,
            origin_traceparent: None,
        })
        .await?;
    Ok(TimelineProjectionEventDto::from(&stored))
}
fn request(e: &TimelineProjectionEventDto) -> Result<TimelineProjectionApplyDto> {
    Ok(TimelineProjectionApplyDto {
        authority: None,
        work: TimelineProjectionWorkDto {
            event: e.clone(),
            effect: e.effect()?,
        },
        follower_count: Some(2),
        high_fanout_threshold: 1000,
    })
}
pub(super) async fn authority(
    f: &Fixture,
    aggregate: Uuid,
) -> Result<post_service::application::dto::delivery_schedule_dto::TargetDeliveryClaimDto> {
    use post_service::application::ports::DeliverySchedulePort;
    use post_service::domain::post::{DeliveryTarget, EventReplayFilter};
    if let Some(claim) = f
        .schedule()
        .claim_target(DeliveryTarget::Timeline, Utc::now(), 1, &Default::default())
        .await?
        .claims
        .pop()
    {
        anyhow::ensure!(
            claim.event.aggregate_id == aggregate,
            "fixture selected another aggregate"
        );
        return Ok(claim);
    }
    // Reuse the real scheduler's retained live claim after a failed transaction.
    let row=sqlx::query("select j.event_id,j.lease_token,j.attempt_id,j.attempt_count,j.lease_expires_at from post_event_delivery_jobs j join post_event_outbox e on e.event_id=j.event_id where e.aggregate_id=$1 and j.target='TIMELINE' and j.state='LEASED'").bind(aggregate).fetch_one(&f.pool).await?;
    let events = PgEventOutbox::new(f.pool.clone())
        .find_events(
            &EventReplayFilter {
                actor_id: None,
                event_type: None,
                attribute: None,
                from: None,
                to: None,
                published: None,
            },
            10000,
        )
        .await?;
    let event = events
        .into_iter()
        .find(|e| e.event_id == row.get::<Uuid, _>("event_id"))
        .unwrap();
    Ok(
        post_service::application::dto::delivery_schedule_dto::TargetDeliveryClaimDto {
            event,
            target: DeliveryTarget::Timeline,
            lease_token: row.get("lease_token"),
            attempt_id: row.get("attempt_id"),
            attempt_number: row.get("attempt_count"),
            lease_expires_at: row.get("lease_expires_at"),
        },
    )
}
async fn authorized_request(
    f: &Fixture,
    e: &TimelineProjectionEventDto,
) -> Result<TimelineProjectionApplyDto> {
    let mut q = request(e)?;
    q.authority = Some(authority(f, e.aggregate_id).await?);
    Ok(q)
}
pub(super) async fn apply(
    f: &Fixture,
    e: &TimelineProjectionEventDto,
) -> Result<TimelineProjectionOutcomeDto> {
    use post_service::application::ports::DeliverySchedulePort;
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    let mut q = request(e)?;
    if sqlx::query_scalar::<_, bool>(
        "select exists(select 1 from timeline_projection_receipts where event_id=$1)",
    )
    .bind(e.event_id)
    .fetch_one(&f.pool)
    .await?
    {
        return repo.apply(&q).await;
    }
    q.authority = Some(authority(f, e.aggregate_id).await?);
    let outcome = repo.apply(&q).await?;
    if q.authority.as_ref().unwrap().event.event_id == e.event_id {
        f.schedule()
            .complete_target(q.authority.as_ref().unwrap())
            .await?;
    }
    Ok(outcome)
}
pub(super) async fn cursor(f: &Fixture, post: Uuid) -> Result<i64> {
    Ok(sqlx::query_scalar(
        "select last_version from timeline_projection_cursors where aggregate_id=$1",
    )
    .bind(post)
    .fetch_optional(&f.pool)
    .await?
    .unwrap_or(0))
}
async fn receipts(f: &Fixture) -> Result<i64> {
    Ok(
        sqlx::query_scalar("select count(*) from timeline_projection_receipts")
            .fetch_one(&f.pool)
            .await?,
    )
}

#[tokio::test]
async fn all_generated_event_types_commit_in_sequence_and_delete_retires_fanout() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let actor = Uuid::new_v4();
    let mut events = vec![event(&f, post, author, author, "POST_CREATED").await?];
    for kind in [
        "POST_LIKED",
        "POST_UNLIKED",
        "POST_COMMENTED",
        "POST_COMMENT_DELETED",
    ] {
        events.push(event(&f, post, author, actor, kind).await?);
    }
    events.push(event(&f, post, author, author, "POST_DELETED").await?);
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    let TimelineProjectionPlanDto::Events(works) =
        repo.prepare(events.last().unwrap(), 100).await?
    else {
        panic!("new events")
    };
    assert_eq!(works.len(), 6);
    for work in works {
        assert_eq!(work.effect, work.event.effect()?);
        apply(&f, &work.event).await?;
    }
    assert_eq!(cursor(&f, post).await?, 6);
    assert_eq!(receipts(&f).await?, 6);
    let row = sqlx::query(
        "select like_events,comment_events from post_engagement_velocity where post_id=$1",
    )
    .bind(post)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(row.get::<i64, _>("like_events"), 0);
    assert_eq!(row.get::<i64, _>("comment_events"), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_fanout_jobs")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from home_timeline_entries")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    assert_eq!(
        apply(&f, &events[1]).await?,
        TimelineProjectionOutcomeDto::Duplicate
    );
    f.shutdown().await
}
#[tokio::test]
async fn out_of_order_apply_fails_then_normal_prefix_recovery_preserves_clamped_deltas(
) -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let actor = Uuid::new_v4();
    let create = event(&f, post, author, author, "POST_CREATED").await?;
    let unlike = event(&f, post, author, actor, "POST_UNLIKED").await?;
    let like = event(&f, post, author, actor, "POST_LIKED").await?;
    assert!(apply(&f, &like).await.is_err());
    assert_eq!(receipts(&f).await?, 0);
    assert_eq!(cursor(&f, post).await?, 0);
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    let TimelineProjectionPlanDto::Events(works) = repo.prepare(&like, 100).await? else {
        panic!("prefix")
    };
    assert_eq!(
        works.iter().map(|w| w.event.event_id).collect::<Vec<_>>(),
        vec![create.event_id, unlike.event_id, like.event_id]
    );
    for work in works {
        apply(&f, &work.event).await?;
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select like_events from post_engagement_velocity where post_id=$1"
        )
        .bind(post)
        .fetch_one(&f.pool)
        .await?,
        1
    );
    f.shutdown().await
}
#[tokio::test]
async fn source_gap_and_missing_genesis_hold_without_max_bootstrap() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let actor = Uuid::new_v4();
    let create = event(&f, post, author, author, "POST_CREATED").await?;
    let prior = event(&f, post, author, actor, "POST_LIKED").await?;
    let target = event(&f, post, author, actor, "POST_UNLIKED").await?;
    sqlx::query("delete from post_event_outbox where event_id=$1")
        .bind(prior.event_id)
        .execute(&f.pool)
        .await?;
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    assert!(repo.prepare(&target, 100).await.is_err());
    assert_eq!(receipts(&f).await?, 0);
    sqlx::query("delete from post_event_outbox where event_id=$1")
        .bind(create.event_id)
        .execute(&f.pool)
        .await?;
    assert!(repo.prepare(&target, 100).await.is_err());
    assert_eq!(cursor(&f, post).await?, 0);
    f.shutdown().await
}
#[tokio::test]
async fn identity_receipts_survive_simulated_source_absence_but_conflicts_fail() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    apply(&f, &e).await?;
    // Current production constraints forbid source pruning beneath immutable
    // attempt history. Prove that rejection first, then inject source absence
    // in this disposable fixture while preserving every audit attempt row.
    assert!(
        sqlx::query("delete from post_event_outbox where event_id=$1")
            .bind(e.event_id)
            .execute(&f.pool)
            .await
            .is_err()
    );
    sqlx::query("alter table post_event_delivery_attempts drop constraint post_event_delivery_attempts_event_id_fkey").execute(&f.pool).await?;
    sqlx::query("delete from post_event_outbox where event_id=$1")
        .bind(e.event_id)
        .execute(&f.pool)
        .await?;
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    assert!(matches!(
        repo.prepare(&e, 100).await?,
        TimelineProjectionPlanDto::Duplicate
    ));
    assert_eq!(
        apply(&f, &e).await?,
        TimelineProjectionOutcomeDto::Duplicate
    );
    for mutate in 0..4 {
        let mut conflict = e.clone();
        match mutate {
            0 => conflict.payload.push(' '),
            1 => conflict.attributes.push("changed".into()),
            2 => conflict.aggregate_version = Some(2),
            _ => conflict.actor_id = Some(Uuid::new_v4()),
        };
        assert!(repo.prepare(&conflict, 100).await.is_err());
        assert!(repo
            .apply(
                &request(&conflict).unwrap_or_else(|_| TimelineProjectionApplyDto {
                    authority: None,
                    work: TimelineProjectionWorkDto {
                        event: conflict,
                        effect: e.effect().unwrap()
                    },
                    follower_count: Some(2),
                    high_fanout_threshold: 1000
                })
            )
            .await
            .is_err());
    }
    assert_eq!(receipts(&f).await?, 1);
    f.shutdown().await
}
#[tokio::test]
async fn concurrent_duplicates_apply_once_and_stale_preparations_recheck() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    let repo = Arc::new(PgTimelineProjectionRepository::new(f.pool.clone()));
    assert!(matches!(
        repo.prepare(&e, 100).await?,
        TimelineProjectionPlanDto::Events(_)
    ));
    let mut tasks = vec![];
    for _ in 0..8 {
        let r = repo.clone();
        let q = authorized_request(&f, &e).await?;
        tasks.push(tokio::spawn(async move { r.apply(&q).await }));
    }
    let mut applied = 0;
    for task in tasks {
        if task.await?? == TimelineProjectionOutcomeDto::Applied {
            applied += 1;
        }
    }
    assert_eq!(applied, 1);
    assert_eq!(receipts(&f).await?, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_fanout_jobs")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    f.shutdown().await
}
#[tokio::test]
async fn receipt_failure_rolls_back_engagement_inbox_features_and_cursor() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let create = event(&f, post, author, author, "POST_CREATED").await?;
    apply(&f, &create).await?;
    let e = event(&f, post, author, Uuid::new_v4(), "POST_LIKED").await?;
    sqlx::raw_sql("create function reject_projection_receipt() returns trigger language plpgsql as $$ begin raise exception 'injected receipt failure'; end; $$; create trigger reject_projection_receipt before insert on timeline_projection_receipts for each row execute function reject_projection_receipt();").execute(&f.pool).await?;
    assert!(apply(&f, &e).await.is_err());
    assert_eq!(cursor(&f, post).await?, 1);
    for table in [
        "feed_projection_inbox",
        "post_engagement_velocity",
        "feed_author_affinities",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("select count(*) from {table}"))
                .fetch_one(&f.pool)
                .await?,
            0
        );
    }
    sqlx::query("drop trigger reject_projection_receipt on timeline_projection_receipts")
        .execute(&f.pool)
        .await?;
    assert_eq!(apply(&f, &e).await?, TimelineProjectionOutcomeDto::Applied);
    f.shutdown().await
}
#[tokio::test]
async fn cursor_failure_rolls_back_creation_job_entry_and_receipt() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    sqlx::raw_sql("create function reject_projection_cursor() returns trigger language plpgsql as $$ begin raise exception 'injected cursor failure'; end; $$; create trigger reject_projection_cursor before update on timeline_projection_cursors for each row execute function reject_projection_cursor();").execute(&f.pool).await?;
    assert!(apply(&f, &e).await.is_err());
    assert_eq!(receipts(&f).await?, 0);
    assert_eq!(cursor(&f, post).await?, 0);
    for table in ["home_timeline_entries", "feed_fanout_jobs"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("select count(*) from {table}"))
                .fetch_one(&f.pool)
                .await?,
            0
        );
    }
    f.shutdown().await
}
#[tokio::test]
async fn historical_acknowledgements_are_not_effect_proof() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    let q = authorized_request(&f, &e).await?;
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    sqlx::query("update post_event_outbox set published=true where event_id=$1")
        .bind(e.event_id)
        .execute(&f.pool)
        .await?;
    assert!(repo.apply(&q).await.is_err());
    assert_eq!(receipts(&f).await?, 0);
    sqlx::query("update post_event_outbox set published=false where event_id=$1")
        .bind(e.event_id)
        .execute(&f.pool)
        .await?;
    sqlx::query("insert into event_publications(event_id,target,published_at,payload) values($1,'TIMELINE',now(),'historical checkpoint')").bind(e.event_id).execute(&f.pool).await?;
    assert!(repo.apply(&q).await.is_err());
    assert_eq!(cursor(&f, post).await?, 0);
    f.shutdown().await
}
#[tokio::test]
async fn retained_creation_job_is_proven_but_old_engagement_order_is_held() -> Result<()> {
    use post_service::adapters::out::persistence::feed_recommendation_repository::PgFeedRecommendationRepository;
    use post_service::application::ports::{
        FeedFeatureRepositoryPort, TimelineFanoutRepositoryPort,
    };
    use post_service::domain::ranking::FeedEngagementAction;
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let actor = Uuid::new_v4();
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    let old = PgFeedRecommendationRepository::new(f.pool.clone());
    old.enqueue_post(e.event_id, post, author, e.occurred_at, 2, 1000, Utc::now())
        .await?;
    assert_eq!(apply(&f, &e).await?, TimelineProjectionOutcomeDto::Adopted);
    let like = event(&f, post, author, actor, "POST_LIKED").await?;
    old.record_engagement(
        like.event_id,
        post,
        actor,
        author,
        FeedEngagementAction::Like,
        like.occurred_at,
    )
    .await?;
    assert!(apply(&f, &like).await.is_err());
    assert_eq!(cursor(&f, post).await?, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select like_events from post_engagement_velocity where post_id=$1"
        )
        .bind(post)
        .fetch_one(&f.pool)
        .await?,
        1
    );
    f.shutdown().await
}
#[tokio::test]
async fn legacy_null_history_and_late_unseen_events_cannot_skip_cursor() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    sqlx::query("update post_event_outbox set aggregate_version=null where event_id=$1")
        .bind(e.event_id)
        .execute(&f.pool)
        .await?;
    let mut legacy = e.clone();
    legacy.aggregate_version = None;
    let next = event(&f, post, author, Uuid::new_v4(), "POST_LIKED").await?;
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    assert!(repo.prepare(&legacy, 100).await.is_err());
    assert!(repo.prepare(&next, 100).await.is_err());
    assert_eq!(receipts(&f).await?, 0);
    f.shutdown().await
}
#[tokio::test]
async fn unknown_types_and_wrong_author_fields_fail_without_consuming_versions() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let create = event(&f, post, author, author, "POST_CREATED").await?;
    apply(&f, &create).await?;
    let unknown = event(&f, post, author, Uuid::new_v4(), "UNKNOWN_OWNER_EVENT").await?;
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    assert!(repo.prepare(&unknown, 100).await.is_err());
    let mut malformed = unknown.clone();
    malformed.event_type = "POST_COMMENTED".into();
    malformed.payload =
        serde_json::json!({"postId":post,"authorId":malformed.actor_id}).to_string();
    assert!(malformed.effect().is_err());
    malformed.payload=serde_json::json!({"postId":Uuid::new_v4(),"authorId":malformed.actor_id,"notifyUserId":author}).to_string();
    assert!(malformed.effect().is_err());
    assert_eq!(cursor(&f, post).await?, 1);
    f.shutdown().await
}
#[tokio::test]
async fn deleted_post_and_deleted_actor_events_record_suppression_without_resurrection(
) -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let actor = Uuid::new_v4();
    let create = event(&f, post, author, author, "POST_CREATED").await?;
    apply(&f, &create).await?;
    let like = event(&f, post, author, actor, "POST_LIKED").await?;
    sqlx::query("insert into post_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) values($1,$2,$1,now())").bind(actor).bind(Uuid::new_v4()).execute(&f.pool).await?;
    assert_eq!(
        apply(&f, &like).await?,
        TimelineProjectionOutcomeDto::Suppressed
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_author_affinities")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    let comment = event(&f, post, author, Uuid::new_v4(), "POST_COMMENTED").await?;
    sqlx::query("update posts set deleted_at=now() where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    assert_eq!(
        apply(&f, &comment).await?,
        TimelineProjectionOutcomeDto::Suppressed
    );
    assert_eq!(cursor(&f, post).await?, 3);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from post_engagement_velocity")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.shutdown().await
}
#[tokio::test]
async fn authoritative_source_and_actual_post_owner_are_rechecked_at_apply() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    repo.prepare(&e, 100).await?;
    sqlx::query("update post_event_outbox set attributes=array['different'] where event_id=$1")
        .bind(e.event_id)
        .execute(&f.pool)
        .await?;
    assert!(apply(&f, &e).await.is_err());
    assert_eq!(receipts(&f).await?, 0);
    sqlx::query("update post_event_outbox set attributes=$2 where event_id=$1")
        .bind(e.event_id)
        .bind(&e.attributes)
        .execute(&f.pool)
        .await?;
    sqlx::query("update posts set author_id=$2 where id=$1")
        .bind(post)
        .bind(Uuid::new_v4())
        .execute(&f.pool)
        .await?;
    assert!(apply(&f, &e).await.is_err());
    assert_eq!(cursor(&f, post).await?, 0);
    f.shutdown().await
}
#[tokio::test]
async fn retired_owner_receipt_survives_simulated_source_absence_and_rejects_unseen_identity(
) -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    sqlx::query("delete from posts where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    assert_eq!(
        apply(&f, &e).await?,
        TimelineProjectionOutcomeDto::Suppressed
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from home_timeline_entries")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    // Current production constraints forbid source pruning beneath immutable
    // attempt history. Prove that rejection first, then inject source absence
    // in this disposable fixture while preserving every audit attempt row.
    assert!(
        sqlx::query("delete from post_event_outbox where event_id=$1")
            .bind(e.event_id)
            .execute(&f.pool)
            .await
            .is_err()
    );
    sqlx::query("alter table post_event_delivery_attempts drop constraint post_event_delivery_attempts_event_id_fkey").execute(&f.pool).await?;
    sqlx::query("delete from post_event_outbox where event_id=$1")
        .bind(e.event_id)
        .execute(&f.pool)
        .await?;
    assert_eq!(
        apply(&f, &e).await?,
        TimelineProjectionOutcomeDto::Duplicate
    );
    let mut unseen = e;
    unseen.event_id = Uuid::new_v4();
    assert!(apply(&f, &unseen).await.is_err());
    f.shutdown().await
}
#[tokio::test]
async fn bounded_prepare_and_forwarder_cross_batch_recovery_do_not_fail_at_page_boundary(
) -> Result<()> {
    use post_service::adapters::r#in::messaging::external_event_forwarder::ExternalEventForwarder;
    use post_service::application::dto::delivery_schedule_dto::DeliveryExecutionConfigDto;
    use post_service::domain::post::DeliveryTarget;
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let actor = Uuid::new_v4();
    let mut target = event(&f, post, author, author, "POST_CREATED").await?;
    for _ in 0..204 {
        target = event(&f, post, author, actor, "POST_LIKED").await?;
    }
    let repo = Arc::new(PgTimelineProjectionRepository::new(f.pool.clone()));
    let TimelineProjectionPlanDto::Events(works) = repo.prepare(&target, 100).await? else {
        panic!("batch")
    };
    assert_eq!(works.len(), 100);
    // Inject a live late-event lease/STARTED marker to isolate prefix recovery
    // from head selection; authority and terminal validation remain real PG.
    let token = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    sqlx::query("update post_event_delivery_jobs set state='LEASED',lease_token=$2,attempt_id=$3,attempt_count=1,lease_expires_at=clock_timestamp()+interval '30 seconds' where event_id=$1 and target='TIMELINE'").bind(target.event_id).bind(token).bind(attempt).execute(&f.pool).await?;
    sqlx::query("insert into post_event_delivery_attempts(attempt_id,event_id,target,source,phase,occurred_at) values($1,$2,'TIMELINE','OUTBOX_WORKER','STARTED',clock_timestamp())").bind(attempt).bind(target.event_id).execute(&f.pool).await?;
    let owned=sqlx::query("select lease_expires_at from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'").bind(target.event_id).fetch_one(&f.pool).await?;
    let claim = post_service::application::dto::delivery_schedule_dto::TargetDeliveryClaimDto {
        event: StoredEvent {
            event_id: target.event_id,
            event_type: target.event_type.clone(),
            aggregate_id: target.aggregate_id,
            aggregate_version: target.aggregate_version,
            actor_id: target.actor_id,
            attributes: target.attributes.clone(),
            payload: target.payload.clone(),
            occurred_at: target.occurred_at,
            created_at: target.created_at,
            published: false,
            published_at: None,
            replay_resolution_id: None,
            origin_traceparent: None,
        },
        target: DeliveryTarget::Timeline,
        lease_token: token,
        attempt_id: attempt,
        attempt_number: 1,
        lease_expires_at: owned.get("lease_expires_at"),
    };
    let ports = ports::Harness::new();
    let schedule = Arc::new(InjectedSchedule {
        schedule: f.schedule(),
        claim: std::sync::Mutex::new(Some(claim)),
    });
    let config = DeliveryExecutionConfigDto {
        effect_timeout_ms: 20000,
        ..Default::default()
    };
    let worker = ExternalEventForwarder::new(
        schedule,
        ports.publisher,
        ports.internal,
        ports.feed_cache,
        ports.post_cache,
        ports.friend,
        repo,
    )
    .with_execution_config(config)?;
    worker
        .republish_target(DeliveryTarget::Timeline, Utc::now(), 1)
        .await?;
    assert_eq!(cursor(&f, post).await?, 205);
    assert_eq!(receipts(&f).await?, 205);
    f.shutdown().await
}

struct InjectedSchedule {
    schedule: post_service::adapters::out::persistence::delivery_schedule::PgDeliverySchedule,
    claim: std::sync::Mutex<
        Option<post_service::application::dto::delivery_schedule_dto::TargetDeliveryClaimDto>,
    >,
}
#[async_trait::async_trait]
impl post_service::application::ports::DeliverySchedulePort for InjectedSchedule {
    async fn claim_target(
        &self,
        _target: post_service::domain::post::DeliveryTarget,
        _older: chrono::DateTime<Utc>,
        _limit: i32,
        _config: &post_service::application::dto::delivery_schedule_dto::DeliveryScheduleConfigDto,
    ) -> Result<post_service::application::dto::delivery_schedule_dto::TargetDeliveryBatchDto> {
        Ok(
            post_service::application::dto::delivery_schedule_dto::TargetDeliveryBatchDto {
                claims: self.claim.lock().unwrap().take().into_iter().collect(),
                reconciled_published_events: 0,
            },
        )
    }
    async fn lease_status(
        &self,
        claim: &post_service::application::dto::delivery_schedule_dto::TargetDeliveryClaimDto,
    ) -> Result<post_service::application::dto::delivery_schedule_dto::DeliveryLeaseStatusDto> {
        self.schedule.lease_status(claim).await
    }
    async fn complete_target(
        &self,
        claim: &post_service::application::dto::delivery_schedule_dto::TargetDeliveryClaimDto,
    ) -> Result<post_service::application::dto::delivery_schedule_dto::DeliveryLeaseOutcomeDto>
    {
        self.schedule.complete_target(claim).await
    }
    async fn fail_target(
        &self,
        claim: &post_service::application::dto::delivery_schedule_dto::TargetDeliveryClaimDto,
    ) -> Result<post_service::application::dto::delivery_schedule_dto::DeliveryLeaseOutcomeDto>
    {
        self.schedule.fail_target(claim).await
    }
}

async fn wait_for_lock(f: &Fixture, pattern: &str) -> Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(5),async {
        loop {
            let waiting:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like $1)").bind(pattern).fetch_one(&f.pool).await?;
            if waiting {return Ok::<_,anyhow::Error>(());}
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await??;
    Ok(())
}
#[tokio::test]
async fn missing_or_replaced_delivery_authority_cannot_write_projection() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    assert!(repo.apply(&request(&e)?).await.is_err());
    assert_eq!(receipts(&f).await?, 0);
    let q = authorized_request(&f, &e).await?;
    sqlx::query("update post_event_delivery_jobs set lease_token=$2 where event_id=$1 and target='TIMELINE'").bind(e.event_id).bind(Uuid::new_v4()).execute(&f.pool).await?;
    assert!(repo.apply(&q).await.is_err());
    assert_eq!(cursor(&f, post).await?, 0);
    f.shutdown().await
}
#[tokio::test]
async fn expired_delivery_authority_is_rechecked_after_post_lock_wait() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    let q = authorized_request(&f, &e).await?;
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select id from posts where id=$1 for update")
        .bind(post)
        .fetch_one(&mut *barrier)
        .await?;
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    let task = tokio::spawn(async move { repo.apply(&q).await });
    wait_for_lock(
        &f,
        "select author_id,deleted_at is not null as deleted from posts%",
    )
    .await?;
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='TIMELINE'").bind(e.event_id).execute(&f.pool).await?;
    barrier.commit().await?;
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(5), task)
            .await??
            .is_err()
    );
    assert_eq!(receipts(&f).await?, 0);
    assert_eq!(cursor(&f, post).await?, 0);
    f.shutdown().await
}
#[tokio::test]
async fn deferred_commit_fence_rolls_back_expiry_during_effect_writes() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    let q = authorized_request(&f, &e).await?;
    sqlx::raw_sql("create function delay_projection_commit() returns trigger language plpgsql as $$ begin perform pg_sleep(0.25); return new; end; $$;create trigger delay_projection_commit before update on timeline_projection_cursors for each row execute function delay_projection_commit();").execute(&f.pool).await?;
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()+interval '150 milliseconds' where event_id=$1 and target='TIMELINE'").bind(e.event_id).execute(&f.pool).await?;
    let error = PgTimelineProjectionRepository::new(f.pool.clone())
        .apply(&q)
        .await
        .expect_err("commit after expiry must rollback");
    assert!(error.to_string().contains("expired before commit"));
    for table in [
        "home_timeline_entries",
        "feed_fanout_jobs",
        "timeline_projection_receipts",
        "timeline_projection_cursors",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("select count(*) from {table}"))
                .fetch_one(&f.pool)
                .await?,
            0
        );
    }
    f.shutdown().await
}
#[tokio::test]
async fn actor_tombstone_does_not_adopt_unproven_deltas_on_live_post() -> Result<()> {
    use post_service::adapters::out::persistence::feed_recommendation_repository::PgFeedRecommendationRepository;
    use post_service::application::ports::FeedFeatureRepositoryPort;
    use post_service::domain::ranking::FeedEngagementAction;
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let actor = Uuid::new_v4();
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    apply(&f, &e).await?;
    let like = event(&f, post, author, actor, "POST_LIKED").await?;
    PgFeedRecommendationRepository::new(f.pool.clone())
        .record_engagement(
            like.event_id,
            post,
            actor,
            author,
            FeedEngagementAction::Like,
            like.occurred_at,
        )
        .await?;
    sqlx::query("insert into post_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) values($1,$2,$1,now())").bind(actor).bind(Uuid::new_v4()).execute(&f.pool).await?;
    assert!(apply(&f, &like).await.is_err());
    assert_eq!(cursor(&f, post).await?, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select like_events from post_engagement_velocity where post_id=$1"
        )
        .bind(post)
        .fetch_one(&f.pool)
        .await?,
        1
    );
    f.shutdown().await
}
#[tokio::test]
async fn actor_deletion_wins_subject_lock_before_delayed_projection() -> Result<()> {
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let actor = Uuid::new_v4();
    let create = event(&f, post, author, author, "POST_CREATED").await?;
    apply(&f, &create).await?;
    let like = event(&f, post, author, actor, "POST_LIKED").await?;
    let q = authorized_request(&f, &like).await?;
    let mut deletion = f.pool.begin().await?;
    sqlx::query("select post_lock_subject_ids($1::uuid[])")
        .bind(vec![actor])
        .execute(&mut *deletion)
        .await?;
    sqlx::query("insert into post_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) values($1,$2,$1,now())").bind(actor).bind(Uuid::new_v4()).execute(&mut *deletion).await?;
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    let task = tokio::spawn(async move { repo.apply(&q).await });
    wait_for_lock(&f, "select post_lock_subject_ids%").await?;
    deletion.commit().await?;
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(5), task).await???,
        TimelineProjectionOutcomeDto::Suppressed
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_author_affinities")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.shutdown().await
}
#[tokio::test]
async fn fanout_page_and_deletion_projection_share_subject_before_post_order() -> Result<()> {
    use post_service::adapters::out::persistence::feed_recommendation_repository::PgFeedRecommendationRepository;
    use post_service::application::dto::timeline_fanout_dto::*;
    use post_service::application::ports::TimelineFanoutRepositoryPort;
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let create = event(&f, post, author, author, "POST_CREATED").await?;
    apply(&f, &create).await?;
    let delete = event(&f, post, author, author, "POST_DELETED").await?;
    let q = authorized_request(&f, &delete).await?;
    let repo = PgFeedRecommendationRepository::new(f.pool.clone());
    let claim = repo
        .claim_jobs(&TimelineFanoutClaimRequestDto {
            lease_timeout: std::time::Duration::from_secs(60),
            limit: 1,
            max_page_attempts: 8,
            retry_base: std::time::Duration::from_secs(1),
            excluded_job_ids: vec![],
        })
        .await?
        .remove(0);
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select job_id from feed_fanout_jobs where job_id=$1 for update")
        .bind(claim.job.job_id)
        .fetch_one(&mut *barrier)
        .await?;
    let page = TimelineFanoutPageDto {
        follower_ids: vec![Uuid::new_v4()],
        next_cursor: None,
        expires_at: Utc::now() + Duration::days(14),
    };
    let task = tokio::spawn(async move { repo.apply_page(&claim, &page).await });
    wait_for_lock(&f, "%from feed_fanout_jobs where job_id=$1%for update%").await?;
    let projection = PgTimelineProjectionRepository::new(f.pool.clone());
    let retire = tokio::spawn(async move { projection.apply(&q).await });
    wait_for_lock(&f, "select post_lock_subject_ids%").await?;
    barrier.commit().await?;
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(5), task).await???,
        TimelineFanoutOutcomeDto::Applied
    );
    tokio::time::timeout(std::time::Duration::from_secs(5), retire).await???;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from home_timeline_entries")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.shutdown().await
}
#[tokio::test]
async fn sqlx_twenty_to_current_preserves_history_and_never_invents_cursor() -> Result<()> {
    let f = Fixture::new(20).await?;
    let (post, author) = seed(&f).await?;
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    sqlx::query(
        "update post_event_outbox set aggregate_version=null,published=true where event_id=$1",
    )
    .bind(e.event_id)
    .execute(&f.pool)
    .await?;
    let before: serde_json::Value =
        sqlx::query_scalar("select jsonb_agg(to_jsonb(e)) from post_event_outbox e")
            .fetch_one(&f.pool)
            .await?;
    f.upgrade().await?;
    let after: serde_json::Value =
        sqlx::query_scalar("select jsonb_agg(to_jsonb(e)) from post_event_outbox e")
            .fetch_one(&f.pool)
            .await?;
    assert_eq!(before, after);
    assert_eq!(receipts(&f).await?, 0);
    assert_eq!(cursor(&f, post).await?, 0);
    f.shutdown().await
}
#[tokio::test]
async fn missing_creation_genesis_and_unproven_high_fanout_state_require_adoption() -> Result<()> {
    use post_service::adapters::out::persistence::feed_recommendation_repository::PgFeedRecommendationRepository;
    use post_service::application::ports::TimelineFanoutRepositoryPort;
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let create = event(&f, post, author, author, "POST_CREATED").await?;
    let old = PgFeedRecommendationRepository::new(f.pool.clone());
    old.enqueue_post(
        create.event_id,
        post,
        author,
        create.occurred_at,
        2000,
        1000,
        Utc::now(),
    )
    .await?;
    assert!(apply(&f, &create).await.is_err());
    assert_eq!(receipts(&f).await?, 0);
    let second = event(&f, post, author, author, "POST_CREATED").await?;
    assert!(apply(&f, &second).await.is_err());
    f.shutdown().await
}

#[tokio::test]
async fn actual_owner_use_cases_and_native_timeline_worker_preserve_all_six_event_contracts(
) -> Result<()> {
    use post_service::adapters::out::persistence::post_repository::PgPostRepository;
    use post_service::adapters::r#in::messaging::external_event_forwarder::ExternalEventForwarder;
    use post_service::application::dto::post_dto::{
        CreateCommentDto, CreatePostDto, DeleteCommentDto, DeletePostDto,
    };
    use post_service::domain::post::DeliveryTarget;
    let f = Fixture::new(24).await?;
    let ports = ports::Harness::new();
    let author = Uuid::new_v4();
    let actor = Uuid::new_v4();
    let owner = Arc::new(PgPostRepository::new(f.pool.clone()));
    let mut service = (*ports.service).clone();
    service.posts = owner.clone();
    service.likes = Arc::new(
        post_service::adapters::out::persistence::post_repository::PgPostLikeRepository::new(
            f.pool.clone(),
        ),
    );
    let projection = Arc::new(PgTimelineProjectionRepository::new(f.pool.clone()));
    let worker = ExternalEventForwarder::new(
        Arc::new(f.schedule()),
        ports.publisher,
        ports.internal,
        ports.feed_cache,
        ports.post_cache,
        ports.friend,
        projection,
    );
    let created = service
        .create_post(CreatePostDto {
            author_id: author.to_string(),
            content: "actual producer fixture".into(),
            visibility: "PUBLIC".into(),
        })
        .await?;
    let post = Uuid::parse_str(&created.id)?;
    worker
        .republish_target(DeliveryTarget::Timeline, Utc::now(), 10)
        .await?;
    let comment = service
        .create_comment(CreateCommentDto {
            post_id: post.to_string(),
            author_id: actor.to_string(),
            content: "actual comment fixture".into(),
        })
        .await?;
    worker
        .republish_target(DeliveryTarget::Timeline, Utc::now(), 10)
        .await?;
    service.like_post(post, actor).await?;
    worker
        .republish_target(DeliveryTarget::Timeline, Utc::now(), 10)
        .await?;
    service.unlike_post(post, actor).await?;
    worker
        .republish_target(DeliveryTarget::Timeline, Utc::now(), 10)
        .await?;
    service
        .delete_comment(DeleteCommentDto {
            comment_id: comment.id,
            actor_id: actor.to_string(),
            request_id: None,
            ip_address: None,
            user_agent: None,
        })
        .await?;
    worker
        .republish_target(DeliveryTarget::Timeline, Utc::now(), 10)
        .await?;
    service
        .delete_post(DeletePostDto {
            post_id: post.to_string(),
            actor_id: author.to_string(),
            request_id: None,
            ip_address: None,
            user_agent: None,
        })
        .await?;
    worker
        .republish_target(DeliveryTarget::Timeline, Utc::now(), 10)
        .await?;
    assert_eq!(cursor(&f, post).await?, 6);
    assert_eq!(receipts(&f).await?, 6);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from event_publications where target='TIMELINE'"
        )
        .fetch_one(&f.pool)
        .await?,
        6
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_fanout_jobs")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    let row = sqlx::query(
        "select like_events,comment_events from post_engagement_velocity where post_id=$1",
    )
    .bind(post)
    .fetch_one(&f.pool)
    .await?;
    assert_eq!(row.get::<i64, _>("like_events"), 0);
    assert_eq!(row.get::<i64, _>("comment_events"), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_affinity_source_contributions")
            .fetch_one(&f.pool)
            .await?,
        4
    );
    let sequences: Vec<i64> = sqlx::query_scalar(
        "select source_sequence from feed_affinity_source_contributions order by source_sequence",
    )
    .fetch_all(&f.pool)
    .await?;
    assert_eq!(sequences, vec![1, 2, 3, 4]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_affinity_legacy_effect_observations o join feed_affinity_source_contributions c using(namespace,source_id) where o.namespace='OUTBOX' and o.source_identity_sha256=c.identity_sha256 and o.source_sequence=c.source_sequence")
            .fetch_one(&f.pool).await?,
        4
    );
    f.shutdown().await
}
#[tokio::test]
async fn lifecycle_deletion_locks_foreign_post_owners_before_counter_and_feature_cleanup(
) -> Result<()> {
    use post_service::adapters::out::persistence::user_lifecycle_repository::PgUserLifecycleRepository;
    use post_service::application::ports::UserLifecycleRepositoryPort;
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let (other, other_author) = seed(&f).await?;
    let actor = Uuid::new_v4();
    sqlx::query("insert into comments(id,post_id,author_id,content,created_at) values($1,$2,$3,'actor comment',now())").bind(Uuid::new_v4()).bind(post).bind(actor).execute(&f.pool).await?;
    sqlx::query("insert into post_likes(post_id,user_id,created_at) values($1,$2,now())")
        .bind(other)
        .bind(actor)
        .execute(&f.pool)
        .await?;
    sqlx::query("update posts set comment_count=1 where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    sqlx::query("update posts set like_count=1 where id=$1")
        .bind(other)
        .execute(&f.pool)
        .await?;
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select post_lock_subject_ids($1::uuid[])")
        .bind(vec![author, other_author])
        .execute(&mut *barrier)
        .await?;
    let repo = PgUserLifecycleRepository::new(f.pool.clone());
    let saga = Uuid::new_v4();
    let retention = Utc::now() + Duration::days(30);
    let task = tokio::spawn(async move {
        repo.apply_user_deletion(saga, actor, actor, retention)
            .await
    });
    wait_for_lock(&f, "select post_lock_subject_ids%").await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from post_user_deletion_tombstones")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    barrier.commit().await?;
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(5), task)
            .await???
            .is_empty()
    );
    let row = sqlx::query("select comment_count from posts where id=$1")
        .bind(post)
        .fetch_one(&f.pool)
        .await?;
    assert_eq!(row.get::<i64, _>("comment_count"), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select like_count from posts where id=$1")
            .bind(other)
            .fetch_one(&f.pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_user_deletion_tombstones where user_id=$1"
        )
        .bind(actor)
        .fetch_one(&f.pool)
        .await?,
        1
    );
    f.shutdown().await
}

#[tokio::test]
async fn actual_snapshot_impression_and_recommendation_writers_prelock_complete_subject_sets(
) -> Result<()> {
    use post_service::adapters::out::persistence::feed_recommendation_repository::PgFeedRecommendationRepository;
    use post_service::application::ports::FeedRecommendationRepositoryPort;
    use post_service::domain::recommendation::*;
    let f = Fixture::new(22).await?;
    let (post, mut author) = seed(&f).await?;
    let mut viewer = Uuid::new_v4();
    let author_first: bool =
        sqlx::query_scalar("select hashtextextended($1::text,0)<hashtextextended($2::text,0)")
            .bind(author)
            .bind(viewer)
            .fetch_one(&f.pool)
            .await?;
    if !author_first {
        std::mem::swap(&mut author, &mut viewer);
        sqlx::query("update posts set author_id=$2 where id=$1")
            .bind(post)
            .bind(author)
            .execute(&f.pool)
            .await?;
    }
    let now = Utc::now();
    let snapshot = FeedSnapshot {
        snapshot_id: Uuid::new_v4(),
        viewer_id: viewer,
        request_id: "native snapshot barrier".into(),
        session_id: None,
        rank_version: "feed-linear-v1".into(),
        fallback_used: false,
        failed_sources: vec![],
        created_at: now,
        expires_at: now + Duration::days(1),
        items: vec![FeedSnapshotItem {
            post_id: post,
            author_id: author,
            source: FeedCandidateSource::SelfHistory,
            reason_code: "probe".into(),
            score_micros: 1,
            position: 0,
            impression_id: Uuid::new_v4(),
            propensity_micros: 1000000,
        }],
    };
    let repo = PgFeedRecommendationRepository::new(f.pool.clone());
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select post_lock_subject_ids($1::uuid[])")
        .bind(vec![author])
        .execute(&mut *barrier)
        .await?;
    let writer = repo.clone();
    let data = snapshot.clone();
    let task = tokio::spawn(async move { writer.create_snapshot(&data).await });
    wait_for_lock(&f, "select post_lock_subject_ids%").await?;
    // With the old header-before-item order the snapshot owns viewer while
    // waiting for author. This reverse acquisition then deadlocks. New writer
    // waits on author before any row or viewer lock, so this completes.
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        sqlx::query("select post_lock_subject_ids($1::uuid[])")
            .bind(vec![viewer])
            .execute(&mut *barrier),
    )
    .await??;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_recommendation_snapshots")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    barrier.commit().await?;
    tokio::time::timeout(std::time::Duration::from_secs(5), task).await???;
    let impression = FeedRecommendationImpression {
        impression_id: snapshot.items[0].impression_id,
        snapshot_id: snapshot.snapshot_id,
        viewer_id: viewer,
        post_id: post,
        author_id: author,
        source: FeedCandidateSource::SelfHistory,
        reason_code: "probe".into(),
        position: 0,
        rank_version: snapshot.rank_version.clone(),
        request_id: snapshot.request_id.clone(),
        session_id: None,
        score_micros: 1,
        propensity_micros: 1000000,
        occurred_at: now,
    };
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select post_lock_subject_ids($1::uuid[])")
        .bind(vec![author])
        .execute(&mut *barrier)
        .await?;
    let writer = repo.clone();
    let data = impression.clone();
    let task = tokio::spawn(async move { writer.record_impressions(&[data]).await });
    wait_for_lock(&f, "select post_lock_subject_ids%").await?;
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        sqlx::query("select post_lock_subject_ids($1::uuid[])")
            .bind(vec![viewer])
            .execute(&mut *barrier),
    )
    .await??;
    barrier.commit().await?;
    tokio::time::timeout(std::time::Duration::from_secs(5), task).await???;
    let click = RecommendationEvent {
        event_id: Uuid::new_v4(),
        schema_version: 1,
        occurred_at: now,
        viewer_id: viewer,
        subject_type: RecommendationSubjectType::Post,
        subject_id: post,
        actor_id: Some(viewer),
        surface: RecommendationSurface::HomeFeed,
        position: Some(0),
        request_id: Some(snapshot.request_id),
        session_id: None,
        action: RecommendationAction::Click,
        source: "native".into(),
        model_version: "feed-linear-v1".into(),
        metadata_json: "{}".into(),
        propensity: Some(1.0),
        impression_id: Some(impression.impression_id),
        dwell_ms: None,
    };
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select post_lock_subject_ids($1::uuid[])")
        .bind(vec![author])
        .execute(&mut *barrier)
        .await?;
    let writer = repo.clone();
    let task = tokio::spawn(async move { writer.append_event(&click).await });
    wait_for_lock(&f, "select post_lock_subject_ids%").await?;
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        sqlx::query("select post_lock_subject_ids($1::uuid[])")
            .bind(vec![viewer])
            .execute(&mut *barrier),
    )
    .await??;
    barrier.commit().await?;
    assert!(tokio::time::timeout(std::time::Duration::from_secs(5), task).await???);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from feed_author_affinities where viewer_id=$1 and author_id=$2"
        )
        .bind(viewer)
        .bind(author)
        .fetch_one(&f.pool)
        .await?,
        1
    );
    f.shutdown().await
}

#[derive(Clone)]
struct SlowCommittedProjection {
    repo: PgTimelineProjectionRepository,
}
#[async_trait::async_trait]
impl TimelineProjectionRepositoryPort for SlowCommittedProjection {
    async fn prepare(
        &self,
        event: &TimelineProjectionEventDto,
        limit: i32,
    ) -> Result<TimelineProjectionPlanDto> {
        self.repo.prepare(event, limit).await
    }
    async fn apply(&self, q: &TimelineProjectionApplyDto) -> Result<TimelineProjectionOutcomeDto> {
        let outcome = self.repo.apply(q).await?;
        // Lost/late acknowledgement after the actual owner transaction commits.
        tokio::time::sleep(std::time::Duration::from_millis(400)).await;
        Ok(outcome)
    }
}
#[tokio::test]
async fn production_head_recovers_committed_effect_after_deadline_with_frozen_single_failure_budget(
) -> Result<()> {
    use post_service::adapters::r#in::messaging::external_event_forwarder::ExternalEventForwarder;
    use post_service::application::dto::delivery_schedule_dto::DeliveryExecutionConfigDto;
    use post_service::application::ports::DeliverySchedulePort;
    use post_service::domain::post::DeliveryTarget;
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    let ports = ports::Harness::new();
    let mut config = DeliveryExecutionConfigDto {
        effect_timeout_ms: 200,
        completion_reserve_ms: 100,
        ..Default::default()
    };
    config.schedule.max_attempts = 1;
    config.schedule.lease_duration_ms = 1000;
    let worker = ExternalEventForwarder::new(
        Arc::new(f.schedule()),
        ports.publisher,
        ports.internal,
        ports.feed_cache,
        ports.post_cache,
        ports.friend,
        Arc::new(SlowCommittedProjection {
            repo: PgTimelineProjectionRepository::new(f.pool.clone()),
        }),
    )
    .with_execution_config(config.clone())?;
    worker
        .republish_target(DeliveryTarget::Timeline, Utc::now(), 1)
        .await?;
    assert_eq!(cursor(&f, post).await?, 1);
    assert_eq!(receipts(&f).await?, 1);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'"
        )
        .bind(e.event_id)
        .fetch_one(&f.pool)
        .await?,
        "LEASED"
    );
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='TIMELINE'").bind(e.event_id).execute(&f.pool).await?;
    let retry = f
        .schedule()
        .claim_target(DeliveryTarget::Timeline, Utc::now(), 1, &config.schedule)
        .await?
        .claims
        .remove(0);
    assert_eq!(retry.attempt_number, 2);
    let row=sqlx::query("select attempt_count,progress_credits,max_attempts from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'").bind(e.event_id).fetch_one(&f.pool).await?;
    assert_eq!(row.get::<i32, _>("progress_credits"), 1);
    assert_eq!(row.get::<i32, _>("max_attempts"), 1);
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where event_id=$1 and phase='PROGRESSED'").bind(e.event_id).fetch_one(&f.pool).await?,1);
    assert!(matches!(
        PgTimelineProjectionRepository::new(f.pool.clone())
            .prepare(&e, 100)
            .await?,
        TimelineProjectionPlanDto::Duplicate
    ));
    f.schedule().complete_target(&retry).await?;
    assert_eq!(receipts(&f).await?, 1);
    f.shutdown().await
}

async fn inject_late_claim(
    f: &Fixture,
    e: &TimelineProjectionEventDto,
) -> Result<post_service::application::dto::delivery_schedule_dto::TargetDeliveryClaimDto> {
    use post_service::application::dto::delivery_schedule_dto::TargetDeliveryClaimDto;
    use post_service::domain::post::DeliveryTarget;
    let token = Uuid::new_v4();
    let attempt = Uuid::new_v4();
    sqlx::query("update post_event_outbox set delivery_required_targets=array['TIMELINE','CACHE','INTERNAL'] where event_id=$1 and delivery_required_targets is null").bind(e.event_id).execute(&f.pool).await?;
    let row=sqlx::query("update post_event_delivery_jobs set state='LEASED',lease_kind='AUTO',replay_restore_state=null,lease_token=$2,attempt_id=$3,attempt_count=attempt_count+1,max_attempts=1,lease_expires_at=clock_timestamp()+interval '1 second' where event_id=$1 and target='TIMELINE' and attempt_count-progress_credits<1 and (state in ('PENDING','RETRY') or (state='LEASED' and lease_expires_at<=clock_timestamp())) returning attempt_count,lease_expires_at")
        .bind(e.event_id).bind(token).bind(attempt).fetch_one(&f.pool).await?;
    sqlx::query("insert into post_event_delivery_attempts(attempt_id,event_id,target,source,phase,occurred_at) values($1,$2,'TIMELINE','OUTBOX_WORKER','STARTED',clock_timestamp())").bind(attempt).bind(e.event_id).execute(&f.pool).await?;
    Ok(TargetDeliveryClaimDto {
        event: StoredEvent {
            event_id: e.event_id,
            event_type: e.event_type.clone(),
            aggregate_id: e.aggregate_id,
            aggregate_version: e.aggregate_version,
            actor_id: e.actor_id,
            attributes: e.attributes.clone(),
            payload: e.payload.clone(),
            occurred_at: e.occurred_at,
            created_at: e.created_at,
            published: false,
            published_at: None,
            replay_resolution_id: None,
            origin_traceparent: None,
        },
        target: DeliveryTarget::Timeline,
        lease_token: token,
        attempt_id: attempt,
        attempt_number: row.get("attempt_count"),
        lease_expires_at: row.get("lease_expires_at"),
    })
}
#[tokio::test]
async fn six_healthy_deadline_expiries_refund_only_their_own_committed_prefix_and_finish(
) -> Result<()> {
    use post_service::adapters::r#in::messaging::external_event_forwarder::ExternalEventForwarder;
    use post_service::application::dto::delivery_schedule_dto::DeliveryExecutionConfigDto;
    use post_service::application::ports::DeliverySchedulePort;
    use post_service::domain::post::{DeadLetterReplayError, DeliveryTarget};
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let actor = Uuid::new_v4();
    let mut target = event(&f, post, author, author, "POST_CREATED").await?;
    for _ in 0..6 {
        target = event(&f, post, author, actor, "POST_LIKED").await?;
    }
    let config = DeliveryExecutionConfigDto {
        effect_timeout_ms: 200,
        completion_reserve_ms: 100,
        ..Default::default()
    };
    // Controlled late-head selection, native job/attempt ownership, cursor,
    // receipts, prefix effects, credit maintenance and completion. Production
    // head reachability is separately exercised by the preceding normal-head test.
    for expected in 1..=6 {
        let claim = inject_late_claim(&f, &target).await?;
        let attempt = claim.attempt_id;
        let ports = ports::Harness::new();
        let worker = ExternalEventForwarder::new(
            Arc::new(InjectedSchedule {
                schedule: f.schedule(),
                claim: std::sync::Mutex::new(Some(claim)),
            }),
            ports.publisher,
            ports.internal,
            ports.feed_cache,
            ports.post_cache,
            ports.friend,
            Arc::new(SlowCommittedProjection {
                repo: PgTimelineProjectionRepository::new(f.pool.clone()),
            }),
        )
        .with_execution_config(config.clone())?;
        worker
            .republish_target(DeliveryTarget::Timeline, Utc::now(), 1)
            .await?;
        assert_eq!(cursor(&f, post).await?, expected);
        sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='TIMELINE'").bind(target.event_id).execute(&f.pool).await?;
        assert_eq!(
            sqlx::query_scalar::<_, i32>("select post_timeline_pending_progress_credit($1,$2)")
                .bind(target.event_id)
                .bind(attempt)
                .fetch_one(&f.pool)
                .await?,
            1
        );
        let denied = f
            .resolve(target.event_id, Uuid::new_v4())
            .await
            .expect_err("healthy positive prefix does not authorize exhausted-target recovery");
        assert!(matches!(
            denied.downcast_ref::<DeadLetterReplayError>(),
            Some(DeadLetterReplayError::NotDeadLettered)
        ));
        // Independent bounded maintenance can credit this live-source prefix
        // even while its normal head remains earlier in the aggregate.
        // SKIP LOCKED may defer a parent while an earlier failed transaction
        // releases it. The contract is bounded eventual credit, not an exact
        // two-tick deadline. Keep every later identity/count assertion strict.
        let maintenance = f.schedule();
        tokio::time::timeout(std::time::Duration::from_secs(5),async {
            for _ in 0..100 {
                maintenance.claim_target(DeliveryTarget::External,Utc::now(),1,&Default::default()).await?;
                let credited:i32=sqlx::query_scalar("select progress_credits from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'")
                    .bind(target.event_id).fetch_one(&f.pool).await?;
                if i64::from(credited)==expected {return Ok::<_,anyhow::Error>(());}
                tokio::time::sleep(std::time::Duration::from_millis(10)).await;
            }
            anyhow::bail!("own committed progress not credited within 100 bounded ticks")
        }).await??;
        let row=sqlx::query("select attempt_count,progress_credits,max_attempts from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'").bind(target.event_id).fetch_one(&f.pool).await?;
        assert_eq!(i64::from(row.get::<i32, _>("attempt_count")), expected);
        assert_eq!(i64::from(row.get::<i32, _>("progress_credits")), expected);
        assert_eq!(row.get::<i32, _>("max_attempts"), 1);
        assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where attempt_id=$1 and phase='PROGRESSED'").bind(attempt).fetch_one(&f.pool).await?,1);
        assert!(
            sqlx::query_scalar::<_, bool>(
                "select dead_lettered_at is null from post_event_outbox where event_id=$1"
            )
            .bind(target.event_id)
            .fetch_one(&f.pool)
            .await?
        );
    }
    let claim = inject_late_claim(&f, &target).await?;
    let ports = ports::Harness::new();
    let worker = ExternalEventForwarder::new(
        Arc::new(InjectedSchedule {
            schedule: f.schedule(),
            claim: std::sync::Mutex::new(Some(claim)),
        }),
        ports.publisher,
        ports.internal,
        ports.feed_cache,
        ports.post_cache,
        ports.friend,
        Arc::new(PgTimelineProjectionRepository::new(f.pool.clone())),
    )
    .with_execution_config(config)?;
    worker
        .republish_target(DeliveryTarget::Timeline, Utc::now(), 1)
        .await?;
    assert_eq!(cursor(&f, post).await?, 7);
    assert_eq!(receipts(&f).await?, 7);
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where event_id=$1 and phase='PROGRESSED'").bind(target.event_id).fetch_one(&f.pool).await?,6);
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where event_id=$1 and phase='SUCCEEDED'").bind(target.event_id).fetch_one(&f.pool).await?,1);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select state from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'"
        )
        .bind(target.event_id)
        .fetch_one(&f.pool)
        .await?,
        "SUCCEEDED"
    );
    f.shutdown().await
}
#[tokio::test]
async fn expiry_without_own_attempt_progress_still_exhausts_and_other_targets_cannot_receive_credit(
) -> Result<()> {
    use post_service::application::ports::DeliverySchedulePort;
    use post_service::domain::post::DeliveryTarget;
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    let claim = inject_late_claim(&f, &e).await?;
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='TIMELINE'").bind(e.event_id).execute(&f.pool).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i32>("select post_timeline_pending_progress_credit($1,$2)")
            .bind(e.event_id)
            .bind(claim.attempt_id)
            .fetch_one(&f.pool)
            .await?,
        0
    );
    assert!(f
        .schedule()
        .claim_target(DeliveryTarget::Timeline, Utc::now(), 1, &Default::default())
        .await?
        .claims
        .is_empty());
    let row=sqlx::query("select state,progress_credits from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'").bind(e.event_id).fetch_one(&f.pool).await?;
    assert_eq!(row.get::<String, _>("state"), "DEAD");
    assert_eq!(row.get::<i32, _>("progress_credits"), 0);
    assert!(sqlx::query("update post_event_delivery_jobs set attempt_count=1,progress_credits=1 where event_id=$1 and target='INTERNAL'").bind(e.event_id).execute(&f.pool).await.is_err());
    f.shutdown().await
}

#[tokio::test]
async fn progress_maintenance_bounds_candidate_scan_and_retires_no_progress_checks() -> Result<()> {
    use post_service::adapters::out::persistence::timeline_progress_sql::{
        AFTER_CANDIDATES_SQL, CANDIDATES_SQL,
    };
    use post_service::application::ports::DeliverySchedulePort;
    use post_service::domain::post::DeliveryTarget;
    let f = Fixture::new(22).await?;
    sqlx::query("insert into post_event_outbox(event_id,event_type,aggregate_id,actor_id,attributes,payload,occurred_at,created_at,published) select gen_random_uuid(),'PROGRESS_MAINTENANCE_PROBE',gen_random_uuid(),null,array[]::text[],'{}',now(),now()-interval '1 second',false from generate_series(1,30000)").execute(&f.pool).await?;
    sqlx::query("update post_event_delivery_jobs set state='LEASED',attempt_count=1,lease_token=gen_random_uuid(),attempt_id=gen_random_uuid(),lease_expires_at=clock_timestamp()-interval '1 second' where target='TIMELINE'").execute(&f.pool).await?;
    sqlx::query("analyze post_event_delivery_jobs")
        .execute(&f.pool)
        .await?;
    sqlx::query("analyze post_event_outbox")
        .execute(&f.pool)
        .await?;
    let plan: serde_json::Value = sqlx::query_scalar(&format!(
        "explain (analyze,buffers,format json) {CANDIDATES_SQL}"
    ))
    .bind(100i64)
    .fetch_one(&f.pool)
    .await?;
    fn inspect(v: &serde_json::Value, scans: &mut usize) {
        if let Some(o) = v.as_object() {
            if o.get("Relation Name").and_then(serde_json::Value::as_str)
                == Some("post_event_delivery_jobs")
            {
                assert!(matches!(
                    o["Node Type"].as_str(),
                    Some("Index Scan" | "Index Only Scan")
                ));
                assert_eq!(
                    o["Index Name"],
                    "post_delivery_timeline_progress_expiry_idx"
                );
                assert!(o["Actual Rows"].as_u64().unwrap() <= 100);
                assert_eq!(
                    o.get("Rows Removed by Filter")
                        .and_then(serde_json::Value::as_u64)
                        .unwrap_or(0),
                    0
                );
                *scans += 1;
            }
            for child in o.values() {
                inspect(child, scans);
            }
        } else if let Some(a) = v.as_array() {
            for child in a {
                inspect(child, scans);
            }
        }
    }
    let mut scans = 0;
    inspect(&plan, &mut scans);
    assert_eq!(scans, 1);
    println!("TIMELINE_PROGRESS_ZERO_PLAN {plan}");
    let selected = sqlx::query(CANDIDATES_SQL)
        .bind(100i64)
        .fetch_all(&f.pool)
        .await?;
    let last = selected.last().unwrap();
    let seek: serde_json::Value = sqlx::query_scalar(&format!(
        "explain (analyze,buffers,format json) {AFTER_CANDIDATES_SQL}"
    ))
    .bind(100i64)
    .bind(last.get::<chrono::DateTime<Utc>, _>("candidate_expiry"))
    .bind(last.get::<Uuid, _>("candidate_event_id"))
    .fetch_one(&f.pool)
    .await?;
    let mut scans = 0;
    inspect(&seek, &mut scans);
    assert_eq!(scans, 1);
    assert!(seek
        .to_string()
        .contains("ROW(lease_expires_at, event_id) > ROW("));
    println!("TIMELINE_PROGRESS_SEEK_PLAN {seek}");
    f.schedule()
        .claim_target(
            DeliveryTarget::External,
            Utc::now(),
            100,
            &Default::default(),
        )
        .await?;
    let first=sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_jobs where target='TIMELINE' and progress_checked_attempt_id=attempt_id").fetch_one(&f.pool).await?;
    // Independent maintenance checks at most 100; target head preparation
    // may inspect up to another 100 parents in the same call.
    assert!((100..=200).contains(&first));
    f.schedule()
        .claim_target(
            DeliveryTarget::External,
            Utc::now(),
            100,
            &Default::default(),
        )
        .await?;
    let second=sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_jobs where target='TIMELINE' and progress_checked_attempt_id=attempt_id").fetch_one(&f.pool).await?;
    assert!((first + 100..=first + 200).contains(&second));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_jobs where progress_credits<>0"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase='PROGRESSED'"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    // A real owner-validated progress receipt is then mixed into the same
    // 30k backlog. The earlier negative windows are retired, not rescanned.
    let (post, author) = seed(&f).await?;
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    let claim = inject_late_claim(&f, &e).await?;
    let mut q = request(&e)?;
    q.authority = Some(claim.clone());
    PgTimelineProjectionRepository::new(f.pool.clone())
        .apply(&q)
        .await?;
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 day' where event_id=$1 and target='TIMELINE'").bind(e.event_id).execute(&f.pool).await?;
    let mixed: serde_json::Value = sqlx::query_scalar(&format!(
        "explain (analyze,buffers,format json) {CANDIDATES_SQL}"
    ))
    .bind(100i64)
    .fetch_one(&f.pool)
    .await?;
    let mut scans = 0;
    inspect(&mixed, &mut scans);
    assert_eq!(scans, 1);
    println!("TIMELINE_PROGRESS_MIXED_PLAN {mixed}");
    f.schedule()
        .claim_target(
            DeliveryTarget::External,
            Utc::now(),
            100,
            &Default::default(),
        )
        .await?;
    let third=sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_jobs where target='TIMELINE' and progress_checked_attempt_id=attempt_id").fetch_one(&f.pool).await?;
    assert!((second + 100..=second + 200).contains(&third));
    assert_eq!(sqlx::query_scalar::<_,i32>("select progress_credits from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'").bind(e.event_id).fetch_one(&f.pool).await?,1);
    f.schedule()
        .claim_target(
            DeliveryTarget::External,
            Utc::now(),
            100,
            &Default::default(),
        )
        .await?;
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where attempt_id=$1 and phase='PROGRESSED'").bind(claim.attempt_id).fetch_one(&f.pool).await?,1);
    f.shutdown().await
}

#[tokio::test]
async fn purge_waits_for_lifecycle_subject_before_parent_cascade() -> Result<()> {
    use post_service::adapters::out::persistence::post_repository::PgPostRepository;
    use post_service::adapters::out::persistence::user_lifecycle_repository::PgUserLifecycleRepository;
    use post_service::application::ports::{PostRepositoryPort, UserLifecycleRepositoryPort};
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    let actor = Uuid::new_v4();
    let comment = Uuid::new_v4();
    sqlx::query("insert into comments(id,post_id,author_id,content,created_at) values($1,$2,$3,'live actor comment',now())")
        .bind(comment).bind(post).bind(actor).execute(&f.pool).await?;
    sqlx::query("update posts set deleted_at=now(),retention_until=now()-interval '1 day',comment_count=1 where id=$1")
        .bind(post).execute(&f.pool).await?;
    // Force the actual lifecycle writer to hold the actor comment while its
    // counter update waits on the parent. Purge must wait on the subject gate,
    // before it can acquire that parent and cascade back onto the comment.
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select id from posts where id=$1 for update")
        .bind(post)
        .execute(&mut *barrier)
        .await?;
    let repo = PgUserLifecycleRepository::new(f.pool.clone());
    let deletion = tokio::spawn(async move {
        repo.apply_user_deletion(
            Uuid::new_v4(),
            actor,
            actor,
            Utc::now() + Duration::days(30),
        )
        .await
    });
    wait_for_lock(&f, "%update posts p%comment_count%removed_count%").await?;
    let repo = PgPostRepository::new(f.pool.clone());
    let purge = tokio::spawn(async move {
        repo.purge_expired_posts(
            Utc::now(),
            author,
            Some("purge-lifecycle-barrier".into()),
            None,
            None,
            false,
        )
        .await
    });
    wait_for_lock(&f, "select post_lock_subject_ids%").await?;
    barrier.commit().await?;
    tokio::time::timeout(std::time::Duration::from_secs(5), deletion).await???;
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), purge).await???;
    assert_eq!(result.post_ids, vec![post]);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from comments where id=$1")
            .bind(comment)
            .fetch_one(&f.pool)
            .await?,
        0
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_audit_events where action='POST_PURGED' and request_id='purge-lifecycle-barrier'").fetch_one(&f.pool).await?,1);
    f.shutdown().await
}

#[tokio::test]
async fn purge_rechecks_retention_after_owner_deletion_wins_lock() -> Result<()> {
    use post_service::adapters::out::persistence::post_repository::PgPostRepository;
    use post_service::adapters::out::persistence::user_lifecycle_repository::PgUserLifecycleRepository;
    use post_service::application::ports::{PostRepositoryPort, UserLifecycleRepositoryPort};
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    sqlx::query(
        "update posts set deleted_at=now(),retention_until=now()-interval '1 day' where id=$1",
    )
    .bind(post)
    .execute(&f.pool)
    .await?;
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select id from posts where id=$1 for update")
        .bind(post)
        .execute(&mut *barrier)
        .await?;
    let repo = PgUserLifecycleRepository::new(f.pool.clone());
    let deletion = tokio::spawn(async move {
        repo.apply_user_deletion(
            Uuid::new_v4(),
            author,
            author,
            Utc::now() + Duration::days(30),
        )
        .await
    });
    wait_for_lock(&f, "%update posts%where author_id=$1%").await?;
    let repo = PgPostRepository::new(f.pool.clone());
    let purge = tokio::spawn(async move {
        repo.purge_expired_posts(
            Utc::now(),
            author,
            Some("purge-retention-recheck".into()),
            None,
            None,
            false,
        )
        .await
    });
    wait_for_lock(&f, "select post_lock_subject_ids%").await?;
    barrier.commit().await?;
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(5), deletion).await???,
        vec![post]
    );
    let result = tokio::time::timeout(std::time::Duration::from_secs(5), purge).await???;
    assert!(result.post_ids.is_empty());
    assert!(result.comment_ids.is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from posts where id=$1")
            .bind(post)
            .fetch_one(&f.pool)
            .await?,
        1
    );
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_audit_events where action='POST_PURGED' and request_id='purge-retention-recheck'").fetch_one(&f.pool).await?,0);
    f.shutdown().await
}

#[tokio::test]
async fn sqlx_21_to_22_preserves_live_claim_and_receipt_then_credits_expired_progress() -> Result<()>
{
    use post_service::application::dto::delivery_schedule_dto::TargetDeliveryClaimDto;
    use post_service::application::ports::DeliverySchedulePort;
    use post_service::domain::post::DeliveryTarget;
    let f = Fixture::new(21).await?;
    let (post, author) = seed(&f).await?;
    let e = event(&f, post, author, author, "POST_CREATED").await?;
    let attempt = Uuid::new_v4();
    let token = Uuid::new_v4();
    // Provision the existing schema-21 claim; current scheduling code needs
    // schema 22, so it is deliberately not used before the actual upgrade.
    sqlx::query("update post_event_outbox set delivery_required_targets=array['TIMELINE','CACHE','INTERNAL'] where event_id=$1")
        .bind(e.event_id).execute(&f.pool).await?;
    let expires = sqlx::query_scalar("update post_event_delivery_jobs set state='LEASED',attempt_count=1,max_attempts=1,lease_token=$2,attempt_id=$3,lease_expires_at=clock_timestamp()+interval '30 seconds' where event_id=$1 and target='TIMELINE' returning lease_expires_at")
        .bind(e.event_id).bind(token).bind(attempt).fetch_one(&f.pool).await?;
    sqlx::query("insert into post_event_delivery_attempts(attempt_id,event_id,target,source,phase,occurred_at) values($1,$2,'TIMELINE','OUTBOX_WORKER','STARTED',clock_timestamp())")
        .bind(attempt).bind(e.event_id).execute(&f.pool).await?;
    let claim = TargetDeliveryClaimDto {
        event: StoredEvent {
            event_id: e.event_id,
            event_type: e.event_type.clone(),
            aggregate_id: post,
            aggregate_version: e.aggregate_version,
            actor_id: e.actor_id,
            attributes: e.attributes.clone(),
            payload: e.payload.clone(),
            occurred_at: e.occurred_at,
            created_at: e.created_at,
            published: false,
            published_at: None,
            replay_resolution_id: None,
            origin_traceparent: None,
        },
        target: DeliveryTarget::Timeline,
        lease_token: token,
        attempt_id: attempt,
        attempt_number: 1,
        lease_expires_at: expires,
    };
    let mut q = request(&e)?;
    q.authority = Some(claim.clone());
    PgTimelineProjectionRepository::new(f.pool.clone())
        .apply(&q)
        .await?;
    let job:serde_json::Value = sqlx::query_scalar("select to_jsonb(j) from post_event_delivery_jobs j where event_id=$1 and target='TIMELINE'").bind(e.event_id).fetch_one(&f.pool).await?;
    let receipt: serde_json::Value = sqlx::query_scalar(
        "select to_jsonb(r) from timeline_projection_receipts r where event_id=$1",
    )
    .bind(e.event_id)
    .fetch_one(&f.pool)
    .await?;
    let history: serde_json::Value = sqlx::query_scalar(
        "select to_jsonb(a) from post_event_delivery_attempts a where attempt_id=$1",
    )
    .bind(attempt)
    .fetch_one(&f.pool)
    .await?;
    f.upgrade().await?;
    assert_eq!(sqlx::query_scalar::<_,serde_json::Value>("select to_jsonb(j)-'progress_credits'-'progress_checked_attempt_id' from post_event_delivery_jobs j where event_id=$1 and target='TIMELINE'").bind(e.event_id).fetch_one(&f.pool).await?,job);
    assert_eq!(
        sqlx::query_scalar::<_, serde_json::Value>(
            "select to_jsonb(r) from timeline_projection_receipts r where event_id=$1"
        )
        .bind(e.event_id)
        .fetch_one(&f.pool)
        .await?,
        receipt
    );
    assert_eq!(
        sqlx::query_scalar::<_, serde_json::Value>(
            "select to_jsonb(a) from post_event_delivery_attempts a where attempt_id=$1"
        )
        .bind(attempt)
        .fetch_one(&f.pool)
        .await?,
        history
    );
    assert!(sqlx::query_scalar::<_,bool>("select progress_credits=0 and progress_checked_attempt_id is null from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'").bind(e.event_id).fetch_one(&f.pool).await?);
    // A live upgraded attempt is untouched; only subsequent real DB expiry
    // and its same-attempt receipt permit another lifetime identity.
    assert!(f
        .schedule()
        .claim_target(DeliveryTarget::Timeline, Utc::now(), 1, &Default::default())
        .await?
        .claims
        .is_empty());
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='TIMELINE'").bind(e.event_id).execute(&f.pool).await?;
    let next = f
        .schedule()
        .claim_target(DeliveryTarget::Timeline, Utc::now(), 1, &Default::default())
        .await?
        .claims
        .pop()
        .unwrap();
    assert_eq!(next.attempt_number, 2);
    assert_ne!(next.attempt_id, attempt);
    assert_ne!(next.lease_token, token);
    assert_eq!(sqlx::query_as::<_,(i32,i32)>("select progress_credits,max_attempts from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'").bind(e.event_id).fetch_one(&f.pool).await?,(1,1));
    f.shutdown().await
}

#[tokio::test]
async fn independent_progress_window_rotates_past_locked_parent_at_limit_one() -> Result<()> {
    use post_service::application::ports::DeliverySchedulePort;
    use post_service::domain::post::DeliveryTarget;
    let f = Fixture::new(22).await?;
    let mut events = Vec::new();
    let mut claims = Vec::new();
    for _ in 0..2 {
        let (post, author) = seed(&f).await?;
        let e = event(&f, post, author, author, "POST_CREATED").await?;
        let claim = inject_late_claim(&f, &e).await?;
        let mut q = request(&e)?;
        q.authority = Some(claim.clone());
        PgTimelineProjectionRepository::new(f.pool.clone())
            .apply(&q)
            .await?;
        events.push(e);
        claims.push(claim);
    }
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-case when event_id=$1 then interval '2 days' else interval '1 day' end where target='TIMELINE'")
        .bind(events[0].event_id).execute(&f.pool).await?;
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select event_id from post_event_outbox where event_id=$1 for update")
        .bind(events[0].event_id)
        .execute(&mut *barrier)
        .await?;
    let schedule = f.schedule();
    // Exclude every source from target-head preparation. Only the independent
    // progress window can credit the later attempt in this probe.
    let older_than = Utc::now() - Duration::days(365);
    assert!(schedule
        .claim_target(DeliveryTarget::External, older_than, 1, &Default::default())
        .await?
        .claims
        .is_empty());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select sum(progress_credits)::bigint from post_event_delivery_jobs"
        )
        .fetch_one(&f.pool)
        .await?,
        0
    );
    let clone = schedule.clone();
    assert!(clone
        .claim_target(DeliveryTarget::External, older_than, 1, &Default::default())
        .await?
        .claims
        .is_empty());
    assert_eq!(sqlx::query_scalar::<_,i32>("select progress_credits from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'").bind(events[1].event_id).fetch_one(&f.pool).await?,1);
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where attempt_id=$1 and phase='PROGRESSED'").bind(claims[1].attempt_id).fetch_one(&f.pool).await?,1);
    assert!(sqlx::query_scalar::<_,bool>("select progress_credits=0 and progress_checked_attempt_id is null and state='LEASED' from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'").bind(events[0].event_id).fetch_one(&f.pool).await?);
    // Wrapping does not duplicate the credited receipt. A fresh scheduler
    // starts a safe new sweep; no durable attempt/effect state was skipped.
    schedule
        .claim_target(DeliveryTarget::External, older_than, 1, &Default::default())
        .await?;
    let restarted = f.schedule();
    restarted
        .claim_target(DeliveryTarget::External, older_than, 1, &Default::default())
        .await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select sum(progress_credits)::bigint from post_event_delivery_jobs"
        )
        .fetch_one(&f.pool)
        .await?,
        1
    );
    barrier.commit().await?;
    restarted
        .claim_target(DeliveryTarget::External, older_than, 1, &Default::default())
        .await?;
    assert_eq!(sqlx::query_scalar::<_,i32>("select progress_credits from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'").bind(events[0].event_id).fetch_one(&f.pool).await?,1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase='PROGRESSED'"
        )
        .fetch_one(&f.pool)
        .await?,
        2
    );
    f.shutdown().await
}

#[tokio::test]
async fn locked_parent_defers_positive_progress_credit_until_bounded_maintenance_can_lock_it(
) -> Result<()> {
    use post_service::application::ports::DeliverySchedulePort;
    use post_service::domain::post::DeliveryTarget;
    let f = Fixture::new(22).await?;
    let (post, author) = seed(&f).await?;
    event(&f, post, author, author, "POST_CREATED").await?;
    let target = event(&f, post, author, Uuid::new_v4(), "POST_LIKED").await?;
    let claim = inject_late_claim(&f, &target).await?;
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    let TimelineProjectionPlanDto::Events(works) = repo.prepare(&target, 100).await? else {
        anyhow::bail!("missing actual owner prefix")
    };
    let work = works
        .into_iter()
        .next()
        .ok_or_else(|| anyhow::anyhow!("missing actual next source"))?;
    repo.apply(&TimelineProjectionApplyDto {
        authority: Some(claim.clone()),
        work,
        follower_count: Some(2),
        high_fanout_threshold: 1000,
    })
    .await?;
    assert_eq!(cursor(&f, post).await?, 1);
    sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()-interval '1 second' where event_id=$1 and target='TIMELINE'")
        .bind(target.event_id).execute(&f.pool).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i32>("select post_timeline_pending_progress_credit($1,$2)")
            .bind(target.event_id)
            .bind(claim.attempt_id)
            .fetch_one(&f.pool)
            .await?,
        1
    );
    let mut held = f.pool.begin().await?;
    sqlx::query("select event_id from post_event_outbox where event_id=$1 for update")
        .bind(target.event_id)
        .execute(&mut *held)
        .await?;
    let maintenance = f.schedule();
    for _ in 0..3 {
        maintenance
            .claim_target(DeliveryTarget::External, Utc::now(), 1, &Default::default())
            .await?;
        assert_eq!(sqlx::query_scalar::<_,i32>("select progress_credits from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'")
            .bind(target.event_id).fetch_one(&f.pool).await?,0);
        assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where attempt_id=$1 and phase='PROGRESSED'")
            .bind(claim.attempt_id).fetch_one(&f.pool).await?,0);
    }
    held.rollback().await?;
    tokio::time::timeout(std::time::Duration::from_secs(5),async {
        for _ in 0..100 {
            maintenance.claim_target(DeliveryTarget::External,Utc::now(),1,&Default::default()).await?;
            let credited:i32=sqlx::query_scalar("select progress_credits from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'")
                .bind(target.event_id).fetch_one(&f.pool).await?;
            if credited==1 {return Ok::<_,anyhow::Error>(());}
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
        anyhow::bail!("released parent not credited within finite maintenance bound")
    }).await??;
    assert_eq!(sqlx::query_scalar::<_,(i32,i32,i32)>("select row(attempt_count,progress_credits,max_attempts) from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'")
        .bind(target.event_id).fetch_one(&f.pool).await?,(1,1,1));
    assert_eq!(sqlx::query_scalar::<_,i64>("select count(*) from post_event_delivery_attempts where attempt_id=$1 and phase='PROGRESSED'")
        .bind(claim.attempt_id).fetch_one(&f.pool).await?,1);
    assert_eq!(cursor(&f, post).await?, 1);
    f.shutdown().await
}
