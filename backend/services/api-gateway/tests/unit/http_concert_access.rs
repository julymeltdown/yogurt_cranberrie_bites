use crate::http_support::{RouteHarness, SESSION_ID, USER_ID};
use api_gateway::adapters::r#in::http::{self, concert_access::HttpConcertAccess};
use axum::body::Body;
use axum::extract::Request as AxumRequest;
use axum::http::{header, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::routing::get;
use axum::Router;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower::ServiceExt;

fn app(harness: &RouteHarness, calls: Arc<AtomicUsize>, delay_ms: u64) -> Router {
    let state = harness.state.clone();
    Router::new()
        .route(
            "/concert-context",
            get(move |access: HttpConcertAccess| async move {
                calls.fetch_add(1, Ordering::SeqCst);
                assert!(access.check().is_ok());
                assert_eq!(access.user_id().unwrap().to_string(), USER_ID);
                assert_eq!(access.session_family().unwrap().to_string(), SESSION_ID);
                StatusCode::NO_CONTENT
            }),
        )
        .layer(middleware::from_fn(
            move |request: AxumRequest, next: Next| async move {
                if delay_ms > 0 {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }
                next.run(request).await
            },
        ))
        .layer(middleware::from_fn_with_state(
            state.clone(),
            http::timeout_middleware,
        ))
        .with_state(state)
}

fn strict_token() -> (String, String) {
    use base64ct::Encoding;
    use jsonwebtoken::{Algorithm, EncodingKey, Header};
    use rand::rngs::OsRng;
    use rsa::pkcs8::EncodePrivateKey;
    use rsa::traits::PublicKeyParts;
    use rsa::{RsaPrivateKey, RsaPublicKey};
    use serde_json::json;
    let private = RsaPrivateKey::new(&mut OsRng, 2048).expect("private key");
    let public = RsaPublicKey::from(&private);
    let pem = private.to_pkcs8_pem(Default::default()).expect("pem");
    let key = EncodingKey::from_rsa_pem(pem.as_bytes()).expect("key");
    let jwks = json!({"keys":[{"kty":"RSA","use":"sig","alg":"RS256","kid":"strict-test",
        "n":base64ct::Base64UrlUnpadded::encode_string(&public.n().to_bytes_be()),
        "e":base64ct::Base64UrlUnpadded::encode_string(&public.e().to_bytes_be())}]})
    .to_string();
    let claims = json!({"sub":USER_ID,"sid":SESSION_ID,"iss":"auth-service-test",
        "aud":"port-rust-api-test","exp":chrono::Utc::now().timestamp()+900,
        "token_type":"access","roles":["ROLE_USER"]});
    let token = jsonwebtoken::encode(
        &Header {
            kid: Some("strict-test".into()),
            ..Header::new(Algorithm::RS256)
        },
        &claims,
        &key,
    )
    .expect("token");
    (token, jwks)
}

fn request(token: &str) -> Request<Body> {
    Request::builder()
        .uri("/concert-context")
        .header(header::AUTHORIZATION, token)
        .body(Body::empty())
        .unwrap()
}

#[tokio::test]
async fn verified_request_captures_exact_private_context_after_middleware_delay() {
    let harness = RouteHarness::new().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let response = app(&harness, calls.clone(), 30)
        .oneshot(request(&harness.bearer_token(USER_ID)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn handler_delay_consumes_the_original_deadline_without_extension() {
    let (token, jwks) = strict_token();
    let harness = RouteHarness::with_configured_keys(&jwks).await.unwrap();
    let state = harness.state.clone();
    let before = Instant::now();
    let router = Router::new()
        .route(
            "/concert-context",
            get(|access: HttpConcertAccess| async move {
                tokio::time::sleep(Duration::from_millis(30)).await;
                let remaining = access.remaining(Duration::from_secs(10)).unwrap();
                if remaining >= Duration::from_secs(1) {
                    StatusCode::INTERNAL_SERVER_ERROR
                } else {
                    StatusCode::NO_CONTENT
                }
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            http::timeout_middleware,
        ))
        .with_state(state);
    let response = router
        .oneshot(request(&format!("Bearer {token}")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(before.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn missing_trusted_deadline_is_refused_without_context_dispatch() {
    let harness = RouteHarness::new().await;
    let state = harness.state.clone();
    let missing_calls = Arc::new(AtomicUsize::new(0));
    let missing_calls_for_route = missing_calls.clone();
    let missing = Router::new()
        .route(
            "/concert-context",
            get(move |_access: HttpConcertAccess| {
                let missing_calls_for_route = missing_calls_for_route.clone();
                async move {
                    missing_calls_for_route.fetch_add(1, Ordering::SeqCst);
                    StatusCode::NO_CONTENT
                }
            }),
        )
        .with_state(state.clone());
    let response = missing
        .oneshot(request(&harness.bearer_token(USER_ID)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    assert_eq!(missing_calls.load(Ordering::SeqCst), 0);
}

#[tokio::test]
async fn production_timeout_refuses_stalled_context_before_handler_counter_runs() {
    #[derive(Clone)]
    struct Stalled(Arc<AtomicUsize>);
    async fn stalled(axum::extract::State(calls): axum::extract::State<Stalled>) -> StatusCode {
        calls.0.fetch_add(1, Ordering::SeqCst);
        std::future::pending().await
    }
    let harness = RouteHarness::new().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let started = Instant::now();
    let response = Router::new()
        .route("/concert-context", get(stalled))
        .with_state(Stalled(calls.clone()))
        .layer(middleware::from_fn_with_state(
            harness.state.clone(),
            http::timeout_middleware,
        ))
        .oneshot(request(&harness.bearer_token(USER_ID)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert!(started.elapsed() >= Duration::from_millis(800));
    assert!(started.elapsed() < Duration::from_secs(2));
}

#[tokio::test]
async fn malformed_missing_session_and_foreign_issuer_tokens_fail_closed() {
    let harness = RouteHarness::new().await;
    for token in [
        "Bearer not-a-token".into(),
        harness.bearer_token_without_session(USER_ID, vec!["ROLE_USER".into()]),
    ] {
        let calls = Arc::new(AtomicUsize::new(0));
        let response = app(&harness, calls.clone(), 0)
            .oneshot(request(&token))
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
        assert_eq!(calls.load(Ordering::SeqCst), 0);
    }
}

#[tokio::test]
async fn unknown_key_refresh_wait_cannot_extend_original_deadline() {
    use api_gateway::adapters::out::signing_keys::{RefreshPolicy, SigningKeyRefresher};
    use api_gateway::application::dto::issuer_dto::SigningKeysDto;
    use api_gateway::application::ports::SigningKeySourcePort;
    use async_trait::async_trait;

    struct SlowIssuer;
    #[async_trait]
    impl SigningKeySourcePort for SlowIssuer {
        async fn signing_keys(
            &self,
        ) -> api_gateway::application::client_error::GatewayClientResult<SigningKeysDto> {
            tokio::time::sleep(Duration::from_secs(5)).await;
            Err(
                api_gateway::adapters::out::client_error::classify_client_error(anyhow::anyhow!(
                    "unavailable"
                )),
            )
        }
    }
    let verifier = api_gateway::adapters::out::jwt::JwtVerifier::without_keys(
        "auth-service-test",
        "port-rust-api-test",
    )
    .unwrap();
    let mut state = (*RouteHarness::new().await.state).clone();
    state.jwt_verifier = verifier.clone();
    state.signing_keys = Some(SigningKeyRefresher::new(
        verifier,
        Arc::new(SlowIssuer),
        RefreshPolicy {
            interval: Duration::from_secs(60),
            unknown_kid_min_interval: Duration::from_secs(10),
        },
        None,
    ));
    let state = Arc::new(state);
    let router = Router::new()
        .route(
            "/concert-context",
            get(|_access: HttpConcertAccess| async move { StatusCode::NO_CONTENT }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            http::timeout_middleware,
        ))
        .with_state(state);
    let started = Instant::now();
    let response = router
        .oneshot(request(&RouteHarness::new().await.bearer_token(USER_ID)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);
    assert!(started.elapsed() >= Duration::from_millis(800));
    assert!(started.elapsed() < Duration::from_secs(2));
}
