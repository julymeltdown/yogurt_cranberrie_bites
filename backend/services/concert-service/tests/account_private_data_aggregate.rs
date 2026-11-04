//! BE42 aggregate protocol source checks. These do not prove any owner effect.
use chrono::{DateTime, Duration, Utc};
use concert_service::account_private_data::*;
use concert_service::world::account_deletion::{
    VerifiedDeletionSource, WorldDeletionCommand, WorldDeletionTarget,
};
use std::sync::Arc;
use std::time::Duration as WallDuration;
use uuid::Uuid;

fn command() -> WorldDeletionCommand {
    WorldDeletionCommand {
        saga_id: Uuid::from_u128(11),
        user_id: Uuid::from_u128(12),
        target: WorldDeletionTarget::PrivateData,
        actor_id: Uuid::from_u128(13),
        retention_until: "2070-01-01T00:00:00.123456789Z".parse().unwrap(),
    }
}

#[test]
fn registry_is_closed_and_core_is_never_a_whole_ack() {
    assert_eq!(PrivateDataCategory::ALL.len(), 7);
    assert_eq!(
        PrivateDataCategory::ALL[0],
        PrivateDataCategory::CorePreferencesAndAvatar
    );
    assert_ne!(PrivateDataCategory::ALL[0], PrivateDataCategory::IntroText);
    let c = command();
    let receipt = PrivateDataCategoryReceipt {
        command: c.clone(),
        category: PrivateDataCategory::CorePreferencesAndAvatar,
        version: CATEGORY_VERSION,
        source_binding: [1; 32],
        progress_revision: 1,
        complete: true,
    };
    let mut source = [1; 32];
    assert!(receipt.verifies(&c, &source));
    source[0] = 2;
    assert!(!receipt.verifies(&c, &source));
    let mut other = c.clone();
    other.saga_id = Uuid::from_u128(14);
    assert!(!receipt.verifies(&other, &[1; 32]));
}

#[test]
fn submicrosecond_command_is_part_of_category_proof() {
    let c = command();
    let deadline = tokio::time::Instant::now() + WallDuration::from_secs(4);
    let source = VerifiedDeletionSource::from_verified_auth_source(&c, deadline, [7; 32]).unwrap();
    let mut other = c.clone();
    other.retention_until += Duration::nanoseconds(1);
    assert_ne!(c.binding().unwrap(), other.binding().unwrap());
    assert_eq!(format!("{source:?}"), "VerifiedDeletionSource([REDACTED])");
}

#[test]
fn aggregate_debug_and_full_receipt_are_redacted() {
    let c = command();
    let ack = PrivateDataAggregateAcknowledgement {
        command: c.clone(),
        fence_revision: 1,
        completed_categories: PrivateDataCategory::ALL.len(),
    };
    assert!(ack.verifies(&c));
    let mut short = ack.clone();
    short.completed_categories = 1;
    assert!(!short.verifies(&c));
    assert_eq!(
        format!("{ack:?}"),
        "PrivateDataAggregateAcknowledgement([REDACTED])"
    );
}

#[test]
fn production_core_only_registry_is_explicitly_incomplete() {
    let owners = PrivateDataCategoryOwners::default();
    assert_eq!(owners.configured(), 0);
    for category in PrivateDataCategory::ALL {
        assert!(owners.get(category).is_none());
    }
}

#[test]
fn category_ordinal_is_closed_over_the_seven_registry_entries() {
    let mut seen = std::collections::BTreeSet::new();
    for category in PrivateDataCategory::ALL {
        let index = PrivateDataCategory::ALL
            .iter()
            .position(|candidate| candidate == &category)
            .unwrap();
        assert!(seen.insert(index));
    }
    assert_eq!(seen.len(), PrivateDataCategory::ALL.len());
}

#[test]
fn registry_extensions_cannot_replace_an_existing_owner() {
    let fixture: Arc<dyn PrivateDataCategoryOwner> =
        Arc::new(FixtureCategoryOwner::new(PrivateDataCategory::IntroText));
    let owners =
        PrivateDataCategoryOwners::new([(PrivateDataCategory::IntroText, fixture.clone())]);
    assert!(owners
        .add_owners([(PrivateDataCategory::IntroText, fixture)])
        .is_err());
}

#[test]
fn registry_can_add_one_disjoint_fixture_owner() {
    let fixtures: Arc<dyn PrivateDataCategoryOwner> =
        Arc::new(FixtureCategoryOwner::new(PrivateDataCategory::IntroText));
    let owners = PrivateDataCategoryOwners::empty()
        .add_owners([(PrivateDataCategory::IntroText, fixtures)])
        .unwrap();
    assert_eq!(owners.configured(), 1);
    assert!(owners.get(PrivateDataCategory::IntroText).is_some());
    assert!(owners.get(PrivateDataCategory::AgeClaims).is_none());
}

#[test]
fn persisted_retention_round_trip_preserves_exactly_one_nanosecond() {
    let c = command();
    let seconds = c.retention_until.timestamp();
    let nanos = c.retention_until.timestamp_subsec_nanos();
    let restored = DateTime::<Utc>::from_timestamp(seconds, nanos).unwrap();
    assert_eq!(restored, c.retention_until);
    assert_eq!(restored.timestamp_subsec_nanos(), 123_456_789);
    assert_ne!(restored, c.retention_until - Duration::nanoseconds(1));
}

#[test]
fn unbound_or_incomplete_category_result_is_never_positive() {
    let c = command();
    let source = [17; 32];
    for category in PrivateDataCategory::ALL {
        let receipt = PrivateDataCategoryReceipt {
            command: c.clone(),
            category,
            version: CATEGORY_VERSION,
            source_binding: source,
            progress_revision: 1,
            complete: true,
        };
        assert!(receipt.verifies(&c, &source));
        let mut incomplete = receipt.clone();
        incomplete.complete = false;
        assert!(!incomplete.verifies(&c, &source));
        let mut other_source = source;
        other_source[31] ^= 1;
        assert!(!receipt.verifies(&c, &other_source));
    }
}

#[test]
fn noncomplete_owner_receipt_cannot_be_terminal_proof() {
    let c = command();
    let receipt = PrivateDataCategoryReceipt {
        command: c.clone(),
        category: PrivateDataCategory::AgeClaims,
        version: CATEGORY_VERSION,
        source_binding: [3; 32],
        progress_revision: 4,
        complete: false,
    };
    let mut partial = receipt.clone();
    partial.progress_revision = 0;
    assert!(partial.verifies(&c, &[3; 32]));
    assert!(!receipt.verifies(&c, &[3; 32]));
    assert!(!PrivateDataAggregateAcknowledgement {
        command: c,
        fence_revision: 1,
        completed_categories: 1,
    }
    .verifies(&command()));
}

#[test]
fn retention_record_is_reconstructible_for_any_valid_nanosecond() {
    for nanos in [0, 1, 999_999_998, 999_999_999] {
        let c = command();
        let seconds = c.retention_until.timestamp();
        let restored = DateTime::<Utc>::from_timestamp(seconds, nanos).unwrap();
        assert_eq!(restored.timestamp_subsec_nanos(), nanos);
        assert_eq!(restored.timestamp(), seconds);
    }
}
