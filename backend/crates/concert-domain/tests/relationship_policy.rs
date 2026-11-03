use chrono::{Duration, Utc};
use concert_domain::{relationship::*, AuthUserId, EventId, EventPhase, Revision};
use uuid::Uuid;
fn snapshot() -> PolicySnapshot {
    let now = Utc::now();
    let query = PolicyQuery {
        event: EventId::from_uuid(Uuid::from_u128(1)),
        actor: AuthUserId::from_uuid(Uuid::from_u128(2)),
        subject: AuthUserId::from_uuid(Uuid::from_u128(3)),
        field: Field::PublicProfile,
    };
    PolicySnapshot {
        query,
        phase: EventPhase::Live,
        event_revision: Revision::initial(),
        relationship_revision: Revision::initial(),
        actor_member: true,
        subject_member: true,
        blocked: false,
        now,
        authority: FreshAuthority {
            query,
            checked_at: now,
            valid_until: now + Duration::seconds(1),
            revision: Revision::initial(),
            actor_allowed: true,
            subject_allowed: true,
            inherited_block: false,
        },
    }
}
#[test]
fn deny_matrix_and_private_fields() {
    for i in 0..11 {
        let mut s = snapshot();
        match i {
            0 => s.actor_member = false,
            1 => s.subject_member = false,
            2 => s.blocked = true,
            3 => s.authority.inherited_block = true,
            4 => s.authority.actor_allowed = false,
            5 => s.authority.subject_allowed = false,
            6 => s.phase = EventPhase::Draft,
            7 => s.phase = EventPhase::Archived,
            8 => {
                s.query.field = Field::PrivateHandle;
                s.authority.query = s.query
            }
            9 => {
                s.phase = EventPhase::ReadOnly;
                s.query.field = Field::Interaction;
                s.authority.query = s.query
            }
            _ => {
                s.query.field = Field::SafetySelf;
                s.authority.query = s.query
            }
        }
        assert_eq!(decide(s), Err(RelationshipError::Denied), "case {i}");
    }
}
#[test]
fn unknown_stale_misbound_and_expired_authority_fail_closed() {
    for i in 0..4 {
        let mut s = snapshot();
        match i {
            0 => s.authority.valid_until = s.now,
            1 => s.authority.checked_at = s.now - Duration::milliseconds(1),
            2 => s.authority.query.subject = s.query.actor,
            _ => s.authority.query.event = EventId::from_uuid(Uuid::new_v4()),
        };
        assert_eq!(decide(s), Err(RelationshipError::Unavailable));
    }
}
#[test]
fn revisions_and_expiry_invalidate_projection() {
    let s = snapshot();
    let d = decide(s).unwrap();
    assert!(d.ensure_current(s.query, d.stamp(), s.now).is_ok());
    let mut stamp = d.stamp();
    stamp.relationship_revision = stamp.relationship_revision.next().unwrap();
    assert_eq!(
        d.ensure_current(s.query, stamp, s.now),
        Err(RelationshipError::Stale)
    );
    stamp = d.stamp();
    stamp.authority_revision = stamp.authority_revision.next().unwrap();
    assert!(d.ensure_current(s.query, stamp, s.now).is_err());
    stamp = d.stamp();
    stamp.event_revision = stamp.event_revision.next().unwrap();
    assert!(d.ensure_current(s.query, stamp, s.now).is_err());
    assert!(d
        .ensure_current(s.query, d.stamp(), d.valid_until())
        .is_err());
}
#[test]
fn own_safety_and_readonly_public_projection() {
    let mut s = snapshot();
    s.phase = EventPhase::ReadOnly;
    assert!(decide(s).is_ok());
    s.query.subject = s.query.actor;
    s.query.field = Field::SafetySelf;
    s.authority.query = s.query;
    assert!(decide(s).is_ok());
}

#[test]
fn platform_normalization_and_invalid_inputs() {
    use concert_domain::relationship::social::*;
    assert_eq!(
        PrivateHandle::normalize(Platform::Instagram, "Some.Name")
            .unwrap()
            .expose_for_reveal(),
        "some.name"
    );
    for bad in [
        "", "@name", ".name", "name.", "a..b", "a b", "a/b", "한글", "a\nb",
    ] {
        assert!(
            PrivateHandle::normalize(Platform::Instagram, bad).is_err(),
            "{bad}"
        );
    }
    assert!(PrivateHandle::normalize(Platform::X, "dot.name").is_err());
    assert!(PrivateHandle::normalize(Platform::X, "sixteenlettersxx").is_err());
    assert!(!format!(
        "{:?}",
        PrivateHandle::normalize(Platform::Instagram, "private.name").unwrap()
    )
    .contains("private.name"));
}

#[test]
fn cross_user_safety_in_readonly_still_checks_blocks() {
    let mut s = snapshot();
    s.phase = EventPhase::ReadOnly;
    s.query.field = Field::SafetyInteraction;
    s.authority.query = s.query;
    assert!(decide(s).is_ok());
    s.blocked = true;
    assert_eq!(decide(s), Err(RelationshipError::Denied));
}
