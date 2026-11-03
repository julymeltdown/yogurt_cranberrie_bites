use chrono::Utc;
use concert_domain::place::*;
use serde_json::{json, Value};
use std::str::FromStr;
use uuid::Uuid;

const PLACE_A: &str = "00000000-0000-4000-8000-000000000001";
const PLACE_B: &str = "00000000-0000-4000-8000-000000000002";
fn digest(bytes: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    format!(
        "sha256:{}",
        Sha256::digest(bytes)
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>()
    )
}
fn canonical(v: &Value) -> Vec<u8> {
    serde_json::to_vec(v).unwrap()
}
fn provenance(kind: SourceKind, uri: Option<&str>, review: ReviewState) -> Provenance {
    Provenance::new(
        "fixture_source".into(),
        kind,
        "revision1".into(),
        uri.map(str::to_owned),
        "MCR fixture author".into(),
        "Synthetic metadata only".into(),
        review,
        (review == ReviewState::Reviewed).then(|| "parent-reviewer".into()),
        (review == ReviewState::Reviewed).then(Utc::now),
        PhotoPermission::NotApplicable,
    )
    .unwrap()
}
fn place(
    id: &str,
    kind: PlaceKind,
    verification: VerificationState,
    closure: ClosureState,
    uri: Option<&str>,
    review: ReviewState,
) -> Place {
    Place::new(
        id.parse().unwrap(),
        "Fixture place".into(),
        kind,
        verification,
        closure,
        "Accessible metadata fixture.".into(),
        provenance(SourceKind::PlaceReference, uri, review),
    )
    .unwrap()
}
fn base_manifest() -> Value {
    let source: Value = serde_json::from_slice(include_bytes!(
        "../../../../contracts/fixtures/assets/map-manifest.json"
    ))
    .unwrap();
    let mut value = source;
    hash(&mut value);
    value
}
fn hash(v: &mut Value) {
    v.as_object_mut().unwrap().remove("content_hash");
    let geometry = digest(&canonical(&v["geometry"]));
    v["geometry_hash"] = Value::String(geometry);
    let h = digest(&canonical(v));
    v["content_hash"] = Value::String(h);
}

#[test]
fn canonical_place_and_provenance_values_are_closed_and_reviewed_when_verified() {
    for text in ["", "   ", "bad\nlabel", &"é".repeat(513)] {
        assert_eq!(
            Place::new(
                PLACE_A.parse().unwrap(),
                text.into(),
                PlaceKind::VirtualConcept,
                VerificationState::ConceptOnly,
                ClosureState::Open,
                "notes".into(),
                provenance(SourceKind::OriginalFixture, None, ReviewState::Unreviewed)
            )
            .unwrap_err(),
            PlaceError::Text
        );
    }
    assert!(PlaceId::from_uuid(Uuid::nil()).is_err());
    assert!(PlaceId::from_str(&PLACE_A.to_uppercase()).is_err());
    assert!(place(
        PLACE_A,
        PlaceKind::Physical,
        VerificationState::Verified,
        ClosureState::Open,
        Some("https://fixture.invalid/place"),
        ReviewState::Reviewed
    )
    .permits_new_physical_selection());
    assert!(!place(
        PLACE_A,
        PlaceKind::Physical,
        VerificationState::Unverified,
        ClosureState::Open,
        Some("https://fixture.invalid/place"),
        ReviewState::Unreviewed
    )
    .permits_new_physical_selection());
    let reviewed = place(
        PLACE_A,
        PlaceKind::Physical,
        VerificationState::Verified,
        ClosureState::Open,
        Some("https://fixture.invalid/place"),
        ReviewState::Reviewed,
    );
    assert!(ReviewedPlace::new(
        reviewed.clone(),
        ReviewStatus::Reviewed {
            current_closure: ClosureState::Open,
        }
    )
    .is_ok());
    assert!(ReviewedPlace::new(reviewed, ReviewStatus::Pending).is_err());
    assert!(!place(
        PLACE_A,
        PlaceKind::Physical,
        VerificationState::Verified,
        ClosureState::Closed,
        Some("https://fixture.invalid/place"),
        ReviewState::Reviewed
    )
    .permits_new_physical_selection());
    assert!(Place::new(
        PLACE_A.parse().unwrap(),
        "physical".into(),
        PlaceKind::Physical,
        VerificationState::Verified,
        ClosureState::Open,
        "notes".into(),
        provenance(SourceKind::PlaceReference, None, ReviewState::Reviewed)
    )
    .is_err());
}

