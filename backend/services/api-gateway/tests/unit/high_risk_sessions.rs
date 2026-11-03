//! High-risk actions are confirmed with the issuer, not trusted to the token.
use crate::http_support::{
    response_json, RouteHarness, SessionAnswer, OTHER_USER_ID, SESSION_ID, USER_ID,
};
use api_gateway::adapters::r#in::http::{self, api_docs_routes};
use api_gateway::application::dto::issuer_dto::{SessionAuthorityRequestDto, SessionStateDto};
use api_gateway::application::ports::GatewayRuntimePort;
use api_gateway::application::ports::SessionAuthorityPort;
use api_gateway::application::service::high_risk_session_service::{
    HighRiskSessionGuard, MAX_CACHED_SESSIONS, MAX_SESSION_FRESHNESS,
};
use api_gateway::config::RuntimeMode;
use api_gateway::domain::high_risk::{classify, HighRiskAction, SessionDecision, SessionState};
use async_trait::async_trait;
use axum::body::{to_bytes, Body};
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use serde_json::Value;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};
use tower::ServiceExt;

// ---------------------------------------------------------------------------
// Classification
// ---------------------------------------------------------------------------

#[test]
fn administration_account_deletion_and_provider_linking_are_high_risk() {
    let administration = Some(HighRiskAction::Administration);
    let account = Some(HighRiskAction::AccountSecurity);
    for (method, route, expected) in [
        ("GET", "/admin/policies", administration),
        (
            "POST",
            "/admin/account-deletions/:saga_id/retry",
            administration,
        ),
        ("PUT", "/admin/policies/:policy_id", administration),
        ("GET", "/admin/moderation/reports", administration),
        (
            "POST",
            "/admin/moderation/reports/:report_id/decisions",
            administration,
        ),
        ("DELETE", "/auth/account", account),
        ("POST", "/auth/oauth/:provider/link", account),
        // Ordinary reads and writes trust the token until it expires.
        ("GET", "/auth/account", None),
        ("GET", "/auth/account/deletion", None),
        ("POST", "/api/moderation/reports", None),
        ("POST", "/api/posts", None),
        ("GET", "/administrators", None),
        ("GET", "unmatched", None),
    ] {
        assert_eq!(classify(method, route), expected, "{method} {route}");
    }
}

#[test]
fn only_administration_requires_a_current_role() {
    let user_only = SessionState::Active {
        roles: vec!["ROLE_USER".into()],
    };
    assert_eq!(
        user_only.permits(HighRiskAction::Administration),
        SessionDecision::RoleRevoked
    );
    assert_eq!(
        user_only.permits(HighRiskAction::AccountSecurity),
        SessionDecision::Allowed
    );
    assert_eq!(
        SessionState::Ended.permits(HighRiskAction::AccountSecurity),
        SessionDecision::SessionEnded
    );
}

// ---------------------------------------------------------------------------
// Freshness window
// ---------------------------------------------------------------------------

/// An issuer whose answer a test can change, counting how often it is asked.
#[derive(Default)]
struct ScriptedIssuer {
    /// None makes the check fail, as an unreachable issuer does.
    answer: Mutex<Option<SessionState>>,
    calls: AtomicUsize,
}

impl ScriptedIssuer {
    fn answering(state: SessionState) -> Arc<Self> {
        let issuer = Arc::new(Self::default());
        issuer.set(Some(state));
        issuer
    }

    fn set(&self, answer: Option<SessionState>) {
        *self.answer.lock().unwrap() = answer;
    }

    fn calls(&self) -> usize {
        self.calls.load(Ordering::SeqCst)
    }
}

impl ScriptedIssuer {
    async fn session_state_session_authority_port_inner(
        &self,
        _request: &SessionAuthorityRequestDto,
    ) -> anyhow::Result<SessionStateDto> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        self.answer
            .lock()
            .unwrap()
            .clone()
            .map(SessionStateDto::from)
            .ok_or_else(|| anyhow::anyhow!("issuer unavailable"))
    }
}

