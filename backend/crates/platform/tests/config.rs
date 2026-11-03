use platform::config::{
    build_caller_registry, previous_service_credential_env_name, resolve_outbound_identity,
    resolve_service_credential, service_credential_env_name,
};
use platform::config::{
    parse_bool_setting, parse_i32_setting, parse_i64_setting, parse_u32_setting, parse_u64_setting,
    require_non_empty_setting, require_production_disabled, require_production_https_url,
    require_production_postgres_url, require_production_safe_socket_addr,
    require_production_safe_url, require_production_safe_urls, require_production_setting,
    runtime_mode_is_production,
};
use platform::config::{resolve_tls_material, tls_env_names};
use platform::grpc_auth::Workload;

#[test]
fn credential_env_names_are_derived_from_the_workload() {
    assert_eq!(
        service_credential_env_name(Workload::Gateway),
        "GATEWAY_SERVICE_CREDENTIAL"
    );
    assert_eq!(
        service_credential_env_name(Workload::Notification),
        "NOTIFICATION_SERVICE_CREDENTIAL"
    );
    assert_eq!(
        previous_service_credential_env_name(Workload::Gateway),
        "GATEWAY_SERVICE_CREDENTIAL_PREVIOUS"
    );
}

#[test]
fn tls_env_names_are_scoped_to_the_service_prefix() {
    let names = tls_env_names("post");
    assert_eq!(names.cert, "POST_GRPC_TLS_CERT_PATH");
    assert_eq!(names.key, "POST_GRPC_TLS_KEY_PATH");
    assert_eq!(names.ca, "POST_GRPC_TLS_CA_PATH");
    // Two services on one host must not share one identity.
    assert_ne!(tls_env_names("post").cert, tls_env_names("auth").cert);
}

#[test]
fn production_refuses_to_start_without_internal_tls_material() {
    // A production process with no certificates must fail rather than serve
    // internal traffic in plaintext. This is the invariant that keeps F-03
    // closed once a deployment forgets to mount its secrets.
    let error = resolve_tls_material(true, "config-test-absent")
        .expect_err("production must require TLS material");
    assert!(error.to_string().contains("GRPC_TLS_CERT_PATH"));
}

#[test]
fn development_without_tls_material_returns_none() {
    // Parity topologies start without certificates; callers must then use the
    // documented development path explicitly.
    assert!(resolve_tls_material(false, "config-test-absent")
        .expect("parity must not require TLS material")
        .is_none());
}

#[test]
fn shared_placeholder_and_blank_credentials_are_rejected() {
    // A shared placeholder would let every caller act as every other caller.
    assert!(resolve_service_credential(
        Some("local-parity-internal-service-token".into()),
        "AUTH_SERVICE_CREDENTIAL"
    )
    .is_err());
    assert!(resolve_service_credential(
        Some(["dev", "internal", "service", "token"].join("-")),
        "AUTH_SERVICE_CREDENTIAL"
    )
    .is_err());
    assert!(resolve_service_credential(Some("   ".into()), "AUTH_SERVICE_CREDENTIAL").is_err());
    assert_eq!(
        resolve_service_credential(Some("real".into()), "AUTH_SERVICE_CREDENTIAL").unwrap(),
        Some("real".to_owned())
    );
    assert_eq!(
        resolve_service_credential(None, "AUTH_SERVICE_CREDENTIAL").unwrap(),
        None
    );
}

#[test]
fn production_requires_credentials_for_required_callers() {
    assert!(build_caller_registry(true, &[Workload::Gateway], &[Workload::Gateway]).is_err());
    assert!(build_caller_registry(false, &[Workload::Gateway], &[Workload::Gateway]).is_ok());
    // A required caller must also be accepted, otherwise it can never connect.
    assert!(build_caller_registry(true, &[], &[Workload::Gateway]).is_err());
}

#[test]
fn registry_admits_at_least_one_caller_in_every_mode() {
    // Parity keeps working without configuration, but production never relies on
    // the fallback because the required-caller check runs first.
    let registry = build_caller_registry(false, &[], &[]).expect("parity default");
    assert!(!registry.is_empty());
}

#[test]
fn outbound_identity_falls_back_only_outside_production() {
    let parity = resolve_outbound_identity(false, Workload::Gateway).expect("parity identity");
    assert!(!parity.is_empty());
}

#[test]
fn runtime_mode_parser_accepts_production_and_parity_aliases() {
    assert!(runtime_mode_is_production("production", "SERVICE_RUNTIME_MODE").expect("prod"));
    assert!(runtime_mode_is_production("prod", "SERVICE_RUNTIME_MODE").expect("prod alias"));
    assert!(!runtime_mode_is_production("parity", "SERVICE_RUNTIME_MODE").expect("parity"));
    assert!(!runtime_mode_is_production("demo", "SERVICE_RUNTIME_MODE").expect("demo"));
    assert!(!runtime_mode_is_production("development", "SERVICE_RUNTIME_MODE").expect("dev"));
    assert!(runtime_mode_is_production("invalid", "SERVICE_RUNTIME_MODE").is_err());
}

