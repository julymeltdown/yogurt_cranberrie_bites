//! Actual PG28 scheduling hints and projection authority; no live providers.
use super::{affinity_baseline::controlled_version, projection, Fixture};
use anyhow::{ensure, Result};
use chrono::Utc;
use post_service::adapters::out::persistence::{
    delivery_schedule::PgDeliverySchedule,
    timeline_projection_repository::PgTimelineProjectionRepository,
};
use post_service::application::dto::{delivery_schedule_dto::*, timeline_projection_dto::*};
use post_service::application::ports::{DeliverySchedulePort, TimelineProjectionRepositoryPort};
use post_service::domain::post::DeliveryTarget;
use sqlx::Row;
use uuid::Uuid;

async fn ready(f: &Fixture, id: Uuid) -> Result<bool> {
    Ok(
        sqlx::query_scalar("select post_timeline_affinity_ready($1)")
            .bind(id)
            .fetch_one(&f.pool)
            .await?,
    )
}
async fn claim(schedule: &PgDeliverySchedule, expected: Uuid) -> Result<TargetDeliveryClaimDto> {
    let config = DeliveryScheduleConfigDto {
        max_attempts: 1,
        ..Default::default()
    };
    for _ in 0..30 {
        let batch = schedule
            .claim_target(DeliveryTarget::Timeline, Utc::now(), 1, &config)
            .await?;
        if let Some(claim) = batch.claims.into_iter().next() {
            ensure!(
                claim.event.event_id == expected,
                "unexpected aggregate claimed"
            );
            return Ok(claim);
        }
    }
    anyhow::bail!("bounded fixture sweep never selected ready head")
}
async fn apply_claim(f: &Fixture, c: &TargetDeliveryClaimDto) -> Result<()> {
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    if let TimelineProjectionPlanDto::Events(works) = repo
        .prepare(&TimelineProjectionEventDto::from(&c.event), 100)
        .await?
    {
        for work in works {
            repo.apply(&TimelineProjectionApplyDto {
                authority: Some(c.clone()),
                work,
                follower_count: Some(2),
                high_fanout_threshold: 1000,
            })
            .await?;
        }
    }
    assert_eq!(
        f.schedule().complete_target(c).await?,
        DeliveryLeaseOutcomeDto::Recorded {
            event_published: false
        }
    );
    Ok(())
}
async fn pair_posts(f: &Fixture) -> Result<(Uuid, Uuid, Uuid)> {
    let (a, author) = projection::seed(f).await?;
    let ca = projection::event(f, a, author, author, "POST_CREATED").await?;
    apply_claim(f, &claim(&f.schedule(), ca.event_id).await?).await?;
    let (b, _) = projection::seed(f).await?;
    sqlx::query("update posts set author_id=$2 where id=$1")
        .bind(b)
        .bind(author)
        .execute(&f.pool)
        .await?;
    let cb = projection::event(f, b, author, author, "POST_CREATED").await?;
    apply_claim(f, &claim(&f.schedule(), cb.event_id).await?).await?;
    Ok((a, b, author))
}

