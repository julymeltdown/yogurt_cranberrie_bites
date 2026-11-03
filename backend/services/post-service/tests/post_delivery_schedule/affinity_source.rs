//! Real owner-transaction source capture; consumer ordering is still a separate
//! required phase. These tests do not certify feature adoption or target gating.
use super::{recommendation_admission::seed, Fixture};
use anyhow::Result;
use chrono::{Duration, Utc};
use post_service::adapters::out::persistence::event_outbox::PgEventOutbox;
use post_service::adapters::out::persistence::user_lifecycle_repository::PgUserLifecycleRepository;
use post_service::application::ports::{
    EventOutboxPort, FeedRecommendationRepositoryPort, UserLifecycleRepositoryPort,
};
use post_service::domain::post::StoredEvent;
use post_service::domain::recommendation::{RecommendationAction, RecommendationEvent};
use sqlx::Row;
use uuid::Uuid;

#[tokio::test]
async fn concurrent_outbox_and_recommendation_sources_allocate_one_contiguous_pair_prefix(
) -> Result<()> {
    let f = Fixture::new(23).await?;
    let (repo, event) = seed(&f).await?;
    let mut other = event.clone();
    other.subject_id = Uuid::new_v4();
    sqlx::query("insert into posts(id,author_id,content,visibility,created_at) select $1,author_id,'concurrent pair post',visibility,now() from posts where id=$2")
        .bind(other.subject_id).bind(event.subject_id).execute(&f.pool).await?;
    let (a, b, c) = tokio::time::timeout(std::time::Duration::from_secs(5), async {
        tokio::join!(
            outbox(&f, &event, "POST_LIKED", false),
            repo.append_event(&event),
            outbox(&f, &other, "POST_COMMENTED", false)
        )
    })
    .await?;
    a?;
    assert!(b?);
    c?;
    let sequences: Vec<i64> = sqlx::query_scalar(
        "select source_sequence from feed_affinity_source_contributions order by source_sequence",
    )
    .fetch_all(&f.pool)
    .await?;
    assert_eq!(sequences, vec![1, 2, 3]);
    assert_eq!(count(&f, "feed_affinity_source_pairs").await?, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select allocated_sequence from feed_affinity_source_pairs")
            .fetch_one(&f.pool)
            .await?,
        3
    );
    f.shutdown().await
}