#[test]
fn golden_manifest_parses_and_rehashed_invalid_geometry_or_place_relations_are_rejected() {
    let golden = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../../contracts/fixtures/assets/map-manifest.json");
    let source = std::fs::read(golden).unwrap();
    let bytes = canonical(&serde_json::from_slice::<Value>(&source).unwrap());
    let parsed = parse_stored_release(&bytes).unwrap();
    assert_eq!(parsed.map_id, "map_fixture");
    assert_eq!(parsed.places.len(), 1);
    for mutate in [
        |v: &mut Value| {
            v["geometry"]["collision_grid"][1][1] = json!(2);
        },
        |v: &mut Value| {
            v["geometry"]["interactions"][0]["place_id"] = json!(PLACE_B);
        },
        |v: &mut Value| {
            v["places"][0]["place_kind"] = json!("PHYSICAL");
        },
        |v: &mut Value| {
            v["geometry"]["spawns"][0]["cell_x"] = json!(0);
        },
    ] {
        let mut value = serde_json::from_slice::<Value>(&bytes).unwrap();
        mutate(&mut value);
        hash(&mut value);
        assert!(parse_stored_release(&canonical(&value)).is_err());
    }
    for mutate in [
        |v: &mut Value| {
            v["places"][0]["extra"] = json!(true);
        },
        |v: &mut Value| {
            v["geometry"]["collision_grid"][0][0] = json!(false);
        },
        |v: &mut Value| {
            v["geometry"]["spawns"][0]["extra"] = json!(true);
        },
        |v: &mut Value| {
            v["geometry"]["interactions"][0]["cell_x"] = json!(2);
        },
        |v: &mut Value| {
            v["geometry_hash"] = json!(format!("sha256:{}", "0".repeat(64)));
        },
    ] {
        let mut value = serde_json::from_slice::<Value>(&bytes).unwrap();
        mutate(&mut value);
        hash(&mut value);
        assert!(parse_stored_release(&canonical(&value)).is_err());
    }
    let mut pretty = serde_json::from_slice::<Value>(&bytes).unwrap();
    pretty["places"].as_array_mut().unwrap().clear();
    assert!(parse_stored_release(&serde_json::to_vec_pretty(&pretty).unwrap()).is_err());
    let mut broken_route = serde_json::from_slice::<Value>(&bytes).unwrap();
    broken_route["geometry"]["collision_grid"][2][1] = json!(1);
    broken_route["geometry"]["collision_grid"][3][1] = json!(1);
    hash(&mut broken_route);
    assert!(parse_stored_release(&canonical(&broken_route)).is_err());
    let mut unknown = serde_json::from_slice::<Value>(&bytes).unwrap();
    unknown["metadata"] = json!({});
    hash(&mut unknown);
    assert!(parse_stored_release(&canonical(&unknown)).is_err());
    let mut float = serde_json::from_slice::<Value>(&bytes).unwrap();
    float["geometry"]["width_cells"] = json!(5.0);
    hash(&mut float);
    assert!(parse_stored_release(&canonical(&float)).is_err());
}

#[test]
fn immutable_release_identity_checks_content_and_cannot_rebind_different_bytes() {
    let value = base_manifest();
    let bytes = canonical(&value);
    let manifest = parse_stored_release(&bytes).unwrap();
    let release = CatalogRelease::new(manifest, bytes.clone()).unwrap();
    assert!(release.verify_immutable(&bytes).is_ok());
    let mut changed = value;
    changed["places"][0]["label"] = json!("Changed");
    hash(&mut changed);
    assert_eq!(
        release.verify_immutable(&canonical(&changed)).unwrap_err(),
        PlaceError::ImmutableRelease
    );
    let mut stale = base_manifest();
    stale["places"][0]["label"] = json!("Changed");
    assert_eq!(
        CatalogRelease::new(
            parse_stored_release(&canonical(&stale)).unwrap(),
            canonical(&stale)
        )
        .unwrap_err(),
        PlaceError::ContentHash
    );
}
use std::path::Path;
