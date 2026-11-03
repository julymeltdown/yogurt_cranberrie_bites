#[path = "support/mod.rs"]
mod support;

use axum::{body::Body, http::Request, http::StatusCode};
use http_body_util::BodyExt;
use tower::ServiceExt;
use uuid::Uuid;

use api_gateway::adapters::r#in::http::{client_identity::TrustedProxies, server};

const ACTOR_ID: &str = "11111111-1111-1111-1111-111111111111";

#[tokio::test]
async fn malformed_owner_mutation_is_durably_rejected_before_dispatch() -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    let router =
        server::with_transport_layers(app.router.clone(), &app.config, TrustedProxies::default());
    let token = support::bearer_token(ACTOR_ID, &["ROLE_USER"]);
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/posts")
                .header("authorization", format!("Bearer {token}"))
                .header("idempotency-key", "bad-json-command")
                .header("content-type", "application/json")
                .body(Body::from("{"))?,
        )
        .await?;
    assert!(response.status().is_client_error());
    let operation_id = Uuid::parse_str(
        response
            .headers()
            .get("x-operation-id")
            .and_then(|value| value.to_str().ok())
            .expect("journaled operation id"),
    )?;

    let recorded: (String, String, String, Option<i32>) = sqlx::query_as(
        "select state, outcome, outcome_evidence, response_status from gateway_request_journal where id = $1",
    )
    .bind(operation_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(recorded.0, "COMPLETED");
    assert_eq!(recorded.1, "REJECTED_BEFORE_COMMIT");
    assert_eq!(recorded.2, "PRE_DISPATCH_REJECTION");
    assert!(recorded
        .3
        .is_some_and(|status| (400..500).contains(&status)));

    let status = router
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/operations/{operation_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(status.status(), StatusCode::OK);
    let body = status.into_body().collect().await?.to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(json["state"], "COMPLETED");
    assert_eq!(json["outcome"], "REJECTED_BEFORE_COMMIT");
    assert!(json.get("ownerReceipt").is_none());

    app.shutdown().await?;
    Ok(())
}

#[tokio::test]
async fn ambiguous_owner_failure_after_dispatch_remains_unknown() -> anyhow::Result<()> {
    let app = support::start_gateway_app(false).await?;
    app.post_client
        .set_create_post_error("simulated owner transport failure");
    let router =
        server::with_transport_layers(app.router.clone(), &app.config, TrustedProxies::default());
    let token = support::bearer_token(ACTOR_ID, &["ROLE_USER"]);
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/posts")
                .header("authorization", format!("Bearer {token}"))
                .header("idempotency-key", "ambiguous-owner-command")
                .header("content-type", "application/json")
                .body(Body::from(
                    r#"{"content":"Owner boundary test","visibility":"PUBLIC"}"#,
                ))?,
        )
        .await?;
    assert!(response.status().is_server_error());
    let operation_id = Uuid::parse_str(
        response
            .headers()
            .get("x-operation-id")
            .and_then(|value| value.to_str().ok())
            .expect("journaled operation id"),
    )?;

    let recorded: (String, String, String) = sqlx::query_as(
        "select state, outcome, outcome_evidence from gateway_request_journal where id = $1",
    )
    .bind(operation_id)
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(
        recorded,
        (
            "UNKNOWN".into(),
            "UNKNOWN".into(),
            "RESPONSE_STATUS_ONLY".into()
        )
    );

    let status = router
        .oneshot(
            Request::builder()
                .uri(format!("/api/operations/{operation_id}"))
                .header("authorization", format!("Bearer {token}"))
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(status.status(), StatusCode::OK);
    let body = status.into_body().collect().await?.to_bytes();
    let json: serde_json::Value = serde_json::from_slice(&body)?;
    assert_eq!(json["state"], "UNKNOWN");
    assert_eq!(json["outcome"], "UNKNOWN");

    app.shutdown().await?;
    Ok(())
}
