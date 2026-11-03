use chrono::{DateTime, Utc};
use profile_service::application::dto::profile_dto::{
    DeleteProfileDto, PurgeExpiredProfilesDto, UpdateProfileDto,
};
use uuid::Uuid;

type SoftDeleteRow = (
    Option<DateTime<Utc>>,
    Option<Uuid>,
    Option<String>,
    Option<DateTime<Utc>>,
);

#[tokio::test]
async fn postgres_backed_profile_flow_omits_missing_and_soft_deleted_profiles_from_batches() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");

    let created = app
        .state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: "11111111-1111-1111-1111-111111111111".into(),
            username: "hana_ported".into(),
            nickname: "Hana Ported".into(),
            avatar_url: "/avatars/ported.svg".into(),
            bio: "Porting profile flows".into(),
        })
        .await
        .expect("update");
    assert_eq!(created.username, "hana_ported");

    let surviving = app
        .state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: "22222222-2222-2222-2222-222222222222".into(),
            username: "batch_survivor".into(),
            nickname: "Batch Survivor".into(),
            avatar_url: "/avatars/batch-survivor.svg".into(),
            bio: "Survives batch lookup".into(),
        })
        .await
        .expect("create surviving batch profile");
    assert_eq!(surviving.username, "batch_survivor");

    let soft_deleted_id = Uuid::parse_str("33333333-3333-3333-3333-333333333333").expect("uuid");
    app.state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: soft_deleted_id.to_string(),
            username: "batch_deleted".into(),
            nickname: "Batch Deleted".into(),
            avatar_url: "/avatars/batch-deleted.svg".into(),
            bio: "Omitted deleted profile".into(),
        })
        .await
        .expect("create soft-deleted batch profile");
    app.state
        .delete_profile
        .execute(DeleteProfileDto {
            user_id: soft_deleted_id.to_string(),
            actor_id: soft_deleted_id.to_string(),
            request_id: Some("req-batch-omission-delete-1".into()),
            ip_address: Some("203.0.113.30".into()),
            user_agent: Some("profile-batch-omission-test".into()),
        })
        .await
        .expect("soft delete batch profile");

    let by_username = app
        .state
        .get_profile_by_username
        .execute("hana_ported")
        .await
        .expect("by username");
    assert_eq!(by_username.nickname, "Hana Ported");

    let search = app
        .state
        .search_profiles
        .execute("porting", None, 10)
        .await
        .expect("search");
    assert_eq!(search.profiles.len(), 1);

    let batch = app
        .state
        .get_profiles_batch
        .execute(&[
            "22222222-2222-2222-2222-222222222222"
                .parse()
                .expect("uuid"),
            "bbbbbbbb-bbbb-bbbb-bbbb-bbbbbbbbbbbb"
                .parse()
                .expect("uuid"),
            "11111111-1111-1111-1111-111111111111"
                .parse()
                .expect("uuid"),
            "22222222-2222-2222-2222-222222222222"
                .parse()
                .expect("uuid"),
        ])
        .await
        .expect("batch");
    assert_eq!(
        batch
            .iter()
            .map(|profile| profile.user_id.as_str())
            .collect::<Vec<_>>(),
        vec![
            "22222222-2222-2222-2222-222222222222",
            "11111111-1111-1111-1111-111111111111",
        ]
    );

    let deleted_batch = app
        .state
        .get_profiles_batch
        .execute(&[soft_deleted_id])
        .await
        .expect("soft-deleted batch");
    assert!(deleted_batch.is_empty());

    let deleted_by_username = app
        .state
        .get_profile_by_username
        .execute("batch_deleted")
        .await
        .expect_err("soft-deleted profile should be hidden by username");
    assert_eq!(deleted_by_username.to_string(), "Profile not found");

    let deleted_search = app
        .state
        .search_profiles
        .execute("omitted deleted", None, 10)
        .await
        .expect("search");
    assert!(deleted_search.profiles.is_empty());

    let uploaded = app
        .state
        .upload_avatar
        .execute(
            "11111111-1111-1111-1111-111111111111",
            crate::support::webp_fixture(),
            "image/webp",
            "avatar.webp",
        )
        .await
        .expect("upload");
    assert_eq!(uploaded.user_id, "11111111-1111-1111-1111-111111111111");

    let avatar = app
        .state
        .get_avatar
        .execute("11111111-1111-1111-1111-111111111111")
        .await
        .expect("avatar")
        .expect("stored avatar");
    assert_eq!(avatar.content, crate::support::webp_fixture());
    assert_eq!(avatar.content_type, "image/webp");
}

