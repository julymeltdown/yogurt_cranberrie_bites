//! HTTP/port regressions. The shared harness uses a lazy pool, not real persistence.
#[allow(dead_code)]
#[path = "unit/http_support.rs"]
mod http_support;

use api_gateway::adapters::r#in::http::{self, verification_routes};
use api_gateway::application::dto::verification_dto::{
    ResendVerificationRequest, ResendVerificationResponse,
};
use api_gateway::application::verification::{VerificationClientPort, VerificationError};
use api_gateway::config::{AppState, InMemoryRateLimiter};
use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::extract::ConnectInfo;
use axum::http::{header, Request, StatusCode};
use axum::middleware::from_fn;
use axum::Router;
use serde_json::{json, Value};
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower::ServiceExt;

#[derive(Clone)]
struct VerificationDouble {
    calls: Arc<Mutex<Vec<String>>>,
    error: Option<VerificationError>,
    delay: Duration,
}

impl Default for VerificationDouble {
    fn default() -> Self {
        Self {
            calls: Arc::new(Mutex::new(Vec::new())),
            error: None,
            delay: Duration::ZERO,
        }
    }
}

#[async_trait]
impl VerificationClientPort for VerificationDouble {
    async fn resend_verification(
        &self,
        request: ResendVerificationRequest,
    ) -> Result<ResendVerificationResponse, VerificationError> {
        self.calls.lock().unwrap().push(request.email);
        tokio::time::sleep(self.delay).await;
        match self.error {
            Some(error) => Err(error),
            None => Ok(ResendVerificationResponse::accepted()),
        }
    }
}

async fn state(client: VerificationDouble) -> AppState {
    state_with_pool(client).await.0
}

/// The state and the lazy pool behind its persistence ports.
async fn state_with_pool(client: VerificationDouble) -> (AppState, sqlx::PgPool) {
    let harness = http_support::RouteHarness::new().await;
    let mut state = harness.state.as_ref().clone();
    state.auth_service = state
        .auth_service
        .with_verification_client(Arc::new(client));
    (state, harness.pool.clone())
}

fn isolated_router(state: AppState) -> Router {
    verification_routes::router(Arc::new(state))
        .layer(from_fn(verification_routes::protect_response))
}

fn request(body: impl Into<Body>) -> Request<Body> {
    Request::builder()
        .method("POST")
        .uri(verification_routes::PATH)
        .header(header::CONTENT_TYPE, "application/json")
        .header("x-correlation-id", "verification-http-test")
        .body(body.into())
        .unwrap()
}

async fn checked_body(response: axum::response::Response, expected: StatusCode) -> Value {
    assert_eq!(response.status(), expected);
    assert_eq!(response.headers()[header::CACHE_CONTROL], "no-store");
    assert!(response.headers().contains_key("x-correlation-id"));
    assert!(!response.headers().contains_key(header::SET_COOKIE));
    let bytes = to_bytes(response.into_body(), 64 * 1024).await.unwrap();
    serde_json::from_slice(&bytes).unwrap()
}