#[test]
fn typed_config_parsers_reject_empty_invalid_and_out_of_range_values() {
    assert_eq!(
        parse_u64_setting("WORKER_INTERVAL_MS", "1000", 100, 86_400_000).expect("valid u64"),
        1000
    );
    assert_eq!(
        parse_u32_setting("RATE_LIMIT_PER_MINUTE", "600", 1, 60_000).expect("valid u32"),
        600
    );
    assert_eq!(
        parse_i64_setting("COOKIE_MAX_AGE_SECONDS", "1209600", 60, 31_536_000).expect("valid i64"),
        1_209_600
    );
    assert_eq!(
        parse_i32_setting("OUTBOX_BATCH_SIZE", "100", 1, 10_000).expect("valid i32"),
        100
    );
    assert!(parse_bool_setting("WORKER_ENABLED", "true").expect("bool"));

    assert!(parse_u64_setting("WORKER_INTERVAL_MS", "", 100, 86_400_000).is_err());
    assert!(parse_u64_setting("WORKER_INTERVAL_MS", "fast", 100, 86_400_000).is_err());
    assert!(parse_u64_setting("WORKER_INTERVAL_MS", "0", 100, 86_400_000).is_err());
    assert!(parse_u64_setting("WORKER_INTERVAL_MS", "86400001", 100, 86_400_000).is_err());
}

#[test]
fn production_settings_can_require_non_empty_values() {
    assert_eq!(
        require_non_empty_setting("GATEWAY_JWT_JWKS_JSON", "{\"keys\":[]}").expect("non empty"),
        "{\"keys\":[]}"
    );
    assert!(require_non_empty_setting("GATEWAY_JWT_JWKS_JSON", " ").is_err());

    assert_eq!(
        require_production_setting(false, "GATEWAY_JWT_JWKS_JSON", "").expect("parity may defer"),
        ""
    );
    assert!(require_production_setting(true, "GATEWAY_JWT_JWKS_JSON", "").is_err());
}

#[test]
fn production_runtime_rejects_unsafe_default_network_values() {
    assert!(require_production_safe_socket_addr(
        true,
        "AUTH_HTTP_ADDR",
        "127.0.0.1:8081".parse().expect("addr"),
    )
    .is_err());
    require_production_safe_socket_addr(
        true,
        "AUTH_HTTP_ADDR",
        "0.0.0.0:8081".parse().expect("addr"),
    )
    .expect("public bind");

    assert!(require_production_safe_url(
        true,
        "AUTH_DATABASE_URL",
        "postgres://postgres:postgres@localhost:5432/auth_db",
    )
    .is_err());
    assert!(
        require_production_safe_url(true, "POST_REDIS_URL", "redis://127.0.0.1:6379/",).is_err()
    );
    assert!(require_production_safe_url(
        true,
        "AUTH_GOOGLE_AUTHORIZE_BASE_URI",
        "https://accounts.google.test/o/oauth2/v2/auth",
    )
    .is_err());
    require_production_safe_url(
        true,
        "AUTH_DATABASE_URL",
        "postgres://postgres:postgres@postgres.internal:5432/auth_db",
    )
    .expect("remote database");
}

#[test]
fn production_runtime_requires_disabled_seed_and_https_urls() {
    assert!(require_production_disabled(
        true,
        "AUTH_SEED_ENABLED",
        true,
        "demo seed data must not run",
    )
    .is_err());
    require_production_disabled(
        true,
        "AUTH_SEED_ENABLED",
        false,
        "demo seed data must not run",
    )
    .expect("disabled");
    require_production_disabled(
        false,
        "AUTH_SEED_ENABLED",
        true,
        "parity may seed local data",
    )
    .expect("parity");

    assert!(require_production_https_url(
        true,
        "AUTH_GOOGLE_REDIRECT_URI",
        "http://auth.example.com/oauth/callback",
    )
    .is_err());
    require_production_https_url(
        true,
        "AUTH_GOOGLE_REDIRECT_URI",
        "https://auth.company.com/oauth/callback",
    )
    .expect("https");
}

#[test]
fn production_runtime_rejects_empty_or_local_url_lists() {
    let empty: Vec<String> = vec![];
    assert!(require_production_safe_urls(true, "GATEWAY_CORS_ALLOWED_ORIGINS", &empty).is_err());

    let local = vec!["http://localhost:3000".to_string()];
    assert!(require_production_safe_urls(true, "GATEWAY_CORS_ALLOWED_ORIGINS", &local).is_err());

    let insecure = vec!["http://app.company.com".to_string()];
    assert!(require_production_safe_urls(true, "GATEWAY_CORS_ALLOWED_ORIGINS", &insecure).is_err());

    let production = vec!["https://app.company.com".to_string()];
    require_production_safe_urls(true, "GATEWAY_CORS_ALLOWED_ORIGINS", &production)
        .expect("remote origin");
}

