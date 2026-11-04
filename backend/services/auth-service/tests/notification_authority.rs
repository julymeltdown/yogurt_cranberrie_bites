//! BE33 producer separation and owner RPC capability regressions. Native proofs
//! are qualified separately; these pure checks cannot certify store mutation.
use auth_service::concert_authority::proto;
use platform::{actor_delegation::owner_binding::identity_digest, grpc_auth::Workload};
use uuid::Uuid;

#[test]
fn notification_wire_value_preserves_all_published_producers() {
    assert_eq!(proto::Producer::Unspecified as i32, 0);
    assert_eq!(proto::Producer::Concert as i32, 1);
    assert_eq!(proto::Producer::Post as i32, 2);
    assert_eq!(proto::Producer::Profile as i32, 3);
    assert_eq!(proto::Producer::Notification as i32, 4);
    assert_eq!(proto::Producer::Notification.as_str_name(), "NOTIFICATION");
    assert!(proto::Producer::try_from(5).is_err());
}

#[test]
fn notification_identity_cannot_masquerade_as_an_old_producer() {
    let digest = |producer, database, xid, effect: &[u8; 32]| {
        identity_digest(
            producer,
            "123",
            database,
            xid,
            7,
            100,
            effect,
            Uuid::from_u128(1),
            Uuid::from_u128(2).as_bytes(),
        )
    };
    let original = digest("NOTIFICATION", 1, "99", &[2; 32]).unwrap();
    for old in ["CONCERT", "POST", "PROFILE"] {
        assert_ne!(original, digest(old, 1, "99", &[2; 32]).unwrap());
    }
    assert_ne!(original, digest("NOTIFICATION", 2, "99", &[2; 32]).unwrap());
    assert_ne!(
        original,
        digest("NOTIFICATION", 1, "100", &[2; 32]).unwrap()
    );
    assert_ne!(original, digest("NOTIFICATION", 1, "99", &[3; 32]).unwrap());
    assert!(digest("notification", 1, "99", &[2; 32]).is_err());
    assert!(digest("NOTIFICATION", 1, "099", &[2; 32]).is_err());
}

#[test]
fn child_capability_never_acquires_or_completes_root_authority() {
    let policy = &auth_service::adapters::r#in::grpc::concert_authority::POLICY;
    for method in ["Check", "Enroll", "Track"] {
        let path = format!("/{}/{method}", policy.service);
        assert!(policy.allows(&path, Workload::Notification));
        assert!(!policy.allows(&path, Workload::Gateway));
        assert!(!policy.allows(&(path + "Extra"), Workload::Notification));
    }
    for method in ["Acquire", "Complete"] {
        let path = format!("/{}/{method}", policy.service);
        assert!(!policy.allows(&path, Workload::Notification));
        assert!(policy.allows(&path, Workload::Concert));
    }
    assert_eq!(policy.actor_fields, &[("Acquire", &[&[1u32, 1][..]][..])]);
}
