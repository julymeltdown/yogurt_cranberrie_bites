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
#[test]
fn root_callback_and_external_delivery_are_distinct_capabilities() {
    let root = MethodPolicy {
        service: "authority.lease.v1.ConcertRootReadService",
        workload: Some(Workload::Concert),
        methods: &[("CheckSubject", &[Workload::Post])],
        actor_fields: &[("CheckSubject", &[&[1, 3, 1]])],
    };
    root.validate(root.service).unwrap();
    assert!(MethodPolicy {
        actor_fields: &[],
        ..root
    }
    .validate(root.service)
    .is_err());
    let delivery = MethodPolicy {
        service: "concert.product.v1.ProductDeliveryService",
        workload: Some(Workload::Concert),
        methods: &[("CompleteResponse", &[Workload::Gateway])],
        actor_fields: &[],
    };
    delivery.validate(delivery.service).unwrap();
    assert!(!delivery.allows(
        "/concert.product.v1.ProductDeliveryService/CompleteResponse",
        Workload::Post
    ));
    assert!(MethodPolicy {
        methods: &[("CompleteResponse", &[Workload::Concert])],
        ..delivery
    }
    .validate(delivery.service)
    .is_err());
}
#[test]
fn self_profile_never_accepts_gateway_or_an_unbound_subject() {
    let exact = MethodPolicy {
        service: "profile.self.v1.ConcertSelfProfileService",
        workload: Some(Workload::Profile),
        methods: &[
            ("Read", &[Workload::Concert]),
            ("PrepareUpdate", &[Workload::Concert]),
            ("ReadOperation", &[Workload::Concert]),
            ("Reconcile", &[Workload::Concert]),
        ],
        actor_fields: &[
            ("Read", &[&[1, 1, 1], &[2]]),
            ("PrepareUpdate", &[&[1, 1, 1], &[2]]),
            ("ReadOperation", &[&[1, 1, 1]]),
            ("Reconcile", &[&[1, 1, 1]]),
        ],
    };
    exact.validate(exact.service).unwrap();
    assert!(!exact.allows(
        "/profile.self.v1.ConcertSelfProfileService/PrepareUpdate",
        Workload::Gateway
    ));
    assert!(MethodPolicy {
        actor_fields: &[
            ("Read", &[&[1, 1, 1]]),
            ("PrepareUpdate", &[&[1, 1, 1]]),
            ("ReadOperation", &[&[1, 1, 1]]),
            ("Reconcile", &[&[1, 1, 1]])
        ],
        ..exact
    }
    .validate(exact.service)
    .is_err());
}
