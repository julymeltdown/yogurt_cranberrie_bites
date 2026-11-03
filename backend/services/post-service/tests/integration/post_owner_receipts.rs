use chrono::{Duration, Utc};
use post_service::adapters::out::persistence::post_repository::{
    PgPostLikeRepository, PgPostRepository,
};
use post_service::application::dto::mutation_dto::{
    MutationContextDto, MutationError, MutationKind, MutationOperationDto, MutationOutcomeDto,
};
use post_service::application::ports::{PostLikeRepositoryPort, PostRepositoryPort};
use post_service::domain::{
    comment::Comment,
    post::{Post, PostAuditEvent, StoredEvent},
};
use serde_json::json;
use sqlx::PgPool;
use uuid::Uuid;

struct OwnerDatabase {
    _postgres: Option<test_support::postgres::PostgresContainer>,
    pool: PgPool,
}

async fn database() -> OwnerDatabase {
    // These tests exercise only PostgreSQL transactions. Starting Redis and the
    // whole application here obscures transaction failures with unrelated setup.
    let (postgres, database_url) = if std::env::var_os("POST_TEST_DATABASE_URL").is_some() {
        (
            None,
            crate::support::native_postgres::database_url()
                .await
                .unwrap(),
        )
    } else {
        let postgres = test_support::postgres::start_postgres().await.unwrap();
        let url = format!(
            "postgres://postgres:postgres@{}:{}/postgres",
            postgres.get_host().await.unwrap(),
            postgres.get_host_port_ipv4(5432).await.unwrap()
        );
        (Some(postgres), url)
    };
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(5)
        .connect(&database_url)
        .await
        .unwrap();
    let migrations = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    sqlx::migrate::Migrator::new(migrations.as_path())
        .await
        .unwrap()
        .run(&pool)
        .await
        .unwrap();
    OwnerDatabase {
        _postgres: postgres,
        pool,
    }
}

fn operation(actor_id: Uuid, kind: MutationKind, input: serde_json::Value) -> MutationOperationDto {
    MutationOperationDto::new(
        MutationContextDto {
            operation_id: Uuid::new_v4(),
            actor_id,
            idempotency_key: Some(Uuid::new_v4().to_string()),
            lookup_only: false,
        },
        actor_id,
        kind,
        input,
    )
    .unwrap()
}

fn post(actor_id: Uuid) -> Post {
    Post {
        id: Uuid::new_v4(),
        author_id: actor_id,
        content: "owner receipt".into(),
        visibility: "PUBLIC".into(),
        created_at: Utc::now(),
        comment_count: 0,
        like_count: 0,
    }
}

fn event(kind: &str, post_id: Uuid, actor_id: Uuid) -> StoredEvent {
    StoredEvent {
        event_id: Uuid::new_v4(),
        event_type: kind.into(),
        aggregate_id: post_id,
        aggregate_version: None,
        origin_traceparent: None,
        actor_id: Some(actor_id),
        attributes: vec![],
        payload: json!({"postId":post_id,"actorId":actor_id,"authorId":actor_id,"notifyUserId":actor_id,"postAuthorId":actor_id}).to_string(),
        occurred_at: Utc::now(),
        created_at: Utc::now(),
        published: false,
        published_at: None,
        replay_resolution_id: None,
    }
}

#[tokio::test]
async fn outbox_persists_traceparent_as_internal_metadata_only() {
    const PARENT: &str = "00-4bf92f3577b34da6a3ce929d0e0e4736-00f067aa0ba902b7-01";
    let database = database().await;
    let repository = PgPostRepository::new(database.pool.clone());
    let actor_id = Uuid::new_v4();
    let post = post(actor_id);
    let event = event("POST_CREATED", post.id, actor_id);
    let context = platform::trace_context::TraceContext::child_from_traceparent(PARENT)
        .expect("valid trace parent");
    let expected_traceparent = context.traceparent();

    let (_, saved_event) = platform::trace_context::scope(
        context,
        repository.save_post_with_event(&post, &event, None),
    )
    .await
    .expect("save post and trace-linked outbox event");

    assert_eq!(
        saved_event.origin_traceparent.as_deref(),
        Some(expected_traceparent.as_str())
    );
    let persisted: Option<String> =
        sqlx::query_scalar("select origin_traceparent from post_event_outbox where event_id = $1")
            .bind(event.event_id)
            .fetch_one(&database.pool)
            .await
            .expect("read outbox trace metadata");
    assert_eq!(persisted.as_deref(), Some(expected_traceparent.as_str()));
    assert!(
        serde_json::to_value(&saved_event)
            .expect("serialize stored event")
            .get("origin_traceparent")
            .is_none(),
        "trace context is internal metadata, not part of serialized receipts"
    );
}

