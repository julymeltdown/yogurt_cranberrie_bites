use auth_service::application::ports::OAuthProviderMetricsPort;
use auth_service::config::AuthMetrics;
use auth_service::domain::user::IdentityProvider;
use axum::body::Body;
use axum::http::StatusCode;
use axum::response::IntoResponse;
use http_body_util::BodyExt;
use platform::edge::Edge;

#[test]
fn lifecycle_deleted_record_metrics_escape_labels_and_clamp_negative_counts() {
    let metrics = AuthMetrics::default();

    metrics.record_lifecycle_cleanup_deleted_records("token\"type\\\n", "failure", -9);

    assert_eq!(
        metrics.render_prometheus(),
        "auth_lifecycle_cleanup_deleted_records_total{record_type=\"token\\\"type\\\\\\n\",outcome=\"failure\"} 0\n"
    );
}

#[test]
fn oauth_provider_metrics_use_provider_and_outcome_labels() {
    let metrics = AuthMetrics::default();

    metrics.record_provider_exchange((IdentityProvider::Email).into(), "success");
    metrics.record_provider_exchange((IdentityProvider::Google).into(), "failure");
    metrics.record_provider_exchange((IdentityProvider::Apple).into(), "success");

    let rendered = metrics.render_prometheus();
    assert!(rendered.contains(
        "auth_oauth_provider_exchanges_total{provider=\"email\",outcome=\"success\"} 1\n"
    ));
    assert!(rendered.contains(
        "auth_oauth_provider_exchanges_total{provider=\"google\",outcome=\"failure\"} 1\n"
    ));
    assert!(rendered.contains(
        "auth_oauth_provider_exchanges_total{provider=\"apple\",outcome=\"success\"} 1\n"
    ));
}

#[test]
fn auth_metrics_export_shared_internal_edge_deadline_counts() {
    let metrics = AuthMetrics::default();
    metrics
        .edge_deadline_metrics()
        .record_deadline_exhausted(Edge::AuthNotification);

    assert!(metrics
        .render_prometheus()
        .contains("grpc_edge_deadline_exhaustions_total{edge=\"auth->notification\"} 1\n"));
}

#[tokio::test]
async fn bad_request_error_returns_client_safe_json_body() {
    let response = auth_service::adapters::r#in::http::AppError::BadRequest("invalid email".into())
        .into_response();

    let (status, body) = response_parts(response).await;

    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        body,
        serde_json::json!({
            "code": "BAD_REQUEST",
            "message": "invalid email"
        })
    );
}

#[tokio::test]
async fn internal_error_returns_generic_json_body() {
    let response =
        auth_service::adapters::r#in::http::AppError::Internal(anyhow::anyhow!("secret detail"))
            .into_response();

    let (status, body) = response_parts(response).await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        body,
        serde_json::json!({
            "code": "INTERNAL_ERROR",
            "message": "Internal error"
        })
    );
}

async fn response_parts(
    response: axum::response::Response<Body>,
) -> (StatusCode, serde_json::Value) {
    let status = response.status();
    let body = response
        .into_body()
        .collect()
        .await
        .expect("collect response body")
        .to_bytes();
    let json = serde_json::from_slice(&body).expect("json response body");
    (status, json)
}