#[tokio::test]
async fn public_resend_normalizes_email_and_returns_only_accepted() {
    let client = VerificationDouble::default();
    let app = isolated_router(state(client.clone()).await);
    let response = app
        .oneshot(request(r#"{"email":"  USER@Example.COM  "}"#))
        .await
        .unwrap();
    let body = checked_body(response, StatusCode::ACCEPTED).await;
    assert_eq!(body, json!({"status":"ACCEPTED"}));
    assert_eq!(*client.calls.lock().unwrap(), vec!["user@example.com"]);
}

#[tokio::test]
async fn invalid_input_is_rejected_without_a_transport_call() {
    let client = VerificationDouble::default();
    let app = isolated_router(state(client.clone()).await);
    for email in ["", "not-an-email", "aa@localhost", "aa\n@example.com"] {
        let response = app
            .clone()
            .oneshot(request(json!({"email":email}).to_string()))
            .await
            .unwrap();
        assert_eq!(
            checked_body(response, StatusCode::BAD_REQUEST).await["message"],
            "Invalid email"
        );
    }
    assert!(client.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn typed_failures_never_appear_as_successful_delivery() {
    for (error, status) in [
        (VerificationError::InvalidEmail, StatusCode::BAD_REQUEST),
        (
            VerificationError::Unavailable,
            StatusCode::SERVICE_UNAVAILABLE,
        ),
        (VerificationError::Timeout, StatusCode::GATEWAY_TIMEOUT),
        (VerificationError::InvalidResponse, StatusCode::BAD_GATEWAY),
        (
            VerificationError::Internal,
            StatusCode::INTERNAL_SERVER_ERROR,
        ),
    ] {
        let client = VerificationDouble {
            error: Some(error),
            ..Default::default()
        };
        let response = isolated_router(state(client.clone()).await)
            .oneshot(request(r#"{"email":"user@example.com"}"#))
            .await
            .unwrap();
        let body = checked_body(response, status).await;
        assert_eq!(body["message"], error.to_string());
        assert_eq!(body["code"], format!("GATEWAY_{}", status.as_str()));
        assert!(body.get("status").is_none());
        assert_eq!(client.calls.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn missing_dependency_fails_closed() {
    let harness = http_support::RouteHarness::new().await;
    let response = isolated_router(harness.state.as_ref().clone())
        .oneshot(request(r#"{"email":"user@example.com"}"#))
        .await
        .unwrap();
    checked_body(response, StatusCode::SERVICE_UNAVAILABLE).await;
}

#[tokio::test]
async fn malformed_unknown_and_oversized_bodies_use_safe_envelopes() {
    let client = VerificationDouble::default();
    let app = isolated_router(state(client.clone()).await);
    for (body, status) in [
        ("{".to_owned(), StatusCode::BAD_REQUEST),
        (
            r#"{"email":123}"#.to_owned(),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            r#"{"email":"user@example.com","userId":"injected"}"#.to_owned(),
            StatusCode::UNPROCESSABLE_ENTITY,
        ),
        (
            json!({"email":"x".repeat(5000)}).to_string(),
            StatusCode::PAYLOAD_TOO_LARGE,
        ),
    ] {
        let response = app.clone().oneshot(request(body)).await.unwrap();
        let body = checked_body(response, status).await;
        assert!(!body["message"].as_str().unwrap().contains("injected"));
    }
    let response = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(verification_routes::PATH)
                .body(Body::from(r#"{"email":"user@example.com"}"#))
                .unwrap(),
        )
        .await
        .unwrap();
    checked_body(response, StatusCode::UNSUPPORTED_MEDIA_TYPE).await;
    assert!(client.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn global_socket_quota_stops_resend_before_the_client() {
    let client = VerificationDouble::default();
    let (mut state, pool) = state_with_pool(client.clone()).await;
    pool.close().await; // No real persistence is claimed by this test.
    state.rate_limiter = InMemoryRateLimiter::new(1, Duration::from_secs(60));
    assert!(state
        .rate_limiter
        .admit(
            api_gateway::domain::admission::RouteClass::VerificationResend,
            "203.0.113.10"
        )
        .await
        .is_admitted());
    let app = http::router(Arc::new(state));
    let mut request = request(r#"{"email":"user@example.com"}"#);
    request.extensions_mut().insert(ConnectInfo(
        "203.0.113.10:12345".parse::<SocketAddr>().unwrap(),
    ));
    request
        .headers_mut()
        .insert("x-forwarded-for", "198.51.100.99".parse().unwrap());
    let response = app.oneshot(request).await.unwrap();
    checked_body(response, StatusCode::TOO_MANY_REQUESTS).await;
    assert!(client.calls.lock().unwrap().is_empty());
}

#[tokio::test]
async fn global_deadline_still_has_no_store_and_correlation_headers() {
    let client = VerificationDouble {
        delay: Duration::from_millis(200),
        ..Default::default()
    };
    let (mut state, pool) = state_with_pool(client).await;
    pool.close().await;
    state.request_timeout = Duration::from_millis(10);
    let response = http::router(Arc::new(state))
        .oneshot(request(r#"{"email":"user@example.com"}"#))
        .await
        .unwrap();
    assert_eq!(
        response.headers()["x-correlation-id"],
        "verification-http-test"
    );
    checked_body(response, StatusCode::GATEWAY_TIMEOUT).await;
}

#[tokio::test]
async fn invalid_correlation_is_replaced_with_a_bounded_value() {
    let app = isolated_router(state(VerificationDouble::default()).await);
    let mut request = request(r#"{"email":"user@example.com"}"#);
    request
        .headers_mut()
        .insert("x-correlation-id", "x".repeat(129).parse().unwrap());
    let response = app.oneshot(request).await.unwrap();
    let correlation = response.headers()["x-correlation-id"].to_str().unwrap();
    uuid::Uuid::parse_str(correlation).unwrap();
    checked_body(response, StatusCode::ACCEPTED).await;
}