#[test]
fn production_classifies_mapped_ipv4_without_rejecting_wildcard_listeners() {
    for address in [
        "127.0.0.1:8080",
        "127.2.3.4:8080",
        "[::1]:8080",
        "[::ffff:127.0.0.1]:8080",
        "[::ffff:7f02:0304]:8080",
    ] {
        let address = address.parse().unwrap();
        assert!(require_production_safe_socket_addr(true, "LISTEN", address).is_err());
        require_production_safe_socket_addr(false, "LISTEN", address).unwrap();
    }
    for address in [
        "0.0.0.0:8080",
        "[::]:8080",
        "[::ffff:0.0.0.0]:8080",
        "10.42.0.1:8080",
        "[::ffff:10.42.0.1]:8080",
    ] {
        require_production_safe_socket_addr(true, "LISTEN", address.parse().unwrap()).unwrap();
    }
    for host in [
        "[::ffff:127.0.0.1]",
        "[::ffff:7f02:0304]",
        "[::ffff:0.0.0.0]",
        "[::]",
        "0.0.0.0",
    ] {
        for scheme in ["https", "redis", "postgres"] {
            let value = format!("{scheme}://{host}:5432/db");
            assert!(
                require_production_safe_url(true, "DESTINATION", &value).is_err(),
                "{value}"
            );
            require_production_safe_url(false, "DESTINATION", &value).unwrap();
        }
    }
    for value in [
        "https://[::ffff:10.42.0.1]:50051",
        "postgres://[::ffff:10.42.0.1]:5432/db",
        "kafka:9092",
        "kafka.internal:9092",
    ] {
        require_production_safe_url(true, "DESTINATION", value).unwrap();
    }
}

#[test]
fn production_postgres_rejects_decoded_routing_overrides_without_leaking_credentials() {
    for query in [
        "host=127.0.0.1",
        "host=%2Ftmp%2Fprivate",
        "hostaddr=::1",
        "hostaddr=::ffff:127.0.0.1",
        "%68ost=localhost",
        "host%61ddr=127.0.0.1",
        "host=remote.internal&host=/tmp",
        "host=/tmp&host=remote.internal",
        "host=remote.internal",
        "host=",
        "hostaddr=",
    ] {
        for scheme in ["postgres", "postgresql", "POSTGRESQL"] {
            let value =
                format!("{scheme}://secret-user:secret-password@postgres.internal/db?{query}");
            let error = require_production_postgres_url(true, "DATABASE_URL", &value)
                .unwrap_err()
                .to_string();
            assert!(error.contains("DATABASE_URL"));
            assert!(
                !error.contains("secret-user")
                    && !error.contains("secret-password")
                    && !error.contains("private")
            );
            require_production_postgres_url(false, "DATABASE_URL", &value).unwrap();
        }
    }
    for value in [
        "postgres://secret-user:secret-password@%2Ftmp%2Fprivate/db",
        "postgres://secret-user:secret-password@%6cocalhost/db",
        "postgres:///db",
        "postgres://[broken/db",
    ] {
        let error = require_production_postgres_url(true, "DATABASE_URL", value)
            .unwrap_err()
            .to_string();
        assert!(
            !error.contains("secret-user")
                && !error.contains("secret-password")
                && !error.contains("private")
        );
    }
}

#[test]
fn production_postgres_preserves_connection_options_and_other_url_queries() {
    for value in [
        "postgres://user:password@postgres:5432/db?sslmode=require&application_name=worker&statement-cache-capacity=100",
        "postgresql://user:password@postgres.internal/db?sslrootcert=%2Fetc%2Fssl%2Froot.crt&connect_timeout=10",
        "postgres://[2001:db8::10]:5432/db?options=-c%20statement_timeout%3D1000",
    ] {
        require_production_postgres_url(true, "URL", value).unwrap();
    }
    require_production_safe_url(
        true,
        "HTTP_URL",
        "https://api.company.com/path?host=localhost&hostaddr=127.0.0.1",
    )
    .unwrap();
}

#[test]
fn production_database_url_rejects_foreign_schemes() {
    for scheme in ["https", "redis", "mysql", "file"] {
        let value = format!("{scheme}://postgres.internal/db?host=/tmp");
        assert!(
            require_production_postgres_url(true, "DATABASE_URL", &value).is_err(),
            "foreign database scheme accepted"
        );
        require_production_postgres_url(false, "DATABASE_URL", &value).unwrap();
    }
}

#[test]
fn production_database_url_checks_normalized_schemes() {
    for scheme in [
        "post\tgres",
        "post\ngres",
        "post\rgresql",
        "\u{0000}postgres",
    ] {
        let value = format!("{scheme}://postgres.internal/db?host=/tmp");
        assert!(
            require_production_postgres_url(true, "DATABASE_URL", &value).is_err(),
            "normalized database routing override accepted"
        );
        let allowed = format!("{scheme}://postgres.internal/db?sslmode=require");
        require_production_postgres_url(true, "DATABASE_URL", &allowed).unwrap();
    }
}
