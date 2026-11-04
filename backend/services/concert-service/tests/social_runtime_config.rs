//! Pure composition/closed-registration checks; never a positive owner scope.
use concert_service::{
    config::social_runtime::SocialRuntimeConfig,
    product_rpc::{operation, Operation},
    social_live::{
        keys::{KeyMaterial, ManagedSocialKeys},
        protocol::SocialOperation,
    },
};
use uuid::Uuid;

fn keys() -> ManagedSocialKeys {
    // Explicit deterministic key material exists only in this test fixture.
    ManagedSocialKeys::new(
        KeyMaterial::new("test-handle".into(), vec![11; 32]),
        vec![],
        KeyMaterial::new("test-command".into(), vec![22; 32]),
        vec![],
        &[33; 32],
    )
    .unwrap()
}
#[test]
fn immutable_trust_is_explicit_and_missing_trust_does_not_invent_a_claim() {
    assert!(SocialRuntimeConfig::new(keys(), None, 3600).is_ok());
    assert!(
        SocialRuntimeConfig::new(keys(), Some(("accepted-issuer", "revision-1")), 3600).is_ok()
    );
    for pair in [
        ("", "v1"),
        ("issuer", ""),
        (" issuer", "v1"),
        ("issuer", "rev/1"),
    ] {
        assert!(SocialRuntimeConfig::new(keys(), Some(pair), 3600).is_err());
    }
}
#[test]
fn ttl_is_bounded_without_process_environment_or_defaults() {
    for ttl in [0, -1, 86401, i64::MAX] {
        assert!(SocialRuntimeConfig::new(keys(), None, ttl).is_err());
    }
    for ttl in [1, 3600, 86400] {
        assert!(SocialRuntimeConfig::new(keys(), None, ttl).is_ok());
    }
}
#[test]
fn managed_material_rejects_missing_short_and_duplicate_versions() {
    let material = |id: &str, n| KeyMaterial::new(id.into(), vec![1; n]);
    for length in [0, 16, 31, 33] {
        assert!(ManagedSocialKeys::new(
            material("h", length),
            vec![],
            material("c", 32),
            vec![],
            &[3; 32]
        )
        .is_err());
    }
    assert!(ManagedSocialKeys::new(
        material("h", 32),
        vec![material("h", 32)],
        material("c", 32),
        vec![],
        &[3; 32]
    )
    .is_err());
    assert!(ManagedSocialKeys::new(
        material("h", 32),
        vec![],
        material("c", 32),
        vec![material("c", 32)],
        &[3; 32]
    )
    .is_err());
    assert!(ManagedSocialKeys::new(
        material("h", 32),
        vec![],
        material("c", 31),
        vec![],
        &[3; 32]
    )
    .is_err());
    assert!(
        ManagedSocialKeys::new(material("h", 32), vec![], material("c", 32), vec![], &[]).is_err()
    );
}
#[test]
fn thirteen_frozen_operations_use_the_actual_closed_root_registration() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let rows: serde_json::Value = serde_json::from_slice(
        &std::fs::read(root.join("contracts/domain/v1/operations.json")).unwrap(),
    )
    .unwrap();
    let event = Uuid::new_v4();
    let id = Uuid::new_v4();
    let mut found = 0;
    for row in rows.as_array().unwrap() {
        let name = row["operationId"].as_str().unwrap();
        let path = row["path"]
            .as_str()
            .unwrap()
            .replace("{eventId}", &event.to_string())
            .replace("{requestId}", &id.to_string())
            .replace("{userId}", &id.to_string())
            .replace("{platform}", "INSTAGRAM");
        if let Ok(Operation::Social(op)) = operation(
            event,
            &row["method"].as_str().unwrap().to_ascii_uppercase(),
            &path,
        ) {
            found += 1;
            assert_eq!(op.id(), name);
            assert_eq!(row["success_status"], 200);
        }
    }
    assert_eq!(found, 13);
    assert!(matches!(
        operation(
            event,
            "POST",
            &format!("/api/concert/events/{event}/social-requests")
        )
        .unwrap(),
        Operation::Social(SocialOperation::Create)
    ));
}
#[test]
fn unknown_social_methods_platforms_and_nested_routes_remain_closed() {
    let event = Uuid::new_v4();
    let id = Uuid::new_v4();
    let root = format!("/api/concert/events/{event}");
    for (method, tail) in [
        ("PUT", "me/social-handles/UNKNOWN".into()),
        ("POST", format!("social-requests/{id}/reveal")),
        ("GET", format!("social-requests/{id}/private")),
        ("GET", "me/social-handles/INSTAGRAM".into()),
        (
            "GET",
            "me/blocks/00000000-0000-0000-0000-000000000000".into(),
        ),
    ] {
        assert!(operation(event, method, &format!("{root}/{tail}")).is_err());
    }
    assert!(matches!(
        operation(event, "GET", &format!("{root}/feed")).unwrap(),
        Operation::Feed
    ));
    assert!(matches!(
        operation(event, "POST", &format!("{root}/parties")).unwrap(),
        Operation::CreateParty
    ));
}
