use chrono::Utc;
use post_service::adapters::out::persistence::event_outbox::PgEventOutbox;
use post_service::application::dto::post_dto::{
    CreateCommentDto, CreatePostDto, DeletePostDto, ReplayRequestDto,
    ResolveDeadLetterReplayRequestDto,
};
use post_service::domain::post::{DeadLetterReplayError, ReplayTarget};
use sqlx::Row;
use uuid::Uuid;

#[tokio::test]
async fn postgres_outbox_replay_checkpoints_external_without_completing_other_targets() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");

    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Replay me".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");

    let before: (i64,) = sqlx::query_as(
        "select count(*) from post_event_outbox where aggregate_id = $1 and published = false",
    )
    .bind(Uuid::parse_str(&post.id).expect("uuid"))
    .fetch_one(&app.pool)
    .await
    .expect("outbox count");
    assert_eq!(before.0, 1);

    let replayed = app
        .state
        .replay_events
        .execute(ReplayRequestDto {
            actor_id: Some("22222222-2222-2222-2222-222222222222".into()),
            event_type: Some("POST_CREATED".into()),
            attribute: None,
            from: None,
            to: None,
            published: Some(false),
            target: Some(ReplayTarget::External),
            limit: Some(10),
        })
        .await
        .expect("replay");
    assert_eq!(replayed.replayed_count, 1);

    let replay_attempts: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        "select attempt_id, source, target, phase from post_event_delivery_attempts where event_id = (select event_id from post_event_outbox where aggregate_id = $1)",
    )
    .bind(Uuid::parse_str(&post.id).expect("post id"))
    .fetch_all(&app.pool)
    .await
    .expect("replay attempt history");
    assert_eq!(replay_attempts.len(), 2);
    assert_eq!(replay_attempts[0].0, replay_attempts[1].0);
    assert_eq!(replay_attempts[0].1, "ADMIN_REPLAY");
    assert_eq!(replay_attempts[0].2, "EXTERNAL");
    assert_eq!(replay_attempts[0].3, "STARTED");
    assert_eq!(replay_attempts[1].3, "SUCCEEDED");

    let publications: (i64,) =
        sqlx::query_as("select count(*) from event_publications where event_id in (select event_id from post_event_outbox where aggregate_id = $1)")
            .bind(Uuid::parse_str(&post.id).expect("uuid"))
            .fetch_one(&app.pool)
            .await
            .expect("publications");
    assert_eq!(publications.0, 1);

    let published: (bool,) =
        sqlx::query_as("select published from post_event_outbox where aggregate_id = $1")
            .bind(Uuid::parse_str(&post.id).expect("uuid"))
            .fetch_one(&app.pool)
            .await
            .expect("published");
    assert!(!published.0);

    let forwarded = app
        .outbox_forwarder()
        .republish_unpublished(Utc::now(), 10)
        .await
        .expect("complete the remaining required targets");
    assert_eq!(forwarded, 1);

    let attempt_totals: (i64, i64, i64) = sqlx::query_as(
        r#"
        select count(*),
               count(*) filter (where source = 'ADMIN_REPLAY'),
               count(*) filter (where source = 'OUTBOX_WORKER')
        from post_event_delivery_attempts
        where event_id = (select event_id from post_event_outbox where aggregate_id = $1)
        "#,
    )
    .bind(Uuid::parse_str(&post.id).expect("post id"))
    .fetch_one(&app.pool)
    .await
    .expect("combined attempt history");
    assert_eq!(attempt_totals, (8, 2, 6));

    let update_attempt_history = sqlx::query(
        "update post_event_delivery_attempts set phase = 'FAILED' where attempt_id = $1 and phase = 'SUCCEEDED'",
    )
    .bind(replay_attempts[1].0)
    .execute(&app.pool)
    .await;
    assert!(
        update_attempt_history.is_err(),
        "attempt outcome history must be immutable"
    );
    let delete_attempt_history =
        sqlx::query("delete from post_event_delivery_attempts where attempt_id = $1")
            .bind(replay_attempts[1].0)
            .execute(&app.pool)
            .await;
    assert!(
        delete_attempt_history.is_err(),
        "attempt history deletion must wait for an explicit retention policy"
    );
    let contradictory_terminal = sqlx::query(
        r#"
        insert into post_event_delivery_attempts (
            attempt_id, event_id, target, source, phase, occurred_at
        )
        select $1, event_id, 'EXTERNAL', 'ADMIN_REPLAY', 'FAILED', now()
        from post_event_outbox
        where aggregate_id = $2 and event_type = 'POST_CREATED'
        "#,
    )
    .bind(replay_attempts[1].0)
    .bind(Uuid::parse_str(&post.id).expect("post id"))
    .execute(&app.pool)
    .await;
    assert!(
        contradictory_terminal.is_err(),
        "an attempt may have no more than one terminal outcome"
    );

    let delete_with_attempt_history = sqlx::query(
        "delete from post_event_outbox where aggregate_id = $1 and event_type = 'POST_CREATED'",
    )
    .bind(Uuid::parse_str(&post.id).expect("post id"))
    .execute(&app.pool)
    .await;
    assert!(
        delete_with_attempt_history.is_err(),
        "outbox retention must explicitly preserve or expire the attempt history"
    );

    let published: (bool,) =
        sqlx::query_as("select published from post_event_outbox where aggregate_id = $1")
            .bind(Uuid::parse_str(&post.id).expect("uuid"))
            .fetch_one(&app.pool)
            .await
            .expect("published after all targets");
    assert!(published.0);
}

