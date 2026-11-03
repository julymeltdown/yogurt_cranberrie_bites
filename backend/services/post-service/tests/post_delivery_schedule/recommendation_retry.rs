//! PG27 source-absence proof, including the actual use case and erasure.
//! Gateway-generated HTTP event IDs and production retention are separate.
use super::{
    affinity_baseline::controlled_version, projection, recommendation_admission::seed, Fixture,
};
use anyhow::Result;
use chrono::{Duration, Utc};
use post_service::adapters::out::persistence::{
    feed_recommendation_repository::PgFeedRecommendationRepository,
    user_lifecycle_repository::PgUserLifecycleRepository,
};
use post_service::application::dto::{
    mutation_dto::MutationError, post_dto::RecordFeedRecommendationEventDto,
};
use post_service::application::ports::{
    FeedRecommendationMaintenancePort, FeedRecommendationRepositoryPort,
    TimelineProjectionRepositoryPort, UserLifecycleRepositoryPort,
};
use post_service::application::use_cases::record_feed_recommendation_event::RecordFeedRecommendationEventUseCase;
use post_service::domain::recommendation::RecommendationEvent;
use std::sync::Arc;
use uuid::Uuid;

fn request(e: &RecommendationEvent) -> RecordFeedRecommendationEventDto {
    RecordFeedRecommendationEventDto {
        event_id: e.event_id.to_string(),
        schema_version: e.schema_version,
        occurred_at: e.occurred_at.to_rfc3339(),
        viewer_id: e.viewer_id.to_string(),
        post_id: e.subject_id.to_string(),
        actor_id: e.actor_id.map(|id| id.to_string()),
        position: e.position,
        request_id: e.request_id.clone(),
        session_id: e.session_id.map(|id| id.to_string()),
        action: e.action.as_str().into(),
        source: Some(e.source.clone()),
        model_version: Some(e.model_version.clone()),
        impression_id: e.impression_id.map(|id| id.to_string()),
        propensity_micros: e.propensity.map(|p| (p * 1_000_000.0).round() as u32),
        dwell_ms: e.dwell_ms,
        metadata_json: e.metadata_json.clone(),
    }
}
async fn head(f: &Fixture) -> Result<serde_json::Value> {
    Ok(
        sqlx::query_scalar("select to_jsonb(h) from feed_affinity_pair_heads h")
            .fetch_one(&f.pool)
            .await?,
    )
}
async fn count(f: &Fixture, table: &str) -> Result<i64> {
    Ok(sqlx::query_scalar(&format!("select count(*) from {table}"))
        .fetch_one(&f.pool)
        .await?)
}
async fn prune(repo: &PgFeedRecommendationRepository, f: &Fixture) -> Result<()> {
    let removed = repo
        .purge_expired(
            Utc::now() + Duration::days(2),
            Utc::now() + Duration::days(1),
            100,
        )
        .await?;
    assert!(removed.events > 0);
    assert_eq!(count(f, "feed_recommendation_events").await?, 0);
    assert_eq!(count(f, "feed_recommendation_impressions").await?, 0);
    assert_eq!(count(f, "feed_recent_actions").await?, 0);
    Ok(())
}

#[tokio::test]
async fn exact_repository_retry_after_actual_retention_is_effect_free_and_conflicts_fail(
) -> Result<()> {
    let f = controlled_version(27).await?;
    let (repo, mut e) = seed(&f).await?;
    e.metadata_json = r#"{"number":9007199254740993,"nested":{"x":1}}"#.into();
    assert!(repo.append_event(&e).await?);
    let before = head(&f).await?;
    prune(&repo, &f).await?;
    sqlx::query("delete from posts where id=$1")
        .bind(e.subject_id)
        .execute(&f.pool)
        .await?;
    let mut equivalent = e.clone();
    equivalent.metadata_json = r#"{"nested":{"x":1.0},"number":9007199254740993}"#.into();
    assert!(!repo.append_event(&equivalent).await?);
    for n in 0..12 {
        let mut changed = e.clone();
        match n {
            0 => changed.metadata_json = r#"{"number":9007199254740994,"nested":{"x":1}}"#.into(),
            1 => changed.actor_id = None,
            2 => changed.position = None,
            3 => changed.request_id = None,
            4 => changed.session_id = Some(Uuid::new_v4()),
            5 => changed.source = "other".into(),
            6 => changed.model_version = "other-v1".into(),
            7 => changed.impression_id = Some(Uuid::new_v4()),
            8 => changed.propensity = Some(0.5),
            9 => changed.dwell_ms = Some(1),
            10 => changed.occurred_at += Duration::microseconds(1),
            _ => changed.viewer_id = Uuid::new_v4(),
        }
        let err = repo.append_event(&changed).await.unwrap_err();
        assert!(
            matches!(
                err.downcast_ref::<MutationError>(),
                Some(MutationError::Conflict)
            ),
            "{n}: {err}"
        );
    }
    assert_eq!(head(&f).await?, before);
    assert_eq!(count(&f, "feed_affinity_resolution_receipts").await?, 1);
    assert_eq!(count(&f, "feed_recommendation_retry_contexts").await?, 1);
    assert_eq!(count(&f, "feed_recommendation_events").await?, 0);
    let mut unseen = e;
    unseen.event_id = Uuid::new_v4();
    assert!(repo.append_event(&unseen).await.is_err());
    f.shutdown().await
}