#[tokio::test]
async fn deleting_profile_soft_deletes_profile_avatar_records_audit_and_hides_reads() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let user_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid");

    app.state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: user_id.to_string(),
            username: "profile_delete_probe".into(),
            nickname: "Delete Probe".into(),
            avatar_url: "/avatars/delete-probe.svg".into(),
            bio: "Profile soft delete probe".into(),
        })
        .await
        .expect("profile");
    app.state
        .upload_avatar
        .execute(
            &user_id.to_string(),
            crate::support::png_fixture(),
            "image/png",
            "profile.png",
        )
        .await
        .expect("avatar");

    let deleted = app
        .state
        .delete_profile
        .execute(DeleteProfileDto {
            user_id: user_id.to_string(),
            actor_id: user_id.to_string(),
            request_id: Some("req-profile-delete-1".into()),
            ip_address: Some("203.0.113.30".into()),
            user_agent: Some("profile-soft-delete-test".into()),
        })
        .await
        .expect("delete profile");
    assert!(deleted.deleted);
    assert!(!deleted.account_deleted);

    let profile_row: SoftDeleteRow = sqlx::query_as(
        r#"
        select deleted_at, deleted_by, delete_reason, retention_until
        from profiles
        where user_id = $1
        "#,
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .expect("profile row");
    let deleted_at = profile_row.0.expect("profile deleted_at");
    assert_eq!(profile_row.1, Some(user_id));
    assert_eq!(profile_row.2.as_deref(), Some("user_request"));
    assert!(profile_row.3.expect("profile retention") > deleted_at);

    let avatar_deleted_at: Option<DateTime<Utc>> =
        sqlx::query_scalar("select deleted_at from avatars where user_id = $1")
            .bind(user_id)
            .fetch_one(&app.pool)
            .await
            .expect("avatar row");
    assert!(avatar_deleted_at.is_some());

    let audit_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from profile_audit_events
        where profile_user_id = $1
          and actor_id = $1
          and action = 'PROFILE_DELETED'
          and outcome = 'SUCCESS'
          and request_id = 'req-profile-delete-1'
          and ip_address = '203.0.113.30'
          and user_agent = 'profile-soft-delete-test'
        "#,
    )
    .bind(user_id)
    .fetch_one(&app.pool)
    .await
    .expect("audit count");
    assert_eq!(audit_count, 1);

    let by_username = app
        .state
        .get_profile_by_username
        .execute("profile_delete_probe")
        .await
        .expect_err("deleted username should be hidden");
    assert_eq!(by_username.to_string(), "Profile not found");

    let search = app
        .state
        .search_profiles
        .execute("delete probe", None, 10)
        .await
        .expect("search");
    assert!(search.profiles.is_empty());

    let batch = app
        .state
        .get_profiles_batch
        .execute(&[user_id])
        .await
        .expect("batch");
    assert!(batch.is_empty());

    let avatar = app
        .state
        .get_avatar
        .execute(&user_id.to_string())
        .await
        .expect("avatar");
    assert!(avatar.is_none());
}

