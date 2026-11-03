//! Actual controlled PG26-manifest reducer transactions. External services and
//! production clone/restore authority are not qualified by these probes.
use super::{
    affinity_baseline::controlled_version, projection, recommendation_admission::seed, Fixture,
};
use anyhow::Result;
use chrono::{Duration, Utc};
use post_service::adapters::out::persistence::user_lifecycle_repository::PgUserLifecycleRepository;
use post_service::application::dto::mutation_dto::MutationError;
use post_service::application::ports::{
    DeliverySchedulePort, FeedRecommendationRepositoryPort, UserLifecycleRepositoryPort,
};
use post_service::domain::{post::DeliveryTarget, recommendation::RecommendationAction};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

async fn count(f: &Fixture, table: &str) -> Result<i64> {
    Ok(sqlx::query_scalar(&format!("select count(*) from {table}"))
        .fetch_one(&f.pool)
        .await?)
}
async fn state(f: &Fixture) -> Result<(i64, f64, f64, Option<chrono::DateTime<Utc>>)> {
    Ok(sqlx::query_as("select resolved_sequence,positive_score,negative_score,last_interacted_at from feed_affinity_pair_heads").fetch_one(&f.pool).await?)
}
async fn raw_recommendation(
    tx: &mut Transaction<'_, Postgres>,
    e: &post_service::domain::recommendation::RecommendationEvent,
) -> Result<()> {
    sqlx::query("insert into feed_recommendation_events(event_id,schema_version,occurred_at,viewer_id,subject_type,subject_id,actor_id,surface,position,request_id,session_id,action,source,model_version,impression_id,propensity_micros,dwell_ms,metadata_json) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18::jsonb)")
        .bind(e.event_id).bind(i32::from(e.schema_version)).bind(e.occurred_at).bind(e.viewer_id)
        .bind(e.subject_type.as_str()).bind(e.subject_id).bind(e.actor_id).bind(e.surface.as_str())
        .bind(e.position).bind(&e.request_id).bind(e.session_id).bind(e.action.as_str()).bind(&e.source)
        .bind(&e.model_version).bind(e.impression_id).bind(e.propensity.map(|p|(p*1_000_000.0).round() as i32))
        .bind(e.dwell_ms.map(|m|m as i32)).bind(&e.metadata_json).execute(&mut **tx).await?;
    Ok(())
}
async fn resolve_recommendation(tx: &mut Transaction<'_, Postgres>, id: Uuid) -> Result<bool> {
    Ok(sqlx::query_scalar(
        "select feed_affinity_resolve_ordered('RECOMMENDATION',$1,null,null,null,null,null)",
    )
    .bind(id)
    .fetch_one(&mut **tx)
    .await?)
}

#[tokio::test]
async fn ordinary_schema26_preserves_capture_only_compatibility() -> Result<()> {
    let f = Fixture::new(26).await?;
    let (repo, e) = seed(&f).await?;
    assert!(repo.append_event(&e).await?);
    assert_eq!(count(&f, "feed_affinity_pair_heads").await?, 0);
    assert_eq!(count(&f, "feed_affinity_resolution_receipts").await?, 0);
    f.shutdown().await
}

