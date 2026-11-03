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
