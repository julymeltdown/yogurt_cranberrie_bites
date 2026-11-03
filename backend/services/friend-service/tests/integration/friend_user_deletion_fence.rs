use friend_service::adapters::r#in::grpc::server::proto;
use friend_service::adapters::r#in::grpc::server::proto::friend_service_server::FriendService;
use friend_service::application::dto::friend_dto::ApplyUserDeletionDto;
use platform::grpc_auth::{VerifiedCaller, Workload};
use sqlx::PgPool;
use std::time::Duration;
use tonic::{Code, Request};
use uuid::Uuid;

const DELETED_USER: &str = "11111111-1111-4111-8111-111111111111";
const OTHER_USER: &str = "22222222-2222-4222-8222-222222222222";
const SAGA_ID: &str = "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa";
const ACTOR_ID: &str = "33333333-3333-4333-8333-333333333333";

#[tokio::test]
async fn deletion_fence_rejects_stale_mutations_and_late_recommendation_events() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("PostgreSQL app");
    let deleted_user = Uuid::parse_str(DELETED_USER).expect("deleted user UUID");
    let other_user = Uuid::parse_str(OTHER_USER).expect("other user UUID");

    // Seed all four relationship shapes before deletion. The account saga must
    // preserve their soft-delete audit state while leaving a permanent fence.
    sqlx::query("insert into follows (follower_id, followee_id) values ($1, $2)")
        .bind(deleted_user)
        .bind(other_user)
        .execute(&app.pool)
        .await
        .expect("seed follow");
    sqlx::query(
        "insert into follow_requests (id, follower_id, followee_id, status, created_at) values ($1, $2, $3, 'PENDING', now())",
    )
    .bind(Uuid::new_v4())
    .bind(deleted_user)
    .bind(other_user)
    .execute(&app.pool)
    .await
    .expect("seed request");
    sqlx::query(
        "insert into user_blocks (blocker_id, blocked_id, created_at) values ($1, $2, now())",
    )
    .bind(deleted_user)
    .bind(other_user)
    .execute(&app.pool)
    .await
    .expect("seed block");
    sqlx::query("insert into user_mutes (muter_id, muted_id, created_at) values ($1, $2, now())")
        .bind(deleted_user)
        .bind(other_user)
        .execute(&app.pool)
        .await
        .expect("seed mute");

    let deletion = ApplyUserDeletionDto {
        saga_id: SAGA_ID.into(),
        user_id: DELETED_USER.into(),
        actor_id: ACTOR_ID.into(),
        retention_until: "2027-01-01T00:00:00Z".into(),
    };
    app.state
        .apply_user_deletion
        .execute(deletion.clone())
        .await
        .expect("apply deletion");
    app.state
        .apply_user_deletion
        .execute(deletion)
        .await
        .expect("exact saga retry is idempotent");

    let receipt: (Uuid, Uuid, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
        "select saga_id, actor_id, retention_until from friend_user_deletion_tombstones where user_id=$1",
    )
    .bind(deleted_user)
    .fetch_one(&app.pool)
    .await
    .expect("permanent tombstone");
    assert_eq!(receipt.0, Uuid::parse_str(SAGA_ID).expect("saga UUID"));
    assert_eq!(receipt.1, Uuid::parse_str(ACTOR_ID).expect("actor UUID"));
    assert_eq!(receipt.2.to_rfc3339(), "2027-01-01T00:00:00+00:00");

    let active_relationships: i64 = sqlx::query_scalar(
        r#"select
            (select count(*) from follows where (follower_id=$1 or followee_id=$1) and deleted_at is null) +
            (select count(*) from follow_requests where (follower_id=$1 or followee_id=$1) and deleted_at is null) +
            (select count(*) from user_blocks where (blocker_id=$1 or blocked_id=$1) and deleted_at is null) +
            (select count(*) from user_mutes where (muter_id=$1 or muted_id=$1) and deleted_at is null)"#,
    )
    .bind(deleted_user)
    .fetch_one(&app.pool)
    .await
    .expect("active relationship count");
    assert_eq!(active_relationships, 0);

    // Exercise the actual service entry point and verify the SQL fence maps to
    // a stable client-facing denial rather than an internal server error.
    let grpc = crate::support::grpc_server(app.state.clone());
    for (follower_id, followee_id) in [
        (DELETED_USER, "44444444-4444-4444-8444-444444444444"),
        ("55555555-5555-4555-8555-555555555555", DELETED_USER),
    ] {
        let mut request = Request::new(proto::FollowUserRequest {
            follower_id: follower_id.into(),
            followee_id: followee_id.into(),
        });
        request
            .extensions_mut()
            .insert(VerifiedCaller(Workload::Gateway));
        let error = FriendService::follow_user(&grpc, request)
            .await
            .expect_err("deleted actor or target must not create a relationship");
        assert_eq!(error.code(), Code::PermissionDenied);
        assert_eq!(error.message(), "Account is deleted");
    }

    assert_fenced(
        sqlx::query("insert into follows (follower_id, followee_id) values ($1, $2)")
            .bind(deleted_user)
            .bind(other_user),
        &app.pool,
        "late follow",
    )
    .await;
    assert_fenced(
        sqlx::query(
            "insert into follow_requests (id, follower_id, followee_id, status, created_at) values ($1, $2, $3, 'PENDING', now())",
        )
        .bind(Uuid::new_v4())
        .bind(other_user)
        .bind(deleted_user),
        &app.pool,
        "late request",
    )
    .await;
    assert_fenced(
        sqlx::query(
            "insert into user_blocks (blocker_id, blocked_id, created_at) values ($1, $2, now())",
        )
        .bind(other_user)
        .bind(deleted_user),
        &app.pool,
        "late block",
    )
    .await;
    assert_fenced(
        sqlx::query(
            "insert into user_mutes (muter_id, muted_id, created_at) values ($1, $2, now())",
        )
        .bind(deleted_user)
        .bind(other_user),
        &app.pool,
        "late mute",
    )
    .await;
    assert_fenced(
        sqlx::query(
            "insert into people_recommendation_events (event_id, schema_version, occurred_at, viewer_id, subject_type, subject_id, actor_id, surface, action, source, model_version) values ($1, 1, now(), $2, 'USER', $3, $4, 'PEOPLE', 'LIKE', 'test', 'test')",
        )
        .bind(Uuid::new_v4())
        .bind(other_user)
        .bind(other_user)
        .bind(deleted_user),
        &app.pool,
        "late actor event",
    )
    .await;
    app.shutdown().await.expect("fixture shutdown");
}

