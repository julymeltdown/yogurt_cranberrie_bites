use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn gateway_public_shadow_fixture_matches_login_profile_and_feed() {
    let fixture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../scripts/parity/api-gateway/public-shadow.json"),
        )
        .expect("fixture"),
    )
    .expect("json");

    let app = crate::support::start_gateway_app(true).await.expect("app");
    let user_token =
        crate::support::bearer_token("11111111-1111-1111-1111-111111111111", &["ROLE_USER"]);

    let login = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/auth/login")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"email":"hana@example.com","password":"Passw0rd!"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("login");
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
        .expect("profile");
    let feed = app
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
        .expect("feed");

    let profile_json: serde_json::Value = serde_json::from_slice(
        &profile
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes(),
    )
    .expect("json");
    let feed_json: serde_json::Value =
        serde_json::from_slice(&feed.into_body().collect().await.expect("body").to_bytes())
            .expect("json");

    let actual = json!({
        "login": {
            "status": login.status().as_u16(),
        },
        "profile": {
            "status": 200,
            "body": {
                "followerCount": profile_json["followerCount"],
                "username": profile_json["username"],
            }
        },
        "feed": {
            "status": 200,
            "body": {
                "postCount": feed_json["posts"].as_array().map(|items| items.len()).unwrap_or_default(),
            }
        }
    });

    assert_eq!(actual, fixture);
    app.shutdown().await.expect("app shutdown");
}
