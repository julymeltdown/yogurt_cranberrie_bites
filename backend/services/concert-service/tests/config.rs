use concert_service::config::AppConfig;
use migration_runtime::MigrationMode;
#[test]
fn settings_are_explicit_and_never_default_to_a_source_database() {
    for url in [
        None,
        Some(""),
        Some("not-a-url"),
        Some("postgres://localhost"),
        Some("postgres://localhost/?dbname=mcr_concert"),
        Some("postgres:///mcr_concert"),
        Some("postgres://secret@localhost/postgres"),
        Some("postgres://secret@localhost/profile_db"),
        Some("postgres://secret@localhost/mcr_concert_"),
        Some("postgres://secret@localhost/mcr_concert?options=-csearch_path%3Dforeign"),
    ] {
        let error = AppConfig::from_values(url, Some("127.0.0.1:1"), Some("local"), None)
            .err()
            .expect("invalid database");
        assert!(!error.to_string().contains("secret"));
    }
    assert!(AppConfig::from_values(
        Some("postgres://user@localhost/mcr_concert"),
        None,
        Some("local"),
        None
    )
    .is_err());
    assert!(AppConfig::from_values(
        Some("postgres://user@localhost/mcr_concert"),
        Some("0.0.0.0:1"),
        Some("local"),
        None
    )
    .is_err());
    assert!(AppConfig::from_values(
        Some("postgres://user@localhost/mcr_concert"),
        Some("127.0.0.1:1"),
        Some("demo"),
        None
    )
    .is_err());
}
#[test]
fn production_does_not_apply_schema_and_local_startup_requires_an_explicit_setting() {
    let url = Some("postgres://user@database/mcr_concert");
    let address = Some("0.0.0.0:43100");
    assert_eq!(
        AppConfig::from_values(url, address, None, None)
            .unwrap()
            .migration_mode,
        MigrationMode::External
    );
    for mode in [Some("startup"), Some(""), Some("EXTERNAL")] {
        assert!(AppConfig::from_values(url, address, None, mode).is_err());
    }
    assert!(AppConfig::from_values(url, Some("127.0.0.1:0"), None, None).is_err());
    assert_eq!(
        AppConfig::from_values(url, Some("127.0.0.1:0"), Some("local"), Some("startup"))
            .unwrap()
            .migration_mode,
        MigrationMode::Startup
    );
}
