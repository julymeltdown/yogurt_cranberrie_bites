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
    assert_eq!(
        value["content_hash"],
        "sha256:21b91a4e02b04943100909d7a008f68cafb9e21b284f26b93ba09ddcc39a4dff"
    );
    hash(&mut value);
    value
}

#[test]
fn canonical_json_uses_codepoint_order_and_ascii_surrogate_escapes() {
    let value = json!({
        "😀": "astral",
        "é": "bmp",
        "a": "ascii",
        "\u{e000}": "private",
        "line": "é😀",
    });
    let bytes = canonical_json_bytes(&serde_json::to_vec(&value).unwrap()).unwrap();
    assert_eq!(
        bytes,
        br#"{"a":"ascii","line":"\u00e9\ud83d\ude00","\u00e9":"bmp","\ue000":"private","\ud83d\ude00":"astral"}"#
    );
    let parsed: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(parsed, value);
    let escaped = json!({"v": "\u{7f}\u{0}\n\t\\\""});
    assert_eq!(
        canonical_json_bytes(&serde_json::to_vec(&escaped).unwrap()).unwrap(),
        br#"{"v":"\u007f\u0000\n\t\\\""}"#
    );
    assert_eq!(
        serde_json::from_slice::<Value>(
            &canonical_json_bytes(&serde_json::to_vec(&escaped).unwrap()).unwrap()
        )
        .unwrap(),
        escaped
    );
}

fn hash(v: &mut Value) {
    v.as_object_mut().unwrap().remove("content_hash");
    let geometry = digest(&canonical(&v["geometry"]));
    v["geometry_hash"] = Value::String(geometry);
    let h = digest(&canonical(v));
    v["content_hash"] = Value::String(h);
}
fn assert_rehashed_invalid(mut value: Value) {
    hash(&mut value);
    let bytes = canonical(&value);
    assert!(
        parse_stored_release(&bytes).is_err(),
        "rehashed invalid manifest unexpectedly parsed: {}",
        String::from_utf8_lossy(&bytes)
    );
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
    assert!(PlaceId::from_str("00000000-0000-4000-8000-00000000000A").is_err());
    for invalid in [
        "relative/path",
        "https://example.invalid/a b",
        "https://example.invalid/%zz",
        "https://example.invalid/ref\n",
        "https://example.invalid/ref\u{7f}",
        "https://example.invalid/refé",
        "https://example.invalid/[docs]",
        "https://example.invalid/a?x=<y>",
        "https://example.invalid/a?x={y}",
        "urn:x<y",
        "urn:x{y}",
        "urn:x[y]",
        "",
    ] {
        assert!(Provenance::new(
            "fixture_source".into(),
            SourceKind::PlaceReference,
            "revision1".into(),
            Some(invalid.into()),
            "MCR fixture author".into(),
            "Synthetic metadata only".into(),
            ReviewState::Unreviewed,
            None,
            None,
            PhotoPermission::NotApplicable,
        )
        .is_err());
    }
    for valid in [
        "https://example.invalid/encoded%20space",
        "urn:mcr:format-fixture",
        "mailto:fixture@example.invalid",
        "file:///reviewed/source",
        "custom:/source",
    ] {
        assert!(Provenance::new(
            "fixture_source".into(),
            SourceKind::PlaceReference,
            "revision1".into(),
            Some(valid.into()),
            "MCR fixture author".into(),
            "Synthetic metadata only".into(),
            ReviewState::Unreviewed,
            None,
            None,
            PhotoPermission::NotApplicable,
        )
        .is_ok());
    }
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
        |v: &mut Value| {
            v["tilemap"]["resource_state"] = json!("PROVIDED");
            v["tilemap"]["path"] = json!("world/a.b.json");
            v["tilemap"]["sha256"] = json!("sha256:".to_owned() + &"0".repeat(64));
        },
    ] {
        let mut value = serde_json::from_slice::<Value>(&bytes).unwrap();
        mutate(&mut value);
        assert_rehashed_invalid(value);
    }
    let mut valid_path = serde_json::from_slice::<Value>(&bytes).unwrap();
    valid_path["tilemap"]["resource_state"] = json!("PROVIDED");
    valid_path["tilemap"]["path"] = json!("world/a-b_1.json");
    valid_path["tilemap"]["sha256"] = json!("sha256:".to_owned() + &"0".repeat(64));
    hash(&mut valid_path);
    assert!(parse_stored_release(&canonical(&valid_path)).is_ok());
    for mutate in [
        |v: &mut Value| {
            v["places"][0]["extra"] = json!(true);
        },
        |v: &mut Value| {
            v["geometry"]["spawns"][0]["extra"] = json!(true);
        },
        |v: &mut Value| {
            v["geometry"]["interactions"]
                .as_array_mut()
                .unwrap()
                .insert(
                    0,
                    json!({
                        "interaction_id": "inspect_second",
                        "place_id": PLACE_A,
                        "cell_x": 0,
                        "cell_y": 0
                    }),
                );
        },
    ] {
        let mut value = serde_json::from_slice::<Value>(&bytes).unwrap();
        mutate(&mut value);
        assert_rehashed_invalid(value);
    }
    let mut internal = serde_json::from_slice::<Value>(&bytes).unwrap();
    internal["geometry"]["collision_grid"][0][0] = json!(true);
    internal["geometry"]["collision_grid"][0][1] = json!(true);
    hash(&mut internal);
    assert!(parse_stored_release(&canonical(&internal)).is_err());
    let mut pretty = serde_json::from_slice::<Value>(&bytes).unwrap();
    pretty["places"].as_array_mut().unwrap().clear();
    assert!(parse_stored_release(&serde_json::to_vec_pretty(&pretty).unwrap()).is_err());
    let mut broken_route = serde_json::from_slice::<Value>(&bytes).unwrap();
    broken_route["geometry"]["collision_grid"][1][2] = json!(1);
    broken_route["geometry"]["collision_grid"][1][3] = json!(1);
    broken_route["geometry"]["collision_grid"][2][3] = json!(1);
    broken_route["geometry"]["collision_grid"][3][2] = json!(1);
    broken_route["geometry"]["collision_grid"][3][3] = json!(1);
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
