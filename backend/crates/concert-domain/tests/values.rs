use concert_domain::*;
use std::str::FromStr;

#[test]
fn canonical_auth_and_event_references_do_not_accept_aliases_or_provider_ids() {
    let canonical = "bcb5278a-a0f6-4e76-b783-83b36cd71700";
    assert!(AuthUserId::from_str(canonical).is_ok());
    assert!(EventId::from_str(canonical).is_ok());
    for value in [
        "X-account-1",
        "bcb5278aa0f64e76b78383b36cd71700",
        "BCB5278A-A0F6-4E76-B783-83B36CD71700",
        "",
        "uuid",
    ] {
        assert_eq!(
            AuthUserId::from_str(value).unwrap_err(),
            ValidationError::Identifier
        );
    }
}
#[test]
fn revisions_are_positive_bounded_and_checked_on_increment() {
    for value in [i64::MIN, -1, 0, MAX_REVISION + 1, i64::MAX] {
        assert!(Revision::new(value).is_err());
    }
    assert_eq!(Revision::initial().next().unwrap().get(), 2);
    assert!(Revision::new(MAX_REVISION).unwrap().next().is_err());
}
#[test]
fn event_names_preserve_unicode_and_enforce_slug_and_character_bounds() {
    for value in [
        "",
        "Upper",
        "-event",
        "event_name",
        "two words",
        &"a".repeat(65),
    ] {
        assert!(EventSlug::new(value.into()).is_err());
    }
    assert!(EventSlug::new("a0-concert".into()).is_ok());
    assert!(EventSlug::new("a".repeat(64)).is_ok());
    for value in ["", "   ", "bad\nname", &"é".repeat(121)] {
        assert!(EventTitle::new(value.into()).is_err());
    }
    assert_eq!(
        EventTitle::new("é".repeat(120))
            .unwrap()
            .as_str()
            .chars()
            .count(),
        120
    );
    assert_eq!(EventTitle::new(" Live ".into()).unwrap().as_str(), " Live ");
}
#[test]
fn asset_values_are_identifiers_never_urls_and_do_not_select_a_fallback() {
    for value in [
        "https://example.test/body.png",
        "../old",
        "Body",
        "a-b",
        "",
        &"a".repeat(65),
    ] {
        assert!(AssetId::new(value.into()).is_err());
    }
    assert!(AssetId::new("a_1".into()).is_ok());
    assert!(AssetId::new("a".repeat(64)).is_ok());
}
#[test]
fn frozen_event_and_theme_values_have_explicit_mutation_eligibility() {
    for value in ["OPEN", "LIVE"] {
        assert!(value
            .parse::<EventPhase>()
            .unwrap()
            .permits_participant_writes());
    }
    for value in ["DRAFT", "READ_ONLY", "ARCHIVED"] {
        assert!(!value
            .parse::<EventPhase>()
            .unwrap()
            .permits_participant_writes());
    }
    for value in ["CLOSED", "open", ""] {
        assert!(value.parse::<EventPhase>().is_err());
    }
    for value in ["REVENGE", "PARADE", "DANGER"] {
        assert_eq!(value.parse::<Theme>().unwrap().as_str(), value);
    }
    assert!("OTHER".parse::<Theme>().is_err());
    let now = chrono::Utc::now();
    assert!(validate_times(now, now).is_ok());
    assert!(validate_times(now, now - chrono::Duration::seconds(1)).is_err());
}
