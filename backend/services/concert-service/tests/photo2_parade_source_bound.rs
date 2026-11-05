//! Constructor regressions use existing immutable native fixtures, never new ART.
use concert_domain::place::canonical_json_bytes;
use concert_service::bootstrap::{BootstrapFeatures, ProductReleaseBundle};
use serde_json::Value;
use sha2::{Digest, Sha256};
use std::{
    fs,
    path::PathBuf,
    time::{SystemTime, UNIX_EPOCH},
};

struct Files(PathBuf);
impl Files {
    fn new() -> Self {
        let id = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "mcr-photo2-source-test-{}-{id}",
            std::process::id()
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
    fn put(&self, name: &str, raw: &[u8]) -> PathBuf {
        let path = self.0.join(name);
        fs::write(&path, raw).unwrap();
        path
    }
}
impl Drop for Files {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}
const BOOTSTRAP: &[u8] =
    include_bytes!("../../../releases/operator-bundle/quality-v6/bootstrap.json");
const AVATAR: &[u8] =
    include_bytes!("../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json");
const MAP: &[u8] =
    include_bytes!("../../../releases/reference-scene-v2/quality-v6/map-manifest.json");
const PARADE: &[u8] =
    include_bytes!("../../../releases/operator-bundle/quality-v6/theme-parade.json");
const REVENGE: &[u8] =
    include_bytes!("../../../releases/operator-bundle/quality-v6/theme-revenge.json");
const DANGER: &[u8] =
    include_bytes!("../../../releases/operator-bundle/quality-v6/theme-danger.json");
fn rehash(value: &mut Value) {
    value.as_object_mut().unwrap().remove("content_hash");
    let bytes = canonical_json_bytes(&serde_json::to_vec(value).unwrap()).unwrap();
    value["content_hash"] = format!("sha256:{:x}", Sha256::digest(bytes)).into();
}
#[test]
fn legacy_product_constructor_remains_valid() {
    let files = Files::new();
    let b = files.put("bootstrap.json", BOOTSTRAP);
    let a = files.put("avatar.json", AVATAR);
    let m = files.put("map.json", MAP);
    let p = files.put("parade.json", PARADE);
    let r = files.put("revenge.json", REVENGE);
    let d = files.put("danger.json", DANGER);
    assert!(ProductReleaseBundle::from_product_files(
        &b,
        &a,
        &m,
        [&r, &p, &d],
        BootstrapFeatures::default()
    )
    .is_ok());
}
#[test]
fn full_source_constructor_rejects_unregistered_display_before_resource_access() {
    let files = Files::new();
    let mut parade: Value = serde_json::from_slice(PARADE).unwrap();
    parade["theme_revision"] = 2.into();
    parade["scenery"]["ground"]["path"] =
        "world/reference-scene-v2/photo2-world-display-v1/parade/ground.png".into();
    parade["scenery"]["ground"]["width_px"] = 499.into();
    parade["scenery"]["ground"]["height_px"] = 1200.into();
    rehash(&mut parade);
    let mut bootstrap: Value = serde_json::from_slice(BOOTSTRAP).unwrap();
    for theme in bootstrap["theme_manifests"].as_array_mut().unwrap() {
        if theme["theme_id"] == "PARADE" {
            *theme = parade.clone();
        }
    }
    let b = files.put("bootstrap.json", &serde_json::to_vec(&bootstrap).unwrap());
    let a = files.put("avatar.json", AVATAR);
    let m = files.put("map.json", MAP);
    let p = files.put("parade.json", &serde_json::to_vec(&parade).unwrap());
    let r = files.put("revenge.json", REVENGE);
    let d = files.put("danger.json", DANGER);
    // Assert the early C03 rejection separately so a missing binding cannot make this test falsely green.
    assert!(ProductReleaseBundle::from_product_files(
        &b,
        &a,
        &m,
        [&r, &p, &d],
        BootstrapFeatures::default()
    )
    .is_err());
    assert!(ProductReleaseBundle::from_source_bound_files(
        &b,
        &a,
        &m,
        [&r, &p, &d],
        BootstrapFeatures::default(),
        &files.0.join("MUST_NOT_BE_READ"),
        &files.0.join("MISSING_BINDING")
    )
    .is_err());
}
