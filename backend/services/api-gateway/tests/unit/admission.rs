//! Admission classes: each route class spends its own budget.
use crate::http_support::RouteHarness;
use api_gateway::adapters::r#in::http;
use api_gateway::domain::admission::{
    AdmissionMode, AdmissionOutcome, AdmissionPolicies, AdmissionScope, RouteClass,
};
use axum::body::Body;
use axum::extract::ConnectInfo;
use axum::http::{header, Method, Request, StatusCode};
use serde_json::json;
use sqlx::postgres::PgPoolOptions;
use std::sync::Arc;
use std::time::Duration;
use tower::ServiceExt;

#[test]
fn routes_are_classified_by_method_and_template() {
    for (method, route, expected) in [
        ("POST", "/auth/login", RouteClass::Login),
        ("GET", "/auth/oauth/:provider/authorize", RouteClass::Login),
        ("GET", "/auth/oauth/:provider/callback", RouteClass::Login),
        ("POST", "/auth/oauth/:provider/callback", RouteClass::Login),
        (
            "POST",
            "/auth/oauth/:provider/callback/complete",
            RouteClass::Login,
        ),
        ("POST", "/shadow/auth/login", RouteClass::Login),
        ("POST", "/auth/email/signup", RouteClass::Signup),
        ("POST", "/auth/email/check", RouteClass::Signup),
        ("POST", "/auth/email/verify", RouteClass::Signup),
        ("POST", "/auth/password/reset/request", RouteClass::Reset),
        ("POST", "/auth/password/reset/confirm", RouteClass::Reset),
        ("POST", "/auth/email/resend", RouteClass::VerificationResend),
        ("POST", "/auth/refresh", RouteClass::Refresh),
        ("GET", "/api/profile/search", RouteClass::Search),
        ("GET", "/api/posts/search", RouteClass::Search),
        ("POST", "/api/posts", RouteClass::Write),
        ("DELETE", "/api/posts/:post_id", RouteClass::Write),
        ("POST", "/auth/logout", RouteClass::Write),
        ("GET", "/api/feed", RouteClass::Read),
        ("GET", "/api/posts/:post_id", RouteClass::Read),
        ("GET", "unmatched", RouteClass::Read),
        ("POST", "unmatched", RouteClass::Write),
    ] {
        assert_eq!(
            RouteClass::classify(method, route),
            expected,
            "{method} {route}"
        );
    }
}

#[test]
fn budget_overrides_are_validated_and_the_read_budget_has_one_owner() {
    let policies = AdmissionPolicies::default()
        .with_overrides(" login=20 , verification_resend=2 ")
        .expect("valid overrides");
    assert_eq!(policies.budget(RouteClass::Login), 20);
    assert_eq!(policies.budget(RouteClass::VerificationResend), 2);
    assert_eq!(
        policies.budget(RouteClass::Signup),
        RouteClass::Signup.default_budget()
    );

    for (overrides, expected) in [
        ("read=5", "GATEWAY_RATE_LIMIT_PER_MINUTE"),
        ("logins=5", "unknown admission class"),
        ("login", "expected class=requests"),
        ("login=0", "between 1 and 100000"),
        ("login=many", "whole number"),
    ] {
        let error = AdmissionPolicies::default()
            .with_overrides(overrides)
            .expect_err(overrides);
        assert!(error.contains(expected), "{overrides}: {error}");
    }
}

#[test]
fn identity_budget_overrides_only_accept_anonymous_email_classes() {
    let policies = AdmissionPolicies::default()
        .with_identity_overrides("login=6,signup=3,reset=2,verification_resend=1")
        .expect("valid identity budgets");
    assert_eq!(policies.identity_budget(RouteClass::Login), 6);
    assert_eq!(policies.identity_budget(RouteClass::Signup), 3);
    assert_eq!(policies.identity_budget(RouteClass::Reset), 2);
    assert_eq!(policies.identity_budget(RouteClass::VerificationResend), 1);
    assert!(!AdmissionPolicies::default().has_identity_budgets());
    assert!(policies.has_identity_budgets());

    for (overrides, expected) in [
        ("read=2", "not supported for the read class"),
        ("refresh=2", "not supported for the refresh class"),
        ("login=0", "between 1 and 100000"),
        ("login=many", "whole number"),
    ] {
        let error = AdmissionPolicies::default()
            .with_identity_overrides(overrides)
            .expect_err(overrides);
        assert!(error.contains(expected), "{overrides}: {error}");
    }
}

