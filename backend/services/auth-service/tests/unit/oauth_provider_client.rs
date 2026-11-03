use crate::support::{shared_test_rsa_private_key, shared_test_rsa_private_key_pem};
use auth_service::adapters::out::oauth::provider_client::{
    HttpOAuthProviderClient, OAuthHttpProviderSettings, OAuthTokenAuthMethod, OAuthUserInfoFormat,
};
use auth_service::application::ports::OAuthProviderPort;
use auth_service::config::{build_oauth_provider, AppConfig, PasswordResetMailerMode, RuntimeMode};
use auth_service::domain::user::IdentityProvider;
use axum::body::Body;
use axum::extract::Form;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use base64ct::{Base64, Base64UrlUnpadded, Encoding};
use chrono::{Duration, Utc};
use jsonwebtoken::{Algorithm, EncodingKey, Header};
use p256::pkcs8::EncodePrivateKey;
use rsa::traits::PublicKeyParts;
use rsa::RsaPublicKey;
use serde::Serialize;
use serde_json::json;
use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio::net::TcpListener;

const EXPECTED_OAUTH_PROVIDER_RESPONSE_LIMIT_BYTES: usize = 64 * 1024;
const EXPECTED_OAUTH_PROVIDER_ERROR_LIMIT_BYTES: usize = 8 * 1024;

#[test]
fn production_oauth_provider_requires_real_provider_configuration() {
    let config = AppConfig {
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        runtime_mode: RuntimeMode::Production,
        http_addr: "127.0.0.1:18081".parse().expect("addr"),
        metrics_addr: "127.0.0.1:0".parse().expect("metrics addr"),
        grpc_addr: "127.0.0.1:15051".parse().expect("addr"),
        database_pool_max_connections: 5,
        database_url: "postgres://postgres:postgres@localhost:5432/auth_db".into(),
        seed_enabled: false,
        public_application_routes_enabled: false,
        access_ttl_seconds: 900,
        refresh_ttl_seconds: 1209600,
        issuer: "auth-service-test".into(),
        access_audience: "port-rust-api-test".into(),
        google_redirect_uri: "http://127.0.0.1:8080/auth/oauth/google/callback".into(),
        apple_redirect_uri: "http://127.0.0.1:8080/auth/oauth/apple/callback".into(),
        x_redirect_uri: "http://127.0.0.1:8080/auth/oauth/x/callback".into(),
        google_authorize_base_uri: "https://accounts.google.test/o/oauth2/v2/auth".into(),
        apple_authorize_base_uri: "https://appleid.apple.test/auth/authorize".into(),
        x_authorize_base_uri: "https://x.test/i/oauth2/authorize".into(),
        google_client_id: None,
        google_client_secret: None,
        google_token_uri: None,
        google_userinfo_uri: None,
        google_issuer: "https://accounts.google.test".into(),
        google_jwks_json: None,
        google_jwks_uri: None,
        apple_client_id: None,
        apple_client_secret: None,
        apple_team_id: None,
        apple_key_id: None,
        apple_private_key_pem: None,
        apple_client_secret_ttl_seconds: 86_400,
        apple_token_uri: None,
        apple_userinfo_uri: None,
        apple_issuer: "https://appleid.apple.test".into(),
        apple_jwks_json: None,
        apple_jwks_uri: None,
        x_client_id: None,
        x_client_secret: None,
        x_token_uri: None,
        x_userinfo_uri: None,
        jwt_private_key_pem: None,
        jwt_key_id: None,
        jwt_rotation_jwks_json: None,
        actor_delegation: platform::actor_delegation::ActorDelegationVerifier::local_parity(
            platform::grpc_auth::Workload::Auth,
        ),
        caller_registry: platform::grpc_auth::CallerRegistry::new()
            .register(
                platform::grpc_auth::Workload::Gateway,
                "test-gateway-credential",
            )
            .expect("gateway caller"),
        outbound_identity: "test-internal-service-token".into(),
        tls_material: None,
        login_lockout_max_failures: 5,
        login_lockout_window_seconds: 900,
        password_work: auth_service::application::password_work::PasswordWorkLimits::default(),
        password_reset_base_url: "https://app.example.com/auth/reset".into(),
        password_reset_mailer_mode: PasswordResetMailerMode::Http,
        password_reset_mailer_endpoint: Some("https://mailer.example.com/password-reset".into()),
        password_reset_mailer_bearer_token: Some("mailer-token".into()),
        lifecycle_cleanup_worker_enabled: true,
        lifecycle_cleanup_worker_interval_ms: 1000,
        profile_grpc_endpoint: "http://127.0.0.1:50052".into(),
        friend_grpc_endpoint: "http://127.0.0.1:50053".into(),
        notification_grpc_endpoint: "http://127.0.0.1:50054".into(),
        post_grpc_endpoint: "http://127.0.0.1:50055".into(),
        user_deletion_worker_enabled: true,
        user_deletion_worker_interval_ms: 1_000,
        user_deletion_worker_batch_size: 10,
        user_deletion_worker_max_attempts: 3,
        user_deletion_worker_lease_timeout_ms: 300_000,
        user_deletion_worker_base_retry_ms: 1_000,
    };

    let error = match build_oauth_provider(&config) {
        Ok(_) => panic!("production OAuth config should fail"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("AUTH_GOOGLE_CLIENT_ID"));
}

#[tokio::test]
async fn http_oauth_provider_exchanges_code_and_reads_userinfo() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let id_token_key = TestIdTokenKey::new("oauth-test-key");
    let server_state = Arc::new(TestOAuthServerState {
        id_token: id_token_key.id_token(
            "https://accounts.google.test",
            "google-client-id",
            "google-real-subject",
            "nonce-123",
        ),
        jwks_json: id_token_key.jwks_json(),
    });
    let app = Router::new()
        .route("/token", post(token_handler))
        .route("/userinfo", get(userinfo_handler))
        .with_state(server_state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server");
    });

    let base = format!("http://{addr}");
    let provider = HttpOAuthProviderClient::new(
        OAuthHttpProviderSettings {
            authorize_base_uri: format!("{base}/authorize"),
            token_uri: format!("{base}/token"),
            userinfo_uri: format!("{base}/userinfo"),
            issuer: "https://accounts.google.test".into(),
            jwks_json: id_token_key.jwks_json(),
            jwks_uri: None,
            client_id: "google-client-id".into(),
            client_secret: ["google", "client", "placeholder"].join("-"),
            client_secret_generator: None,
            scopes: vec!["openid".into(), "email".into(), "profile".into()],
            token_auth_method: OAuthTokenAuthMethod::ClientSecretPost,
            userinfo_format: OAuthUserInfoFormat::Oidc,
        },
        OAuthHttpProviderSettings {
            authorize_base_uri: format!("{base}/apple-authorize"),
            token_uri: format!("{base}/apple-token"),
            userinfo_uri: format!("{base}/apple-userinfo"),
            issuer: "https://appleid.apple.test".into(),
            jwks_json: id_token_key.jwks_json(),
            jwks_uri: None,
            client_id: "apple-client-id".into(),
            client_secret: ["apple", "client", "placeholder"].join("-"),
            client_secret_generator: None,
            scopes: vec!["name".into(), "email".into()],
            token_auth_method: OAuthTokenAuthMethod::ClientSecretPost,
            userinfo_format: OAuthUserInfoFormat::Oidc,
        },
        OAuthHttpProviderSettings {
            authorize_base_uri: format!("{base}/x-authorize"),
            token_uri: format!("{base}/x-token"),
            userinfo_uri: format!("{base}/x-userinfo"),
            issuer: String::new(),
            jwks_json: String::new(),
            jwks_uri: None,
            client_id: "x-client-id".into(),
            client_secret: ["x", "client", "placeholder"].join("-"),
            client_secret_generator: None,
            scopes: vec!["users.read".into(), "users.email".into()],
            token_auth_method: OAuthTokenAuthMethod::BasicAuth,
            userinfo_format: OAuthUserInfoFormat::XApiV2,
        },
    )
    .expect("provider");

    let authorize_uri = provider
        .build_authorization_uri(
            (IdentityProvider::Google).into(),
            "https://app.example.test/auth/oauth/google/callback",
            "state-123",
            "challenge-123",
            "nonce-123",
        )
        .expect("authorize uri");
    assert!(authorize_uri.contains("client_id=google-client-id"));
    assert!(authorize_uri.contains("scope=openid+email+profile"));
    assert!(authorize_uri.contains("state=state-123"));
    assert!(authorize_uri.contains("code_challenge=challenge-123"));
    assert!(authorize_uri.contains("code_challenge_method=S256"));
    assert!(authorize_uri.contains("nonce=nonce-123"));

    let user = provider
        .fetch_user_info(
            (IdentityProvider::Google).into(),
            "real-provider-code",
            "https://app.example.test/auth/oauth/google/callback",
            Some("verifier-123"),
            "nonce-123",
        )
        .await
        .expect("userinfo");

    assert_eq!(user.provider, IdentityProvider::Google.into());
    assert_eq!(user.subject, "google-real-subject");
    assert_eq!(user.email, "real-user@example.com");
    assert!(user.email_verified);
}

