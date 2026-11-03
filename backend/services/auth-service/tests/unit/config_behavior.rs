use crate::support::{shared_test_rsa_private_key_pem, test_jwt_service};
use auth_service::application::ports::{OAuthProviderPort, TokenServicePort};
use auth_service::config::{
    build_oauth_provider, AppConfig, JwtTokenService, PasswordResetMailerMode, RuntimeMode,
};
use auth_service::domain::user::IdentityProvider;

#[test]
fn runtime_mode_parse_accepts_documented_aliases() {
    for value in ["production", "prod"] {
        assert_eq!(
            RuntimeMode::parse(value).expect("production"),
            RuntimeMode::Production
        );
    }
    for value in ["parity", "demo", "development", "dev"] {
        assert_eq!(
            RuntimeMode::parse(value).expect("parity"),
            RuntimeMode::Parity
        );
    }
}

#[test]
fn runtime_mode_parse_rejects_unknown_values() {
    let error = RuntimeMode::parse("staging").expect_err("unknown runtime mode");

    assert_eq!(
        error.to_string(),
        "AUTH_RUNTIME_MODE must be production or parity"
    );
}

#[test]
fn jwt_token_service_rejects_missing_identity_settings() {
    let issuer_error = error_string(JwtTokenService::new("", "port-rust-api-test", 900, 1209600));
    let audience_error = error_string(JwtTokenService::new("auth-service-test", " ", 900, 1209600));

    assert_eq!(issuer_error.to_string(), "JWT issuer is required");
    assert_eq!(
        audience_error.to_string(),
        "JWT access audience is required"
    );
}

#[test]
fn jwt_token_service_rejects_empty_key_id_for_configured_key() {
    let private_pem = shared_test_rsa_private_key_pem();

    let error = error_string(JwtTokenService::from_private_key_pem(
        "auth-service-test",
        "port-rust-api-test",
        900,
        1209600,
        private_pem,
        " ",
    ));

    assert_eq!(error.to_string(), "JWT key id is required");
}

#[test]
fn refresh_decoder_rejects_access_tokens() {
    let token_service = test_jwt_service();
    let tokens = token_service
        .issue_token_pair(uuid::Uuid::new_v4(), vec!["ROLE_USER".into()])
        .expect("issue token pair");

    let error = token_service
        .decode_refresh_token(&tokens.access_token)
        .expect_err("access token is not a refresh token");

    assert_eq!(error.to_string(), "Invalid token type");
}

#[tokio::test]
async fn parity_oauth_provider_uses_fixture_authorization_and_userinfo() {
    let config = oauth_config(RuntimeMode::Parity);
    let provider = build_oauth_provider(&config).expect("parity provider");

    let authorization_uri = provider
        .build_authorization_uri(
            (IdentityProvider::Google).into(),
            "http://127.0.0.1/callback",
            "state-1",
            "challenge-1",
            "nonce-1",
        )
        .expect("authorization URI");
    let userinfo = provider
        .fetch_user_info(
            (IdentityProvider::Google).into(),
            "google-hana-code",
            "http://127.0.0.1/callback",
            Some("verifier"),
            "nonce-1",
        )
        .await
        .expect("fixture userinfo");

    assert!(authorization_uri.starts_with("https://accounts.google.test/o/oauth2/v2/auth?"));
    assert!(authorization_uri.contains("client_id=google-rust-client"));
    assert_eq!(userinfo.subject, "google-hana-subject");
    assert_eq!(userinfo.email, "hana@example.com");
}

#[test]
fn apple_alone_requests_form_post_in_both_provider_modes() {
    for mode in [RuntimeMode::Parity, RuntimeMode::Production] {
        let provider = build_oauth_provider(&oauth_config(mode)).expect("configured provider");
        for identity in [
            IdentityProvider::Apple,
            IdentityProvider::Google,
            IdentityProvider::X,
        ] {
            let uri = provider
                .build_authorization_uri(
                    identity.into(),
                    "http://127.0.0.1/callback",
                    "original-state",
                    "challenge",
                    "nonce",
                )
                .expect("authorization URI");
            let url = reqwest::Url::parse(&uri).expect("authorization URL");
            let modes: Vec<_> = url
                .query_pairs()
                .filter(|(key, _)| key == "response_mode")
                .map(|(_, value)| value.into_owned())
                .collect();
            if identity == IdentityProvider::Apple {
                assert_eq!(modes, ["form_post"]);
            } else {
                assert!(modes.is_empty());
            }
            assert_eq!(
                url.query_pairs().find(|(key, _)| key == "state").unwrap().1,
                "original-state"
            );
        }
    }
}

#[test]
fn apple_form_post_replaces_configured_modes_and_retains_other_base_query_pairs() {
    for mode in [RuntimeMode::Parity, RuntimeMode::Production] {
        let mut config = oauth_config(mode);
        config
            .apple_authorize_base_uri
            .push_str("?tenant=local&response_mode=query&response_mode=form_post");
        let provider = build_oauth_provider(&config).expect("configured provider");
        let uri = provider
            .build_authorization_uri(
                IdentityProvider::Apple.into(),
                "http://127.0.0.1/callback",
                "original-state",
                "challenge",
                "nonce",
            )
            .expect("authorization URI");
        let url = reqwest::Url::parse(&uri).expect("authorization URL");
        let modes: Vec<_> = url
            .query_pairs()
            .filter(|(key, _)| key == "response_mode")
            .map(|(_, value)| value.into_owned())
            .collect();
        assert_eq!(modes, ["form_post"]);
        assert_eq!(
            url.query_pairs()
                .find(|(key, _)| key == "tenant")
                .unwrap()
                .1,
            "local"
        );
        assert_eq!(
            url.query_pairs().find(|(key, _)| key == "state").unwrap().1,
            "original-state"
        );
    }
}

