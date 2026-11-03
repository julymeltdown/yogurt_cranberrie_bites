//! Source-byte binding for one canonical spatial candidate. Candidate inspection
//! never grants product admission or changes event-current map state.
use super::BootstrapServiceError as Error;
use concert_application::bootstrap::ValidatedProductRelease;
use concert_domain::place::{canonical_json_bytes, parse_stored_release, CatalogRelease};
use serde::Deserialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
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
    map_id: String,
    map_revision: i64,
    map_content_hash: String,
    geometry_hash: String,
    geometry_source: FilePin,
    scenery_source: FilePin,
    scene_sources: Vec<FilePin>,
    runtime_resources: Vec<ImagePin>,
}
#[derive(Deserialize)]
struct ProposalGeometry {
    geometry: Value,
    geometry_hash: String,
}

#[derive(Clone, Debug)]
pub struct SourceBoundSpatialCandidate {
    pub catalog: CatalogRelease,
    pub source_commit: String,
    pub art_commit: String,
    pub scenery_bytes: u64,
    pub scenery_pixels: u64,
    pub scenery_within_budget: bool,
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
            && binding.version == 1
            && revision(&binding.source_commit)
            && revision(&binding.art_commit)
            && !binding.scene_sources.is_empty()
            && !binding.runtime_resources.is_empty()
            && binding.scene_sources.len() <= 32
            && binding.runtime_resources.len() <= 32,
    )?;
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
            && proposal.geometry_hash == binding.geometry_hash
            && digest(&geometry) == binding.geometry_hash,
    )?;
    let mut paths = BTreeMap::new();
    paths.insert(
        binding.geometry_source.path.clone(),
        binding.geometry_source.sha256.clone(),
    );
    let scenery =
        super::json::parse(&verify_file(root, &binding.scenery_source)?).map_err(invalid)?;
    ensure(
        paths
            .insert(
                binding.scenery_source.path.clone(),
                binding.scenery_source.sha256.clone(),
            )
            .is_none(),
    )?;
    let descriptors = std::iter::once(&scenery["ground"])
        .chain(
            scenery["atlases"]
                .as_array()
                .ok_or(Error::ReleaseUnavailable)?
                .iter(),
        )
        .collect::<Vec<_>>();
    ensure(descriptors.len() == binding.runtime_resources.len())?;
    for pin in &binding.scene_sources {
        ensure(paths.insert(pin.path.clone(), pin.sha256.clone()).is_none())?;
        verify_file(root, pin)?;
    }
    let (mut bytes, mut pixels) = (0u64, 0u64);
    for (pin, descriptor) in binding.runtime_resources.iter().zip(descriptors) {
        ensure(
            descriptor["path"] == pin.path
                && descriptor["sha256"] == pin.sha256
                && descriptor["bytes"] == pin.bytes
                && descriptor["width_px"] == pin.width_px
                && descriptor["height_px"] == pin.height_px,
        )?;
        ensure(paths.insert(pin.path.clone(), pin.sha256.clone()).is_none())?;
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
    let candidate = SourceBoundSpatialCandidate {
        catalog,
        source_commit: binding.source_commit.clone(),
        art_commit: binding.art_commit.clone(),
        scenery_bytes: bytes,
        scenery_pixels: pixels,
        scenery_within_budget: bytes <= SCENERY_BUDGET && pixels <= SCENERY_BUDGET,
    };
    Ok((candidate, binding))
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
    let mut textures = vec![&doc["avatar_manifest"]["atlas"]["texture"]];
    let themes = doc["theme_manifests"]
        .as_array()
        .ok_or(Error::ReleaseUnavailable)?;
    let supplied_scenery =
        super::json::parse(&verify_file(root, &binding.scenery_source)?).map_err(invalid)?;
    let parade = themes
        .iter()
        .find(|t| t["theme_id"] == "PARADE")
        .ok_or(Error::ReleaseUnavailable)?;
    ensure(parade["scenery"] == supplied_scenery)?;
    let mut scenery_resources = BTreeMap::new();
    for theme in themes {
        textures.push(&theme["texture"]);
        let scenery = theme.get("scenery").ok_or(Error::ReleaseUnavailable)?;
        for texture in std::iter::once(&scenery["ground"]).chain(
            scenery["atlases"]
                .as_array()
                .ok_or(Error::ReleaseUnavailable)?
                .iter(),
        ) {
            let path = texture["path"].as_str().ok_or(Error::ReleaseUnavailable)?;
            let raw = resource(root, path)?;
            verify_texture(texture, &raw)?;
            ensure(texture["bytes"].as_u64() == Some(raw.len() as u64))?;
            scenery_resources.insert(path.to_owned(), texture.clone());
        }
    }
    for texture in textures {
        ensure(texture["image_state"] == "PROVIDED")?;
        let raw = resource(
            root,
            texture["path"].as_str().ok_or(Error::ReleaseUnavailable)?,
        )?;
        verify_texture(texture, &raw)?;
    }
    for pin in &binding.runtime_resources {
        let texture = scenery_resources
            .get(&pin.path)
            .ok_or(Error::ReleaseUnavailable)?;
        ensure(
            texture["sha256"] == pin.sha256
                && texture["width_px"] == pin.width_px
                && texture["height_px"] == pin.height_px,
        )?;
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
