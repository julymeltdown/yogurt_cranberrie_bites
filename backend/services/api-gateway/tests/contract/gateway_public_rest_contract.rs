use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn gateway_public_contract_covers_auth_profile_friend_notification_post_and_aggregation() {
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let user_token =
        crate::support::bearer_token("11111111-1111-1111-1111-111111111111", &["ROLE_USER"]);

    let login = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"hana@example.com","password":"Passw0rd!"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(login.status(), StatusCode::OK);
    assert!(login.headers().get("set-cookie").is_some());
    let login_body = login.into_body().collect().await.expect("body").to_bytes();
    let login_json: serde_json::Value = serde_json::from_slice(&login_body).expect("json");
    assert_eq!(login_json["refreshToken"], "");

    let oauth_authorize = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/oauth/google/authorize")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(oauth_authorize.status(), StatusCode::FOUND);
    assert!(oauth_authorize
        .headers()
        .get("location")
        .and_then(|value| value.to_str().ok())
        .expect("location")
        .contains("google-state"));

    let oauth_callback = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/auth/oauth/google/callback?code=google-code&state=google-state")
                .header("cookie", "oauth_state_google=google-state")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(oauth_callback.status(), StatusCode::OK);

    let refresh = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/auth/refresh")
                .header("cookie", "refresh_token=refresh-token")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(refresh.status(), StatusCode::OK);

    let profile = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/profile/11111111-1111-1111-1111-111111111111")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(profile.status(), StatusCode::OK);
    let profile_body = profile
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let profile_json: serde_json::Value = serde_json::from_slice(&profile_body).expect("json");
    assert_eq!(profile_json["followerCount"], 12);

    let delete_profile = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/profile/11111111-1111-1111-1111-111111111111")
                .header("authorization", format!("Bearer {user_token}"))
                .header("x-request-id", "gateway-profile-delete-1")
                .header("x-forwarded-for", "203.0.113.31")
                .header("user-agent", "gateway-public-contract")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(delete_profile.status(), StatusCode::OK);
    let delete_profile_body = delete_profile
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let delete_profile_json: serde_json::Value =
        serde_json::from_slice(&delete_profile_body).expect("json");
    assert_eq!(
        delete_profile_json["userId"],
        "11111111-1111-1111-1111-111111111111"
    );
    assert_eq!(delete_profile_json["deleted"], true);
    assert_eq!(delete_profile_json["accountDeleted"], false);

    let follows = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::GET)
                .uri("/api/follows/counts")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(follows.status(), StatusCode::OK);

    let notifications = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/notifications")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(notifications.status(), StatusCode::OK);

    let post_feed = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/feed")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(post_feed.status(), StatusCode::OK);

    let delete_post = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/posts/post-1")
                .header("authorization", format!("Bearer {user_token}"))
                .header("x-request-id", "gateway-delete-1")
                .header("x-forwarded-for", "203.0.113.21")
                .header("user-agent", "gateway-public-contract")
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(delete_post.status(), StatusCode::OK);
    let delete_body = delete_post
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let delete_json: serde_json::Value = serde_json::from_slice(&delete_body).expect("json");
    assert_eq!(delete_json["postId"], "post-1");
    assert_eq!(delete_json["deleted"], true);

    let aggregation = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/aggregate/feed-summary")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(aggregation.status(), StatusCode::OK);
    app.shutdown().await.expect("app shutdown");
}