#[tokio::test]
async fn waiting_pair_spends_no_attempts_healthy_post_runs_and_predecessor_wakes_own_claim(
) -> Result<()> {
    let f = controlled_version(28).await?;
    let (a, b, author) = pair_posts(&f).await?;
    let actor = Uuid::new_v4();
    let first = projection::event(&f, a, author, actor, "POST_LIKED").await?;
    sqlx::query("update post_event_delivery_jobs set next_attempt_at=now()+interval '1 day' where event_id=$1 and target='TIMELINE'").bind(first.event_id).execute(&f.pool).await?;
    let waiting = projection::event(&f, b, author, actor, "POST_LIKED").await?;
    assert!(!ready(&f, waiting.event_id).await?);
    let (healthy, ha) = projection::seed(&f).await?;
    let hc = projection::event(&f, healthy, ha, ha, "POST_CREATED").await?;
    let schedule = f.schedule();
    let healthy_claim = claim(&schedule, hc.event_id).await?;
    apply_claim(&f, &healthy_claim).await?;
    for _ in 0..15 {
        assert!(schedule
            .claim_target(DeliveryTarget::Timeline, Utc::now(), 1, &Default::default())
            .await?
            .claims
            .is_empty());
    }
    let row=sqlx::query("select state,attempt_count,progress_credits from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'").bind(waiting.event_id).fetch_one(&f.pool).await?;
    assert_eq!(row.get::<String, _>("state"), "PENDING");
    assert_eq!(row.get::<i32, _>("attempt_count"), 0);
    assert_eq!(row.get::<i32, _>("progress_credits"), 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where event_id=$1"
        )
        .bind(waiting.event_id)
        .fetch_one(&f.pool)
        .await?,
        0
    );
    assert_eq!(projection::cursor(&f, b).await?, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select resolved_sequence from feed_affinity_pair_heads")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.due(first.event_id, DeliveryTarget::Timeline).await?;
    let ac = claim(&schedule, first.event_id).await?;
    apply_claim(&f, &ac).await?;
    assert_eq!(
        projection::cursor(&f, b).await?,
        1,
        "another Post cannot be consumed under first claim"
    );
    assert!(ready(&f, waiting.event_id).await?);
    let bc = claim(&schedule, waiting.event_id).await?;
    assert_eq!(bc.attempt_number, 1);
    apply_claim(&f, &bc).await?;
    assert_eq!(projection::cursor(&f, b).await?, 2);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select resolved_sequence from feed_affinity_pair_heads")
            .fetch_one(&f.pool)
            .await?,
        2
    );
    f.shutdown().await
}

