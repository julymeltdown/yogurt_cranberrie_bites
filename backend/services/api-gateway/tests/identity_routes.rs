//! T-208: the identity routes in the gateway.
//!
//! The four identity operations are split across two transports, and this binary
//! proves the split is real:
//!
//! * the three authenticated operations reach profile-product dispatch, so they
//!   fail closed with `DEPENDENCY_UNAVAILABLE` when the profile client is absent
//!   (which is exactly how this harness is composed);
//! * the callback is anonymous, answers with a `303` whose `Location` is a
//!   constant path from the start allowlist plus one of eight constant markers,
//!   and never reaches the browser boundary or the profile product envelope.
//!
//! The callback's own `Location` construction and query parsing are unit-level
//! properties, so they are asserted directly against the production functions as
//! well as through the router.
use api_gateway::adapters::r#in::http::identity_routes::{self, parse_query, PATH};
use api_gateway::domain::identity_verification::{CallbackReply, CallbackReplyError, RESULT_MARKERS, RETURN_PATHS};
use axum::{
    body::{to_bytes, Body},
    http::{header, Method, Request, StatusCode},
};
use tower::ServiceExt;

#[path = "unit/http_support.rs"]
mod http_support;
use http_support::{RouteHarness, USER_ID};

const KEY: &str = "c2c3dd2f-30ec-48af-9220-794b6fe090b4";

fn bearer(harness: &RouteHarness, method: &str, path: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .body(Body::empty())
        .unwrap()
}

fn mutation(harness: &RouteHarness, method: &str, path: &str, body: &str) -> Request<Body> {
    Request::builder()
        .method(method)
        .uri(path)
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::ORIGIN, "http://localhost:3000")
        .header("x-mcr-intent", "browser")
        .header("idempotency-key", KEY)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_owned()))
        .unwrap()
}

async fn reason_of(response: axum::response::Response) -> (StatusCode, String) {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 65536).await.unwrap();
    let value: serde_json::Value = serde_json::from_slice(&bytes).unwrap_or_default();
    (
        status,
        value["details"]["reason"].as_str().unwrap_or_default().to_owned(),
    )
}

