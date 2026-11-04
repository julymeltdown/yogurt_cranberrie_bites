//! Runtime settings and actual composite construction; no positive business fixtures.
#[path = "support/auth_fixture_config.rs"]
mod fixture;
use auth_service::{
    application::{
        dto::deletion_dto::AuthUserDeletionTargetDto as Target, errors::AuthError,
        ports::UserDeletionDispatcherPort,
    },
    config::concert_deletion::ConcertDeletionSettings as Settings,
};
use std::time::Duration;
#[test]
fn explicit_managed_settings_refuse_ambiguous_endpoint_and_clock() {
    assert!(!Settings::from_values(None, None, None).unwrap().managed());
    for (mode, endpoint, clock) in [
        ("", None, None),
        ("auto", None, None),
        ("managed", None, None),
        ("off", Some("https://concert.example"), None),
        ("managed", Some("http://concert.example"), None),
        ("managed", Some("https://user@concert.example"), None),
        ("managed", Some("https://concert.example/path"), None),
        (
            "managed",
            Some("https://concert.example?query=secret"),
            None,
        ),
        ("managed", Some("https://concert.example"), Some("0")),
        ("managed", Some("https://concert.example"), Some("5001")),
        ("managed", Some("https://concert.example"), Some("bad")),
    ] {
        assert!(Settings::from_values(Some(mode), endpoint, clock).is_err());
    }
    for ms in [1, 40, 5000] {
        let s = Settings::from_values(
            Some("managed"),
            Some("https://concert.example:50056/"),
            Some(&ms.to_string()),
        )
        .unwrap();
        assert!(s.managed());
        assert_eq!(s.deadline(), Duration::from_millis(ms));
    }
}
#[tokio::test]
async fn off_constructs_actual_composite_but_never_acknowledges_required_concert_steps(
) -> anyhow::Result<()> {
    let config = fixture::test_config("postgres://unused/mcr_auth".into())?;
    let settings = Settings::from_values(Some("off"), None, None)?;
    let d = settings.dispatcher(&config, Default::default())?;
    let user = uuid::Uuid::new_v4();
    for target in [
        Target::ConcertTickets,
        Target::ConcertSessions,
        Target::ConcertSharing,
        Target::ConcertPrivateData,
    ] {
        assert_eq!(
            d.apply(target, uuid::Uuid::new_v4(), user, user, chrono::Utc::now())
                .await,
            Err(AuthError::DependencyUnavailable)
        );
    }
    let managed = Settings::from_values(Some("managed"), Some("https://concert.example"), None)?;
    assert!(
        managed.validate(&config).is_err(),
        "managed never accepts absent mTLS"
    );
    Ok(())
}