#[tokio::test]
async fn deleting_distinct_recommendation_actor_erases_source_without_rebasing_other_pair(
) -> Result<()> {
    let f = Fixture::new(23).await?;
    let (repo, mut event) = seed(&f).await?;
    let actor = Uuid::new_v4();
    event.actor_id = Some(actor);
    assert!(repo.append_event(&event).await?);
    PgUserLifecycleRepository::new(f.pool.clone())
        .apply_user_deletion(
            Uuid::new_v4(),
            actor,
            Uuid::new_v4(),
            Utc::now() + Duration::days(7),
        )
        .await?;
    assert_eq!(count(&f, "feed_affinity_source_contributions").await?, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select allocated_sequence from feed_affinity_source_pairs")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    assert!(repo.append_event(&event).await.is_err());
    assert_eq!(count(&f, "feed_affinity_source_contributions").await?, 0);
    f.shutdown().await
}

#[tokio::test]
async fn persisted_capture_hash_is_stable_across_timezone_and_nested_numeric_json() -> Result<()> {
    let f = Fixture::new(23).await?;
    let (repo, mut event) = seed(&f).await?;
    event.metadata_json = r#"{"nested":{"n":1,"a":[1,0.5,{},[]]}}"#.into();
    assert!(repo.append_event(&event).await?);
    let mut tx = f.pool.begin().await?;
    sqlx::query("set local timezone='Asia/Seoul'")
        .execute(&mut *tx)
        .await?;
    let second = Uuid::new_v4();
    // Real INSERT trigger under different session settings; capture identity
    // evidence rather than public acceptance or feature completion.
    sqlx::query("insert into feed_recommendation_events select $2,schema_version,occurred_at,viewer_id,subject_type,subject_id,actor_id,surface,position,request_id,session_id,action,source,model_version,impression_id,propensity_micros,dwell_ms,$3::jsonb from feed_recommendation_events where event_id=$1")
        .bind(event.event_id).bind(second).bind(r#"{"nested":{"a":[1.0,0.500,{},[]],"n":1.000}}"#).execute(&mut *tx).await?;
    tx.commit().await?;
    let hashes: Vec<String> = sqlx::query_scalar(
        "select identity_sha256 from feed_affinity_source_contributions order by source_sequence",
    )
    .fetch_all(&f.pool)
    .await?;
    assert_eq!(hashes.len(), 2);
    assert_eq!(hashes[0], hashes[1]);
    f.shutdown().await
}

pub(super) async fn outbox(
    f: &Fixture,
    event: &RecommendationEvent,
    kind: &str,
    stored_unlike: bool,
) -> Result<StoredEvent> {
    let author: Uuid = sqlx::query_scalar("select author_id from posts where id=$1")
        .bind(event.subject_id)
        .fetch_one(&f.pool)
        .await?;
    let actor = event.viewer_id;
    let body = match kind {
        "POST_COMMENTED" => {
            serde_json::json!({"postId":event.subject_id,"authorId":actor,"notifyUserId":author})
        }
        "POST_COMMENT_DELETED" => {
            serde_json::json!({"postId":event.subject_id,"authorId":actor,"postAuthorId":author,"actorId":actor,"commentId":Uuid::new_v4()})
        }
        "POST_UNLIKED" if stored_unlike => {
            serde_json::json!({"postId":event.subject_id,"authorId":actor})
        }
        _ => serde_json::json!({"postId":event.subject_id,"authorId":author,"actorId":actor}),
    };
    PgEventOutbox::new(f.pool.clone())
        .save(&StoredEvent {
            event_id: Uuid::new_v4(),
            event_type: kind.into(),
            aggregate_id: event.subject_id,
            aggregate_version: None,
            actor_id: Some(actor),
            attributes: vec![],
            payload: body.to_string(),
            occurred_at: Utc::now() - Duration::days(1),
            created_at: Utc::now(),
            published: false,
            published_at: None,
            replay_resolution_id: None,
            origin_traceparent: None,
        })
        .await
}
async fn count(f: &Fixture, table: &str) -> Result<i64> {
    Ok(sqlx::query_scalar(&format!("select count(*) from {table}"))
        .fetch_one(&f.pool)
        .await?)
}

#[tokio::test]
async fn shared_pair_source_order_spans_posts_and_namespaces_without_applying_outbox() -> Result<()>
{
    let f = Fixture::new(23).await?;
    let (repo, event) = seed(&f).await?;
    let first = outbox(&f, &event, "POST_LIKED", false).await?;
    assert!(repo.append_event(&event).await?);
    let mut other = event.clone();
    other.subject_id = Uuid::new_v4();
    sqlx::query("insert into posts(id,author_id,content,visibility,created_at) select $1,author_id,'second pair post',visibility,now() from posts where id=$2")
        .bind(other.subject_id).bind(event.subject_id).execute(&f.pool).await?;
    let last = outbox(&f, &other, "POST_COMMENTED", false).await?;
    let rows=sqlx::query("select namespace,source_id,source_sequence,positive_delta,occurred_at from feed_affinity_source_contributions order by source_sequence")
        .fetch_all(&f.pool).await?;
    assert_eq!(rows.len(), 3);
    for (row, (id, ns, seq, delta)) in rows.iter().zip([
        (first.event_id, "OUTBOX", 1i64, 0.08f64),
        (event.event_id, "RECOMMENDATION", 2, 0.08),
        (last.event_id, "OUTBOX", 3, 0.16),
    ]) {
        assert_eq!(row.get::<Uuid, _>("source_id"), id);
        assert_eq!(row.get::<String, _>("namespace"), ns);
        assert_eq!(row.get::<i64, _>("source_sequence"), seq);
        assert!((row.get::<f64, _>("positive_delta") - delta).abs() < 1e-12);
    }
    // Later source commits deliberately carry older source times. Allocation
    // establishes transaction order rather than sorting request timestamps.
    assert!(
        rows[2].get::<chrono::DateTime<Utc>, _>("occurred_at")
            < rows[1].get::<chrono::DateTime<Utc>, _>("occurred_at")
    );
    assert_eq!(count(&f, "feed_affinity_source_pairs").await?, 1);
    assert_eq!(count(&f, "timeline_projection_receipts").await?, 0);
    let score: f64 = sqlx::query_scalar("select affinity_score from feed_author_affinities")
        .fetch_one(&f.pool)
        .await?;
    assert!(
        (score - 0.08).abs() < 1e-12,
        "capture must not consume outbox authority"
    );
    f.shutdown().await
}

#[tokio::test]
async fn source_capture_duplicates_skip_allocation_and_conflicting_identity_is_immutable(
) -> Result<()> {
    let f = Fixture::new(23).await?;
    let (repo, event) = seed(&f).await?;
    assert!(repo.append_event(&event).await?);
    assert!(!repo.append_event(&event).await?);
    let mut conflict = event.clone();
    conflict.action = RecommendationAction::Like;
    assert!(repo.append_event(&conflict).await.is_err());
    assert!(sqlx::query(
        "update feed_affinity_source_contributions set positive_delta=1 where source_id=$1"
    )
    .bind(event.event_id)
    .execute(&f.pool)
    .await
    .is_err());
    assert_eq!(count(&f, "feed_affinity_source_contributions").await?, 1);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select allocated_sequence from feed_affinity_source_pairs")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    f.shutdown().await
}

#[tokio::test]
async fn feature_failure_rolls_back_source_counter_and_retry_allocates_no_hole() -> Result<()> {
    let f = Fixture::new(23).await?;
    let (repo, event) = seed(&f).await?;
    sqlx::raw_sql("create function fail_captured_feature() returns trigger language plpgsql as $$ begin raise exception 'controlled feature failure'; end; $$; create trigger fail_captured_feature before insert on feed_author_affinities for each row execute function fail_captured_feature();")
        .execute(&f.pool).await?;
    assert!(repo.append_event(&event).await.is_err());
    for table in [
        "feed_affinity_source_contributions",
        "feed_affinity_source_pairs",
        "feed_recent_actions",
    ] {
        assert_eq!(count(&f, table).await?, 0);
    }
    sqlx::query("drop trigger fail_captured_feature on feed_author_affinities")
        .execute(&f.pool)
        .await?;
    assert!(repo.append_event(&event).await?);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select source_sequence from feed_affinity_source_contributions"
        )
        .fetch_one(&f.pool)
        .await?,
        1
    );
    f.shutdown().await
}

