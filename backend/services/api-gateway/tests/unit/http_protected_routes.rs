use crate::http_support::{
    response_json, DeletePostCall, FeedRecommendationEventCall, RelationshipCall, RouteHarness,
    OTHER_USER_ID, USER_ID,
};
use api_gateway::adapters::r#in::http::{friend_routes, post_routes, profile_routes};
use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn protected_post_route_without_bearer_returns_unauthorized() {
    let harness = RouteHarness::new().await;
    let app = post_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/feed")
        .body(Body::empty())
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let body = response_json(response).await;
    assert_eq!(body["message"], "Missing bearer token");
}

#[tokio::test]
async fn protected_post_route_accepts_case_insensitive_bearer_scheme() {
    let harness = RouteHarness::new().await;
    let app = post_routes::router(harness.state.clone());
    let access_token = harness
        .bearer_token(USER_ID)
        .trim_start_matches("Bearer ")
        .to_owned();
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/feed")
        .header(header::AUTHORIZATION, format!("bEaReR {access_token}  "))
        .body(Body::empty())
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::OK);
}

#[tokio::test]
async fn profile_search_rejects_a_cursor_from_another_query_with_http_400() {
    let harness = RouteHarness::new().await;
    let cursor = harness
        .state
        .search_cursor_codec
        .encode(
            Some("hana_ani"),
            "original",
            Some(USER_ID),
            api_gateway::application::search_cursor::SearchSort::ProfilesUsernameAscV1,
        )
        .expect("encode public cursor")
        .expect("continuation cursor");
    let app = profile_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/profile/search?q=different&cursor={cursor}"))
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .body(Body::empty())
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let body = response_json(response).await;
    assert_eq!(body["code"], "GATEWAY_400");
    assert_eq!(body["message"], "Invalid search cursor");
}

#[tokio::test]
async fn profile_update_for_different_user_returns_forbidden() {
    let harness = RouteHarness::new().await;
    let app = profile_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::PUT)
        .uri(format!("/api/profile/{OTHER_USER_ID}"))
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({
                "username": "mira_ghibli",
                "nickname": "Mira",
                "avatarUrl": "/avatars/mira.png",
                "bio": "not allowed"
            })
            .to_string(),
        ))
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let body = response_json(response).await;
    assert_eq!(body["code"], "GATEWAY_403");
    assert_eq!(body["message"], "Not allowed");
}

#[tokio::test]
async fn post_delete_forwards_request_context_headers() {
    let harness = RouteHarness::new().await;
    let app = post_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::DELETE)
        .uri("/api/posts/post-1")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header("x-request-id", "req-123")
        .header("x-forwarded-for", "198.51.100.8")
        .header(header::USER_AGENT, "route-test")
        .body(Body::empty())
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["deleted"], true);
    assert_eq!(
        *harness
            .clients
            .delete_post_calls
            .lock()
            .expect("delete post calls"),
        vec![DeletePostCall {
            user_id: USER_ID.into(),
            post_id: "post-1".into(),
            request_id: Some("req-123".into()),
            ip_address: Some("198.51.100.8".into()),
            user_agent: Some("route-test".into()),
        }]
    );
}

#[tokio::test]
async fn malformed_avatar_content_type_maps_to_internal_gateway_error() {
    let harness = RouteHarness::new().await;
    harness.clients.avatar_content_type("image/png\nbad");
    let app = profile_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::GET)
        .uri(format!("/api/profile/avatar/{USER_ID}"))
        .body(Body::empty())
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = response_json(response).await;
    assert_eq!(body["code"], "GATEWAY_500");
    assert_eq!(body["message"], "Internal error");
}

#[tokio::test]
async fn relationship_routes_use_the_authenticated_user_as_viewer() {
    let harness = RouteHarness::new().await;
    let app = friend_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/relationships/blocks/{OTHER_USER_ID}"))
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .body(Body::empty())
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["blockedByViewer"], true);
    assert_eq!(
        *harness.clients.relationship_calls.lock().unwrap(),
        vec![RelationshipCall {
            operation: "block".into(),
            viewer_id: USER_ID.into(),
            target_id: OTHER_USER_ID.into(),
        }]
    );
}

#[tokio::test]
async fn ranked_feed_route_forwards_request_id_and_returns_rank_metadata() {
    let harness = RouteHarness::new().await;
    let app = post_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::GET)
        .uri("/api/feed?limit=10")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header("x-request-id", "feed-request-1")
        .body(Body::empty())
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["requestId"], "feed-request-1");
    assert_eq!(body["rankVersion"], "feed-deterministic-v1");
    assert_eq!(body["fallbackUsed"], false);
    assert_eq!(body["recommendationMetadata"][0]["postId"], "post-1");
}

#[tokio::test]
async fn feed_event_route_binds_authenticated_viewer_and_request_context() {
    let harness = RouteHarness::new().await;
    let app = post_routes::router(harness.state.clone());
    let request = Request::builder()
        .method(Method::POST)
        .uri("/api/feed/events")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-request-id", "feed-event-request-1")
        .body(Body::from(
            json!({
                "postId": "post-1",
                "impressionId": "22222222-2222-2222-2222-222222222222",
                "action": "LIKE",
                "source": "MATERIALIZED",
                "modelVersion": "feed-deterministic-v1",
                "position": 0,
                "propensityMicros": 1000000,
                "metadata": {"surface": "home"}
            })
            .to_string(),
        ))
        .expect("request");

    let response = app.oneshot(request).await.expect("response");

    assert_eq!(response.status(), StatusCode::OK);
    let body = response_json(response).await;
    assert_eq!(body["accepted"], true);
    assert_eq!(
        *harness
            .clients
            .feed_recommendation_event_calls
            .lock()
            .unwrap(),
        vec![FeedRecommendationEventCall {
            viewer_id: USER_ID.into(),
            request_id: "feed-event-request-1".into(),
            post_id: "post-1".into(),
            impression_id: "22222222-2222-2222-2222-222222222222".into(),
            action: "LIKE".into(),
        }]
    );
}
