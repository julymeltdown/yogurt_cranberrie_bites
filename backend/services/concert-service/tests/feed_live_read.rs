//! Adapter integration fixtures are not live owner or HTTP acceptance.
mod feed_live_read_support;
use concert_application::feed::{Feed, UnavailableFeed};
use concert_domain::feed::{FeedError, Filter};
use concert_service::feed::{
    cursor::CursorCodec,
    live_read::{CommentsQuery, FeedQuery, ReadFeed},
};
use feed_live_read_support::*;
use std::sync::{atomic::Ordering, Arc};
use std::time::Duration;
use uuid::Uuid;

#[tokio::test]
async fn unavailable_owner_never_becomes_an_empty_success() {
    let service = ReadFeed::new(
        Feed {
            reader: UnavailableFeed,
            policy: UnavailableFeed,
            resources: UnavailableFeed,
            clock: WallClock,
        },
        CursorCodec::new(&[42; 32]).unwrap(),
    );
    assert_eq!(
        service.page(context(), FeedQuery::default()).await,
        Err(FeedError::DependencyUnavailable)
    );
}
#[tokio::test]
async fn filtered_empty_page_keeps_opaque_cursor_bound_to_actor_event_filter_limit_and_before() {
    let context = context();
    let state = State::new(context);
    state.more.store(true, Ordering::SeqCst);
    let service = facade(state.clone());
    let first = service.page(context, FeedQuery::default()).await.unwrap();
    assert_eq!(first["items"], serde_json::json!([]));
    let cursor = first["next_cursor"].as_str().unwrap().to_owned();
    assert!(!cursor.contains(&state.post.id.to_string()));
    let first_window = state.last_window.lock().unwrap().unwrap();
    state.more.store(false, Ordering::SeqCst);
    service
        .page(
            context,
            FeedQuery {
                cursor: Some(cursor.clone()),
                ..Default::default()
            },
        )
        .await
        .unwrap();
    let second_window = state.last_window.lock().unwrap().unwrap();
    assert_eq!(first_window.upper, second_window.upper);
    assert_eq!(second_window.after, Some(state.post.position()));
    let calls = state.calls.load(Ordering::SeqCst);
    for (ctx, query) in [
        (
            concert_application::feed::Context {
                actor: Uuid::from_u128(99),
                ..context
            },
            FeedQuery {
                cursor: Some(cursor.clone()),
                ..Default::default()
            },
        ),
        (
            concert_application::feed::Context {
                event: Uuid::from_u128(99),
                ..context
            },
            FeedQuery {
                cursor: Some(cursor.clone()),
                ..Default::default()
            },
        ),
        (
            context,
            FeedQuery {
                cursor: Some(cursor.clone()),
                filter: Some(Filter::PartyLink),
                ..Default::default()
            },
        ),
        (
            context,
            FeedQuery {
                cursor: Some(cursor.clone()),
                limit: Some(21),
                ..Default::default()
            },
        ),
        (
            context,
            FeedQuery {
                cursor: Some(cursor),
                before: Some(first_window.upper),
                ..Default::default()
            },
        ),
    ] {
        assert_eq!(service.page(ctx, query).await, Err(FeedError::InvalidInput));
    }
    assert_eq!(state.calls.load(Ordering::SeqCst), calls);
}
#[tokio::test]
async fn detail_uses_current_author_and_comments_have_exact_parent_cursor_binding() {
    let context = context();
    let state = State::new(context);
    state.more.store(true, Ordering::SeqCst);
    let service = facade(state.clone());
    let detail = service.detail(context, state.post.id).await.unwrap();
    assert_eq!(detail["post"]["text"], "Current owner text");
    assert_eq!(
        detail["post"]["author"]["user_id"],
        state.post.author.to_string()
    );
    assert_eq!(detail["comments"][0]["post_id"], state.post.id.to_string());
    let cursor = detail["next_cursor"].as_str().unwrap();
    assert_eq!(
        service
            .comments(
                context,
                Uuid::from_u128(999),
                CommentsQuery {
                    cursor: Some(cursor.into()),
                    ..Default::default()
                }
            )
            .await,
        Err(FeedError::InvalidInput)
    );
    assert_eq!(
        service
            .page(
                context,
                FeedQuery {
                    cursor: Some(cursor.into()),
                    ..Default::default()
                }
            )
            .await,
        Err(FeedError::InvalidInput)
    );
    state.more.store(false, Ordering::SeqCst);
    // The fixture reader intentionally ignores keysets. Application validation
    // rejects the repeated tuple instead of emitting a duplicate comment page.
    assert_eq!(
        service
            .comments(
                context,
                state.post.id,
                CommentsQuery {
                    cursor: Some(cursor.into()),
                    ..Default::default()
                }
            )
            .await,
        Err(FeedError::DependencyUnavailable)
    );
}
#[tokio::test]
async fn missing_or_private_author_and_deleted_parent_fail_closed() {
    let context = context();
    let state = State::new(context);
    state.with_row.store(true, Ordering::SeqCst);
    let service = facade(state.clone());
    state.missing_author.store(true, Ordering::SeqCst);
    assert_eq!(
        service.page(context, FeedQuery::default()).await,
        Err(FeedError::DependencyUnavailable)
    );
    state.missing_author.store(false, Ordering::SeqCst);
    state.invalid_author.store(true, Ordering::SeqCst);
    assert_eq!(
        service.detail(context, state.post.id).await,
        Err(FeedError::DependencyUnavailable)
    );
    state.invalid_author.store(false, Ordering::SeqCst);
    state.delete_parent.store(true, Ordering::SeqCst);
    assert_eq!(
        service
            .comments(context, state.post.id, CommentsQuery::default())
            .await,
        Err(FeedError::ResourceUnavailable)
    );
}
#[tokio::test]
async fn denied_author_is_filtered_without_losing_scan_continuation() {
    let context = context();
    let state = State::new(context);
    state.with_row.store(true, Ordering::SeqCst);
    state.more.store(true, Ordering::SeqCst);
    state.deny_author.store(true, Ordering::SeqCst);
    let value = facade(state)
        .page(context, FeedQuery::default())
        .await
        .unwrap();
    assert_eq!(value["items"], serde_json::json!([]));
    assert!(value["next_cursor"].is_string());
}
#[tokio::test]
async fn original_deadline_drops_pending_owner_read() {
    let mut context = context();
    context.expires_at = chrono::Utc::now() + chrono::Duration::milliseconds(40);
    let state = State::new(context);
    state.hang.store(true, Ordering::SeqCst);
    let result = facade(state.clone())
        .page(context, FeedQuery::default())
        .await;
    assert_eq!(result, Err(FeedError::DependencyUnavailable));
    assert!(state.dropped.load(Ordering::SeqCst));
}
#[tokio::test]
async fn caller_cancellation_drops_pending_owner_read_before_deadline() {
    let context = context();
    let state = State::new(context);
    state.hang.store(true, Ordering::SeqCst);
    let service = Arc::new(facade(state.clone()));
    let task = tokio::spawn(async move { service.page(context, FeedQuery::default()).await });
    tokio::time::timeout(Duration::from_secs(1), state.started.notified())
        .await
        .unwrap();
    task.abort();
    assert!(task.await.unwrap_err().is_cancelled());
    assert!(state.dropped.load(Ordering::SeqCst));
}
#[tokio::test]
async fn invalid_limit_and_future_before_do_not_call_owner() {
    let context = context();
    let state = State::new(context);
    let service = facade(state.clone());
    for query in [
        FeedQuery {
            limit: Some(0),
            ..Default::default()
        },
        FeedQuery {
            limit: Some(101),
            ..Default::default()
        },
        FeedQuery {
            before: Some(chrono::Utc::now() + chrono::Duration::seconds(1)),
            ..Default::default()
        },
    ] {
        assert_eq!(
            service.page(context, query).await,
            Err(FeedError::InvalidInput)
        );
    }
    assert_eq!(state.calls.load(Ordering::SeqCst), 0);
}