#[tokio::test]
async fn outbox_source_mappings_preserve_actual_unlike_and_self_exclusion() -> Result<()> {
    let f = Fixture::new(23).await?;
    let (_, event) = seed(&f).await?;
    for (kind, stored, positive, negative) in [
        ("POST_LIKED", false, 0.08, 0.0),
        ("POST_UNLIKED", false, 0.0, 0.08),
        ("POST_UNLIKED", true, 0.0, 0.08),
        ("POST_COMMENTED", false, 0.16, 0.0),
        ("POST_COMMENT_DELETED", false, 0.0, 0.16),
    ] {
        let source = outbox(&f, &event, kind, stored).await?;
        let r=sqlx::query("select positive_delta,negative_delta from feed_affinity_source_contributions where source_id=$1")
            .bind(source.event_id).fetch_one(&f.pool).await?;
        assert!((r.get::<f64, _>("positive_delta") - positive).abs() < 1e-12);
        assert!((r.get::<f64, _>("negative_delta") - negative).abs() < 1e-12);
    }
    let mut own = event.clone();
    own.viewer_id = sqlx::query_scalar("select author_id from posts where id=$1")
        .bind(event.subject_id)
        .fetch_one(&f.pool)
        .await?;
    outbox(&f, &own, "POST_LIKED", false).await?;
    assert_eq!(count(&f, "feed_affinity_source_contributions").await?, 5);
    assert_eq!(count(&f, "feed_author_affinities").await?, 0);
    f.shutdown().await
}

