use chrono::{Duration, Utc};
use concert_domain::community::party::*;
use uuid::Uuid;
fn fields() -> PartyFields {
    let now = Utc::now();
    PartyFields {
        title: "Entry friends".into(),
        description: String::new(),
        ticket_section: None,
        capacity: 2,
        meeting_starts_at: now + Duration::hours(1),
        meeting_ends_at: now + Duration::hours(2),
        requirements: String::new(),
        public_place_id: Uuid::new_v4(),
    }
}
fn party() -> Party {
    Party::create(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        fields(),
        Utc::now(),
    )
    .unwrap()
}
fn apply(p: &mut Party, u: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    p.change(
        u,
        &PartyAction::Apply {
            party_revision: p.revision,
            note: "Private note".into(),
        },
        id,
        Utc::now(),
    )
    .unwrap();
    id
}
fn approve(p: &mut Party, a: Uuid) {
    p.change(
        p.owner,
        &PartyAction::Decide {
            application: a,
            expected: 1,
            party_revision: p.revision,
            approve: true,
        },
        Uuid::new_v4(),
        Utc::now(),
    )
    .unwrap();
}
#[test]
fn last_slot_and_duplicate_application_are_guarded() {
    let mut p = party();
    let a = Uuid::new_v4();
    let aid = apply(&mut p, a);
    let bid = apply(&mut p, Uuid::new_v4());
    let before = p.clone();
    assert_eq!(
        p.change(
            a,
            &PartyAction::Apply {
                party_revision: 1,
                note: String::new()
            },
            Uuid::new_v4(),
            Utc::now()
        ),
        Err(PartyError::DuplicateApplication)
    );
    assert_eq!(p, before);
    approve(&mut p, aid);
    let before = p.clone();
    assert_eq!(
        p.change(
            p.owner,
            &PartyAction::Decide {
                application: bid,
                expected: 1,
                party_revision: 2,
                approve: true
            },
            Uuid::new_v4(),
            Utc::now()
        ),
        Err(PartyError::Full)
    );
    assert_eq!(p, before);
    assert_eq!(p.member_count(), 2);
}
#[test]
fn removal_revokes_audience_and_capacity_owner_never_orphaned() {
    let mut p = party();
    let u = Uuid::new_v4();
    let a = apply(&mut p, u);
    approve(&mut p, a);
    assert!(p.member_access(u));
    assert_eq!(
        p.change(
            p.owner,
            &PartyAction::Remove {
                user: p.owner,
                expected: 1,
                party_revision: 2
            },
            Uuid::new_v4(),
            Utc::now()
        ),
        Err(PartyError::InvalidTransition)
    );
    p.change(
        u,
        &PartyAction::Remove {
            user: u,
            expected: 1,
            party_revision: 2,
        },
        Uuid::new_v4(),
        Utc::now(),
    )
    .unwrap();
    assert!(!p.member_access(u));
    assert_eq!(p.audience_revision, 2);
    assert_eq!(p.member_count(), 1);
    assert_eq!(p.applications[0].state, ApplicationState::Approved);
}
#[test]
fn cancellation_is_terminal_and_cancels_pending() {
    let mut p = party();
    let a = apply(&mut p, Uuid::new_v4());
    p.change(
        p.owner,
        &PartyAction::Cancel { expected: 1 },
        Uuid::new_v4(),
        Utc::now(),
    )
    .unwrap();
    assert_eq!(
        p.applications.iter().find(|x| x.id == a).unwrap().state,
        ApplicationState::Cancelled
    );
    assert!(!p.member_access(p.owner));
    assert_eq!(
        p.change(
            p.owner,
            &PartyAction::Update {
                expected: 2,
                fields: fields(),
                state: PartyState::Completed
            },
            Uuid::new_v4(),
            Utc::now()
        ),
        Err(PartyError::InvalidTransition)
    );
}
#[test]
fn transfer_preserves_one_owner_and_meeting_move_fences_guidance() {
    let mut p = party();
    let u = Uuid::new_v4();
    let a = apply(&mut p, u);
    approve(&mut p, a);
    let old = p.owner;
    p.change(
        old,
        &PartyAction::Transfer {
            expected: 2,
            user: u,
        },
        Uuid::new_v4(),
        Utc::now(),
    )
    .unwrap();
    assert_eq!(p.owner, u);
    assert_eq!(p.members.iter().filter(|m| m.owner).count(), 1);
    let mut f = p.fields.clone();
    f.public_place_id = Uuid::new_v4();
    p.change(
        u,
        &PartyAction::Update {
            expected: 3,
            fields: f,
            state: PartyState::Closed,
        },
        Uuid::new_v4(),
        Utc::now(),
    )
    .unwrap();
    assert_eq!(p.meeting_revision, 2);
    assert_eq!(p.audience_revision, 3);
    assert!(p.member_access(old));
    assert_eq!(
        p.change(
            old,
            &PartyAction::Cancel { expected: 4 },
            Uuid::new_v4(),
            Utc::now()
        ),
        Err(PartyError::Denied)
    );
}
#[test]
fn invalid_fields_revision_and_window_leave_no_effect() {
    let mut p = party();
    let before = p.clone();
    let mut f = p.fields.clone();
    f.capacity = 1;
    assert_eq!(
        p.change(
            p.owner,
            &PartyAction::Update {
                expected: 1,
                fields: f,
                state: PartyState::Open
            },
            Uuid::new_v4(),
            Utc::now()
        ),
        Err(PartyError::InvalidInput)
    );
    assert_eq!(p, before);
    let mut f = p.fields.clone();
    f.meeting_ends_at = f.meeting_starts_at;
    assert!(f.validate(Utc::now()).is_err());
    assert!(!p.can_read_application(Uuid::new_v4(), Uuid::new_v4()));
}
