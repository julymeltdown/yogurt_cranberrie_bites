#[path = "support/mod.rs"]
mod support;

use sqlx::postgres::PgConnectOptions;

const SAFE_IPV4: &str = "postgres://notification:secret@127.0.0.1:5432/port_rust_notification_test";

#[test]
fn owned_database_mode_requires_url_and_exact_opt_in() {
    assert!(!support::owned_database_mode(false, None).unwrap());
    assert!(!support::owned_database_mode(true, None).unwrap());
    assert!(support::owned_database_mode(true, Some("1")).unwrap());
    for (url, flag) in [(false, "1"), (true, "0"), (true, ""), (true, "true")] {
        assert!(support::owned_database_mode(url, Some(flag)).is_err());
    }
}

fn validation_error(database_url: &str) -> String {
    support::validate_local_database_url_for_test(database_url)
        .expect_err("URL must be rejected")
        .to_string()
}

#[test]
fn accepts_canonical_ipv4_and_ipv6_test_databases() {
    let ipv4 = support::validate_local_database_url_for_test(SAFE_IPV4).expect("IPv4 URL");
    assert_eq!(ipv4.get_host(), "127.0.0.1");
    assert_eq!(ipv4.get_database(), Some("port_rust_notification_test"));

    let ipv6 = support::validate_local_database_url_for_test(
        "postgresql://notification:secret@[::1]:5432/port_rust_notification_test_a1_b2",
    )
    .expect("IPv6 URL");
    assert_eq!(
        ipv6.get_host()
            .trim_start_matches('[')
            .trim_end_matches(']'),
        "::1"
    );
    assert_eq!(
        ipv6.get_database(),
        Some("port_rust_notification_test_a1_b2")
    );
}

#[test]
fn rejects_all_raw_query_and_fragment_overrides() {
    for database_url in [
        "postgres://notification:secret@127.0.0.1:5432/port_rust_notification_test?host=198.51.100.10",
        "postgres://notification:secret@127.0.0.1:5432/port_rust_notification_test?dbname=production",
        "postgres://notification:secret@127.0.0.1:5432/port_rust_notification_test?options=-c%20search_path%3Dpublic",
        "postgres://notification:secret@127.0.0.1:5432/port_rust_notification_test#fragment",
    ] {
        assert_eq!(
            validation_error(database_url),
            "NOTIFICATION_TEST_DATABASE_URL must not contain query parameters or a fragment"
        );
    }
}

#[test]
fn rejects_noncanonical_percent_unicode_and_path_database_names() {
    for database_url in [
        "postgres://notification:secret@127.0.0.1:5432/%70ort_rust_notification_test",
        "postgres://notification:secret@127.0.0.1:5432/port_rust_notification_test_%E2%98%83",
        "postgres://notification:secret@127.0.0.1:5432/port_rust_notification_test_눈",
        "postgres://notification:secret@127.0.0.1:5432/port_rust_notification_test/nested",
        "postgres://notification:secret@127.0.0.1:5432/port_rust_notification_test%2Fnested",
        "postgres://notification:secret@127.0.0.1:5432/port_rust_notification_test__double",
        "postgres://notification:secret@127.0.0.1:5432/port_rust_notification_test-UPPER",
    ] {
        assert_eq!(
            validation_error(database_url),
            "NOTIFICATION_TEST_DATABASE_URL must name a canonical test database"
        );
    }
}

#[test]
fn rejects_names_longer_than_postgresql_identifier_limit() {
    let database = format!("port_rust_notification_test_{}", "a".repeat(40));
    let database_url = format!("postgres://notification:secret@127.0.0.1:5432/{database}");
    assert_eq!(
        validation_error(&database_url),
        "NOTIFICATION_TEST_DATABASE_URL must name a canonical test database"
    );
}

#[test]
fn validation_errors_do_not_echo_destination_database_or_credentials() {
    let cases = [
        (
            "postgres://sensitive_user:sensitive_password@198.51.100.77:5432/port_rust_notification_test",
            "NOTIFICATION_TEST_DATABASE_URL must use TCP loopback",
        ),
        (
            "postgres://sensitive_user:sensitive_password@127.0.0.1:5432/production_secret",
            "NOTIFICATION_TEST_DATABASE_URL must name a canonical test database",
        ),
        (
            "postgres://sensitive_user:sensitive_password@[::1/port_rust_notification_test",
            "NOTIFICATION_TEST_DATABASE_URL must be a valid PostgreSQL URL",
        ),
    ];
    for (database_url, expected) in cases {
        let error = validation_error(database_url);
        assert_eq!(error, expected);
        for secret in [
            "sensitive_user",
            "sensitive_password",
            "198.51.100.77",
            "production_secret",
        ] {
            assert!(!error.contains(secret));
        }
    }
}

#[test]
fn fixture_owned_options_are_added_only_after_clean_validation() {
    support::validate_local_database_url_for_test(SAFE_IPV4).expect("clean base URL");
    let scoped = support::scoped_database_url_for_test(
        SAFE_IPV4,
        "notification_fixture_app",
        Some("notification_fixture_deadbeef"),
    );
    let options: PgConnectOptions = scoped.parse().expect("scoped URL");
    assert_eq!(
        options.get_application_name(),
        Some("notification_fixture_app")
    );
    assert_eq!(
        options.get_options(),
        Some("-c search_path=notification_fixture_deadbeef,pg_catalog")
    );
}
