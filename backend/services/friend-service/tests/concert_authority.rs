use friend_service::concert_authority::{
    proto, OperationBudget, PgOwnerAuthority, SettlementResolver, MAX_GUARDS,
};
use std::time::{Duration, Instant};
use uuid::Uuid;
#[test]
fn original_budget_and_policy_are_bounded() {
    assert!(OperationBudget::new(Instant::now()).is_err());
    assert!(OperationBudget::new(Instant::now() + Duration::from_secs(6)).is_err());
    let p = &friend_service::adapters::r#in::grpc::concert_authority::POLICY;
    assert_eq!(p.methods.len(), 5);
    assert_eq!(p.actor_fields, &[("Acquire", &[&[1u32, 1][..]][..])]);
    assert_eq!(
        p.methods.iter().find(|(m, _)| *m == "Complete").unwrap().1,
        &[platform::grpc_auth::Workload::Concert]
    );
}
#[path = "concert_authority_support/native.rs"]
mod native;

#[path = "concert_authority_support/transport.rs"]
mod transport;
