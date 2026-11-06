use axum::http::{Method, StatusCode};
use serde_json::json;
use tower::ServiceExt;

use crate::support::http::{
    json_request, json_request_ok, multipart_avatar_upload_ok, multipart_avatar_upload_with_content,
};

const USER_ID: &str = "11111111-1111-1111-1111-111111111111";
const OTHER_USER_ID: &str = "22222222-2222-2222-2222-222222222222";
const BLOCKED_USER_ID: &str = "33333333-3333-3333-3333-333333333333";
const MUTED_USER_ID: &str = "44444444-4444-4444-4444-444444444444";

#[tokio::test]
async fn profile_routes_cover_lookup_search_update_delete_and_avatar_upload() {
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let token = crate::support::bearer_token(USER_ID, &["ROLE_USER"]);
    app.friend_client
        .set_default_relationship_policy(Default::default());
    app.friend_client.set_relationship_policy(
        USER_ID,
        BLOCKED_USER_ID,
        api_gateway::application::dto::friend_dto::RelationshipPolicyResponse {
            blocked_by_viewer: true,
            ..Default::default()
        },
    );
    app.friend_client.set_relationship_policy(
        USER_ID,
        MUTED_USER_ID,
        api_gateway::application::dto::friend_dto::RelationshipPolicyResponse {
            muted_by_viewer: true,
            ..Default::default()
        },
    );

    let (_, profile) = json_request_ok(
        &app,
        Method::GET,
        &format!("/api/profile/{USER_ID}"),
        Some(&token),
        &[],
        None,
    )
    .await;
    assert_eq!(profile["username"], "hana_ani");

    let (_, by_username) = json_request_ok(
        &app,
        Method::GET,
        "/api/profile/username/hana_ani",
        Some(&token),
        &[],
        None,
    )
    .await;
    assert_eq!(by_username["userId"], USER_ID);

    let (_, batch) = json_request_ok(
        &app,
        Method::POST,
        "/api/profile/batch",
        Some(&token),
        &[],
        Some(json!({"userIds":[USER_ID, OTHER_USER_ID, BLOCKED_USER_ID]})),
    )
    .await;
    assert_eq!(
        batch["profiles"]
            .as_array()
            .expect("profiles")
            .iter()
            .map(|profile| profile["userId"].as_str().expect("profile user ID"))
            .collect::<Vec<_>>(),
        vec![USER_ID, OTHER_USER_ID]
    );

    let wrong_caller_cursor = app
        .state
        .search_cursor_codec
        .encode(
            Some("profiles_next"),
            "muted",
            Some(OTHER_USER_ID),
            api_gateway::application::search_cursor::SearchSort::ProfilesUsernameAscV1,
        )
        .expect("encode a correctly signed cursor for another caller")
        .expect("profile continuation");
    let (status, _, invalid_cursor) = json_request(
        &app,
        Method::GET,
        &format!("/api/profile/search?q=muted&cursor={wrong_caller_cursor}&limit=2"),
        Some(&token),
        &[],
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(invalid_cursor["message"], "Invalid search cursor");

    let search_cursor = app
        .state
        .search_cursor_codec
        .encode(
            Some("profiles_next"),
            "muted",
            Some(USER_ID),
            api_gateway::application::search_cursor::SearchSort::ProfilesUsernameAscV1,
        )
        .expect("encode public profile cursor")
        .expect("profile continuation");
    let (_, search) = json_request_ok(
        &app,
        Method::GET,
        &format!("/api/profile/search?q=muted&cursor={search_cursor}&limit=2"),
        Some(&token),
        &[],
        None,
    )
    .await;
    // The muted profile is the only match and is filtered out, so the page is
    // empty and the scan had nothing left to read. Reporting a continuation here
    // would promise rows that do not exist: the search must distinguish "the
    // results ended" from "the first batch was hidden".
    assert!(search["profiles"].as_array().expect("profiles").is_empty());
    assert_eq!(search["nextCursor"], serde_json::Value::Null);
    assert_eq!(
        app.friend_client.relationship_calls(),
        vec![
            (USER_ID.to_owned(), OTHER_USER_ID.to_owned()),
            (USER_ID.to_owned(), BLOCKED_USER_ID.to_owned()),
            (USER_ID.to_owned(), MUTED_USER_ID.to_owned()),
        ]
    );

    let (_, updated) = json_request_ok(
        &app,
        Method::PUT,
        &format!("/api/profile/{USER_ID}"),
        Some(&token),
        &[],
        Some(json!({
            "username":"hana_ani",
            "nickname":"Hana",
            "avatarUrl":"/avatars/hana.png",
            "bio":"Gateway update"
        })),
    )
    .await;
    assert_eq!(updated["bio"], "Gateway update");

    let (_, uploaded) = multipart_avatar_upload_ok(&app, &token).await;
    assert_eq!(
        uploaded["avatarUrl"],
        format!("/api/profile/avatar/{USER_ID}")
    );

    const MAX_AVATAR_BYTES: usize = 1_048_576;
    let max_size_payload = vec![0u8; MAX_AVATAR_BYTES];
    let (max_size_status, _, _) =
        multipart_avatar_upload_with_content(&app, &token, &max_size_payload).await;
    assert_eq!(max_size_status, StatusCode::OK);
    let calls_after_max_size = app.profile_avatar_upload_calls();
    assert_eq!(calls_after_max_size, 2);

    let oversized_payload = vec![0u8; MAX_AVATAR_BYTES + 1];
    let (oversized_status, _, oversized_body) =
        multipart_avatar_upload_with_content(&app, &token, &oversized_payload).await;
    assert_eq!(oversized_status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(
        oversized_body["message"],
        format!("Avatar image must be <= {MAX_AVATAR_BYTES} bytes")
    );
    assert_eq!(
        app.profile_avatar_upload_calls(),
        calls_after_max_size,
        "the gateway must reject oversized avatar bytes before forwarding to Profile"
    );

    let (delete_status, _, deleted) = json_request(
        &app,
        Method::DELETE,
        &format!("/api/profile/{USER_ID}"),
        Some(&token),
        &[
            ("x-request-id", "profile-delete-1"),
            ("user-agent", "gateway-it"),
        ],
        None,
    )
    .await;
    assert_eq!(delete_status, StatusCode::OK);
    assert_eq!(deleted["deleted"], true);
    assert_eq!(deleted["accountDeleted"], false);
    app.shutdown().await.expect("app shutdown");
}

#[tokio::test]
async fn avatar_reads_are_not_shared_cacheable_and_are_content_sniff_protected() {
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let token = crate::support::bearer_token(USER_ID, &["ROLE_USER"]);

    let response = app
        .router
        .clone()
        .oneshot(
            axum::http::Request::builder()
                .uri(format!("/api/profile/avatar/{USER_ID}"))
                .header("authorization", format!("Bearer {token}"))
                .body(axum::body::Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::OK);

    // The visibility check above is viewer-dependent, so a shared cache must
    // never hold this response: a later block or privacy change would otherwise
    // keep serving the image to viewers who are no longer allowed to see it.
    assert_eq!(
        response.headers()[axum::http::header::CACHE_CONTROL],
        "private, no-store"
    );
    assert_eq!(
        response.headers()[axum::http::header::X_CONTENT_TYPE_OPTIONS],
        "nosniff"
    );
    app.shutdown().await.expect("app shutdown");
}

#[tokio::test]
async fn profile_batch_reports_unavailable_relationship_lookup_without_leaking_internals() {
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let token = crate::support::bearer_token(USER_ID, &["ROLE_USER"]);
    app.friend_client
        .set_relationship_unavailable(USER_ID, OTHER_USER_ID);

    let (status, _, body) = json_request(
        &app,
        Method::POST,
        "/api/profile/batch",
        Some(&token),
        &[],
        Some(json!({"userIds":[USER_ID, OTHER_USER_ID]})),
    )
    .await;

    // A dependency failure is a server fault. It must not be reported as a
    // client error, and the downstream text must not reach the caller.
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(body["code"], "GATEWAY_500");
    assert_eq!(body["message"], "Internal error");
    assert!(!body.to_string().contains("Friend relationship"));
    assert!(body.get("profiles").is_none());
    assert_eq!(
        app.friend_client.relationship_calls(),
        vec![(USER_ID.to_owned(), OTHER_USER_ID.to_owned())]
    );
    app.shutdown().await.expect("app shutdown");
}
