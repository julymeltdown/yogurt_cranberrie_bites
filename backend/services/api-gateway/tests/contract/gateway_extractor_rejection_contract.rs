use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use http_body_util::BodyExt;
use std::path::Path;
use tower::ServiceExt;

#[tokio::test]
async fn assembled_gateway_normalizes_framework_extractor_failures_after_correlation() {
    // Given: the production-assembled router with real persistence and its correlation middleware.
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let cases = [
        (
            Request::builder()
                .method(Method::POST)
                .uri("/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-correlation-id", "extractor-malformed-json")
                .body(Body::from(r#"{"email":"hana@example.com""#))
                .expect("malformed JSON request"),
            StatusCode::BAD_REQUEST,
            "GATEWAY_400",
            "Malformed JSON request body",
            "extractor-malformed-json",
        ),
        (
            Request::builder()
                .method(Method::POST)
                .uri("/auth/login")
                .header("x-correlation-id", "extractor-missing-content-type")
                .body(Body::from(
                    r#"{"email":"hana@example.com","password":"Passw0rd!"}"#,
                ))
                .expect("missing content type request"),
            StatusCode::UNSUPPORTED_MEDIA_TYPE,
            "GATEWAY_415",
            "Content-Type must be application/json",
            "extractor-missing-content-type",
        ),
        (
            Request::builder()
                .method(Method::POST)
                .uri("/auth/login")
                .header(header::CONTENT_TYPE, "application/json")
                .header("x-correlation-id", "extractor-wrong-field-type")
                .body(Body::from(r#"{"email":"hana@example.com","password":42}"#))
                .expect("wrong field type request"),
            StatusCode::UNPROCESSABLE_ENTITY,
            "GATEWAY_422",
            "JSON request body does not match the expected schema",
            "extractor-wrong-field-type",
        ),
        (
            Request::builder()
                .uri("/api/profile/search?q=hana&limit=wrong")
                .header("x-correlation-id", "extractor-invalid-query")
                .body(Body::empty())
                .expect("invalid query request"),
            StatusCode::BAD_REQUEST,
            "GATEWAY_400",
            "Invalid query parameters",
            "extractor-invalid-query",
        ),
    ];

    // When: every public extractor failure class crosses the assembled middleware stack.
    for (request, expected_status, expected_code, expected_message, correlation_id) in cases {
        let response = app.router.clone().oneshot(request).await.expect("response");

        // Then: normalization observes the inner correlation header and hides Axum diagnostics.
        assert_eq!(response.status(), expected_status);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "application/json");
        assert_eq!(response.headers()["x-correlation-id"], correlation_id);
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("body")
            .to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&bytes).expect("JSON body");
        assert_eq!(body["code"], expected_code);
        assert_eq!(body["message"], expected_message);
        assert_eq!(body["traceId"], correlation_id);
        chrono::DateTime::parse_from_rfc3339(body["timestamp"].as_str().expect("timestamp"))
            .expect("RFC 3339 timestamp");
        let public_body = String::from_utf8(bytes.to_vec()).expect("UTF-8 body");
        assert!(!public_body.contains("Failed to"));
        assert!(!public_body.contains("invalid type"));
        assert!(!public_body.contains("serde"));
    }
    app.shutdown().await.expect("app shutdown");
}

#[tokio::test]
async fn assembled_gateway_preserves_happy_extractors_and_typed_gateway_errors() {
    // Given: the production-assembled router and valid JSON/query requests.
    let app = crate::support::start_gateway_app(true).await.expect("app");
    let login_request = Request::builder()
        .method(Method::POST)
        .uri("/auth/login")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            r#"{"email":"hana@example.com","password":"Passw0rd!"}"#,
        ))
        .expect("login request");

    // When: valid JSON crosses the outer normalization layer.
    let login = app
        .router
        .clone()
        .oneshot(login_request)
        .await
        .expect("login response");

    // Then: the handler's successful public response remains intact.
    assert_eq!(login.status(), StatusCode::OK);
    let login_bytes = login.into_body().collect().await.expect("body").to_bytes();
    let login_body: serde_json::Value = serde_json::from_slice(&login_bytes).expect("login JSON");
    assert_eq!(login_body["accessToken"], "access.token");
    assert_eq!(login_body["refreshToken"], "");

    // Given: a valid numeric query and an application-owned typed GatewayError route.
    let query_request = Request::builder()
        .uri("/api/profile/search?q=hana&limit=10")
        .body(Body::empty())
        .expect("query request");
    let typed_error_request = Request::builder()
        .method(Method::POST)
        .uri("/auth/refresh")
        .body(Body::empty())
        .expect("typed error request");

    // When: both responses cross the same outer layer.
    let query = app
        .router
        .clone()
        .oneshot(query_request)
        .await
        .expect("query response");
    let typed_error = app
        .router
        .clone()
        .oneshot(typed_error_request)
        .await
        .expect("typed error response");

    // Then: the query succeeds and the existing GatewayError semantics are preserved.
    assert_eq!(query.status(), StatusCode::OK);
    assert_eq!(typed_error.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        typed_error.headers()[header::CONTENT_TYPE],
        "application/json"
    );
    let typed_error_bytes = typed_error
        .into_body()
        .collect()
        .await
        .expect("body")
        .to_bytes();
    let typed_error_body: serde_json::Value =
        serde_json::from_slice(&typed_error_bytes).expect("typed error JSON");
    assert_eq!(typed_error_body["code"], "GATEWAY_401");
    assert_eq!(typed_error_body["message"], "Missing refresh token");
    app.shutdown().await.expect("app shutdown");
}

#[test]
fn gateway_public_rest_openapi_shares_exact_extractor_error_response_schemas() {
    // Given: the public Gateway OpenAPI contract as the runtime serves it.
    // Read it through the same resolver used for serving so this assertion
    // cannot pass against a differently assembled view of the sources.
    let workspace = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let index = std::fs::read_to_string(workspace.join("docs/api/openapi.json"))
        .expect("checked-in OpenAPI index");
    let core = std::fs::read_to_string(workspace.join("docs/api/openapi-core.json"))
        .expect("checked-in OpenAPI core");
    let bundled = api_contracts::bundle(&index, &core).expect("OpenAPI bundle resolves");
    let openapi: serde_json::Value =
        serde_json::from_str(&bundled).expect("OpenAPI contract must be valid JSON");

    // When/Then: every operation documents the exact shared schema only for
    // extractor failures that can apply to its request shape. JSON body
    // extractors can reject unsupported or malformed JSON, while bodyless and
    // non-JSON operations cannot produce those JSON extractor responses.
    for (path, path_item) in openapi["paths"].as_object().expect("paths") {
        for (method, operation) in path_item.as_object().expect("path item") {
            let has_json_body = operation["requestBody"]["content"]["application/json"].is_object();
            let mut statuses = vec!["400"];
            if has_json_body {
                statuses.extend(["415", "422"]);
            } else {
                for status in ["415", "422"] {
                    assert!(
                        operation["responses"][status].is_null(),
                        "{method} {path} must not document JSON body extractor response {status} without an application/json request body"
                    );
                }
            }

            for status in statuses {
                let schema =
                    &operation["responses"][status]["content"]["application/json"]["schema"];
                assert_eq!(
                    schema["$ref"], "#/components/schemas/ErrorResponse",
                    "{method} {path} response {status}"
                );
                assert_eq!(
                    schema.as_object().expect("schema").len(),
                    1,
                    "{method} {path} response {status} must use an exact shared ref"
                );
            }
        }
    }
}
