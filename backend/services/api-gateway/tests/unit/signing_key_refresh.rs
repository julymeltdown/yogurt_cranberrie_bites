//! The signing-key refresher and its effect on protected routes.
//!
//! Refresher tests run on a paused clock, so the unknown-kid rate limit is
//! checked exactly instead of with sleeps.

use crate::http_support::{RouteHarness, USER_ID};
use crate::signing_key_support::{first_key, key_set, second_key, weak_key, AUDIENCE, ISSUER};
use api_gateway::adapters::out::jwt::{JwtVerifier, TokenRejection, MAX_JWKS_DOCUMENT_BYTES};
use api_gateway::adapters::out::signing_keys::{
    RefreshOutcome, RefreshPolicy, SigningKeyRefresher,
};
use api_gateway::adapters::r#in::http::post_routes;
use api_gateway::application::dto::issuer_dto::SigningKeysDto;
use api_gateway::application::ports::SigningKeySourcePort;
use api_gateway::config::{AppState, GatewayMetrics};
use async_trait::async_trait;
use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use jsonwebtoken::Header;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
use tower::ServiceExt;

const POLICY: RefreshPolicy = RefreshPolicy {
    interval: Duration::from_secs(60),
    unknown_kid_min_interval: Duration::from_secs(10),
};

#[derive(Clone)]
enum Answer {
    Document(String),
    Failure,
    /// The issuer accepts the call and never answers.
    Stall,
}

/// An issuer double that counts every fetch.
struct FakeIssuer {
    answer: Mutex<Answer>,
    calls: AtomicU64,
}

impl FakeIssuer {
    fn publishing(document: String) -> Arc<Self> {
        Arc::new(Self {
            answer: Mutex::new(Answer::Document(document)),
            calls: AtomicU64::new(0),
        })
    }

    fn answer(&self, answer: Answer) {
        *self.answer.lock().expect("issuer answer") = answer;
    }

    fn calls(&self) -> u64 {
        self.calls.load(Ordering::SeqCst)
    }
}

impl FakeIssuer {
    async fn signing_keys_signing_key_source_port_inner(&self) -> anyhow::Result<SigningKeysDto> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let answer = self.answer.lock().expect("issuer answer").clone();
        match answer {
            Answer::Document(document) => Ok(SigningKeysDto {
                jwks_json: document,
            }),
            Answer::Failure => anyhow::bail!("issuer unavailable"),
            Answer::Stall => std::future::pending().await,
        }
    }
}

#[async_trait]
impl SigningKeySourcePort for FakeIssuer {
    async fn signing_keys(
        &self,
    ) -> api_gateway::application::client_error::GatewayClientResult<SigningKeysDto> {
        self.signing_keys_signing_key_source_port_inner()
            .await
            .map_err(api_gateway::adapters::out::client_error::classify_client_error)
    }
}

fn refresher(
    verifier: &JwtVerifier,
    issuer: &Arc<FakeIssuer>,
    metrics: Option<GatewayMetrics>,
) -> SigningKeyRefresher {
    SigningKeyRefresher::new(verifier.clone(), issuer.clone(), POLICY, metrics)
}

fn verifier_with(keys: &[&crate::signing_key_support::SigningKey]) -> JwtVerifier {
    JwtVerifier::from_jwks_json(ISSUER, AUDIENCE, &key_set(keys)).expect("verifier")
}

fn gauge(metrics: &GatewayMetrics, name: &str) -> Option<i64> {
    let prefix = format!("{name}{{}} ");
    metrics.render_prometheus().lines().find_map(|line| {
        line.strip_prefix(prefix.as_str())
            .map(|value| value.parse().expect("gauge value"))
    })
}

#[tokio::test(start_paused = true)]
async fn a_scheduled_refresh_installs_a_key_published_after_startup() {
    // Given: a Gateway started with the first key, and an issuer that has since
    // published the second.
    let verifier = verifier_with(&[first_key()]);
    let issuer = FakeIssuer::publishing(key_set(&[first_key(), second_key()]));
    let rotated = second_key().access_token("member");
    assert!(matches!(
        verifier.verify_access_token(&rotated),
        Err(TokenRejection::UnknownKid)
    ));

    // When: the scheduled refresh runs.
    let outcome = refresher(&verifier, &issuer, None).refresh().await;

    // Then: the new key verifies without a restart, and the first key still does.
    assert_eq!(outcome, RefreshOutcome::Installed);
    assert_eq!(
        verifier
            .verify_access_token(&rotated)
            .expect("rotated key")
            .subject,
        "member"
    );
    assert!(verifier
        .verify_access_token(&first_key().access_token("member"))
        .is_ok());
}

