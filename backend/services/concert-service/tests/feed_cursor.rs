use chrono::{Duration, TimeZone, Utc};
use concert_domain::feed::*;
use concert_service::feed::cursor::*;
use uuid::Uuid;
fn b() -> CursorBinding {
    CursorBinding {
        actor: Uuid::from_u128(1),
        event: Uuid::from_u128(2),
        operation: "listConcertFeed".into(),
        collection: "feed".into(),
        filter: Filter::All,
        before: None,
        limit: 20,
    }
}
#[test]
fn aead_cursor_binds_context_filter_collection_and_window() {
    let codec = CursorCodec::new(&[91; 32]).unwrap();
    let now = Utc.timestamp_opt(1793710000, 0).unwrap();
    let w = Window {
        upper: now,
        after: Some(Position {
            created_at: now - Duration::seconds(1),
            id: Uuid::from_u128(9),
        }),
    };
    let token = codec.seal(&b(), w, now).unwrap();
    assert!(token
        .bytes()
        .all(|v| v.is_ascii_alphanumeric() || v == b'-' || v == b'_'));
    assert!(!token.contains(&b().actor.to_string()));
    assert_eq!(codec.open(&b(), &token, now).unwrap(), w);
    for changed in [
        CursorBinding {
            actor: Uuid::from_u128(3),
            ..b()
        },
        CursorBinding {
            event: Uuid::from_u128(3),
            ..b()
        },
        CursorBinding {
            filter: Filter::PartyLink,
            ..b()
        },
        CursorBinding {
            collection: "comments-other".into(),
            ..b()
        },
        CursorBinding {
            before: Some(now),
            ..b()
        },
        CursorBinding {
            operation: "listConcertComments".into(),
            ..b()
        },
        CursorBinding { limit: 10, ..b() },
    ] {
        assert!(codec.open(&changed, &token, now).is_err());
    }
    assert!(codec
        .open(&b(), &token, now + Duration::minutes(15))
        .is_err());
    assert!(CursorCodec::new(&[92; 32])
        .unwrap()
        .open(&b(), &token, now)
        .is_err());
}
#[test]
fn cursor_tamper_truncation_padding_and_noncanonical_input_fail() {
    let c = CursorCodec::new(&[11; 32]).unwrap();
    let n = Utc.timestamp_opt(1793710000, 0).unwrap();
    let t = c
        .seal(
            &b(),
            Window {
                upper: n,
                after: None,
            },
            n,
        )
        .unwrap();
    for bad in [
        "".into(),
        "a".into(),
        format!("{t}="),
        t[..t.len() - 1].into(),
        "x".repeat(1025),
    ] {
        assert!(c.open(&b(), &bad, n).is_err());
    }
    let mut tampered = t.into_bytes();
    tampered[20] = if tampered[20] == b'A' { b'B' } else { b'A' };
    assert!(c
        .open(&b(), std::str::from_utf8(&tampered).unwrap(), n)
        .is_err());
}
