//! Actual recommendation source admission, identity and retention races on PG.
use super::Fixture;
use anyhow::Result;
use chrono::{Duration, Utc};
use post_service::adapters::out::persistence::feed_recommendation_repository::PgFeedRecommendationRepository;
use post_service::application::dto::post_dto::RecordFeedRecommendationEventDto;
use post_service::application::ports::{
    FeedRecommendationMaintenancePort, FeedRecommendationRepositoryPort,
};
use post_service::application::use_cases::record_feed_recommendation_event::RecordFeedRecommendationEventUseCase;
use post_service::domain::recommendation::*;
use std::sync::Arc;
use uuid::Uuid;

pub(super) async fn seed(
    f: &Fixture,
) -> Result<(PgFeedRecommendationRepository, RecommendationEvent)> {
    let repo = PgFeedRecommendationRepository::new(f.pool.clone());
    let viewer = Uuid::new_v4();
    let author = Uuid::new_v4();
    let post = Uuid::new_v4();
    let now = Utc::now();
    sqlx::query("insert into posts(id,author_id,content,visibility,created_at) values($1,$2,'admission fixture','PUBLIC',now())")
        .bind(post).bind(author).execute(&f.pool).await?;
    let impression = Uuid::new_v4();
    let snapshot = FeedSnapshot {
        snapshot_id: Uuid::new_v4(),
        viewer_id: viewer,
        request_id: "native admission".into(),
        session_id: None,
        rank_version: "feed-linear-v1".into(),
        fallback_used: false,
        failed_sources: vec![],
        created_at: now,
        expires_at: now + Duration::hours(1),
        items: vec![FeedSnapshotItem {
            post_id: post,
            author_id: author,
            source: FeedCandidateSource::SelfHistory,
            reason_code: "admission fixture".into(),
            score_micros: 1,
            position: 0,
            impression_id: impression,
            propensity_micros: 1_000_000,
        }],
    };
    repo.create_snapshot(&snapshot).await?;
    repo.record_impressions(&[FeedRecommendationImpression {
        impression_id: impression,
        snapshot_id: snapshot.snapshot_id,
        viewer_id: viewer,
        post_id: post,
        author_id: author,
        source: FeedCandidateSource::SelfHistory,
        reason_code: "admission fixture".into(),
        position: 0,
        rank_version: snapshot.rank_version,
        request_id: snapshot.request_id.clone(),
        session_id: None,
        score_micros: 1,
        propensity_micros: 1_000_000,
        occurred_at: now,
    }])
    .await?;
    Ok((
        repo,
        RecommendationEvent {
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
            metadata_json: r#"{"private":"fixture","number":1}"#.into(),
            propensity: Some(1.0),
            impression_id: Some(impression),
            dwell_ms: None,
        },
    ))
}