#[tokio::test]
async fn recommendation_source_weights_preserve_all_actions_and_impressions_do_not_allocate(
) -> Result<()> {
    let f = Fixture::new(23).await?;
    let (repo, event) = seed(&f).await?;
    assert_eq!(count(&f, "feed_affinity_source_pairs").await?, 0);
    for (action, positive, negative) in [
        (RecommendationAction::Like, 0.20, 0.0),
        (RecommendationAction::Comment, 0.30, 0.0),
        (RecommendationAction::Save, 0.35, 0.0),
        (RecommendationAction::Share, 0.35, 0.0),
        (RecommendationAction::Follow, 0.50, 0.0),
        (RecommendationAction::AcceptFollow, 0.50, 0.0),
        (RecommendationAction::Click, 0.08, 0.0),
        (RecommendationAction::Dwell, 0.20, 0.0),
        (RecommendationAction::FastScroll, 0.0, 0.15),
        (RecommendationAction::Hide, 0.0, 0.40),
        (RecommendationAction::NotInterested, 0.0, 0.40),
        (RecommendationAction::Mute, 0.0, 0.70),
        (RecommendationAction::Block, 0.0, 1.0),
        (RecommendationAction::Report, 0.0, 1.0),
    ] {
        let mut e = event.clone();
        e.event_id = Uuid::new_v4();
        e.action = action;
        e.dwell_ms = Some(120_000);
        assert!(repo.append_event(&e).await?);
        let r=sqlx::query("select positive_delta,negative_delta from feed_affinity_source_contributions where source_id=$1")
            .bind(e.event_id).fetch_one(&f.pool).await?;
        assert!((r.get::<f64, _>("positive_delta") - positive).abs() < 1e-12);
        assert!((r.get::<f64, _>("negative_delta") - negative).abs() < 1e-12);
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select allocated_sequence from feed_affinity_source_pairs")
            .fetch_one(&f.pool)
            .await?,
        14
    );
    f.shutdown().await
}

#[tokio::test]
async fn actual_account_deletion_erases_captured_state_and_fences_delayed_sources() -> Result<()> {
    let f = Fixture::new(23).await?;
    let (repo, event) = seed(&f).await?;
    outbox(&f, &event, "POST_LIKED", false).await?;
    assert!(repo.append_event(&event).await?);
    PgUserLifecycleRepository::new(f.pool.clone())
        .apply_user_deletion(
            Uuid::new_v4(),
            event.viewer_id,
            Uuid::new_v4(),
            Utc::now() + Duration::days(7),
        )
        .await?;
    assert_eq!(count(&f, "feed_affinity_source_pairs").await?, 0);
    assert_eq!(count(&f, "feed_affinity_source_contributions").await?, 0);
    assert!(outbox(&f, &event, "POST_COMMENTED", false).await.is_err());
    assert!(repo.append_event(&event).await.is_err());
    assert_eq!(count(&f, "feed_affinity_source_pairs").await?, 0);
    f.shutdown().await
}

#[tokio::test]
async fn actual_schema_upgrade_captures_new_sources_without_adopting_or_resetting_scalar(
) -> Result<()> {
    let f = Fixture::new(22).await?;
    let (repo, event) = seed(&f).await?;
    assert!(repo.append_event(&event).await?);
    f.upgrade().await?;
    assert_eq!(count(&f, "feed_affinity_source_pairs").await?, 0);
    let score: f64 = sqlx::query_scalar("select affinity_score from feed_author_affinities")
        .fetch_one(&f.pool)
        .await?;
    assert!((score - 0.08).abs() < 1e-12);
    assert!(!repo.append_event(&event).await?);
    assert_eq!(count(&f, "feed_affinity_source_contributions").await?, 0);
    let mut next = event.clone();
    next.event_id = Uuid::new_v4();
    next.action = RecommendationAction::Like;
    assert!(repo.append_event(&next).await?);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select source_sequence from feed_affinity_source_contributions"
        )
        .fetch_one(&f.pool)
        .await?,
        1
    );
    let score: f64 = sqlx::query_scalar("select affinity_score from feed_author_affinities")
        .fetch_one(&f.pool)
        .await?;
    assert!(
        (score - 0.276).abs() < 1e-12,
        "source capture must not reset legacy scalar"
    );
    f.shutdown().await
}
