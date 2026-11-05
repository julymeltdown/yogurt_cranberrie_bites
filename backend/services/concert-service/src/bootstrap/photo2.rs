//! Exact source-qualified v3 Photo2 display registration; no runtime admission.
//! A namespace, self-reported descriptor hash or source-interface READY is not admission.
use super::BootstrapServiceError as Error;
use concert_domain::place::canonical_json_bytes;
use serde_json::Value;
use sha2::{Digest, Sha256};

pub(super) const PROFILE: &str = "mcr-photo2-world-display-v1";
pub(super) const PREFIX: &str = "world/reference-scene-v2/photo2-world-display-v1/parade/";
const GEOMETRY: &str = "sha256:899aca9bd6ce499c6f46aff5eaf21e30ab34b301e988771b978a6fab7c2d469f";
const MAP_HASH: &str = "sha256:736afd273a067f6c40db76b8cc54412416fba88984a50d9a6b6738d869831fb8";
const NATIVE_OBJECTS: &str =
    "sha256:8305532e5438901164b2fcd208b9603ea32d2779a5b6cbaacb277685e4b05499";
const NATIVE_TEXTURE: &str =
    "sha256:e2d9f785d31ea9e9a1888dc8f73d182eca917b54e1924fbf5159bc91be96e795";
const CONTRACT: &str = "cc12d028b4615246ea457722d13af6d35fc505e44b6d1946adcf1811a37ab98e";
const NATIVE_INPUT: &str = "847d9c92a4ddfa22b8187bb6afe9a9fc2e6e09b1c7fe8e5f85c5a7f5c0d846a9";

/// Literal pins are filled only from an immutable, genuinely qualified ART handoff.
/// These are backend-internal source bindings, not new C03 fields or ART metadata.
#[derive(Clone, Copy)]
pub(super) struct Registration {
    pub theme_hash: &'static str,
    pub scenery_hash: &'static str,
    pub interface_hash: &'static str,
    pub provenance_hash: &'static str,
    pub art_commit: &'static str,
}
// Exact canonical ART tuple; lamp source correction complete; World/full visual acceptance remains open.
const REGISTERED: &[Registration] = &[Registration {
    theme_hash: "sha256:146d1fdc816052d3a78ef7b8c90d7cb1a6eeaff29c7dec9a1be458c0b377508f",
    scenery_hash: "sha256:d714d971dc45ea2d191552db9ccdad0c8a62441f3962b161301686c9584aeef6",
    interface_hash: "sha256:dd2d09f2aebe3c631fcd0eed8cc09e11df8661550a38fa214ee50e6c39b81600",
    provenance_hash: "sha256:8cebea0e8f5adcb9fa479b01936e89e65fa15a1a625f5afc47bd0a898e9ed465",
    art_commit: "a8027217ec9bc5b33a9219b74726ea707f33e0cc",
}];
fn ensure(ok: bool) -> Result<(), Error> {
    if ok {
        Ok(())
    } else {
        Err(Error::ReleaseUnavailable)
    }
}
fn canonical_hash(value: &Value) -> Result<String, Error> {
    let bytes = serde_json::to_vec(value).map_err(|_| Error::ReleaseUnavailable)?;
    let bytes = canonical_json_bytes(&bytes).map_err(|_| Error::ReleaseUnavailable)?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}
