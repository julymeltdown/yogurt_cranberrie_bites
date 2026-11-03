use axum::body::Body;
use axum::http::Request;
use http_body_util::BodyExt;
use serde_json::json;
use tower::ServiceExt;

#[tokio::test]
async fn gateway_governance_shadow_fixture_matches_seeded_contracts_and_summary() {
    let fixture: serde_json::Value = serde_json::from_slice(
        &std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../scripts/parity/api-gateway/governance-shadow.json"),
        )
        .expect("fixture"),
    )
    .expect("json");

    let app = crate::support::start_gateway_app(true).await.expect("app");
    let admin_token =
        crate::support::bearer_token("aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa", &["ROLE_ADMIN"]);

    let contracts = app
        .router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/admin/contracts")
                .header("authorization", format!("Bearer {admin_token}"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("contracts");
    // The telemetry write is queued off the request path, so the aggregate is read
    // after the writer has caught up rather than while records are still waiting.
    app.state.telemetry_service.drain().await;
    let summary = app
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
        .expect("summary");

    let contracts_json: serde_json::Value = serde_json::from_slice(
        &contracts
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes(),
    )
    .expect("json");
    let summary_json: serde_json::Value = serde_json::from_slice(
        &summary
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes(),
    )
    .expect("json");

    let actual = json!({
        "contracts": {
            "status": 200,
            "body": {
                "count": contracts_json.as_array().map(|items| items.len()).unwrap_or_default(),
            }
        },
        "summary": {
            "status": 200,
            "body": {
                "totalRequests": summary_json["totalRequests"],
            }
        }
    });

    assert_eq!(actual, fixture);
    app.shutdown().await.expect("app shutdown");
}