/// The three authenticated identity operations reach profile dispatch.
#[tokio::test]
async fn the_authenticated_identity_operations_reach_profile_dispatch() {
    let harness = RouteHarness::new().await;
    // The read.
    let (status, reason) = reason_of(
        harness
            .transport_router()
            .oneshot(bearer(&harness, "GET", "/api/me/identity/naver"))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(reason, "DEPENDENCY_UNAVAILABLE");
    // The start.
    let (status, reason) = reason_of(
        harness
            .transport_router()
            .oneshot(mutation(
                &harness,
                "POST",
                "/api/me/identity/naver/verifications",
                r#"{"return_path":"/me/identity"}"#,
            ))
            .await
            .unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(reason, "DEPENDENCY_UNAVAILABLE");
    // The disconnect (a bodyless delete).
    let request = Request::builder()
        .method("DELETE")
        .uri("/api/me/identity/naver")
        .header(header::AUTHORIZATION, harness.bearer_token(USER_ID))
        .header(header::ORIGIN, "http://localhost:3000")
        .header("x-mcr-intent", "browser")
        .header("idempotency-key", KEY)
        .body(Body::empty())
        .unwrap();
    let (status, reason) = reason_of(harness.transport_router().oneshot(request).await.unwrap()).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(reason, "DEPENDENCY_UNAVAILABLE");
}

/// `{provider}` is a closed enum: anything but `naver` never reaches a producer.
///
/// The gateway's route table matches `:provider` because the frozen contract
/// declares the path with a parameter. The closed enum is enforced by the
/// profile producer, which admits only `naver` and answers every other value
/// with `permission_denied`; the gateway renders that as `403`. The property
/// this case proves is therefore the one that matters: an unknown provider is
/// refused, and it is refused *without* reaching a producer that would treat it
/// as a real provider.
#[tokio::test]
async fn an_unknown_provider_is_refused_by_the_producer_boundary() {
    use profile_service::profile_product::targets::parse;
    for provider in ["google", "apple", "x", "NAVER", "naver2"] {
        let path = format!("/api/me/identity/{provider}");
        let status = parse("GET", &path).expect_err("an unknown provider is not an operation");
        assert_eq!(
            status.code(),
            tonic::Code::PermissionDenied,
            "{path} must be refused by the closed producer route set"
        );
    }
    // And the one provider the contract names is a real operation.
    assert!(parse("GET", "/api/me/identity/naver").is_ok());
}

/// The callback is not authenticated and is not a browser mutation.
#[tokio::test]
async fn the_callback_is_anonymous_and_does_not_require_browser_intent() {
    let harness = RouteHarness::new().await;
    // No Authorization, no Origin, no X-MCR-Intent, no Idempotency-Key: exactly
    // what a provider redirect looks like. It must not be refused by the browser
    // boundary, so the answer is the callback's own closed refusal.
    let response = harness
        .transport_router()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("{PATH}?code=abc&state=not-a-real-state"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    // The callback client is absent in this harness, so the route fails closed
    // with a dependency error rather than an authentication error. That is the
    // proof it passed the boundary.
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{status}");
    // Every callback response is uncacheable and carries no referrer.
    assert_eq!(response.headers().get("cache-control").unwrap(), "no-store");
    assert_eq!(response.headers().get("pragma").unwrap(), "no-cache");
    assert_eq!(
        response.headers().get("referrer-policy").unwrap(),
        "no-referrer"
    );
}

/// A callback without a valid state is a `400` that reveals nothing.
#[tokio::test]
async fn a_callback_without_a_valid_state_is_a_closed_400() {
    let harness = RouteHarness::new().await;
    for query in [
        // No state at all.
        "?code=abc",
        // Empty state.
        "?code=abc&state=",
        // A state outside the grammar.
        "?code=abc&state=has%20a%20space",
        // A repeated state is ambiguous.
        "?code=abc&state=one&state=two",
        // An unexpected parameter.
        "?code=abc&state=abc&next=https://evil.example",
        // Code and error together is not a provider redirect.
        "?code=abc&error=access_denied&state=abc",
    ] {
        let response = harness
            .transport_router()
            .oneshot(
                Request::builder()
                    .method("GET")
                    .uri(format!("{PATH}{query}"))
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let (status, reason) = reason_of(response).await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{query}");
        assert_eq!(reason, "INVALID_INPUT", "{query}");
    }
}

/// The callback cannot be used to reach any other operation.
#[tokio::test]
async fn the_callback_cannot_reach_another_operation() {
    let harness = RouteHarness::new().await;
    for (method, path) in [
        ("POST", PATH.to_owned()),
        ("DELETE", PATH.to_owned()),
        ("PUT", PATH.to_owned()),
        ("GET", format!("{PATH}/../me/identity/naver")),
        ("GET", format!("{PATH}/extra")),
        ("GET", "/api/identity/google/callback".to_owned()),
        ("GET", "/api/identity/naver/callback/".to_owned()),
    ] {
        let response = harness
            .transport_router()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(&path)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert!(
            response.status().is_client_error() || response.status() == StatusCode::SERVICE_UNAVAILABLE,
            "{method} {path}: {}",
            response.status()
        );
        // Never a redirect: only the exact callback path may answer with one.
        assert_ne!(response.status(), StatusCode::SEE_OTHER, "{method} {path}");
    }
}

/// The route module recognises exactly the callback path.
#[test]
fn only_the_callback_path_is_the_callback() {
    assert!(identity_routes::is_callback(PATH));
    assert!(!identity_routes::is_callback("/api/me/identity/naver"));
    assert!(!identity_routes::is_callback("/api/identity/naver/callback/"));
    assert!(!identity_routes::is_callback("/api/identity/google/callback"));
}

/// The query parser accepts exactly the provider's three parameters.
#[test]
fn the_query_parser_accepts_only_the_provider_parameters() {
    let query = parse_query(Some("code=abc&state=xyz")).expect("valid");
    assert_eq!(query.state, "xyz");
    assert_eq!(query.code.as_deref(), Some("abc"));
    assert_eq!(query.provider_error, None);
    // A provider error instead of a code.
    let query = parse_query(Some("error=access_denied&state=xyz")).expect("valid");
    assert_eq!(query.code, None);
    assert_eq!(query.provider_error.as_deref(), Some("access_denied"));
    // Percent-decoding.
    let query = parse_query(Some("code=a%2Fb&state=x-y_z.1~2")).expect("valid");
    assert_eq!(query.code.as_deref(), Some("a/b"));
    assert_eq!(query.state, "x-y_z.1~2");
    // Refusals.
    for raw in [
        None,
        Some(""),
        Some("state="),
        Some("state=a b"),
        Some("state=a&state=b"),
        Some("state=a&unknown=1"),
        Some("code=a&error=b&state=c"),
    ] {
        assert!(parse_query(raw).is_err(), "{raw:?}");
    }
}

/// A `Location` is a constant path plus a constant marker.
#[test]
fn a_location_is_a_constant_path_plus_a_constant_marker() {
    for path in RETURN_PATHS {
        for marker in RESULT_MARKERS {
            let reply = CallbackReply::validate(path.to_owned(), marker.to_owned())
                .expect("an allowlisted pair");
            let location = reply.location();
            assert_eq!(location, format!("{path}?identity={marker}"));
            // Never absolute, never protocol-relative.
            assert!(location.starts_with('/'));
            assert!(!location.starts_with("//"));
            assert!(!location.contains("://"));
        }
    }
}

/// A producer reply outside the closed sets is refused, not sanitized.
#[test]
fn a_reply_outside_the_closed_sets_is_refused() {
    for path in [
        "https://evil.example",
        "//evil.example",
        "/me/identity?next=https://evil.example",
        "/me/identity#fragment",
        "/evil",
        "",
    ] {
        assert_eq!(
            CallbackReply::validate(path.to_owned(), "success".to_owned()),
            Err(CallbackReplyError::Unavailable),
            "{path:?}"
        );
    }
    for marker in ["", "ok", "SUCCESS", "success ", "success&next=evil"] {
        assert_eq!(
            CallbackReply::validate("/me/identity".to_owned(), marker.to_owned()),
            Err(CallbackReplyError::Unavailable),
            "{marker:?}"
        );
    }
}

/// The callback is classified as its own admission class.
#[test]
fn the_callback_has_its_own_admission_class() {
    use api_gateway::domain::admission::RouteClass;
    assert_eq!(
        RouteClass::classify("GET", PATH),
        RouteClass::VerificationCallback
    );
    // A path parameter cannot move it into a cheaper class.
    assert_eq!(
        RouteClass::classify("GET", "/api/identity/naver/callback"),
        RouteClass::VerificationCallback
    );
    // The class has a bounded budget, so a flood of guessed states cannot use
    // the callback as an oracle.
    assert!(RouteClass::VerificationCallback.default_budget() > 0);
    assert!(RouteClass::VerificationCallback.default_budget() <= 30);
    assert_eq!(RouteClass::VerificationCallback.as_str(), "verification_callback");
}

/// The callback path is not a profile product route.
#[test]
fn the_callback_is_not_a_profile_product_route() {
    assert!(!api_gateway::adapters::r#in::http::profile_product::protects(PATH));
    assert!(api_gateway::adapters::r#in::http::profile_product::protects(
        "/api/me/identity/naver"
    ));
    assert!(api_gateway::adapters::r#in::http::profile_product::route(
        &Method::POST,
        "/api/me/identity/naver/verifications"
    )
    .is_some());
    assert!(api_gateway::adapters::r#in::http::profile_product::route(
        &Method::DELETE,
        "/api/me/identity/naver"
    )
    .is_some());
}