#[tokio::test]
async fn http_oauth_provider_abandons_a_stalled_token_endpoint_at_the_caller_deadline() {
    // The provider accepts the connection and never answers. Without a client
    // ceiling the request would wait for as long as the task lives.
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            held.push(socket);
        }
    });
    let key = TestIdTokenKey::new("oauth-test-key");
    let base = format!("http://{addr}");
    let provider = test_provider(
        &format!("{base}/token"),
        &format!("{base}/userinfo"),
        "https://accounts.google.test",
        key.jwks_json(),
    );
    let ceiling = auth_service::adapters::out::oauth::provider_client::provider_request_ceiling();
    assert_eq!(
        ceiling,
        platform::edge::Edge::GatewayAuth.budget().deadline,
        "the ceiling is the Gateway to Auth deadline, not a separate tuned value"
    );
    let started = std::time::Instant::now();
    let error = provider
        .fetch_user_info(
            (IdentityProvider::Google).into(),
            "real-provider-code",
            "https://app.example.test/auth/oauth/google/callback",
            Some("verifier-123"),
            "nonce-123",
        )
        .await
        .expect_err("a stalled provider must fail");
    let elapsed = started.elapsed();
    assert!(
        elapsed >= ceiling,
        "returned before the ceiling: {elapsed:?}"
    );
    assert!(
        elapsed < ceiling + std::time::Duration::from_secs(2),
        "the stalled request outlived its ceiling: {elapsed:?}"
    );
    assert!(matches!(
        error,
        auth_service::application::errors::AuthError::ProviderFailure(_)
    ));
}

#[tokio::test]
async fn http_oauth_provider_stops_at_the_callers_remaining_time() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    tokio::spawn(async move {
        let mut held = Vec::new();
        while let Ok((socket, _)) = listener.accept().await {
            held.push(socket);
        }
    });
    let key = TestIdTokenKey::new("oauth-test-key");
    let base = format!("http://{addr}");
    let provider = test_provider(
        &format!("{base}/token"),
        &format!("{base}/userinfo"),
        "https://accounts.google.test",
        key.jwks_json(),
    );
    // The Gateway sent 200 ms of remaining time, well below the ceiling.
    let caller_budget = std::time::Duration::from_millis(200);
    let started = std::time::Instant::now();
    let result = platform::call_deadline::scope(started + caller_budget, async {
        assert!(
            auth_service::adapters::out::oauth::provider_client::provider_request_budget()
                <= caller_budget
        );
        provider
            .fetch_user_info(
                (IdentityProvider::Google).into(),
                "real-provider-code",
                "https://app.example.test/auth/oauth/google/callback",
                Some("verifier-123"),
                "nonce-123",
            )
            .await
    })
    .await;
    let elapsed = started.elapsed();
    assert!(result.is_err(), "a stalled provider must fail");
    assert!(
        elapsed < std::time::Duration::from_millis(600),
        "the provider request outlived the caller's remaining time: {elapsed:?}"
    );
}

#[tokio::test]
async fn http_oauth_provider_rejects_oversized_token_response_before_json_decode() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let app = Router::new().route("/token", post(oversized_token_response));
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server");
    });

    let key = TestIdTokenKey::new("oauth-test-key");
    let base = format!("http://{addr}");
    let provider = test_provider(
        &format!("{base}/token"),
        &format!("{base}/userinfo"),
        "https://accounts.google.test",
        key.jwks_json(),
    );
    let error = provider
        .fetch_user_info(
            (IdentityProvider::Google).into(),
            "real-provider-code",
            "https://app.example.test/auth/oauth/google/callback",
            Some("verifier-123"),
            "nonce-123",
        )
        .await
        .expect_err("an oversized token body must be rejected");
    let auth_service::application::errors::AuthError::ProviderFailure(failure) = &error else {
        panic!("stable provider error: {error:?}");
    };
    assert_eq!(failure.code(), "PROVIDER_TOKEN_RESPONSE_INVALID");
    assert_eq!(error.to_string(), "OAuth token response was invalid");
}

