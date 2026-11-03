use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn gateway_public_routes_preserve_cookie_auth_and_payload_shapes() {
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let user_token =
        crate::support::bearer_token("11111111-1111-1111-1111-111111111111", &["ROLE_USER"]);

    let avatar = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/profile/avatar/11111111-1111-1111-1111-111111111111")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(avatar.status(), StatusCode::OK);
    assert_eq!(
        avatar
            .headers()
            .get("content-type")
            .and_then(|value| value.to_str().ok()),
        Some("image/png")
    );

    let create_post = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/posts")
                .header("authorization", format!("Bearer {user_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"content":"hello rust","visibility":"PUBLIC"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(create_post.status(), StatusCode::OK);
    let create_post_body = create_post
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let create_post_json: serde_json::Value =
        serde_json::from_slice(&create_post_body).expect("json");
    assert_eq!(
        create_post_json["authorId"],
        "11111111-1111-1111-1111-111111111111"
    );

    let like = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/api/posts/post-1/likes")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(like.status(), StatusCode::OK);

    let unlike = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::DELETE)
                .uri("/api/posts/post-1/likes")
                .header("authorization", format!("Bearer {user_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(unlike.status(), StatusCode::OK);

    let aggregate_first = app
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
    let aggregate_first_body = aggregate_first
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let aggregate_first_json: serde_json::Value =
        serde_json::from_slice(&aggregate_first_body).expect("json");
    assert_eq!(aggregate_first_json["cached"], false);

    let aggregate_second = app
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
    let aggregate_second_body = aggregate_second
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let aggregate_second_json: serde_json::Value =
        serde_json::from_slice(&aggregate_second_body).expect("json");
    assert_eq!(aggregate_second_json["cached"], true);
    app.shutdown().await.expect("app shutdown");
}
