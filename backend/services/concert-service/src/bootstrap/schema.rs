//! Frozen C03 schema plus the C06 cross-document invariants. Mirrors the
//! authoritative Python validator; tests run both against actual RPC bytes.
use super::BootstrapServiceError as Error;
use serde_json::Value;
use std::{
    collections::{BTreeMap, BTreeSet, VecDeque},
    sync::OnceLock,
};

pub fn validate(document: &Value) -> Result<(), Error> {
    static VALIDATOR: OnceLock<Result<jsonschema::Validator, Error>> = OnceLock::new();
    let validator = VALIDATOR.get_or_init(|| {
        let source: Value = serde_json::from_str(include_str!("../../../../docs/api/mcr-core.openapi.json"))
            .map_err(|_| Error::ReleaseUnavailable)?;
        let wrapper = serde_json::json!({"$schema":"https://json-schema.org/draft/2020-12/schema",
            "$ref":"#/components/schemas/Bootstrap", "components":source["components"]});
        jsonschema::options().should_validate_formats(true).build(&wrapper)
            .map_err(|_| Error::ReleaseUnavailable)
    }).as_ref().map_err(Clone::clone)?;
    ensure(validator.is_valid(document))?;
    let p = &document["participant"];
    ensure(
        p["user_id"] == p["profile"]["user_id"]
            && p["event_id"] == document["event"]["event_id"]
            && p["profile"]["avatar"] == document["avatar"],
    )?;
    if matches!(
        document["event"]["phase"].as_str(),
        Some("READ_ONLY" | "ARCHIVED")
    ) {
        ensure(document["capabilities"]["mutations"]["available"] == false)?;
    }
    // Required URI/date-time wire formats are printable ASCII, as in C06 Python.
    fn formats(value: &Value) -> bool {
        match value {
            Value::Object(map) => map.iter().all(|(key, v)| {
                (!matches!(key.as_str(), "source_uri" | "reviewed_at" | "server_time")
                    || v.is_null()
                    || v.as_str()
                        .is_some_and(|s| s.bytes().all(|b| (b'!'..=b'~').contains(&b))))
                    && formats(v)
            }),
            Value::Array(array) => array.iter().all(formats),
            _ => true,
        }
    }
    ensure(formats(document))?;
    let avatar = &document["avatar_manifest"];
    let map = &document["map_manifest"];
    super::release::validate_avatar(avatar)?;
    super::release::validate_map(map)?;
    avatar_semantics(avatar, &document["avatar"]["config"])?;
    let (places, interactions) = map_semantics(map)?;
    let themes = document["theme_manifests"]
        .as_array()
        .ok_or(Error::ReleaseUnavailable)?;
    super::release::validate_themes(themes, avatar, map)?;
    let geometry = &map["geometry"];
    for theme in themes {
        ensure(
            string_set(&theme["place_ids"])? == places
                && string_set(&theme["interaction_ids"])? == interactions,
        )?;
        let tw = number(&geometry["tile_width_px"])?;
        let th = number(&geometry["tile_height_px"])?;
        let w = number(&theme["texture"]["width_px"])?;
        let h = number(&theme["texture"]["height_px"])?;
        ensure(w % tw == 0 && h % th == 0)?;
        let count = (w / tw)
            .checked_mul(h / th)
            .ok_or(Error::ReleaseUnavailable)?;
        for layer in ["terrain_layer", "overdraw_layer"] {
            for row in array(&geometry[layer])? {
                for gid in array(row)? {
                    ensure(number(gid)? <= count)?;
                }
            }
        }
        if let Some(scenery) = theme.get("scenery") {
            scenery_semantics(scenery, geometry)?;
        }
    }
    Ok(())
}

/// Closed C06 schema validation precedes these cross-field bounds. Matching
/// Python/Phaser limits do not confer source-art or venue approval.
fn scenery_semantics(scenery: &Value, geometry: &Value) -> Result<(), Error> {
    let width = number(&geometry["width_cells"])?
        .checked_mul(number(&geometry["tile_width_px"])?)
        .ok_or(Error::ReleaseUnavailable)?;
    let height = number(&geometry["height_cells"])?
        .checked_mul(number(&geometry["tile_height_px"])?)
        .ok_or(Error::ReleaseUnavailable)?;
    let ground = &scenery["ground"];
    let atlases = unique(&scenery["atlases"], "id")?;
    let mut ids = BTreeSet::new();
    let mut paths = BTreeSet::new();
    let mut pixels = 0_u64;
    let mut bytes = 0_u64;
    for texture in std::iter::once(ground).chain(atlases.values().copied()) {
        ensure(ids.insert(text(&texture["id"])?))?;
        ensure(paths.insert(text(&texture["path"])?))?;
        pixels = pixels
            .checked_add(number(&texture["width_px"])? * number(&texture["height_px"])?)
            .ok_or(Error::ReleaseUnavailable)?;
        bytes = bytes
            .checked_add(number(&texture["bytes"])?)
            .ok_or(Error::ReleaseUnavailable)?;
    }
    ensure(bytes <= 2_097_152 && pixels <= 2_097_152)?;
    ensure(number(&ground["width_px"])? == width && number(&ground["height_px"])? == height)?;
    // Ground's schema bounds establish these conversions and additions as safe.
    let (width, height) = (width as i64, height as i64);
    for object in unique(&scenery["objects"], "id")?.values() {
        let atlas = atlases
            .get(text(&object["atlas_id"])?)
            .ok_or(Error::ReleaseUnavailable)?;
        let frame = &object["frame"];
        ensure(number(&frame["x"])? + number(&frame["width"])? <= number(&atlas["width_px"])?)?;
        ensure(number(&frame["y"])? + number(&frame["height"])? <= number(&atlas["height_px"])?)?;
        let integer = |key: &str| object[key].as_i64().ok_or(Error::ReleaseUnavailable);
        let (x, y, w, h) = (
            integer("x")?,
            integer("y")?,
            integer("width")?,
            integer("height")?,
        );
        ensure(
            x >= -128
                && y >= -128
                && x < width
                && y < height
                && x + w > 0
                && y + h > 0
                && x + w <= width + 128
                && y + h <= height + 128
                && integer("depth_y")? <= height + 128,
        )?;
    }
    Ok(())
}