#[tokio::test]
async fn http_oauth_provider_accepts_token_response_at_the_configured_limit() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let key = TestIdTokenKey::new("oauth-test-key");
    let server_state = Arc::new(TestOAuthServerState {
        id_token: key.id_token(
            "https://accounts.google.test",
            "google-client-id",
            "google-real-subject",
            "nonce-123",
        ),
        jwks_json: key.jwks_json(),
    });
    let app = Router::new()
        .route("/token", post(at_limit_token_response))
        .route("/userinfo", get(userinfo_handler))
        .with_state(server_state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server");
    });

    let base = format!("http://{addr}");
    let provider = test_provider(
        &format!("{base}/token"),
        &format!("{base}/userinfo"),
        "https://accounts.google.test",
        key.jwks_json(),
    );
    let user = provider
        .fetch_user_info(
            (IdentityProvider::Google).into(),
            "real-provider-code",
            "https://app.example.test/auth/oauth/google/callback",
            Some("verifier-123"),
            "nonce-123",
        )
        .await
        .expect("the exact response-size limit is accepted");
    assert_eq!(user.subject, "google-real-subject");
}

#[tokio::test]
async fn http_oauth_provider_bounds_chunked_userinfo_response_without_content_length() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let key = TestIdTokenKey::new("oauth-test-key");
    let server_state = Arc::new(TestOAuthServerState {
        id_token: key.id_token(
            "https://accounts.google.test",
            "google-client-id",
            "google-real-subject",
            "nonce-123",
        ),
        jwks_json: key.jwks_json(),
    });
    let app = Router::new()
        .route("/token", post(token_handler))
        .route("/userinfo", get(oversized_chunked_response))
        .with_state(server_state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server");
    });

    let base = format!("http://{addr}");
    let userinfo_uri = format!("{base}/userinfo");
    let response = reqwest::Client::new()
        .get(&userinfo_uri)
        .send()
        .await
        .expect("chunked userinfo fixture");
    assert!(
        response.content_length().is_none(),
        "stream fixture must not declare a response length"
    );
    let _ = response.bytes().await.expect("fixture body");
    let provider = test_provider(
        &format!("{base}/token"),
        &userinfo_uri,
        "https://accounts.google.test",
        key.jwks_json(),
    );
    let error = provider
        .fetch_user_info(
            (IdentityProvider::Google).into(),
            "real-provider-code",
            "https://app.example.test/auth/oauth/google/callback",
            Some("verifier-123"),
            "nonce-123",
        )
        .await
        .expect_err("an oversized chunked userinfo body must be rejected");
    let auth_service::application::errors::AuthError::ProviderFailure(failure) = &error else {
        panic!("stable provider error: {error:?}");
    };
    assert_eq!(failure.code(), "PROVIDER_USERINFO_RESPONSE_INVALID");
    assert_eq!(error.to_string(), "OAuth userinfo response was invalid");
}

#[tokio::test]
async fn http_oauth_provider_rejects_oversized_chunked_jwks_refresh() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let stale_key = TestIdTokenKey::new("stale-oauth-key");
    let rotated_key = TestIdTokenKey::new("rotated-oauth-key");
    let server_state = Arc::new(TestOAuthServerState {
        id_token: rotated_key.id_token(
            "https://accounts.google.test",
            "google-client-id",
            "google-real-subject",
            "nonce-123",
        ),
        jwks_json: rotated_key.jwks_json(),
    });
    let jwks_requests = Arc::new(AtomicUsize::new(0));
    let jwks_requests_for_handler = Arc::clone(&jwks_requests);
    let app = Router::new()
        .route("/token", post(token_handler))
        .route("/userinfo", get(userinfo_handler))
        .route(
            "/jwks",
            get(move || {
                let requests = Arc::clone(&jwks_requests_for_handler);
                async move {
                    requests.fetch_add(1, Ordering::SeqCst);
                    oversized_chunked_response().await
                }
            }),
        )
        .with_state(server_state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server");
    });

    let base = format!("http://{addr}");
    let jwks_uri = format!("{base}/jwks");
    let provider = test_provider_with_jwks_uri(
        &format!("{base}/token"),
        &format!("{base}/userinfo"),
        Some(&jwks_uri),
        "https://accounts.google.test",
        stale_key.jwks_json(),
    );
    let error = provider
        .fetch_user_info(
            (IdentityProvider::Google).into(),
            "real-provider-code",
            "https://app.example.test/auth/oauth/google/callback",
            Some("verifier-123"),
            "nonce-123",
        )
        .await
        .expect_err("an oversized refreshed JWKS body must be rejected");
    let auth_service::application::errors::AuthError::ProviderFailure(failure) = &error else {
        panic!("stable provider error: {error:?}");
    };
    assert_eq!(failure.code(), "PROVIDER_ID_TOKEN_INVALID");
    assert_eq!(
        error.to_string(),
        "OAuth provider ID token validation failed"
    );
    assert_eq!(jwks_requests.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn http_oauth_provider_bounds_error_body_before_classification() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let key = TestIdTokenKey::new("oauth-test-key");
    let server_state = Arc::new(TestOAuthServerState {
        id_token: key.id_token(
            "https://accounts.google.test",
            "google-client-id",
            "google-real-subject",
            "nonce-123",
        ),
        jwks_json: key.jwks_json(),
    });
    let app = Router::new()
        .route("/token", post(token_handler))
        .route("/userinfo", get(oversized_userinfo_error_response))
        .with_state(server_state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server");
    });

    let base = format!("http://{addr}");
    let provider = test_provider(
        &format!("{base}/token"),
        &format!("{base}/userinfo"),
        "https://accounts.google.test",
        key.jwks_json(),
    );
    let error = provider
        .fetch_user_info(
            (IdentityProvider::Google).into(),
            "real-provider-code",
            "https://app.example.test/auth/oauth/google/callback",
            Some("verifier-123"),
            "nonce-123",
        )
        .await
        .expect_err("an oversized provider error document is not classified");
    let auth_service::application::errors::AuthError::ProviderFailure(failure) = &error else {
        panic!("stable provider error: {error:?}");
    };
    assert_eq!(failure.code(), "PROVIDER_USERINFO_HTTP_FAILURE");
    assert_eq!(error.to_string(), "OAuth provider userinfo request failed");
}

