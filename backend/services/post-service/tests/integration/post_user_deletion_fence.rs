use platform::grpc_auth::{VerifiedCaller, Workload};
use post_service::adapters::r#in::grpc::server::proto;
use post_service::adapters::r#in::grpc::server::proto::post_service_server::PostService;
use post_service::application::dto::post_dto::{ApplyUserDeletionDto, CreatePostDto};
use sqlx::PgPool;
use std::time::Duration;
use tonic::{Code, Request};
use uuid::Uuid;

const USER_ID: &str = "11111111-1111-4111-8111-111111111111";
const OTHER_USER_ID: &str = "22222222-2222-4222-8222-222222222222";
const ACTOR_ID: &str = "33333333-3333-4333-8333-333333333333";
const SAGA_ID: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";

#[tokio::test]
async fn deletion_fence_rejects_stale_content_and_delayed_feed_writes() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("PostgreSQL/Redis app");
    let user_id = Uuid::parse_str(USER_ID).expect("user UUID");
    let other_user_id = Uuid::parse_str(OTHER_USER_ID).expect("other user UUID");
    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: USER_ID.into(),
            content: "post before account deletion".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("create original post");
    let post_id = Uuid::parse_str(&post.id).expect("created post UUID");

    sqlx::query(
        "insert into comments (id, post_id, author_id, content, created_at) values ($1, $2, $3, 'comment before deletion', now())",
    )
    .bind(Uuid::new_v4())
    .bind(post_id)
    .bind(user_id)
    .execute(&app.pool)
    .await
    .expect("seed comment");
    sqlx::query("insert into post_likes (post_id, user_id, created_at) values ($1, $2, now())")
        .bind(post_id)
        .bind(user_id)
        .execute(&app.pool)
        .await
        .expect("seed like");

    let deletion = ApplyUserDeletionDto {
        saga_id: SAGA_ID.into(),
        user_id: USER_ID.into(),
        actor_id: ACTOR_ID.into(),
        retention_until: "2027-01-01T00:00:00Z".into(),
    };
    app.state
        .apply_user_deletion
        .execute(deletion.clone())
        .await
        .expect("apply account deletion");
    app.state
        .apply_user_deletion
        .execute(deletion.clone())
        .await
        .expect("exact retry is idempotent");

    let receipt: (Uuid, Uuid) = sqlx::query_as(
        "select saga_id, actor_id from post_user_deletion_tombstones where user_id=$1",
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .expect("permanent deletion tombstone");
    assert_eq!(receipt.0, Uuid::parse_str(SAGA_ID).expect("saga UUID"));
    assert_eq!(receipt.1, Uuid::parse_str(ACTOR_ID).expect("actor UUID"));

    let active_content: i64 = sqlx::query_scalar(
        r#"select
            (select count(*) from posts where author_id=$1 and deleted_at is null) +
            (select count(*) from comments where author_id=$1 and deleted_at is null) +
            (select count(*) from post_likes where user_id=$1 and deleted_at is null)"#,
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .expect("count active user content");
    assert_eq!(active_content, 0);

    let grpc = crate::support::grpc_server(app.state.clone());
    let mut request = Request::new(proto::CreatePostRequest {
        author_id: USER_ID.into(),
        content: "stale access token post".into(),
        visibility: "PUBLIC".into(),
    });
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Gateway));
    let error = PostService::create_post(&grpc, request)
        .await
        .expect_err("stale user must not create a post");
    assert_eq!(error.code(), Code::PermissionDenied);
    assert_eq!(error.message(), "Account is deleted");

    assert_fenced(
        sqlx::query(
            "insert into posts (id, author_id, content, visibility, created_at) values ($1, $2, 'late post', 'PUBLIC', now())",
        )
        .bind(Uuid::new_v4())
        .bind(user_id),
        &app.pool,
        "post recreation",
    )
    .await;
    assert_fenced(
        sqlx::query(
            "insert into comments (id, post_id, author_id, content, created_at) values ($1, $2, $3, 'late comment', now())",
        )
        .bind(Uuid::new_v4())
        .bind(post_id)
        .bind(user_id),
        &app.pool,
        "comment recreation",
    )
    .await;
    assert_fenced(
        sqlx::query("insert into post_likes (post_id, user_id, created_at) values ($1, $2, now())")
            .bind(post_id)
            .bind(user_id),
        &app.pool,
        "like recreation",
    )
    .await;
    assert_fenced(
        sqlx::query(
            "insert into post_event_outbox (event_id, event_type, aggregate_id, actor_id, payload, occurred_at, created_at) values ($1, 'POST_CREATED', $2, $3, '{}', now(), now())",
        )
        .bind(Uuid::new_v4())
        .bind(post_id)
        .bind(user_id),
        &app.pool,
        "late outbox event",
    )
    .await;
    assert_fenced(
        sqlx::query(
            "insert into feed_fanout_jobs (job_id, event_id, post_id, author_id, post_created_at, next_attempt_at) values ($1, $2, $3, $4, now(), now())",
        )
        .bind(Uuid::new_v4())
        .bind(Uuid::new_v4())
        .bind(post_id)
        .bind(user_id),
        &app.pool,
        "delayed fanout job",
    )
    .await;
    assert_fenced(
        sqlx::query(
            "insert into home_timeline_entries (viewer_id, post_id, author_id, source, source_score, inserted_at, expires_at) values ($1, $2, $3, 'FOLLOWING', 1.0, now(), now() + interval '1 day')",
        )
        .bind(other_user_id)
        .bind(post_id)
        .bind(user_id),
        &app.pool,
        "delayed timeline item",
    )
    .await;
    assert_fenced(
        sqlx::query(
            "insert into feed_recommendation_events (event_id, schema_version, occurred_at, viewer_id, subject_type, subject_id, actor_id, surface, action, source, model_version) values ($1, 1, now(), $2, 'POST', $3, $4, 'HOME_FEED', 'LIKE', 'test', 'test')",
        )
        .bind(Uuid::new_v4())
        .bind(other_user_id)
        .bind(post_id)
        .bind(user_id),
        &app.pool,
        "late recommendation event",
    )
    .await;

    let mut conflicting_retry = deletion;
    conflicting_retry.actor_id = OTHER_USER_ID.into();
    let error = app
        .state
        .apply_user_deletion
        .execute(conflicting_retry)
        .await
        .expect_err("changed actor under one saga must fail");
    assert!(error.to_string().contains("Post deletion identity"));
}

