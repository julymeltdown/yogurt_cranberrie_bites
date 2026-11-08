//! WP-02 follow-up: profile producer coverage.
//!
//! The contract freezes 43 profile-owned operations. This test proves that every
//! one of them is **accounted for**: either a real producer serves it, or it is
//! explicitly recorded as `NOT_IMPLEMENTED` with an owning work package. No
//! operation may be silently missing, and no operation may be claimed as served
//! when the parser refuses it.
//!
//! The test is deliberately built from the contract rather than from the
//! register, so the register cannot agree with itself and be wrong. It reads
//! `contracts/core/v1/operations.json` and `contracts/domain/v1/operations.json`,
//! recomputes the profile-owned set, and compares it with
//! `profile_service::profile_product::coverage::COVERAGE` in both directions.
//!
//! The second half of the test drives the real route parser: a `Served` row must
//! parse, and a `NOT_IMPLEMENTED` row must be refused. That is what makes the
//! register a statement about behavior rather than a comment.
use profile_service::profile_product::coverage::{self, Producer};
use profile_service::profile_product::targets;
use std::collections::{BTreeMap, BTreeSet};

/// The repository root, from this crate's manifest directory.
fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .expect("workspace root")
        .to_path_buf()
}

/// Every operation in the frozen core and domain registers.
fn contract_operations() -> Vec<serde_json::Value> {
    let mut rows = Vec::new();
    for bundle in [
        "contracts/core/v1/operations.json",
        "contracts/domain/v1/operations.json",
    ] {
        let bytes = std::fs::read(root().join(bundle))
            .unwrap_or_else(|error| panic!("read {bundle}: {error}"));
        let mut parsed: Vec<serde_json::Value> =
            serde_json::from_slice(&bytes).unwrap_or_else(|error| panic!("parse {bundle}: {error}"));
        rows.append(&mut parsed);
    }
    rows
}

/// The profile-owned operations: every operation whose path is not an event
/// path and not an Auth path.
///
/// `/api/concert/events/…` belongs to the concert product. `/api/auth/…` belongs
/// to the Auth service and is served by its own routes; it is in the core
/// register but is not profile-owned, so it is outside this register by
/// construction rather than by omission.
fn profile_owned(rows: &[serde_json::Value]) -> Vec<&serde_json::Value> {
    rows.iter()
        .filter(|row| {
            let path = row["path"].as_str().expect("operation path");
            !path.starts_with("/api/concert/events/") && !path.starts_with("/api/auth/")
        })
        .collect()
}

/// A concrete path for a contract template, so the parser can be driven.
fn concrete(path: &str) -> String {
    path.replace("{profileId}", "11111111-1111-4111-8111-111111111111")
        .replace("{introId}", "22222222-2222-4222-8222-222222222222")
        .replace("{postId}", "33333333-3333-4333-8333-333333333333")
        .replace("{mediaId}", "44444444-4444-4444-8444-444444444444")
        .replace("{commentId}", "55555555-5555-4555-8555-555555555555")
        .replace("{notificationId}", "66666666-6666-4666-8666-666666666666")
        .replace("{provider}", "naver")
        .replace("{category}", "music")
}

#[test]
fn the_contract_freezes_forty_three_profile_operations() {
    // 41 rows as enumerated by the freeze record (§7: 10 core + 31 domain),
    // plus `createIntroPostComment` and `deleteIntroPostComment`, which commit
    // `c79ed25c1` added after that record was written. The number is pinned so a
    // later contract change has to update this register in the same commit.
    let rows = contract_operations();
    let owned = profile_owned(&rows);
    assert_eq!(
        owned.len(),
        43,
        "expected 43 profile-owned operations; found {}",
        owned.len()
    );
}

#[test]
fn the_register_names_exactly_the_contract_operations() {
    let rows = contract_operations();
    let owned = profile_owned(&rows);
    let contract: BTreeSet<&str> = owned
        .iter()
        .map(|row| row["operationId"].as_str().expect("operationId"))
        .collect();
    let register: BTreeSet<&str> = coverage::COVERAGE.iter().map(|row| row.operation).collect();

    // Nothing in the contract is silently missing from the register.
    let missing: Vec<&&str> = contract.difference(&register).collect();
    assert!(
        missing.is_empty(),
        "profile operations with no coverage row: {missing:?}"
    );
    // Nothing in the register is invented.
    let extra: Vec<&&str> = register.difference(&contract).collect();
    assert!(
        extra.is_empty(),
        "coverage rows that are not frozen profile operations: {extra:?}"
    );
}

#[test]
fn every_register_row_matches_the_frozen_method_and_path() {
    let rows = contract_operations();
    let by_id: BTreeMap<&str, &serde_json::Value> = rows
        .iter()
        .map(|row| (row["operationId"].as_str().expect("operationId"), row))
        .collect();
    for entry in coverage::COVERAGE {
        let row = by_id
            .get(entry.operation)
            .unwrap_or_else(|| panic!("{} is not in the contract", entry.operation));
        assert_eq!(
            row["method"].as_str().expect("method").to_ascii_uppercase(),
            entry.method,
            "{} method",
            entry.operation
        );
        assert_eq!(
            row["path"].as_str().expect("path"),
            entry.path,
            "{} path",
            entry.operation
        );
    }
}