/// Build a client whose Apple settings point at a server that serves the given
/// token response, with no userinfo route at all.
//
// Leaving out the userinfo route is the point: Apple has no userinfo endpoint to
// call, so a flow that depends on one must fail here rather than appear to work.
async fn apple_provider(
    id_token: String,
    jwks_json: String,
) -> (HttpOAuthProviderClient, std::net::SocketAddr) {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let server_state = Arc::new(TestOAuthServerState {
        id_token,
        jwks_json: jwks_json.clone(),
    });
    let app = Router::new()
        .route("/token", post(apple_token_handler))
        .with_state(server_state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server");
    });
    let base = format!("http://{addr}");
    let provider = HttpOAuthProviderClient::new(
        OAuthHttpProviderSettings {
            authorize_base_uri: format!("{base}/authorize"),
            token_uri: format!("{base}/token"),
            userinfo_uri: format!("{base}/userinfo"),
            issuer: "https://accounts.google.test".into(),
            jwks_json: jwks_json.clone(),
            jwks_uri: None,
            client_id: "google-client-id".into(),
            client_secret: ["google", "client", "placeholder"].join("-"),
            client_secret_generator: None,
            scopes: vec!["openid".into()],
            token_auth_method: OAuthTokenAuthMethod::ClientSecretPost,
            userinfo_format: OAuthUserInfoFormat::Oidc,
        },
        OAuthHttpProviderSettings {
            authorize_base_uri: format!("{base}/authorize"),
            token_uri: format!("{base}/token"),
            userinfo_uri: format!("{base}/userinfo"),
            issuer: "https://appleid.apple.com".into(),
            jwks_json,
            jwks_uri: None,
            client_id: "apple-client-id".into(),
            client_secret: ["apple", "client", "placeholder"].join("-"),
            client_secret_generator: None,
            scopes: vec!["email".into()],
            token_auth_method: OAuthTokenAuthMethod::ClientSecretPost,
            userinfo_format: OAuthUserInfoFormat::Oidc,
        },
        OAuthHttpProviderSettings {
            authorize_base_uri: format!("{base}/authorize"),
            token_uri: format!("{base}/token"),
            userinfo_uri: format!("{base}/userinfo"),
            issuer: String::new(),
            jwks_json: String::new(),
            jwks_uri: None,
            client_id: "x-client-id".into(),
            client_secret: ["x", "client", "placeholder"].join("-"),
            client_secret_generator: None,
            scopes: vec!["users.read".into()],
            token_auth_method: OAuthTokenAuthMethod::BasicAuth,
            userinfo_format: OAuthUserInfoFormat::XApiV2,
        },
    )
    .expect("provider");
    (provider, addr)
}

#[tokio::test]
async fn apple_sign_in_succeeds_without_a_userinfo_endpoint() {
    // The regression this guards: the shared flow always called userinfo, so an
    // Apple sign-in failed against a provider that has no such endpoint. Apple
    // signals ES256 in the JWKS, which the verifier also had to accept.
    let key = TestEcIdTokenKey::new("apple-es256-key");
    let id_token = key.id_token(
        "apple-client-id",
        "apple-subject-1",
        "nonce-apple",
        Some("person@privaterelay.appleid.com"),
    );
    let (provider, _addr) = apple_provider(id_token, key.jwks_json()).await;

    let user = provider
        .fetch_user_info(
            (IdentityProvider::Apple).into(),
            "apple-code",
            "https://app.example.test/auth/oauth/apple/callback",
            None,
            "nonce-apple",
        )
        .await
        .expect("Apple must succeed from the identity token alone");

    assert_eq!(user.provider, IdentityProvider::Apple.into());
    assert_eq!(user.subject, "apple-subject-1");
    assert_eq!(user.email, "person@privaterelay.appleid.com");
    // Apple sends the string "true"; a boolean-only check would clear this.
    assert!(user.email_verified);
}

#[tokio::test]
async fn apple_sign_in_tolerates_a_token_without_an_email_claim() {
    // Apple returns the address only on the first authorization, so later
    // sign-ins carry no email. That must not fail an existing account.
    let key = TestEcIdTokenKey::new("apple-es256-key");
    let id_token = key.id_token("apple-client-id", "apple-subject-2", "nonce-apple", None);
    let (provider, _addr) = apple_provider(id_token, key.jwks_json()).await;

    let user = provider
        .fetch_user_info(
            (IdentityProvider::Apple).into(),
            "apple-code",
            "https://app.example.test/auth/oauth/apple/callback",
            None,
            "nonce-apple",
        )
        .await
        .expect("an absent email must not fail the exchange");

    assert_eq!(user.subject, "apple-subject-2");
    assert!(user.email.is_empty());
    assert!(
        !user.email_verified,
        "an absent email cannot be reported as verified"
    );
}

#[tokio::test]
async fn apple_id_token_with_the_wrong_nonce_is_rejected() {
    // The nonce binds the token to this authorization request, so accepting a
    // mismatched one would let a token from another session be replayed.
    let key = TestEcIdTokenKey::new("apple-es256-key");
    let id_token = key.id_token("apple-client-id", "apple-subject-3", "nonce-a", None);
    let (provider, _addr) = apple_provider(id_token, key.jwks_json()).await;

    let error = provider
        .fetch_user_info(
            (IdentityProvider::Apple).into(),
            "apple-code",
            "https://app.example.test/auth/oauth/apple/callback",
            None,
            "nonce-b",
        )
        .await
        .expect_err("a mismatched nonce must be rejected");
    assert!(
        error.to_string().contains("AUTH_") || !error.to_string().is_empty(),
        "the rejection must be reported: {error}"
    );
}

#[tokio::test]
async fn an_rsa_token_is_rejected_when_the_key_is_published_as_ec() {
    // Algorithm confusion: a token must not be verifiable under an algorithm its
    // published key was not issued for.
    let rsa_key = TestIdTokenKey::new("shared-kid");
    let ec_key = TestEcIdTokenKey::new("shared-kid");
    let rsa_token = rsa_key.id_token(
        "https://appleid.apple.com",
        "apple-client-id",
        "apple-subject-4",
        "nonce-apple",
    );
    let (provider, _addr) = apple_provider(rsa_token, ec_key.jwks_json()).await;

    let result = provider
        .fetch_user_info(
            (IdentityProvider::Apple).into(),
            "apple-code",
            "https://app.example.test/auth/oauth/apple/callback",
            None,
            "nonce-apple",
        )
        .await;
    assert!(
        result.is_err(),
        "an RS256 token must not verify against an ES256 key"
    );
}