async fn counts(pool: &PgPool) -> (i64, i64, i64, i64, i64) {
    sqlx::query_as(
        "select (select count(*) from posts), (select count(*) from comments),
        (select count(*) from post_likes), (select count(*) from post_event_outbox),
        (select count(*) from post_mutation_receipts)",
    )
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn concurrent_create_and_lost_reply_replay_one_committed_outcome() {
    let app = database().await;
    let repo = PgPostRepository::new(app.pool.clone());
    let actor = Uuid::new_v4();
    let op = operation(
        actor,
        MutationKind::CreatePost,
        json!(["owner receipt", "PUBLIC"]),
    );
    let a = post(actor);
    let b = post(actor);
    let ae = event("POST_CREATED", a.id, actor);
    let be = event("POST_CREATED", b.id, actor);
    let (first, second) = tokio::join!(
        repo.save_post_with_event(&a, &ae, Some(&op)),
        repo.save_post_with_event(&b, &be, Some(&op)),
    );
    let first = first.unwrap();
    assert_eq!(first, second.unwrap());
    assert_eq!(counts(&app.pool).await, (1, 0, 0, 1, 1));

    // The caller can lose the reply and discard all in-process state.
    drop(repo);
    let reopened = PgPostRepository::new(app.pool.clone());
    let replay = reopened.lookup_mutation(&op).await.unwrap().unwrap();
    assert_eq!(
        replay,
        MutationOutcomeDto::Post(first.0.clone(), first.1.clone())
    );
    assert_eq!(
        reopened
            .save_post_with_event(&b, &be, Some(&op))
            .await
            .unwrap(),
        first
    );

    let mut changed = op.clone();
    changed.canonical_command = json!(["different", "PUBLIC"]).to_string();
    assert_eq!(
        reopened
            .lookup_mutation(&changed)
            .await
            .unwrap_err()
            .downcast_ref::<MutationError>(),
        Some(&MutationError::Conflict)
    );
    changed = op.clone();
    changed.kind = MutationKind::LikePost;
    assert!(reopened.lookup_mutation(&changed).await.is_err());
    changed = op.clone();
    changed.context.actor_id = Uuid::new_v4();
    assert!(reopened.lookup_mutation(&changed).await.is_err());
    changed = op.clone();
    changed.context.operation_id = Uuid::new_v4();
    assert!(reopened.lookup_mutation(&changed).await.is_err());
    assert_eq!(counts(&app.pool).await, (1, 0, 0, 1, 1));

    sqlx::query("update post_mutation_receipts set replay_until = now() - interval '1 second'")
        .execute(&app.pool)
        .await
        .unwrap();
    assert_eq!(
        reopened
            .save_post_with_event(&b, &be, Some(&op))
            .await
            .unwrap_err()
            .downcast_ref::<MutationError>(),
        Some(&MutationError::Expired)
    );
    reopened
        .purge_expired_posts(Utc::now(), actor, None, None, None, false)
        .await
        .unwrap();
    let tombstone: (i64, i64) =
        sqlx::query_as("select count(*), count(outcome_json) from post_mutation_receipts")
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(tombstone, (1, 0));
    assert!(reopened
        .save_post_with_event(&b, &be, Some(&op))
        .await
        .is_err());
    assert_eq!(counts(&app.pool).await, (1, 0, 0, 1, 1));
}

#[tokio::test]
async fn comment_like_unlike_and_delete_receipts_share_the_effect_transaction() {
    let app = database().await;
    let posts = PgPostRepository::new(app.pool.clone());
    let likes = PgPostLikeRepository::new(app.pool.clone());
    let actor = Uuid::new_v4();
    let parent = post(actor);
    posts.save_post(&parent).await.unwrap();
    let comment = Comment {
        id: Uuid::new_v4(),
        post_id: parent.id,
        author_id: actor,
        content: "one comment".into(),
        created_at: Utc::now(),
    };
    let commented = event("POST_COMMENTED", parent.id, actor);
    let comment_op = operation(
        actor,
        MutationKind::CreateComment,
        json!([parent.id, comment.content]),
    );
    let first = posts
        .save_comment_with_counter_event(&comment, &commented, Some(&comment_op))
        .await
        .unwrap();
    let retry = Comment {
        id: Uuid::new_v4(),
        ..comment.clone()
    };
    assert_eq!(
        posts
            .save_comment_with_counter_event(&retry, &commented, Some(&comment_op))
            .await
            .unwrap(),
        first
    );

    let liked = event("POST_LIKED", parent.id, actor);
    let like_op = operation(actor, MutationKind::LikePost, json!([parent.id]));
    assert_eq!(
        likes
            .add_like_with_event(parent.id, actor, &liked, Some(&like_op))
            .await
            .unwrap(),
        (true, 1)
    );
    let unliked = event("POST_UNLIKED", parent.id, actor);
    let unlike_op = operation(actor, MutationKind::UnlikePost, json!([parent.id]));
    assert_eq!(
        likes
            .remove_like_with_event(parent.id, actor, &unliked, Some(&unlike_op))
            .await
            .unwrap(),
        (true, 0)
    );
    // A late replay of LIKE must not undo a subsequent UNLIKE.
    assert_eq!(
        likes
            .add_like_with_event(parent.id, actor, &liked, Some(&like_op))
            .await
            .unwrap(),
        (true, 1)
    );
    assert_eq!(
        likes
            .remove_like_with_event(parent.id, actor, &unliked, Some(&unlike_op))
            .await
            .unwrap(),
        (true, 0)
    );
    assert_eq!(counts(&app.pool).await, (1, 1, 0, 3, 3));
    let counters: (i64, i64) =
        sqlx::query_as("select comment_count, like_count from posts where id=$1")
            .bind(parent.id)
            .fetch_one(&app.pool)
            .await
            .unwrap();
    assert_eq!(counters, (1, 0));

    let deleted = event("POST_DELETED", parent.id, actor);
    let delete_op = operation(actor, MutationKind::DeletePost, json!([parent.id]));
    let audit = PostAuditEvent {
        id: Uuid::new_v4(),
        actor_id: Some(actor),
        post_id: Some(parent.id),
        comment_id: None,
        action: "POST_DELETED".into(),
        outcome: "SUCCESS".into(),
        request_id: None,
        ip_address: None,
        user_agent: None,
        metadata_json: "{}".into(),
        occurred_at: Utc::now(),
    };
    for _ in 0..2 {
        assert!(posts
            .soft_delete_post_with_event_audit(
                parent.id,
                actor,
                "user_request",
                Utc::now() + Duration::days(30),
                &deleted,
                &audit,
                Some(&delete_op)
            )
            .await
            .unwrap());
    }
    assert_eq!(counts(&app.pool).await, (1, 1, 0, 4, 4));
    let audits: i64 = sqlx::query_scalar("select count(*) from post_audit_events")
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert_eq!(audits, 1);
}

#[tokio::test]
async fn failed_outbox_or_receipt_write_rolls_back_everything_and_allows_safe_retry() {
    let app = database().await;
    let repo = PgPostRepository::new(app.pool.clone());
    let actor = Uuid::new_v4();
    let value = post(actor);
    let emitted = event("POST_CREATED", value.id, actor);
    let op = operation(
        actor,
        MutationKind::CreatePost,
        json!([value.content, value.visibility]),
    );
    sqlx::query("alter table post_event_outbox add constraint reject_event check (event_type <> 'POST_CREATED')")
        .execute(&app.pool).await.unwrap();
    assert!(repo
        .save_post_with_event(&value, &emitted, Some(&op))
        .await
        .is_err());
    assert_eq!(counts(&app.pool).await, (0, 0, 0, 0, 0));
    sqlx::query("alter table post_event_outbox drop constraint reject_event")
        .execute(&app.pool)
        .await
        .unwrap();
    sqlx::query("alter table post_mutation_receipts add constraint reject_completion check (outcome_json is null)")
        .execute(&app.pool).await.unwrap();
    assert!(repo
        .save_post_with_event(&value, &emitted, Some(&op))
        .await
        .is_err());
    assert_eq!(counts(&app.pool).await, (0, 0, 0, 0, 0));
    sqlx::query("alter table post_mutation_receipts drop constraint reject_completion")
        .execute(&app.pool)
        .await
        .unwrap();
    repo.save_post_with_event(&value, &emitted, Some(&op))
        .await
        .unwrap();
    assert_eq!(counts(&app.pool).await, (1, 0, 0, 1, 1));
}

#[tokio::test]
async fn terminating_the_owner_connection_before_commit_leaves_no_effect_or_receipt() {
    let app = database().await;
    let actor = Uuid::new_v4();
    let value = post(actor);
    let emitted = event("POST_CREATED", value.id, actor);
    let op = operation(
        actor,
        MutationKind::CreatePost,
        json!([value.content, value.visibility]),
    );
    // Stop at the final receipt write, after the post/outbox SQL has run but
    // before COMMIT. Terminating this connection models loss of the owner process.
    sqlx::query("create function delay_receipt_completion() returns trigger language plpgsql as $$ begin perform pg_sleep(10); return new; end $$")
        .execute(&app.pool).await.unwrap();
    sqlx::query("create trigger pause_receipt before update on post_mutation_receipts for each row execute function delay_receipt_completion()")
        .execute(&app.pool).await.unwrap();
    let repo = PgPostRepository::new(app.pool.clone());
    let (candidate, candidate_event, candidate_op) = (value.clone(), emitted.clone(), op.clone());
    let attempt = tokio::spawn(async move {
        repo.save_post_with_event(&candidate, &candidate_event, Some(&candidate_op))
            .await
    });
    let backend = tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            let pid: Option<i32> = sqlx::query_scalar("select pid from pg_stat_activity where datname=current_database() and wait_event='PgSleep' and query like 'update post_mutation_receipts%' limit 1")
                .fetch_optional(&app.pool).await.unwrap();
            if let Some(pid) = pid { break pid; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("owner reached the pre-commit barrier");
    let terminated: bool = sqlx::query_scalar("select pg_terminate_backend($1)")
        .bind(backend)
        .fetch_one(&app.pool)
        .await
        .unwrap();
    assert!(terminated);
    assert!(attempt.await.unwrap().is_err());
    assert_eq!(counts(&app.pool).await, (0, 0, 0, 0, 0));
    sqlx::query("drop trigger pause_receipt on post_mutation_receipts")
        .execute(&app.pool)
        .await
        .unwrap();
    let reopened = PgPostRepository::new(app.pool.clone());
    reopened
        .save_post_with_event(&value, &emitted, Some(&op))
        .await
        .unwrap();
    assert_eq!(counts(&app.pool).await, (1, 0, 0, 1, 1));
}

/// Holds pool connections open together once, so connection setup does not
/// separate the contenders.
async fn warm_pool(pool: &PgPool, connections: usize) {
    let mut held = Vec::with_capacity(connections);
    for _ in 0..connections {
        held.push(pool.acquire().await.unwrap());
    }
}

/// Runs two attempts of one operation with a forced interleaving. A SHARE lock
/// on the outbox parks the first claimant after its receipt and effect writes;
/// the duplicate must be observed waiting at "loser_waits_on" (the receipt
/// insert for CreatePost, the post subject lock for commands that take it
/// before claiming) before the outbox is released.
async fn forced_duplicate<T, F, Fut>(pool: &PgPool, loser_waits_on: &str, attempt: F) -> (T, T)
where
    T: Send + 'static,
    F: Fn(u8) -> Fut,
    Fut: std::future::Future<Output = anyhow::Result<T>> + Send + 'static,
{
    warm_pool(pool, 4).await;
    let mut gate = pool.begin().await.unwrap();
    sqlx::query("lock table post_event_outbox in share mode")
        .execute(&mut *gate)
        .await
        .unwrap();
    let first = tokio::spawn(attempt(0));
    let second = tokio::spawn(attempt(1));
    let winner = "insert into post_event_outbox";
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(10);
    loop {
        // Backend activity is otherwise snapshotted once per transaction.
        sqlx::query("select pg_stat_clear_snapshot()")
            .execute(&mut *gate)
            .await
            .unwrap();
        let waiting: Vec<String> = sqlx::query_scalar(
            "select regexp_replace(query, '^\\s+', '') from pg_stat_activity
             where datname = current_database() and wait_event_type = 'Lock'
               and pid <> pg_backend_pid()",
        )
        .fetch_all(&mut *gate)
        .await
        .unwrap();
        let parked = waiting.iter().filter(|q| q.starts_with(winner)).count();
        let losers = waiting
            .iter()
            .filter(|q| q.starts_with(loser_waits_on))
            .count();
        if waiting.len() == 2 && parked == 1 && losers == 1 {
            break;
        }
        assert!(
            waiting.len() == parked + losers && parked <= 1 && losers <= 1,
            "unexpected lock waiters: {waiting:#?}"
        );
        assert!(
            std::time::Instant::now() < deadline,
            "contenders never parked: {waiting:#?}"
        );
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
    gate.commit().await.unwrap();
    (
        first.await.unwrap().expect("first contender"),
        second.await.unwrap().expect("second contender"),
    )
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn forced_concurrent_duplicates_of_every_post_receipt_kind_commit_one_effect() {
    let app = database().await;
    let pool = app.pool.clone();
    let actor = Uuid::new_v4();

    // CreatePost: the duplicate waits on the uncommitted receipt itself.
    let create = operation(actor, MutationKind::CreatePost, json!(["race", "PUBLIC"]));
    let created = forced_duplicate(&pool, "insert into post_mutation_receipts", |attempt| {
        let repo = PgPostRepository::new(pool.clone());
        let op = create.clone();
        async move {
            let value = post(actor);
            let emitted = event("POST_CREATED", value.id, actor);
            let _ = attempt;
            repo.save_post_with_event(&value, &emitted, Some(&op)).await
        }
    })
    .await;
    assert_eq!(created.0, created.1);
    assert_eq!(counts(&pool).await, (1, 0, 0, 1, 1));
    let parent = created.0 .0.id;

    // The remaining kinds take the post subject lock before claiming, so the
    // duplicate waits there and then reads the committed receipt.
    let subject_lock = "select post_lock_subject_ids";
    let comment_op = operation(actor, MutationKind::CreateComment, json!([parent, "race"]));
    let commented = forced_duplicate(&pool, subject_lock, |_| {
        let repo = PgPostRepository::new(pool.clone());
        let op = comment_op.clone();
        async move {
            let comment = Comment {
                id: Uuid::new_v4(),
                post_id: parent,
                author_id: actor,
                content: "race".into(),
                created_at: Utc::now(),
            };
            let emitted = event("POST_COMMENTED", parent, actor);
            repo.save_comment_with_counter_event(&comment, &emitted, Some(&op))
                .await
        }
    })
    .await;
    assert_eq!(commented.0, commented.1);
    assert_eq!(counts(&pool).await, (1, 1, 0, 2, 2));

    let like_op = operation(actor, MutationKind::LikePost, json!([parent]));
    let liked = forced_duplicate(&pool, subject_lock, |_| {
        let likes = PgPostLikeRepository::new(pool.clone());
        let op = like_op.clone();
        async move {
            let emitted = event("POST_LIKED", parent, actor);
            likes
                .add_like_with_event(parent, actor, &emitted, Some(&op))
                .await
        }
    })
    .await;
    assert_eq!(liked, ((true, 1), (true, 1)));
    assert_eq!(counts(&pool).await, (1, 1, 1, 3, 3));

    let unlike_op = operation(actor, MutationKind::UnlikePost, json!([parent]));
    let unliked = forced_duplicate(&pool, subject_lock, |_| {
        let likes = PgPostLikeRepository::new(pool.clone());
        let op = unlike_op.clone();
        async move {
            let emitted = event("POST_UNLIKED", parent, actor);
            likes
                .remove_like_with_event(parent, actor, &emitted, Some(&op))
                .await
        }
    })
    .await;
    assert_eq!(unliked, ((true, 0), (true, 0)));
    assert_eq!(counts(&pool).await, (1, 1, 0, 4, 4));

    let delete_op = operation(actor, MutationKind::DeletePost, json!([parent]));
    let deleted = forced_duplicate(&pool, subject_lock, |_| {
        let repo = PgPostRepository::new(pool.clone());
        let op = delete_op.clone();
        async move {
            let emitted = event("POST_DELETED", parent, actor);
            let audit = PostAuditEvent {
                id: Uuid::new_v4(),
                actor_id: Some(actor),
                post_id: Some(parent),
                comment_id: None,
                action: "POST_DELETED".into(),
                outcome: "SUCCESS".into(),
                request_id: None,
                ip_address: None,
                user_agent: None,
                metadata_json: "{}".into(),
                occurred_at: Utc::now(),
            };
            repo.soft_delete_post_with_event_audit(
                parent,
                actor,
                "user_request",
                Utc::now() + Duration::days(30),
                &emitted,
                &audit,
                Some(&op),
            )
            .await
        }
    })
    .await;
    assert_eq!(deleted, (true, true));
    assert_eq!(counts(&pool).await, (1, 1, 0, 5, 5));
    let effects: (i64, i64, i64) = sqlx::query_as(
        "select (select count(*) from post_audit_events where action = 'POST_DELETED'),
                (select comment_count from posts where id = $1)::bigint,
                (select like_count from posts where id = $1)::bigint",
    )
    .bind(parent)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(effects, (1, 1, 0));
}

/// Receipt completion is the last write in every Post owner transaction. A
/// failure there must leave neither the effect, its outbox event, an audit row
/// nor a receipt, and the same operation must then succeed once.
#[tokio::test]
async fn receipt_completion_failure_rolls_back_each_post_command_kind() {
    let app = database().await;
    let posts = PgPostRepository::new(app.pool.clone());
    let actor = Uuid::new_v4();
    let parent = post(actor);
    posts.save_post(&parent).await.unwrap();
    let reject = "alter table post_mutation_receipts add constraint reject_completion check (outcome_json is null) not valid";
    let allow = "alter table post_mutation_receipts drop constraint reject_completion";
    let state = |pool: PgPool| async move {
        sqlx::query_as::<_, (i64, i64, i64, i64, i64, bool)>(
            "select (select count(*) from comments), (select count(*) from post_likes),
                    (select count(*) from post_event_outbox), (select count(*) from post_mutation_receipts),
                    (select count(*) from post_audit_events),
                    (select deleted_at is null from posts limit 1)",
        )
        .fetch_one(&pool)
        .await
        .unwrap()
    };

    let comment = Comment {
        id: Uuid::new_v4(),
        post_id: parent.id,
        author_id: actor,
        content: "fails once".into(),
        created_at: Utc::now(),
    };
    let commented = event("POST_COMMENTED", parent.id, actor);
    let comment_op = operation(
        actor,
        MutationKind::CreateComment,
        json!([parent.id, comment.content]),
    );
    let liked = event("POST_LIKED", parent.id, actor);
    let like_op = operation(actor, MutationKind::LikePost, json!([parent.id]));
    let unliked = event("POST_UNLIKED", parent.id, actor);
    let unlike_op = operation(actor, MutationKind::UnlikePost, json!([parent.id]));
    let deleted = event("POST_DELETED", parent.id, actor);
    let delete_op = operation(actor, MutationKind::DeletePost, json!([parent.id]));
    let audit = PostAuditEvent {
        id: Uuid::new_v4(),
        actor_id: Some(actor),
        post_id: Some(parent.id),
        comment_id: None,
        action: "POST_DELETED".into(),
        outcome: "SUCCESS".into(),
        request_id: None,
        ip_address: None,
        user_agent: None,
        metadata_json: "{}".into(),
        occurred_at: Utc::now(),
    };
    let retention = Utc::now() + Duration::days(30);

    // Each step: the faulted attempt changes nothing, then the same operation
    // commits exactly one effect, event and receipt.
    let expected_after = [
        (1, 0, 1, 1, 0, true),
        (1, 1, 2, 2, 0, true),
        (1, 0, 3, 3, 0, true),
        (1, 0, 4, 4, 1, false),
    ];
    for (step, after) in expected_after.into_iter().enumerate() {
        let before = state(app.pool.clone()).await;
        sqlx::query(reject).execute(&app.pool).await.unwrap();
        let attempt = |pool: PgPool| {
            let posts = PgPostRepository::new(pool.clone());
            let likes = PgPostLikeRepository::new(pool);
            let (comment, commented, comment_op) = (&comment, &commented, &comment_op);
            let (liked, like_op, unliked, unlike_op) = (&liked, &like_op, &unliked, &unlike_op);
            let (deleted, delete_op, audit) = (&deleted, &delete_op, &audit);
            async move {
                match step {
                    0 => posts
                        .save_comment_with_counter_event(comment, commented, Some(comment_op))
                        .await
                        .map(|_| ()),
                    1 => likes
                        .add_like_with_event(parent.id, actor, liked, Some(like_op))
                        .await
                        .map(|_| ()),
                    2 => likes
                        .remove_like_with_event(parent.id, actor, unliked, Some(unlike_op))
                        .await
                        .map(|_| ()),
                    _ => posts
                        .soft_delete_post_with_event_audit(
                            parent.id,
                            actor,
                            "user_request",
                            retention,
                            deleted,
                            audit,
                            Some(delete_op),
                        )
                        .await
                        .map(|_| ()),
                }
            }
        };
        assert!(
            attempt(app.pool.clone()).await.is_err(),
            "step {step} must fail"
        );
        assert_eq!(
            state(app.pool.clone()).await,
            before,
            "step {step} left a write"
        );
        sqlx::query(allow).execute(&app.pool).await.unwrap();
        attempt(app.pool.clone()).await.unwrap();
        assert_eq!(state(app.pool.clone()).await, after, "step {step} retry");
    }
}