#[test]
fn production_oauth_provider_builds_authorization_uris_from_config() {
    let config = oauth_config(RuntimeMode::Production);
    let provider = build_oauth_provider(&config).expect("production provider");

    let authorization_uri = provider
        .build_authorization_uri(
            (IdentityProvider::Apple).into(),
            "https://app.example.com/callback",
            "state-2",
            "challenge-2",
            "nonce-2",
        )
        .expect("authorization URI");

    assert!(authorization_uri.starts_with("https://appleid.apple.com/auth/authorize?"));
    assert!(authorization_uri.contains("client_id=apple-client"));
    assert!(authorization_uri.contains("scope=name+email"));

    let x_authorization_uri = provider
        .build_authorization_uri(
            (IdentityProvider::X).into(),
            "https://app.example.com/x/callback",
            "state-3",
            "challenge-3",
            "nonce-3",
        )
        .expect("X authorization URI");
    assert!(x_authorization_uri.starts_with("https://x.com/i/oauth2/authorize?"));
    assert!(x_authorization_uri.contains("client_id=x-client"));
    assert!(x_authorization_uri.contains("scope=users.read+users.email"));
    assert!(!x_authorization_uri.contains("nonce="));
}

#[test]
fn production_oauth_provider_requires_configured_credentials() {
    let mut config = oauth_config(RuntimeMode::Production);
    config.google_client_secret = None;

    let error = error_string(build_oauth_provider(&config));

    assert_eq!(
        error.to_string(),
        "AUTH_GOOGLE_CLIENT_SECRET is required in production OAuth mode"
    );

    let mut config = oauth_config(RuntimeMode::Production);
    config.x_client_id = None;

    let error = error_string(build_oauth_provider(&config));

    assert_eq!(
        error.to_string(),
        "AUTH_X_CLIENT_ID is required in production OAuth mode"
    );
}

fn error_string<T>(result: anyhow::Result<T>) -> String {
    match result {
        Ok(_) => panic!("expected error"),
        Err(error) => error.to_string(),
    }
}

pub(crate) fn oauth_config(runtime_mode: RuntimeMode) -> AppConfig {
    let jwks_json =
        auth_service::adapters::r#in::jwks::public_jwks(&(test_jwt_service().jwk_set()))
            .to_string();
    AppConfig {
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        http_addr: "127.0.0.1:8081".parse().expect("http addr"),
        metrics_addr: "127.0.0.1:9081".parse().expect("metrics addr"),
        grpc_addr: "127.0.0.1:50051".parse().expect("grpc addr"),
        database_pool_max_connections: 5,
        database_url: "postgres://postgres:postgres@localhost:5432/auth_db".into(),
        seed_enabled: false,
        public_application_routes_enabled: false,
        runtime_mode,
        access_ttl_seconds: 900,
        refresh_ttl_seconds: 1_209_600,
        issuer: "auth-service-test".into(),
        access_audience: "port-rust-api-test".into(),
        google_redirect_uri: "http://127.0.0.1:8080/auth/oauth/google/callback".into(),
        apple_redirect_uri: "http://127.0.0.1:8080/auth/oauth/apple/callback".into(),
        x_redirect_uri: "http://127.0.0.1:8080/auth/oauth/x/callback".into(),
        google_authorize_base_uri: "https://accounts.google.test/o/oauth2/v2/auth".into(),
        apple_authorize_base_uri: "https://appleid.apple.com/auth/authorize".into(),
        x_authorize_base_uri: "https://x.com/i/oauth2/authorize".into(),
        google_client_id: Some("google-client".into()),
        google_client_secret: Some("google-secret".into()),
        google_token_uri: Some("https://oauth2.googleapis.com/token".into()),
        google_userinfo_uri: Some("https://openidconnect.googleapis.com/v1/userinfo".into()),
        google_issuer: "https://accounts.google.com".into(),
        google_jwks_json: Some(jwks_json.clone()),
        google_jwks_uri: Some("https://www.googleapis.com/oauth2/v3/certs".into()),
        apple_client_id: Some("apple-client".into()),
        apple_client_secret: Some("apple-secret".into()),
        apple_team_id: None,
        apple_key_id: None,
        apple_private_key_pem: None,
        apple_client_secret_ttl_seconds: 86_400,
        apple_token_uri: Some("https://appleid.apple.com/auth/token".into()),
        apple_userinfo_uri: Some("https://appleid.apple.com/auth/userinfo".into()),
        apple_issuer: "https://appleid.apple.com".into(),
        apple_jwks_json: Some(jwks_json),
        apple_jwks_uri: Some("https://appleid.apple.com/auth/keys".into()),
        x_client_id: Some("x-client".into()),
        x_client_secret: Some("x-secret".into()),
        x_token_uri: Some("https://api.x.com/2/oauth2/token".into()),
        x_userinfo_uri: Some("https://api.x.com/2/users/me?user.fields=confirmed_email".into()),
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
        lifecycle_cleanup_worker_interval_ms: 3_600_000,
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
    }
}