#[tokio::test]
async fn http_oauth_provider_exchanges_x_code_with_basic_auth_and_reads_v2_userinfo() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let id_token_key = TestIdTokenKey::new("oauth-test-key");
    let server_state = Arc::new(TestOAuthServerState {
        id_token: id_token_key.id_token(
            "https://accounts.google.test",
            "google-client-id",
            "google-real-subject",
            "nonce-123",
        ),
        jwks_json: id_token_key.jwks_json(),
    });
    let app = Router::new()
        .route("/token", post(token_handler))
        .route("/userinfo", get(userinfo_handler))
        .route("/x-token", post(x_token_handler))
        .route("/x-userinfo", get(x_userinfo_handler))
        .with_state(server_state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server");
    });

    let base = format!("http://{addr}");
    let provider = HttpOAuthProviderClient::new(
        OAuthHttpProviderSettings {
            authorize_base_uri: format!("{base}/authorize"),
            token_uri: format!("{base}/token"),
            userinfo_uri: format!("{base}/userinfo"),
            issuer: "https://accounts.google.test".into(),
            jwks_json: id_token_key.jwks_json(),
            jwks_uri: None,
            client_id: "google-client-id".into(),
            client_secret: ["google", "client", "placeholder"].join("-"),
            client_secret_generator: None,
            scopes: vec!["openid".into(), "email".into(), "profile".into()],
            token_auth_method: OAuthTokenAuthMethod::ClientSecretPost,
            userinfo_format: OAuthUserInfoFormat::Oidc,
        },
        OAuthHttpProviderSettings {
            authorize_base_uri: format!("{base}/apple-authorize"),
            token_uri: format!("{base}/apple-token"),
            userinfo_uri: format!("{base}/apple-userinfo"),
            issuer: "https://appleid.apple.test".into(),
            jwks_json: id_token_key.jwks_json(),
            jwks_uri: None,
            client_id: "apple-client-id".into(),
            client_secret: ["apple", "client", "placeholder"].join("-"),
            client_secret_generator: None,
            scopes: vec!["name".into(), "email".into()],
            token_auth_method: OAuthTokenAuthMethod::ClientSecretPost,
            userinfo_format: OAuthUserInfoFormat::Oidc,
        },
        OAuthHttpProviderSettings {
            authorize_base_uri: format!("{base}/x-authorize"),
            token_uri: format!("{base}/x-token"),
            userinfo_uri: format!("{base}/x-userinfo"),
            issuer: String::new(),
            jwks_json: String::new(),
            jwks_uri: None,
            client_id: "x-client-id".into(),
            client_secret: "x-client-secret".into(),
            client_secret_generator: None,
            scopes: vec!["users.read".into(), "users.email".into()],
            token_auth_method: OAuthTokenAuthMethod::BasicAuth,
            userinfo_format: OAuthUserInfoFormat::XApiV2,
        },
    )
    .expect("provider");

    let authorize_uri = provider
        .build_authorization_uri(
            (IdentityProvider::X).into(),
            "https://app.example.test/auth/oauth/x/callback",
            "state-123",
            "challenge-123",
            "nonce-should-not-be-sent",
        )
        .expect("authorize uri");
    assert!(authorize_uri.contains("client_id=x-client-id"));
    assert!(authorize_uri.contains("scope=users.read+users.email"));
    assert!(authorize_uri.contains("state=state-123"));
    assert!(authorize_uri.contains("code_challenge=challenge-123"));
    assert!(!authorize_uri.contains("nonce="));

    let user = provider
        .fetch_user_info(
            (IdentityProvider::X).into(),
            "x-provider-code",
            "https://app.example.test/auth/oauth/x/callback",
            Some("x-verifier-123"),
            "unused-nonce",
        )
        .await
        .expect("x userinfo");

    assert_eq!(user.provider, IdentityProvider::X.into());
    assert_eq!(user.subject, "2244994945");
    assert_eq!(user.email, "x-user@example.com");
    assert!(user.email_verified);
}

#[tokio::test]
async fn http_oauth_provider_rejects_id_token_with_wrong_audience() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let id_token_key = TestIdTokenKey::new("oauth-test-key");
    let server_state = Arc::new(TestOAuthServerState {
        id_token: id_token_key.id_token(
            "https://accounts.google.test",
            "different-client",
            "google-real-subject",
            "nonce-123",
        ),
        jwks_json: id_token_key.jwks_json(),
    });
    let app = Router::new()
        .route("/token", post(token_handler))
        .route("/userinfo", get(userinfo_handler))
        .with_state(server_state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server");
    });

    let base = format!("http://{addr}");
    let provider = test_provider(
        &format!("{base}/token"),
        &format!("{base}/userinfo"),
        "https://accounts.google.test",
        id_token_key.jwks_json(),
    );
    let error = provider
        .fetch_user_info(
            (IdentityProvider::Google).into(),
            "real-provider-code",
            "https://app.example.test/auth/oauth/google/callback",
            Some("verifier-123"),
            "nonce-123",
        )
        .await
        .expect_err("wrong ID token audience should fail");
    let error = error.to_string();
    assert_eq!(error, "OAuth provider ID token validation failed");
    assert!(!error.contains("different-client"));
    assert!(!error.contains("google-real-subject"));
}

#[tokio::test]
async fn http_oauth_provider_rejects_id_token_with_wrong_nonce() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let id_token_key = TestIdTokenKey::new("oauth-test-key");
    let server_state = Arc::new(TestOAuthServerState {
        id_token: id_token_key.id_token(
            "https://accounts.google.test",
            "google-client-id",
            "google-real-subject",
            "provider-nonce",
        ),
        jwks_json: id_token_key.jwks_json(),
    });
    let app = Router::new()
        .route("/token", post(token_handler))
        .route("/userinfo", get(userinfo_handler))
        .with_state(server_state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server");
    });

    let base = format!("http://{addr}");
    let provider = test_provider(
        &format!("{base}/token"),
        &format!("{base}/userinfo"),
        "https://accounts.google.test",
        id_token_key.jwks_json(),
    );
    let error = provider
        .fetch_user_info(
            (IdentityProvider::Google).into(),
            "real-provider-code",
            "https://app.example.test/auth/oauth/google/callback",
            Some("verifier-123"),
            "stored-authorization-nonce",
        )
        .await
        .expect_err("wrong ID token nonce should fail");
    let error = error.to_string();
    assert_eq!(error, "OAuth provider ID token validation failed");
    assert!(!error.contains("provider-nonce"));
    assert!(!error.contains("stored-authorization-nonce"));
}