#[tokio::test(start_paused = true)]
async fn the_scheduled_loop_refreshes_at_once_and_then_every_interval() {
    // Given: a Gateway started without configured keys.
    let verifier = JwtVerifier::without_keys(ISSUER, AUDIENCE).expect("verifier");
    let issuer = FakeIssuer::publishing(key_set(&[first_key()]));

    // When: the refresh loop starts.
    let task = tokio::spawn(refresher(&verifier, &issuer, None).run());

    // Then: the first refresh does not wait for an interval, so keys arrive as
    // soon as the issuer answers, and later refreshes follow the interval.
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert_eq!((issuer.calls(), verifier.key_count()), (1, 1));
    tokio::time::sleep(POLICY.interval).await;
    assert_eq!(issuer.calls(), 2);
    task.abort();
}

#[tokio::test(start_paused = true)]
async fn a_gateway_without_keys_retries_a_failed_first_fetch_at_the_short_bound() {
    // Given: a Gateway started without configured keys while its issuer is not
    // answering yet, as when an orchestrator starts both at once.
    let verifier = JwtVerifier::without_keys(ISSUER, AUDIENCE).expect("verifier");
    let issuer = FakeIssuer::publishing(key_set(&[first_key()]));
    issuer.answer(Answer::Failure);

    // When: the refresh loop starts and the issuer comes up shortly after.
    let task = tokio::spawn(refresher(&verifier, &issuer, None).run());
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert_eq!((issuer.calls(), verifier.key_count()), (1, 0));
    issuer.answer(Answer::Document(key_set(&[first_key()])));

    // Then: the retry follows the unknown-kid bound, well inside one interval.
    tokio::time::sleep(POLICY.unknown_kid_min_interval).await;
    assert_eq!((issuer.calls(), verifier.key_count()), (2, 1));

    // And: with keys installed, refreshes return to the ordinary interval.
    tokio::time::sleep(POLICY.interval - Duration::from_millis(2)).await;
    assert_eq!(issuer.calls(), 2);
    tokio::time::sleep(Duration::from_millis(3)).await;
    assert_eq!(issuer.calls(), 3);
    task.abort();
}

#[tokio::test(start_paused = true)]
async fn configured_keys_keep_the_ordinary_schedule_when_a_fetch_fails() {
    // Given: a Gateway that already verifies with a configured key.
    let verifier = verifier_with(&[first_key()]);
    let issuer = FakeIssuer::publishing(key_set(&[first_key()]));
    issuer.answer(Answer::Failure);

    // When: the refresh loop starts and its first fetch fails.
    let task = tokio::spawn(refresher(&verifier, &issuer, None).run());
    tokio::time::sleep(Duration::from_millis(1)).await;
    assert_eq!(issuer.calls(), 1);

    // Then: it keeps its key and waits a whole interval, without fast retries.
    tokio::time::sleep(POLICY.unknown_kid_min_interval * 2).await;
    assert_eq!((issuer.calls(), verifier.key_count()), (1, 1));
    tokio::time::sleep(POLICY.interval).await;
    assert_eq!(issuer.calls(), 2);
    task.abort();
}

#[tokio::test(start_paused = true)]
async fn an_unknown_kid_refreshes_once_and_the_token_then_verifies() {
    // Given: an issuer that published a key after the last scheduled refresh.
    let verifier = verifier_with(&[first_key()]);
    let issuer = FakeIssuer::publishing(key_set(&[first_key(), second_key()]));
    let refresher = refresher(&verifier, &issuer, None);

    // When: a token names that key.
    let refreshed = refresher.refresh_for_unknown_kid().await;

    // Then: one early fetch installs it, and the token verifies on the retry.
    assert!(refreshed);
    assert_eq!(issuer.calls(), 1);
    assert!(verifier
        .verify_access_token(&second_key().access_token("member"))
        .is_ok());
}

