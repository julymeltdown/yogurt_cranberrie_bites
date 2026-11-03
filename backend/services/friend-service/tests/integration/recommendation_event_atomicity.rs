use crate::support::start_postgres_app;
use chrono::{Duration, Utc};
use friend_service::adapters::out::persistence::people_recommendation_repository::PgPeopleRecommendationRepository;
use friend_service::application::ports::PeopleRecommendationRepositoryPort;
use platform::recommendation::{
    RecommendationAction, RecommendationEvent, RecommendationSubjectType, RecommendationSurface,
    RECOMMENDATION_SCHEMA_VERSION,
};
use uuid::Uuid;

pub(super) fn hide_event(viewer_id: Uuid) -> RecommendationEvent {
    RecommendationEvent {
        event_id: Uuid::new_v4(),
        schema_version: RECOMMENDATION_SCHEMA_VERSION,
        occurred_at: Utc::now(),
        viewer_id,
        subject_type: RecommendationSubjectType::User,
        subject_id: Uuid::new_v4(),
        actor_id: None,
        surface: RecommendationSurface::PeopleRecommendation,
        position: Some(0),
        request_id: Some("atomic-refresh-test".into()),
        session_id: None,
        action: RecommendationAction::Hide,
        source: "mutual_follow".into(),
        model_version: "people-linear-v1".into(),
        impression_id: Some(Uuid::new_v4()),
        propensity_micros: None,
        dwell_ms: None,
        metadata_json: "{}".into(),
    }
}

pub(super) async fn seed_impression(pool: &sqlx::PgPool, event: &RecommendationEvent) {
    let snapshot = Uuid::new_v4();
    sqlx::query("insert into people_recommendation_snapshots(id,viewer_id,rank_version,created_at,expires_at) values($1,$2,$3,now(),now()+interval '1 hour')")
        .bind(snapshot).bind(event.viewer_id).bind(&event.model_version).execute(pool).await.expect("seed actual snapshot");
    sqlx::query("insert into people_recommendation_impressions(impression_id,snapshot_id,viewer_id,candidate_user_id,source,reason_code,rank_version,position,score_micros,request_id,occurred_at,propensity_micros) values($1,$2,$3,$4,$5,'MUTUAL_FOLLOW',$6,0,1,'fixture',now(),1000000)")
        .bind(event.impression_id).bind(snapshot).bind(event.viewer_id).bind(event.subject_id).bind(&event.source).bind(&event.model_version).execute(pool).await.expect("seed actual impression");
}

async fn event_count(pool: &sqlx::PgPool, event_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "select count(*)::bigint from people_recommendation_events where event_id=$1",
    )
    .bind(event_id)
    .fetch_one(pool)
    .await
    .expect("count persisted recommendation events")
}

async fn refresh_job_count(pool: &sqlx::PgPool, viewer_id: Uuid) -> i64 {
    sqlx::query_scalar(
        "select count(*)::bigint from people_recommendation_refresh_jobs where viewer_id=$1",
    )
    .bind(viewer_id)
    .fetch_one(pool)
    .await
    .expect("count recommendation refresh jobs")
}

#[tokio::test]
async fn failed_refresh_enqueue_rolls_back_negative_event_and_retry_recovers_atomically() {
    let app = start_postgres_app(false)
        .await
        .expect("start a real PostgreSQL FriendService app");
    sqlx::query(
        r#"
        create function reject_recommendation_refresh_enqueue() returns trigger
        language plpgsql as $$
        begin
            raise exception 'refresh enqueue rejected';
        end;
        $$
        "#,
    )
    .execute(&app.pool)
    .await
    .expect("install queue failure injection");
    sqlx::query(
        "create trigger reject_recommendation_refresh_enqueue before insert on people_recommendation_refresh_jobs for each row execute function reject_recommendation_refresh_enqueue()",
    )
    .execute(&app.pool)
    .await
    .expect("attach queue failure injection");

    let viewer_id = Uuid::new_v4();
    let event = hide_event(viewer_id);
    seed_impression(&app.pool, &event).await;
    let result = app
        .state
        .record_recommendation_event
        .execute(event.clone())
        .await;

    assert!(result.is_err(), "injected refresh enqueue must fail");
    assert_eq!(event_count(&app.pool, event.event_id).await, 0);
    assert_eq!(refresh_job_count(&app.pool, viewer_id).await, 0);

    sqlx::query(
        "drop trigger reject_recommendation_refresh_enqueue on people_recommendation_refresh_jobs",
    )
    .execute(&app.pool)
    .await
    .expect("remove queue failure injection");
    sqlx::query("drop function reject_recommendation_refresh_enqueue()")
        .execute(&app.pool)
        .await
        .expect("remove queue failure function");

    assert!(app
        .state
        .record_recommendation_event
        .execute(event.clone())
        .await
        .expect("retry event and refresh enqueue"));
    assert!(!app
        .state
        .record_recommendation_event
        .execute(event.clone())
        .await
        .expect("duplicate event remains idempotent"));
    assert_eq!(event_count(&app.pool, event.event_id).await, 1);
    assert_eq!(refresh_job_count(&app.pool, viewer_id).await, 1);
    app.shutdown().await.expect("fixture shutdown");
}