#[test]
fn principal_budget_overrides_only_accept_actor_scoped_classes() {
    let policies = AdmissionPolicies::default()
        .with_principal_overrides("search=20,write=5,read=10")
        .expect("valid principal budgets");
    assert_eq!(policies.principal_budget(RouteClass::Search), 20);
    assert_eq!(policies.principal_budget(RouteClass::Write), 5);
    assert_eq!(policies.principal_budget(RouteClass::Read), 10);
    assert!(!AdmissionPolicies::default().has_principal_budgets());
    assert!(policies.has_principal_budgets());

    for (overrides, expected) in [
        ("login=2", "not supported for the login class"),
        ("refresh=2", "not supported for the refresh class"),
        ("search=0", "between 1 and 100000"),
        ("write=many", "whole number"),
    ] {
        let error = AdmissionPolicies::default()
            .with_principal_overrides(overrides)
            .expect_err(overrides);
        assert!(error.contains(expected), "{overrides}: {error}");
    }
}

#[test]
fn per_process_in_flight_overrides_are_bounded_for_each_class() {
    let policies = AdmissionPolicies::default()
        .with_max_in_flight_overrides("login=4,read=96")
        .expect("valid concurrency limits");
    assert_eq!(policies.max_in_flight(RouteClass::Login), 4);
    assert_eq!(policies.max_in_flight(RouteClass::Read), 96);
    assert_eq!(policies.max_in_flight(RouteClass::Reset), 8);

    for (overrides, expected) in [
        ("unknown=2", "unknown admission class"),
        ("login=0", "between 1 and 10000"),
        ("login=10001", "between 1 and 10000"),
        ("login=many", "whole number"),
    ] {
        let error = AdmissionPolicies::default()
            .with_max_in_flight_overrides(overrides)
            .expect_err(overrides);
        assert!(error.contains(expected), "{overrides}: {error}");
    }
}

#[test]
fn shadow_mode_only_bypasses_quota_and_cardinality_refusals() {
    assert_eq!(
        AdmissionMode::parse("enforce"),
        Some(AdmissionMode::Enforce)
    );
    assert_eq!(AdmissionMode::parse("shadow"), Some(AdmissionMode::Shadow));
    assert_eq!(AdmissionMode::parse("unknown"), None);
    assert_eq!(AdmissionPolicies::default().mode, AdmissionMode::Enforce);
    assert!(AdmissionOutcome::RejectedQuota.is_shadowable_rejection());
    assert!(AdmissionOutcome::RejectedCardinality.is_shadowable_rejection());
    assert!(!AdmissionOutcome::RejectedStoreUnavailable.is_shadowable_rejection());
    assert!(!AdmissionOutcome::RejectedFallbackExhausted.is_shadowable_rejection());
}

#[test]
fn admission_latency_metric_exports_bounded_cumulative_buckets() {
    let metrics = api_gateway::config::GatewayMetrics::default();
    metrics.observe_admission_duration(
        RouteClass::Login,
        AdmissionScope::Network,
        Duration::from_millis(20),
    );

    let rendered = metrics.render_prometheus();
    assert!(rendered
        .contains("gateway_admission_duration_seconds_bucket{class=\"login\",scope=\"network\",le=\"0.01\"} 0"));
    assert!(rendered
        .contains("gateway_admission_duration_seconds_bucket{class=\"login\",scope=\"network\",le=\"0.025\"} 1"));
    assert!(rendered
        .contains("gateway_admission_duration_seconds_bucket{class=\"login\",scope=\"network\",le=\"+Inf\"} 1"));
    assert!(rendered.contains(
        "gateway_admission_duration_seconds_sum{class=\"login\",scope=\"network\"} 0.020000000"
    ));
    assert!(rendered
        .contains("gateway_admission_duration_seconds_count{class=\"login\",scope=\"network\"} 1"));
}

#[test]
fn postgres_lock_wait_metric_is_bounded_and_scoped() {
    let metrics = api_gateway::config::GatewayMetrics::default();
    metrics.observe_admission_lock_wait(
        RouteClass::Login,
        AdmissionScope::Network,
        Duration::from_millis(20),
    );

    let rendered = metrics.render_prometheus();
    assert!(rendered.contains(
        "gateway_admission_lock_wait_seconds_bucket{class=\"login\",scope=\"network\",le=\"0.01\"} 0"
    ));
    assert!(rendered.contains(
        "gateway_admission_lock_wait_seconds_bucket{class=\"login\",scope=\"network\",le=\"0.025\"} 1"
    ));
    assert!(rendered.contains(
        "gateway_admission_lock_wait_seconds_sum{class=\"login\",scope=\"network\"} 0.020000000"
    ));
    assert!(rendered.contains(
        "gateway_admission_lock_wait_seconds_count{class=\"login\",scope=\"network\"} 1"
    ));
}

