use chrono::{DateTime, Duration, Timelike, Utc};
use concert_domain::community::giveaway::*;
use uuid::Uuid;
fn now() -> DateTime<Utc> {
    "2030-01-01T00:00:00Z".parse().unwrap()
}
fn gift() -> Giveaway {
    let at = now();
    let place = Uuid::new_v4();
    let mut g = Giveaway::create(
        Uuid::new_v4(),
        Uuid::new_v4(),
        Uuid::new_v4(),
        GiftFields {
            title: "Gift".into(),
            description: String::new(),
            image_media_id: None,
            quantity_total: 1,
            pickup_starts_at: at + Duration::days(2),
            pickup_ends_at: at + Duration::days(2) + Duration::hours(1),
            public_place_id: place,
        },
        60,
        at,
    )
    .unwrap();
    let m = PickupMeeting {
        place_id: place,
        map_revision: 1,
        place_revision: 1,
        instructions: "Reviewed test instructions".into(),
        identification: "Test only".into(),
        expires_at: g.expires_at(),
    };
    g.change(
        g.owner,
        &GiftAction::SetMeeting {
            expected: 1,
            meeting: m,
            physical_acknowledged: true,
        },
        Uuid::new_v4(),
        at,
    )
    .unwrap();
    g.change(
        g.owner,
        &GiftAction::Update {
            expected: 2,
            fields: g.fields.clone(),
            state: GiftState::Open,
        },
        Uuid::new_v4(),
        at,
    )
    .unwrap();
    g
}
fn reserve(g: &mut Giveaway, user: Uuid) -> Uuid {
    let id = Uuid::new_v4();
    g.change(
        user,
        &GiftAction::Reserve {
            giveaway_revision: g.revision,
        },
        id,
        now(),
    )
    .unwrap();
    id
}
#[test]
fn one_unit_stock_duplicate_and_window_based_expiry() {
    let mut g = gift();
    let u = Uuid::new_v4();
    let id = reserve(&mut g, u);
    assert_eq!(g.reserved_unclaimed, 1);
    assert_eq!(g.available(), 0);
    let r = &g.reservations[0];
    assert_eq!(r.expires_at, g.expires_at());
    assert!(r.expires_at > now() + Duration::days(1));
    assert_eq!(r.state, ReservationState::Held);
    assert_eq!(
        g.change(
            u,
            &GiftAction::Reserve {
                giveaway_revision: g.revision
            },
            Uuid::new_v4(),
            now()
        ),
        Err(GiftError::DuplicateReservation)
    );
    assert_eq!(
        g.change(
            Uuid::new_v4(),
            &GiftAction::Reserve {
                giveaway_revision: g.revision
            },
            Uuid::new_v4(),
            now()
        ),
        Err(GiftError::SoldOut)
    );
    assert_eq!(g.reservations[0].id, id);
}
#[test]
fn expiry_and_cancel_release_once_at_exclusive_boundary() {
    let mut g = gift();
    let u = Uuid::new_v4();
    let id = reserve(&mut g, u);
    let expiry = g.expires_at();
    assert_eq!(g.advance_time(expiry, 100).unwrap(), 1);
    assert_eq!(g.available(), 1);
    assert_eq!(g.reservations[0].state, ReservationState::Expired);
    assert_eq!(g.advance_time(expiry, 100).unwrap(), 0);
    assert_eq!(
        g.change(
            u,
            &GiftAction::Cancel {
                reservation: id,
                expected: 2
            },
            Uuid::new_v4(),
            expiry
        ),
        Err(GiftError::InvalidTransition)
    );
}
#[test]
fn ready_code_expiry_and_one_time_claim() {
    let mut g = gift();
    let user = Uuid::new_v4();
    let id = reserve(&mut g, user);
    let start = g.fields.pickup_starts_at;
    assert_eq!(g.advance_time(start, 100).unwrap(), 1);
    assert_eq!(g.reservations[0].revision, 2);
    g.change(
        user,
        &GiftAction::IssueCode {
            reservation: id,
            expected: 2,
        },
        Uuid::new_v4(),
        start,
    )
    .unwrap();
    let expiry = g.reservations[0].code_expires_at.unwrap();
    assert_eq!(expiry, start + Duration::seconds(300));
    assert_eq!(
        g.change(
            g.owner,
            &GiftAction::Claim {
                reservation: id,
                expected: 3
            },
            Uuid::new_v4(),
            expiry
        ),
        Err(GiftError::CodeInvalid)
    );
    let receipt = Uuid::new_v4();
    let _first = g
        .change(
            g.owner,
            &GiftAction::Claim {
                reservation: id,
                expected: 3,
            },
            receipt,
            start,
        )
        .unwrap();
    let duplicate = g
        .change(
            g.owner,
            &GiftAction::Claim {
                reservation: id,
                expected: 3,
            },
            Uuid::new_v4(),
            start,
        );
    // A distinct command key cannot claim an already-consumed code: the frozen
    // contract refuses it instead of minting a second receipt.
    assert_eq!(duplicate, Err(GiftError::CodeInvalid));
    assert_eq!(g.quantity_claimed, 1);
    assert_eq!(g.reserved_unclaimed, 0);
    assert_eq!(g.available(), 0);
    assert_eq!(g.reservations[0].claim_receipt_id, Some(receipt));
    assert_eq!(g.advance_time(g.expires_at(), 100).unwrap(), 0);
}
#[test]
fn pause_honors_existing_holds_cancel_clears_codes() {
    let mut g = gift();
    let u = Uuid::new_v4();
    let id = reserve(&mut g, u);
    g.change(
        g.owner,
        &GiftAction::Update {
            expected: g.revision,
            fields: g.fields.clone(),
            state: GiftState::Paused,
        },
        Uuid::new_v4(),
        now(),
    )
    .unwrap();
    let start = g.fields.pickup_starts_at;
    g.advance_time(start, 100).unwrap();
    g.change(
        u,
        &GiftAction::IssueCode {
            reservation: id,
            expected: 2,
        },
        Uuid::new_v4(),
        start,
    )
    .unwrap();
    g.change(
        g.owner,
        &GiftAction::Update {
            expected: g.revision,
            fields: g.fields.clone(),
            state: GiftState::Cancelled,
        },
        Uuid::new_v4(),
        start,
    )
    .unwrap();
    assert_eq!(g.reserved_unclaimed, 0);
    assert_eq!(g.reservations[0].state, ReservationState::Cancelled);
    assert!(g.reservations[0].code_expires_at.is_none());
    assert!(g.meeting.is_none());
}
#[test]
fn quantity_window_owner_and_meeting_are_enforced() {
    let mut g = gift();
    let u = Uuid::new_v4();
    reserve(&mut g, u);
    let mut f = g.fields.clone();
    f.pickup_ends_at += Duration::seconds(1);
    assert_eq!(
        g.change(
            g.owner,
            &GiftAction::Update {
                expected: g.revision,
                fields: f,
                state: GiftState::Open
            },
            Uuid::new_v4(),
            now()
        ),
        Err(GiftError::InvalidTransition)
    );
    assert_eq!(
        g.change(
            u,
            &GiftAction::Update {
                expected: g.revision,
                fields: g.fields.clone(),
                state: GiftState::Paused
            },
            Uuid::new_v4(),
            now()
        ),
        Err(GiftError::Denied)
    );
    assert_eq!(
        g.change(
            g.owner,
            &GiftAction::Update {
                expected: g.revision,
                fields: g.fields.clone(),
                state: GiftState::Closed
            },
            Uuid::new_v4(),
            now()
        ),
        Err(GiftError::InvalidTransition)
    );
}