#[tokio::test]
async fn create_post_rolls_back_post_row_when_outbox_write_fails() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    sqlx::query(
        "alter table post_event_outbox add constraint reject_post_created_events check (event_type <> 'POST_CREATED')",
    )
    .execute(&app.pool)
    .await
    .expect("constraint");

    let result = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Atomic outbox rollback probe".into(),
            visibility: "PUBLIC".into(),
        })
        .await;

    assert!(result.is_err());
    let post_count: i64 = sqlx::query_scalar("select count(*) from posts where content = $1")
        .bind("Atomic outbox rollback probe")
        .fetch_one(&app.pool)
        .await
        .expect("post count");
    let outbox_count: i64 = sqlx::query_scalar(
        "select count(*) from post_event_outbox where event_type = 'POST_CREATED'",
    )
    .fetch_one(&app.pool)
    .await
    .expect("outbox count");

    assert_eq!(post_count, 0);
    assert_eq!(outbox_count, 0);
}

#[tokio::test]
async fn create_comment_rolls_back_comment_and_counter_when_outbox_write_fails() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Comment rollback parent".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let post_id = Uuid::parse_str(&post.id).expect("post id");
    sqlx::query(
        "alter table post_event_outbox add constraint reject_post_commented_events check (event_type <> 'POST_COMMENTED')",
    )
    .execute(&app.pool)
    .await
    .expect("constraint");

    let result = app
        .state
        .create_comment
        .execute(CreateCommentDto {
            post_id: post.id,
            author_id: "11111111-1111-1111-1111-111111111111".into(),
            content: "Atomic comment rollback probe".into(),
        })
        .await;

    assert!(result.is_err());
    let comment_count: i64 = sqlx::query_scalar("select count(*) from comments where content = $1")
        .bind("Atomic comment rollback probe")
        .fetch_one(&app.pool)
        .await
        .expect("comment count");
    let stored_counter: i64 = sqlx::query_scalar("select comment_count from posts where id = $1")
        .bind(post_id)
        .fetch_one(&app.pool)
        .await
        .expect("stored counter");
    let outbox_count: i64 = sqlx::query_scalar(
        "select count(*) from post_event_outbox where event_type = 'POST_COMMENTED'",
    )
    .fetch_one(&app.pool)
    .await
    .expect("outbox count");

    assert_eq!(comment_count, 0);
    assert_eq!(stored_counter, 0);
    assert_eq!(outbox_count, 0);
}

