//! Replay safety for the feed engagement projection.
//!
//! Delivery is at-least-once, so the same event is applied more than once: a
//! crash between the broker acknowledgement and the delivery checkpoint causes a
//! retry, and an operator replay after a fix sends stored events on purpose.
//!
//! These tests drive the repository directly rather than through the forwarder,
//! because the forwarder already has a checkpoint that stops it from attempting a
//! delivered target a second time. The case that checkpoint cannot cover is a
//! replay, which reaches the projection again by design, so the projection itself
//! is what has to be idempotent.

use chrono::Utc;
use post_service::adapters::out::persistence::feed_recommendation_repository::PgFeedRecommendationRepository;
use post_service::application::ports::FeedFeatureRepositoryPort;
use post_service::domain::ranking::FeedEngagementAction;
use uuid::Uuid;

fn uuid(value: &str) -> Uuid {
    Uuid::parse_str(value).expect("uuid")
}

async fn velocity(pool: &sqlx::PgPool, post_id: Uuid) -> (i64, i64) {
    sqlx::query_as(
        "select like_events, comment_events from post_engagement_velocity where post_id = $1",
    )
    .bind(post_id)
    .fetch_one(pool)
    .await
    .expect("velocity row")
}

async fn affinity(pool: &sqlx::PgPool, viewer_id: Uuid, author_id: Uuid) -> f64 {
    sqlx::query_scalar(
        "select affinity_score from feed_author_affinities where viewer_id = $1 and author_id = $2",
    )
    .bind(viewer_id)
    .bind(author_id)
    .fetch_one(pool)
    .await
    .expect("affinity row")
}

#[tokio::test]
async fn replayed_like_event_does_not_double_count_velocity_or_affinity() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let repository = PgFeedRecommendationRepository::new(app.pool.clone());
    let actor_id = uuid("11111111-1111-1111-1111-111111111111");
    let author_id = uuid("22222222-2222-2222-2222-222222222222");
    let post_id = uuid("33333333-3333-3333-3333-333333333333");
    let event_id = uuid("44444444-4444-4444-4444-444444444444");
    sqlx::query(
        "insert into posts (id, author_id, content, visibility, created_at, comment_count, like_count) values ($1, $2, $3, 'PUBLIC', $4, 0, 0)",
    )
    .bind(post_id)
    .bind(author_id)
    .bind("Replay target")
    .bind(Utc::now())
    .execute(&app.pool)
    .await
    .expect("post");

    repository
        .record_engagement(
            event_id,
            post_id,
            actor_id,
            author_id,
            FeedEngagementAction::Like,
            Utc::now(),
        )
        .await
        .expect("first apply");
    let after_first = velocity(&app.pool, post_id).await;
    let affinity_first = affinity(&app.pool, actor_id, author_id).await;

    // The same event arrives again, which is what a replay does.
    repository
        .record_engagement(
            event_id,
            post_id,
            actor_id,
            author_id,
            FeedEngagementAction::Like,
            Utc::now(),
        )
        .await
        .expect("replay");

    assert_eq!(velocity(&app.pool, post_id).await, after_first);
    assert_eq!(
        affinity(&app.pool, actor_id, author_id).await,
        affinity_first
    );
    assert_eq!(after_first, (1, 0));
}

#[tokio::test]
async fn distinct_events_still_accumulate() {
    // Idempotency is per event, not per post. Two likes from different events
    // are two likes, so the inbox must not collapse unrelated events.
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let repository = PgFeedRecommendationRepository::new(app.pool.clone());
    let author_id = uuid("22222222-2222-2222-2222-222222222223");
    let post_id = uuid("33333333-3333-3333-3333-333333333334");
    let first_actor = uuid("11111111-1111-1111-1111-111111111112");
    let second_actor = uuid("11111111-1111-1111-1111-111111111113");
    sqlx::query(
        "insert into posts (id, author_id, content, visibility, created_at, comment_count, like_count) values ($1, $2, $3, 'PUBLIC', $4, 0, 0)",
    )
    .bind(post_id)
    .bind(author_id)
    .bind("Accumulating post")
    .bind(Utc::now())
    .execute(&app.pool)
    .await
    .expect("post");

    repository
        .record_engagement(
            uuid("44444444-4444-4444-4444-444444444445"),
            post_id,
            first_actor,
            author_id,
            FeedEngagementAction::Like,
            Utc::now(),
        )
        .await
        .expect("first like");
    repository
        .record_engagement(
            uuid("44444444-4444-4444-4444-444444444446"),
            post_id,
            second_actor,
            author_id,
            FeedEngagementAction::Like,
            Utc::now(),
        )
        .await
        .expect("second like");

    assert_eq!(velocity(&app.pool, post_id).await, (2, 0));
}