async fn no_effects(f: &Fixture) -> Result<()> {
    for table in [
        "feed_recommendation_events",
        "feed_recent_actions",
        "feed_author_affinities",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!(
                "select count(*) from {table}{}",
                if table == "feed_recommendation_events" {
                    " where action<>'IMPRESSION'"
                } else {
                    ""
                }
            ))
            .fetch_one(&f.pool)
            .await?,
            0,
            "unexpected row in {table}"
        );
    }
    Ok(())
}
async fn wait(f: &Fixture, pattern: &str) -> Result<()> {
    tokio::time::timeout(std::time::Duration::from_secs(5),async {
        loop {
            let found:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like $1)")
                .bind(pattern).fetch_one(&f.pool).await?;
            if found {return Ok::<_,anyhow::Error>(());}
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await??;
    Ok(())
}

#[tokio::test]
async fn admission_locks_post_before_impression_and_source_pair_allocation() -> Result<()> {
    for soft_deleted in [false, true] {
        let f = Fixture::new(25).await?;
        let (repo, event) = seed(&f).await?;
        let mut barrier = f.pool.begin().await?;
        sqlx::query("select id from posts where id=$1 for update")
            .bind(event.subject_id)
            .execute(&mut *barrier)
            .await?;
        if soft_deleted {
            sqlx::query("update posts set deleted_at=now() where id=$1")
                .bind(event.subject_id)
                .execute(&mut *barrier)
                .await?;
        }
        let impression = event.impression_id;
        let task = tokio::spawn(async move { repo.append_event(&event).await });
        wait(&f, "select author_id from posts where id=%for update").await?;
        // Actual PG Lock wait proves the admission is stopped at the post.
        // The impression is still available, and no pair/source/effect exists.
        sqlx::query("select impression_id from feed_recommendation_impressions where impression_id=$1 for update nowait")
            .bind(impression).execute(&mut *barrier).await?;
        no_effects(&f).await?;
        for table in [
            "feed_affinity_source_pairs",
            "feed_affinity_source_contributions",
        ] {
            assert_eq!(
                sqlx::query_scalar::<_, i64>(&format!("select count(*) from {table}"))
                    .fetch_one(&f.pool)
                    .await?,
                0
            );
        }
        barrier.commit().await?;
        assert!(tokio::time::timeout(std::time::Duration::from_secs(5), task).await???);
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "select allocated_sequence from feed_affinity_source_pairs"
            )
            .fetch_one(&f.pool)
            .await?,
            1
        );
        f.shutdown().await?;
    }
    Ok(())
}

#[tokio::test]
async fn missing_impression_is_rejected_without_any_outcome_or_feature() -> Result<()> {
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    sqlx::query("delete from feed_recommendation_impressions where impression_id=$1")
        .bind(event.impression_id)
        .execute(&f.pool)
        .await?;
    assert!(
        repo.append_event(&event).await.is_err(),
        "an absent admission source must not be accepted"
    );
    no_effects(&f).await?;
    f.shutdown().await
}

#[tokio::test]
async fn actual_use_case_rechecks_impression_after_subject_lock_wait() -> Result<()> {
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    let request = RecordFeedRecommendationEventDto {
        event_id: event.event_id.to_string(),
        schema_version: 1,
        occurred_at: event.occurred_at.to_rfc3339(),
        viewer_id: event.viewer_id.to_string(),
        post_id: event.subject_id.to_string(),
        actor_id: Some(event.viewer_id.to_string()),
        position: Some(0),
        request_id: event.request_id.clone(),
        session_id: None,
        action: "CLICK".into(),
        source: Some(event.source),
        model_version: Some(event.model_version),
        impression_id: event.impression_id.map(|id| id.to_string()),
        propensity_micros: Some(1_000_000),
        dwell_ms: None,
        metadata_json: event.metadata_json,
    };
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select post_lock_subject_ids($1::uuid[])")
        .bind(vec![event.viewer_id])
        .execute(&mut *barrier)
        .await?;
    let case = RecordFeedRecommendationEventUseCase::new(Arc::new(repo));
    let task = tokio::spawn(async move { case.execute(request).await });
    wait(&f, "select post_lock_subject_ids%").await?;
    // The actual use case has already read/validated the impression; its
    // repository write is blocked. Retention wins before source admission.
    sqlx::query("delete from feed_recommendation_impressions where impression_id=$1")
        .bind(event.impression_id)
        .execute(&mut *barrier)
        .await?;
    barrier.commit().await?;
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(5), task)
            .await??
            .is_err()
    );
    no_effects(&f).await?;
    f.shutdown().await
}

#[tokio::test]
async fn retention_winning_impression_row_lock_rejects_before_insert() -> Result<()> {
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select impression_id from feed_recommendation_impressions where impression_id=$1 for update")
        .bind(event.impression_id).execute(&mut *barrier).await?;
    let copy = event.clone();
    let task = tokio::spawn(async move { repo.append_event(&copy).await });
    wait(&f, "%from feed_recommendation_impressions%for share%").await?;
    no_effects(&f).await?;
    sqlx::query("delete from feed_recommendation_impressions where impression_id=$1")
        .bind(event.impression_id)
        .execute(&mut *barrier)
        .await?;
    barrier.commit().await?;
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(5), task)
            .await??
            .is_err()
    );
    no_effects(&f).await?;
    f.shutdown().await
}

