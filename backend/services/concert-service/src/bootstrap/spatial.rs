//! Source-byte binding for one canonical spatial candidate. Candidate inspection
//! never grants product admission or changes event-current map state.
use super::BootstrapServiceError as Error;
use concert_application::bootstrap::ValidatedProductRelease;
use concert_domain::place::{canonical_json_bytes, parse_stored_release, CatalogRelease};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Component, Path},
};

const MAX_FILE: u64 = 8 * 1024 * 1024;
const SCENERY_BUDGET: u64 = 2_097_152;
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FilePin {
    path: String,
    sha256: String,
    bytes: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ImagePin {
    path: String,
    sha256: String,
    bytes: u64,
    width_px: u64,
    height_px: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Binding {
    format: String,
    version: u32,
    source_commit: String,
    art_commit: String,
    tilemap_commit: String,
    map_id: String,
    map_revision: i64,
    map_content_hash: String,
    geometry_hash: String,
    geometry_source: FilePin,
    tilemap_source: FilePin,
    scenes: Vec<SceneBinding>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct SceneBinding {
    theme_id: String,
    #[serde(default, deserialize_with = "display_profile")]
    display_profile: Option<String>,
    scenery_source: FilePin,
    scene_sources: Vec<FilePin>,
    runtime_resources: Vec<ImagePin>,
}
// Absent is legacy; an explicit null must not expand the closed version-2 shape.
fn display_profile<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<String>, D::Error> {
    String::deserialize(deserializer).map(Some)
}
#[derive(Deserialize)]
struct ProposalGeometry {
    geometry: Value,
    geometry_hash: String,
    places: Value,
    placements: Vec<SourcePlacement>,
}
#[derive(Deserialize)]
struct SourcePlacement {
    id: String,
    footprint: Value,
}
#[derive(Deserialize)]
struct SourceLayout {
    geometry_hash: String,
    placements: Vec<LayoutPlacement>,
    frames: std::collections::BTreeMap<String, Value>,
}
#[derive(Deserialize)]
struct LayoutPlacement {
    id: String,
    frame: String,
    x: i64,
    y: i64,
    width: i64,
    height: i64,
    depth_y: i64,
    footprint: Value,
}

#[derive(Clone, Debug)]
pub struct SourceBoundSpatialCandidate {
    pub catalog: CatalogRelease,
    pub source_commit: String,
    pub art_commit: String,
    pub scenes: Vec<SpatialSceneBudget>,
    /// Sum across theme scenes, including a shared ground once per scene.
    pub scenery_bytes: u64,
    pub scenery_pixels: u64,
    pub scenery_within_budget: bool,
}
#[derive(Clone, Debug)]
pub struct SpatialSceneBudget {
    pub theme_id: String,
    pub bytes: u64,
    pub pixels: u64,
    pub within_budget: bool,
}
fn ensure(value: bool) -> Result<(), Error> {
    if value {
        Ok(())
    } else {
        Err(Error::ReleaseUnavailable)
    }
}
fn invalid(_: impl std::fmt::Debug) -> Error {
    Error::ReleaseUnavailable
}
fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}
fn revision(value: &str) -> bool {
    value.len() == 40
        && value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn read(path: &Path) -> Result<Vec<u8>, Error> {
    let file = fs::File::open(path).map_err(invalid)?;
    ensure(file.metadata().map_err(invalid)?.is_file())?;
    let mut bytes = Vec::new();
    file.take(MAX_FILE + 1)
        .read_to_end(&mut bytes)
        .map_err(invalid)?;
    ensure(!bytes.is_empty() && bytes.len() as u64 <= MAX_FILE)?;
    Ok(bytes)
}
fn resource(root: &Path, path: &str) -> Result<Vec<u8>, Error> {
    ensure(
        !path.is_empty()
            && path.len() <= 512
            && path
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/_-.".contains(&b))
            && path
                .split('/')
                .all(|part| !part.is_empty() && part != "." && part != ".."),
    )?;
    let root = root.canonicalize().map_err(invalid)?;
    let mut location = root.clone();
    for component in Path::new(path).components() {
        let Component::Normal(part) = component else {
            return Err(Error::ReleaseUnavailable);
        };
        location.push(part);
        ensure(
            !fs::symlink_metadata(&location)
                .map_err(invalid)?
                .file_type()
                .is_symlink(),
        )?;
    }
    ensure(location.canonicalize().map_err(invalid)?.starts_with(&root))?;
    read(&location)
}
fn verify_file(root: &Path, pin: &FilePin) -> Result<Vec<u8>, Error> {
    let bytes = resource(root, &pin.path)?;
    ensure(bytes.len() as u64 == pin.bytes && digest(&bytes) == pin.sha256)?;
    Ok(bytes)
}
fn dimensions(bytes: &[u8]) -> Result<(u64, u64), Error> {
    ensure(
        bytes.len() >= 33
            && &bytes[..8] == b"\x89PNG\r\n\x1a\n"
            && &bytes[8..16] == b"\0\0\0\rIHDR",
    )?;
    let w = u32::from_be_bytes(bytes[16..20].try_into().map_err(invalid)?) as u64;
    let h = u32::from_be_bytes(bytes[20..24].try_into().map_err(invalid)?) as u64;
    ensure(w > 0 && h > 0)?;
    Ok((w, h))
}
fn load(
    map: &Path,
    index: &Path,
    root: &Path,
) -> Result<(SourceBoundSpatialCandidate, Binding), Error> {
    let index = read(index)?;
    super::json::parse(&index).map_err(invalid)?;
    let binding: Binding = serde_json::from_slice(&index).map_err(invalid)?;
    ensure(
        binding.format == "MCR_SOURCE_BOUND_SPATIAL_RELEASE"
            && matches!(binding.version, 2 | 3)
            && revision(&binding.source_commit)
            && revision(&binding.art_commit)
            && revision(&binding.tilemap_commit)
            && !binding.scenes.is_empty()
            && binding.scenes.len() <= 3,
    )?;
    let display_count = binding
        .scenes
        .iter()
        .filter(|scene| scene.display_profile.is_some())
        .count();
    ensure(
        (binding.version == 2 && display_count == 0)
            || (binding.version == 3 && display_count == 1),
    )?;
    for scene in &binding.scenes {
        if let Some(profile) = &scene.display_profile {
            ensure(
                scene.theme_id == "PARADE"
                    && profile == super::photo2::PROFILE
                    && scene.scenery_source.path
                        == format!("{}asset-interface.json", super::photo2::PREFIX),
            )?;
            let registered = super::photo2::registration(&scene.scenery_source.sha256)?;
            ensure(binding.art_commit == registered.art_commit)?;
        }
    }
    let map_bytes = read(map)?;
    super::json::parse(&map_bytes).map_err(invalid)?;
    let canonical = canonical_json_bytes(&map_bytes).map_err(invalid)?;
    let manifest = parse_stored_release(&canonical).map_err(invalid)?;
    let catalog = CatalogRelease::new(manifest, canonical).map_err(invalid)?;
    ensure(
        catalog.map_id == binding.map_id
            && catalog.manifest.map_revision == binding.map_revision
            && catalog.manifest.content_hash == binding.map_content_hash
            && catalog.manifest.geometry_hash == binding.geometry_hash,
    )?;
    // Artist documents may contain fractional illustration positions outside
    // geometry. The typed projection rejects duplicate geometry/hash fields;
    // exact whole-document bytes are independently pinned before projection.
    let source = verify_file(root, &binding.geometry_source)?;
    let proposal: ProposalGeometry = serde_json::from_slice(&source).map_err(invalid)?;
    let map_value: Value = serde_json::from_slice(&catalog.canonical_bytes).map_err(invalid)?;
    let geometry = canonical_json_bytes(&serde_json::to_vec(&proposal.geometry).map_err(invalid)?)
        .map_err(invalid)?;
    ensure(
        proposal.geometry == map_value["geometry"]
            && proposal.places == map_value["places"]
            && proposal.geometry_hash == binding.geometry_hash
            && digest(&geometry) == binding.geometry_hash,
    )?;
    let tiled =
        super::json::parse(&verify_file(root, &binding.tilemap_source)?).map_err(invalid)?;
    ensure(
        map_value["tilemap"]["resource_state"] == "PROVIDED"
            && map_value["tilemap"]["path"] == binding.tilemap_source.path
            && map_value["tilemap"]["sha256"] == binding.tilemap_source.sha256,
    )?;
    let mut names = std::collections::BTreeSet::new();
    let mut visual_sets = std::collections::BTreeSet::new();
    let mut scenes = Vec::new();
    let (mut bytes, mut pixels) = (0u64, 0u64);
    for scene in &binding.scenes {
        ensure(
            matches!(scene.theme_id.as_str(), "PARADE" | "REVENGE" | "DANGER")
                && names.insert(scene.theme_id.clone())
                && !scene.scene_sources.is_empty()
                && scene.scene_sources.len() <= 32
                && scene.runtime_resources.len() >= 2
                && scene.runtime_resources.len() <= 32,
        )?;
        let mut atlas_hashes = scene
            .runtime_resources
            .iter()
            .skip(1)
            .map(|pin| pin.sha256.clone())
            .collect::<Vec<_>>();
        atlas_hashes.sort();
        ensure(visual_sets.insert(atlas_hashes))?;
        let budget = verify_scene(scene, root, &map_value, &tiled, &binding, &proposal)?;
        bytes = bytes
            .checked_add(budget.bytes)
            .ok_or(Error::ReleaseUnavailable)?;
        pixels = pixels
            .checked_add(budget.pixels)
            .ok_or(Error::ReleaseUnavailable)?;
        scenes.push(budget);
    }
    let within_budget = scenes.iter().all(|scene| scene.within_budget);
    let candidate = SourceBoundSpatialCandidate {
        catalog,
        source_commit: binding.source_commit.clone(),
        art_commit: binding.art_commit.clone(),
        scenes,
        scenery_bytes: bytes,
        scenery_pixels: pixels,
        scenery_within_budget: within_budget,
    };
    Ok((candidate, binding))
}
fn verify_scene(
    scene: &SceneBinding,
    root: &Path,
    map: &Value,
    tiled: &Value,
    binding: &Binding,
    proposal: &ProposalGeometry,
) -> Result<SpatialSceneBudget, Error> {
    let mut paths = std::collections::BTreeSet::new();
    for pin in [
        &binding.geometry_source,
        &binding.tilemap_source,
        &scene.scenery_source,
    ] {
        ensure(paths.insert(pin.path.clone()))?;
    }
    let raw = verify_file(root, &scene.scenery_source)?;
    let display = scene.display_profile.is_some();
    let scenery = if display {
        // The independently pinned interface contains fractional display mesh coordinates.
        // It is never parsed as an integer-only C03 document or native Tiled texture.
        let interface: Value = serde_json::from_slice(&raw).map_err(invalid)?;
        super::photo2::interface_scenery(&interface, &scene.scenery_source.sha256, map)?
    } else {
        super::json::parse(&raw).map_err(invalid)?
    };
    if !display {
        validate_tiled(
            tiled,
            &map["geometry"],
            &[serde_json::json!({"texture": {
                "width_px": scenery["ground"]["width_px"], "height_px": scenery["ground"]["height_px"],
                "path": scenery["ground"]["path"]
            }})],
        )?;
    }
    let mut layout = None;
    let mut provenance = false;
    for pin in &scene.scene_sources {
        ensure(paths.insert(pin.path.clone()))?;
        let raw = verify_file(root, pin)?;
        if display {
            if pin.path == format!("{}source-provenance.json", super::photo2::PREFIX) {
                ensure(
                    !provenance
                        && pin.sha256
                            == super::photo2::registration(&scene.scenery_source.sha256)?
                                .provenance_hash,
                )?;
                // Exact qualified bytes, not self-reported REVIEWED labels, establish this source.
                let _: Value = serde_json::from_slice(&raw).map_err(invalid)?;
                provenance = true;
            }
        } else if pin.path.ends_with("/layout.json") {
            ensure(layout.is_none())?;
            layout = Some(serde_json::from_slice::<SourceLayout>(&raw).map_err(invalid)?);
        }
    }
    if display {
        ensure(provenance && proposal.placements.len() == 22)?;
        let expected = proposal
            .placements
            .iter()
            .map(|p| p.id.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let actual = scenery["objects"]
            .as_array()
            .ok_or(Error::ReleaseUnavailable)?
            .iter()
            .map(|v| v["id"].as_str().ok_or(Error::ReleaseUnavailable))
            .collect::<Result<std::collections::BTreeSet<_>, _>>()?;
        ensure(expected.len() == 22 && expected == actual)?;
    } else {
        verify_layout(
            layout.as_ref().ok_or(Error::ReleaseUnavailable)?,
            proposal,
            &scenery,
        )?;
    }
    let descriptors = std::iter::once(&scenery["ground"])
        .chain(
            scenery["atlases"]
                .as_array()
                .ok_or(Error::ReleaseUnavailable)?
                .iter(),
        )
        .collect::<Vec<_>>();
    ensure(descriptors.len() == scene.runtime_resources.len())?;
    let (mut bytes, mut pixels) = (0u64, 0u64);
    for (pin, descriptor) in scene.runtime_resources.iter().zip(descriptors) {
        ensure(
            paths.insert(pin.path.clone())
                && descriptor["path"] == pin.path
                && descriptor["sha256"] == pin.sha256
                && descriptor["bytes"] == pin.bytes
                && descriptor["width_px"] == pin.width_px
                && descriptor["height_px"] == pin.height_px,
        )?;
        let raw = verify_file(
            root,
            &FilePin {
                path: pin.path.clone(),
                sha256: pin.sha256.clone(),
                bytes: pin.bytes,
            },
        )?;
        ensure(dimensions(&raw)? == (pin.width_px, pin.height_px))?;
        bytes = bytes
            .checked_add(pin.bytes)
            .ok_or(Error::ReleaseUnavailable)?;
        pixels = pixels
            .checked_add(
                pin.width_px
                    .checked_mul(pin.height_px)
                    .ok_or(Error::ReleaseUnavailable)?,
            )
            .ok_or(Error::ReleaseUnavailable)?;
    }
    Ok(SpatialSceneBudget {
        theme_id: scene.theme_id.clone(),
        bytes,
        pixels,
        within_budget: bytes <= SCENERY_BUDGET && pixels <= SCENERY_BUDGET,
    })
}
// Exact hashes alone cannot prevent a binding assembled from valid sources of
// different generations. Tie the scenery's integer projection to its pinned
// layout and that layout's collision footprints to the canonical proposal.
fn verify_layout(
    layout: &SourceLayout,
    proposal: &ProposalGeometry,
    scenery: &Value,
) -> Result<(), Error> {
    let objects = scenery["objects"]
        .as_array()
        .ok_or(Error::ReleaseUnavailable)?;
    ensure(
        layout.geometry_hash == proposal.geometry_hash
            && !layout.placements.is_empty()
            && layout.placements.len() == proposal.placements.len()
            && objects.len() == layout.placements.len(),
    )?;
    let mut source_ids = std::collections::BTreeSet::new();
    for source in &proposal.placements {
        ensure(source_ids.insert(&source.id))?;
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut object_ids = std::collections::BTreeSet::new();
    for object in objects {
        ensure(object_ids.insert(object["id"].as_str().ok_or(Error::ReleaseUnavailable)?))?;
    }
    for placement in &layout.placements {
        ensure(ids.insert(&placement.id))?;
        let source = proposal
            .placements
            .iter()
            .find(|p| p.id == placement.id)
            .ok_or(Error::ReleaseUnavailable)?;
        let object = objects
            .iter()
            .find(|o| o["id"] == placement.id)
            .ok_or(Error::ReleaseUnavailable)?;
        ensure(source.footprint == placement.footprint && source.footprint.is_array())?;
        ensure(
            object["frame"]
                == *layout
                    .frames
                    .get(&placement.frame)
                    .ok_or(Error::ReleaseUnavailable)?,
        )?;
        for (key, expected) in [
            ("x", placement.x),
            ("y", placement.y),
            ("width", placement.width),
            ("height", placement.height),
            ("depth_y", placement.depth_y),
        ] {
            ensure(object[key].as_i64() == Some(expected))?;
        }
    }
    Ok(())
}
/// Inspects real source/resource bytes and the owning canonical map domain.
/// This type deliberately does not contain a ValidatedProductRelease.
pub fn load_candidate(
    map: &Path,
    source_binding: &Path,
    resource_root: &Path,
) -> Result<SourceBoundSpatialCandidate, Error> {
    load(map, source_binding, resource_root).map(|(candidate, _)| candidate)
}

/// Additional source/byte fence after the existing complete product policy.
/// Missing skins, unreviewed provenance and unavailable resources are never waived.
pub(super) fn validate_product(
    release: &ValidatedProductRelease,
    map: &Path,
    index: &Path,
    root: &Path,
) -> Result<(), Error> {
    let (candidate, binding) = load(map, index, root)?;
    ensure(candidate.scenery_within_budget)?;
    let doc: Value = serde_json::from_slice(release.release().bootstrap_json()).map_err(invalid)?;
    let expected: Value =
        serde_json::from_slice(&candidate.catalog.canonical_bytes).map_err(invalid)?;
    ensure(doc["map_manifest"] == expected)?;
    let mut textures = if doc["avatar_manifest"]["contract_version"] == "2.0.0" {
        // Avatar-only consumer adaptation: the closed reviewed family has a
        // pinned atlas Resource, not the legacy atlas.texture wire shape.
        // Revalidate its typed proof and every exact resource from the same
        // repository layout; map/theme/source validation below is unchanged.
        concert_domain::avatar_motion_v2::reviewed::VisuallyReviewedCatalog::from_manifest(
            &doc["avatar_manifest"],
        )
        .map_err(invalid)?;
        let public = fs::canonicalize(root).map_err(invalid)?;
        let repository = public
            .parent()
            .and_then(Path::parent)
            .ok_or(Error::ReleaseUnavailable)?;
        super::release::validate_motion_files(
            &repository
                .join("backend/releases/selected-character-motion-v2/reviewed/manifest.json"),
            &doc["avatar_manifest"],
        )?;
        Vec::new()
    } else {
        vec![&doc["avatar_manifest"]["atlas"]["texture"]]
    };
    let themes = doc["theme_manifests"]
        .as_array()
        .ok_or(Error::ReleaseUnavailable)?;
    ensure(binding.scenes.len() == 3 && themes.len() == 3)?;
    for scene in &binding.scenes {
        let raw = verify_file(root, &scene.scenery_source)?;
        let supplied = if scene.display_profile.is_some() {
            let interface: Value = serde_json::from_slice(&raw).map_err(invalid)?;
            super::photo2::interface_scenery(&interface, &scene.scenery_source.sha256, &expected)?
        } else {
            super::json::parse(&raw).map_err(invalid)?
        };
        let theme = themes
            .iter()
            .find(|theme| theme["theme_id"] == scene.theme_id)
            .ok_or(Error::ReleaseUnavailable)?;
        ensure(theme["scenery"] == supplied)?;
        if scene.display_profile.is_some() {
            super::photo2::bind_theme(theme, &expected, &scene.scenery_source.sha256)?;
        }
        textures.push(&theme["texture"]);
    }
    for texture in textures {
        ensure(texture["image_state"] == "PROVIDED")?;
        let raw = resource(
            root,
            texture["path"].as_str().ok_or(Error::ReleaseUnavailable)?,
        )?;
        verify_texture(texture, &raw)?;
    }
    let tiled = &doc["map_manifest"]["tilemap"];
    ensure(tiled["resource_state"] == "PROVIDED")?;
    let raw = resource(
        root,
        tiled["path"].as_str().ok_or(Error::ReleaseUnavailable)?,
    )?;
    ensure(tiled["sha256"].as_str() == Some(digest(&raw).as_str()))?;
    let tiled = super::json::parse(&raw).map_err(invalid)?;
    validate_tiled(&tiled, &doc["map_manifest"]["geometry"], themes)?;
    Ok(())
}
fn verify_texture(texture: &Value, raw: &[u8]) -> Result<(), Error> {
    let (w, h) = dimensions(raw)?;
    ensure(
        texture["sha256"].as_str() == Some(digest(raw).as_str())
            && texture["width_px"].as_u64() == Some(w)
            && texture["height_px"].as_u64() == Some(h),
    )
}
fn closed(value: &Value, fields: &[&str]) -> Result<(), Error> {
    ensure(
        value
            .as_object()
            .ok_or(Error::ReleaseUnavailable)?
            .keys()
            .all(|key| fields.contains(&key.as_str())),
    )
}
fn validate_tiled(tiled: &Value, geometry: &Value, themes: &[Value]) -> Result<(), Error> {
    closed(
        tiled,
        &[
            "type",
            "orientation",
            "renderorder",
            "infinite",
            "width",
            "height",
            "tilewidth",
            "tileheight",
            "layers",
            "tilesets",
            "version",
            "tiledversion",
        ],
    )?;
    ensure(
        tiled["tiledversion"]
            .as_str()
            .is_some_and(|v| !v.is_empty() && v.len() <= 32),
    )?;
    ensure(
        tiled["type"] == "map"
            && tiled["orientation"] == "orthogonal"
            && tiled["renderorder"] == "right-down"
            && tiled["infinite"] == false
            && tiled["version"] == "1.10",
    )?;
    for (key, field) in [
        ("width", "width_cells"),
        ("height", "height_cells"),
        ("tilewidth", "tile_width_px"),
        ("tileheight", "tile_height_px"),
    ] {
        ensure(tiled[key] == geometry[field])?;
    }
    let layers = tiled["layers"]
        .as_array()
        .ok_or(Error::ReleaseUnavailable)?;
    ensure(layers.len() == 3)?;
    let mut names = std::collections::BTreeSet::new();
    let mut ids = std::collections::BTreeSet::new();
    for layer in layers {
        closed(
            layer,
            &[
                "id", "name", "type", "width", "height", "x", "y", "opacity", "visible", "data",
            ],
        )?;
        let name = layer["name"].as_str().ok_or(Error::ReleaseUnavailable)?;
        ensure(
            names.insert(name)
                && ids.insert(
                    layer["id"]
                        .as_u64()
                        .filter(|v| *v > 0)
                        .ok_or(Error::ReleaseUnavailable)?,
                ),
        )?;
        let field = match name {
            "terrain" => "terrain_layer",
            "collision" => "collision_grid",
            "overdraw" => "overdraw_layer",
            _ => return Err(Error::ReleaseUnavailable),
        };
        let rows = geometry[field]
            .as_array()
            .ok_or(Error::ReleaseUnavailable)?;
        let flat = rows
            .iter()
            .map(|r| r.as_array().ok_or(Error::ReleaseUnavailable))
            .collect::<Result<Vec<_>, _>>()?
            .into_iter()
            .flatten()
            .cloned()
            .collect::<Vec<_>>();
        ensure(
            layer["type"] == "tilelayer"
                && layer["data"] == Value::Array(flat)
                && layer["width"] == tiled["width"]
                && layer["height"] == tiled["height"]
                && layer["x"] == 0
                && layer["y"] == 0
                && layer["opacity"] == 1
                && layer["visible"] == true,
        )?;
    }
    let sets = tiled["tilesets"]
        .as_array()
        .ok_or(Error::ReleaseUnavailable)?;
    ensure(sets.len() == 1)?;
    let set = &sets[0];
    closed(
        set,
        &[
            "firstgid",
            "name",
            "image",
            "imagewidth",
            "imageheight",
            "tilewidth",
            "tileheight",
            "tilecount",
            "columns",
            "margin",
            "spacing",
        ],
    )?;
    let width = geometry["tile_width_px"]
        .as_u64()
        .ok_or(Error::ReleaseUnavailable)?;
    let height = geometry["tile_height_px"]
        .as_u64()
        .ok_or(Error::ReleaseUnavailable)?;
    for theme in themes {
        let texture = &theme["texture"];
        let w = texture["width_px"]
            .as_u64()
            .ok_or(Error::ReleaseUnavailable)?;
        let h = texture["height_px"]
            .as_u64()
            .ok_or(Error::ReleaseUnavailable)?;
        ensure(
            width > 0
                && height > 0
                && w % width == 0
                && h % height == 0
                && set["firstgid"] == 1
                && set["name"] == "theme"
                && (set["image"] == "mcr-theme.png" || set["image"] == texture["path"])
                && set["imagewidth"] == w
                && set["imageheight"] == h
                && set["tilewidth"] == width
                && set["tileheight"] == height
                && set["columns"] == w / width
                && set["tilecount"] == (w / width) * (h / height)
                && set["margin"] == 0
                && set["spacing"] == 0,
        )?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn tiled_fixture() -> (Value, Value, Vec<Value>) {
        let geometry = json!({"width_cells":2,"height_cells":1,"tile_width_px":16,"tile_height_px":16,
            "terrain_layer":[[1,2]],"collision_grid":[[0,1]],"overdraw_layer":[[0,0]]});
        let layers = [
            ("terrain", vec![1, 2]),
            ("collision", vec![0, 1]),
            ("overdraw", vec![0, 0]),
        ]
        .into_iter()
        .enumerate()
        .map(|(i, (name, data))| {
            json!({"id":i+1,"name":name,
                "type":"tilelayer","width":2,"height":1,"x":0,"y":0,"opacity":1,
                "visible":true,"data":data})
        })
        .collect::<Vec<_>>();
        let tiled = json!({"type":"map","orientation":"orthogonal","renderorder":"right-down",
            "infinite":false,"version":"1.10","tiledversion":"1.10.2","width":2,"height":1,
            "tilewidth":16,"tileheight":16,"layers":layers,"tilesets":[{"firstgid":1,
            "name":"theme","image":"mcr-theme.png","imagewidth":32,"imageheight":16,
            "tilewidth":16,"tileheight":16,"tilecount":2,"columns":2,"margin":0,"spacing":0}]});
        let themes = ["PARADE","REVENGE","DANGER"].into_iter().map(|id|
            json!({"theme_id":id,"texture":{"path":format!("themes/{id}.png"),"width_px":32,"height_px":16}})).collect();
        (tiled, geometry, themes)
    }
    #[test]
    fn finite_tiled_preserves_all_three_canonical_grids_and_theme_marker() {
        let (tiled, geometry, themes) = tiled_fixture();
        validate_tiled(&tiled, &geometry, &themes).unwrap();
        for (pointer, value) in [
            ("/layers/0/data/0", json!(2)),
            ("/layers/1/data/1", json!(0)),
            ("/layers/2/data/0", json!(1)),
            ("/layers/1/id", json!(1)),
            ("/layers/0/name", json!("objects")),
            ("/layers/0/x", json!(1)),
            ("/infinite", json!(true)),
            ("/width", json!(3)),
            (
                "/tilesets/0/image",
                json!("https://remote.invalid/tile.png"),
            ),
            ("/tilesets/0/columns", json!(1)),
            ("/tilesets/0/tilecount", json!(3)),
            ("/tilesets/0/spacing", json!(1)),
        ] {
            let mut invalid = tiled.clone();
            *invalid.pointer_mut(pointer).unwrap() = value;
            assert!(
                validate_tiled(&invalid, &geometry, &themes).is_err(),
                "{pointer}"
            );
        }
        let mut invalid = tiled.clone();
        invalid["properties"] = json!([]);
        assert!(validate_tiled(&invalid, &geometry, &themes).is_err());
        let mut invalid = themes.clone();
        invalid[2]["texture"]["width_px"] = json!(48);
        assert!(validate_tiled(&tiled, &geometry, &invalid).is_err());
    }
    #[test]
    fn image_header_and_actual_hash_dimensions_are_required() {
        let mut png = vec![0; 33];
        png[..16].copy_from_slice(b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR");
        png[16..20].copy_from_slice(&16u32.to_be_bytes());
        png[20..24].copy_from_slice(&32u32.to_be_bytes());
        let texture = json!({"sha256":digest(&png),"width_px":16,"height_px":32});
        verify_texture(&texture, &png).unwrap();
        let mut changed = png.clone();
        changed[32] = 1;
        assert!(verify_texture(&texture, &changed).is_err());
        assert!(dimensions(&png[..24]).is_err());
        png[16..20].copy_from_slice(&0u32.to_be_bytes());
        assert!(dimensions(&png).is_err());
    }
}
