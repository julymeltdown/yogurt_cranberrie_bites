//! T-202: the provider client, against a fake provider on loopback.
//!
//! The client under test is the production `NaverProviderClient`. Only the far
//! end is fake, so the real token exchange, the real bearer fetch, the real size
//! bound and the real error classification all run. No test here makes a network
//! request off the host.
//!
//! The cases are the six the task document names: success, 401, 500, timeout,
//! oversized body, and the token-redaction assertion on `Debug`.
use profile_service::identity::naver_profile::NaverGender;
use profile_service::identity::naver_provider::{
    NaverAccessToken, NaverProviderClient, NaverProviderEndpoints, NaverProviderError,
    NaverProviderFailure, NaverProviderStage,
};
use serde_json::json;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;

#[path = "wp04b_support.rs"]
mod support;
use support::{FakeProfile, FakeProvider};

#[tokio::test]
async fn a_successful_exchange_and_fetch_returns_the_claim_inputs() {
    let provider = FakeProvider::start(FakeProfile::woman()).await.expect("fake provider");
    let client = provider.client().expect("client");
    let token = client
        .exchange_authorization_code("fake-code", "fake-state")
        .await
        .expect("exchange succeeds");
    assert_eq!(token.expose(), provider.access_token);
    let profile = client.load_profile(&token).await.expect("profile loads");
    assert_eq!(profile.subject.as_deref(), Some("naver-subject-woman"));
    assert_eq!(profile.gender, Some(NaverGender::Female));
    assert_eq!(profile.birthyear.as_deref(), Some("2000"));
    assert_eq!(profile.birthday.as_deref(), Some("07-12"));

    // The exchange was a form POST carrying the configured client id, secret,
    // redirect URI and state, and the fetch was a bearer GET of the configured
    // profile URL. Nothing else was called.
    let requests = provider.requests();
    assert_eq!(requests.len(), 2, "{requests:?}");
    assert_eq!(requests[0].method, "POST");
    assert_eq!(requests[0].path, "/token");
    for field in [
        "grant_type=authorization_code",
        "code=fake-code",
        "state=fake-state",
        "client_id=test-client",
        "client_secret=test-secret",
        "redirect_uri=http",
    ] {
        assert!(requests[0].body.contains(field), "{field} missing: {}", requests[0].body);
    }
    assert_eq!(requests[1].method, "GET");
    assert_eq!(requests[1].path, "/profile");
    assert_eq!(
        requests[1].authorization.as_deref(),
        Some("Bearer fake-naver-token")
    );
}

#[tokio::test]
async fn a_token_401_is_a_typed_rejection_without_the_token() {
    let provider = FakeProvider::start(FakeProfile::woman()).await.expect("fake provider");
    provider.set_token_status(401);
    let error = provider
        .client()
        .expect("client")
        .exchange_authorization_code("fake-code", "fake-state")
        .await
        .expect_err("401 must fail");
    assert_eq!(error.stage, NaverProviderStage::TokenExchange);
    assert_eq!(
        error.failure,
        NaverProviderFailure::Rejected { status: 401 }
    );
    // The rendered error carries the stage and the classification only.
    let rendered = format!("{error}");
    assert!(!rendered.contains("fake-code"), "{rendered}");
    assert!(!rendered.contains("fake-state"), "{rendered}");
    assert!(!rendered.contains("fake-naver-token"), "{rendered}");
}

#[tokio::test]
async fn a_profile_500_is_a_typed_rejection() {
    let provider = FakeProvider::start(FakeProfile::woman()).await.expect("fake provider");
    let client = provider.client().expect("client");
    let token = client
        .exchange_authorization_code("fake-code", "fake-state")
        .await
        .expect("exchange succeeds");
    provider.set_profile_status(500);
    let error = client
        .load_profile(&token)
        .await
        .expect_err("500 must fail");
    assert_eq!(error.stage, NaverProviderStage::ProfileFetch);
    assert_eq!(
        error.failure,
        NaverProviderFailure::Rejected { status: 500 }
    );
    assert_eq!(error.classification(), "rejected");
}

#[tokio::test]
async fn a_timeout_is_a_transport_failure() {
    // A listener that accepts and then never answers. The client's own timeout
    // must fire; the test's own timeout is the backstop that proves it did.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("addr");
    let held = tokio::spawn(async move {
        let (mut socket, _) = listener.accept().await.expect("accept");
        let mut buffer = [0u8; 1024];
        let _ = socket.read(&mut buffer).await;
        // Hold the connection open without answering.
        tokio::time::sleep(Duration::from_secs(30)).await;
        let _ = socket.write_all(b"HTTP/1.1 200 OK\r\n\r\n").await;
    });
    let client = NaverProviderClient::new(NaverProviderEndpoints {
        token_url: format!("http://{address}/token"),
        profile_url: format!("http://{address}/profile"),
        client_id: "test-client".into(),
        client_secret: "test-secret".into(),
        redirect_uri: "http://localhost:8080/api/identity/naver/callback".into(),
    })
    .expect("client");
    let error = tokio::time::timeout(
        Duration::from_secs(20),
        client.exchange_authorization_code("fake-code", "fake-state"),
    )
    .await
    .expect("the client's own timeout must fire before the test's")
    .expect_err("a hung provider must fail");
    assert_eq!(error.stage, NaverProviderStage::TokenExchange);
    assert_eq!(error.failure, NaverProviderFailure::Transport);
    held.abort();
}

