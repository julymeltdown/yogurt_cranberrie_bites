//! BE27 pure command, serialization-boundary and cursor contracts.
use chrono::{Duration, Utc};
use concert_application::intro::{self, cursor, *};
use concert_domain::intro_policy::IntroState;
use concert_domain::{AuthUserId, EventId, Revision};
use uuid::Uuid;

fn draft(text: &str) -> IntroDraft {
    IntroDraft {
        text: text.into(),
        age_bounds: "null:null".into(),
        expires_at: None,
    }
}

#[test]
fn closed_draft_bounds_expiry_and_publication_rules() {
    let now = Utc::now();
    for (minimum, maximum) in [(None, Some(15)), (Some(14), None), (Some(13), Some(101))] {
        assert_eq!(
            DraftInput {
                text: "draft".into(),
                minimum_age: minimum,
                maximum_age_exclusive: maximum,
                expires_at: None,
            }
            .validate_at(now)
            .err(),
            Some(IntroError::Invalid)
        );
    }
    let bounded = DraftInput {
        text: "x".repeat(1000),
        minimum_age: Some(24),
        maximum_age_exclusive: Some(30),
        expires_at: Some(now + Duration::minutes(1)),
    }
    .validate_at(now)
    .unwrap();
    assert_eq!(bounded.age_bounds, "24:30");
    assert_eq!(format!("{bounded:?}"), "IntroDraft([REDACTED])");
    assert_eq!(
        DraftInput {
            text: "x".repeat(1001),
            minimum_age: None,
            maximum_age_exclusive: None,
            expires_at: None,
        }
        .validate_at(now)
        .err(),
        Some(IntroError::Invalid)
    );
    assert_eq!(
        DraftInput {
            text: "expires".into(),
            minimum_age: None,
            maximum_age_exclusive: None,
            expires_at: Some(now),
        }
        .validate_at(now)
        .err(),
        Some(IntroError::Invalid)
    );
    assert_eq!(
        DraftInput {
            text: "line\rbreak".into(),
            minimum_age: None,
            maximum_age_exclusive: None,
            expires_at: None,
        }
        .validate_at(now)
        .err(),
        Some(IntroError::Invalid)
    );

    let mut owner = OwnerIntro {
        intro_id: Uuid::new_v4(),
        event_id: Uuid::new_v4(),
        owner_id: Uuid::new_v4(),
        text: String::new(),
        age_bounds: "null:null".into(),
        expires_at: None,
        created_at: now,
        state: IntroState::Draft,
        revision: Revision::initial(),
    };
    assert!(owner.validate_at(now).is_ok());
    owner.state = IntroState::Published;
    assert_eq!(owner.validate_at(now), Err(IntroError::Invalid));
    owner.text = "line@example.com".into();
    assert_eq!(owner.validate_at(now), Err(IntroError::Invalid));
    owner.text = "hello".into();
    owner.age_bounds = "30:30".into();
    assert_eq!(owner.validate_at(now), Err(IntroError::Invalid));
    owner.age_bounds = "null:null".into();
    owner.expires_at = Some(now);
    assert_eq!(owner.validate_at(now), Err(IntroError::Invalid));
}

#[test]
fn command_identity_is_stable_but_actor_and_event_scoped() {
    let event = EventId::from_uuid(Uuid::new_v4());
    let other_event = EventId::from_uuid(Uuid::new_v4());
    let actor = AuthUserId::from_uuid(Uuid::new_v4());
    let other_actor = AuthUserId::from_uuid(Uuid::new_v4());
    let first = IntroCommand::set(actor, event, Uuid::new_v4(), 0, draft("")).unwrap();
    let same = IntroCommand::set(actor, event, Uuid::new_v4(), 0, draft("")).unwrap();
    let foreign_actor =
        IntroCommand::set(other_actor, event, Uuid::new_v4(), 0, draft("")).unwrap();
    let foreign_event =
        IntroCommand::set(actor, other_event, Uuid::new_v4(), 0, draft("")).unwrap();
    assert_eq!(first.intro(), same.intro());
    assert_ne!(first.intro(), foreign_actor.intro());
    assert_ne!(first.intro(), foreign_event.intro());
    assert_eq!(first.intro().get_version_num(), 5);
    assert_eq!(first.intro().get_variant(), uuid::Variant::RFC4122);
    assert_eq!(first.operation(), "setConcertIntro");
    assert_eq!(first.scope().unwrap().event, event.uuid());
    assert!(first.request().is_ok());
    assert_eq!(
        IntroCommand::set(actor, event, Uuid::new_v4(), -1, draft("")).err(),
        Some(IntroError::Invalid)
    );
}