#[tokio::test]
async fn purging_expired_profile_retention_hard_deletes_only_expired_rows_and_records_audit() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let expired_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("uuid");
    let retained_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("uuid");
    let operator_id = Uuid::parse_str("99999999-9999-9999-9999-999999999999").expect("uuid");

    for (user_id, username) in [
        (expired_id, "expired_retention"),
        (retained_id, "retained_retention"),
    ] {
        app.state
            .update_profile
            .execute(UpdateProfileDto {
                user_id: user_id.to_string(),
                username: username.into(),
                nickname: username.into(),
                avatar_url: format!("/avatars/{username}.png"),
                bio: "Retention purge probe".into(),
            })
            .await
            .expect("profile");
        app.state
            .upload_avatar
            .execute(
                &user_id.to_string(),
                crate::support::png_fixture(),
                "image/png",
                "retention.png",
            )
            .await
            .expect("avatar");
        app.state
            .delete_profile
            .execute(DeleteProfileDto {
                user_id: user_id.to_string(),
                actor_id: user_id.to_string(),
                request_id: Some(format!("req-delete-{username}")),
                ip_address: Some("203.0.113.40".into()),
                user_agent: Some("profile-retention-test".into()),
            })
            .await
            .expect("delete profile");
    }

    sqlx::query(
        r#"
        update profiles
        set retention_until = now() - interval '1 day'
        where user_id = $1
        "#,
    )
    .bind(expired_id)
    .execute(&app.pool)
    .await
    .expect("expire profile");
    sqlx::query(
        r#"
        update avatars
        set retention_until = now() - interval '1 day'
        where user_id = $1
        "#,
    )
    .bind(expired_id)
    .execute(&app.pool)
    .await
    .expect("expire avatar");

    let purged = app
        .state
        .purge_expired_profiles
        .execute(PurgeExpiredProfilesDto {
            actor_id: operator_id.to_string(),
            request_id: Some("req-profile-retention-purge-1".into()),
            ip_address: Some("203.0.113.41".into()),
            user_agent: Some("profile-retention-purge-test".into()),
            dry_run: false,
        })
        .await
        .expect("purge profiles");
    assert_eq!(purged.purged_count, 1);

    let expired_profile_count: i64 =
        sqlx::query_scalar("select count(*) from profiles where user_id = $1")
            .bind(expired_id)
            .fetch_one(&app.pool)
            .await
            .expect("expired profile count");
    assert_eq!(expired_profile_count, 0);
    let expired_avatar_count: i64 =
        sqlx::query_scalar("select count(*) from avatars where user_id = $1")
            .bind(expired_id)
            .fetch_one(&app.pool)
            .await
            .expect("expired avatar count");
    assert_eq!(expired_avatar_count, 0);

    let retained_profile_count: i64 =
        sqlx::query_scalar("select count(*) from profiles where user_id = $1")
            .bind(retained_id)
            .fetch_one(&app.pool)
            .await
            .expect("retained profile count");
    assert_eq!(retained_profile_count, 1);

    let audit_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from profile_audit_events
        where profile_user_id = $1
          and actor_id = $2
          and action = 'PROFILE_PURGED'
          and outcome = 'SUCCESS'
          and request_id = 'req-profile-retention-purge-1'
          and ip_address = '203.0.113.41'
          and user_agent = 'profile-retention-purge-test'
        "#,
    )
    .bind(expired_id)
    .bind(operator_id)
    .fetch_one(&app.pool)
    .await
    .expect("purge audit count");
    assert_eq!(audit_count, 1);
}

#[tokio::test]
async fn dry_run_profile_retention_purge_counts_expired_rows_without_deleting_and_records_audit() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let user_id = Uuid::parse_str("33333333-3333-3333-3333-333333333333").expect("uuid");
    let operator_id = Uuid::parse_str("99999999-9999-9999-9999-999999999999").expect("uuid");

    app.state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: user_id.to_string(),
            username: "dry_run_profile".into(),
            nickname: "Dry Run Profile".into(),
            avatar_url: "/avatars/dry-run-profile.png".into(),
            bio: "Retention dry-run probe".into(),
        })
        .await
        .expect("profile");
    app.state
        .delete_profile
        .execute(DeleteProfileDto {
            user_id: user_id.to_string(),
            actor_id: user_id.to_string(),
            request_id: Some("req-delete-profile-dry-run".into()),
            ip_address: Some("203.0.113.42".into()),
            user_agent: Some("profile-retention-dry-run-test".into()),
        })
        .await
        .expect("delete profile");
    sqlx::query(
        r#"
        update profiles
        set retention_until = now() - interval '1 day'
        where user_id = $1
        "#,
    )
    .bind(user_id)
    .execute(&app.pool)
    .await
    .expect("expire profile");

    let purged = app
        .state
        .purge_expired_profiles
        .execute(PurgeExpiredProfilesDto {
            actor_id: operator_id.to_string(),
            request_id: Some("req-profile-retention-dry-run-1".into()),
            ip_address: Some("203.0.113.43".into()),
            user_agent: Some("profile-retention-dry-run-test".into()),
            dry_run: true,
        })
        .await
        .expect("dry run purge profiles");
    assert_eq!(purged.purged_count, 1);

    let profile_count: i64 = sqlx::query_scalar("select count(*) from profiles where user_id = $1")
        .bind(user_id)
        .fetch_one(&app.pool)
        .await
        .expect("profile count");
    assert_eq!(profile_count, 1);

    let audit_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from profile_audit_events
        where profile_user_id = $1
          and actor_id = $2
          and action = 'PROFILE_PURGED'
          and outcome = 'DRY_RUN'
          and request_id = 'req-profile-retention-dry-run-1'
          and ip_address = '203.0.113.43'
          and user_agent = 'profile-retention-dry-run-test'
        "#,
    )
    .bind(user_id)
    .bind(operator_id)
    .fetch_one(&app.pool)
    .await
    .expect("dry-run audit count");
    assert_eq!(audit_count, 1);
}