#[tokio::test]
async fn an_oversized_profile_body_is_refused() {
    let provider = FakeProvider::start(FakeProfile::woman()).await.expect("fake provider");
    let client = provider.client().expect("client");
    let token = client
        .exchange_authorization_code("fake-code", "fake-state")
        .await
        .expect("exchange succeeds");
    provider.set_oversized_profile(true);
    let error = client
        .load_profile(&token)
        .await
        .expect_err("an oversized body must fail");
    assert_eq!(error.stage, NaverProviderStage::ProfileFetch);
    assert_eq!(error.failure, NaverProviderFailure::InvalidResponse);
}

#[tokio::test]
async fn a_token_response_without_an_access_token_is_refused() {
    let provider = FakeProvider::start(FakeProfile::woman()).await.expect("fake provider");
    provider.set_token_body(Some(&json!({"resultcode": "024"}).to_string()));
    let error = provider
        .client()
        .expect("client")
        .exchange_authorization_code("fake-code", "fake-state")
        .await
        .expect_err("no token must fail");
    assert_eq!(error.failure, NaverProviderFailure::MissingAccessToken);
}

/// The access token never appears in `Debug` output.
///
/// V-12-o. The token type has no derived `Debug`; this asserts the hand-written
/// one redacts the value, and that the provider profile does the same for the
/// subject and the birth date.
#[test]
fn the_access_token_and_profile_render_no_value_in_debug() {
    let token = NaverAccessToken::new("fake-naver-token".into());
    let rendered = format!("{token:?}");
    assert!(!rendered.contains("fake-naver-token"), "{rendered}");
    assert_eq!(rendered, "NaverAccessToken([REDACTED])");

    let profile = profile_service::identity::NaverProviderProfile {
        subject: Some("naver-subject-woman".into()),
        gender: Some(NaverGender::Female),
        birthyear: Some("2000".into()),
        birthday: Some("07-12".into()),
    };
    let rendered = format!("{profile:?}");
    for secret in ["naver-subject-woman", "2000", "07-12"] {
        assert!(!rendered.contains(secret), "{secret} leaked: {rendered}");
    }
}

/// The client never follows a redirect.
///
/// A provider redirect would move the bearer token to another origin. The
/// configured endpoint is the only destination, so a `3xx` is a rejection.
#[tokio::test]
async fn a_provider_redirect_is_not_followed() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let address = listener.local_addr().expect("addr");
    let hits = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counter = hits.clone();
    let server = tokio::spawn(async move {
        loop {
            let Ok((mut socket, _)) = listener.accept().await else {
                break;
            };
            let counter = counter.clone();
            tokio::spawn(async move {
                let mut buffer = [0u8; 4096];
                let _ = socket.read(&mut buffer).await;
                counter.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                let _ = socket
                    .write_all(
                        b"HTTP/1.1 302 Found\r\nlocation: http://127.0.0.1:1/elsewhere\r\ncontent-length: 0\r\nconnection: close\r\n\r\n",
                    )
                    .await;
            });
        }
    });
    let client = NaverProviderClient::new(NaverProviderEndpoints {
        token_url: format!("http://{address}/token"),
        profile_url: format!("http://{address}/profile"),
        client_id: "test-client".into(),
        client_secret: "test-secret".into(),
        redirect_uri: "http://localhost:8080/api/identity/naver/callback".into(),
    })
    .expect("client");
    let error = client
        .exchange_authorization_code("fake-code", "fake-state")
        .await
        .expect_err("a redirect is not a success");
    assert_eq!(
        error.failure,
        NaverProviderFailure::Rejected { status: 302 }
    );
    // Exactly one request reached the provider: the redirect target was never
    // contacted.
    assert_eq!(hits.load(std::sync::atomic::Ordering::SeqCst), 1);
    server.abort();
}

/// A provider error renders nothing but its classification.
#[test]
fn a_provider_error_renders_only_its_classification() {
    let error = NaverProviderError::new(
        NaverProviderStage::TokenExchange,
        NaverProviderFailure::Rejected { status: 400 },
    );
    assert_eq!(error.to_string(), "NAVER token_exchange failed (rejected)");
    assert_eq!(error.classification(), "rejected");
}