#[tokio::test]
async fn writer_source_lock_makes_actual_retention_skip_until_commit() -> Result<()> {
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    let key = Uuid::new_v4().as_u128() as i64;
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(&mut *barrier)
        .await?;
    sqlx::raw_sql(&format!("create function hold_admission_insert() returns trigger language plpgsql as $$ begin perform pg_advisory_xact_lock({key}); return new; end; $$; create trigger hold_admission_insert before insert on feed_recommendation_events for each row execute function hold_admission_insert();"))
        .execute(&f.pool).await?;
    let writer = repo.clone();
    let copy = event.clone();
    let task = tokio::spawn(async move { writer.append_event(&copy).await });
    wait(&f, "%insert into feed_recommendation_events%").await?;
    let expired = Utc::now() + Duration::hours(2);
    let result = repo
        .purge_expired(expired, Utc::now() - Duration::days(1), 10)
        .await?;
    assert_eq!(result.impressions, 0);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_recommendation_impressions")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    barrier.commit().await?;
    assert!(tokio::time::timeout(std::time::Duration::from_secs(5), task).await???);
    assert_eq!(
        repo.purge_expired(expired, Utc::now() - Duration::days(1), 10)
            .await?
            .impressions,
        1
    );
    // Exact durable duplicates need no new source admission or decay.
    assert!(!repo.append_event(&event).await?);
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from feed_recommendation_events where action<>'IMPRESSION'"
        )
        .fetch_one(&f.pool)
        .await?,
        1
    );
    f.shutdown().await
}

#[tokio::test]
async fn outcome_identity_rejects_conflicts_and_normalizes_persisted_json() -> Result<()> {
    use post_service::application::dto::mutation_dto::MutationError;
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    assert!(repo.append_event(&event).await?);
    let mut exact = event.clone();
    exact.metadata_json = r#"{ "number":1.0, "private":"fixture" }"#.into();
    assert!(!repo.append_event(&exact).await?);
    for variation in 0..6 {
        let mut changed = event.clone();
        match variation {
            0 => changed.action = RecommendationAction::Like,
            1 => changed.viewer_id = Uuid::new_v4(),
            2 => changed.subject_id = Uuid::new_v4(),
            3 => changed.impression_id = Some(Uuid::new_v4()),
            4 => changed.occurred_at += Duration::seconds(1),
            _ => changed.metadata_json = r#"{"private":"changed"}"#.into(),
        }
        let error = repo
            .append_event(&changed)
            .await
            .expect_err("changed identity must conflict");
        assert!(matches!(
            error.downcast_ref::<MutationError>(),
            Some(MutationError::Conflict)
        ));
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_recent_actions")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    let score: f64 = sqlx::query_scalar("select affinity_score from feed_author_affinities")
        .fetch_one(&f.pool)
        .await?;
    assert!((score - 0.08).abs() < 1e-12);
    f.shutdown().await
}

#[tokio::test]
async fn concurrent_outcome_duplicates_commit_one_source_recent_action_and_feature() -> Result<()> {
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    let (a, b) = tokio::join!(repo.append_event(&event), repo.append_event(&event));
    assert_eq!(i32::from(a?) + i32::from(b?), 1);
    for table in [
        "feed_recommendation_events",
        "feed_recent_actions",
        "feed_author_affinities",
    ] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!(
                "select count(*) from {table}{}",
                if table == "feed_recommendation_events" {
                    " where action<>'IMPRESSION'"
                } else {
                    ""
                }
            ))
            .fetch_one(&f.pool)
            .await?,
            1
        );
    }
    f.shutdown().await
}

