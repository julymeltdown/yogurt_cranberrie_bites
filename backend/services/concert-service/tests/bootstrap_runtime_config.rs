use concert_service::config::bootstrap_runtime::{settings, RELEASE_SETTINGS};
use std::collections::HashMap;

fn resolve(
    values: &[(&str, &str)],
) -> anyhow::Result<Option<(std::net::SocketAddr, [std::path::PathBuf; 6], bool)>> {
    let mut values: HashMap<&str, &str> = values.iter().copied().collect();
    for name in RELEASE_SETTINGS {
        values.entry(name).or_insert("/explicit/reviewed.json");
    }
    settings(|name| Ok(values.get(name).map(|v| v.to_string())))
}
#[test]
fn bootstrap_listener_requires_an_explicit_safe_opt_in() {
    assert!(resolve(&[]).unwrap().is_none());
    for address in ["", "bad", "127.0.0.1:50056", "0.0.0.0:0"] {
        assert!(resolve(&[("CONCERT_GRPC_ADDR", address)]).is_err());
    }
    assert!(
        resolve(&[("CONCERT_GRPC_ADDR", "0.0.0.0:50056")])
            .unwrap()
            .unwrap()
            .2
    );
    assert!(resolve(&[
        ("CONCERT_GRPC_ADDR", "0.0.0.0:50056"),
        ("CONCERT_RUNTIME_MODE", "local")
    ])
    .is_err());
    assert!(
        !resolve(&[
            ("CONCERT_GRPC_ADDR", "127.0.0.1:0"),
            ("CONCERT_RUNTIME_MODE", "local")
        ])
        .unwrap()
        .unwrap()
        .2
    );
    assert!(resolve(&[
        ("CONCERT_GRPC_ADDR", "127.0.0.1:0"),
        ("CONCERT_RUNTIME_MODE", "demo")
    ])
    .is_err());
}
#[test]
fn every_product_file_is_required_without_echoing_values() {
    for absent in RELEASE_SETTINGS {
        let error = settings(|name| {
            Ok(match name {
                "CONCERT_GRPC_ADDR" => Some("0.0.0.0:50056".into()),
                name if RELEASE_SETTINGS.contains(&name) && name != absent => {
                    Some("private-path".into())
                }
                _ => None,
            })
        })
        .err()
        .expect("missing product file refused");
        assert!(error.to_string().contains(absent));
        assert!(!error.to_string().contains("private-path"));
    }
}

