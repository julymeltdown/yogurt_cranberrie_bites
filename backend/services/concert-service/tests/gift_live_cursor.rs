#[path = "../src/gift_live/cursor.rs"]
mod cursor;
use chrono::{Duration, Utc};
use cursor::*;
use std::collections::BTreeMap;
use uuid::Uuid;

fn fixture() -> (CursorCodec, CursorBinding, Position) {
    (
        CursorCodec::new(&[37; 32]).unwrap(),
        CursorBinding {
            actor: Uuid::new_v4(),
            event: Uuid::new_v4(),
            collection: Collection::Public,
            giveaway: None,
            filters: BTreeMap::from([("state".into(), "OPEN".into())]),
            limit: 20,
        },
        Position::Gift {
            created_at: Utc::now() - Duration::seconds(1),
            id: Uuid::new_v4(),
        },
    )
}
#[test]
fn opaque_nonce_unique_roundtrip_and_exact_expiry() {
    let (codec, binding, p) = fixture();
    let now = Utc::now();
    let a = codec.seal(&binding, p, now).unwrap();
    let b = codec.seal(&binding, p, now).unwrap();
    assert_ne!(a, b);
    assert!(
        a.len() <= 1024
            && a.bytes()
                .all(|v| v.is_ascii_alphanumeric() || v == b'_' || v == b'-')
    );
    assert_eq!(codec.open(&binding, &a, now).unwrap(), p);
    assert!(codec
        .open(&binding, &a, now - Duration::microseconds(1))
        .is_err());
    assert!(codec
        .open(&binding, &a, now + Duration::minutes(15))
        .is_err());
    assert_eq!(
        codec
            .open(
                &binding,
                &a,
                now + Duration::minutes(15) - Duration::microseconds(1)
            )
            .unwrap(),
        p
    );
}
#[test]
fn identity_event_collection_filters_limit_and_key_are_authenticated() {
    let (codec, binding, p) = fixture();
    let now = Utc::now();
    let token = codec.seal(&binding, p, now).unwrap();
    let mut variants = vec![];
    let mut b = binding.clone();
    b.actor = Uuid::new_v4();
    variants.push(b);
    let mut b = binding.clone();
    b.event = Uuid::new_v4();
    variants.push(b);
    let mut b = binding.clone();
    b.limit = 21;
    variants.push(b);
    let mut b = binding.clone();
    b.filters.insert("state".into(), "CLOSED".into());
    variants.push(b);
    let mut b = binding.clone();
    b.collection = Collection::Giving;
    b.filters.clear();
    variants.push(b);
    for b in variants {
        assert!(codec.open(&b, &token, now).is_err());
    }
    assert!(CursorCodec::new(&[38; 32])
        .unwrap()
        .open(&binding, &token, now)
        .is_err());
}
#[test]
fn corruption_noncanonical_and_unbounded_tokens_are_rejected() {
    let (codec, b, p) = fixture();
    let now = Utc::now();
    let token = codec.seal(&b, p, now).unwrap();
    let mut corrupt = token.as_bytes().to_vec();
    corrupt[20] = if corrupt[20] == b'A' { b'B' } else { b'A' };
    for token in [
        String::from_utf8(corrupt).unwrap(),
        format!("{token}="),
        "A".into(),
        "_".repeat(1025),
        "AB".into(),
        "".into(),
    ] {
        assert!(codec.open(&b, &token, now).is_err());
    }
}
#[test]
fn management_target_and_position_types_cannot_be_substituted() {
    let (codec, mut b, p) = fixture();
    let now = Utc::now();
    b.collection = Collection::Management;
    b.filters.clear();
    b.giveaway = Some(Uuid::new_v4());
    assert!(codec.seal(&b, p, now).is_err());
    let reservation = Position::Reservation { id: Uuid::new_v4() };
    let token = codec.seal(&b, reservation, now).unwrap();
    assert_eq!(codec.open(&b, &token, now).unwrap(), reservation);
    b.giveaway = Some(Uuid::new_v4());
    assert!(codec.open(&b, &token, now).is_err());
    b.giveaway = None;
    assert!(codec.seal(&b, reservation, now).is_err());
    b.collection = Collection::Receiving;
    assert!(codec.seal(&b, reservation, now).is_ok());
    b.filters.insert("cursor".into(), "nested".into());
    assert!(codec.seal(&b, reservation, now).is_err());
}