#[tokio::test]
async fn recommendations_reduce_exact_order_and_preserve_immutable_genesis() -> Result<()> {
    let f = controlled_version(26).await?;
    let (repo, mut e) = seed(&f).await?;
    assert!(repo.append_event(&e).await?);
    let (seq, p, n, t) = state(&f).await?;
    assert_eq!((seq, p, n), (1, 0.08, 0.0));
    e.event_id = Uuid::new_v4();
    e.action = RecommendationAction::Hide;
    e.occurred_at -= Duration::days(1);
    assert!(repo.append_event(&e).await?);
    let (seq, p, n, next_t) = state(&f).await?;
    assert_eq!(seq, 2);
    assert!((p - 0.076).abs() < 1e-12);
    assert_eq!(n, 0.40);
    assert_eq!(t, next_t);
    assert_eq!(sqlx::query_as::<_,(f64,f64,Option<chrono::DateTime<Utc>>,i64)>("select positive_score,negative_score,last_interacted_at,resolved_sequence from feed_affinity_pair_baselines")
        .fetch_one(&f.pool).await?,(0.0,0.0,None,0));
    assert_eq!(
        count(&f, "feed_affinity_legacy_effect_observations").await?,
        0
    );
    assert!(
        !sqlx::query_scalar::<_, bool>(
            "select scalar_observed or legacy_write_observed from feed_affinity_pair_provenance"
        )
        .fetch_one(&f.pool)
        .await?
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_affinity_write_permits")
            .fetch_one(f.controller_pool.as_ref().unwrap())
            .await?,
        0
    );
    assert!(!repo.append_event(&e).await?);
    assert_eq!(state(&f).await?.0, 2);
    f.shutdown().await
}

#[tokio::test]
async fn outbox_projection_reduces_score_receipt_and_both_cursors_atomically() -> Result<()> {
    let f = controlled_version(26).await?;
    let (post, author) = projection::seed(&f).await?;
    let create = projection::event(&f, post, author, author, "POST_CREATED").await?;
    projection::apply(&f, &create).await?;
    let actor = Uuid::new_v4();
    for kind in ["POST_LIKED", "POST_UNLIKED"] {
        let e = projection::event(&f, post, author, actor, kind).await?;
        projection::apply(&f, &e).await?;
    }
    let (seq, p, n, _) = state(&f).await?;
    assert_eq!(seq, 2);
    assert!((p - 0.076).abs() < 1e-12);
    assert_eq!(n, 0.08);
    assert_eq!(projection::cursor(&f, post).await?, 3);
    assert_eq!(count(&f, "feed_affinity_resolution_receipts").await?, 2);
    assert_eq!(
        count(&f, "feed_affinity_legacy_effect_observations").await?,
        0
    );
    f.shutdown().await
}

#[tokio::test]
async fn recommendation_cannot_consume_unresolved_outbox_predecessor() -> Result<()> {
    let f = controlled_version(26).await?;
    let (repo, e) = seed(&f).await?;
    let author: Uuid = sqlx::query_scalar("select author_id from posts where id=$1")
        .bind(e.subject_id)
        .fetch_one(&f.pool)
        .await?;
    let create = projection::event(&f, e.subject_id, author, author, "POST_CREATED").await?;
    projection::apply(&f, &create).await?;
    let like = projection::event(&f, e.subject_id, author, e.viewer_id, "POST_LIKED").await?;
    let err = repo.append_event(&e).await.unwrap_err();
    assert!(matches!(
        err.downcast_ref::<MutationError>(),
        Some(MutationError::Unavailable)
    ));
    assert_eq!(state(&f).await?.0, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select allocated_sequence from feed_affinity_source_pairs")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    assert_eq!(count(&f, "feed_recent_actions").await?, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from feed_recommendation_events where event_id=$1"
        )
        .bind(e.event_id)
        .fetch_one(&f.pool)
        .await?,
        0
    );
    projection::apply(&f, &like).await?;
    assert!(repo.append_event(&e).await?);
    let (seq, p, _, _) = state(&f).await?;
    assert_eq!(seq, 2);
    assert!((p - 0.156).abs() < 1e-12);
    f.shutdown().await
}

#[tokio::test]
async fn soft_deleted_post_resolves_suppression_without_score_or_time() -> Result<()> {
    let f = controlled_version(26).await?;
    let (post, author) = projection::seed(&f).await?;
    let create = projection::event(&f, post, author, author, "POST_CREATED").await?;
    projection::apply(&f, &create).await?;
    let like = projection::event(&f, post, author, Uuid::new_v4(), "POST_LIKED").await?;
    sqlx::query("update posts set deleted_at=now() where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    projection::apply(&f, &like).await?;
    assert_eq!(state(&f).await?, (1, 0.0, 0.0, None));
    assert_eq!(count(&f, "feed_author_affinities").await?, 0);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select disposition from feed_affinity_resolution_receipts"
        )
        .fetch_one(&f.pool)
        .await?,
        "SUPPRESSED"
    );
    assert_eq!(projection::cursor(&f, post).await?, 2);
    f.shutdown().await
}