#[test]
fn admission_ends_before_existing_reservation_grace() {
    let mut g = gift();
    g.fields.quantity_total = 2;
    let id = reserve(&mut g, Uuid::new_v4());
    let end = g.fields.pickup_ends_at;
    g.advance_time(end, 100).unwrap();
    assert_eq!(g.reservations[0].id, id);
    assert_eq!(g.reservations[0].state, ReservationState::Ready);
    assert_eq!(g.reserved_unclaimed, 1);
    for at in [end, end + Duration::seconds(30)] {
        assert_eq!(
            g.change(
                Uuid::new_v4(),
                &GiftAction::Reserve {
                    giveaway_revision: g.revision
                },
                Uuid::new_v4(),
                at
            ),
            Err(GiftError::Expired)
        );
    }
    assert_eq!(g.advance_time(end + Duration::seconds(60), 100).unwrap(), 1);
}

#[test]
fn code_expiry_capped_by_fractional_reservation_uses_database_precision() {
    let mut g = gift();
    let start = g.fields.pickup_starts_at;
    g.fields.pickup_ends_at = (start + Duration::seconds(30))
        .with_nanosecond(123456789)
        .unwrap();
    let user = Uuid::new_v4();
    let id = reserve(&mut g, user);
    g.advance_time(start, 100).unwrap();
    g.change(
        user,
        &GiftAction::IssueCode {
            reservation: id,
            expected: 2,
        },
        Uuid::new_v4(),
        start,
    )
    .unwrap();
    let expires = g.reservations[0].code_expires_at.unwrap();
    assert_eq!(expires.timestamp_subsec_nanos(), 123456000);
    assert!(expires <= g.reservations[0].expires_at);
}