#[tokio::test]
async fn affinity_failure_rolls_back_admitted_source_and_recent_action() -> Result<()> {
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    sqlx::raw_sql("create function fail_admission_affinity() returns trigger language plpgsql as $$ begin raise exception 'controlled feature rollback'; end; $$; create trigger fail_admission_affinity before insert on feed_author_affinities for each row execute function fail_admission_affinity();")
        .execute(&f.pool).await?;
    assert!(repo.append_event(&event).await.is_err());
    no_effects(&f).await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from feed_recommendation_impressions")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    f.shutdown().await
}

#[tokio::test]
async fn wrong_viewer_post_or_subject_kind_cannot_admit_a_new_outcome() -> Result<()> {
    let f = Fixture::new(24).await?;
    let (repo, event) = seed(&f).await?;
    for variation in 0..3 {
        let mut changed = event.clone();
        match variation {
            0 => {
                changed.viewer_id = Uuid::new_v4();
                changed.actor_id = Some(changed.viewer_id);
            }
            1 => changed.subject_id = Uuid::new_v4(),
            _ => changed.subject_type = RecommendationSubjectType::User,
        }
        assert!(repo.append_event(&changed).await.is_err());
        no_effects(&f).await?;
    }
    f.shutdown().await
}

#[tokio::test]
async fn disjoint_subject_writers_serialize_global_identity_before_admission() -> Result<()> {
    use post_service::application::dto::mutation_dto::MutationError;
    let f = Fixture::new(24).await?;
    let (a, event_a) = seed(&f).await?;
    let (b, mut event_b) = seed(&f).await?;
    event_b.event_id = event_a.event_id;
    let key = Uuid::new_v4().as_u128() as i64;
    let mut barrier = f.pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock($1)")
        .bind(key)
        .execute(&mut *barrier)
        .await?;
    sqlx::raw_sql(&format!("create function hold_identity_race() returns trigger language plpgsql as $$ begin perform pg_advisory_xact_lock({key}); return new; end; $$; create trigger hold_identity_race before insert on feed_recommendation_events for each row execute function hold_identity_race();"))
        .execute(&f.pool).await?;
    let task_a = tokio::spawn(async move { a.append_event(&event_a).await });
    let task_b = tokio::spawn(async move { b.append_event(&event_b).await });
    tokio::time::timeout(std::time::Duration::from_secs(5),async {
        loop {
            let n:i64=sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like '%insert into feed_recommendation_events%'")
                .fetch_one(&f.pool).await?;
            let identity_waiters:i64=sqlx::query_scalar("select count(*) from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like 'select pg_advisory_xact_lock(1380270915,%'")
                .fetch_one(&f.pool).await?;
            if n==1 && identity_waiters==1 {return Ok::<_,anyhow::Error>(());}
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await??;
    // One real source insertion is held; the disjoint-subject contender
    // waits before discovery/admission on the shared global event identity.
    barrier.commit().await?;
    let a = tokio::time::timeout(std::time::Duration::from_secs(5), task_a).await??;
    let b = tokio::time::timeout(std::time::Duration::from_secs(5), task_b).await??;
    let mut accepted = 0;
    let mut conflicts = 0;
    for r in [a, b] {
        match r {
            Ok(true) => accepted += 1,
            Err(e)
                if matches!(
                    e.downcast_ref::<MutationError>(),
                    Some(MutationError::Conflict)
                ) =>
            {
                conflicts += 1
            }
            other => panic!("unexpected collision outcome: {other:?}"),
        }
    }
    assert_eq!((accepted, conflicts), (1, 1));
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from feed_recommendation_events where action<>'IMPRESSION'"
        )
        .fetch_one(&f.pool)
        .await?,
        1
    );
    for table in ["feed_recent_actions", "feed_author_affinities"] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(&format!("select count(*) from {table}"))
                .fetch_one(&f.pool)
                .await?,
            1
        );
    }
    f.shutdown().await
}