#[tokio::test]
async fn retention_purge_worker_hard_deletes_expired_profiles_and_records_audit() {
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");
    let user_id = Uuid::parse_str("44444444-4444-4444-4444-444444444444").expect("uuid");
    let operator_id = Uuid::parse_str("99999999-9999-9999-9999-999999999999").expect("uuid");

    app.state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: user_id.to_string(),
            username: "worker_retention".into(),
            nickname: "Worker Retention".into(),
            avatar_url: "/avatars/worker-retention.png".into(),
            bio: "Profile retention worker probe".into(),
        })
        .await
        .expect("profile");
    app.state
        .upload_avatar
        .execute(
            &user_id.to_string(),
            crate::support::png_fixture(),
            "image/png",
            "worker.png",
        )
        .await
        .expect("avatar");
    app.state
        .delete_profile
        .execute(DeleteProfileDto {
            user_id: user_id.to_string(),
            actor_id: user_id.to_string(),
            request_id: Some("req-worker-profile-delete".into()),
            ip_address: Some("203.0.113.80".into()),
            user_agent: Some("profile-retention-worker-test".into()),
        })
        .await
        .expect("delete profile");
    sqlx::query(
        r#"
        update profiles
        set retention_until = now() - interval '1 day'
        where user_id = $1
        "#,
    )
    .bind(user_id)
    .execute(&app.pool)
    .await
    .expect("expire profile");
    sqlx::query(
        r#"
        update avatars
        set retention_until = now() - interval '1 day'
        where user_id = $1
        "#,
    )
    .bind(user_id)
    .execute(&app.pool)
    .await
    .expect("expire avatar");

    let purged = app
        .state
        .retention_purge_worker
        .tick()
        .await
        .expect("retention worker tick");

    assert_eq!(purged, 1);
    let profile_count: i64 = sqlx::query_scalar("select count(*) from profiles where user_id = $1")
        .bind(user_id)
        .fetch_one(&app.pool)
        .await
        .expect("profile count");
    assert_eq!(profile_count, 0);
    let avatar_count: i64 = sqlx::query_scalar("select count(*) from avatars where user_id = $1")
        .bind(user_id)
        .fetch_one(&app.pool)
        .await
        .expect("avatar count");
    assert_eq!(avatar_count, 0);
    let audit_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from profile_audit_events
        where profile_user_id = $1
          and actor_id = $2
          and action = 'PROFILE_PURGED'
          and outcome = 'SUCCESS'
          and request_id = 'profile-retention-worker'
          and user_agent = 'profile-retention-worker'
        "#,
    )
    .bind(user_id)
    .bind(operator_id)
    .fetch_one(&app.pool)
    .await
    .expect("audit count");
    assert_eq!(audit_count, 1);
    let metrics = app.metrics().await.expect("metrics");
    assert!(metrics.contains("profile_retention_purge_runs_total{outcome=\"success\"} 1"));
    assert!(metrics.contains("profile_retention_purged_profiles_total{outcome=\"success\"} 1"));
}

#[tokio::test]
async fn a_soft_deleted_username_cannot_be_reclaimed_by_another_user() {
    // Deleting a profile hides it but keeps the row, and the username column has a
    // plain unique constraint. So the name stays reserved after deletion, which is
    // the intent. The risk is that the reservation is enforced only by the index:
    // the application pre-check skips deleted rows, so a second user editing their
    // profile to that name would reach the insert and receive a raw database error
    // instead of a clean conflict.
    let app = crate::support::start_postgres_app(false)
        .await
        .expect("app");

    let owner = "44444444-4444-4444-4444-444444444444";
    app.state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: owner.into(),
            username: "reserved_name".into(),
            nickname: "Owner".into(),
            avatar_url: "/avatars/owner.svg".into(),
            bio: "Owns the name".into(),
        })
        .await
        .expect("create owner");

    app.state
        .delete_profile
        .execute(DeleteProfileDto {
            user_id: owner.into(),
            actor_id: owner.into(),
            request_id: Some("req-1".into()),
            ip_address: Some("127.0.0.1".into()),
            user_agent: Some("test".into()),
        })
        .await
        .expect("delete owner profile");

    // When: a different user tries to take the deleted name.
    let outcome = app
        .state
        .update_profile
        .execute(UpdateProfileDto {
            user_id: "55555555-5555-5555-5555-555555555555".into(),
            username: "reserved_name".into(),
            nickname: "Claimant".into(),
            avatar_url: "/avatars/claimant.svg".into(),
            bio: "Wants the name".into(),
        })
        .await;

    // Then: the name is not handed over, and the refusal is a deliberate one that
    // the Gateway can map to a conflict rather than an unexpected server error.
    let error = outcome.expect_err("a deleted name must stay reserved");
    let message = error.to_string();
    assert!(
        message.contains("Username already taken"),
        "the refusal must be a deliberate conflict, got: {message}"
    );
    assert!(
        !message.contains("duplicate key"),
        "the refusal must not surface a raw database error, got: {message}"
    );
}
