use platform::grpc_auth::{VerifiedCaller, Workload};
use profile_service::adapters::r#in::grpc::server::proto;
use profile_service::adapters::r#in::grpc::server::proto::profile_service_server::ProfileService;
use profile_service::application::dto::profile_dto::{ApplyUserDeletionDto, UpdateProfileDto};
use sqlx::PgPool;
use std::time::Duration;
use tonic::{Code, Request};
use uuid::Uuid;

const USER_ID: &str = "11111111-1111-4111-8111-111111111111";
const ACTOR_ID: &str = "22222222-2222-4222-8222-222222222222";

#[tokio::test]
async fn deletion_fence_prevents_stale_profile_and_avatar_writes() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("PostgreSQL app");
    let user_id = Uuid::parse_str(USER_ID).expect("user UUID");

    app.state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: USER_ID.into(),
            username: "fenced_profile".into(),
            nickname: "Before deletion".into(),
            avatar_url: "/avatar/before.svg".into(),
            bio: "Profile before account deletion".into(),
        })
        .await
        .expect("create profile");
    app.state
        .upload_avatar
        .execute(
            USER_ID,
            crate::support::png_fixture(),
            "image/png",
            "before.png",
        )
        .await
        .expect("create avatar");

    let deletion = ApplyUserDeletionDto {
        saga_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(),
        user_id: USER_ID.into(),
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
        .expect("exact retry is idempotent");

    let deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("select deleted_at from profiles where user_id=$1")
            .bind(user_id)
            .fetch_one(&app.pool)
            .await
            .expect("soft-deleted profile");
    assert!(deleted_at.is_some());
    let avatar_deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("select deleted_at from avatars where user_id=$1")
            .bind(user_id)
            .fetch_one(&app.pool)
            .await
            .expect("soft-deleted avatar");
    assert!(avatar_deleted_at.is_some());

    // The actual gRPC mutation path returns a stable authorization failure,
    // not an invalid-input response or an internal database error.
    let grpc = crate::support::grpc_server(app.state.clone());
    let mut update = Request::new(proto::UpdateProfileRequest {
        user_id: USER_ID.into(),
        username: "fenced_profile_again".into(),
        nickname: "After deletion".into(),
        avatar_url: "/avatar/after.svg".into(),
        bio: "Must not be written".into(),
    });
    update
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Gateway));
    let error = ProfileService::update_profile(&grpc, update)
        .await
        .expect_err("stale Gateway mutation must be rejected");
    assert_eq!(error.code(), Code::PermissionDenied);
    assert_eq!(error.message(), "Account is deleted");

    let avatar_error = app
        .state
        .upload_avatar
        .execute(
            USER_ID,
            crate::support::webp_fixture(),
            "image/webp",
            "after.webp",
        )
        .await
        .expect_err("stale avatar upload must be rejected");
    assert_eq!(avatar_error.to_string(), "Profile not found");

    assert_fenced(
        sqlx::query(
            "insert into avatars (user_id, content, content_type, filename) values ($1, $2, 'image/png', 'late.png')",
        )
        .bind(user_id)
        .bind(vec![1_u8, 2, 3]),
        &app.pool,
        "avatar recreation",
    )
    .await;

    assert_fenced(
        sqlx::query(
            "insert into profiles (user_id, username, nickname, avatar_url, bio) values ($1, 'fenced_recreated', 'Again', '/again.svg', 'late')",
        )
        .bind(user_id),
        &app.pool,
        "profile recreation",
    )
    .await;
}

#[tokio::test]
async fn deletion_fence_rejects_conflicting_saga_replay() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("PostgreSQL app");
    let deletion = ApplyUserDeletionDto {
        saga_id: "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa".into(),
        user_id: USER_ID.into(),
        actor_id: ACTOR_ID.into(),
        retention_until: "2027-01-01T00:00:00Z".into(),
    };
    app.state
        .apply_user_deletion
        .execute(deletion.clone())
        .await
        .expect("apply deletion");
    let mut conflicting = deletion;
    conflicting.actor_id = "33333333-3333-4333-8333-333333333333".into();

    let error = app
        .state
        .apply_user_deletion
        .execute(conflicting)
        .await
        .expect_err("changed actor under the same saga must fail");
    assert!(error.to_string().contains("Profile deletion identity"));
}

#[tokio::test]
async fn in_flight_profile_write_losing_the_deletion_race_maps_to_permission_denied() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("PostgreSQL app");
    let user_id = Uuid::parse_str(USER_ID).expect("user UUID");
    let mut deletion_tx = app.pool.begin().await.expect("begin deletion transaction");
    sqlx::query("select profile_lock_subject_ids($1::uuid[])")
        .bind(vec![user_id])
        .execute(&mut *deletion_tx)
        .await
        .expect("claim profile deletion fence");

    let grpc = crate::support::grpc_server(app.state.clone());
    let update_task = tokio::spawn(async move {
        let mut request = Request::new(proto::UpdateProfileRequest {
            user_id: USER_ID.into(),
            username: "profile_race_late".into(),
            nickname: "Must be refused".into(),
            avatar_url: "/race.svg".into(),
            bio: "Deletion won".into(),
        });
        request
            .extensions_mut()
            .insert(VerifiedCaller(Workload::Gateway));
        ProfileService::update_profile(&grpc, request).await
    });
    wait_for_lock_wait(&app.pool, "insert into profiles").await;

    sqlx::query(
        "insert into profile_user_deletion_tombstones (user_id, saga_id, actor_id, retention_until) values ($1, $2, $3, $4)",
    )
    .bind(user_id)
    .bind(Uuid::parse_str("bbbbbbbb-bbbb-4bbb-8bbb-bbbbbbbbbbbb").expect("saga UUID"))
    .bind(Uuid::parse_str(ACTOR_ID).expect("actor UUID"))
    .bind(chrono::DateTime::parse_from_rfc3339("2027-02-01T00:00:00Z").expect("deadline"))
    .execute(&mut *deletion_tx)
    .await
    .expect("write the tombstone");
    deletion_tx.commit().await.expect("commit deletion fence");

    let error = update_task
        .await
        .expect("profile update task")
        .expect_err("late profile write must be rejected");
    assert_eq!(error.code(), Code::PermissionDenied);
    assert_eq!(error.message(), "Account is deleted");

    let profile_count: i64 = sqlx::query_scalar("select count(*) from profiles where user_id=$1")
        .bind(user_id)
        .fetch_one(&app.pool)
        .await
        .expect("check that no profile was recreated");
    assert_eq!(profile_count, 0);
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
        Some("profile_user_deletion_fence"),
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
    .expect("expected profile write to wait on the account-deletion fence");
}
