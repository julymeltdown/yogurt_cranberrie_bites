//! The Concert exception is confined to the new minimal self content service.
use super::*;
use crate::actor_delegation::{
    relationship_read::{RelationshipReadSigner, RelationshipReadVerifier},
    ActorDelegationSigner,
};
const NAME: &str = "profile.v1.ConcertWorldProfileService";
fn exact() -> MethodPolicy {
    MethodPolicy {
        service: NAME,
        workload: Some(Workload::Profile),
        methods: &[("ReadCurrent", &[Workload::Concert])],
        actor_fields: &[("ReadCurrent", &[&[1]])],
    }
}
#[test]
fn world_profile_registration_is_exact_and_does_not_relax_gateway_routes() {
    assert!(exact().validate(NAME).is_ok());
    for policy in [
        MethodPolicy {
            workload: Some(Workload::Auth),
            ..exact()
        },
        MethodPolicy {
            methods: &[("Other", &[Workload::Concert])],
            ..exact()
        },
        MethodPolicy {
            methods: &[("ReadCurrent", &[Workload::Gateway])],
            ..exact()
        },
        MethodPolicy {
            methods: &[("ReadCurrent", &[Workload::Gateway, Workload::Concert])],
            ..exact()
        },
        MethodPolicy {
            methods: &[
                ("ReadCurrent", &[Workload::Concert]),
                ("Other", &[Workload::Concert]),
            ],
            ..exact()
        },
        MethodPolicy {
            actor_fields: &[],
            ..exact()
        },
        MethodPolicy {
            actor_fields: &[("ReadCurrent", &[&[2]])],
            ..exact()
        },
        MethodPolicy {
            actor_fields: &[("ReadCurrent", &[&[1], &[3]])],
            ..exact()
        },
    ] {
        assert!(policy.validate(NAME).is_err(), "{policy:?}");
    }
    for name in [
        "profile.v1.ProfileService",
        "profile.v1.ProfileAuthorAuthorityService",
        "profile.v1.Other",
    ] {
        let policy = MethodPolicy {
            service: name,
            ..exact()
        };
        assert!(policy.validate(name).is_err());
        let gateway = MethodPolicy {
            methods: &[("ReadCurrent", &[Workload::Gateway])],
            ..policy
        };
        assert!(gateway.validate(name).is_ok());
    }
}
#[test]
fn profile_read_audience_requires_explicit_independent_keys() {
    let old = ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32]).unwrap();
    let gateway = ActorDelegationVerifier::new(Workload::Profile, &[old.public_key()]).unwrap();
    let signer = RelationshipReadSigner::from_seed(&[37; 32], &gateway).unwrap();
    assert!(signer.client((), Workload::Profile).is_ok());
    for audience in [Workload::Concert, Workload::Gateway, Workload::Post] {
        assert!(signer.client((), audience).is_err());
        assert!(RelationshipReadVerifier::new(audience, &[signer.public_key()], &gateway).is_err());
    }
    assert!(
        RelationshipReadVerifier::new(Workload::Profile, &[signer.public_key()], &gateway).is_ok()
    );
    assert!(
        RelationshipReadVerifier::new(Workload::Profile, &[old.public_key()], &gateway).is_err()
    );
    assert!(RelationshipReadVerifier::new(Workload::Profile, &[], &gateway).is_err());
    let parity = ActorDelegationSigner::local_parity(Workload::Gateway).public_key();
    assert!(RelationshipReadVerifier::new(Workload::Profile, &[parity], &gateway).is_err());
}
