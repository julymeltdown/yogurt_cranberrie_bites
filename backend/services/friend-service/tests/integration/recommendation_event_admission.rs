//! Actual PG identity/admission transactions. No public HTTP replay or retention
//! horizon is qualified; exact retries require a retained authoritative event.
use super::recommendation_event_atomicity::{hide_event, seed_impression};
use crate::support::start_postgres_app;
use anyhow::Result;
use chrono::Duration;
use friend_service::application::dto::mutation_dto::MutationError;
use friend_service::domain::error::FriendError;
use platform::recommendation::{
    RecommendationAction, RecommendationSubjectType, RecommendationSurface,
};
use uuid::Uuid;

async fn snapshot(pool: &sqlx::PgPool) -> Result<serde_json::Value> {
    Ok(sqlx::query_scalar("select jsonb_build_object('events',coalesce((select jsonb_agg(to_jsonb(e) order by event_id) from people_recommendation_events e),'[]'::jsonb),'refresh',coalesce((select jsonb_agg(to_jsonb(j) order by viewer_id) from people_recommendation_refresh_jobs j),'[]'::jsonb))").fetch_one(pool).await?)
}

#[tokio::test]
async fn exact_semantic_retry_survives_impression_pruning_and_conflicting_fields_are_effect_free(
) -> Result<()> {
    let app = start_postgres_app(false).await?;
    let mut event = hide_event(Uuid::new_v4());
    event.metadata_json = r#"{"large":9007199254740993,"nested":{"b":2,"a":1}}"#.into();
    seed_impression(&app.pool, &event).await;
    assert!(
        app.state
            .record_recommendation_event
            .execute(event.clone())
            .await?
    );
    let before = snapshot(&app.pool).await?;
    sqlx::query("delete from people_recommendation_impressions where impression_id=$1")
        .bind(event.impression_id)
        .execute(&app.pool)
        .await?;
    let mut equivalent = event.clone();
    equivalent.metadata_json = r#"{ "nested":{"a":1,"b":2}, "large":9007199254740993.0 }"#.into();
    assert!(
        !app.state
            .record_recommendation_event
            .execute(equivalent)
            .await?
    );
    let mut variants = Vec::new();
    let mut v = event.clone();
    v.viewer_id = Uuid::new_v4();
    variants.push(v);
    let mut v = event.clone();
    v.subject_id = Uuid::new_v4();
    variants.push(v);
    let mut v = event.clone();
    v.actor_id = Some(Uuid::new_v4());
    variants.push(v);
    let mut v = event.clone();
    v.occurred_at += Duration::microseconds(1);
    variants.push(v);
    let mut v = event.clone();
    v.subject_type = RecommendationSubjectType::Post;
    variants.push(v);
    let mut v = event.clone();
    v.surface = RecommendationSurface::HomeFeed;
    variants.push(v);
    let mut v = event.clone();
    v.position = None;
    variants.push(v);
    let mut v = event.clone();
    v.request_id = None;
    variants.push(v);
    let mut v = event.clone();
    v.session_id = Some(Uuid::new_v4());
    variants.push(v);
    let mut v = event.clone();
    v.action = RecommendationAction::Click;
    variants.push(v);
    let mut v = event.clone();
    v.source = "other".into();
    variants.push(v);
    let mut v = event.clone();
    v.model_version = "different".into();
    variants.push(v);
    let mut v = event.clone();
    v.impression_id = Some(Uuid::new_v4());
    variants.push(v);
    let mut v = event.clone();
    v.propensity_micros = Some(999999);
    variants.push(v);
    let mut v = event.clone();
    v.dwell_ms = Some(1);
    variants.push(v);
    let mut v = event.clone();
    v.metadata_json = r#"{"large":9007199254740992,"nested":{"b":2,"a":1}}"#.into();
    variants.push(v);
    for changed in variants {
        let error = app
            .state
            .record_recommendation_event
            .execute(changed)
            .await
            .unwrap_err();
        assert_eq!(
            error.downcast_ref::<MutationError>(),
            Some(&MutationError::Conflict)
        );
        assert_eq!(snapshot(&app.pool).await?, before);
    }
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn new_outcomes_reject_missing_foreign_or_wrong_candidate_impressions_without_refresh(
) -> Result<()> {
    let app = start_postgres_app(false).await?;
    let original = hide_event(Uuid::new_v4());
    seed_impression(&app.pool, &original).await;
    let mut variants = Vec::new();
    let mut e = original.clone();
    e.impression_id = Some(Uuid::new_v4());
    variants.push(e);
    let mut e = original.clone();
    e.viewer_id = Uuid::new_v4();
    variants.push(e);
    let mut e = original.clone();
    e.subject_id = Uuid::new_v4();
    variants.push(e);
    for event in variants {
        let error = app
            .state
            .record_recommendation_event
            .execute(event)
            .await
            .unwrap_err();
        assert!(matches!(
            error.downcast_ref::<FriendError>(),
            Some(FriendError::InvalidInput(_))
        ));
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from people_recommendation_events")
            .fetch_one(&app.pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from people_recommendation_refresh_jobs")
            .fetch_one(&app.pool)
            .await?,
        0
    );
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn concurrent_exact_retries_insert_one_event_and_do_not_requeue_after_completion(
) -> Result<()> {
    let app = start_postgres_app(false).await?;
    let event = hide_event(Uuid::new_v4());
    seed_impression(&app.pool, &event).await;
    let mut handles = Vec::new();
    for _ in 0..12 {
        let state = app.state.clone();
        let e = event.clone();
        handles.push(tokio::spawn(async move {
            state.record_recommendation_event.execute(e).await
        }));
    }
    let mut accepted = 0;
    for handle in handles {
        accepted += usize::from(handle.await??);
    }
    assert_eq!(accepted, 1);
    sqlx::query("delete from people_recommendation_refresh_jobs where viewer_id=$1")
        .bind(event.viewer_id)
        .execute(&app.pool)
        .await?;
    let before = snapshot(&app.pool).await?;
    assert!(!app.state.record_recommendation_event.execute(event).await?);
    assert_eq!(snapshot(&app.pool).await?, before);
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn identity_wait_precedes_subject_locks_and_raw_writer_cannot_mutate_retained_identity(
) -> Result<()> {
    let app = start_postgres_app(false).await?;
    let event = hide_event(Uuid::new_v4());
    seed_impression(&app.pool, &event).await;
    let mut hold = app.pool.begin().await?;
    sqlx::query("select pg_advisory_xact_lock(1380270916,hashtext($1::text))")
        .bind(event.event_id.to_string())
        .execute(&mut *hold)
        .await?;
    let state = app.state.clone();
    let e = event.clone();
    let job = tokio::spawn(async move { state.record_recommendation_event.execute(e).await });
    tokio::time::timeout(std::time::Duration::from_secs(5),async {
        loop {
            let waiting:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event='advisory' and pid<>pg_backend_pid())").fetch_one(&app.pool).await?;
            if waiting {break Ok::<_,anyhow::Error>(());}
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await??;
    assert!(
        sqlx::query_scalar::<_, bool>(
            "select pg_try_advisory_xact_lock(hashtextextended($1::text,0))"
        )
        .bind(event.viewer_id.to_string())
        .fetch_one(&mut *hold)
        .await?
    );
    hold.rollback().await?;
    assert!(job.await??);
    let error =
        sqlx::query("update people_recommendation_events set action='CLICK' where event_id=$1")
            .bind(event.event_id)
            .execute(&app.pool)
            .await
            .unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.constraint()),
        Some("friend_recommendation_event_identity")
    );
    let error=sqlx::query("insert into people_recommendation_events(event_id,schema_version,occurred_at,viewer_id,subject_type,subject_id,surface,action,source,model_version,impression_id) values($1,1,now(),$2,'USER',$3,'PEOPLE_RECOMMENDATION','HIDE','test','test',$4)")
        .bind(Uuid::new_v4()).bind(event.viewer_id).bind(event.subject_id).bind(Uuid::new_v4()).execute(&app.pool).await.unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.constraint()),
        Some("friend_recommendation_impression_admission")
    );
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn actual_impression_lock_wait_rechecks_deletion_before_event_or_refresh_commit() -> Result<()>
{
    let app = start_postgres_app(false).await?;
    let event = hide_event(Uuid::new_v4());
    seed_impression(&app.pool, &event).await;
    let mut purge = app.pool.begin().await?;
    sqlx::query("select impression_id from people_recommendation_impressions where impression_id=$1 for update")
        .bind(event.impression_id).execute(&mut *purge).await?;
    let state = app.state.clone();
    let e = event.clone();
    let job = tokio::spawn(async move { state.record_recommendation_event.execute(e).await });
    tokio::time::timeout(std::time::Duration::from_secs(5),async {
        loop {
            let waiting:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event='transactionid' and pid<>pg_backend_pid())").fetch_one(&app.pool).await?;
            if waiting {break Ok::<_,anyhow::Error>(());}
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await??;
    sqlx::query("delete from people_recommendation_impressions where impression_id=$1")
        .bind(event.impression_id)
        .execute(&mut *purge)
        .await?;
    purge.commit().await?;
    let error = job.await?.unwrap_err();
    assert!(matches!(
        error.downcast_ref::<FriendError>(),
        Some(FriendError::InvalidInput(_))
    ));
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from people_recommendation_events")
            .fetch_one(&app.pool)
            .await?,
        0
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from people_recommendation_refresh_jobs")
            .fetch_one(&app.pool)
            .await?,
        0
    );
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn actual_distinct_actor_erasure_removes_event_and_stale_retry_cannot_recreate_it(
) -> Result<()> {
    use friend_service::application::dto::friend_dto::ApplyUserDeletionDto;
    let app = start_postgres_app(false).await?;
    let mut event = hide_event(Uuid::new_v4());
    let actor = Uuid::new_v4();
    event.actor_id = Some(actor);
    seed_impression(&app.pool, &event).await;
    assert!(
        app.state
            .record_recommendation_event
            .execute(event.clone())
            .await?
    );
    app.state
        .apply_user_deletion
        .execute(ApplyUserDeletionDto {
            saga_id: Uuid::new_v4().to_string(),
            user_id: actor.to_string(),
            actor_id: actor.to_string(),
            retention_until: "2027-01-01T00:00:00Z".into(),
        })
        .await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from people_recommendation_events where event_id=$1"
        )
        .bind(event.event_id)
        .fetch_one(&app.pool)
        .await?,
        0
    );
    let before = snapshot(&app.pool).await?;
    let error = app
        .state
        .record_recommendation_event
        .execute(event)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<FriendError>(),
        Some(FriendError::PermissionDenied(_))
    ));
    assert_eq!(snapshot(&app.pool).await?, before);
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn actual_grpc_handler_reports_conflicting_identity_with_existing_conflict_metadata(
) -> Result<()> {
    use friend_service::adapters::r#in::grpc::server::{proto, FriendGrpcServer};
    use proto::friend_service_server::FriendService;
    let app = start_postgres_app(false).await?;
    let event = hide_event(Uuid::new_v4());
    seed_impression(&app.pool, &event).await;
    assert!(
        app.state
            .record_recommendation_event
            .execute(event.clone())
            .await?
    );
    let grpc = FriendGrpcServer::new(app.state.clone());
    let request = proto::RecordRecommendationEventRequest {
        event_id: event.event_id.to_string(),
        schema_version: 1,
        occurred_at: event.occurred_at.to_rfc3339(),
        viewer_id: event.viewer_id.to_string(),
        subject_type: "USER".into(),
        subject_id: event.subject_id.to_string(),
        surface: "PEOPLE_RECOMMENDATION".into(),
        position: 0,
        request_id: event.request_id.unwrap(),
        action: "CLICK".into(),
        source: event.source,
        model_version: event.model_version,
        impression_id: event.impression_id.unwrap().to_string(),
        metadata_json: "{}".into(),
        ..Default::default()
    };
    let status = grpc
        .record_people_recommendation_event(tonic::Request::new(request))
        .await
        .unwrap_err();
    assert_eq!(status.code(), tonic::Code::AlreadyExists);
    assert_eq!(
        status
            .metadata()
            .get(platform::mutation_context::ERROR_CODE)
            .unwrap(),
        "conflict"
    );
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn raw_noop_update_rejects_before_waiting_on_lifecycle_subject_lock() -> Result<()> {
    let app = start_postgres_app(false).await?;
    let event = hide_event(Uuid::new_v4());
    seed_impression(&app.pool, &event).await;
    assert!(
        app.state
            .record_recommendation_event
            .execute(event.clone())
            .await?
    );
    let before = snapshot(&app.pool).await?;
    let mut erasure = app.pool.begin().await?;
    sqlx::query("select friend_lock_subject_ids($1::uuid[])")
        .bind(vec![event.viewer_id, event.subject_id])
        .execute(&mut *erasure)
        .await?;
    // The row is acquired by UPDATE before its BEFORE trigger. Rejection must
    // complete while lifecycle retains its subject lock; no reversed wait.
    let error = tokio::time::timeout(
        std::time::Duration::from_secs(2),
        sqlx::query("update people_recommendation_events set event_id=event_id where event_id=$1")
            .bind(event.event_id)
            .execute(&app.pool),
    )
    .await?
    .unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.constraint()),
        Some("friend_recommendation_event_identity")
    );
    erasure.rollback().await?;
    assert_eq!(snapshot(&app.pool).await?, before);
    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn raw_invalid_metadata_negative_control_and_bounded_object_admission() -> Result<()> {
    let app = start_postgres_app(false).await?;
    let event = hide_event(Uuid::new_v4());
    seed_impression(&app.pool, &event).await;
    let sql="insert into people_recommendation_events(event_id,schema_version,occurred_at,viewer_id,subject_type,subject_id,surface,action,source,model_version,impression_id,metadata_json) values($1,1,now(),$2,'USER',$3,'PEOPLE_RECOMMENDATION','HIDE','raw','raw',$4,$5)";
    // Reproduce the committed0012 path in this disposable fixture by disabling
    // only the additive metadata guard; no historical production cause claim.
    sqlx::query(
        "alter table people_recommendation_events disable trigger a0000_people_event_metadata",
    )
    .execute(&app.pool)
    .await?;
    sqlx::query(sql)
        .bind(event.event_id)
        .bind(event.viewer_id)
        .bind(event.subject_id)
        .bind(event.impression_id)
        .bind("not-json")
        .execute(&app.pool)
        .await?;
    let error=sqlx::query("insert into people_recommendation_events select * from people_recommendation_events where event_id=$1")
        .bind(event.event_id).execute(&app.pool).await.unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("22P02")
    );
    sqlx::query("delete from people_recommendation_events where event_id=$1")
        .bind(event.event_id)
        .execute(&app.pool)
        .await?;
    sqlx::query(
        "alter table people_recommendation_events enable trigger a0000_people_event_metadata",
    )
    .execute(&app.pool)
    .await?;
    let oversized = format!("{{\"data\":\"{}\"}}", "x".repeat(4096));
    for metadata in [
        "not-json",
        "[]",
        "null",
        "1",
        r#"{"data":"\u0000"}"#,
        oversized.as_str(),
    ] {
        let error = sqlx::query(sql)
            .bind(event.event_id)
            .bind(event.viewer_id)
            .bind(event.subject_id)
            .bind(event.impression_id)
            .bind(metadata)
            .execute(&app.pool)
            .await
            .unwrap_err();
        assert_eq!(
            error.as_database_error().and_then(|e| e.constraint()),
            Some("friend_recommendation_metadata_admission")
        );
        assert_eq!(
            sqlx::query_scalar::<_, i64>("select count(*) from people_recommendation_events")
                .fetch_one(&app.pool)
                .await?,
            0
        );
    }
    assert_eq!(
        sqlx::query(sql)
            .bind(event.event_id)
            .bind(event.viewer_id)
            .bind(event.subject_id)
            .bind(event.impression_id)
            .bind(r#"{"valid":1}"#)
            .execute(&app.pool)
            .await?
            .rows_affected(),
        1
    );
    assert_eq!(sqlx::query("insert into people_recommendation_events select * from people_recommendation_events where event_id=$1").bind(event.event_id).execute(&app.pool).await?.rows_affected(),0);
    app.shutdown().await?;
    Ok(())
}