fn native_objects(scenery: &Value) -> Result<Value, Error> {
    let mut objects = scenery["objects"]
        .as_array()
        .ok_or(Error::ReleaseUnavailable)?
        .clone();
    ensure(objects.len() == 22)?;
    for object in &mut objects {
        let map = object.as_object_mut().ok_or(Error::ReleaseUnavailable)?;
        map.remove("atlas_id");
        map.remove("frame");
    }
    Ok(Value::Array(objects))
}
fn native_map(map: &Value) -> Result<(), Error> {
    ensure(
        map["map_id"] == "mcr_reference_scene_r3"
            && map["map_revision"] == 4
            && map["geometry_hash"] == GEOMETRY
            && map["content_hash"] == MAP_HASH,
    )
}
fn scenery_checks(scenery: &Value) -> Result<(), Error> {
    ensure(
        scenery["version"] == 1
            && scenery["ground"]["path"] == format!("{PREFIX}ground.png")
            && scenery["ground"]["width_px"] == 499
            && scenery["ground"]["height_px"] == 1200,
    )?;
    let atlases = scenery["atlases"]
        .as_array()
        .ok_or(Error::ReleaseUnavailable)?;
    ensure(
        atlases.len() == 1
            && atlases[0]["path"] == format!("{PREFIX}atlas.png")
            && atlases[0]["width_px"] == 512
            && atlases[0]["height_px"] == 1024,
    )?;
    ensure(canonical_hash(&native_objects(scenery)?)? == NATIVE_OBJECTS)?;
    let atlas_id = atlases[0]["id"].as_str().ok_or(Error::ReleaseUnavailable)?;
    for object in scenery["objects"]
        .as_array()
        .ok_or(Error::ReleaseUnavailable)?
    {
        ensure(object["atlas_id"] == atlas_id)?;
    }
    Ok(())
}
fn theme_checks(theme: &Value, map: &Value, registration: &Registration) -> Result<(), Error> {
    native_map(map)?;
    ensure(
        theme["theme_id"] == "PARADE"
            && theme["theme_revision"].as_u64().is_some_and(|v| v >= 2)
            && theme["content_hash"] == registration.theme_hash
            && canonical_hash(&theme["scenery"])? == registration.scenery_hash,
    )?;
    ensure(
        theme["texture"]["image_state"] == "PROVIDED"
            && theme["texture"]["path"] == "world/reference-scene-v2/quality-v6/ground.png"
            && theme["texture"]["sha256"] == NATIVE_TEXTURE
            && theme["texture"]["width_px"] == 768
            && theme["texture"]["height_px"] == 1296,
    )?;
    scenery_checks(&theme["scenery"])
}
/// Legacy themes return false and retain their exact existing native-extent gate.
/// Any attempted Photo2 resource must match a literal registered theme/scenery tuple.
pub(super) fn registered_theme(theme: &Value, map: &Value) -> Result<bool, Error> {
    let scenery = &theme["scenery"];
    let attempted = scenery["ground"]["path"]
        .as_str()
        .is_some_and(|p| p.starts_with(PREFIX))
        || scenery["atlases"].as_array().is_some_and(|a| {
            a.iter()
                .any(|v| v["path"].as_str().is_some_and(|p| p.starts_with(PREFIX)))
        });
    if !attempted {
        return Ok(false);
    }
    let registration = REGISTERED
        .iter()
        .find(|r| theme["content_hash"] == r.theme_hash)
        .ok_or(Error::ReleaseUnavailable)?;
    theme_checks(theme, map, registration)?;
    Ok(true)
}
pub(super) fn registration(interface_hash: &str) -> Result<&'static Registration, Error> {
    REGISTERED
        .iter()
        .find(|r| r.interface_hash == interface_hash)
        .ok_or(Error::ReleaseUnavailable)
}
/// Called only after exact source bytes/hash are verified under the existing public root.
pub(super) fn interface_scenery(interface: &Value, pin: &str, map: &Value) -> Result<Value, Error> {
    let r = registration(pin)?;
    native_map(map)?;
    ensure(
        interface["schemaVersion"] == "mcr-photo2-world-art-source-v1"
            && interface["version"] == 1
            && interface["themeId"] == "PARADE"
            && interface["variantId"] == "photo2-parade-display-v1",
    )?;
    let input = &interface["sourceInput"];
    ensure(
        input["contractSHA256"] == CONTRACT
            && input["nativeSource"]["sha256"] == NATIVE_INPUT
            && input["nativeSource"]["geometryHash"] == GEOMETRY,
    )?;
    let scene = &interface["sceneDescriptor"];
    ensure(
        scene["schema"] == PROFILE
            && scene["version"] == 1
            && scene["nativeTileTexture"] == "SEPARATE_EXISTING_PINNED_C06_TILED_TEXTURE_UNCHANGED",
    )?;
    let scenery = &interface["scenery"];
    scenery_checks(scenery)?;
    ensure(canonical_hash(scenery)? == r.scenery_hash)?;
    let frames = scene["frames"]
        .as_array()
        .ok_or(Error::ReleaseUnavailable)?;
    let objects = scenery["objects"]
        .as_array()
        .ok_or(Error::ReleaseUnavailable)?;
    ensure(frames.len() == objects.len())?;
    for (frame, object) in frames.iter().zip(objects) {
        ensure(frame["id"] == object["id"] && frame["atlasRect"] == object["frame"])?;
        for key in ["id", "x", "y", "width", "height", "depth_y"] {
            ensure(frame["native"][key] == object[key])?;
        }
    }
    let boards = scene["boards"]
        .as_array()
        .ok_or(Error::ReleaseUnavailable)?;
    ensure(boards.len() == 2)?;
    for (board, (id, destination)) in boards.iter().zip([
        ("photo2-shortcut-meet", "/meet"),
        ("photo2-shortcut-wall", "/wall"),
    ]) {
        ensure(
            board["id"] == id
                && board["destination"] == destination
                && board["kind"] == "nonphysical-panel-shortcut"
                && board.get("interactionId") == Some(&Value::Null),
        )?;
    }
    Ok(scenery.clone())
}
pub(super) fn bind_theme(theme: &Value, map: &Value, interface_hash: &str) -> Result<(), Error> {
    theme_checks(theme, map, registration(interface_hash)?)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn fixture() -> (Value, Value, Registration) {
        let mut theme: Value = serde_json::from_slice(include_bytes!(
            "../../../../releases/operator-bundle/quality-v6/theme-parade.json"
        ))
        .unwrap();
        theme["theme_revision"] = 2.into();
        theme["content_hash"] = "sha256:synthetic-semantic-fixture".into();
        let scenery = &mut theme["scenery"];
        scenery["ground"]["path"] = format!("{PREFIX}ground.png").into();
        scenery["ground"]["width_px"] = 499.into();
        scenery["ground"]["height_px"] = 1200.into();
        scenery["atlases"][0]["path"] = format!("{PREFIX}atlas.png").into();
        scenery["atlases"][0]["width_px"] = 512.into();
        scenery["atlases"][0]["height_px"] = 1024.into();
        let map = serde_json::json!({"map_id":"mcr_reference_scene_r3","map_revision":4,
            "geometry_hash":GEOMETRY,"content_hash":MAP_HASH});
        let registration = Registration {
            theme_hash: "sha256:synthetic-semantic-fixture",
            scenery_hash: Box::leak(canonical_hash(scenery).unwrap().into_boxed_str()),
            interface_hash: "not-a-product-interface",
            provenance_hash: "not-product-provenance",
            art_commit: "not-product-art",
        };
        (theme, map, registration)
    }
    #[test]
    fn registered_semantic_fixture_keeps_native_texture_and_geometry() {
        let (theme, map, registration) = fixture();
        assert!(theme_checks(&theme, &map, &registration).is_ok());
        // Local synthetic semantic pins never populate the actual product table.
        assert!(registered_theme(&theme, &map).is_err());
    }
    #[test]
    fn even_rehashed_native_foot_mutation_is_rejected() {
        let (mut theme, map, mut registration) = fixture();
        theme["scenery"]["objects"][0]["depth_y"] = 361.into();
        registration.scenery_hash =
            Box::leak(canonical_hash(&theme["scenery"]).unwrap().into_boxed_str());
        assert!(theme_checks(&theme, &map, &registration).is_err());
    }
    #[test]
    fn display_cannot_replace_native_texture_or_change_map_revision() {
        let (mut theme, mut map, registration) = fixture();
        map["map_revision"] = 6.into();
        assert!(theme_checks(&theme, &map, &registration).is_err());
        map["map_revision"] = 4.into();
        theme["texture"]["height_px"] = 1200.into();
        assert!(theme_checks(&theme, &map, &registration).is_err());
    }
}
