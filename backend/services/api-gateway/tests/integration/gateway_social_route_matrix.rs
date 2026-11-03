use axum::http::Method;
use serde_json::json;

use crate::support::http::json_request_ok;

const USER_ID: &str = "11111111-1111-1111-1111-111111111111";
const OTHER_USER_ID: &str = "22222222-2222-2222-2222-222222222222";

#[tokio::test]
async fn friend_notification_and_post_routes_cover_query_and_mutation_contracts() {
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let token = crate::support::bearer_token(USER_ID, &["ROLE_USER"]);

    let (_, follow) = json_request_ok(
        &app,
        Method::POST,
        "/api/follows",
        Some(&token),
        &[],
        Some(json!({"targetUserId":OTHER_USER_ID})),
    )
    .await;
    assert_eq!(follow["following"], true);

    let (_, followers) = json_request_ok(
        &app,
        Method::GET,
        "/api/follows/followers?cursor=followers-next&limit=2",
        Some(&token),
        &[],
        None,
    )
    .await;
    assert_eq!(followers["edges"][0]["userId"], OTHER_USER_ID);
    assert_eq!(followers["nextCursor"], "followers-next");

    let (_, following) = json_request_ok(
        &app,
        Method::GET,
        &format!("/api/follows/following?user_id={USER_ID}&limit=2"),
        Some(&token),
        &[],
        None,
    )
    .await;
    assert_eq!(
        following["edges"][0]["userId"],
        "33333333-3333-3333-3333-333333333333"
    );

    let (_, counts) = json_request_ok(
        &app,
        Method::GET,
        "/api/follows/counts",
        Some(&token),
        &[],
        None,
    )
    .await;
    assert_eq!(counts["followerCount"], 12);

    let (_, status) = json_request_ok(
        &app,
        Method::GET,
        &format!("/api/follows/status?target_user_id={OTHER_USER_ID}"),
        Some(&token),
        &[],
        None,
    )
    .await;
    assert_eq!(status["following"], true);

    let (_, unfollow) = json_request_ok(
        &app,
        Method::DELETE,
        &format!("/api/follows/{OTHER_USER_ID}"),
        Some(&token),
        &[],
        None,
    )
    .await;
    assert_eq!(unfollow["following"], false);

    let (_, notifications) = json_request_ok(
        &app,
        Method::GET,
        "/api/notifications?cursor=notif-next&limit=5",
        Some(&token),
        &[],
        None,
    )
    .await;
    assert_eq!(notifications["items"][0]["id"], "notif-1");
    let notification_payload: serde_json::Value = serde_json::from_str(
        notifications["items"][0]["payloadJson"]
            .as_str()
            .expect("notification payload"),
    )
    .expect("payload JSON");
    assert_eq!(
        notification_payload["postId"],
        "aaaaaaaa-aaaa-4aaa-8aaa-aaaaaaaaaaaa"
    );
    assert_eq!(notifications["nextCursor"], "notif-next");

    let (_, read_all) = json_request_ok(
        &app,
        Method::PATCH,
        "/api/notifications/read-all",
        Some(&token),
        &[],
        None,
    )
    .await;
    assert_eq!(read_all["updatedCount"], 2);

    let (_, read_one) = json_request_ok(
        &app,
        Method::PATCH,
        "/api/notifications/notif-1/read",
        Some(&token),
        &[],
        None,
    )
    .await;
    assert_eq!(read_one["readAt"], "2026-03-12T00:01:00Z");

    assert_feed(&app, &token, "/api/feed?cursor=feed-next&limit=3", "post-1").await;
    assert_feed(
        &app,
        &token,
        &format!("/api/posts/author/{OTHER_USER_ID}?cursor=author-next&limit=3"),
        "post-author",
    )
    .await;
    assert_feed(
        &app,
        &token,
        "/api/posts/search?q=rust&limit=3",
        "post-search",
    )
    .await;
    assert_feed(&app, &token, "/api/posts/trending?limit=3", "post-trending").await;

    let (_, post_detail) = json_request_ok(
        &app,
        Method::GET,
        "/api/posts/post-1",
        Some(&token),
        &[],
        None,
    )
    .await;
    assert_eq!(post_detail["comments"][0]["id"], "comment-1");

    let (_, comment) = json_request_ok(
        &app,
        Method::POST,
        "/api/posts/post-1/comments",
        Some(&token),
        &[],
        Some(json!({"content":"Nice"})),
    )
    .await;
    assert_eq!(comment["id"], "comment-new");

    let (_, deleted_post) = json_request_ok(
        &app,
        Method::DELETE,
        "/api/posts/post-1",
        Some(&token),
        &[
            ("x-correlation-id", "post-delete-1"),
            ("x-forwarded-for", "203.0.113.21"),
        ],
        None,
    )
    .await;
    assert_eq!(deleted_post["deleted"], true);
    app.shutdown().await.expect("app shutdown");
}

async fn assert_feed(
    app: &crate::support::TestApp,
    token: &str,
    uri: &str,
    expected_post_id: &str,
) {
    let (_, feed) = json_request_ok(app, Method::GET, uri, Some(token), &[], None).await;
    assert_eq!(feed["posts"][0]["id"], expected_post_id);
}
