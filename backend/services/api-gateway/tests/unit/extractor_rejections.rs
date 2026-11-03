use api_gateway::adapters::r#in::http::normalize_extractor_rejections;
use axum::body::{to_bytes, Body};
use axum::extract::Query;
use axum::http::{header, Method, Request, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use serde_json::Value;
use tower::ServiceExt;

const CORRELATION_ID: &str = "11111111-2222-3333-4444-555555555555";

#[derive(Deserialize)]
struct CharacterizedJson {
    count: u64,
}

#[derive(Deserialize)]
struct CharacterizedQuery {
    limit: u64,
}

async fn characterized_json(Json(payload): Json<CharacterizedJson>) -> String {
    payload.count.to_string()
}

async fn characterized_query(Query(query): Query<CharacterizedQuery>) -> String {
    query.limit.to_string()
}

fn characterization_router() -> Router {
    Router::new()
        .route("/json", post(characterized_json))
        .route("/query", get(characterized_query))
}

fn normalized_characterization_router() -> Router {
    characterization_router()
        .route(
            "/plain-error",
            get(|| async { (StatusCode::BAD_REQUEST, "plain application response") }),
        )
        .route(
            "/gateway-error",
            get(|| async {
                (
                    StatusCode::BAD_REQUEST,
                    [
                        (header::CONTENT_TYPE, "application/json"),
                        (header::HeaderName::from_static("x-custom"), "kept"),
                    ],
                    r#"{"code":"GATEWAY_400","message":"typed","traceId":"fixed","timestamp":"2026-08-09T00:00:00Z","details":null}"#,
                )
            }),
        )
        .layer(axum::middleware::from_fn(add_characterized_correlation_id))
        .layer(axum::middleware::from_fn(normalize_extractor_rejections))
}

async fn add_characterized_correlation_id(
    request: Request<Body>,
    next: axum::middleware::Next,
) -> Response {
    let mut response = next.run(request).await;
    response.headers_mut().insert(
        "x-correlation-id",
        CORRELATION_ID.parse().expect("correlation header"),
    );
    response
}

async fn assert_normalized_extractor_response(
    request: Request<Body>,
    expected_status: StatusCode,
    expected_message: &str,
) {
    let response = normalized_characterization_router()
        .oneshot(request)
        .await
        .expect("response");

    assert_eq!(response.status(), expected_status);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    assert_eq!(response.headers()["x-correlation-id"], CORRELATION_ID);
    let body = response_json(response).await;
    assert_eq!(
        body["code"],
        format!("GATEWAY_{}", expected_status.as_u16())
    );
    assert_eq!(body["message"], expected_message);
    assert_eq!(body["traceId"], CORRELATION_ID);
    assert!(body["timestamp"].is_string());
    chrono::DateTime::parse_from_rfc3339(body["timestamp"].as_str().expect("timestamp"))
        .expect("RFC 3339 timestamp");
    let serialized = body.to_string();
    assert!(!serialized.contains("serde"));
    assert!(!serialized.contains("Failed to"));
    assert!(!serialized.contains("invalid type"));
}

#[tokio::test]
async fn outer_layer_normalizes_malformed_json_as_safe_gateway_400() {
    // Given: malformed JSON sent through the layered router.
    let request = Request::builder()
        .method(Method::POST)
        .uri("/json")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"count":1"#))
        .expect("request");

    // When/Then: the outer layer emits the fixed public Gateway envelope.
    assert_normalized_extractor_response(
        request,
        StatusCode::BAD_REQUEST,
        "Malformed JSON request body",
    )
    .await;
}

#[tokio::test]
async fn outer_layer_normalizes_missing_json_content_type_as_safe_gateway_415() {
    // Given: valid JSON without its required JSON Content-Type.
    let request = Request::builder()
        .method(Method::POST)
        .uri("/json")
        .body(Body::from(r#"{"count":1}"#))
        .expect("request");

    // When/Then: the outer layer emits the fixed public Gateway envelope.
    assert_normalized_extractor_response(
        request,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "Content-Type must be application/json",
    )
    .await;
}

#[tokio::test]
async fn outer_layer_normalizes_wrong_json_content_type_as_safe_gateway_415() {
    // Given: valid JSON labeled with a non-JSON media type.
    let request = Request::builder()
        .method(Method::POST)
        .uri("/json")
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from(r#"{"count":1}"#))
        .expect("request");

    // When/Then: the outer layer emits the fixed public Gateway envelope.
    assert_normalized_extractor_response(
        request,
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "Content-Type must be application/json",
    )
    .await;
}

#[tokio::test]
async fn outer_layer_normalizes_wrong_json_field_type_as_safe_gateway_422() {
    // Given: valid JSON whose field type does not match the DTO.
    let request = Request::builder()
        .method(Method::POST)
        .uri("/json")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"count":"wrong"}"#))
        .expect("request");

    // When/Then: the outer layer emits the fixed public Gateway envelope.
    assert_normalized_extractor_response(
        request,
        StatusCode::UNPROCESSABLE_ENTITY,
        "JSON request body does not match the expected schema",
    )
    .await;
}

#[tokio::test]
async fn outer_layer_normalizes_invalid_numeric_query_as_safe_gateway_400() {
    // Given: a non-numeric value for a numeric query field.
    let request = Request::builder()
        .uri("/query?limit=wrong")
        .body(Body::empty())
        .expect("request");

    // When/Then: the outer layer emits the fixed public Gateway envelope.
    assert_normalized_extractor_response(
        request,
        StatusCode::BAD_REQUEST,
        "Invalid query parameters",
    )
    .await;
}

#[tokio::test]
async fn outer_layer_preserves_successful_extractor_response_bytes_and_headers() {
    // Given: valid JSON accepted by the same extractor route.
    let request = Request::builder()
        .method(Method::POST)
        .uri("/json")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"count":7}"#))
        .expect("request");

    // When: the request crosses the outer normalization layer.
    let response = normalized_characterization_router()
        .oneshot(request)
        .await
        .expect("response");

    // Then: successful status, media type, and body bytes remain unchanged.
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    assert_eq!(response_text(response).await.as_bytes(), b"7");
}

#[tokio::test]
async fn outer_layer_preserves_non_extractor_plain_error_bytes_and_headers() {
    // Given: an application-owned plain 400 unrelated to extraction.
    let request = Request::builder()
        .uri("/plain-error")
        .body(Body::empty())
        .expect("request");

    // When: the response crosses the outer normalization layer.
    let response = normalized_characterization_router()
        .oneshot(request)
        .await
        .expect("response");

    // Then: its status, content type, and bytes are unchanged.
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    assert_eq!(
        response_text(response).await.as_bytes(),
        b"plain application response"
    );
}

#[tokio::test]
async fn outer_layer_preserves_existing_json_gateway_error_bytes_and_headers() {
    // Given: an already-normalized JSON GatewayError-like response.
    let expected = br#"{"code":"GATEWAY_400","message":"typed","traceId":"fixed","timestamp":"2026-08-09T00:00:00Z","details":null}"#;
    let request = Request::builder()
        .uri("/gateway-error")
        .body(Body::empty())
        .expect("request");

    // When: the response crosses the outer normalization layer.
    let response = normalized_characterization_router()
        .oneshot(request)
        .await
        .expect("response");

    // Then: its status, original headers, exact bytes, and JSON semantics survive.
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
    assert_eq!(response.headers()["x-custom"], "kept");
    let bytes = to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("response body");
    assert_eq!(bytes.as_ref(), expected);
    assert_eq!(
        serde_json::from_slice::<Value>(&bytes).expect("json body")["message"],
        "typed"
    );
}

async fn response_json(response: Response<Body>) -> Value {
    let bytes = to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("response body");
    serde_json::from_slice(&bytes).expect("json body")
}

async fn response_text(response: Response<Body>) -> String {
    let bytes = to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("response body");
    String::from_utf8(bytes.to_vec()).expect("utf-8 body")
}