#[test]
fn query_and_request_context_are_closed_and_bounded() {
    assert_eq!(
        intro::wire::Query::validate(Some(0), None, None).err(),
        Some(IntroError::Invalid)
    );
    assert_eq!(
        intro::wire::Query::validate(Some(101), None, None).err(),
        Some(IntroError::Invalid)
    );
    assert_eq!(
        intro::wire::Query::validate(Some(20), Some("x".repeat(1025)), None).err(),
        Some(IntroError::Invalid)
    );
    assert_eq!(
        intro::wire::Query::validate(None, Some(String::new()), Some(" ".into())).err(),
        Some(IntroError::Invalid)
    );
    let query =
        intro::wire::Query::validate(None, Some(String::new()), Some("  Alice  ".into())).unwrap();
    assert_eq!(query.limit, 20);
    assert!(query.cursor.is_none());
    assert_eq!(query.q.as_deref(), Some("Alice"));

    let now = Utc::now();
    let valid = IntroReadContext {
        actor: AuthUserId::from_uuid(Uuid::new_v4()),
        event: EventId::from_uuid(Uuid::new_v4()),
        session: Uuid::new_v4(),
        request: Uuid::new_v4(),
        expires_at: now + Duration::seconds(1),
    };
    assert!(valid.check(now).is_ok());
    assert_eq!(valid.check(valid.expires_at), Err(IntroError::Denied));
    let mut nil = valid;
    nil.request = Uuid::nil();
    assert_eq!(nil.check(now), Err(IntroError::Denied));
}

#[test]
fn authenticated_cursor_binds_request_position_and_lifetime() {
    let now = Utc::now();
    let decode_at = now;
    let actor = Uuid::new_v4();
    let event = Uuid::new_v4();
    let binding = cursor::CursorBinding {
        actor,
        event,
        operation: "listConcertIntros",
        collection: "intros",
        query: Some(17),
    };
    let window = cursor::CursorWindow {
        upper: now,
        after: Some(Position {
            created_at: now - Duration::seconds(1),
            intro_id: Uuid::new_v4(),
        }),
    };
    let key = [91_u8; 32];
    let token = cursor::encode_authenticated(&key, &binding, window, now).unwrap();
    let mut probe = cursor::encode_authenticated(&key, &binding, window, now).unwrap();
    probe.push('x');
    assert_eq!(
        cursor::decode_authenticated(&key, &binding, &probe, decode_at),
        Err(IntroError::Invalid)
    );
    assert_eq!(
        cursor::decode_authenticated(&key, &binding, &token, decode_at),
        Ok(window)
    );
    assert_eq!(
        cursor::decode_authenticated(
            &key,
            &binding,
            &token,
            now + Duration::minutes(15) + Duration::seconds(1)
        ),
        Err(IntroError::Invalid)
    );
    assert_eq!(
        cursor::decode_authenticated(&[92_u8; 32], &binding, &token, now + Duration::minutes(1),),
        Err(IntroError::Invalid)
    );
    for changed in [
        cursor::CursorBinding {
            actor: Uuid::new_v4(),
            ..binding
        },
        cursor::CursorBinding {
            event: Uuid::new_v4(),
            ..binding
        },
        cursor::CursorBinding {
            query: Some(18),
            ..binding
        },
    ] {
        assert_eq!(
            cursor::decode_authenticated(&key, &changed, &token, now),
            Err(IntroError::Invalid)
        );
    }
    let mut tampered = token.into_bytes();
    tampered[0] = if tampered[0] == b'0' { b'1' } else { b'0' };
    assert_eq!(
        cursor::decode_authenticated(&key, &binding, std::str::from_utf8(&tampered).unwrap(), now,),
        Err(IntroError::Invalid)
    );
}
