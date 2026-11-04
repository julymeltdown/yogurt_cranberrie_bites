use api_gateway::config::{
    AppConfig, DeveloperPortalLinkSettings, DeveloperPortalLinks, RuntimeMode,
};

#[test]
fn concert_endpoint_is_explicit_and_obeys_production_endpoint_policy() {
    let mut config = production_config();
    assert!(config.concert_grpc_endpoint.is_none());
    config.validate().unwrap();
    config.concert_grpc_endpoint = Some("http://concert.internal:50056".into());
    config.validate().unwrap();
    for endpoint in [
        "",
        "http://127.0.0.1:50056",
        "not-a-url",
        "ftp://concert.internal",
        "http://concert.internal/path",
        "http://concert.internal?secret=value",
        "http://user:password@concert.internal",
        "http://concert.internal#fragment",
    ] {
        config.concert_grpc_endpoint = Some(endpoint.into());
        let error = config.validate().expect_err(endpoint);
        assert!(error.to_string().contains("GATEWAY_CONCERT_GRPC_ENDPOINT"));
    }
}

#[test]
fn production_config_rejects_unsafe_gateway_defaults() {
    let mut config = production_config();
    config.http_addr = "127.0.0.1:8080".parse().expect("addr");
    assert!(config
        .validate()
        .expect_err("loopback bind must fail")
        .to_string()
        .contains("GATEWAY_HTTP_ADDR"));

    let mut config = production_config();
    config.grpc_addr = "127.0.0.1:50050".parse().expect("addr");
    assert!(config
        .validate()
        .expect_err("loopback exchange listener must fail")
        .to_string()
        .contains("GATEWAY_GRPC_ADDR"));

    let mut config = production_config();
    config.database_url = "postgres://postgres:postgres@localhost:5432/gateway_db".into();
    assert!(config
        .validate()
        .expect_err("local database must fail")
        .to_string()
        .contains("GATEWAY_DATABASE_URL"));

    let mut config = production_config();
    config.auth_grpc_endpoint = "http://127.0.0.1:50051".into();
    assert!(config
        .validate()
        .expect_err("local downstream must fail")
        .to_string()
        .contains("GATEWAY_AUTH_GRPC_ENDPOINT"));

    let mut config = production_config();
    config.seed_enabled = true;
    assert!(config
        .validate()
        .expect_err("seed must fail")
        .to_string()
        .contains("GATEWAY_SEED_ENABLED"));

    let mut config = production_config();
    config.refresh_cookie_secure = false;
    assert!(config
        .validate()
        .expect_err("insecure cookie must fail")
        .to_string()
        .contains("GATEWAY_REFRESH_COOKIE_SECURE"));

    let mut config = production_config();
    // With refresh off, configuration is the only key source.
    config.jwt_keys_refresh_seconds = 0;
    config.jwt_jwks_json = String::new();
    assert!(config
        .validate()
        .expect_err("missing JWKS must fail")
        .to_string()
        .contains("GATEWAY_JWT_JWKS_JSON"));

    let mut config = production_config();
    config.cors_allowed_origins = vec!["http://app.company.com".into()];
    assert!(config
        .validate()
        .expect_err("insecure CORS origin must fail")
        .to_string()
        .contains("GATEWAY_CORS_ALLOWED_ORIGINS"));

    let mut config = production_config();
    config.cors_allowed_origins = vec!["https://app.company.com\nbad".into()];
    assert!(config
        .validate()
        .expect_err("invalid CORS origin must fail")
        .to_string()
        .contains("GATEWAY_CORS_ALLOWED_ORIGINS"));
}

#[test]
fn browser_origin_allowlist_rejects_non_origins_in_every_runtime_mode() {
    for mode in [RuntimeMode::Production, RuntimeMode::Parity] {
        for origin in [
            "*",
            "null",
            "https://*.company.com",
            "https://app.company.com/",
            "https://app.company.com/path",
            "https://app.company.com?next=hostile",
            "https://app.company.com#fragment",
            "https://user@app.company.com",
        ] {
            let mut config = production_config();
            config.runtime_mode = mode;
            config.cors_allowed_origins = vec![origin.into()];
            assert!(
                config
                    .validate()
                    .expect_err(origin)
                    .to_string()
                    .contains("GATEWAY_CORS_ALLOWED_ORIGINS"),
                "{mode:?}: {origin}"
            );
        }
    }
}

#[test]
fn production_config_accepts_explicit_remote_gateway_settings() {
    production_config().validate().expect("production config");
}

