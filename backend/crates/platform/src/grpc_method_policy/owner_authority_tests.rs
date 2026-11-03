use super::*;
const PUBLIC: &str = "profile.concert_public.v1.ConcertPublicProfileService";
const POST: &str = "post.concert.v1.PostConcertFeedReadService";
#[test]
fn exact_public_profile_and_post_contracts_reject_policy_broadening() {
    let profile = MethodPolicy {
        service: PUBLIC,
        workload: Some(Workload::Profile),
        methods: &[
            ("ReadCurrent", &[Workload::Concert]),
            ("Check", &[Workload::Concert]),
            ("Complete", &[Workload::Concert]),
        ],
        actor_fields: &[
            ("ReadCurrent", &[&[1]]),
            ("Check", &[&[1]]),
            ("Complete", &[&[1]]),
        ],
    };
    let post = MethodPolicy {
        service: POST,
        workload: Some(Workload::Post),
        methods: &[
            ("GetPost", &[Workload::Concert]),
            ("ListPosts", &[Workload::Concert]),
            ("GetComment", &[Workload::Concert]),
            ("ListComments", &[Workload::Concert]),
        ],
        actor_fields: &[
            ("GetPost", &[&[1, 1, 1]]),
            ("ListPosts", &[&[1, 1, 1]]),
            ("GetComment", &[&[1, 1, 1]]),
            ("ListComments", &[&[1, 1, 1]]),
        ],
    };
    for exact in [profile, post] {
        exact.validate(exact.service).unwrap();
        let wrong_owner = MethodPolicy {
            workload: Some(Workload::Gateway),
            ..exact
        };
        assert!(wrong_owner.validate(exact.service).is_err());
        let no_proof = MethodPolicy {
            actor_fields: &[],
            ..exact
        };
        assert!(no_proof.validate(exact.service).is_err());
        for method in exact.method_names() {
            let path = format!("/{}/{method}", exact.service);
            assert!(exact.allows(&path, Workload::Concert));
            assert!(!exact.allows(&path, Workload::Gateway));
            assert!(!exact.allows(&(path + "Extra"), Workload::Concert));
        }
    }
}
#[test]
fn owner_acquire_and_system_settlement_have_separate_exact_policies() {
    for (service, workload) in [
        (
            "authority.lease.v1.AuthOwnerAuthorityService",
            Workload::Auth,
        ),
        (
            "authority.lease.v1.FriendOwnerAuthorityService",
            Workload::Friend,
        ),
    ] {
        let exact = MethodPolicy {
            service,
            workload: Some(workload),
            methods: &[
                ("Acquire", &[Workload::Concert]),
                (
                    "Check",
                    &[Workload::Concert, Workload::Post, Workload::Profile],
                ),
                (
                    "Enroll",
                    &[Workload::Concert, Workload::Post, Workload::Profile],
                ),
                (
                    "Track",
                    &[Workload::Concert, Workload::Post, Workload::Profile],
                ),
                ("Complete", &[Workload::Concert]),
            ],
            actor_fields: &[("Acquire", &[&[1, 1]])],
        };
        exact.validate(service).unwrap();
        assert!(!exact.allows(&format!("/{service}/Complete"), Workload::Post));
        assert!(!exact.allows(&format!("/{service}/Acquire"), Workload::Profile));
        assert!(MethodPolicy {
            actor_fields: &[],
            ..exact
        }
        .validate(service)
        .is_err());
    }
    let exact = MethodPolicy {
        service: "authority.lease.v1.ProducerSettlementService",
        workload: Some(Workload::Concert),
        methods: &[
            ("ResolveTransaction", &[Workload::Auth, Workload::Friend]),
            ("ResolveScope", &[Workload::Auth, Workload::Friend]),
        ],
        actor_fields: &[],
    };
    exact.validate(exact.service).unwrap();
    for caller in [
        Workload::Gateway,
        Workload::Post,
        Workload::Profile,
        Workload::Concert,
    ] {
        assert!(!exact.allows(
            "/authority.lease.v1.ProducerSettlementService/ResolveTransaction",
            caller
        ));
    }
    assert!(MethodPolicy {
        methods: &[
            (
                "ResolveTransaction",
                &[Workload::Auth, Workload::Friend, Workload::Concert]
            ),
            ("ResolveScope", &[Workload::Auth, Workload::Friend])
        ],
        ..exact
    }
    .validate(exact.service)
    .is_err());
}
