use axum::body::Body;
use axum::http::{Method, Request, StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;

#[tokio::test]
async fn gateway_governance_routes_persist_and_report_via_postgres() {
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let admin_token =
        crate::support::bearer_token("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", &["ROLE_ADMIN"]);

    let policy = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::POST)
                .uri("/admin/policies")
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"strict","rateLimit":"30/min","timeoutMs":500,"retryPolicy":"retry-0","cachePolicy":"none"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(policy.status(), StatusCode::CREATED);
    let policy_body = policy.into_body().collect().await.expect("body").to_bytes();
    let policy_json: serde_json::Value = serde_json::from_slice(&policy_body).expect("json");
    let policy_id = policy_json["id"].as_str().expect("id");

    let updated = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .method(Method::PUT)
                .uri(format!("/admin/policies/{policy_id}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"name":"strict","rateLimit":"45/min","timeoutMs":450,"retryPolicy":"retry-1","cachePolicy":"default"}"#,
                ))
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(updated.status(), StatusCode::OK);

    let fetch_policy = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/admin/policies/{policy_id}"))
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(fetch_policy.status(), StatusCode::OK);
    let fetched_body = fetch_policy
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let fetched_json: serde_json::Value = serde_json::from_slice(&fetched_body).expect("json");
    assert_eq!(fetched_json["rateLimit"], "45/min");

    // The telemetry write is queued, so the summary is read after the writer has
    // caught up. Asserting before that would measure the queue's depth rather
    // than the fact that the requests were recorded at all.
    app.state.telemetry_service.drain().await;
    let telemetry = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/telemetry/summary")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("response");
    assert_eq!(telemetry.status(), StatusCode::OK);
    let telemetry_body = telemetry
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let telemetry_json: serde_json::Value = serde_json::from_slice(&telemetry_body).expect("json");
    assert!(telemetry_json["totalRequests"].as_i64().unwrap_or_default() >= 2);
    app.shutdown().await.expect("app shutdown");
}