#[test]
fn only_reads_may_continue_without_the_store() {
    use api_gateway::domain::admission::StoreFailurePolicy;
    for class in RouteClass::ALL {
        let expected = if class == RouteClass::Read {
            StoreFailurePolicy::BoundedLocalFallback
        } else {
            StoreFailurePolicy::Reject
        };
        assert_eq!(class.store_failure(), expected, "{class}");
    }
}

fn post(uri: &str, body: serde_json::Value) -> Request<Body> {
    Request::builder()
        .method(Method::POST)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .expect("request")
}

#[tokio::test]
async fn a_reset_flood_does_not_lock_the_same_network_out_of_login() {
    // Given: the default budgets, and one network sending reset requests.
    let harness = RouteHarness::new().await;
    let app = http::router(harness.state.clone());
    let reset_budget = RouteClass::Reset.default_budget();

    // When: it sends one more reset request than the reset budget allows.
    let mut statuses = Vec::new();
    for _ in 0..=reset_budget {
        let response = app
            .clone()
            .oneshot(post(
                "/auth/password/reset/request",
                json!({"email": "hana@example.com"}),
            ))
            .await
            .expect("response");
        statuses.push(response.status());
    }
    let login = app
        .clone()
        .oneshot(post(
            "/auth/login",
            json!({"email": "hana@example.com", "password": "secret"}),
        ))
        .await
        .expect("response");

    // Then: only the reset class is exhausted; login still has its budget.
    assert!(statuses[..reset_budget as usize]
        .iter()
        .all(|status| *status != StatusCode::TOO_MANY_REQUESTS));
    assert_eq!(
        statuses[reset_budget as usize],
        StatusCode::TOO_MANY_REQUESTS
    );
    assert_eq!(login.status(), StatusCode::OK);
    let rendered = harness.state.metrics.render_prometheus();
    assert!(rendered.contains(&format!(
        "gateway_admission_decisions_total{{class=\"reset\",scope=\"network\",outcome=\"admitted\"}} {reset_budget}"
    )));
    assert!(rendered.contains(
        "gateway_admission_decisions_total{class=\"reset\",scope=\"network\",outcome=\"rejected_quota\"} 1"
    ));
    assert!(rendered
        .contains("gateway_admission_decisions_total{class=\"login\",scope=\"network\",outcome=\"admitted\"} 1"));
}

#[tokio::test]
async fn shadow_mode_records_a_quota_refusal_and_still_runs_the_route() {
    let policies = AdmissionPolicies::default()
        .with_budget(RouteClass::Login, 1)
        .with_mode(AdmissionMode::Shadow);
    let harness = RouteHarness::with_admission(policies)
        .await
        .expect("shadow harness");
    let app = http::router(harness.state.clone());

    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(post(
                "/auth/login",
                json!({"email": "hana@example.com", "password": "secret"}),
            ))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    let rendered = harness.state.metrics.render_prometheus();
    assert!(rendered.contains(
        "gateway_admission_decisions_total{class=\"login\",scope=\"network\",outcome=\"rejected_quota\"} 1"
    ));
    assert!(rendered.contains(
        "gateway_admission_shadow_passes_total{class=\"login\",scope=\"network\",decision=\"rejected_quota\"} 1"
    ));
    assert!(rendered
        .contains("gateway_admission_duration_seconds_count{class=\"login\",scope=\"network\"} 2"));
    assert!(!rendered.contains("gateway_rate_limit_rejections_total"));
}

#[tokio::test]
async fn shadow_mode_does_not_pass_requests_when_the_admission_store_is_unavailable() {
    let policies = AdmissionPolicies::default().with_mode(AdmissionMode::Shadow);
    let mut harness = RouteHarness::with_admission(policies.clone())
        .await
        .expect("shadow harness");
    let unavailable_pool = PgPoolOptions::new()
        .acquire_timeout(Duration::from_millis(50))
        .connect_lazy("postgres://postgres:postgres@127.0.0.1:1/gateway_test")
        .expect("lazy pool");
    Arc::get_mut(&mut harness.state)
        .expect("test harness state is uniquely owned")
        .rate_limiter = api_gateway::config::InMemoryRateLimiter::production(policies)
        .with_database(unavailable_pool);
    let app = http::router(harness.state.clone());

    let response = app
        .oneshot(post(
            "/auth/login",
            json!({"email": "hana@example.com", "password": "secret"}),
        ))
        .await
        .expect("response");
    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);

    let rendered = harness.state.metrics.render_prometheus();
    assert!(rendered.contains(
        "gateway_admission_decisions_total{class=\"login\",scope=\"network\",outcome=\"rejected_store_unavailable\"} 1"
    ));
    assert!(!rendered.contains("gateway_admission_shadow_passes_total"));
}