#[tokio::test]
async fn http_oauth_provider_refreshes_configured_jwks_uri_when_id_token_kid_rotates() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let stale_key = TestIdTokenKey::new("stale-oauth-key");
    let rotated_key = TestIdTokenKey::new("rotated-oauth-key");
    let server_state = Arc::new(TestOAuthServerState {
        id_token: rotated_key.id_token(
            "https://accounts.google.test",
            "google-client-id",
            "google-real-subject",
            "nonce-123",
        ),
        jwks_json: rotated_key.jwks_json(),
    });
    let app = Router::new()
        .route("/token", post(token_handler))
        .route("/userinfo", get(userinfo_handler))
        .route("/jwks", get(jwks_handler))
        .with_state(server_state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server");
    });

    let base = format!("http://{addr}");
    let provider = HttpOAuthProviderClient::new(
        OAuthHttpProviderSettings {
            authorize_base_uri: format!("{base}/authorize"),
            token_uri: format!("{base}/token"),
            userinfo_uri: format!("{base}/userinfo"),
            issuer: "https://accounts.google.test".into(),
            jwks_json: stale_key.jwks_json(),
            jwks_uri: Some(format!("{base}/jwks")),
            client_id: "google-client-id".into(),
            client_secret: ["google", "client", "placeholder"].join("-"),
            client_secret_generator: None,
            scopes: vec!["openid".into(), "email".into(), "profile".into()],
            token_auth_method: OAuthTokenAuthMethod::ClientSecretPost,
            userinfo_format: OAuthUserInfoFormat::Oidc,
        },
        OAuthHttpProviderSettings {
            authorize_base_uri: "https://appleid.apple.test/auth/authorize".into(),
            token_uri: "https://appleid.apple.test/auth/token".into(),
            userinfo_uri: "https://appleid.apple.test/userinfo".into(),
            issuer: "https://appleid.apple.test".into(),
            jwks_json: stale_key.jwks_json(),
            jwks_uri: None,
            client_id: "apple-client-id".into(),
            client_secret: ["apple", "client", "placeholder"].join("-"),
            client_secret_generator: None,
            scopes: vec!["name".into(), "email".into()],
            token_auth_method: OAuthTokenAuthMethod::ClientSecretPost,
            userinfo_format: OAuthUserInfoFormat::Oidc,
        },
        OAuthHttpProviderSettings {
            authorize_base_uri: "https://x.test/i/oauth2/authorize".into(),
            token_uri: "https://api.x.test/2/oauth2/token".into(),
            userinfo_uri: "https://api.x.test/2/users/me".into(),
            issuer: String::new(),
            jwks_json: String::new(),
            jwks_uri: None,
            client_id: "x-client-id".into(),
            client_secret: ["x", "client", "placeholder"].join("-"),
            client_secret_generator: None,
            scopes: vec!["users.read".into(), "users.email".into()],
            token_auth_method: OAuthTokenAuthMethod::BasicAuth,
            userinfo_format: OAuthUserInfoFormat::XApiV2,
        },
    )
    .expect("provider");

    let user = provider
        .fetch_user_info(
            (IdentityProvider::Google).into(),
            "real-provider-code",
            "https://app.example.test/auth/oauth/google/callback",
            Some("verifier-123"),
            "nonce-123",
        )
        .await
        .expect("userinfo after JWKS refresh");

    assert_eq!(user.subject, "google-real-subject");
}

#[tokio::test]
async fn http_oauth_provider_maps_provider_failures_to_safe_errors() {
    let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind");
    let addr = listener.local_addr().expect("addr");
    let id_token_key = TestIdTokenKey::new("oauth-test-key");
    let server_state = Arc::new(TestOAuthServerState {
        id_token: id_token_key.id_token(
            "https://accounts.google.test",
            "google-client-id",
            "google-real-subject",
            "nonce-123",
        ),
        jwks_json: id_token_key.jwks_json(),
    });
    let app = Router::new()
        .route("/token-error", post(token_error_handler))
        .route("/token", post(token_handler))
        .route("/userinfo-error", get(userinfo_error_handler))
        .with_state(server_state);
    tokio::spawn(async move {
        axum::serve(listener, app).await.expect("server");
    });

    let base = format!("http://{addr}");
    let token_failure_provider = test_provider(
        &format!("{base}/token-error"),
        &format!("{base}/userinfo-error"),
        "https://accounts.google.test",
        id_token_key.jwks_json(),
    );
    let token_error = token_failure_provider
        .fetch_user_info(
            (IdentityProvider::Google).into(),
            "real-provider-code",
            "https://app.example.test/auth/oauth/google/callback",
            Some("verifier-123"),
            "nonce-123",
        )
        .await
        .expect_err("token failure should be mapped");
    let auth_service::application::errors::AuthError::ProviderFailure(token_failure) = &token_error
    else {
        panic!("classified provider failure: {token_error:?}");
    };
    assert_eq!(token_failure.code(), "PROVIDER_TOKEN_INVALID_GRANT");
    let token_error = token_error.to_string();
    assert_eq!(token_error, "OAuth provider token exchange failed");
    assert!(!token_error.contains("invalid_grant"));
    assert!(!token_error.contains("real-provider-code"));
    assert!(!token_error.contains("provider-secret-detail"));

    let userinfo_failure_provider = test_provider(
        &format!("{base}/token"),
        &format!("{base}/userinfo-error"),
        "https://accounts.google.test",
        id_token_key.jwks_json(),
    );
    let userinfo_error = userinfo_failure_provider
        .fetch_user_info(
            (IdentityProvider::Google).into(),
            "real-provider-code",
            "https://app.example.test/auth/oauth/google/callback",
            Some("verifier-123"),
            "nonce-123",
        )
        .await
        .expect_err("userinfo failure should be mapped");
    let userinfo_error = userinfo_error.to_string();
    assert_eq!(userinfo_error, "OAuth provider userinfo request failed");
    assert!(!userinfo_error.contains("insufficient_scope"));
    assert!(!userinfo_error.contains("provider-access-token"));
    assert!(!userinfo_error.contains("provider-secret-detail"));
}

fn test_provider(
    token_uri: &str,
    userinfo_uri: &str,
    issuer: &str,
    jwks_json: String,
) -> HttpOAuthProviderClient {
    test_provider_with_jwks_uri(token_uri, userinfo_uri, None, issuer, jwks_json)
}