#[tokio::test]
async fn use_case_reconstructs_original_omitted_defaults_after_impression_and_body_removal(
) -> Result<()> {
    let f = controlled_version(27).await?;
    let (repo, e) = seed(&f).await?;
    let case = RecordFeedRecommendationEventUseCase::new(Arc::new(repo.clone()));
    let mut q = request(&e);
    q.position = None;
    q.request_id = None;
    q.session_id = None;
    q.source = None;
    q.model_version = None;
    q.propensity_micros = None;
    q.impression_id = None;
    assert!(case.execute(q.clone()).await?.accepted);
    let before = head(&f).await?;
    // A newer impression with different defaults must not rewrite an old
    // accepted request's normalization. It remains original receipt context.
    sqlx::query("insert into feed_recommendation_impressions select $1,snapshot_id,viewer_id,post_id,author_id,source,reason_code,position,rank_version,'newer-request',$2,score_micros,500000,occurred_at+interval '1 second' from feed_recommendation_impressions where impression_id=$3")
        .bind(Uuid::new_v4()).bind(Uuid::new_v4()).bind(e.impression_id).execute(&f.pool).await?;
    assert!(!case.execute(q.clone()).await?.accepted);
    prune(&repo, &f).await?;
    assert!(!case.execute(q.clone()).await?.accepted);
    q.impression_id = e.impression_id.map(|id| id.to_string());
    assert!(!case.execute(q.clone()).await?.accepted);
    let mut changed = q.clone();
    changed.metadata_json = "{}".into();
    assert!(case.execute(changed).await.is_err());
    changed = q.clone();
    changed.impression_id = Some(Uuid::new_v4().to_string());
    assert!(case.execute(changed).await.is_err());
    for changed_post in [false, true] {
        let mut changed = q.clone();
        if changed_post {
            changed.post_id = Uuid::new_v4().to_string();
        } else {
            changed.viewer_id = Uuid::new_v4().to_string();
        }
        let err = case.execute(changed).await.unwrap_err();
        assert!(matches!(
            err.downcast_ref::<MutationError>(),
            Some(MutationError::Conflict)
        ));
    }
    q.event_id = Uuid::new_v4().to_string();
    assert!(case.execute(q).await.is_err());
    assert_eq!(head(&f).await?, before);
    assert_eq!(count(&f, "feed_recommendation_events").await?, 0);
    f.shutdown().await
}

#[tokio::test]
async fn dropping_an_original_explicit_override_is_a_conflict_after_retention() -> Result<()> {
    let f = controlled_version(27).await?;
    let (repo, e) = seed(&f).await?;
    let case = RecordFeedRecommendationEventUseCase::new(Arc::new(repo.clone()));
    let mut q = request(&e);
    q.position = Some(7);
    q.request_id = Some("override-request".into());
    q.session_id = Some(Uuid::new_v4().to_string());
    q.model_version = Some("override-v1".into());
    q.propensity_micros = Some(600000);
    assert!(case.execute(q.clone()).await?.accepted);
    prune(&repo, &f).await?;
    for n in 0..7 {
        let mut changed = q.clone();
        match n {
            0 => changed.position = None,
            1 => changed.request_id = None,
            2 => changed.session_id = None,
            3 => changed.source = None,
            4 => changed.model_version = None,
            5 => changed.propensity_micros = None,
            _ => changed.actor_id = Some(Uuid::new_v4().to_string()),
        }
        let err = case.execute(changed).await.unwrap_err();
        assert!(
            matches!(
                err.downcast_ref::<MutationError>(),
                Some(MutationError::Conflict)
            ),
            "{n}: {err}"
        );
    }
    assert!(!case.execute(q).await?.accepted);
    f.shutdown().await
}