#[tokio::test]
async fn keyed_login_budget_follows_normalized_email_across_client_networks() {
    let policies = AdmissionPolicies::default().with_identity_budget(RouteClass::Login, 1);
    let harness = RouteHarness::with_admission(policies)
        .await
        .expect("identity-budget harness");
    let app = http::router(harness.state.clone());

    let mut first = post(
        "/auth/login",
        json!({"email": "  HANA@EXAMPLE.COM ", "password": "secret"}),
    );
    first.extensions_mut().insert(ConnectInfo(
        "203.0.113.10:40000"
            .parse::<std::net::SocketAddr>()
            .expect("first client socket"),
    ));
    let first = app.clone().oneshot(first).await.expect("first response");
    assert_eq!(first.status(), StatusCode::OK);

    let mut same_identity = post(
        "/auth/login",
        json!({"email": "hana@example.com", "password": "secret"}),
    );
    same_identity.extensions_mut().insert(ConnectInfo(
        "198.51.100.20:40000"
            .parse::<std::net::SocketAddr>()
            .expect("second client socket"),
    ));
    let same_identity = app
        .clone()
        .oneshot(same_identity)
        .await
        .expect("identity-limited response");
    assert_eq!(same_identity.status(), StatusCode::TOO_MANY_REQUESTS);

    let mut other_identity = post(
        "/auth/login",
        json!({"email": "yura@example.com", "password": "secret"}),
    );
    other_identity.extensions_mut().insert(ConnectInfo(
        "198.51.100.20:40000"
            .parse::<std::net::SocketAddr>()
            .expect("third client socket"),
    ));
    let other_identity = app
        .oneshot(other_identity)
        .await
        .expect("other identity response");
    assert_eq!(other_identity.status(), StatusCode::OK);

    let rendered = harness.state.metrics.render_prometheus();
    assert!(rendered.contains(
        "gateway_admission_decisions_total{class=\"login\",scope=\"identity\",outcome=\"rejected_quota\"} 1"
    ));
    assert!(!rendered.contains("hana@example.com"));
    assert!(!rendered.contains("yura@example.com"));
}

#[tokio::test]
async fn principal_budget_uses_only_verified_subjects_across_client_networks() {
    let policies = AdmissionPolicies::default().with_principal_budget(RouteClass::Read, 1);
    let harness = RouteHarness::with_admission(policies)
        .await
        .expect("principal-budget harness");
    let app = http::router(harness.state.clone());

    let identified_feed = |authorization: &str, address: &str| {
        let mut request = Request::builder()
            .method(Method::GET)
            .uri("/api/feed")
            .header(header::AUTHORIZATION, authorization)
            .body(Body::empty())
            .expect("feed request");
        request.extensions_mut().insert(ConnectInfo(
            format!("{address}:40000")
                .parse::<std::net::SocketAddr>()
                .expect("client socket"),
        ));
        request
    };

    let forged = app
        .clone()
        .oneshot(identified_feed(
            "Bearer forged.token.signature",
            "203.0.113.9",
        ))
        .await
        .expect("forged request");
    assert_eq!(forged.status(), StatusCode::UNAUTHORIZED);

    let first_principal = app
        .clone()
        .oneshot(identified_feed(
            &harness.bearer_token(crate::http_support::USER_ID),
            "203.0.113.10",
        ))
        .await
        .expect("first principal response");
    assert_eq!(first_principal.status(), StatusCode::OK);

    let other_principal = app
        .clone()
        .oneshot(identified_feed(
            &harness.bearer_token(crate::http_support::OTHER_USER_ID),
            "198.51.100.11",
        ))
        .await
        .expect("other principal response");
    assert_eq!(other_principal.status(), StatusCode::OK);

    let same_principal_new_network = app
        .oneshot(identified_feed(
            &harness.bearer_token(crate::http_support::USER_ID),
            "198.51.100.12",
        ))
        .await
        .expect("principal-limited response");
    assert_eq!(
        same_principal_new_network.status(),
        StatusCode::TOO_MANY_REQUESTS
    );

    let rendered = harness.state.metrics.render_prometheus();
    assert!(rendered.contains(
        "gateway_admission_decisions_total{class=\"read\",scope=\"principal\",outcome=\"rejected_quota\"} 1"
    ));
    assert!(rendered.contains(
        "gateway_admission_decisions_total{class=\"read\",scope=\"principal\",outcome=\"admitted\"} 2"
    ));
    assert!(rendered.contains(
        "gateway_admission_duration_seconds_count{class=\"read\",scope=\"principal\"} 3"
    ));
    assert!(!rendered.contains(crate::http_support::USER_ID));
    assert!(!rendered.contains(crate::http_support::OTHER_USER_ID));
}