#[tokio::test]
async fn delete_post_rolls_back_soft_delete_and_audit_when_outbox_write_fails() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Delete rollback parent".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let post_id = Uuid::parse_str(&post.id).expect("post id");
    let author_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("author id");
    sqlx::query(
        "alter table post_event_outbox add constraint reject_post_deleted_events check (event_type <> 'POST_DELETED')",
    )
    .execute(&app.pool)
    .await
    .expect("constraint");

    let result = app
        .state
        .delete_post
        .execute(DeletePostDto {
            post_id: post.id,
            actor_id: author_id.to_string(),
            request_id: Some("req-delete-rollback".into()),
            ip_address: Some("203.0.113.20".into()),
            user_agent: Some("post-delete-rollback-test".into()),
        })
        .await;

    assert!(result.is_err());
    let deleted_at: Option<chrono::DateTime<chrono::Utc>> =
        sqlx::query_scalar("select deleted_at from posts where id = $1")
            .bind(post_id)
            .fetch_one(&app.pool)
            .await
            .expect("deleted_at");
    let audit_count: i64 = sqlx::query_scalar(
        "select count(*) from post_audit_events where post_id = $1 and action = 'POST_DELETED'",
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await
    .expect("audit count");
    let outbox_count: i64 = sqlx::query_scalar(
        "select count(*) from post_event_outbox where aggregate_id = $1 and event_type = 'POST_DELETED'",
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await
    .expect("outbox count");

    assert!(deleted_at.is_none());
    assert_eq!(audit_count, 0);
    assert_eq!(outbox_count, 0);
}

#[tokio::test]
async fn like_post_rolls_back_like_and_counter_when_outbox_write_fails() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Like rollback parent".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let post_id = Uuid::parse_str(&post.id).expect("post id");
    let liker_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("liker id");
    sqlx::query(
        "alter table post_event_outbox add constraint reject_post_liked_events check (event_type <> 'POST_LIKED')",
    )
    .execute(&app.pool)
    .await
    .expect("constraint");

    let result = app.state.like_post.execute(post_id, liker_id).await;

    assert!(result.is_err());
    let like_rows: i64 =
        sqlx::query_scalar("select count(*) from post_likes where post_id = $1 and user_id = $2")
            .bind(post_id)
            .bind(liker_id)
            .fetch_one(&app.pool)
            .await
            .expect("like rows");
    let stored_counter: i64 = sqlx::query_scalar("select like_count from posts where id = $1")
        .bind(post_id)
        .fetch_one(&app.pool)
        .await
        .expect("stored counter");
    let outbox_count: i64 = sqlx::query_scalar(
        "select count(*) from post_event_outbox where aggregate_id = $1 and event_type = 'POST_LIKED'",
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await
    .expect("outbox count");

    assert_eq!(like_rows, 0);
    assert_eq!(stored_counter, 0);
    assert_eq!(outbox_count, 0);
}

#[tokio::test]
async fn duplicate_like_event_payload_reports_current_like_count() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let author_id = Uuid::parse_str("22222222-2222-2222-2222-222222222222").expect("author id");
    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: author_id.to_string(),
            content: "Duplicate like payload parent".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let post_id = Uuid::parse_str(&post.id).expect("post id");
    let liker_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("liker id");

    app.state
        .like_post
        .execute(post_id, liker_id)
        .await
        .expect("first like");
    app.state
        .like_post
        .execute(post_id, liker_id)
        .await
        .expect("duplicate like");

    let payload: String = sqlx::query_scalar(
        r#"
        select payload
        from post_event_outbox
        where aggregate_id = $1
          and event_type = 'POST_LIKED'
        order by created_at desc, event_id desc
        limit 1
        "#,
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await
    .expect("latest like payload");
    let payload: serde_json::Value = serde_json::from_str(&payload).expect("payload json");

    let event_count: i64 = sqlx::query_scalar(
        "select count(*) from post_event_outbox where aggregate_id = $1 and event_type = 'POST_LIKED'",
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await
    .expect("like event count");
    let stored_counter: i64 = sqlx::query_scalar("select like_count from posts where id = $1")
        .bind(post_id)
        .fetch_one(&app.pool)
        .await
        .expect("stored counter");

    assert_eq!(payload["likeCount"], 1);
    assert_eq!(payload["postId"], post_id.to_string());
    assert_eq!(payload["actorId"], liker_id.to_string());
    assert_eq!(payload["authorId"], author_id.to_string());
    assert_eq!(payload["notifyUserId"], author_id.to_string());
    assert_eq!(event_count, 1);
    assert_eq!(stored_counter, 1);
}

#[tokio::test]
async fn duplicate_unlike_does_not_append_a_second_event() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Duplicate unlike parent".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let post_id = Uuid::parse_str(&post.id).expect("post id");
    let liker_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("liker id");

    app.state
        .like_post
        .execute(post_id, liker_id)
        .await
        .expect("like");
    app.state
        .unlike_post
        .execute(post_id, liker_id)
        .await
        .expect("first unlike");
    app.state
        .unlike_post
        .execute(post_id, liker_id)
        .await
        .expect("duplicate unlike");

    let event_count: i64 = sqlx::query_scalar(
        "select count(*) from post_event_outbox where aggregate_id = $1 and event_type = 'POST_UNLIKED'",
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await
    .expect("unlike event count");
    let stored_counter: i64 = sqlx::query_scalar("select like_count from posts where id = $1")
        .bind(post_id)
        .fetch_one(&app.pool)
        .await
        .expect("stored counter");

    assert_eq!(event_count, 1);
    assert_eq!(stored_counter, 0);
}

#[tokio::test]
async fn unlike_post_rolls_back_like_and_counter_when_outbox_write_fails() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Unlike rollback parent".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let post_id = Uuid::parse_str(&post.id).expect("post id");
    let liker_id = Uuid::parse_str("11111111-1111-1111-1111-111111111111").expect("liker id");
    app.state
        .like_post
        .execute(post_id, liker_id)
        .await
        .expect("like");
    sqlx::query(
        "alter table post_event_outbox add constraint reject_post_unliked_events check (event_type <> 'POST_UNLIKED')",
    )
    .execute(&app.pool)
    .await
    .expect("constraint");

    let result = app.state.unlike_post.execute(post_id, liker_id).await;

    assert!(result.is_err());
    let like_rows: i64 =
        sqlx::query_scalar("select count(*) from post_likes where post_id = $1 and user_id = $2")
            .bind(post_id)
            .bind(liker_id)
            .fetch_one(&app.pool)
            .await
            .expect("like rows");
    let stored_counter: i64 = sqlx::query_scalar("select like_count from posts where id = $1")
        .bind(post_id)
        .fetch_one(&app.pool)
        .await
        .expect("stored counter");
    let outbox_count: i64 = sqlx::query_scalar(
        "select count(*) from post_event_outbox where aggregate_id = $1 and event_type = 'POST_UNLIKED'",
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await
    .expect("outbox count");

    assert_eq!(like_rows, 1);
    assert_eq!(stored_counter, 1);
    assert_eq!(outbox_count, 0);
}

#[tokio::test]
async fn concurrent_outbox_claims_are_disjoint() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    for content in ["claim-a", "claim-b", "claim-c", "claim-d"] {
        app.state
            .create_post
            .execute(CreatePostDto {
                author_id: "22222222-2222-2222-2222-222222222222".into(),
                content: content.into(),
                visibility: "PUBLIC".into(),
            })
            .await
            .expect("post");
    }
    let left = PgEventOutbox::new(app.pool.clone());
    let right = PgEventOutbox::new(app.pool.clone());

    let (left_events, right_events) = tokio::join!(
        post_service::application::ports::EventOutboxPort::find_unpublished(&left, Utc::now(), 2,),
        post_service::application::ports::EventOutboxPort::find_unpublished(&right, Utc::now(), 2,),
    );
    let left_ids = left_events
        .expect("left claim")
        .into_iter()
        .map(|event| event.event_id)
        .collect::<std::collections::HashSet<_>>();
    let right_ids = right_events
        .expect("right claim")
        .into_iter()
        .map(|event| event.event_id)
        .collect::<std::collections::HashSet<_>>();

    assert_eq!(left_ids.len(), 2);
    assert_eq!(right_ids.len(), 2);
    assert!(left_ids.is_disjoint(&right_ids));
}

#[tokio::test]
async fn outbox_sequences_aggregate_events_and_holds_later_events_behind_the_head() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Aggregate ordering parent".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    app.state
        .create_comment
        .execute(CreateCommentDto {
            post_id: post.id.clone(),
            author_id: "11111111-1111-1111-1111-111111111111".into(),
            content: "Aggregate ordering child".into(),
        })
        .await
        .expect("comment");

    let post_id = Uuid::parse_str(&post.id).expect("post id");
    let versions: Vec<(String, i64)> = sqlx::query_as(
        "select event_type, aggregate_version from post_event_outbox where aggregate_id = $1 order by aggregate_version",
    )
    .bind(post_id)
    .fetch_all(&app.pool)
    .await
    .expect("aggregate versions");
    assert_eq!(
        versions,
        vec![
            ("POST_CREATED".to_owned(), 1),
            ("POST_COMMENTED".to_owned(), 2)
        ]
    );

    let outbox = PgEventOutbox::new(app.pool.clone());
    let older_than = Utc::now() + chrono::Duration::seconds(1);
    let first_claim = post_service::application::ports::EventOutboxPort::find_unpublished(
        &outbox, older_than, 10,
    )
    .await
    .expect("claim aggregate head");
    assert_eq!(first_claim.len(), 1);
    assert_eq!(first_claim[0].event_type, "POST_CREATED");
    assert_eq!(first_claim[0].aggregate_version, Some(1));

    // A second worker cannot pass the leased head and publish version 2 early.
    let concurrent_claim = post_service::application::ports::EventOutboxPort::find_unpublished(
        &outbox, older_than, 10,
    )
    .await
    .expect("second claim");
    assert!(concurrent_claim.is_empty());

    post_service::application::ports::EventOutboxPort::mark_published(
        &outbox,
        first_claim[0].event_id,
        Utc::now(),
    )
    .await
    .expect("complete aggregate head");

    let next_claim = post_service::application::ports::EventOutboxPort::find_unpublished(
        &outbox, older_than, 10,
    )
    .await
    .expect("claim next aggregate event");
    assert_eq!(next_claim.len(), 1);
    assert_eq!(next_claim[0].event_type, "POST_COMMENTED");
    assert_eq!(next_claim[0].aggregate_version, Some(2));
}

