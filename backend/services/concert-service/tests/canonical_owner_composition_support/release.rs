//! CF01 adapted from process_child_recovery_support/release.rs at 591d908a85a01fe0a494e04c0109bb0bec64a3e1; original SHA256 7f891de7c322e586944b5f8094f5e2664ed2338eeb8102d5c5e18309570b5852.
//! Exact selected source-bound release constructor adapted from native.rs; see provenance.json.
use std::sync::Arc;

pub(super) fn product_release(
) -> anyhow::Result<Arc<concert_application::bootstrap::ValidatedProductRelease>> {
    use concert_service::bootstrap::{BootstrapFeatures, ProductReleaseBundle};
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(3)
        .unwrap();
    let bundle = root.join("backend/releases/operator-bundle/quality-v6");
    let spatial = root.join("backend/releases/reference-scene-v2/quality-v6");
    let themes = [
        bundle.join("theme-revenge.json"),
        bundle.join("theme-parade.json"),
        bundle.join("theme-danger.json"),
    ];
    ProductReleaseBundle::from_source_bound_files(
        &bundle.join("bootstrap.json"),
        &root.join("contracts/fixtures/assets/selected-static/avatar-manifest.json"),
        &spatial.join("map-manifest.json"),
        [&themes[0], &themes[1], &themes[2]],
        BootstrapFeatures::default(),
        &root.join("frontend/public"),
        &spatial.join("source-binding.json"),
    )
    .map_err(|_| anyhow::anyhow!("Actual source-bound public Profile release unavailable"))
}