#[tokio::test(start_paused = true)]
async fn a_flood_of_unknown_kids_starts_one_fetch_per_interval() {
    // Given: a refresher with metrics.
    let metrics = GatewayMetrics::default();
    let verifier = verifier_with(&[first_key()]);
    let issuer = FakeIssuer::publishing(key_set(&[first_key()]));
    let refresher = refresher(&verifier, &issuer, Some(metrics.clone()));

    // When: a thousand requests name made-up keys at once.
    let mut requests = tokio::task::JoinSet::new();
    for _ in 0..1_000 {
        let refresher = refresher.clone();
        requests.spawn(async move { refresher.refresh_for_unknown_kid().await });
    }
    let mut refreshed = 0;
    while let Some(result) = requests.join_next().await {
        if result.expect("request task") {
            refreshed += 1;
        }
    }

    // Then: one fetch was made, and every other request was answered from the
    // installed keys without waiting for it.
    assert_eq!((issuer.calls(), refreshed), (1, 1));
    let rendered = metrics.render_prometheus();
    assert!(
        rendered.contains(
            "gateway_jwt_key_refreshes_total{trigger=\"unknown_kid\",outcome=\"suppressed\"} 999"
        ),
        "{rendered}"
    );

    // And: the limit is a rate, not a lockout.
    tokio::time::advance(POLICY.unknown_kid_min_interval - Duration::from_millis(1)).await;
    assert!(!refresher.refresh_for_unknown_kid().await);
    tokio::time::advance(Duration::from_millis(1)).await;
    assert!(refresher.refresh_for_unknown_kid().await);
    assert_eq!(issuer.calls(), 2);
}

#[tokio::test(start_paused = true)]
async fn a_scheduled_refresh_counts_against_the_unknown_kid_limit() {
    // Given: a scheduled refresh that has just run.
    let verifier = verifier_with(&[first_key()]);
    let issuer = FakeIssuer::publishing(key_set(&[first_key()]));
    let refresher = refresher(&verifier, &issuer, None);
    refresher.refresh().await;

    // When/Then: an unknown kid right after it starts no second fetch, since the
    // keys are already as fresh as the limit allows.
    assert!(!refresher.refresh_for_unknown_kid().await);
    assert_eq!(issuer.calls(), 1);
}