#[tokio::test]
async fn deletion_tombstone_rejects_conflicting_saga_replay_without_rewriting_it() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("PostgreSQL app");
    let deletion = ApplyUserDeletionDto {
        saga_id: SAGA_ID.into(),
        user_id: DELETED_USER.into(),
        actor_id: ACTOR_ID.into(),
        retention_until: "2027-01-01T00:00:00Z".into(),
    };
    app.state
        .apply_user_deletion
        .execute(deletion.clone())
        .await
        .expect("apply deletion");

    let mut conflicting_retry = deletion;
    conflicting_retry.actor_id = OTHER_USER.into();
    let error = app
        .state
        .apply_user_deletion
        .execute(conflicting_retry)
        .await
        .expect_err("same tombstone cannot accept altered saga data");
    assert!(error.to_string().contains("Friend deletion identity"));

    let stored_actor: Uuid =
        sqlx::query_scalar("select actor_id from friend_user_deletion_tombstones where user_id=$1")
            .bind(Uuid::parse_str(DELETED_USER).expect("deleted user UUID"))
            .fetch_one(&app.pool)
            .await
            .expect("original tombstone remains");
    assert_eq!(stored_actor, Uuid::parse_str(ACTOR_ID).expect("actor UUID"));
    app.shutdown().await.expect("fixture shutdown");
}