fn ensure(ok: bool) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(Error::ReleaseUnavailable)
    }
}
fn number(v: &Value) -> Result<u64, Error> {
    v.as_u64().ok_or(Error::ReleaseUnavailable)
}
fn array(v: &Value) -> Result<&Vec<Value>, Error> {
    v.as_array().ok_or(Error::ReleaseUnavailable)
}
fn text(v: &Value) -> Result<&str, Error> {
    v.as_str().ok_or(Error::ReleaseUnavailable)
}
fn unique<'a>(rows: &'a Value, key: &str) -> Result<BTreeMap<&'a str, &'a Value>, Error> {
    let mut values = BTreeMap::new();
    for row in array(rows)? {
        ensure(values.insert(text(&row[key])?, row).is_none())?;
    }
    Ok(values)
}
fn string_set(v: &Value) -> Result<BTreeSet<String>, Error> {
    array(v)?.iter().map(|v| Ok(text(v)?.to_owned())).collect()
}
fn avatar_semantics(avatar: &Value, config: &Value) -> Result<(), Error> {
    if avatar["representation"] == "whole_character" {
        let presets = unique(&avatar["presets"], "preset_id")?;
        ensure(presets.contains_key(text(&avatar["default_preset_id"])?))?;
        let catalog = concert_domain::avatar::AvatarCatalog::from_validated_manifest(avatar)
            .map_err(|_| Error::ReleaseUnavailable)?;
        for config in std::iter::once(config).chain(presets.values().map(|v| &v["config"])) {
            let bytes = serde_json::to_vec(config).map_err(|_| Error::ReleaseUnavailable)?;
            let selection = concert_domain::avatar::parse_config(&bytes)
                .map_err(|_| Error::ReleaseUnavailable)?;
            catalog
                .validate(&selection)
                .map_err(|_| Error::ReleaseUnavailable)?;
        }
        return Ok(());
    }
    let atlas = &avatar["atlas"];
    let items = unique(&atlas["items"], "item_id")?;
    let tones = unique(&avatar["skin_tones"], "skin_tone_id")?;
    let presets = unique(&avatar["presets"], "preset_id")?;
    ensure(presets.contains_key(text(&avatar["fallback_preset_id"])?))?;
    let layers: BTreeSet<_> = items
        .values()
        .map(|v| text(&v["layer_id"]))
        .collect::<Result<_, _>>()?;
    ensure(
        layers
            == ["body", "eyes", "hair", "outfit", "accessory"]
                .into_iter()
                .collect(),
    )?;
    let w = number(&atlas["texture"]["width_px"])?;
    let h = number(&atlas["texture"]["height_px"])?;
    ensure(w % 64 == 0 && h % 64 == 0)?;
    let mut regions = Vec::new();
    for item in items.values() {
        let r = &item["region"];
        let x = number(&r["x"])?;
        let y = number(&r["y"])?;
        let rw = number(&r["width_px"])?;
        let rh = number(&r["height_px"])?;
        let right = x.checked_add(rw).ok_or(Error::ReleaseUnavailable)?;
        let bottom = y.checked_add(rh).ok_or(Error::ReleaseUnavailable)?;
        ensure(x % 64 == 0 && y % 64 == 0 && right <= w && bottom <= h)?;
        for (ox, oy, oright, obottom) in &regions {
            ensure(right <= *ox || *oright <= x || bottom <= *oy || *obottom <= y)?;
        }
        regions.push((x, y, right, bottom));
    }
    for config in std::iter::once(config).chain(presets.values().map(|v| &v["config"])) {
        ensure(
            config["manifest_id"] == avatar["manifest_id"]
                && config["manifest_version"] == avatar["manifest_version"],
        )?;
        ensure(tones.contains_key(text(&config["skin_tone_id"])?))?;
        for (layer, id) in config["item_ids"]
            .as_object()
            .ok_or(Error::ReleaseUnavailable)?
        {
            if layer == "accessory" && id.is_null() {
                continue;
            }
            let item = items.get(text(id)?).ok_or(Error::ReleaseUnavailable)?;
            ensure(text(&item["layer_id"])? == layer)?;
        }
    }
    Ok(())
}
fn map_semantics(map: &Value) -> Result<(BTreeSet<String>, BTreeSet<String>), Error> {
    let g = &map["geometry"];
    let w = number(&g["width_cells"])?;
    let h = number(&g["height_cells"])?;
    for name in ["collision_grid", "terrain_layer", "overdraw_layer"] {
        let rows = array(&g[name])?;
        ensure(rows.len() as u64 == h)?;
        for row in rows {
            ensure(array(row)?.len() as u64 == w)?;
        }
    }
    let places = unique(&map["places"], "place_id")?;
    let spawns = unique(&g["spawns"], "spawn_id")?;
    let interactions = unique(&g["interactions"], "interaction_id")?;
    ensure(spawns.contains_key(text(&g["default_spawn_id"])?))?;
    for place in places.values() {
        if place["provenance"]["source_kind"] == "ORIGINAL_FIXTURE" {
            ensure(
                place["verification_state"] == "CONCEPT_ONLY"
                    && place["place_kind"] == "VIRTUAL_CONCEPT",
            )?;
        }
    }
    let walkable = |x: i64, y: i64| -> bool {
        x >= 0
            && y >= 0
            && (x as u64) < w
            && (y as u64) < h
            && g["collision_grid"][y as usize][x as usize] == 0
    };
    let point = |v: &Value| -> Result<(i64, i64), Error> {
        Ok((
            v["cell_x"].as_i64().ok_or(Error::ReleaseUnavailable)?,
            v["cell_y"].as_i64().ok_or(Error::ReleaseUnavailable)?,
        ))
    };
    for entry in spawns.values().chain(interactions.values()) {
        let (x, y) = point(entry)?;
        ensure(walkable(x, y))?;
    }
    for entry in interactions.values() {
        ensure(places.contains_key(text(&entry["place_id"])?))?;
    }
    for spawn in spawns.values() {
        let origin = point(spawn)?;
        let mut visited = BTreeSet::from([origin]);
        let mut queue = VecDeque::from([origin]);
        while let Some((x, y)) = queue.pop_front() {
            for next in [(x - 1, y), (x + 1, y), (x, y - 1), (x, y + 1)] {
                if walkable(next.0, next.1) && visited.insert(next) {
                    queue.push_back(next);
                }
            }
        }
        for entry in interactions.values() {
            ensure(visited.contains(&point(entry)?))?;
        }
    }
    Ok((
        places.keys().map(|v| (*v).to_owned()).collect(),
        interactions.keys().map(|v| (*v).to_owned()).collect(),
    ))
}