#[tokio::test]
async fn recommendation_refresh_enqueue_survives_stale_worker_completion() {
    let app = start_postgres_app(false)
        .await
        .expect("start a real PostgreSQL FriendService app");
    let repository = PgPeopleRecommendationRepository::new(app.pool.clone());
    let viewer_id = Uuid::new_v4();
    let claim_time = Utc::now();

    repository
        .enqueue_refresh(&[viewer_id], claim_time)
        .await
        .expect("enqueue initial refresh");
    let claims = repository
        .claim_refresh_batch(claim_time, Duration::seconds(30), 10)
        .await
        .expect("claim initial refresh");
    assert_eq!(claims.len(), 1);
    let claim = claims[0];
    assert_eq!(claim.viewer_id, viewer_id);
    assert_eq!(claim.claimed_at, claim_time);

    let event = hide_event(viewer_id);
    seed_impression(&app.pool, &event).await;
    assert!(app
        .state
        .record_recommendation_event
        .execute(event.clone())
        .await
        .expect("persist negative event while initial claim is processing"));
    repository
        .complete_refresh(claim)
        .await
        .expect("complete the initial worker claim");

    assert_eq!(
        refresh_job_count(&app.pool, viewer_id).await,
        1,
        "completion of the old claim must not erase newer queued work"
    );
    assert_eq!(event_count(&app.pool, event.event_id).await, 1);
    let status: String = sqlx::query_scalar(
        "select status from people_recommendation_refresh_jobs where viewer_id=$1",
    )
    .bind(viewer_id)
    .fetch_one(&app.pool)
    .await
    .expect("newer refresh remains queryable");
    assert_eq!(status, "PENDING");
    app.shutdown().await.expect("fixture shutdown");
}

#[tokio::test]
async fn recommendation_refresh_enqueue_survives_stale_worker_failure() {
    let app = start_postgres_app(false)
        .await
        .expect("start a real PostgreSQL FriendService app");
    let repository = PgPeopleRecommendationRepository::new(app.pool.clone());
    let viewer_id = Uuid::new_v4();
    let claim_time = Utc::now();

    repository
        .enqueue_refresh(&[viewer_id], claim_time)
        .await
        .expect("enqueue initial refresh");
    let claims = repository
        .claim_refresh_batch(claim_time, Duration::seconds(30), 10)
        .await
        .expect("claim initial refresh");
    assert_eq!(claims.len(), 1);
    let claim = claims[0];
    assert_eq!(claim.viewer_id, viewer_id);
    assert_eq!(claim.claimed_at, claim_time);

    repository
        .enqueue_refresh(&[viewer_id], claim_time + Duration::seconds(1))
        .await
        .expect("enqueue newer refresh while initial claim is processing");
    repository
        .fail_refresh(
            claim,
            "stale worker failure",
            claim_time + Duration::seconds(60),
            1,
        )
        .await
        .expect("ignore failure from the superseded claim");

    let (status, attempt_count): (String, i32) = sqlx::query_as(
        "select status, attempt_count from people_recommendation_refresh_jobs where viewer_id=$1",
    )
    .bind(viewer_id)
    .fetch_one(&app.pool)
    .await
    .expect("newer refresh remains queryable");
    assert_eq!(status, "PENDING");
    assert_eq!(
        attempt_count, 0,
        "stale failure must not consume retry budget"
    );
    app.shutdown().await.expect("fixture shutdown");
}