#[tokio::test]
async fn raw_recommendation_cannot_commit_without_durable_resolution() -> Result<()> {
    let f = controlled_version(26).await?;
    let (_, e) = seed(&f).await?;
    let mut tx = f.pool.begin().await?;
    raw_recommendation(&mut tx, &e).await?;
    assert!(tx.commit().await.is_err());
    for t in [
        "feed_affinity_pair_baselines",
        "feed_affinity_pair_heads",
        "feed_affinity_source_pairs",
        "feed_author_affinities",
    ] {
        assert_eq!(count(&f, t).await?, 0);
    }
    f.shutdown().await
}

#[tokio::test]
async fn direct_writes_remain_forbidden_after_resolver_return() -> Result<()> {
    let f = controlled_version(26).await?;
    let (repo, e) = seed(&f).await?;
    assert!(repo.append_event(&e).await?);
    for sql in [
        "update feed_author_affinities set affinity_score=0.9",
        "update feed_affinity_pair_heads set resolved_sequence=5",
        "update feed_affinity_resolution_receipts set disposition='SUPPRESSED'",
        "select * from feed_affinity_write_permits",
    ] {
        assert!(sqlx::query(sql).execute(&f.pool).await.is_err(), "{sql}");
    }
    for sql in [
        "update feed_author_affinities set affinity_score=0.9",
        "update feed_affinity_pair_heads set resolved_sequence=5",
        "update feed_affinity_resolution_receipts set disposition='SUPPRESSED'",
    ] {
        assert!(
            sqlx::query(sql)
                .execute(f.controller_pool.as_ref().unwrap())
                .await
                .is_err(),
            "{sql}"
        );
    }
    assert_eq!(state(&f).await?.0, 1);
    f.shutdown().await
}

