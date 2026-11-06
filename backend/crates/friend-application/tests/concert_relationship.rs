use chrono::Utc;
use friend_application::concert_relationship::*;
use uuid::Uuid;
fn query() -> RelationshipRead {
    RelationshipRead::new(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        Purpose::Presence,
        Utc::now() + chrono::Duration::seconds(1),
    )
    .unwrap()
}
#[test]
fn exact_non_nil_bindings_and_purposes_required() {
    let q = query();
    for slot in 0..5 {
        let mut ids = [
            q.event_id(),
            q.actor_id(),
            q.subject_id(),
            q.session_family_id(),
            q.request_id(),
        ];
        ids[slot] = Uuid::nil();
        assert_eq!(
            RelationshipRead::new(
                ids[0],
                ids[1],
                ids[2],
                ids[3],
                ids[4],
                Purpose::Presence,
                q.expires_at()
            )
            .unwrap_err(),
            ReadError::InvalidInput
        );
    }
    for bad in ["", "UNKNOWN", "presence", "PRESENCE ", "private-canary"] {
        assert_eq!(Purpose::parse(bad).unwrap_err(), ReadError::InvalidInput);
    }
    for purpose in [
        Purpose::PublicProfile,
        Purpose::Presence,
        Purpose::Interaction,
        Purpose::SafetySelf,
        Purpose::SafetyInteraction,
        Purpose::PrivateHandle,
    ] {
        assert_eq!(Purpose::parse(purpose.as_str()).unwrap(), purpose);
    }
    assert!(RelationshipRead::new(
        q.event_id(),
        q.actor_id(),
        q.subject_id(),
        q.session_family_id(),
        q.request_id(),
        Purpose::SafetySelf,
        q.expires_at()
    )
    .is_err());
}
#[test]
fn original_expiry_and_budget_cannot_be_reset() {
    let q = query();
    assert!(q.check_at(q.expires_at()).is_err());
    assert!(q
        .check_at(q.expires_at() - chrono::Duration::seconds(3))
        .is_err());
    let cancel = ReadCancellation::default();
    let budget = ReadBudget::new(
        std::time::Instant::now() + std::time::Duration::from_secs(1),
        cancel.clone(),
    )
    .unwrap();
    cancel.cancel();
    assert_eq!(budget.check(), Err(ReadError::Cancelled));
    assert_eq!(budget.clone().remaining(), Err(ReadError::Cancelled));
    assert!(ReadBudget::new(
        std::time::Instant::now() + std::time::Duration::from_secs(3),
        ReadCancellation::default()
    )
    .is_err());
}

fn snapshot(q: RelationshipRead) -> CurrentRelationship {
    CurrentRelationship {
        query: q,
        observed_at: Utc::now(),
        valid_until: q.expires_at(),
        owner_epoch: Uuid::new_v4(),
        revision: 1,
        actor_blocks_subject: false,
        subject_blocks_actor: false,
        actor_deleted: false,
        subject_deleted: false,
        actor_follows_subject: false,
        subject_follows_actor: false,
    }
}
#[test]
fn foreign_query_epoch_revision_and_time_are_rejected() {
    let q = query();
    let now = Utc::now();
    let good = snapshot(q);
    assert!(good.validate(q, Utc::now()).is_ok());
    let cases = [
        CurrentRelationship {
            query: query(),
            ..good
        },
        CurrentRelationship {
            owner_epoch: Uuid::nil(),
            ..good
        },
        CurrentRelationship {
            revision: 0,
            ..good
        },
        CurrentRelationship {
            revision: MAX_REVISION + 1,
            ..good
        },
        CurrentRelationship {
            observed_at: now + chrono::Duration::seconds(1),
            ..good
        },
        CurrentRelationship {
            valid_until: now,
            ..good
        },
        CurrentRelationship {
            valid_until: q.expires_at() + chrono::Duration::microseconds(1),
            ..good
        },
    ];
    for value in cases {
        assert_eq!(
            value.validate(q, Utc::now()).unwrap_err(),
            ReadError::Unavailable
        );
    }
}
#[test]
fn diagnostics_contain_no_query_identity_or_owner_state() {
    let q = query();
    let value = snapshot(q);
    let out = format!("{q:?}{value:?}");
    for id in [
        q.actor_id(),
        q.subject_id(),
        q.event_id(),
        q.session_family_id(),
        q.request_id(),
        value.owner_epoch,
    ] {
        assert!(!out.contains(&id.to_string()));
    }
}
#[test]
fn mutual_follow_requires_both_live_directions() {
    let q = query();
    for (actor_follows_subject, subject_follows_actor) in
        [(true, true), (true, false), (false, true), (false, false)]
    {
        let s = CurrentRelationship {
            actor_follows_subject,
            subject_follows_actor,
            ..snapshot(q)
        };
        assert_eq!(
            s.mutual_follow(),
            actor_follows_subject && subject_follows_actor
        );
    }
}

#[test]
fn both_directions_and_tombstones_are_independent() {
    let q = query();
    for actor_blocks_subject in [true, false] {
        for subject_blocks_actor in [true, false] {
            for actor_deleted in [true, false] {
                for subject_deleted in [true, false] {
                    let s = CurrentRelationship {
                        actor_blocks_subject,
                        subject_blocks_actor,
                        actor_deleted,
                        subject_deleted,
                        ..snapshot(q)
                    };
                    assert_eq!(s.blocked(), actor_blocks_subject || subject_blocks_actor);
                    assert_eq!(s.subjects_present(), !actor_deleted && !subject_deleted);
                    // Independent of blocks: one-way follows are never mutual.
                    assert!(!s.mutual_follow());
                }
            }
        }
    }
}