#[test]
fn authority_exchange_keeps_resource_evidence_keys_separate_from_user_authority() {
    use platform::actor_delegation::exchange::ResourceEvidenceVerifier;
    use platform::actor_delegation::ActorDelegationSigner;
    use platform::grpc_auth::Workload;
    let signer = ActorDelegationSigner::from_seed(Workload::Gateway, &[9; 32]).expect("signer");
    let reused = ResourceEvidenceVerifier::new(&[signer.public_key()]).expect("verifier");
    let error = api_gateway::config::authority_exchange(true, &signer, false, reused)
        .err()
        .expect("a resource key equal to the actor key must be refused");
    assert!(error
        .to_string()
        .contains("RESOURCE_EVIDENCE_VERIFYING_KEYS"));
    let owner = platform::actor_delegation::exchange::ResourceEvidenceSigner::from_seed(&[10; 32])
        .expect("owner");
    let separate = ResourceEvidenceVerifier::new(&[owner.public_key()]).expect("verifier");
    api_gateway::config::authority_exchange(true, &signer, false, separate)
        .expect("separate resource key with the current signing key");
    let gateway_signer =
        ActorDelegationSigner::from_seed(Workload::Post, &[9; 32]).expect("signer");
    assert!(api_gateway::config::authority_exchange(
        false,
        &gateway_signer,
        false,
        ResourceEvidenceVerifier::local_parity(),
    )
    .is_err());

    // During a signing-key rotation the parent list keeps the previous key.
    // That retained key must not be reusable as a resource-evidence key.
    let previous = ActorDelegationSigner::from_seed(Workload::Gateway, &[8; 32]).expect("previous");
    let rotating = || {
        platform::actor_delegation::ActorDelegationVerifier::new(
            Workload::Post,
            &[signer.public_key(), previous.public_key()],
        )
        .expect("rotation verifier")
    };
    let retained = ResourceEvidenceVerifier::new(&[owner.public_key(), previous.public_key()])
        .expect("verifier");
    assert!(
        api_gateway::config::authority_exchange_with_parents(&signer, rotating(), retained)
            .err()
            .expect("a retained actor key must not verify resource evidence")
            .to_string()
            .contains("ACTOR_DELEGATION_VERIFYING_KEYS")
    );
    api_gateway::config::authority_exchange_with_parents(
        &signer,
        rotating(),
        ResourceEvidenceVerifier::new(&[owner.public_key()]).expect("verifier"),
    )
    .expect("separate resource key during a rotation");
    let omits_current = platform::actor_delegation::ActorDelegationVerifier::new(
        Workload::Post,
        &[previous.public_key()],
    )
    .expect("verifier");
    assert!(api_gateway::config::authority_exchange_with_parents(
        &signer,
        omits_current,
        ResourceEvidenceVerifier::new(&[owner.public_key()]).expect("verifier"),
    )
    .is_err());
}

#[test]
fn async_operation_worker_keyring_is_optional_and_validated_when_enabled() {
    let mut config = production_config();
    config.validate().expect("worker is optional by default");

    config.async_operation_keyring = Some(format!("7:{}", "01".repeat(32)));
    config.journal_client_timeout_ms = 729;
    config
        .validate()
        .expect("valid keyring with acceptance-time margin");

    config.async_operation_keyring = Some("7:not-a-key".into());
    assert!(config
        .validate()
        .expect_err("malformed keyring must fail closed")
        .to_string()
        .contains("GATEWAY_ASYNC_OPERATION_KEYRING"));

    config.async_operation_keyring = Some(format!("7:{}", "01".repeat(32)));
    config.journal_client_timeout_ms = 730;
    assert!(config
        .validate()
        .expect_err("queue deadline must fit before the outer timeout")
        .to_string()
        .contains("must leave time for durable queue acceptance"));
}

#[test]
fn anonymous_identity_budgets_require_a_shared_strong_hmac_key() {
    let mut config = production_config();
    config.admission = config
        .admission
        .clone()
        .with_identity_overrides("login=6")
        .expect("identity budget");
    config.admission_hmac_key.clear();
    assert!(config
        .validate()
        .expect_err("identity budget without HMAC key must fail")
        .to_string()
        .contains("GATEWAY_ADMISSION_HMAC_KEY"));

    config.admission_hmac_key = "too-short".into();
    assert!(config
        .validate()
        .expect_err("short HMAC key must fail")
        .to_string()
        .contains("between 32 and 4096 bytes"));

    config.admission_hmac_key = "test-admission-hmac-key-with-at-least-32-bytes".into();
    config
        .validate()
        .expect("configured identity budget with a strong shared key");
}

