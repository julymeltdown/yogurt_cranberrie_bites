//! T-306: the cursor codec in isolation.
//!
//! The page tests exercise the codec through a real database. This test does not
//! touch a database at all: it seals a window and opens it again for the same
//! binding and for a different one, so a defect in the token format, the MAC or
//! the binding tag is visible without any store in the way.
use chrono::{Duration, TimeZone, Utc};
use concert_application::intro::cursor::CursorWindow;
use concert_application::intro::Position;
use concert_service::intro_posts::repository::{
    binding, IntroCursorCodec, IntroPostQuery, OPERATION,
};
use uuid::Uuid;

fn now() -> chrono::DateTime<Utc> {
    Utc.with_ymd_and_hms(2026, 11, 9, 5, 0, 0).unwrap()
}

fn query() -> IntroPostQuery {
    IntroPostQuery::new(Some(20), None).expect("a valid query")
}

fn window() -> CursorWindow {
    CursorWindow {
        upper: now(),
        after: Some(Position {
            created_at: now() - Duration::seconds(1),
            intro_id: Uuid::new_v4(),
        }),
    }
}

#[test]
fn a_sealed_window_opens_for_the_same_viewer() {
    use concert_application::intro::cursor::CursorCodec;
    let codec = IntroCursorCodec::new([0x5a; 32]);
    let viewer = Uuid::new_v4();
    let binding = binding(viewer, &query());
    let window = window();
    let token = codec
        .seal(&binding, window, now())
        .expect("a valid window seals");
    let opened = codec
        .open(&binding, &token, now())
        .expect("the same binding opens its own token");
    assert_eq!(opened.upper, window.upper);
    assert_eq!(opened.after, window.after);
}

#[test]
fn a_sealed_window_is_refused_for_another_viewer() {
    use concert_application::intro::cursor::CursorCodec;
    let codec = IntroCursorCodec::new([0x5a; 32]);
    let viewer_a = Uuid::new_v4();
    let viewer_b = Uuid::new_v4();
    let token = codec
        .seal(&binding(viewer_a, &query()), window(), now())
        .expect("a valid window seals");
    assert!(
        codec
            .open(&binding(viewer_b, &query()), &token, now())
            .is_err(),
        "a cursor is bound to its viewer"
    );
}

#[test]
fn the_binding_is_viewer_scoped_and_matches_the_operation() {
    let viewer = Uuid::new_v4();
    let value = binding(viewer, &query());
    assert_eq!(value.actor, viewer);
    assert_eq!(value.event, viewer, "the profile product has no event");
    assert_eq!(value.operation, OPERATION);
    assert_eq!(value.collection, "intro-posts");
    assert_eq!(value.query, Some(0));
    // Two different viewers never produce the same binding.
    let other = binding(Uuid::new_v4(), &query());
    assert_ne!(value, other);
}

#[test]
fn a_tampered_token_is_refused() {
    use concert_application::intro::cursor::CursorCodec;
    let codec = IntroCursorCodec::new([0x5a; 32]);
    let viewer = Uuid::new_v4();
    let binding = binding(viewer, &query());
    let token = codec.seal(&binding, window(), now()).expect("seals");
    let mut bytes = token.into_bytes();
    let last = bytes.len() - 1;
    bytes[last] = if bytes[last] == b'0' { b'1' } else { b'0' };
    let tampered = String::from_utf8(bytes).expect("ASCII");
    assert!(codec.open(&binding, &tampered, now()).is_err());
}

#[test]
fn a_token_signed_with_another_key_is_refused() {
    use concert_application::intro::cursor::CursorCodec;
    let viewer = Uuid::new_v4();
    let binding = binding(viewer, &query());
    let token = IntroCursorCodec::new([0x5a; 32])
        .seal(&binding, window(), now())
        .expect("seals");
    assert!(IntroCursorCodec::new([0x11; 32])
        .open(&binding, &token, now())
        .is_err());
}

#[test]
fn an_expired_token_is_refused() {
    use concert_application::intro::cursor::CursorCodec;
    let codec = IntroCursorCodec::new([0x5a; 32]);
    let viewer = Uuid::new_v4();
    let binding = binding(viewer, &query());
    let token = codec.seal(&binding, window(), now()).expect("seals");
    let later = now() + Duration::minutes(20);
    assert!(codec.open(&binding, &token, later).is_err());
}

#[test]
fn a_window_whose_position_is_newer_than_its_fence_is_refused() {
    use concert_application::intro::cursor::CursorCodec;
    let codec = IntroCursorCodec::new([0x5a; 32]);
    let viewer = Uuid::new_v4();
    let binding = binding(viewer, &query());
    let bad = CursorWindow {
        upper: now(),
        after: Some(Position {
            created_at: now() + Duration::seconds(1),
            intro_id: Uuid::new_v4(),
        }),
    };
    // The seal refuses it, so a token with that shape cannot exist.
    assert!(codec.seal(&binding, bad, now()).is_err());
}
