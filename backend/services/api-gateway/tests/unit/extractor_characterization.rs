use axum::body::{to_bytes, Body};
use axum::extract::Query;
use axum::http::{header, Method, Request, StatusCode};
use axum::routing::{get, post};
use axum::{Json, Router};
use serde::Deserialize;
use tower::ServiceExt;

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

#[tokio::test]
async fn axum_json_rejection_is_plain_400_when_syntax_is_malformed() {
    // Given: bare Axum JSON extraction with malformed JSON syntax.
    let request = Request::builder()
        .method(Method::POST)
        .uri("/json")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"count":1"#))
        .expect("request");

    // When: Axum runs its framework-provided extractor without Gateway middleware.
    let response = characterization_router()
        .oneshot(request)
        .await
        .expect("response");

    // Then: the raw rejection is a diagnostic-bearing plain-text 400.
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    let body = response_text(response).await;
    assert!(body.starts_with("Failed to parse the request body as JSON:"));
}

#[tokio::test]
async fn axum_json_rejection_is_plain_415_when_content_type_is_missing() {
    // Given: structurally valid JSON without a JSON Content-Type header.
    let request = Request::builder()
        .method(Method::POST)
        .uri("/json")
        .body(Body::from(r#"{"count":1}"#))
        .expect("request");

    // When: Axum runs its framework-provided extractor without Gateway middleware.
    let response = characterization_router()
        .oneshot(request)
        .await
        .expect("response");

    // Then: the raw rejection is a diagnostic-bearing plain-text 415.
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    let body = response_text(response).await;
    assert!(body.starts_with("Expected request with `Content-Type: application/json`"));
}

#[tokio::test]
async fn axum_json_rejection_is_plain_415_when_content_type_is_wrong() {
    // Given: structurally valid JSON labeled as plain text.
    let request = Request::builder()
        .method(Method::POST)
        .uri("/json")
        .header(header::CONTENT_TYPE, "text/plain")
        .body(Body::from(r#"{"count":1}"#))
        .expect("request");

    // When: Axum runs its framework-provided extractor without Gateway middleware.
    let response = characterization_router()
        .oneshot(request)
        .await
        .expect("response");

    // Then: wrong media type has the same diagnostic-bearing plain-text 415 class.
    assert_eq!(response.status(), StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    let body = response_text(response).await;
    assert!(body.starts_with("Expected request with `Content-Type: application/json`"));
}

#[tokio::test]
async fn axum_json_rejection_is_plain_422_when_field_type_is_wrong() {
    // Given: structurally valid JSON whose field type does not match the DTO.
    let request = Request::builder()
        .method(Method::POST)
        .uri("/json")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(r#"{"count":"wrong"}"#))
        .expect("request");

    // When: Axum runs its framework-provided extractor without Gateway middleware.
    let response = characterization_router()
        .oneshot(request)
        .await
        .expect("response");

    // Then: the raw rejection is a diagnostic-bearing plain-text 422.
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    let body = response_text(response).await;
    assert!(body.starts_with("Failed to deserialize the JSON body into the target type:"));
}

#[tokio::test]
async fn axum_query_rejection_is_plain_400_when_numeric_value_is_invalid() {
    // Given: a numeric query parameter containing a non-numeric value.
    let request = Request::builder()
        .uri("/query?limit=wrong")
        .body(Body::empty())
        .expect("request");

    // When: Axum runs its framework-provided extractor without Gateway middleware.
    let response = characterization_router()
        .oneshot(request)
        .await
        .expect("response");

    // Then: the raw rejection is a diagnostic-bearing plain-text 400.
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/plain; charset=utf-8"
    );
    let body = response_text(response).await;
    assert!(body.starts_with("Failed to deserialize query string:"));
}

async fn response_text(response: axum::response::Response) -> String {
    let bytes = to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("response body");
    String::from_utf8(bytes.to_vec()).expect("utf-8 body")
}