fn test_provider_with_jwks_uri(
    token_uri: &str,
    userinfo_uri: &str,
    jwks_uri: Option<&str>,
    issuer: &str,
    jwks_json: String,
) -> HttpOAuthProviderClient {
    HttpOAuthProviderClient::new(
        OAuthHttpProviderSettings {
            authorize_base_uri: "https://accounts.google.test/o/oauth2/v2/auth".into(),
            token_uri: token_uri.into(),
            userinfo_uri: userinfo_uri.into(),
            issuer: issuer.into(),
            jwks_json: jwks_json.clone(),
            jwks_uri: jwks_uri.map(str::to_owned),
            client_id: "google-client-id".into(),
            client_secret: ["google", "client", "placeholder"].join("-"),
            client_secret_generator: None,
            scopes: vec!["openid".into(), "email".into(), "profile".into()],
            token_auth_method: OAuthTokenAuthMethod::ClientSecretPost,
            userinfo_format: OAuthUserInfoFormat::Oidc,
        },
        OAuthHttpProviderSettings {
            authorize_base_uri: "https://appleid.apple.test/auth/authorize".into(),
            token_uri: "https://appleid.apple.test/auth/token".into(),
            userinfo_uri: "https://appleid.apple.test/userinfo".into(),
            issuer: "https://appleid.apple.test".into(),
            jwks_json,
            jwks_uri: None,
            client_id: "apple-client-id".into(),
            client_secret: ["apple", "client", "placeholder"].join("-"),
            client_secret_generator: None,
            scopes: vec!["name".into(), "email".into()],
            token_auth_method: OAuthTokenAuthMethod::ClientSecretPost,
            userinfo_format: OAuthUserInfoFormat::Oidc,
        },
        OAuthHttpProviderSettings {
            authorize_base_uri: "https://x.test/i/oauth2/authorize".into(),
            token_uri: "https://api.x.test/2/oauth2/token".into(),
            userinfo_uri: "https://api.x.test/2/users/me".into(),
            issuer: String::new(),
            jwks_json: String::new(),
            jwks_uri: None,
            client_id: "x-client-id".into(),
            client_secret: ["x", "client", "placeholder"].join("-"),
            client_secret_generator: None,
            scopes: vec!["users.read".into(), "users.email".into()],
            token_auth_method: OAuthTokenAuthMethod::BasicAuth,
            userinfo_format: OAuthUserInfoFormat::XApiV2,
        },
    )
    .expect("provider")
}

async fn oversized_token_response() -> Response {
    let body = format!(
        r#"{{"access_token":"{}"}}"#,
        "x".repeat(EXPECTED_OAUTH_PROVIDER_RESPONSE_LIMIT_BYTES)
    );
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .header("content-length", body.len().to_string())
        .body(Body::from(body))
        .expect("oversized token response")
}

async fn at_limit_token_response(State(state): State<Arc<TestOAuthServerState>>) -> Response {
    let empty_padding = json!({
        "access_token": "provider-access-token",
        "id_token": state.id_token.as_str(),
        "padding": ""
    })
    .to_string();
    let padding_length = EXPECTED_OAUTH_PROVIDER_RESPONSE_LIMIT_BYTES - empty_padding.len();
    let body = json!({
        "access_token": "provider-access-token",
        "id_token": state.id_token.as_str(),
        "padding": "x".repeat(padding_length)
    })
    .to_string();
    assert_eq!(body.len(), EXPECTED_OAUTH_PROVIDER_RESPONSE_LIMIT_BYTES);
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .header("content-length", body.len().to_string())
        .body(Body::from(body))
        .expect("at-limit token response")
}

async fn oversized_chunked_response() -> Response {
    let chunks = tokio_stream::iter([
        Ok::<_, std::convert::Infallible>(vec![
            b'a';
            EXPECTED_OAUTH_PROVIDER_RESPONSE_LIMIT_BYTES / 2
        ]),
        Ok(vec![
            b'b';
            EXPECTED_OAUTH_PROVIDER_RESPONSE_LIMIT_BYTES / 2 + 1
        ]),
    ]);
    Response::builder()
        .status(StatusCode::OK)
        .header("content-type", "application/json")
        .body(Body::from_stream(chunks))
        .expect("chunked oversized response")
}

async fn oversized_userinfo_error_response() -> Response {
    let body = json!({
        "error": "insufficient_scope",
        "error_description": "x".repeat(EXPECTED_OAUTH_PROVIDER_ERROR_LIMIT_BYTES)
    })
    .to_string();
    assert!(body.len() > EXPECTED_OAUTH_PROVIDER_ERROR_LIMIT_BYTES);
    Response::builder()
        .status(StatusCode::FORBIDDEN)
        .header("content-type", "application/json")
        .header("content-length", body.len().to_string())
        .body(Body::from(body))
        .expect("oversized provider error response")
}