#[tokio::test]
async fn relationship_write_and_deletion_serialize_around_the_tombstone() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("PostgreSQL app");
    let user_id = Uuid::parse_str(DELETED_USER).expect("deleted user UUID");
    let other_id = Uuid::parse_str(OTHER_USER).expect("other user UUID");

    // If a relationship write owns the shared subject lock first, deletion
    // waits, then tombstones the account and removes the just-committed edge.
    let mut mutation_tx = app.pool.begin().await.expect("begin mutation transaction");
    sqlx::query("select friend_assert_subjects_not_deleted($1::uuid[])")
        .bind(vec![user_id, other_id])
        .execute(&mut *mutation_tx)
        .await
        .expect("claim the mutation fence");
    sqlx::query("insert into follows (follower_id, followee_id) values ($1, $2)")
        .bind(user_id)
        .bind(other_id)
        .execute(&mut *mutation_tx)
        .await
        .expect("write while holding the mutation fence");

    let state = app.state.clone();
    let deletion_task = tokio::spawn(async move {
        state
            .apply_user_deletion
            .execute(ApplyUserDeletionDto {
                saga_id: "bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb".into(),
                user_id: DELETED_USER.into(),
                actor_id: ACTOR_ID.into(),
                retention_until: "2027-02-01T00:00:00Z".into(),
            })
            .await
    });
    wait_for_lock_wait(&app.pool, "select friend_lock_subject_ids").await;
    mutation_tx.commit().await.expect("commit the first writer");
    deletion_task
        .await
        .expect("deletion task")
        .expect("deletion after the first writer");

    let active_after_deletion: i64 = sqlx::query_scalar(
        "select count(*) from follows where follower_id=$1 and followee_id=$2 and deleted_at is null",
    )
    .bind(user_id)
    .bind(other_id)
    .fetch_one(&app.pool)
    .await
    .expect("count active relationship");
    assert_eq!(active_after_deletion, 0);

    // If deletion owns the subject lock first, a delayed mutation waits and
    // then observes the committed tombstone instead of recreating the edge.
    let mut deletion_tx = app.pool.begin().await.expect("begin deletion transaction");
    sqlx::query("select friend_lock_subject_ids($1::uuid[])")
        .bind(vec![other_id])
        .execute(&mut *deletion_tx)
        .await
        .expect("claim the deletion fence");
    sqlx::query(
        "insert into friend_user_deletion_tombstones (user_id, saga_id, actor_id, retention_until) values ($1, $2, $3, $4)",
    )
    .bind(other_id)
    .bind(Uuid::parse_str("cccccccc-cccc-4ccc-8ccc-cccccccccccc").expect("saga UUID"))
    .bind(Uuid::parse_str(ACTOR_ID).expect("actor UUID"))
    .bind(chrono::DateTime::parse_from_rfc3339("2027-03-01T00:00:00Z").expect("deadline"))
    .execute(&mut *deletion_tx)
    .await
    .expect("write the tombstone before releasing the fence");

    let pool = app.pool.clone();
    let delayed_write = tokio::spawn(async move {
        sqlx::query("insert into follows (follower_id, followee_id) values ($1, $2)")
            .bind(user_id)
            .bind(other_id)
            .execute(&pool)
            .await
    });
    wait_for_lock_wait(&app.pool, "insert into follows").await;
    deletion_tx.commit().await.expect("commit the tombstone");
    let error = delayed_write
        .await
        .expect("delayed mutation task")
        .expect_err("late mutation must observe the committed tombstone");
    let sqlx::Error::Database(database_error) = error else {
        panic!("late mutation failed for the wrong reason: {error}");
    };
    assert_eq!(
        database_error.constraint(),
        Some("friend_user_deletion_fence")
    );
    app.shutdown().await.expect("fixture shutdown");
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
        Some("friend_user_deletion_fence"),
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
    .expect("expected operation to wait on the account-deletion fence");
}