#[test]
fn authenticated_principal_budgets_require_a_shared_strong_hmac_key() {
    let mut config = production_config();
    config.admission = config
        .admission
        .clone()
        .with_principal_overrides("search=20")
        .expect("principal budget");
    config.admission_hmac_key.clear();
    assert!(config
        .validate()
        .expect_err("principal budget without HMAC key must fail")
        .to_string()
        .contains("GATEWAY_ADMISSION_HMAC_KEY"));

    config.admission_hmac_key = "test-admission-hmac-key-with-at-least-32-bytes".into();
    config
        .validate()
        .expect("configured principal budget with a strong shared key");
}

#[test]
fn signing_keys_may_come_from_the_issuer_instead_of_configuration() {
    // Given: production with key refresh on and no configured key set.
    let mut config = production_config();
    config.jwt_jwks_json = String::new();

    // Then: the Gateway may start and take its keys from the issuer. Before key
    // refresh existed, production required a static key set here.
    config.validate().expect("issuer-supplied keys");
}

#[test]
fn signing_key_refresh_settings_are_bounded() {
    let refused = |config: AppConfig| config.validate().expect_err("must fail").to_string();

    // Refresh off with no configured keys would start a Gateway that can never
    // accept a token.
    let mut config = production_config();
    config.jwt_keys_refresh_seconds = 0;
    config.jwt_jwks_json = "  ".into();
    let error = refused(config);
    assert!(error.contains("GATEWAY_JWT_JWKS_JSON"), "{error}");
    assert!(
        error.contains("GATEWAY_JWT_KEYS_REFRESH_SECONDS"),
        "{error}"
    );

    // A refresh interval below the floor would turn the issuer channel into a
    // polling load.
    let mut config = production_config();
    config.jwt_keys_refresh_seconds = 4;
    assert!(refused(config).contains("GATEWAY_JWT_KEYS_REFRESH_SECONDS"));

    // Early refreshes for unknown keys may not be allowed less often than
    // scheduled ones.
    let mut config = production_config();
    config.jwt_keys_min_refresh_seconds = config.jwt_keys_refresh_seconds + 1;
    assert!(refused(config).contains("GATEWAY_JWT_KEYS_MIN_REFRESH_SECONDS"));

    // Control: the floor itself is accepted.
    let mut config = production_config();
    config.jwt_keys_refresh_seconds = 5;
    config.jwt_keys_min_refresh_seconds = 5;
    config.validate().expect("floor values");
}

#[test]
fn parity_config_allows_local_gateway_defaults() {
    let mut config = production_config();
    config.runtime_mode = RuntimeMode::Parity;
    config.http_addr = "127.0.0.1:8080".parse().expect("addr");
    config.database_url = "postgres://postgres:postgres@localhost:5432/gateway_db".into();
    config.auth_grpc_endpoint = "http://127.0.0.1:50051".into();
    config.seed_enabled = true;
    config.refresh_cookie_secure = false;
    config.refresh_cookie_domain = String::new();
    config.cors_allowed_origins = vec!["http://localhost:3000".into()];
    config.validate().expect("parity config");
}

#[test]
fn developer_portal_links_follow_runtime_defaults_and_overrides() {
    let parity = DeveloperPortalLinks::defaults(RuntimeMode::Parity);
    assert_eq!(parity.swagger(), Some("http://localhost:8090/"));
    assert_eq!(parity.grafana(), Some("http://localhost:3000/"));
    assert_eq!(parity.prometheus(), Some("http://localhost:9090/"));

    let production = DeveloperPortalLinks::defaults(RuntimeMode::Production);
    assert_eq!(production.swagger(), None);
    assert_eq!(production.grafana(), None);
    assert_eq!(production.prometheus(), None);

    let configured = DeveloperPortalLinks::from_settings(
        RuntimeMode::Production,
        DeveloperPortalLinkSettings {
            grafana: Some("https://metrics.company.com/d/overview".into()),
            ..DeveloperPortalLinkSettings::default()
        },
    )
    .expect("valid external dashboard link");
    assert_eq!(
        configured.grafana(),
        Some("https://metrics.company.com/d/overview")
    );
    assert_eq!(configured.swagger(), None);

    let hidden = DeveloperPortalLinks::from_settings(
        RuntimeMode::Parity,
        DeveloperPortalLinkSettings {
            grafana: Some("  ".into()),
            ..DeveloperPortalLinkSettings::default()
        },
    )
    .expect("blank override hides the local link");
    assert_eq!(hidden.grafana(), None);
}