#[tokio::test]
async fn concurrent_source_absent_retries_preserve_receipts_and_scores() -> Result<()> {
    let f = controlled_version(27).await?;
    let (repo, e) = seed(&f).await?;
    assert!(repo.append_event(&e).await?);
    prune(&repo, &f).await?;
    let before = head(&f).await?;
    let (a, b) = tokio::join!(repo.append_event(&e), repo.append_event(&e));
    assert!(!a? && !b?);
    assert_eq!(head(&f).await?, before);
    assert_eq!(count(&f, "feed_affinity_resolution_receipts").await?, 1);
    f.shutdown().await
}

#[tokio::test]
async fn event_identity_lock_precedes_all_subject_locks_and_disjoint_collision_is_conflict(
) -> Result<()> {
    let f = controlled_version(27).await?;
    let (repo, e) = seed(&f).await?;
    let (_, mut unrelated) = seed(&f).await?;
    unrelated.event_id = e.event_id;
    let author: Uuid = sqlx::query_scalar("select author_id from posts where id=$1")
        .bind(e.subject_id)
        .fetch_one(&f.pool)
        .await?;
    let mut identity = f.pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(1380270915,hashtext($1::text))")
        .bind(e.event_id.to_string())
        .execute(&mut *identity)
        .await?;
    let repo_task = repo.clone();
    let candidate = e.clone();
    let task = tokio::spawn(async move { repo_task.append_event(&candidate).await });
    tokio::time::timeout(std::time::Duration::from_secs(5),async {
        loop {
            let waiting:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like 'select pg_advisory_xact_lock(1380270915,%')").fetch_one(&f.pool).await?;
            if waiting {return Ok::<_,anyhow::Error>(());}
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await??;
    for subject in [e.viewer_id, author] {
        assert!(
            sqlx::query_scalar::<_, bool>(
                "select pg_try_advisory_xact_lock(hashtextextended($1::text,0))"
            )
            .bind(subject.to_string())
            .fetch_one(&mut *identity)
            .await?
        );
    }
    identity.rollback().await?;
    assert!(tokio::time::timeout(std::time::Duration::from_secs(5), task).await???);
    let err = repo.append_event(&unrelated).await.unwrap_err();
    assert!(matches!(
        err.downcast_ref::<MutationError>(),
        Some(MutationError::Conflict)
    ));
    assert_eq!(count(&f, "feed_affinity_resolution_receipts").await?, 1);
    f.shutdown().await
}

#[tokio::test]
async fn raw_source_recreation_under_a_retained_receipt_is_rejected_before_new_allocation(
) -> Result<()> {
    let f = controlled_version(27).await?;
    let (repo, e) = seed(&f).await?;
    assert!(repo.append_event(&e).await?);
    prune(&repo, &f).await?;
    let before = head(&f).await?;
    let error=sqlx::query("insert into feed_recommendation_events(event_id,schema_version,occurred_at,viewer_id,subject_type,subject_id,actor_id,surface,action,source,model_version,metadata_json) values($1,1,now(),$2,'POST',$3,$2,'HOME_FEED','CLICK','raw','raw-v1','{}')")
        .bind(e.event_id).bind(e.viewer_id).bind(e.subject_id).execute(&f.pool).await.unwrap_err();
    assert_eq!(
        error.as_database_error().unwrap().constraint(),
        Some("feed_recommendation_retry_identity")
    );
    assert_eq!(count(&f, "feed_recommendation_events").await?, 0);
    assert_eq!(head(&f).await?, before);
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select allocated_sequence from feed_affinity_source_pairs")
            .fetch_one(&f.pool)
            .await?,
        1
    );
    f.shutdown().await
}

#[tokio::test]
async fn erasure_winning_between_discovery_and_locked_reread_cannot_recreate_context() -> Result<()>
{
    let f = controlled_version(27).await?;
    let (repo, e) = seed(&f).await?;
    assert!(repo.append_event(&e).await?);
    prune(&repo, &f).await?;
    let author: Uuid =
        sqlx::query_scalar("select author_id from feed_affinity_resolution_receipts")
            .fetch_one(&f.pool)
            .await?;
    let mut erasure = f.controller_pool.as_ref().unwrap().begin().await?;
    sqlx::query("select post_lock_subject_ids($1::uuid[])")
        .bind(vec![e.viewer_id, author])
        .execute(&mut *erasure)
        .await?;
    let q = request(&e);
    let case = RecordFeedRecommendationEventUseCase::new(Arc::new(repo));
    let task = tokio::spawn(async move { case.execute(q).await });
    tokio::time::timeout(std::time::Duration::from_secs(5),async {
        loop {
            let waiting:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like 'select post_lock_subject_ids%')").fetch_one(&f.pool).await?;
            if waiting {return Ok::<_,anyhow::Error>(());}
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await??;
    // Actual terminal tombstone and the owner erasure function, in the winning
    // transaction. The separate lifecycle probe covers coordinator composition.
    sqlx::query("insert into post_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until,applied_at) values($1,$2,$3,now()+interval '7 days',now())")
        .bind(e.viewer_id).bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&mut *erasure).await?;
    sqlx::query("select feed_affinity_forget_subject($1)")
        .bind(e.viewer_id)
        .execute(&mut *erasure)
        .await?;
    erasure.commit().await?;
    assert!(
        tokio::time::timeout(std::time::Duration::from_secs(5), task)
            .await??
            .is_err()
    );
    for table in [
        "feed_recommendation_retry_contexts",
        "feed_affinity_resolution_receipts",
        "feed_affinity_pair_heads",
        "feed_recommendation_events",
    ] {
        assert_eq!(count(&f, table).await?, 0);
    }
    f.shutdown().await
}

#[tokio::test]
async fn actual_terminal_erasure_removes_retry_context_without_reversing_unrelated_head(
) -> Result<()> {
    let f = controlled_version(27).await?;
    let (repo, mut e) = seed(&f).await?;
    let actor = Uuid::new_v4();
    e.actor_id = Some(actor);
    assert!(repo.append_event(&e).await?);
    prune(&repo, &f).await?;
    let before = head(&f).await?;
    PgUserLifecycleRepository::new(f.pool.clone())
        .apply_user_deletion(
            Uuid::new_v4(),
            actor,
            Uuid::new_v4(),
            Utc::now() + Duration::days(7),
        )
        .await?;
    assert_eq!(count(&f, "feed_recommendation_retry_contexts").await?, 0);
    assert_eq!(count(&f, "feed_affinity_resolution_receipts").await?, 0);
    assert_eq!(head(&f).await?, before);
    assert!(repo.append_event(&e).await.is_err());
    f.shutdown().await
}

#[tokio::test]
async fn missing_or_skipped_context_rolls_back_the_whole_acceptance_and_runtime_cannot_forge_it(
) -> Result<()> {
    let f = controlled_version(27).await?;
    let (repo, e) = seed(&f).await?;
    let owner = f.controller_pool.as_ref().unwrap();
    sqlx::raw_sql("create function skip_retry_context() returns trigger language plpgsql as $$begin return null; end;$$;create trigger aa_skip_retry_context before insert on feed_recommendation_retry_contexts for each row execute function skip_retry_context();").execute(owner).await?;
    assert!(repo.append_event(&e).await.is_err());
    assert_eq!(count(&f, "feed_affinity_pair_heads").await?, 0);
    assert_eq!(count(&f, "feed_recommendation_retry_contexts").await?, 0);
    sqlx::query("drop trigger aa_skip_retry_context on feed_recommendation_retry_contexts")
        .execute(owner)
        .await?;
    assert!(repo.append_event(&e).await?);
    for statement in ["update feed_recommendation_retry_contexts set source='forged'","delete from feed_recommendation_retry_contexts","insert into feed_recommendation_retry_contexts select * from feed_recommendation_retry_contexts"] {
        assert_eq!(sqlx::query(statement).execute(&f.pool).await.unwrap_err().as_database_error().unwrap().code().as_deref(),Some("42501"));
        assert!(sqlx::query(statement).execute(owner).await.is_err());
    }
    f.shutdown().await
}

#[tokio::test]
async fn reducer_itself_acknowledges_existing_resolution_without_the_owner_body() -> Result<()> {
    let f = controlled_version(27).await?;
    let (repo, e) = seed(&f).await?;
    assert!(repo.append_event(&e).await?);
    prune(&repo, &f).await?;
    let before = head(&f).await?;
    assert!(
        sqlx::query_scalar::<_, bool>(
            "select feed_affinity_resolve_ordered('RECOMMENDATION',$1,null,null,null,null,null)"
        )
        .bind(e.event_id)
        .fetch_one(&f.pool)
        .await?
    );
    assert_eq!(head(&f).await?, before);
    f.shutdown().await
}

#[tokio::test]
async fn controlled_outbox_duplicate_survives_simulated_source_absence() -> Result<()> {
    use post_service::adapters::out::persistence::timeline_projection_repository::PgTimelineProjectionRepository;
    use post_service::application::dto::timeline_projection_dto::{
        TimelineProjectionOutcomeDto, TimelineProjectionPlanDto,
    };
    let f = controlled_version(27).await?;
    let (post, author) = projection::seed(&f).await?;
    let create = projection::event(&f, post, author, author, "POST_CREATED").await?;
    projection::apply(&f, &create).await?;
    let like = projection::event(&f, post, author, Uuid::new_v4(), "POST_LIKED").await?;
    projection::apply(&f, &like).await?;
    let before = head(&f).await?;
    let owner = f.controller_pool.as_ref().unwrap();
    assert!(
        sqlx::query("delete from post_event_outbox where event_id=$1")
            .bind(like.event_id)
            .execute(owner)
            .await
            .is_err()
    );
    // Fault injection only: current immutable attempt FK forbids production
    // pruning. Every attempt row is preserved in this disposable fixture.
    sqlx::query("alter table post_event_delivery_attempts drop constraint post_event_delivery_attempts_event_id_fkey").execute(owner).await?;
    sqlx::query("delete from post_event_outbox where event_id=$1")
        .bind(like.event_id)
        .execute(owner)
        .await?;
    let repo = PgTimelineProjectionRepository::new(f.pool.clone());
    assert!(matches!(
        repo.prepare(&like, 100).await?,
        TimelineProjectionPlanDto::Duplicate
    ));
    assert_eq!(
        projection::apply(&f, &like).await?,
        TimelineProjectionOutcomeDto::Duplicate
    );
    let mut changed = like;
    changed.payload.push(' ');
    assert!(repo.prepare(&changed, 100).await.is_err());
    assert_eq!(head(&f).await?, before);
    f.shutdown().await
}

#[tokio::test]
async fn automatic_impression_writer_locks_event_batch_before_subjects() -> Result<()> {
    let f = controlled_version(27).await?;
    let (repo, e) = seed(&f).await?;
    let impression = repo
        .latest_impression(e.viewer_id, e.subject_id)
        .await?
        .unwrap();
    let author = impression.author_id;
    let mut identity = f.pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(1380270915,hashtext($1::text))")
        .bind(impression.impression_id.to_string())
        .execute(&mut *identity)
        .await?;
    let task = tokio::spawn(async move { repo.record_impressions(&[impression]).await });
    tokio::time::timeout(std::time::Duration::from_secs(5),async {
        loop {
            let waiting:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like 'select pg_advisory_xact_lock(1380270915,key)%')").fetch_one(&f.pool).await?;
            if waiting {return Ok::<_,anyhow::Error>(());}
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await??;
    for subject in [e.viewer_id, author] {
        assert!(
            sqlx::query_scalar::<_, bool>(
                "select pg_try_advisory_xact_lock(hashtextextended($1::text,0))"
            )
            .bind(subject.to_string())
            .fetch_one(&mut *identity)
            .await?
        );
    }
    identity.rollback().await?;
    tokio::time::timeout(std::time::Duration::from_secs(5), task).await???;
    assert_eq!(count(&f, "feed_affinity_resolution_receipts").await?, 0);
    f.shutdown().await
}

#[tokio::test]
async fn retained_source_defaults_use_the_same_alias_case_and_whitespace_normalization(
) -> Result<()> {
    let f = controlled_version(27).await?;
    let (repo, e) = seed(&f).await?;
    sqlx::query(
        "update feed_recommendation_impressions set source=' trending ' where impression_id=$1",
    )
    .bind(e.impression_id)
    .execute(&f.pool)
    .await?;
    let case = RecordFeedRecommendationEventUseCase::new(Arc::new(repo.clone()));
    let mut q = request(&e);
    q.source = None;
    assert!(case.execute(q.clone()).await?.accepted);
    assert_eq!(
        sqlx::query_scalar::<_, String>(
            "select source from feed_recommendation_events where event_id=$1"
        )
        .bind(e.event_id)
        .fetch_one(&f.pool)
        .await?,
        "FRESH_TRENDING"
    );
    assert_eq!(
        sqlx::query_scalar::<_, String>("select source from feed_recommendation_retry_contexts")
            .fetch_one(&f.pool)
            .await?,
        " trending "
    );
    // Retained defaults must also match while the original body still exists.
    assert!(!case.execute(q.clone()).await?.accepted);
    prune(&repo, &f).await?;
    let before = head(&f).await?;
    assert!(!case.execute(q.clone()).await?.accepted);
    q.source = Some("SELF_HISTORY".into());
    assert!(case.execute(q).await.is_err());
    assert_eq!(head(&f).await?, before);
    f.shutdown().await
}
