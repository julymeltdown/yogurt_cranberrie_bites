//! The profile routing rule: which service owns which profile operation.
//!
//! This is the test for the cross-database producer gap. Fifteen frozen profile
//! operations read tables that live in `mcr_concert`, not `mcr_profile`. The
//! Gateway must send each profile operation to the service that owns its tables,
//! because `domain-contract.md` §1.1 forbids a cross-database join.
//!
//! Three properties are asserted here, and each is a different failure mode:
//!
//! 1. **Every mounted profile route has an owner.** A route with no owner would
//!    be refused at runtime; the test makes that a build-time failure instead.
//! 2. **The owner agrees with the coverage register.** The register in
//!    `profile_service::profile_product::coverage` says which service serves
//!    each operation; the gateway's routing table must say the same thing. If
//!    the two disagree, one of them is routing to a database that does not hold
//!    the rows.
//! 3. **The routing rule is data, not a `match` arm.** `PROFILE_OWNERS` is a
//!    table, so a reviewer can read the whole decision at once and a test can
//!    enumerate it.
use api_gateway::adapters::r#in::http::profile_owner::{
    operations_for, owner_of, ProfileOwner, PROFILE_OWNERS,
};
use api_gateway::adapters::r#in::http::profile_product::ROUTES;
use std::collections::{BTreeMap, BTreeSet};

/// The coverage register, read from the profile-service source.
///
/// The register is a `const` in another crate, so it is parsed from the source
/// text rather than linked. That keeps the two crates' dependency direction
/// intact (the gateway must not depend on the profile service's internals) while
/// still comparing the two tables.
fn coverage_register() -> BTreeMap<String, &'static str> {
    // CARGO_MANIFEST_DIR is `backend/services/api-gateway`. `ancestors()` yields
    // the directory itself first, so `nth(2)` is `backend/`.
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("backend directory")
        .join("services/profile-service/src/profile_product/coverage.rs");
    let source = std::fs::read_to_string(&path)
        .unwrap_or_else(|error| panic!("read {}: {error}", path.display()));
    let mut register = BTreeMap::new();
    for line in source.lines() {
        let line = line.trim();
        // `served("id", "METHOD", "path", "owner")`,
        // `concert("id", "METHOD", "path", "owner")` and
        // `owed("id", "METHOD", "path", "owner")`.
        let (kind, rest) = if let Some(rest) = line.strip_prefix("served(\"") {
            ("served", rest)
        } else if let Some(rest) = line.strip_prefix("concert(\"") {
            ("concert", rest)
        } else if let Some(rest) = line.strip_prefix("owed(\"") {
            ("owed", rest)
        } else {
            continue;
        };
        let operation = rest.split('"').next().expect("operation id");
        register.insert(operation.to_owned(), kind);
    }
    assert!(
        !register.is_empty(),
        "the coverage register could not be parsed from {}",
        path.display()
    );
    register
}

#[test]
fn every_mounted_profile_route_has_an_owner() {
    let mut missing = Vec::new();
    for route in ROUTES {
        if owner_of(route.operation).is_none() {
            missing.push(format!("{} {}", route.method, route.operation));
        }
    }
    assert!(
        missing.is_empty(),
        "mounted profile routes with no owning service: {missing:?}"
    );
}

#[test]
fn the_owner_agrees_with_the_coverage_register() {
    let register = coverage_register();
    let mut disagreements = Vec::new();
    for route in ROUTES {
        let Some(kind) = register.get(route.operation) else {
            disagreements.push(format!(
                "{} is mounted but absent from the coverage register",
                route.operation
            ));
            continue;
        };
        let expected = match *kind {
            "served" => ProfileOwner::ProfileService,
            "concert" => ProfileOwner::ConcertService,
            // A NOT_IMPLEMENTED operation must not be mounted at all; the
            // gateway mounts it so the frontend contract is stable, and the
            // producer refuses it. Its owner is still the service that will
            // build it, which is not a routing decision this test can make.
            "owed" => continue,
            other => panic!("unknown register kind {other}"),
        };
        match owner_of(route.operation) {
            Some(actual) if actual == expected => {}
            Some(actual) => disagreements.push(format!(
                "{} is {kind} in the register but routed to {}",
                route.operation,
                actual.as_str()
            )),
            None => disagreements.push(format!("{} has no owner", route.operation)),
        }
    }
    assert!(disagreements.is_empty(), "{}", disagreements.join("\n"));
}