#[async_trait]
impl SessionAuthorityPort for ScriptedIssuer {
    async fn session_state(
        &self,
        _request: &SessionAuthorityRequestDto,
    ) -> api_gateway::application::client_error::GatewayClientResult<SessionStateDto> {
        self.session_state_session_authority_port_inner(_request)
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
}

/// A clock that moves only when told to.
struct ManualRuntime(Arc<Mutex<Instant>>);

impl GatewayRuntimePort for ManualRuntime {
    fn utc_now(&self) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(0, 0).unwrap()
    }

    fn monotonic_now(&self) -> Instant {
        *self.0.lock().unwrap()
    }

    fn new_id(&self) -> uuid::Uuid {
        uuid::Uuid::from_u128(1)
    }
}

fn manual_clock() -> (Arc<Mutex<Instant>>, Arc<ManualRuntime>) {
    let now = Arc::new(Mutex::new(Instant::now()));
    let runtime = Arc::new(ManualRuntime(now.clone()));
    (now, runtime)
}

fn admin() -> SessionState {
    SessionState::Active {
        roles: vec!["ROLE_USER".into(), "ROLE_ADMIN".into()],
    }
}

#[tokio::test]
async fn a_live_answer_is_reused_only_within_the_freshness_window() {
    // Given: a guard with a five-second window over a live admin session.
    let issuer = ScriptedIssuer::answering(admin());
    let (now, clock) = manual_clock();
    let guard = HighRiskSessionGuard::new(issuer.clone(), Duration::from_secs(5), clock);
    let check = || guard.check("user", "session", HighRiskAction::Administration.into());

    // When: the session is checked, then logged out at the issuer.
    let first = check().await.unwrap();
    issuer.set(Some(SessionState::Ended));
    *now.lock().unwrap() += Duration::from_millis(4_999);
    let within = check().await.unwrap();
    *now.lock().unwrap() += Duration::from_millis(1);
    let after = check().await.unwrap();

    // Then: the logout is honoured as soon as the window closes, and not later.
    assert_eq!(first.decision, SessionDecision::Allowed.into());
    assert!(!first.from_cache);
    assert_eq!(within.decision, SessionDecision::Allowed.into());
    assert!(within.from_cache);
    assert_eq!(after.decision, SessionDecision::SessionEnded.into());
    assert!(!after.from_cache);
    assert_eq!(issuer.calls(), 2);
}

#[tokio::test]
async fn an_ended_session_is_never_remembered() {
    let issuer = ScriptedIssuer::answering(SessionState::Ended);
    let guard = HighRiskSessionGuard::new(
        issuer.clone(),
        Duration::from_secs(5),
        Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime),
    );

    for _ in 0..3 {
        let verdict = guard
            .check("user", "session", HighRiskAction::AccountSecurity.into())
            .await
            .unwrap();
        assert_eq!(verdict.decision, SessionDecision::SessionEnded.into());
    }

    assert_eq!(issuer.calls(), 3);
    assert_eq!(guard.cached_sessions(), 0);
}

#[tokio::test]
async fn an_unreachable_issuer_refuses_and_is_asked_again_next_time() {
    // Given: an issuer that cannot answer.
    let issuer = Arc::new(ScriptedIssuer::default());
    let guard = HighRiskSessionGuard::new(
        issuer.clone(),
        Duration::from_secs(5),
        Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime),
    );

    // When: two checks are made.
    let first = guard
        .check("user", "session", HighRiskAction::Administration.into())
        .await;
    let second = guard
        .check("user", "session", HighRiskAction::Administration.into())
        .await;

    // Then: both fail, so the caller refuses, and nothing was remembered.
    assert!(first.is_err());
    assert!(second.is_err());
    assert_eq!(issuer.calls(), 2);
}