#[cfg(test)]
mod scenery_tests {
    use super::*;
    use sha2::{Digest, Sha256};

    #[test]
    fn embedded_bootstrap_schema_and_scenery_semantics_match_shared_cases() {
        let template: Value = serde_json::from_str(include_str!(
            "../../../../../contracts/fixtures/core/bootstrap-open.json"
        ))
        .unwrap();
        // Legacy releases retain precisely their prior optional-field behavior.
        assert!(validate(&template).is_ok());
        let cases: Value = serde_json::from_str(include_str!(
            "../../../../../contracts/fixtures/asset-semantics/illustrated-cases.json"
        ))
        .unwrap();
        for case in cases["cases"].as_array().unwrap() {
            let mut document = template.clone();
            let theme = document["theme_manifests"]
                .as_array_mut()
                .unwrap()
                .iter_mut()
                .find(|theme| theme["theme_id"] == "REVENGE")
                .unwrap();
            theme["scenery"] = case["scenery"].clone();
            let mut unsigned = theme.clone();
            unsigned.as_object_mut().unwrap().remove("content_hash");
            // Fixture keys/values are ASCII; explicit sorted keys preserve C06
            // canonical bytes even if serde_json enables preserve_order later.
            fn sorted(value: &Value) -> String {
                match value {
                    Value::Object(map) => {
                        let entries: BTreeMap<_, _> = map.iter().collect();
                        format!(
                            "{{{}}}",
                            entries
                                .into_iter()
                                .map(|(k, v)| format!(
                                    "{}:{}",
                                    serde_json::to_string(k).unwrap(),
                                    sorted(v)
                                ))
                                .collect::<Vec<_>>()
                                .join(",")
                        )
                    }
                    Value::Array(values) => format!(
                        "[{}]",
                        values.iter().map(sorted).collect::<Vec<_>>().join(",")
                    ),
                    _ => serde_json::to_string(value).unwrap(),
                }
            }
            theme["content_hash"] =
                format!("sha256:{:x}", Sha256::digest(sorted(&unsigned))).into();
            assert_eq!(
                validate(&document).is_ok(),
                case["valid"].as_bool().unwrap(),
                "{}",
                case["name"]
            );
        }
    }
}