#[tokio::test]
async fn skipped_scalar_or_head_mutation_rolls_back_entire_acceptance() -> Result<()> {
    for table in ["feed_author_affinities", "feed_affinity_pair_heads"] {
        let f = controlled_version(26).await?;
        let (repo, e) = seed(&f).await?;
        sqlx::raw_sql(&format!("create function skip_resolution() returns trigger language plpgsql as $$ begin return null; end; $$; create trigger zz_skip_resolution before insert or update on {table} for each row execute function skip_resolution();"))
            .execute(f.controller_pool.as_ref().unwrap()).await?;
        assert!(repo.append_event(&e).await.is_err());
        for t in [
            "feed_affinity_source_pairs",
            "feed_affinity_pair_baselines",
            "feed_affinity_pair_heads",
            "feed_author_affinities",
            "feed_recent_actions",
        ] {
            assert_eq!(count(&f, t).await?, 0);
        }
        f.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn rollback_to_savepoint_allows_exact_retry_without_stale_permit_or_hole() -> Result<()> {
    let f = controlled_version(26).await?;
    let (_, e) = seed(&f).await?;
    let mut tx = f.pool.begin().await?;
    sqlx::query("savepoint retry").execute(&mut *tx).await?;
    raw_recommendation(&mut tx, &e).await?;
    assert!(resolve_recommendation(&mut tx, e.event_id).await?);
    sqlx::query("rollback to savepoint retry")
        .execute(&mut *tx)
        .await?;
    raw_recommendation(&mut tx, &e).await?;
    assert!(resolve_recommendation(&mut tx, e.event_id).await?);
    assert!(resolve_recommendation(&mut tx, e.event_id).await?);
    tx.commit().await?;
    assert_eq!(state(&f).await?.0, 1);
    assert_eq!(count(&f, "feed_affinity_resolution_receipts").await?, 1);
    f.shutdown().await
}

#[tokio::test]
async fn terminal_actor_erasure_removes_identity_without_reversing_unrelated_head() -> Result<()> {
    let f = controlled_version(26).await?;
    let (repo, mut e) = seed(&f).await?;
    let actor = Uuid::new_v4();
    e.actor_id = Some(actor);
    assert!(repo.append_event(&e).await?);
    PgUserLifecycleRepository::new(f.pool.clone())
        .apply_user_deletion(
            Uuid::new_v4(),
            actor,
            Uuid::new_v4(),
            Utc::now() + Duration::days(7),
        )
        .await?;
    assert_eq!(state(&f).await?.0, 1);
    assert_eq!(count(&f, "feed_affinity_resolution_receipts").await?, 0);
    e.event_id = Uuid::new_v4();
    e.actor_id = Some(e.viewer_id);
    assert!(repo.append_event(&e).await?);
    let (seq, p, _, _) = state(&f).await?;
    assert_eq!(seq, 2);
    assert!((p - 0.156).abs() < 1e-12);
    f.shutdown().await
}

#[tokio::test]
async fn old_schema25_certificate_is_not_refreshed_into_reducer_eligibility() -> Result<()> {
    let f = controlled_version(25).await?;
    let (post, author) = projection::seed(&f).await?;
    projection::event(&f, post, author, Uuid::new_v4(), "POST_LIKED").await?;
    f.migrations()
        .await?
        .run(f.controller_pool.as_ref().unwrap())
        .await?;
    assert!(
        projection::event(&f, post, author, Uuid::new_v4(), "POST_LIKED")
            .await
            .is_err()
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_affinity_pair_heads")
            .fetch_one(f.controller_pool.as_ref().unwrap())
            .await?,
        0
    );
    assert_eq!(count(&f, "feed_affinity_pair_baselines").await?, 1);
    f.shutdown().await
}

#[tokio::test]
async fn different_posts_claim_cannot_authorize_pending_predecessor() -> Result<()> {
    let f = controlled_version(26).await?;
    let (post, author) = projection::seed(&f).await?;
    let other = Uuid::new_v4();
    sqlx::query("insert into posts(id,author_id,content,visibility,created_at) values($1,$2,'other','PUBLIC',now())").bind(other).bind(author).execute(&f.pool).await?;
    for p in [post, other] {
        let create = projection::event(&f, p, author, author, "POST_CREATED").await?;
        projection::apply(&f, &create).await?;
    }
    let actor = Uuid::new_v4();
    let first = projection::event(&f, post, author, actor, "POST_LIKED").await?;
    projection::event(&f, other, author, actor, "POST_LIKED").await?;
    let claims = f
        .schedule()
        .claim_target(
            DeliveryTarget::Timeline,
            Utc::now(),
            100,
            &Default::default(),
        )
        .await?
        .claims;
    let wrong = claims
        .iter()
        .find(|c| c.event.aggregate_id == other)
        .unwrap();
    let error =
        sqlx::query("select feed_affinity_resolve_ordered('OUTBOX',$1,$2,$3,$4,$5,repeat('0',64))")
            .bind(first.event_id)
            .bind(wrong.event.event_id)
            .bind(wrong.lease_token)
            .bind(wrong.attempt_id)
            .bind(wrong.attempt_number)
            .execute(&f.pool)
            .await
            .unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().constraint(),
        Some("feed_affinity_resolution_authority")
    );
    assert_eq!(state(&f).await?.0, 0);
    assert_eq!(count(&f, "feed_affinity_resolution_receipts").await?, 0);
    f.shutdown().await
}

#[tokio::test]
async fn outbox_resolution_without_own_post_receipt_rolls_back() -> Result<()> {
    let f = controlled_version(26).await?;
    let (post, author) = projection::seed(&f).await?;
    let create = projection::event(&f, post, author, author, "POST_CREATED").await?;
    projection::apply(&f, &create).await?;
    let like = projection::event(&f, post, author, Uuid::new_v4(), "POST_LIKED").await?;
    let claim = projection::authority(&f, post).await?;
    let mut tx = f.pool.begin().await?;
    assert!(
        sqlx::query_scalar::<_, bool>(
            "select feed_affinity_resolve_ordered('OUTBOX',$1,$2,$3,$4,$5,repeat('0',64))"
        )
        .bind(like.event_id)
        .bind(claim.event.event_id)
        .bind(claim.lease_token)
        .bind(claim.attempt_id)
        .bind(claim.attempt_number)
        .fetch_one(&mut *tx)
        .await?
    );
    assert!(tx.commit().await.is_err());
    assert_eq!(state(&f).await?.0, 0);
    assert_eq!(count(&f, "feed_author_affinities").await?, 0);
    assert_eq!(projection::cursor(&f, post).await?, 1);
    f.shutdown().await
}

#[tokio::test]
async fn deferred_expiry_rolls_back_but_early_constraint_discharge_is_an_explicit_sql_trust_boundary(
) -> Result<()> {
    for discharge_early in [false, true] {
        let f = controlled_version(26).await?;
        let (post, author) = projection::seed(&f).await?;
        let create = projection::event(&f, post, author, author, "POST_CREATED").await?;
        projection::apply(&f, &create).await?;
        let like = projection::event(&f, post, author, Uuid::new_v4(), "POST_LIKED").await?;
        let claim = projection::authority(&f, post).await?;
        sqlx::query("update post_event_delivery_jobs set lease_expires_at=clock_timestamp()+interval '3 seconds' where event_id=$1 and target='TIMELINE'")
            .bind(like.event_id).execute(f.controller_pool.as_ref().unwrap()).await?;
        let mut tx = f.pool.begin().await?;
        assert!(
            sqlx::query_scalar::<_, bool>(
                "select feed_affinity_resolve_ordered('OUTBOX',$1,$2,$3,$4,$5,repeat('0',64))"
            )
            .bind(like.event_id)
            .bind(claim.event.event_id)
            .bind(claim.lease_token)
            .bind(claim.attempt_id)
            .bind(claim.attempt_number)
            .fetch_one(&mut *tx)
            .await?
        );
        sqlx::query("insert into timeline_projection_receipts(event_id,aggregate_id,aggregate_version,identity_sha256,disposition,delivery_event_id,delivery_attempt_id) values($1,$2,2,repeat('0',64),'APPLIED',$1,$3)")
            .bind(like.event_id).bind(post).bind(claim.attempt_id).execute(&mut *tx).await?;
        sqlx::query("update timeline_projection_cursors set last_version=2 where aggregate_id=$1")
            .bind(post)
            .execute(&mut *tx)
            .await?;
        if discharge_early {
            sqlx::query("set constraints timeline_projection_delivery_authority,feed_affinity_verify_resolution immediate").execute(&mut *tx).await?;
        }
        sqlx::query("select pg_sleep(greatest(0,extract(epoch from(lease_expires_at-clock_timestamp())))+0.05) from post_event_delivery_jobs where event_id=$1 and target='TIMELINE'")
            .bind(like.event_id).execute(&mut *tx).await?;
        let result = tx.commit().await;
        assert_eq!(result.is_ok(), discharge_early);
        assert_eq!(state(&f).await?.0, i64::from(discharge_early));
        f.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn retained_impression_recommendations_keep_effects_after_post_retirement() -> Result<()> {
    for physically_removed in [false, true] {
        let f = controlled_version(26).await?;
        let (repo, e) = seed(&f).await?;
        let sql = if physically_removed {
            "delete from posts where id=$1"
        } else {
            "update posts set deleted_at=now() where id=$1"
        };
        sqlx::query(sql).bind(e.subject_id).execute(&f.pool).await?;
        assert!(repo.append_event(&e).await?);
        assert_eq!(state(&f).await?.0, 1);
        assert_eq!(state(&f).await?.1, 0.08);
        assert_eq!(
            sqlx::query_scalar::<_, String>(
                "select disposition from feed_affinity_resolution_receipts"
            )
            .fetch_one(&f.pool)
            .await?,
            "APPLIED"
        );
        f.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn physically_retired_outbox_post_uses_retained_owner_creation_proof_for_suppression(
) -> Result<()> {
    let f = controlled_version(26).await?;
    let (post, author) = projection::seed(&f).await?;
    let create = projection::event(&f, post, author, author, "POST_CREATED").await?;
    projection::apply(&f, &create).await?;
    let like = projection::event(&f, post, author, Uuid::new_v4(), "POST_LIKED").await?;
    sqlx::query("delete from posts where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    projection::apply(&f, &like).await?;
    assert_eq!(state(&f).await?, (1, 0.0, 0.0, None));
    assert_eq!(projection::cursor(&f, post).await?, 2);
    assert_eq!(count(&f, "feed_author_affinities").await?, 0);
    f.shutdown().await
}

#[tokio::test]
async fn stored_unlike_on_missing_post_locks_captured_owner_before_job_and_cursor() -> Result<()> {
    use post_service::adapters::out::persistence::{
        event_outbox::PgEventOutbox, timeline_projection_repository::PgTimelineProjectionRepository,
    };
    use post_service::application::{
        dto::timeline_projection_dto::*,
        ports::{EventOutboxPort, TimelineProjectionRepositoryPort},
    };
    use post_service::domain::post::StoredEvent;
    let f = controlled_version(26).await?;
    let (post, author) = projection::seed(&f).await?;
    let create = projection::event(&f, post, author, author, "POST_CREATED").await?;
    projection::apply(&f, &create).await?;
    let actor = Uuid::new_v4();
    let now = Utc::now();
    let stored = PgEventOutbox::new(f.pool.clone())
        .save(&StoredEvent {
            event_id: Uuid::new_v4(),
            event_type: "POST_UNLIKED".into(),
            aggregate_id: post,
            aggregate_version: None,
            actor_id: Some(actor),
            attributes: vec![],
            payload: serde_json::json!({"postId":post,"authorId":actor}).to_string(),
            occurred_at: now,
            created_at: now,
            published: false,
            published_at: None,
            replay_resolution_id: None,
            origin_traceparent: None,
        })
        .await?;
    let event = TimelineProjectionEventDto::from(&stored);
    assert!(matches!(
        event.effect()?,
        TimelineProjectionEffectDto::StoredUnlike { .. }
    ));
    sqlx::query("delete from posts where id=$1")
        .bind(post)
        .execute(&f.pool)
        .await?;
    let claim = projection::authority(&f, post).await?;
    let mut barrier = f.controller_pool.as_ref().unwrap().begin().await?;
    sqlx::query("select post_lock_subject_ids($1::uuid[])")
        .bind(vec![author])
        .execute(&mut *barrier)
        .await?;
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    let q = TimelineProjectionApplyDto {
        authority: Some(claim.clone()),
        work: TimelineProjectionWorkDto {
            effect: event.effect()?,
            event,
        },
        follower_count: None,
        high_fanout_threshold: 1000,
    };
    let task = tokio::spawn(async move { repo.apply(&q).await });
    tokio::time::timeout(std::time::Duration::from_secs(5),async {
        loop {
            let waiting:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like 'select post_lock_subject_ids%')").fetch_one(&f.pool).await?;
            if waiting {return Ok::<_,anyhow::Error>(());}
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await??;
    // These rows are still immediately lockable: owner was acquired before
    // either cursor/job lock, including the payload form without an owner.
    sqlx::query("select aggregate_id from timeline_projection_cursors where aggregate_id=$1 for update nowait").bind(post).execute(&mut *barrier).await?;
    sqlx::query("select event_id from post_event_delivery_jobs where event_id=$1 and target='TIMELINE' for update nowait").bind(claim.event.event_id).execute(&mut *barrier).await?;
    barrier.commit().await?;
    assert_eq!(
        tokio::time::timeout(std::time::Duration::from_secs(5), task).await???,
        TimelineProjectionOutcomeDto::Suppressed
    );
    assert_eq!(state(&f).await?, (1, 0.0, 0.0, None));
    f.shutdown().await
}