#[tokio::test]
async fn dead_letter_inspection_reports_blocked_successors_without_payload_or_error_text() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Dead-letter inspection parent".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let post_id = Uuid::parse_str(&post.id).expect("post id");
    let event_id: Uuid = sqlx::query_scalar(
        "select event_id from post_event_outbox where aggregate_id = $1 and event_type = 'POST_CREATED'",
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await
    .expect("created event id");
    sqlx::query(
        "update post_event_outbox set attempt_count = 5, dead_lettered_at = now(), last_error = 'sensitive diagnostic detail' where event_id = $1",
    )
    .bind(event_id)
    .execute(&app.pool)
    .await
    .expect("dead-letter event");
    sqlx::query(
        "insert into event_publications (event_id, target, published_at, payload) select event_id, 'EXTERNAL', now(), payload from post_event_outbox where event_id = $1",
    )
    .bind(event_id)
    .execute(&app.pool)
    .await
    .expect("external target checkpoint");

    app.state
        .create_comment
        .execute(CreateCommentDto {
            post_id: post.id,
            author_id: "11111111-1111-1111-1111-111111111111".into(),
            content: "Successor blocked behind dead-letter".into(),
        })
        .await
        .expect("comment");

    let script = include_str!("../../../../scripts/admin/inspect-post-outbox-dead-letters.sql");
    assert!(script.contains("BEGIN READ ONLY;"));
    assert!(script.contains("\\ir post-outbox-dead-letter-inspection.sql"));
    // psql expands the wrapper's include. Execute that same SQL under an
    // actual read-only transaction rather than sending psql commands to SQLx.
    let query = include_str!("../../../../scripts/admin/post-outbox-dead-letter-inspection.sql");
    let mut transaction = app.pool.begin().await.expect("inspection transaction");
    sqlx::raw_sql("SET TRANSACTION READ ONLY; SET LOCAL statement_timeout='10s';")
        .execute(&mut *transaction)
        .await
        .expect("read-only inspection settings");
    let row = sqlx::query(query)
        .fetch_one(&mut *transaction)
        .await
        .expect("inspect dead letter");
    transaction.commit().await.expect("commit inspection");

    assert_eq!(row.get::<Uuid, _>("event_id"), event_id);
    assert_eq!(row.get::<Uuid, _>("aggregate_id"), post_id);
    assert_eq!(row.get::<i64, _>("aggregate_version"), 1);
    assert_eq!(row.get::<i64, _>("unpublished_successors"), 1);
    let observations: serde_json::Value = row.get("target_observations");
    let targets = observations
        .as_array()
        .expect("structured target observations");
    assert_eq!(targets.len(), 4);
    assert!(targets
        .iter()
        .any(|target| target["target"] == "EXTERNAL" && target["checkpoint"] == true));
    assert!(targets.iter().any(|target| target["target"] == "CACHE"
        && target["state"] == "PENDING"
        && target["checkpoint"] == false));
    assert!(row.try_get::<Uuid, _>("lease_token").is_err());
    assert!(row.try_get::<String, _>("payload").is_err());
    assert!(row.try_get::<String, _>("last_error").is_err());
}