#[tokio::test]
async fn next_step_readiness_includes_published_predecessors_and_allows_safe_creation_prefix(
) -> Result<()> {
    let f = controlled_version(28).await?;
    let (a, author) = projection::seed(&f).await?;
    let ac = projection::event(&f, a, author, author, "POST_CREATED").await?;
    apply_claim(&f, &claim(&f.schedule(), ac.event_id).await?).await?;
    let actor = Uuid::new_v4();
    let predecessor = projection::event(&f, a, author, actor, "POST_LIKED").await?;
    sqlx::query("update post_event_delivery_jobs set next_attempt_at=now()+interval '1 day' where event_id=$1 and target='TIMELINE'")
        .bind(predecessor.event_id).execute(&f.pool).await?;
    let (b, _) = projection::seed(&f).await?;
    sqlx::query("update posts set author_id=$2 where id=$1")
        .bind(b)
        .bind(author)
        .execute(&f.pool)
        .await?;
    let create = projection::event(&f, b, author, author, "POST_CREATED").await?;
    let like = projection::event(&f, b, author, actor, "POST_LIKED").await?;
    // The waiting contribution is not the next step. A later same-Post claim
    // could project creation, but this hint allocates nothing and owns nothing.
    assert!(ready(&f, like.event_id).await?);
    assert_eq!(projection::cursor(&f, b).await?, 0);
    apply_claim(&f, &claim(&f.schedule(), create.event_id).await?).await?;
    assert!(!ready(&f, like.event_id).await?);
    // Published=true is not projection proof: lookup must still see the actual
    // pending predecessor and its blocked pair for a later same-Post request.
    sqlx::query("update post_event_outbox set published=true,published_at=now() where event_id=$1")
        .bind(like.event_id)
        .execute(&f.pool)
        .await?;
    let later = projection::event(&f, b, author, Uuid::new_v4(), "POST_COMMENTED").await?;
    assert!(!ready(&f, later.event_id).await?);
    assert_eq!(projection::cursor(&f, b).await?, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_affinity_resolution_receipts")
            .fetch_one(&f.pool)
            .await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn locked_oldest_parent_does_not_starve_healthy_head_and_wrap_survives_new_arrivals(
) -> Result<()> {
    let f = controlled_version(28).await?;
    let (a, aa) = projection::seed(&f).await?;
    let first = projection::event(&f, a, aa, aa, "POST_CREATED").await?;
    let (b, ba) = projection::seed(&f).await?;
    let second = projection::event(&f, b, ba, ba, "POST_CREATED").await?;
    let mut hold = f.pool.begin().await?;
    sqlx::query("select event_id from post_event_outbox where event_id=$1 for update")
        .bind(first.event_id)
        .execute(&mut *hold)
        .await?;
    let schedule = f.schedule();
    let c = claim(&schedule, second.event_id).await?;
    apply_claim(&f, &c).await?;
    hold.rollback().await?;
    // New rows do not extend this sweep's captured ceiling. The first released
    // row must be revisited before an unbounded stream of newer work.
    let (n, na) = projection::seed(&f).await?;
    projection::event(&f, n, na, na, "POST_CREATED").await?;
    let c = claim(&schedule, first.event_id).await?;
    apply_claim(&f, &c).await?;
    f.shutdown().await
}

#[tokio::test]
async fn candidate_window_is_bounded_before_predicates_and_seeks_the_unpublished_index(
) -> Result<()> {
    use post_service::adapters::out::persistence::timeline_dependency_sql::{
        AFTER_CANDIDATES_SQL, CANDIDATES_SQL,
    };
    let f = Fixture::new(28).await?;
    for _ in 0..130 {
        f.save(Uuid::new_v4(), Some(1), 1).await?;
    }
    let now = Utc::now();
    let ceiling=sqlx::query("select created_at,event_id from post_event_outbox order by created_at desc,event_id desc limit 1").fetch_one(&f.pool).await?;
    let expiry = ceiling.get::<chrono::DateTime<Utc>, _>("created_at");
    let ceiling_id = ceiling.get::<Uuid, _>("event_id");
    let mut tx = f.pool.begin().await?;
    let rows = sqlx::query(CANDIDATES_SQL)
        .bind("TIMELINE")
        .bind(now)
        .bind(100i64)
        .bind(expiry)
        .bind(ceiling_id)
        .fetch_all(&mut *tx)
        .await?;
    assert_eq!(rows.len(), 100);
    let last = rows.last().unwrap();
    let after = last.get::<chrono::DateTime<Utc>, _>("candidate_created_at");
    let after_id = last.get::<Uuid, _>("candidate_event_id");
    let rest = sqlx::query(AFTER_CANDIDATES_SQL)
        .bind("TIMELINE")
        .bind(now)
        .bind(100i64)
        .bind(expiry)
        .bind(ceiling_id)
        .bind(after)
        .bind(after_id)
        .fetch_all(&mut *tx)
        .await?;
    assert_eq!(rest.len(), 30);
    assert!(rest.iter().all(|r| (
        r.get::<chrono::DateTime<Utc>, _>("candidate_created_at"),
        r.get::<Uuid, _>("candidate_event_id")
    ) > (after, after_id)));
    // Force index-capable access only for this small-fixture EXPLAIN. This
    // proves tuple range/index structure, not default production cost or SLO.
    sqlx::query("set local enable_seqscan=off")
        .execute(&mut *tx)
        .await?;
    let plan: serde_json::Value = sqlx::query_scalar(&format!(
        "explain (analyze,format json) {AFTER_CANDIDATES_SQL}"
    ))
    .bind("TIMELINE")
    .bind(now)
    .bind(100i64)
    .bind(expiry)
    .bind(ceiling_id)
    .bind(after)
    .bind(after_id)
    .fetch_one(&mut *tx)
    .await?;
    let text = plan.to_string();
    assert!(text.contains("post_outbox_dependency_window_idx"));
    assert!(text.contains("Index Cond"));
    assert!(
        text.contains("ROW(created_at, event_id) >")
            && text.contains("ROW(created_at, event_id) <=")
    );
    println!("DEPENDENCY_FORCED_INDEX_PLAN {plan}");
    tx.rollback().await?;
    let batch = f
        .schedule()
        .claim_target(DeliveryTarget::Timeline, now, 10_000, &Default::default())
        .await?;
    assert_eq!(
        batch.claims.len(),
        100,
        "public claim limit cannot widen the100-candidate window"
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from post_event_delivery_attempts where phase='STARTED'"
        )
        .fetch_one(&f.pool)
        .await?,
        100
    );
    f.shutdown().await
}
