use crate::http_support::{RouteHarness, SESSION_ID, USER_ID};
use api_gateway::adapters::r#in::http::{self, concert_access::HttpConcertAccess};
use axum::body::Body;
use axum::extract::{Request as AxumRequest, State};
use axum::http::{header, Request, StatusCode};
use axum::middleware::{self, Next};
use axum::routing::get;
use axum::Router;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tower::ServiceExt;

async fn trusted_deadline(
    State(state): State<Arc<api_gateway::config::AppState>>,
    request: AxumRequest<Body>,
    next: Next,
) -> axum::response::Response {
    let mut request = request;
    request.extensions_mut().insert(http::RequestDeadline(
        std::time::Instant::now() + state.request_timeout,
    ));
    next.run(request).await
}

async fn short_deadline(
    State(state): State<Arc<api_gateway::config::AppState>>,
    request: AxumRequest<Body>,
    next: Next,
) -> axum::response::Response {
    let mut request = request;
    request.extensions_mut().insert(http::RequestDeadline(
        Instant::now() + Duration::from_millis(120),
    ));
    next.run(request).await
}

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
            trusted_deadline,
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
                tokio::time::sleep(Duration::from_millis(55)).await;
                let remaining = access.remaining(Duration::from_secs(10)).unwrap();
                if remaining > Duration::from_millis(80) {
                    StatusCode::INTERNAL_SERVER_ERROR
                } else {
                    StatusCode::NO_CONTENT
                }
            }),
        )
        .layer(middleware::from_fn_with_state(
            state.clone(),
            short_deadline,
        ))
        .with_state(state);
    let response = router
        .oneshot(request(&format!("Bearer {token}")))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
    assert!(before.elapsed() < Duration::from_millis(180));
}

#[tokio::test]
async fn missing_or_expired_trusted_deadline_does_not_dispatch_context_handler() {
    let harness = RouteHarness::new().await;
    let calls = Arc::new(AtomicUsize::new(0));
    let state = harness.state.clone();
    let missing = Router::new()
        .route(
            "/concert-context",
            get(move |_access: HttpConcertAccess| async move { StatusCode::NO_CONTENT }),
        )
        .with_state(state.clone());
    let response = missing
        .oneshot(request(&harness.bearer_token(USER_ID)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::GATEWAY_TIMEOUT);

    let expired = Router::new()
        .route(
            "/concert-context",
            get(move |_access: HttpConcertAccess| async move { StatusCode::NO_CONTENT }),
        )
        .layer(middleware::from_fn(
            |mut request: Request<Body>, next: Next| async move {
                request.extensions_mut().insert(http::RequestDeadline(
                    std::time::Instant::now() - std::time::Duration::from_millis(1),
                ));
                next.run(request).await
            },
        ))
        .with_state(state);
    let response = expired
        .oneshot(request(&harness.bearer_token(USER_ID)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
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

#[tokio::test(start_paused = true)]
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
            short_deadline,
        ))
        .with_state(state);
    let response = router
        .oneshot(request(&RouteHarness::new().await.bearer_token(USER_ID)))
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
