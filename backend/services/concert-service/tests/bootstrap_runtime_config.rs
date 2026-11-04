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
        (5_479_473, 6_131_712)
    );
    assert!(candidate.scenery_within_budget);
    assert_eq!(candidate.scenes.len(), 3);
    assert!(candidate.scenes.iter().all(|scene| scene.within_budget));
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
        binding["scenes"][0]["runtime_resources"][0][field] = if field == "sha256" {
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
fn spatial_binding_refuses_resealed_place_qualification_changes() {
    use sha2::{Digest, Sha256};
    let (map, index, root) = spatial_paths();
    let scratch = SpatialScratch::new();
    let mut map: serde_json::Value = serde_json::from_slice(&std::fs::read(map).unwrap()).unwrap();
    let mut binding: serde_json::Value =
        serde_json::from_slice(&std::fs::read(index).unwrap()).unwrap();
    map["places"][0]["label"] = "Unattributed changed place".into();
    map.as_object_mut().unwrap().remove("content_hash");
    let bytes =
        concert_domain::place::canonical_json_bytes(&serde_json::to_vec(&map).unwrap()).unwrap();
    map["content_hash"] = format!("sha256:{:x}", Sha256::digest(bytes)).into();
    binding["map_content_hash"] = map["content_hash"].clone();
    let map = scratch.write("map.json", &map);
    let index = scratch.write("binding.json", &binding);
    assert!(concert_service::bootstrap::spatial::load_candidate(&map, &index, &root).is_err());
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
    std::fs::write(&index, raw.replacen('{', "{\"version\":2,", 1)).unwrap();
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
#[test]
fn spatial_binding_rejects_duplicate_or_aliased_themes() {
    let (map, index, root) = spatial_paths();
    let scratch = SpatialScratch::new();
    let original: serde_json::Value =
        serde_json::from_slice(&std::fs::read(index).unwrap()).unwrap();
    let mut duplicate = original.clone();
    duplicate["scenes"][1]["theme_id"] = duplicate["scenes"][0]["theme_id"].clone();
    let index = scratch.write("binding.json", &duplicate);
    assert!(concert_service::bootstrap::spatial::load_candidate(&map, &index, &root).is_err());
    let mut aliased = original;
    let theme = aliased["scenes"][1]["theme_id"].clone();
    aliased["scenes"][1] = aliased["scenes"][0].clone();
    aliased["scenes"][1]["theme_id"] = theme;
    let index = scratch.write("binding.json", &aliased);
    assert!(concert_service::bootstrap::spatial::load_candidate(&map, &index, &root).is_err());
}
#[test]
fn spatial_binding_rejects_actual_runtime_file_tampering() {
    let (map, index, root) = spatial_paths();
    let scratch = SpatialScratch::new();
    let binding: serde_json::Value =
        serde_json::from_slice(&std::fs::read(&index).unwrap()).unwrap();
    copy_spatial_sources(&binding, &root, &scratch.0);
    concert_service::bootstrap::spatial::load_candidate(&map, &index, &scratch.0).unwrap();
    let path = scratch.0.join(
        binding["scenes"][0]["runtime_resources"][1]["path"]
            .as_str()
            .unwrap(),
    );
    let mut actual = std::fs::read(&path).unwrap();
    let last = actual.len() - 1;
    actual[last] ^= 1;
    std::fs::write(path, actual).unwrap();
    assert!(concert_service::bootstrap::spatial::load_candidate(&map, &index, &scratch.0).is_err());
}

fn copy_spatial_sources(
    binding: &serde_json::Value,
    root: &std::path::Path,
    destination: &std::path::Path,
) {
    let mut pins = vec![&binding["geometry_source"], &binding["tilemap_source"]];
    for scene in binding["scenes"].as_array().unwrap() {
        pins.push(&scene["scenery_source"]);
        pins.extend(scene["scene_sources"].as_array().unwrap());
        pins.extend(scene["runtime_resources"].as_array().unwrap());
    }
    for pin in pins {
        let path = pin["path"].as_str().unwrap();
        let target = destination.join(path);
        std::fs::create_dir_all(target.parent().unwrap()).unwrap();
        std::fs::copy(root.join(path), target).unwrap();
    }
}
fn quality_v6_paths() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let (map, index, root) = spatial_paths();
    (
        map.parent().unwrap().join("quality-v6/map-manifest.json"),
        index
            .parent()
            .unwrap()
            .join("quality-v6/source-binding.json"),
        root,
    )
}
fn quality_v8_paths() -> (std::path::PathBuf, std::path::PathBuf, std::path::PathBuf) {
    let (map, index, root) = spatial_paths();
    (
        map.parent().unwrap().join("quality-v8/map-manifest.json"),
        index
            .parent()
            .unwrap()
            .join("quality-v8/source-binding.json"),
        root,
    )
}
#[test]
fn quality_v8_binds_actual_sources_and_only_adds_72_blocked_cells() {
    let (map, index, root) = quality_v8_paths();
    let candidate =
        concert_service::bootstrap::spatial::load_candidate(&map, &index, &root).unwrap();
    let (map, index, _) = quality_v6_paths();
    let old = concert_service::bootstrap::spatial::load_candidate(&map, &index, &root).unwrap();
    assert_eq!(candidate.catalog.manifest.map_revision, 5);
    assert_eq!(candidate.catalog.map_id, old.catalog.map_id);
    assert_eq!(
        candidate.catalog.manifest.geometry_hash,
        "sha256:0448d5086dc251c6a7aed8c136dbbbf751f95038ea1f7f98e3e40fe6e8067e6f"
    );
    assert_eq!(
        (candidate.scenery_bytes, candidate.scenery_pixels),
        (5_214_642, 6_131_712)
    );
    assert!(candidate.scenery_within_budget);
    assert_eq!(candidate.scenes.len(), 3);
    let before = &old.catalog.manifest.geometry;
    let after = &candidate.catalog.manifest.geometry;
    assert_eq!(before.terrain, after.terrain);
    assert_eq!(before.overdraw, after.overdraw);
    assert_eq!(before.spawns, after.spawns);
    assert_eq!(before.interactions, after.interactions);
    assert_eq!(before.default_spawn_id, after.default_spawn_id);
    let changes: Vec<_> = before
        .collision
        .iter()
        .flatten()
        .zip(after.collision.iter().flatten())
        .filter(|(a, b)| a != b)
        .map(|(a, b)| (*a, *b))
        .collect();
    assert_eq!(changes, vec![(0, 1); 72]);
    assert_eq!(
        candidate.catalog.manifest.places,
        old.catalog.manifest.places
    );
    assert!(candidate
        .catalog
        .manifest
        .places
        .iter()
        .all(|p| !p.permits_new_physical_selection()));
}
#[test]
fn quality_v8_rejects_valid_map4_scenery_under_new_collision_geometry() {
    let (map, index, root) = quality_v8_paths();
    let (_, old_index, _) = quality_v6_paths();
    let mut binding: serde_json::Value =
        serde_json::from_slice(&std::fs::read(index).unwrap()).unwrap();
    let old: serde_json::Value =
        serde_json::from_slice(&std::fs::read(old_index).unwrap()).unwrap();
    binding["scenes"][0]["scenery_source"] = old["scenes"][0]["scenery_source"].clone();
    let scratch = SpatialScratch::new();
    copy_spatial_sources(&binding, &root, &scratch.0);
    let index = scratch.write("binding.json", &binding);
    assert!(concert_service::bootstrap::spatial::load_candidate(&map, &index, &scratch.0).is_err());
}
#[test]
fn quality_v6_binds_new_geometry_and_distinct_three_era_bytes() {
    let (map, index, root) = quality_v6_paths();
    let candidate =
        concert_service::bootstrap::spatial::load_candidate(&map, &index, &root).unwrap();
    assert_eq!(candidate.catalog.manifest.map_revision, 4);
    assert_eq!(
        candidate.catalog.manifest.geometry_hash,
        "sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f"
    );
    assert_eq!(
        (candidate.scenery_bytes, candidate.scenery_pixels),
        (5_147_757, 6_131_712)
    );
    assert!(candidate.scenery_within_budget);
    assert_eq!(candidate.scenes.len(), 3);
    assert!(candidate
        .catalog
        .manifest
        .places
        .iter()
        .all(|p| !p.permits_new_physical_selection()));
}
#[test]
fn spatial_rejects_valid_old_layout_in_new_geometry_binding() {
    let (map, index, root) = quality_v6_paths();
    let (_, old_index, _) = spatial_paths();
    let scratch = SpatialScratch::new();
    let mut binding: serde_json::Value =
        serde_json::from_slice(&std::fs::read(index).unwrap()).unwrap();
    let old: serde_json::Value =
        serde_json::from_slice(&std::fs::read(old_index).unwrap()).unwrap();
    let old_layout = old["scenes"][0]["scene_sources"]
        .as_array()
        .unwrap()
        .iter()
        .find(|pin| pin["path"].as_str().unwrap().ends_with("/layout.json"))
        .unwrap();
    let layout = binding["scenes"][0]["scene_sources"]
        .as_array_mut()
        .unwrap()
        .iter_mut()
        .find(|pin| pin["path"].as_str().unwrap().ends_with("/layout.json"))
        .unwrap();
    *layout = old_layout.clone();
    copy_spatial_sources(&binding, &root, &scratch.0);
    let index = scratch.write("binding.json", &binding);
    assert!(concert_service::bootstrap::spatial::load_candidate(&map, &index, &scratch.0).is_err());
}
#[test]
fn spatial_resealed_layout_must_retain_source_footprints_and_scenery_projection() {
    use sha2::{Digest, Sha256};
    let (map, index, root) = quality_v6_paths();
    let original: serde_json::Value =
        serde_json::from_slice(&std::fs::read(index).unwrap()).unwrap();
    for case in [
        "geometry",
        "footprint",
        "destination",
        "frame",
        "missing",
        "duplicate",
        "no_layout",
    ] {
        let scratch = SpatialScratch::new();
        let mut binding = original.clone();
        copy_spatial_sources(&binding, &root, &scratch.0);
        let pins = binding["scenes"][0]["scene_sources"]
            .as_array_mut()
            .unwrap();
        let position = pins
            .iter()
            .position(|p| p["path"].as_str().unwrap().ends_with("/layout.json"))
            .unwrap();
        let path = scratch.0.join(pins[position]["path"].as_str().unwrap());
        let mut layout: serde_json::Value =
            serde_json::from_slice(&std::fs::read(&path).unwrap()).unwrap();
        match case {
            "geometry" => layout["geometry_hash"] = "sha256:foreign".into(),
            "footprint" => layout["placements"][0]["footprint"][0][0] = 0.into(),
            "destination" => layout["placements"][0]["x"] = 161.into(),
            "frame" => layout["placements"][0]["frame"] = "missing_frame".into(),
            "missing" => {
                layout["placements"].as_array_mut().unwrap().pop();
            }
            "duplicate" => layout["placements"][1] = layout["placements"][0].clone(),
            "no_layout" => {
                pins.remove(position);
            }
            _ => unreachable!(),
        }
        if case != "no_layout" {
            let raw = serde_json::to_vec(&layout).unwrap();
            std::fs::write(&path, &raw).unwrap();
            pins[position]["sha256"] = format!("sha256:{:x}", Sha256::digest(&raw)).into();
            pins[position]["bytes"] = (raw.len() as u64).into();
        }
        let index = scratch.write("binding.json", &binding);
        assert!(
            concert_service::bootstrap::spatial::load_candidate(&map, &index, &scratch.0).is_err(),
            "{case}"
        );
    }
}