#[tokio::test]
async fn zero_freshness_asks_the_issuer_every_time() {
    let issuer = ScriptedIssuer::answering(admin());
    let guard = HighRiskSessionGuard::new(
        issuer.clone(),
        Duration::ZERO,
        Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime),
    );

    for _ in 0..3 {
        let verdict = guard
            .check("user", "session", HighRiskAction::Administration.into())
            .await
            .unwrap();
        assert!(!verdict.from_cache);
    }

    assert_eq!(issuer.calls(), 3);
}

#[tokio::test]
async fn a_removed_admin_role_is_refused_while_the_session_stays_live() {
    let issuer = ScriptedIssuer::answering(SessionState::Active {
        roles: vec!["ROLE_USER".into()],
    });
    let guard = HighRiskSessionGuard::new(
        issuer,
        Duration::from_secs(5),
        Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime),
    );

    let admin_action = guard
        .check("user", "session", HighRiskAction::Administration.into())
        .await
        .unwrap();
    let account_action = guard
        .check("user", "session", HighRiskAction::AccountSecurity.into())
        .await
        .unwrap();

    assert_eq!(admin_action.decision, SessionDecision::RoleRevoked.into());
    assert_eq!(account_action.decision, SessionDecision::Allowed.into());
}

#[tokio::test]
async fn the_window_and_the_cache_are_bounded() {
    let issuer = ScriptedIssuer::answering(admin());
    let guard = HighRiskSessionGuard::new(
        issuer,
        Duration::from_secs(3_600),
        Arc::new(api_gateway::adapters::out::runtime::SystemGatewayRuntime),
    );
    assert_eq!(guard.freshness(), MAX_SESSION_FRESHNESS);

    for session in 0..MAX_CACHED_SESSIONS + 10 {
        guard
            .check(
                "user",
                &session.to_string(),
                HighRiskAction::Administration.into(),
            )
            .await
            .unwrap();
    }

    assert!(guard.cached_sessions() <= MAX_CACHED_SESSIONS);
}

// ---------------------------------------------------------------------------
// Routes
// ---------------------------------------------------------------------------

fn admin_token(harness: &RouteHarness) -> String {
    harness.bearer_token_with_roles(USER_ID, vec!["ROLE_USER".into(), "ROLE_ADMIN".into()])
}

fn request(method: Method, uri: &str, authorization: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder()
        .method(method)
        .uri(uri)
        .header(header::CONTENT_TYPE, "application/json");
    if let Some(authorization) = authorization {
        builder = builder.header(header::AUTHORIZATION, authorization);
    }
    builder.body(Body::from("{}")).expect("request")
}

fn application(harness: &RouteHarness) -> Router {
    http::router(harness.state.clone())
}

fn session_checks(harness: &RouteHarness) -> Vec<(String, String)> {
    harness.clients.session_checks.lock().unwrap().clone()
}