#[tokio::test]
async fn shadow_mode_observes_and_passes_authenticated_principal_refusals() {
    let policies = AdmissionPolicies::default()
        .with_principal_budget(RouteClass::Read, 1)
        .with_mode(AdmissionMode::Shadow);
    let harness = RouteHarness::with_admission(policies)
        .await
        .expect("shadow principal-budget harness");
    let app = http::router(harness.state.clone());

    for address in ["203.0.113.10", "198.51.100.10"] {
        let mut request = Request::builder()
            .method(Method::GET)
            .uri("/api/feed")
            .header(
                header::AUTHORIZATION,
                harness.bearer_token(crate::http_support::USER_ID),
            )
            .body(Body::empty())
            .expect("feed request");
        request.extensions_mut().insert(ConnectInfo(
            format!("{address}:40000")
                .parse::<std::net::SocketAddr>()
                .expect("client socket"),
        ));
        let response = app.clone().oneshot(request).await.expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    let rendered = harness.state.metrics.render_prometheus();
    assert!(rendered.contains(
        "gateway_admission_decisions_total{class=\"read\",scope=\"principal\",outcome=\"rejected_quota\"} 1"
    ));
    assert!(rendered.contains(
        "gateway_admission_shadow_passes_total{class=\"read\",scope=\"principal\",decision=\"rejected_quota\"} 1"
    ));
}

#[tokio::test]
async fn shadow_mode_records_and_passes_identity_budget_refusals() {
    let policies = AdmissionPolicies::default()
        .with_identity_budget(RouteClass::Login, 1)
        .with_mode(AdmissionMode::Shadow);
    let harness = RouteHarness::with_admission(policies)
        .await
        .expect("shadow identity-budget harness");
    let app = http::router(harness.state.clone());

    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(post(
                "/auth/login",
                json!({"email": "hana@example.com", "password": "secret"}),
            ))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::OK);
    }

    let rendered = harness.state.metrics.render_prometheus();
    assert!(rendered.contains(
        "gateway_admission_decisions_total{class=\"login\",scope=\"identity\",outcome=\"rejected_quota\"} 1"
    ));
    assert!(rendered.contains(
        "gateway_admission_shadow_passes_total{class=\"login\",scope=\"identity\",decision=\"rejected_quota\"} 1"
    ));
    assert!(rendered.contains(
        "gateway_admission_duration_seconds_count{class=\"login\",scope=\"identity\"} 2"
    ));
}

#[tokio::test]
async fn saturated_in_flight_class_returns_503_before_spending_quota() {
    let policies = AdmissionPolicies::default()
        .with_max_in_flight(RouteClass::Login, 1)
        .with_mode(AdmissionMode::Shadow);
    let harness = RouteHarness::with_admission(policies)
        .await
        .expect("in-flight harness");
    let held = harness
        .state
        .rate_limiter
        .try_acquire_in_flight(RouteClass::Login)
        .expect("hold the only login permit");
    let app = http::router(harness.state.clone());

    let saturated = app
        .clone()
        .oneshot(post(
            "/auth/login",
            json!({"email": "hana@example.com", "password": "secret"}),
        ))
        .await
        .expect("saturated response");
    assert_eq!(saturated.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert_eq!(saturated.headers().get("retry-after").unwrap(), "1");
    let rendered = harness.state.metrics.render_prometheus();
    assert!(rendered.contains("gateway_admission_in_flight_rejections_total{class=\"login\"} 1"));
    assert!(!rendered.contains(
        "gateway_admission_decisions_total{class=\"login\",scope=\"network\",outcome=\"admitted\"}"
    ));

    drop(held);
    let admitted = app
        .oneshot(post(
            "/auth/login",
            json!({"email": "hana@example.com", "password": "secret"}),
        ))
        .await
        .expect("recovered response");
    assert_eq!(admitted.status(), StatusCode::OK);
}
