use concert_application::{commands::*, community::*};
use concert_domain::community::party::PartyAction;
use uuid::Uuid;
fn command() -> PartyCommand {
    PartyCommand {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
        key: Uuid::new_v4(),
        party: Uuid::new_v4(),
        action: PartyAction::Apply {
            party_revision: 1,
            note: "Private note".into(),
        },
    }
}
#[test]
fn restart_id_is_stable_and_request_is_bound_to_exact_body_and_path() {
    let mut c = command();
    let id = c.created_id();
    let keys = FingerprintKeys::new(SigningKey::new("test", &[91; 32]).unwrap(), vec![]).unwrap();
    let mac = keys.sign(&c.request().unwrap()).unwrap();
    assert_eq!(id, c.created_id());
    assert!(keys.verify(&c.request().unwrap(), &mac).unwrap());
    c.party = Uuid::new_v4();
    assert!(!keys.verify(&c.request().unwrap(), &mac).unwrap());
    c.action = PartyAction::Apply {
        party_revision: 1,
        note: "different".into(),
    };
    assert!(!keys.verify(&c.request().unwrap(), &mac).unwrap());
}
#[test]
fn delete_revision_uses_query_and_never_json_body() {
    let mut c = command();
    c.action = PartyAction::CancelApplication {
        application: Uuid::new_v4(),
        expected: 2,
    };
    assert!(c.request().is_ok());
    assert!(c.safety());
    c.action = PartyAction::Remove {
        user: Uuid::new_v4(),
        expected: 1,
        party_revision: 3,
    };
    assert!(c.request().is_ok());
}

#[test]
fn pickup_hash_binds_reservation_generation_and_rotation() {
    use concert_application::community::giveaway::*;
    let event = Uuid::new_v4();
    let reservation = Uuid::new_v4();
    let expiry = "2030-01-01T00:00:00Z".parse().unwrap();
    let code = PickupCode::parse("012345".into()).unwrap();
    let keys = PickupKeys::new(PickupKey::new("old", &[1; 32]).unwrap(), vec![]).unwrap();
    let (id, hash) = keys.sign(event, reservation, 1, expiry, &code).unwrap();
    let rotated = PickupKeys::new(
        PickupKey::new("new", &[2; 32]).unwrap(),
        vec![PickupKey::new("old", &[1; 32]).unwrap()],
    )
    .unwrap();
    assert!(rotated
        .verify(&id, &hash, event, reservation, 1, expiry, &code)
        .unwrap());
    assert!(!rotated
        .verify(&id, &hash, event, Uuid::new_v4(), 1, expiry, &code)
        .unwrap());
    assert!(!rotated
        .verify(&id, &hash, event, reservation, 2, expiry, &code)
        .unwrap());
    let missing = PickupKeys::new(PickupKey::new("new", &[2; 32]).unwrap(), vec![]).unwrap();
    assert_eq!(
        missing.verify(&id, &hash, event, reservation, 1, expiry, &code),
        Err(CommandError::Unavailable)
    );
}