fn spatial_paths() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    (
        root.join("backend/releases/reference-scene-v2/map-manifest.json"),
        root.join("backend/releases/reference-scene-v2/source-binding.json"),
        root.join("frontend/public"),
    )
}
struct SpatialScratch(std::path::PathBuf);
impl SpatialScratch {
    fn new() -> Self {
        use std::os::unix::fs::PermissionsExt;
        let path = std::env::temp_dir().join(format!("mcr-spatial-test-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&path).unwrap();
        std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o700)).unwrap();
        Self(path)
    }
    fn write(&self, name: &str, value: &serde_json::Value) -> std::path::PathBuf {
        let path = self.0.join(name);
        std::fs::write(&path, serde_json::to_vec(value).unwrap()).unwrap();
        path
    }
}
impl Drop for SpatialScratch {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.0).unwrap();
    }
}
#[test]
fn real_spatial_candidate_preserves_canonical_geometry_without_product_admission() {
    let (map, binding, root) = spatial_paths();
    let candidate =
        concert_service::bootstrap::spatial::load_candidate(&map, &binding, &root).unwrap();
    assert_eq!(candidate.catalog.map_id, "mcr_reference_scene_r3");
    let geometry = &candidate.catalog.manifest.geometry;
    assert_eq!(
        (
            geometry.width,
            geometry.height,
            geometry.tile_width,
            geometry.tile_height
        ),
        (48, 81, 16, 16)
    );
    assert_eq!(geometry.spawns.len(), 2);
    assert_eq!(geometry.interactions.len(), 4);
    assert_eq!(
        (candidate.scenery_bytes, candidate.scenery_pixels),
        (1_947_817, 2_043_904)
    );
    assert!(candidate.scenery_within_budget);
    assert!(candidate
        .catalog
        .manifest
        .places
        .iter()
        .all(|p| !p.permits_new_physical_selection()));
    let missing = root.join("no-product-bootstrap.json");
    assert!(
        concert_service::bootstrap::ProductReleaseBundle::from_source_bound_files(
            &missing,
            &missing,
            &map,
            [&missing, &missing, &missing],
            Default::default(),
            &root,
            &binding
        )
        .is_err()
    );
}
#[test]
fn spatial_binding_refuses_resealed_second_geometry() {
    use sha2::{Digest, Sha256};
    let (map, index, root) = spatial_paths();
    let scratch = SpatialScratch::new();
    let mut map: serde_json::Value = serde_json::from_slice(&std::fs::read(map).unwrap()).unwrap();
    let mut binding: serde_json::Value =
        serde_json::from_slice(&std::fs::read(index).unwrap()).unwrap();
    map["geometry"]["collision_id"] = "foreign_collision".into();
    let bytes =
        concert_domain::place::canonical_json_bytes(&serde_json::to_vec(&map["geometry"]).unwrap())
            .unwrap();
    map["geometry_hash"] = format!("sha256:{:x}", Sha256::digest(bytes)).into();
    map.as_object_mut().unwrap().remove("content_hash");
    let bytes =
        concert_domain::place::canonical_json_bytes(&serde_json::to_vec(&map).unwrap()).unwrap();
    map["content_hash"] = format!("sha256:{:x}", Sha256::digest(bytes)).into();
    binding["geometry_hash"] = map["geometry_hash"].clone();
    binding["map_content_hash"] = map["content_hash"].clone();
    let map = scratch.write("map.json", &map);
    let index = scratch.write("binding.json", &binding);
    assert!(concert_service::bootstrap::spatial::load_candidate(&map, &index, &root).is_err());
}
#[test]
fn spatial_actual_byte_and_dimension_pins_fail_closed() {
    let (map, index, root) = spatial_paths();
    let scratch = SpatialScratch::new();
    let original: serde_json::Value =
        serde_json::from_slice(&std::fs::read(index).unwrap()).unwrap();
    for field in ["sha256", "bytes", "width_px", "height_px"] {
        let mut binding = original.clone();
        binding["runtime_resources"][0][field] = if field == "sha256" {
            serde_json::json!("sha256:invalid")
        } else {
            serde_json::json!(1)
        };
        let index = scratch.write("binding.json", &binding);
        assert!(
            concert_service::bootstrap::spatial::load_candidate(&map, &index, &root).is_err(),
            "{field}"
        );
    }
}
#[test]
fn spatial_index_rejects_unknown_duplicate_and_traversal_inputs() {
    let (map, index, root) = spatial_paths();
    let scratch = SpatialScratch::new();
    let raw = std::fs::read_to_string(index).unwrap();
    let original: serde_json::Value = serde_json::from_str(&raw).unwrap();
    let mut invalid = original.clone();
    invalid["extra"] = true.into();
    let index = scratch.write("binding.json", &invalid);
    assert!(concert_service::bootstrap::spatial::load_candidate(&map, &index, &root).is_err());
    std::fs::write(&index, raw.replacen('{', "{\"version\":1,", 1)).unwrap();
    assert!(concert_service::bootstrap::spatial::load_candidate(&map, &index, &root).is_err());
    for path in [
        "../outside.json",
        "/absolute.json",
        "world/../outside.json",
        "https://example.invalid/source.json",
    ] {
        let mut invalid = original.clone();
        invalid["geometry_source"]["path"] = path.into();
        let index = scratch.write("binding.json", &invalid);
        assert!(
            concert_service::bootstrap::spatial::load_candidate(&map, &index, &root).is_err(),
            "{path}"
        );
    }
}
#[test]
fn spatial_binding_rejects_symlinked_source() {
    let (map, index, root) = spatial_paths();
    let scratch = SpatialScratch::new();
    let binding: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&index).unwrap()).unwrap();
    let path = binding["geometry_source"]["path"].as_str().unwrap();
    let link = scratch.0.join(path);
    std::fs::create_dir_all(link.parent().unwrap()).unwrap();
    std::os::unix::fs::symlink(root.join(path), link).unwrap();
    assert!(concert_service::bootstrap::spatial::load_candidate(&map, &index, &scratch.0).is_err());
}
