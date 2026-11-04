//! Structural contract checks. Actual Auth/TLS/source qualification is driven
//! by Concert's deletion_source_rpc frozen native test, not by positive mocks.
use auth_service::adapters::r#in::grpc::concert_deletion_source::{
    proto, validate_command, POLICY,
};
use platform::grpc_auth::Workload;
use prost::Message;
use uuid::Uuid;
fn command() -> proto::DeletionCommand {
    proto::DeletionCommand {
        saga_id: Uuid::new_v4().to_string(),
        user_id: Uuid::new_v4().to_string(),
        actor_id: Uuid::new_v4().to_string(),
        target: 1,
        retention_seconds: 1_800_000_000,
        retention_nanos: 123_456_789,
    }
}
#[test]
fn source_method_is_concert_only_and_never_claims_an_actor() {
    let path = "/authority.deletion.v1.AuthDeletionSourceService/VerifySource";
    assert!(POLICY.actor_fields.is_empty());
    assert!(POLICY.allows(path, Workload::Concert));
    for w in [
        Workload::Auth,
        Workload::Gateway,
        Workload::Friend,
        Workload::Post,
        Workload::Profile,
        Workload::Notification,
    ] {
        assert!(!POLICY.allows(path, w));
    }
    assert!(!POLICY.allows(
        "/authority.deletion.v1.AuthDeletionSourceService/Complete",
        Workload::Concert
    ));
}
#[test]
fn exact_timestamp_wire_and_unknown_target_refusal() {
    let c = command();
    validate_command(&c).unwrap();
    assert_eq!(
        proto::DeletionCommand::decode(c.encode_to_vec().as_slice()).unwrap(),
        c
    );
    for t in [0, -1, 5, i32::MAX] {
        let mut x = c.clone();
        x.target = t;
        assert!(validate_command(&x).is_err());
    }
    for n in [-1, 1_000_000_000] {
        let mut x = c.clone();
        x.retention_nanos = n;
        assert!(validate_command(&x).is_err());
    }
    for field in 0..3 {
        let mut x = c.clone();
        match field {
            0 => x.saga_id.clear(),
            1 => x.user_id = Uuid::nil().to_string(),
            _ => x.actor_id = "not-an-id".into(),
        };
        let e = validate_command(&x).unwrap_err();
        assert_eq!(e.message(), "Invalid deletion source request");
    }
}