#[tokio::test]
async fn audited_dead_letter_resolution_is_idempotent_and_releases_the_aggregate_head() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Resolve dead-letter head".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let post_id = Uuid::parse_str(&post.id).expect("post ID");
    let event_id: Uuid = sqlx::query_scalar(
        "select event_id from post_event_outbox where aggregate_id = $1 and event_type = 'POST_CREATED'",
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await
    .expect("event ID");
    app.state
        .create_comment
        .execute(CreateCommentDto {
            post_id: post.id,
            author_id: "11111111-1111-1111-1111-111111111111".into(),
            content: "Successor waits for the resolved head".into(),
        })
        .await
        .expect("successor event");
    sqlx::query(
        "update post_event_outbox set attempt_count = 5, dead_lettered_at = now(), last_error = 'broker diagnostic' where event_id = $1",
    )
    .bind(event_id)
    .execute(&app.pool)
    .await
    .expect("dead-letter event");

    let resolution_id = Uuid::new_v4();
    let request = ResolveDeadLetterReplayRequestDto {
        event_id: event_id.to_string(),
        resolution_id: resolution_id.to_string(),
        actor_id: "33333333-3333-3333-3333-333333333333".into(),
        reason: "Broker access was restored and the event is safe to retry".into(),
        evidence_reference: "OPS-incident/INC-2026-1042".into(),
    };
    let result = app
        .state
        .resolve_dead_letter_replay
        .execute(request.clone())
        .await
        .expect("record audited replay");
    assert!(result.created);
    assert_eq!(result.event_id, event_id.to_string());
    assert_eq!(result.resolution_id, resolution_id.to_string());

    let state: (i32, Option<String>, Option<chrono::DateTime<Utc>>, Option<Uuid>) =
        sqlx::query_as(
            "select attempt_count, last_error, dead_lettered_at, replay_resolution_id from post_event_outbox where event_id = $1",
        )
        .bind(event_id)
        .fetch_one(&app.pool)
        .await
        .expect("requeued event");
    assert_eq!(state, (0, None, None, Some(resolution_id)));

    let duplicate = app
        .state
        .resolve_dead_letter_replay
        .execute(request.clone())
        .await
        .expect("same resolution ID is idempotent");
    assert!(!duplicate.created);
    let mut conflicting = request.clone();
    conflicting.reason = "A different decision using the same ID is forbidden".into();
    let conflict = app
        .state
        .resolve_dead_letter_replay
        .execute(conflicting)
        .await
        .expect_err("a resolution ID cannot be reused with changed content");
    assert!(matches!(
        conflict.downcast_ref::<DeadLetterReplayError>(),
        Some(DeadLetterReplayError::Conflict)
    ));

    let forwarded = app
        .outbox_forwarder()
        .republish_unpublished(Utc::now(), 10)
        .await
        .expect("deliver the resolved head and then its successor");
    assert_eq!(forwarded, 2);

    let attempts: (i64, i64, i64) = sqlx::query_as(
        r#"
        select count(*),
               count(*) filter (where source = 'ADMIN_REPLAY'),
               count(*) filter (where resolution_id = $2)
        from post_event_delivery_attempts
        where event_id = $1
        "#,
    )
    .bind(event_id)
    .bind(resolution_id)
    .fetch_one(&app.pool)
    .await
    .expect("attempt history");
    assert_eq!(attempts, (8, 8, 8));

    let published: i64 = sqlx::query_scalar(
        "select count(*) from post_event_outbox where aggregate_id = $1 and published = true",
    )
    .bind(post_id)
    .fetch_one(&app.pool)
    .await
    .expect("published aggregate events");
    assert_eq!(published, 2);

    let audit_count: i64 = sqlx::query_scalar(
        "select count(*) from post_event_dead_letter_resolutions where event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&app.pool)
    .await
    .expect("immutable operator audit");
    assert_eq!(audit_count, 1);
    assert!(sqlx::query(
        "update post_event_dead_letter_resolutions set reason = 'rewritten' where resolution_id = $1",
    )
    .bind(resolution_id)
    .execute(&app.pool)
    .await
    .is_err());
}

#[tokio::test]
async fn postgres_outbox_tracks_retry_failures_and_dead_letters_at_threshold() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let outbox = PgEventOutbox::new(app.pool.clone());
    let post = app
        .state
        .create_post
        .execute(CreatePostDto {
            author_id: "22222222-2222-2222-2222-222222222222".into(),
            content: "Retry me".into(),
            visibility: "PUBLIC".into(),
        })
        .await
        .expect("post");
    let event_id: Uuid =
        sqlx::query_scalar("select event_id from post_event_outbox where aggregate_id = $1")
            .bind(Uuid::parse_str(&post.id).expect("uuid"))
            .fetch_one(&app.pool)
            .await
            .expect("event id");

    outbox
        .record_publish_failure(event_id, "broker unavailable", 2)
        .await
        .expect("first failure");
    outbox
        .record_publish_failure(event_id, "broker unavailable", 2)
        .await
        .expect("second failure");

    let row: (i32, String, Option<chrono::DateTime<chrono::Utc>>) = sqlx::query_as(
        "select attempt_count, last_error, dead_lettered_at from post_event_outbox where event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&app.pool)
    .await
    .expect("outbox row");

    assert_eq!(row.0, 2);
    assert_eq!(row.1, "broker unavailable");
    assert!(row.2.is_some());
}