#[tokio::test]
async fn a_live_admin_session_reaches_the_handler_and_is_confirmed_once_per_window() {
    let harness = RouteHarness::new().await;
    let token = admin_token(&harness);

    for _ in 0..2 {
        let response = application(&harness)
            .oneshot(request(
                Method::GET,
                "/admin/moderation/reports?limit=20",
                Some(&token),
            ))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    // The session the token names was confirmed, and the second request
    // reused that confirmation.
    assert_eq!(
        session_checks(&harness),
        vec![(USER_ID.to_owned(), SESSION_ID.to_owned())]
    );
    let rendered = harness.state.metrics.render_prometheus();
    assert!(rendered.contains(
        "gateway_high_risk_session_checks_total{action=\"administration\",outcome=\"allowed\"} 1"
    ));
    assert!(rendered.contains(
        "gateway_high_risk_session_checks_total{action=\"administration\",outcome=\"allowed_cached\"} 1"
    ));
}

#[tokio::test]
async fn a_logged_out_admin_session_is_refused_before_its_token_expires() {
    let harness = RouteHarness::new().await;
    harness.clients.answer_sessions(SessionAnswer::Ended);

    let response = application(&harness)
        .oneshot(request(
            Method::GET,
            "/admin/moderation/reports?limit=20",
            Some(&admin_token(&harness)),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response_json(response).await["message"],
        "Session has ended; sign in again"
    );
    assert!(harness
        .state
        .metrics
        .render_prometheus()
        .contains("outcome=\"session_ended\"} 1"));
}

#[tokio::test]
async fn a_token_that_still_claims_a_removed_admin_role_is_forbidden() {
    let harness = RouteHarness::new().await;
    harness
        .clients
        .answer_sessions(SessionAnswer::Active(vec!["ROLE_USER".into()]));

    let response = application(&harness)
        .oneshot(request(
            Method::GET,
            "/admin/moderation/reports?limit=20",
            Some(&admin_token(&harness)),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_demoted_admin_cannot_retry_a_dead_lettered_deletion() {
    let harness = RouteHarness::new().await;
    harness
        .clients
        .answer_sessions(SessionAnswer::Active(vec!["ROLE_USER".into()]));

    let response = application(&harness)
        .oneshot(request(
            Method::POST,
            "/admin/account-deletions/aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa/retry",
            Some(&admin_token(&harness)),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    assert!(harness
        .clients
        .retry_user_deletion_calls
        .lock()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn a_demoted_admin_cannot_inspect_account_deletion_backlog_or_detail() {
    let harness = RouteHarness::new().await;
    harness
        .clients
        .answer_sessions(SessionAnswer::Active(vec!["ROLE_USER".into()]));
    let app = application(&harness);

    for path in [
        "/admin/account-deletions?limit=20",
        "/admin/account-deletions/aaaaaaaa-aaaa-aaaa-aaaa-aaaaaaaaaaaa",
    ] {
        let response = app
            .clone()
            .oneshot(request(Method::GET, path, Some(&admin_token(&harness))))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN, "{path}");
    }
    assert!(harness
        .clients
        .list_dead_lettered_deletion_calls
        .lock()
        .unwrap()
        .is_empty());
    assert!(harness
        .clients
        .get_deletion_by_saga_id_calls
        .lock()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn an_unreachable_issuer_fails_high_risk_requests_closed() {
    let harness = RouteHarness::new().await;
    harness.clients.answer_sessions(SessionAnswer::Unavailable);

    let response = application(&harness)
        .oneshot(request(
            Method::GET,
            "/admin/moderation/reports?limit=20",
            Some(&admin_token(&harness)),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::SERVICE_UNAVAILABLE);
    assert!(harness
        .state
        .metrics
        .render_prometheus()
        .contains("outcome=\"unavailable\"} 1"));
}

#[tokio::test]
async fn a_token_without_a_session_must_sign_in_again_for_high_risk_actions() {
    let harness = RouteHarness::new().await;
    let legacy = harness
        .bearer_token_without_session(USER_ID, vec!["ROLE_USER".into(), "ROLE_ADMIN".into()]);

    let admin = application(&harness)
        .oneshot(request(
            Method::GET,
            "/admin/moderation/reports?limit=20",
            Some(&legacy),
        ))
        .await
        .unwrap();
    let ordinary = application(&harness)
        .oneshot(request(Method::GET, "/auth/account", Some(&legacy)))
        .await
        .unwrap();

    assert_eq!(admin.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(
        response_json(admin).await["message"],
        "Sign in again to continue"
    );
    // The same token still serves ordinary requests until it expires.
    assert_eq!(ordinary.status(), StatusCode::OK);
    assert!(session_checks(&harness).is_empty());
}

#[tokio::test]
async fn a_request_without_a_bearer_token_is_refused_by_its_handler_without_a_check() {
    let harness = RouteHarness::new().await;

    let response = application(&harness)
        .oneshot(request(Method::DELETE, "/auth/account", None))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert!(session_checks(&harness).is_empty());
}

#[tokio::test]
async fn a_logged_out_session_cannot_delete_the_account_or_link_a_provider() {
    let harness = RouteHarness::new().await;
    harness.clients.answer_sessions(SessionAnswer::Ended);
    let token = harness.bearer_token(USER_ID);

    let deletion = application(&harness)
        .oneshot(request(Method::DELETE, "/auth/account", Some(&token)))
        .await
        .unwrap();
    let link = application(&harness)
        .oneshot(request(
            Method::POST,
            "/auth/oauth/google/link",
            Some(&token),
        ))
        .await
        .unwrap();

    assert_eq!(deletion.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(link.status(), StatusCode::UNAUTHORIZED);
    assert!(harness
        .clients
        .delete_account_calls
        .lock()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn a_live_session_can_still_delete_its_account() {
    let harness = RouteHarness::new().await;

    let response = application(&harness)
        .oneshot(request(
            Method::DELETE,
            "/auth/account",
            Some(&harness.bearer_token(USER_ID)),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::ACCEPTED);
    assert_eq!(
        harness.clients.delete_account_calls.lock().unwrap().len(),
        1
    );
}

#[tokio::test]
async fn ordinary_requests_trust_the_token_until_it_expires() {
    // The documented bound for everything outside the high-risk set is the
    // access-token lifetime; those requests never wait on the issuer.
    let harness = RouteHarness::new().await;
    harness.clients.answer_sessions(SessionAnswer::Ended);

    let response = application(&harness)
        .oneshot(request(
            Method::GET,
            "/auth/account",
            Some(&harness.bearer_token(USER_ID)),
        ))
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    assert!(session_checks(&harness).is_empty());
}

/// Every documented operation the classifier marks high-risk is refused for an
/// ended session by the router the executable serves. A route added outside the
/// layered routers would fail here.
#[tokio::test]
async fn every_documented_high_risk_operation_is_enforced_by_the_router() {
    let contract = api_docs_routes::router(
        RuntimeMode::Parity,
        api_gateway::config::DeveloperPortalLinks::defaults(RuntimeMode::Parity),
    )
    .oneshot(request(Method::GET, "/openapi.json", None))
    .await
    .unwrap();
    let contract: Value =
        serde_json::from_slice(&to_bytes(contract.into_body(), 8 << 20).await.unwrap()).unwrap();
    let harness = RouteHarness::new().await;
    harness.clients.answer_sessions(SessionAnswer::Ended);
    let token = admin_token(&harness);

    let mut enforced = Vec::new();
    for (path, operations) in contract["paths"].as_object().unwrap() {
        let template = path.replace('{', ":").replace('}', "");
        for method in operations.as_object().unwrap().keys() {
            let method = method.to_ascii_uppercase();
            if classify(&method, &template).is_none() {
                continue;
            }
            let uri = path
                .replace("{provider}", "google")
                .replace("{version}", "1")
                .split('/')
                .map(|segment| {
                    if segment.starts_with('{') {
                        OTHER_USER_ID
                    } else {
                        segment
                    }
                })
                .collect::<Vec<_>>()
                .join("/");
            let response = application(&harness)
                .oneshot(request(
                    Method::from_bytes(method.as_bytes()).unwrap(),
                    &uri,
                    Some(&token),
                ))
                .await
                .unwrap();
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{method} {path} was not confirmed with the issuer"
            );
            enforced.push(format!("{method} {path}"));
        }
    }

    // Guard against a vacuous pass: the contract documents the whole admin
    // surface, account deletion, and provider linking.
    assert!(enforced.len() >= 22, "only {enforced:?}");
    assert!(enforced.contains(&"DELETE /auth/account".to_owned()));
    assert!(enforced.contains(&"POST /auth/oauth/{provider}/link".to_owned()));
}