#[test]
fn developer_portal_links_reject_unsafe_urls() {
    for value in [
        "javascript:alert(1)",
        "//metrics.company.com/d/overview",
        "http://metrics.company.com/d/overview",
        "https://localhost/d/overview",
        "https://user:secret@metrics.company.com/d/overview",
    ] {
        let error = DeveloperPortalLinks::from_settings(
            RuntimeMode::Production,
            DeveloperPortalLinkSettings {
                grafana: Some(value.into()),
                ..DeveloperPortalLinkSettings::default()
            },
        )
        .expect_err("unsafe or non-HTTPS production URL must be rejected");
        assert!(
            error.to_string().contains("GATEWAY_DOCS_GRAFANA_URL"),
            "unexpected error for {value}: {error}"
        );
    }

    let too_long = format!("https://metrics.company.com/{}", "x".repeat(2_050));
    assert!(DeveloperPortalLinks::from_settings(
        RuntimeMode::Parity,
        DeveloperPortalLinkSettings {
            grafana: Some(too_long),
            ..DeveloperPortalLinkSettings::default()
        },
    )
    .expect_err("oversized URL must be rejected")
    .to_string()
    .contains("must not exceed 2048 bytes"));
}

fn production_config() -> AppConfig {
    AppConfig {
        database_migration_mode: migration_runtime::MigrationMode::Startup,
        http_addr: "0.0.0.0:8080".parse().expect("addr"),
        metrics_addr: "127.0.0.1:9080".parse().expect("metrics addr"),
        grpc_addr: "0.0.0.0:50050".parse().expect("grpc addr"),
        database_pool_max_connections: 5,
        journal_pool_max_connections: 4,
        background_pool_max_connections: 3,
        database_url: "postgres://gateway:secret@postgres.internal:5432/gateway_db".into(),
        seed_enabled: false,
        auth_grpc_endpoint: "http://auth.internal:50051".into(),
        concert_grpc_endpoint: None,
        concert_release_digest: None,
        profile_grpc_endpoint: "http://profile.internal:50052".into(),
        friend_grpc_endpoint: "http://friend.internal:50053".into(),
        notification_grpc_endpoint: "http://notification.internal:50054".into(),
        post_grpc_endpoint: "http://post.internal:50055".into(),
        refresh_cookie_domain: ".company.com".into(),
        refresh_cookie_secure: true,
        refresh_cookie_max_age_seconds: 1_209_600,
        aggregation_cache_ttl_seconds: 30,
        cors_allowed_origins: vec!["https://app.company.com".into()],
        jwt_issuer: "auth-service".into(),
        jwt_audience: "port-rust-api".into(),
        jwt_jwks_json: "{\"keys\":[]}".into(),
        jwt_keys_refresh_seconds: 60,
        jwt_keys_min_refresh_seconds: 10,
        high_risk_session_freshness_seconds: 5,
        runtime_mode: RuntimeMode::Production,
        developer_portal_links: api_gateway::config::DeveloperPortalLinks::defaults(
            RuntimeMode::Production,
        ),
        rate_limit_per_minute: 600,
        admission: api_gateway::domain::admission::AdmissionPolicies::default(),
        admission_hmac_key: "test-admission-hmac-key-with-at-least-32-bytes".into(),
        search_cursor_hmac_key: "test-search-cursor-key-has-32-bytes-minimum".into(),
        search_cursor_ttl_seconds: 900,
        search_cursor_legacy_accept_until: None,
        request_timeout_ms: 1000,
        journal_client_timeout_ms: 900,
        async_operation_keyring: None,
        telemetry_retention_days: 30,
        telemetry_queue_capacity: 4096,
        audit_retention_days: 365,
        actor_delegation_signer: platform::actor_delegation::ActorDelegationSigner::local_parity(
            platform::grpc_auth::Workload::Gateway,
        ),
        caller_registry: platform::grpc_auth::CallerRegistry::new(),
        authority_exchange: api_gateway::config::authority_exchange(
            false,
            &platform::actor_delegation::ActorDelegationSigner::local_parity(
                platform::grpc_auth::Workload::Gateway,
            ),
            false,
            platform::actor_delegation::exchange::ResourceEvidenceVerifier::local_parity(),
        )
        .expect("parity authority exchange"),
        outbound_identity: "production-service-token".into(),
        tls_material: None,
    }
}