async fn token_handler(
    State(state): State<Arc<TestOAuthServerState>>,
    Form(form): Form<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if form.get("grant_type").map(String::as_str) != Some("authorization_code") {
        return Err(StatusCode::BAD_REQUEST);
    }
    if form.get("code").map(String::as_str) != Some("real-provider-code") {
        return Err(StatusCode::BAD_REQUEST);
    }
    if form.get("client_id").map(String::as_str) != Some("google-client-id") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let expected_secret = ["google", "client", "placeholder"].join("-");
    if form.get("client_secret").map(String::as_str) != Some(expected_secret.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    if form.get("code_verifier").map(String::as_str) != Some("verifier-123") {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(Json(json!({
        "access_token": "provider-access-token",
        "id_token": state.id_token,
        "token_type": "Bearer"
    })))
}

async fn x_token_handler(
    headers: HeaderMap,
    Form(form): Form<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    let expected_basic = format!(
        "Basic {}",
        Base64::encode_string("x-client-id:x-client-secret".as_bytes())
    );
    let authorization = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    if authorization != Some(expected_basic.as_str()) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    if form.contains_key("client_id") || form.contains_key("client_secret") {
        return Err(StatusCode::BAD_REQUEST);
    }
    if form.get("grant_type").map(String::as_str) != Some("authorization_code") {
        return Err(StatusCode::BAD_REQUEST);
    }
    if form.get("code").map(String::as_str) != Some("x-provider-code") {
        return Err(StatusCode::BAD_REQUEST);
    }
    if form.get("redirect_uri").map(String::as_str)
        != Some("https://app.example.test/auth/oauth/x/callback")
    {
        return Err(StatusCode::BAD_REQUEST);
    }
    if form.get("code_verifier").map(String::as_str) != Some("x-verifier-123") {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(Json(json!({
        "access_token": "x-provider-access-token",
        "token_type": "bearer"
    })))
}

/// Token endpoint for the Apple fixture.
///
/// The Apple exchange differs from the Google one used elsewhere in this file: the
/// client authenticates with its own client id and secret, and no PKCE verifier is
/// sent, so reusing the Google handler would reject a request that is correct for
/// Apple.
async fn apple_token_handler(
    State(state): State<Arc<TestOAuthServerState>>,
    Form(form): Form<HashMap<String, String>>,
) -> Result<Json<serde_json::Value>, StatusCode> {
    if form.get("grant_type").map(String::as_str) != Some("authorization_code") {
        return Err(StatusCode::BAD_REQUEST);
    }
    if form.get("client_id").map(String::as_str) != Some("apple-client-id") {
        return Err(StatusCode::BAD_REQUEST);
    }
    let expected_secret = ["apple", "client", "placeholder"].join("-");
    if form.get("client_secret").map(String::as_str) != Some(expected_secret.as_str()) {
        return Err(StatusCode::BAD_REQUEST);
    }
    Ok(Json(json!({
        "access_token": "apple-access-token",
        "id_token": state.id_token,
        "token_type": "Bearer"
    })))
}

async fn token_error_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::BAD_REQUEST,
        Json(json!({
            "error": "invalid_grant",
            "error_description": "provider-secret-detail real-provider-code"
        })),
    )
}

async fn userinfo_handler(headers: HeaderMap) -> Result<Json<serde_json::Value>, StatusCode> {
    let authorization = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    if authorization != Some("Bearer provider-access-token") {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(Json(json!({
        "sub": "google-real-subject",
        "email": "real-user@example.com",
        "email_verified": true
    })))
}

async fn x_userinfo_handler(headers: HeaderMap) -> Result<Json<serde_json::Value>, StatusCode> {
    let authorization = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok());
    if authorization != Some("Bearer x-provider-access-token") {
        return Err(StatusCode::UNAUTHORIZED);
    }
    Ok(Json(json!({
        "data": {
            "id": "2244994945",
            "name": "X Developers",
            "username": "XDevelopers",
            "confirmed_email": "x-user@example.com"
        }
    })))
}

async fn userinfo_error_handler() -> (StatusCode, Json<serde_json::Value>) {
    (
        StatusCode::FORBIDDEN,
        Json(json!({
            "error": "insufficient_scope",
            "error_description": "provider-secret-detail provider-access-token"
        })),
    )
}

async fn jwks_handler(State(state): State<Arc<TestOAuthServerState>>) -> Json<serde_json::Value> {
    Json(serde_json::from_str(&state.jwks_json).expect("jwks json"))
}

struct TestOAuthServerState {
    id_token: String,
    jwks_json: String,
}

struct TestIdTokenKey {
    kid: String,
    private_pem: String,
    public_key: RsaPublicKey,
}

impl TestIdTokenKey {
    fn new(kid: &str) -> Self {
        let public_key = RsaPublicKey::from(shared_test_rsa_private_key());
        let private_pem = shared_test_rsa_private_key_pem().to_owned();
        Self {
            kid: kid.into(),
            private_pem,
            public_key,
        }
    }

    fn jwks_json(&self) -> String {
        json!({
            "keys": [{
                "kty": "RSA",
                "use": "sig",
                "alg": "RS256",
                "kid": self.kid,
                "n": Base64UrlUnpadded::encode_string(&self.public_key.n().to_bytes_be()),
                "e": Base64UrlUnpadded::encode_string(&self.public_key.e().to_bytes_be())
            }]
        })
        .to_string()
    }

    fn id_token(&self, issuer: &str, audience: &str, subject: &str, nonce: &str) -> String {
        let now = Utc::now();
        let claims = TestIdTokenClaims {
            iss: issuer.into(),
            aud: audience.into(),
            sub: subject.into(),
            exp: (now + Duration::minutes(5)).timestamp(),
            iat: now.timestamp(),
            nonce: nonce.into(),
        };
        let header = Header {
            alg: Algorithm::RS256,
            kid: Some(self.kid.clone()),
            ..Header::default()
        };
        jsonwebtoken::encode(
            &header,
            &claims,
            &EncodingKey::from_rsa_pem(self.private_pem.as_bytes()).expect("encoding key"),
        )
        .expect("id token")
    }
}

#[derive(Serialize)]
struct TestIdTokenClaims {
    iss: String,
    aud: String,
    sub: String,
    exp: i64,
    iat: i64,
    nonce: String,
}

/// An ES256 identity-token signer, standing in for Apple.
///
/// Apple publishes EC keys and signs with ES256, while the other providers here
/// use RSA/RS256. Tests previously covered only the RSA shape, which is why an
/// implementation that assumed RSA passed while Apple sign-in could not work.
struct TestEcIdTokenKey {
    kid: String,
    signing_key: p256::ecdsa::SigningKey,
}

impl TestEcIdTokenKey {
    fn new(kid: &str) -> Self {
        // A fixed scalar keeps the fixture deterministic; the key is a test key
        // and never leaves this file.
        let signing_key = p256::ecdsa::SigningKey::from_bytes(&[7_u8; 32].into())
            .expect("deterministic P-256 signing key");
        Self {
            kid: kid.into(),
            signing_key,
        }
    }

    /// The JWKS a verifier would fetch, in the shape Apple publishes.
    fn jwks_json(&self) -> String {
        let point = self.signing_key.verifying_key().to_encoded_point(false);
        json!({
            "keys": [{
                "kty": "EC",
                "use": "sig",
                "alg": "ES256",
                "crv": "P-256",
                "kid": self.kid,
                "x": Base64UrlUnpadded::encode_string(point.x().expect("x coordinate")),
                "y": Base64UrlUnpadded::encode_string(point.y().expect("y coordinate")),
            }]
        })
        .to_string()
    }

    /// Produce a signed identity token with the given claims.
    fn id_token(&self, audience: &str, subject: &str, nonce: &str, email: Option<&str>) -> String {
        let now = Utc::now();
        let mut claims = json!({
            "iss": "https://appleid.apple.com",
            "aud": audience,
            "sub": subject,
            "exp": (now + Duration::minutes(5)).timestamp(),
            "iat": now.timestamp(),
            "nonce": nonce,
        });
        if let Some(email) = email {
            // Apple sends email_verified as a string, not a JSON boolean.
            claims["email"] = json!(email);
            claims["email_verified"] = json!("true");
        }
        let header = Header {
            alg: Algorithm::ES256,
            kid: Some(self.kid.clone()),
            ..Header::default()
        };
        let pem = self
            .signing_key
            .to_pkcs8_pem(Default::default())
            .expect("pkcs8 pem");
        jsonwebtoken::encode(
            &header,
            &claims,
            &EncodingKey::from_ec_pem(pem.as_bytes()).expect("encoding key"),
        )
        .expect("id token")
    }
}
