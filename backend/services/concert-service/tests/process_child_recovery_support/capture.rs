//! Gateway signer adapted from public_profile/native/capture.rs; see provenance.json.
//! Only the actual guarded Product RPC constructs the parent and canonical scope.
pub(super) fn gateway() -> anyhow::Result<platform::actor_delegation::ActorDelegationSigner> {
    platform::actor_delegation::ActorDelegationSigner::from_seed(
        platform::grpc_auth::Workload::Gateway,
        &[74; 32],
    )
}
