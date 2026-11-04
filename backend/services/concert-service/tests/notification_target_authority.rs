//! Actual original-Root callback, native retention and closed recovery. This is
//! internal mTLS qualification, not public Gateway or complete BE33/IN03 acceptance.
#[path = "canonical_owner_composition_support/capture.rs"]
mod capture;
#[path = "canonical_owner_composition_support/environment.rs"]
mod environment;
mod notification_target_authority_support;
use concert_service::{
    adapters::out::relationship_authority::guarded::{
        ChildNativeIdentity, ChildQuery, ChildRecoveryReference,
    },
    notification_live::target_authority::{TargetReadSettler, POLICY},
};
use platform::grpc_auth::Workload;
use uuid::Uuid;
#[test]
fn target_callback_admits_only_notification_workload() {
    assert_eq!(
        POLICY.service,
        "notification.concert.v1.ConcertNotificationTargetAuthority"
    );
    assert_eq!(POLICY.workload, Some(Workload::Concert));
    assert_eq!(
        POLICY.methods,
        &[("Authorize", &[Workload::Notification][..])]
    );
    assert!(POLICY.actor_fields.is_empty());
}
#[test]
fn target_child_is_closed_private_and_never_a_public_or_social_read() {
    let actor = Uuid::from_u128(1);
    let reference = ChildRecoveryReference::TargetRead {
        query: ChildQuery {
            actor,
            event: Uuid::from_u128(2),
            subject: actor,
            family: Uuid::from_u128(3),
            request: Uuid::from_u128(4),
            expires_unix_micros: 1,
            binding_digest: [5; 32],
        },
        root_scope_id: Uuid::from_u128(6),
        child_nonce: Uuid::from_u128(7),
        native: ChildNativeIdentity {
            system_identifier: "123".into(),
            database_oid: 1,
            full_xid: "77".into(),
            backend_pid: 2,
            backend_started_unix_micros: 1,
            producer_instance_id: Uuid::from_u128(8),
        },
        inbox_intent_id: Uuid::from_u128(9),
        inbox_reference_digest: [10; 32],
        snapshot_digest: [11; 32],
    };
    assert_eq!(reference.kind_name(), "TARGET_READ");
    assert_eq!(reference.intent_id().unwrap(), Uuid::from_u128(7));
    assert_eq!(
        reference.local_prepared_gid().unwrap(),
        format!("mcr-notification-target:{}", Uuid::from_u128(7).simple())
    );
    assert!(
        reference.check_content_time().is_err(),
        "expired disclosure cannot be revived"
    );
    let value = serde_json::to_value(&reference).unwrap();
    for key in [
        "gid",
        "callback_url",
        "completion_secret",
        "authority_until",
    ] {
        let mut bad = value.clone();
        bad[key] = serde_json::json!("not allowed");
        assert!(serde_json::from_value::<ChildRecoveryReference>(bad).is_err());
    }
    for kind in ["LOCAL_PUBLIC", "SOCIAL_READ", "NOTIFICATION_INBOX"] {
        let mut bad = value.clone();
        bad["kind"] = serde_json::json!(kind);
        assert!(serde_json::from_value::<ChildRecoveryReference>(bad).is_err());
    }
}
#[tokio::test]
async fn target_recovery_refuses_unknown_instance_before_io() {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://mcr_owner@127.0.0.1:1/mcr_concert_test")
        .unwrap();
    assert!(TargetReadSettler::new(pool, Uuid::nil()).is_err());
}
#[tokio::test]
#[ignore = "fresh named owned PG16/mTLS allocation required"]
async fn original_root_target_callback_retains_policy_until_notification_settles(
) -> anyhow::Result<()> {
    notification_target_authority_support::run().await
}

use notification_service::concert::{authority, transport, AuthorityFence, Authorization};
#[path = "../../notification-service/src/concert/target_client.rs"]
mod target_client;