#[tokio::test]
async fn unmatched_negative_engagements_keep_new_velocity_rows_nonnegative() {
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let repository = PgFeedRecommendationRepository::new(app.pool.clone());
    let actor_id = uuid("11111111-1111-1111-1111-111111111115");
    let author_id = uuid("22222222-2222-2222-2222-222222222225");
    let unmatched_unlike_post = uuid("33333333-3333-3333-3333-333333333336");
    let unmatched_delete_post = uuid("33333333-3333-3333-3333-333333333337");
    let unmatched_unlike_event = uuid("44444444-4444-4444-4444-444444444448");
    let unmatched_delete_event = uuid("44444444-4444-4444-4444-444444444449");

    for (post_id, content) in [
        (unmatched_unlike_post, "Unlike before any projected like"),
        (
            unmatched_delete_post,
            "Comment deletion before any projected comment",
        ),
    ] {
        sqlx::query(
            "insert into posts (id, author_id, content, visibility, created_at, comment_count, like_count) values ($1, $2, $3, 'PUBLIC', $4, 0, 0)",
        )
        .bind(post_id)
        .bind(author_id)
        .bind(content)
        .bind(Utc::now())
        .execute(&app.pool)
        .await
        .expect("post");
    }

    for (event_id, post_id, action) in [
        (
            unmatched_unlike_event,
            unmatched_unlike_post,
            FeedEngagementAction::Unlike,
        ),
        (
            unmatched_delete_event,
            unmatched_delete_post,
            FeedEngagementAction::DeleteComment,
        ),
    ] {
        repository
            .record_engagement(event_id, post_id, actor_id, author_id, action, Utc::now())
            .await
            .expect("a negative action without a projected positive must be safely clamped");
        assert_eq!(velocity(&app.pool, post_id).await, (0, 0));
        let score: f64 = sqlx::query_scalar(
            "select velocity_score from post_engagement_velocity where post_id = $1",
        )
        .bind(post_id)
        .fetch_one(&app.pool)
        .await
        .expect("velocity score");
        assert_eq!(score, 0.0);
    }

    repository
        .record_engagement(
            unmatched_unlike_event,
            unmatched_unlike_post,
            actor_id,
            author_id,
            FeedEngagementAction::Unlike,
            Utc::now(),
        )
        .await
        .expect("replaying the unmatched unlike must remain harmless");
    assert_eq!(velocity(&app.pool, unmatched_unlike_post).await, (0, 0));

    let inbox_rows: i64 = sqlx::query_scalar(
        "select count(*) from feed_projection_inbox where consumer = 'feed-engagement' and event_id = $1",
    )
    .bind(unmatched_unlike_event)
    .fetch_one(&app.pool)
    .await
    .expect("inbox count");
    assert_eq!(inbox_rows, 1);

    let negative_affinity: f64 = sqlx::query_scalar(
        "select negative_score from feed_author_affinities where viewer_id = $1 and author_id = $2",
    )
    .bind(actor_id)
    .bind(author_id)
    .fetch_one(&app.pool)
    .await
    .expect("negative affinity");
    assert!(negative_affinity > 0.0);
}

#[tokio::test]
async fn a_failed_projection_does_not_record_the_event_as_applied() {
    // The inbox row shares the projection transaction, so a projection that fails
    // must leave the event unrecorded. Recording it in a separate transaction
    // would mark an event applied that never was, losing it instead of duplicating
    // it. This forces the projection to fail with a check constraint on the post
    // row, then asserts the event can still be applied once the cause is removed.
    let app = crate::support::start_postgres_redis_app(false)
        .await
        .expect("app");
    let repository = PgFeedRecommendationRepository::new(app.pool.clone());
    let actor_id = uuid("11111111-1111-1111-1111-111111111114");
    let author_id = uuid("22222222-2222-2222-2222-222222222224");
    let post_id = uuid("33333333-3333-3333-3333-333333333335");
    let event_id = uuid("44444444-4444-4444-4444-444444444447");
    sqlx::query(
        "insert into posts (id, author_id, content, visibility, created_at, comment_count, like_count) values ($1, $2, $3, 'PUBLIC', $4, 0, 0)",
    )
    .bind(post_id)
    .bind(author_id)
    .bind("Failing projection post")
    .bind(Utc::now())
    .execute(&app.pool)
    .await
    .expect("post");

    // A deferred-free check on the projection target makes the update fail after
    // the inbox insert in the same transaction.
    sqlx::query(
        "alter table post_engagement_velocity add constraint reject_replay_probe check (post_id <> '33333333-3333-3333-3333-333333333335')",
    )
    .execute(&app.pool)
    .await
    .expect("constraint");

    let result = repository
        .record_engagement(
            event_id,
            post_id,
            actor_id,
            author_id,
            FeedEngagementAction::Like,
            Utc::now(),
        )
        .await;
    assert!(result.is_err(), "the projection must report its failure");

    let recorded: i64 = sqlx::query_scalar(
        "select count(*) from feed_projection_inbox where consumer = 'feed-engagement' and event_id = $1",
    )
    .bind(event_id)
    .fetch_one(&app.pool)
    .await
    .expect("inbox count");
    assert_eq!(
        recorded, 0,
        "a failed projection must not be recorded as applied"
    );

    sqlx::query("alter table post_engagement_velocity drop constraint reject_replay_probe")
        .execute(&app.pool)
        .await
        .expect("drop constraint");
    repository
        .record_engagement(
            event_id,
            post_id,
            actor_id,
            author_id,
            FeedEngagementAction::Like,
            Utc::now(),
        )
        .await
        .expect("retry after the cause is removed");
    assert_eq!(velocity(&app.pool, post_id).await, (1, 0));
}