#[tokio::test(start_paused = true)]
async fn a_failed_or_unusable_answer_keeps_the_installed_keys() {
    // Given: a refresher that has installed the first key.
    let metrics = GatewayMetrics::default();
    let verifier = JwtVerifier::without_keys(ISSUER, AUDIENCE).expect("verifier");
    let issuer = FakeIssuer::publishing(key_set(&[first_key()]));
    let refresher = refresher(&verifier, &issuer, Some(metrics.clone()));
    assert_eq!(refresher.refresh().await, RefreshOutcome::Installed);
    let refreshed_at = gauge(&metrics, "gateway_jwt_keys_last_refresh_timestamp_seconds");
    assert!(refreshed_at.is_some());
    assert_eq!(gauge(&metrics, "gateway_jwt_verification_keys"), Some(1));
    // The configured interval is exported, so staleness is judged against it.
    assert_eq!(
        gauge(&metrics, "gateway_jwt_keys_refresh_interval_seconds"),
        Some(60)
    );

    for (answer, expected) in [
        (Answer::Failure, RefreshOutcome::FetchFailed),
        // Never answering is a failure after the fetch deadline, not a hang.
        (Answer::Stall, RefreshOutcome::FetchFailed),
        (
            Answer::Document("x".repeat(MAX_JWKS_DOCUMENT_BYTES + 1)),
            RefreshOutcome::Rejected,
        ),
        (
            Answer::Document(r#"{"keys":[]}"#.into()),
            RefreshOutcome::Rejected,
        ),
        (
            Answer::Document("<html>maintenance</html>".into()),
            RefreshOutcome::Rejected,
        ),
    ] {
        // When: the issuer fails or answers with something unusable.
        issuer.answer(answer);
        assert_eq!(refresher.refresh().await, expected);

        // Then: the last good keys keep verifying.
        assert_eq!(verifier.key_count(), 1);
        assert!(verifier
            .verify_access_token(&first_key().access_token("member"))
            .is_ok());
    }

    // And: failures are counted by kind, and the last-refresh time stays put, so
    // its age shows how stale the installed keys are.
    let rendered = metrics.render_prometheus();
    for expected in [
        "gateway_jwt_key_refreshes_total{trigger=\"scheduled\",outcome=\"fetch_failed\"} 2",
        "gateway_jwt_key_refreshes_total{trigger=\"scheduled\",outcome=\"rejected\"} 3",
    ] {
        assert!(rendered.contains(expected), "{rendered}");
    }
    assert_eq!(
        gauge(&metrics, "gateway_jwt_keys_last_refresh_timestamp_seconds"),
        refreshed_at
    );
}

#[tokio::test(start_paused = true)]
async fn a_withdrawn_key_stops_verifying_after_the_next_refresh() {
    // Given: a Gateway holding both keys, and an issuer that has withdrawn the
    // first after a compromise.
    let verifier = verifier_with(&[first_key(), second_key()]);
    let issuer = FakeIssuer::publishing(key_set(&[second_key()]));
    let compromised = first_key().access_token("member");
    assert!(verifier.verify_access_token(&compromised).is_ok());

    // When: the next refresh runs.
    let refresher = refresher(&verifier, &issuer, None);
    refresher.refresh().await;

    // Then: tokens signed with the withdrawn key are refused and cannot bring it
    // back through an early refresh, while the remaining key keeps verifying.
    assert!(matches!(
        verifier.verify_access_token(&compromised),
        Err(TokenRejection::UnknownKid)
    ));
    tokio::time::advance(POLICY.unknown_kid_min_interval).await;
    assert!(refresher.refresh_for_unknown_kid().await);
    assert!(matches!(
        verifier.verify_access_token(&compromised),
        Err(TokenRejection::UnknownKid)
    ));
    assert!(verifier
        .verify_access_token(&second_key().access_token("member"))
        .is_ok());
}

#[tokio::test]
async fn a_protected_route_accepts_a_key_published_after_startup_without_a_restart() {
    // Given: a running Gateway, and an issuer that published a key after it
    // started.
    let harness = RouteHarness::new().await;
    let issuer = FakeIssuer::publishing(key_set(&[second_key()]));
    let app = post_routes::router(with_refresher(&harness, &issuer));

    // When: a member presents a token signed with the new key.
    let response = app
        .oneshot(feed_request(&second_key().access_token(USER_ID)))
        .await
        .expect("response");

    // Then: one early refresh fetched the key and the request succeeded.
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(issuer.calls(), 1);
}

#[tokio::test]
async fn made_up_key_ids_cost_one_fetch_however_many_requests_carry_them() {
    // Given: a running Gateway that refreshes from an issuer.
    let harness = RouteHarness::new().await;
    let issuer = FakeIssuer::publishing(key_set(&[second_key()]));
    let state = with_refresher(&harness, &issuer);
    let app = post_routes::router(state.clone());

    // When: fifty requests carry tokens that name keys no issuer published.
    for index in 0..50 {
        let header = Header {
            kid: Some(format!("made-up-{index}")),
            ..second_key().header()
        };
        let token = second_key().access_token_with_header(USER_ID, header);
        let response = app
            .clone()
            .oneshot(feed_request(&token))
            .await
            .expect("response");
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    // Then: the issuer was asked once, and the refusals are reported as unknown
    // signing keys, the signal an operator watches during a rotation.
    assert_eq!(issuer.calls(), 1);
    let rendered = state.metrics.render_prometheus();
    assert!(
        rendered
            .contains("gateway_jwt_verification_failures_total{reason=\"unknown_signing_key\"} 50"),
        "{rendered}"
    );
}

#[tokio::test]
async fn a_gateway_without_configured_keys_serves_once_the_issuer_answers() {
    // Given: a Gateway started with no configured keys.
    let harness = RouteHarness::with_configured_keys("")
        .await
        .expect("a Gateway may start without configured keys");
    assert_eq!(harness.state.jwt_verifier.key_count(), 0);
    let issuer = FakeIssuer::publishing(key_set(&[first_key()]));
    let app = post_routes::router(with_refresher(&harness, &issuer));

    // When: an authenticated request arrives before any scheduled refresh.
    let response = app
        .oneshot(feed_request(&first_key().access_token(USER_ID)))
        .await
        .expect("response");

    // Then: its unknown key starts the first fetch, and the request is served.
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(issuer.calls(), 1);
}

#[tokio::test]
async fn an_unusable_configured_key_set_stops_startup_and_names_the_setting() {
    // Given/When: a Gateway configured with a key below the accepted size.
    let error = match RouteHarness::with_configured_keys(&key_set(&[weak_key()])).await {
        Ok(_) => panic!("startup must fail"),
        Err(error) => error.to_string(),
    };

    // Then: it refuses to start and says which setting to fix.
    assert!(error.contains("GATEWAY_JWT_JWKS_JSON"), "{error}");
}

fn with_refresher(harness: &RouteHarness, issuer: &Arc<FakeIssuer>) -> Arc<AppState> {
    let mut state = (*harness.state).clone();
    state.signing_keys = Some(SigningKeyRefresher::new(
        state.jwt_verifier.clone(),
        issuer.clone(),
        POLICY,
        Some(state.metrics.clone()),
    ));
    Arc::new(state)
}

fn feed_request(token: &str) -> Request<Body> {
    Request::builder()
        .method(Method::GET)
        .uri("/api/feed")
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request")
}
