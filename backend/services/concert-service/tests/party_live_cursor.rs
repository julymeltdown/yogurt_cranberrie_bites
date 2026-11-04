use chrono::{Duration, TimeZone, Utc};
use concert_domain::community::party::PartyState;
use concert_service::party_live::{
    PartyCursorBinding, PartyCursorCodec, PartyPosition, PartyWindow,
};
use uuid::Uuid;

fn binding() -> PartyCursorBinding {
    PartyCursorBinding {
        actor: Uuid::from_u128(1),
        event: Uuid::from_u128(2),
        operation: "listConcertParties".into(),
        collection: "parties".into(),
        ticket_section: None,
        state: Some(PartyState::Open),
        has_space: Some(true),
        starts_after: None,
        starts_before: None,
        limit: 20,
    }
}

#[test]
fn cursor_binds_actor_event_query_limit_upper_and_position() {
    let codec = PartyCursorCodec::new(&[91; 32]).unwrap();
    let now = Utc.timestamp_opt(1793710000, 0).unwrap();
    let window = PartyWindow {
        upper: now,
        after: Some(PartyPosition {
            created_at: now - Duration::seconds(1),
            party_id: Uuid::from_u128(9),
        }),
    };
    let token = codec.seal(&binding(), window, now).unwrap();
    assert!(token
        .bytes()
        .all(|value| value.is_ascii_alphanumeric() || value == b'-' || value == b'_'));
    assert!(!token.contains(&binding().actor.to_string()));
    assert_eq!(codec.open(&binding(), &token, now).unwrap(), window);
    for changed in [
        PartyCursorBinding {
            actor: Uuid::from_u128(3),
            ..binding()
        },
        PartyCursorBinding {
            event: Uuid::from_u128(3),
            ..binding()
        },
        PartyCursorBinding {
            operation: "getConcertParty".into(),
            ..binding()
        },
        PartyCursorBinding {
            collection: "other".into(),
            ..binding()
        },
        PartyCursorBinding {
            ticket_section: Some("A".into()),
            ..binding()
        },
        PartyCursorBinding {
            state: Some(PartyState::Closed),
            ..binding()
        },
        PartyCursorBinding {
            has_space: Some(false),
            ..binding()
        },
        PartyCursorBinding {
            starts_after: Some(now - Duration::hours(1)),
            ..binding()
        },
        PartyCursorBinding {
            starts_before: Some(now + Duration::hours(1)),
            ..binding()
        },
        PartyCursorBinding {
            limit: 10,
            ..binding()
        },
    ] {
        assert!(codec.open(&changed, &token, now).is_err());
    }
    assert!(codec
        .open(&binding(), &token, now + Duration::minutes(15))
        .is_err());
    assert!(PartyCursorCodec::new(&[92; 32])
        .unwrap()
        .open(&binding(), &token, now)
        .is_err());
}

#[test]
fn timer_tamper_and_noncanonical_encoding_fail() {
    let codec = PartyCursorCodec::new(&[11; 32]).unwrap();
    let now = Utc.timestamp_opt(1793710000, 0).unwrap();
    let token = codec
        .seal(
            &binding(),
            PartyWindow {
                upper: now,
                after: None,
            },
            now,
        )
        .unwrap();
    for value in [
        "".to_string(),
        "a".to_string(),
        format!("{token}="),
        token[..token.len() - 1].to_string(),
        "x".repeat(1025),
    ] {
        assert!(codec.open(&binding(), &value, now).is_err());
    }
    let mut tampered = token.into_bytes();
    tampered[20] = if tampered[20] == b'A' { b'B' } else { b'A' };
    assert!(codec
        .open(&binding(), std::str::from_utf8(&tampered).unwrap(), now)
        .is_err());
}

#[test]
fn future_upper_fails_and_equal_start_bounds_are_accepted_for_query_layer_rejection() {
    let codec = PartyCursorCodec::new(&[42; 32]).unwrap();
    let now = Utc.timestamp_opt(1793710000, 0).unwrap();
    assert!(codec
        .seal(
            &binding(),
            PartyWindow {
                upper: now + Duration::seconds(1),
                after: None,
            },
            now,
        )
        .is_err());
    assert!(codec
        .seal(
            &PartyCursorBinding {
                starts_before: Some(now),
                starts_after: Some(now),
                ..binding()
            },
            PartyWindow {
                upper: now,
                after: None,
            },
            now,
        )
        .is_ok());
}