#[test]
fn the_fifteen_cross_database_operations_route_to_the_concert_service() {
    // The exact set the task names. Pinned so a later change has to justify
    // itself against the migrations that created these tables.
    let concert: BTreeSet<&str> = operations_for(ProfileOwner::ConcertService).collect();
    assert_eq!(
        concert,
        BTreeSet::from([
            "listProfilePosts",
            "createProfilePost",
            "deleteProfilePost",
            "initProfileMediaUpload",
            "putProfileMediaBytes",
            "finalizeProfileMedia",
            "getProfileMedia",
            "readProfileMedia",
            "getProfileRoom",
            "getOwnRoom",
            "saveOwnRoom",
            "setOwnRoomVisibility",
            "uploadRoomPoster",
            "getOwnProfileAvatar",
            "setOwnProfileAvatar",
        ])
    );
    assert_eq!(concert.len(), 15, "fifteen cross-database operations");

    // And every one of them is actually mounted by the gateway, so the route
    // exists for the frontend contract while the producer decides.
    let mounted: BTreeSet<&str> = ROUTES.iter().map(|route| route.operation).collect();
    for operation in &concert {
        assert!(
            mounted.contains(operation),
            "{operation} is routed to the concert service but not mounted"
        );
    }
}

#[test]
fn the_profile_database_operations_stay_in_the_profile_service() {
    // The complement: operations whose tables live in `mcr_profile`. A concert
    // route for one of these would read a database that does not hold its rows.
    for operation in [
        "getOwnProfile",
        "getProfile",
        "searchCatalog",
        "getOwnIdentityVerification",
        "startIdentityVerification",
        "disconnectIdentityVerification",
    ] {
        assert_eq!(
            owner_of(operation),
            Some(ProfileOwner::ProfileService),
            "{operation} reads mcr_profile and must stay in the profile service"
        );
    }
}

#[test]
fn the_routing_rule_is_a_table_not_a_match_arm() {
    // The rule is data: every row is inspectable, and the two services each own
    // a non-empty, disjoint set.
    assert!(!PROFILE_OWNERS.is_empty());
    let profile: BTreeSet<&str> = operations_for(ProfileOwner::ProfileService).collect();
    let concert: BTreeSet<&str> = operations_for(ProfileOwner::ConcertService).collect();
    assert!(!profile.is_empty());
    assert!(!concert.is_empty());
    assert!(
        profile.is_disjoint(&concert),
        "an operation cannot be owned by both services"
    );
    // Every row names a real owner string.
    for (operation, owner) in PROFILE_OWNERS {
        assert!(!operation.is_empty());
        assert!(matches!(
            owner,
            ProfileOwner::ProfileService | ProfileOwner::ConcertService
        ));
    }
}

#[test]
fn an_unknown_operation_is_not_defaulted_to_a_service() {
    // The rule returns `None` rather than a default, so a caller must refuse.
    // A default would route an unknown operation to a service that cannot
    // answer it, which is the exact failure this work removes.
    assert_eq!(owner_of("notAProfileOperation"), None);
    assert_eq!(owner_of(""), None);
    // A concert-product operation is not a profile operation at all.
    assert_eq!(owner_of("listConcertFeed"), None);
}

#[test]
fn no_routed_route_carries_an_event_path() {
    // The routing split must not weaken the event rule: a profile operation
    // never carries an event, whichever service serves it.
    for route in ROUTES {
        assert!(
            !route.path.contains("events") && !route.path.contains(":event_id"),
            "a profile route must never carry an event: {route:?}"
        );
    }
}