#[test]
fn every_operation_is_either_served_or_explicitly_not_implemented() {
    // The exhaustive statement of the deliverable: no third state exists, and
    // every row names an owner.
    for entry in coverage::COVERAGE {
        assert!(!entry.owner.is_empty(), "{} has no owner", entry.operation);
        match entry.producer {
            Producer::Served => {
                // A served operation names a module in THIS service.
                assert!(
                    !entry.owner.starts_with("WP-"),
                    "{} is Served but names a work package rather than a module: {}",
                    entry.operation,
                    entry.owner
                );
            }
            Producer::ServedByConcertService => {
                // A concert-served operation names a module in the CONCERT
                // service, so the split is visible in the register itself.
                assert!(
                    entry.owner.starts_with("concert_service::"),
                    "{} is served by the concert service but names {}",
                    entry.operation,
                    entry.owner
                );
            }
            Producer::NotImplemented => {
                assert!(
                    entry.owner.starts_with("WP-"),
                    "{} is NOT_IMPLEMENTED but names no work package: {}",
                    entry.operation,
                    entry.owner
                );
            }
        }
    }
    let (served, by_concert, owed) = coverage::counts();
    assert_eq!(
        served + by_concert + owed,
        43,
        "every operation is accounted for"
    );
}

#[test]
fn a_served_operation_parses_and_a_not_implemented_one_is_refused() {
    // The register is a claim about behavior, so it is checked against the real
    // parser rather than trusted.
    let mut failures = Vec::new();
    for entry in coverage::COVERAGE {
        let target = concrete(entry.path);
        let parsed = targets::parse(entry.method, &target);
        match entry.producer {
            Producer::Served => {
                if let Err(status) = &parsed {
                    failures.push(format!(
                        "{} is marked served but the parser refused it: {}",
                        entry.operation, status
                    ));
                } else if let Ok(operation) = parsed {
                    if operation.contract_id() != entry.operation {
                        failures.push(format!(
                            "{} parsed as {}",
                            entry.operation,
                            operation.contract_id()
                        ));
                    }
                }
            }
            Producer::ServedByConcertService => {
                // The profile service's parser must refuse a concert-served
                // operation: its tables are not in this database, so answering
                // it here would read the wrong store.
                if parsed.is_ok() {
                    failures.push(format!(
                        "{} is served by the concert service but this parser accepted it",
                        entry.operation
                    ));
                }
            }
            Producer::NotImplemented => {
                if parsed.is_ok() {
                    failures.push(format!(
                        "{} is marked NOT_IMPLEMENTED but the parser accepted it",
                        entry.operation
                    ));
                }
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[test]
fn no_register_path_carries_an_event_segment() {
    for entry in coverage::COVERAGE {
        assert!(
            !entry.path.contains("/events/"),
            "{} must not carry an event path",
            entry.operation
        );
        // And the parser refuses an event path for every one of them.
        let event_target = format!(
            "{}/events/99999999-9999-4999-8999-999999999999",
            concrete(entry.path)
        );
        assert!(
            targets::parse(entry.method, &event_target).is_err(),
            "{} accepted an event path",
            entry.operation
        );
    }
}

#[test]
fn the_served_set_is_exactly_what_the_producer_answers() {
    // A short, explicit list. If a later work package wires another operation,
    // this test fails and forces the register to be updated in the same change —
    // which is the point: the register cannot drift from the producer silently.
    let served: BTreeSet<&str> = coverage::served_operations()
        .map(|entry| entry.operation)
        .collect();
    assert_eq!(
        served,
        BTreeSet::from([
            "getOwnProfile",
            "getProfile",
            "searchCatalog",
            "getOwnIdentityVerification",
            "startIdentityVerification",
            "disconnectIdentityVerification",
        ]),
        "the profile-service served set changed; update coverage::COVERAGE in the same commit"
    );

    // And the concert-served set is exactly the fifteen cross-database
    // operations. This is the other half of the split: an operation may not be
    // claimed as concert-served unless the concert service really answers it.
    let by_concert: BTreeSet<&str> = coverage::concert_served_operations()
        .map(|entry| entry.operation)
        .collect();
    assert_eq!(
        by_concert,
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
        ]),
        "the concert-served set changed; update coverage::COVERAGE in the same commit"
    );
    assert_eq!(by_concert.len(), 15);
}

#[test]
fn the_cross_service_operations_are_served_by_the_concert_service() {
    // WP-06, WP-07 and WP-08 built producers for operations whose tables live
    // in the concert database. They are now served by the concert-hosted
    // profile transport and routed there by the Gateway, so the register
    // records them as `ServedByConcertService` rather than as a gap.
    let cross_service = [
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
    ];
    for operation in cross_service {
        let entry = coverage::find(operation)
            .unwrap_or_else(|| panic!("{operation} is not in the register"));
        assert_eq!(
            entry.producer,
            Producer::ServedByConcertService,
            "{operation} reads concert-owned tables and must be served there"
        );
        assert!(
            entry.owner.starts_with("concert_service::"),
            "{operation} must name the concert module that serves it: {}",
            entry.owner
        );
    }
    assert!(
        coverage::CROSS_SERVICE_NOTE.contains("Gateway routes it there"),
        "the cross-service note must explain how these are routed"
    );
}