#[tokio::test]
async fn in_flight_post_create_losing_deletion_race_is_denied() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("PostgreSQL/Redis app");
    let user_id = Uuid::parse_str(USER_ID).expect("user UUID");
    let mut deletion_tx = app.pool.begin().await.expect("begin deletion transaction");
    sqlx::query("select post_lock_subject_ids($1::uuid[])")
        .bind(vec![user_id])
        .execute(&mut *deletion_tx)
        .await
        .expect("claim post deletion fence");

    let grpc = crate::support::grpc_server(app.state.clone());
    let create_task = tokio::spawn(async move {
        let mut request = Request::new(proto::CreatePostRequest {
            author_id: USER_ID.into(),
            content: "create that loses the deletion race".into(),
            visibility: "PUBLIC".into(),
        });
        request
            .extensions_mut()
            .insert(VerifiedCaller(Workload::Gateway));
        PostService::create_post(&grpc, request).await
    });
    wait_for_lock_wait(&app.pool, "insert into posts").await;

    sqlx::query(
        "insert into post_user_deletion_tombstones (user_id, saga_id, actor_id, retention_until) values ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(Uuid::parse_str(SAGA_ID).expect("saga UUID"))
    .bind(Uuid::parse_str(ACTOR_ID).expect("actor UUID"))
    .bind(chrono::DateTime::parse_from_rfc3339("2027-01-01T00:00:00Z").expect("deadline"))
    .execute(&mut *deletion_tx)
    .await
    .expect("write tombstone");
    deletion_tx.commit().await.expect("commit tombstone");

    let error = create_task
        .await
        .expect("create task")
        .expect_err("post creation must observe the committed tombstone");
    assert_eq!(error.code(), Code::PermissionDenied);
    assert_eq!(error.message(), "Account is deleted");

    let post_count: i64 = sqlx::query_scalar("select count(*) from posts where author_id=$1")
        .bind(user_id)
        .fetch_one(&app.pool)
        .await
        .expect("count post rows");
    assert_eq!(post_count, 0);
}

async fn assert_fenced<'a>(
    query: sqlx::query::Query<'a, sqlx::Postgres, sqlx::postgres::PgArguments>,
    pool: &PgPool,
    label: &str,
) {
    let error = match query.execute(pool).await {
        Ok(_) => panic!("{label} unexpectedly crossed the deletion fence"),
        Err(error) => error,
    };
    let sqlx::Error::Database(database_error) = error else {
        panic!("{label} failed for the wrong reason: {error}");
    };
    assert_eq!(
        database_error.constraint(),
        Some("post_user_deletion_fence"),
        "{label} must fail at the deletion fence"
    );
}

async fn wait_for_lock_wait(pool: &PgPool, query_fragment: &str) {
    tokio::time::timeout(Duration::from_secs(3), async {
        loop {
            let waiting: bool = sqlx::query_scalar(
                r#"select exists (
                    select 1 from pg_stat_activity
                    where datname = current_database()
                      and wait_event_type = 'Lock'
                      and position($1 in lower(query)) > 0
                )"#,
            )
            .bind(query_fragment)
            .fetch_one(pool)
            .await
            .expect("inspect PostgreSQL lock wait");
            if waiting {
                break;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    })
    .await
    .expect("expected PostService write to wait on account-deletion fence");
}
