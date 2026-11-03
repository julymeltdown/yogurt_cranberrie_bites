use chrono::{DateTime, Duration, NaiveDate, Utc};
use concert_domain::age_policy::{arithmetic::*, *};
use concert_domain::intro_policy::{contact::contact_bypass, *};
use concert_domain::{AuthUserId, EventId, Revision};
use uuid::Uuid;
fn time(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}
fn date(s: &str) -> NaiveDate {
    s.parse().unwrap()
}
fn user() -> AuthUserId {
    AuthUserId::from_uuid(Uuid::from_u128(1))
}
fn now() -> DateTime<Utc> {
    time("2026-01-01T00:00:00Z")
}
fn claim(age: i16) -> OwnerClaim {
    OwnerClaim {
        claim_id: Uuid::from_u128(10),
        user_id: user(),
        verifier_id: "fixture-verifier".into(),
        source_revision: "fixture-v1".into(),
        status: StoredStatus::Verified,
        verified_at: now(),
        expires_at: now() + Duration::days(90),
        age_at_verification: Some(age),
        next_birthday_at: Some(date("2026-02-01")),
    }
}
fn intro() -> IntroRecord {
    IntroRecord {
        intro_id: Uuid::from_u128(9),
        event_id: EventId::from_uuid(Uuid::from_u128(2)),
        owner_id: user(),
        text: "Meet near the event lobby".into(),
        age_bounds: AgeBounds::new(Some(25), Some(30)).unwrap(),
        expires_at: Some(now() + Duration::days(1)),
        created_at: now(),
        state: IntroState::Published,
        revision: Revision::initial(),
    }
}
#[test]
fn paired_bounds_are_required_and_half_open() {
    for json in [
        r#"{}"#,
        r#"{"minimum_age":null}"#,
        r#"{"maximum_age_exclusive":30}"#,
        r#"{"minimum_age":25,"maximum_age_exclusive":null}"#,
        r#"{"minimum_age":30,"maximum_age_exclusive":25}"#,
        r#"{"minimum_age":13,"maximum_age_exclusive":30}"#,
        r#"{"minimum_age":14,"maximum_age_exclusive":102}"#,
    ] {
        assert!(serde_json::from_str::<AgeBounds>(json).is_err(), "{json}");
    }
    assert!(!serde_json::from_str::<AgeBounds>(
        r#"{"minimum_age":null,"maximum_age_exclusive":null}"#
    )
    .unwrap()
    .restricted());
    let bounds = AgeBounds::new(Some(25), Some(30)).unwrap();
    for (age, allowed) in [(24, false), (25, true), (29, true), (30, false)] {
        assert_eq!(
            bounds.permits(&assess(user(), &[claim(age)], now()).unwrap(), false),
            allowed
        );
    }
}
#[test]
fn unknown_only_blocks_age_restricted_discovery_and_all_sharing() {
    let unknown = assess(user(), &[], now()).unwrap();
    let unbounded = AgeBounds::new(None, None).unwrap();
    assert!(unbounded.permits(&unknown, false));
    assert!(!unbounded.permits(&unknown, true));
    assert!(!AgeBounds::new(Some(14), Some(101))
        .unwrap()
        .permits(&unknown, false));
}
#[test]
fn audited_birthday_and_leap_semantics() {
    for (day, age, next) in [
        ("2026-01-31", 25, "2026-02-01"),
        ("2026-02-01", 26, "2027-02-01"),
        ("2026-02-02", 26, "2027-02-01"),
    ] {
        let c =
            derive_exact_age_claim(Some("2000"), Some("02-01"), SeoulDate::new(date(day))).unwrap();
        assert_eq!(c.age_at_verification(), age);
        assert_eq!(c.next_birthday_at().as_naive_date(), date(next));
    }
    let leap = derive_exact_age_claim(
        Some("2000"),
        Some("02-29"),
        SeoulDate::new(date("2025-02-27")),
    )
    .unwrap();
    assert_eq!(leap.age_at_verification(), 24);
    assert_eq!(leap.next_birthday_at().as_naive_date(), date("2025-02-28"));
    assert_eq!(leap.current_age(SeoulDate::new(date("2025-02-28"))), 25);
    let leap_year = derive_exact_age_claim(
        Some("2000"),
        Some("02-29"),
        SeoulDate::new(date("2024-02-28")),
    )
    .unwrap();
    assert_eq!(
        leap_year.next_birthday_at().as_naive_date(),
        date("2024-02-29")
    );
}
#[test]
fn malformed_provider_parts_and_supported_limits_fail_without_echo() {
    for (year, birthday) in [
        (None, Some("01-01")),
        (Some("2000"), None),
        (Some("２０00"), Some("01-01")),
        (Some("2000"), Some("2-01")),
        (Some("2001"), Some("02-29")),
        (Some("2027"), Some("01-01")),
        (Some("2013"), Some("01-01")),
        (Some("1925"), Some("01-01")),
    ] {
        assert!(
            derive_exact_age_claim(year, birthday, SeoulDate::new(date("2026-01-01"))).is_err()
        );
    }
    for year in ["2012", "1926"] {
        assert!(derive_exact_age_claim(
            Some(year),
            Some("01-01"),
            SeoulDate::new(date("2026-01-01"))
        )
        .is_ok());
    }
    assert_eq!(
        ExactAgeError::UnsupportedAge.to_string(),
        "claim is outside the supported range"
    );
}
#[test]
fn seoul_midnight_and_expiry_are_exact() {
    let c = claim(25);
    let b = AgeBounds::new(Some(25), Some(26)).unwrap();
    let before = time("2026-01-31T14:59:59Z");
    let on = time("2026-01-31T15:00:00Z");
    assert!(b.permits(&assess(user(), &[c.clone()], before).unwrap(), false));
    assert!(!b.permits(&assess(user(), &[c.clone()], on).unwrap(), false));
    assert_eq!(
        assess(user(), &[c.clone()], before).unwrap().recheck_at(),
        Some(on)
    );
    assert_eq!(
        assess(user(), &[c.clone()], c.expires_at).unwrap().status(),
        AgeStatus::Expired
    );
    assert_eq!(
        seoul_date(time("2025-12-31T15:00:00Z")).unwrap(),
        date("2026-01-01")
    );
}
#[test]
fn canonical_order_is_applied_before_any_validity_filter() {
    let old = claim(25);
    let mut newest = old.clone();
    newest.claim_id = Uuid::from_u128(11);
    newest.verified_at += Duration::seconds(1);
    newest.status = StoredStatus::Revoked;
    assert_eq!(
        assess(
            user(),
            &[old.clone(), newest.clone()],
            now() + Duration::seconds(2)
        )
        .unwrap()
        .status(),
        AgeStatus::Revoked
    );
    newest.verified_at = old.verified_at;
    newest.expires_at = old.expires_at + Duration::seconds(1);
    newest.claim_id = Uuid::from_u128(2);
    assert_eq!(
        assess(user(), &[newest.clone(), old.clone()], now())
            .unwrap()
            .status(),
        AgeStatus::Revoked
    );
    newest.expires_at = old.expires_at;
    newest.claim_id = Uuid::from_u128(11);
    for status in [
        StoredStatus::Unknown,
        StoredStatus::Unsupported,
        StoredStatus::Replaced,
    ] {
        newest.status = status;
        assert_ne!(
            assess(user(), &[old.clone(), newest.clone()], now())
                .unwrap()
                .status(),
            AgeStatus::Verified
        );
    }
    newest.status = StoredStatus::Verified;
    newest.age_at_verification = None;
    assert_eq!(
        assess(user(), &[old, newest], now()).unwrap().status(),
        AgeStatus::Unsupported
    );
}
#[test]
fn malformed_lifetime_binding_and_age_transition_fail_closed() {
    let mut c = claim(25);
    c.expires_at += Duration::seconds(1);
    assert_eq!(
        assess(user(), &[c], now()).unwrap().status(),
        AgeStatus::Unsupported
    );
    let mut c = claim(25);
    c.user_id = AuthUserId::from_uuid(Uuid::from_u128(88));
    assert!(assess(user(), &[c], now()).is_err());
    let c = claim(25);
    assert!(assess(user(), &[c.clone(), c], now()).is_err());
    let c = claim(100);
    assert_eq!(
        assess(user(), &[c], time("2026-02-01T00:00:00Z"))
            .unwrap()
            .status(),
        AgeStatus::Unsupported
    );
    let mut c = claim(25);
    c.next_birthday_at = Some(date("2026-01-01"));
    assert_eq!(
        assess(user(), &[c], now()).unwrap().status(),
        AgeStatus::Unsupported
    );
}
#[test]
fn public_projection_excludes_private_bounds_and_claim_material() {
    let i = intro();
    let public = serde_json::to_value(i.public_content()).unwrap();
    let own = serde_json::to_value(i.owner_projection()).unwrap();
    let mut keys: Vec<_> = public
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort();
    assert_eq!(
        keys,
        vec![
            "created_at",
            "event_id",
            "intro_id",
            "revision",
            "state",
            "text"
        ]
    );
    assert!(own.get("age_bounds").is_some());
    assert!(own.get("owner_id").is_some());
    assert_eq!(i.public_content().author(), user());
}
#[test]
fn draft_publication_expiry_state_and_event_rules() {
    let mut i = intro();
    let cutoff = now() + Duration::days(1);
    assert!(i.validate(now(), cutoff, true).is_ok());
    i.text.clear();
    assert!(i.validate(now(), cutoff, false).is_ok());
    assert!(i.validate(now(), cutoff, true).is_err());
    i.text = "Hello".into();
    i.expires_at = Some(now());
    assert!(i.validate(now(), cutoff, true).is_err());
    i.expires_at = None;
    i.age_bounds = AgeBounds::new(None, None).unwrap();
    assert!(i.visible(i.event_id, now(), None));
    i.state = IntroState::Closed;
    assert!(!i.visible(i.event_id, now(), None));
    i.state = IntroState::Published;
    assert!(!i.visible(EventId::from_uuid(Uuid::from_u128(7)), now(), None));
}
#[test]
fn audited_contact_obfuscation_and_safe_text_fixtures() {
    for text in [
        "@handle",
        "ｈｔｔｐｓ：／／example.com",
        "insta gram [dot] com",
        "discord。gg/invite",
        "t . me/me",
        "010-1234-5678",
        "카카오톡",
        "line id: hello",
    ] {
        assert!(contact_bypass(text), "{text}");
    }
    for text in [
        "See you in the lobby",
        "Queue line starts here",
        "A concert in 2026",
        "The annex.comparison is useful",
    ] {
        assert!(!contact_bypass(text), "{text}");
    }
}

#[test]
fn canonical_expired_newer_row_never_falls_back_to_older_live_row() {
    let old = claim(25);
    let mut newest = old.clone();
    newest.claim_id = Uuid::from_u128(11);
    newest.verified_at += Duration::seconds(1);
    newest.expires_at = now() + Duration::days(1);
    assert_eq!(
        assess(user(), &[newest, old], now() + Duration::days(2))
            .unwrap()
            .status(),
        AgeStatus::Expired
    );
}
