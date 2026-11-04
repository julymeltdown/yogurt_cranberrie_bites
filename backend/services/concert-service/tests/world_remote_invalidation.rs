//! Public configuration boundary; native component tests live in the copied lib.
use concert_service::config::world_runtime::WorldRuntimeSettings;
use std::collections::BTreeMap;
use uuid::Uuid;
#[test]
fn replay_identity_requires_explicit_world_serving_and_canonical_owner_binding() {
    let consumer = Uuid::new_v4().to_string();
    let error = WorldRuntimeSettings::settings(|n| {
        Ok((n == "CONCERT_WORLD_INVALIDATION_CONSUMER_ID").then(|| consumer.clone()))
    })
    .err()
    .unwrap();
    assert!(error.to_string().contains("CONCERT_WORLD_LISTEN_ADDR"));
    let event = Uuid::new_v4().to_string();
    let room = Uuid::new_v4().to_string();
    let values = BTreeMap::from([
        ("CONCERT_WORLD_LISTEN_ADDR", "127.0.0.1:0".to_string()),
        ("CONCERT_RUNTIME_MODE", "local".to_string()),
        (
            "CONCERT_WORLD_ALLOWED_ORIGINS",
            "[\"http://localhost:9000\"]".to_string(),
        ),
        ("CONCERT_WORLD_OWNER_ID", Uuid::new_v4().to_string()),
        ("CONCERT_WORLD_EVENT_ID", event),
        (
            "CONCERT_WORLD_ROOMS_JSON",
            format!("[{{\"room_id\":\"{room}\",\"capacity\":2}}]"),
        ),
        ("CONCERT_WORLD_INVALIDATION_CONSUMER_ID", consumer),
        ("CONCERT_WORLD_RESOURCE_ROOT", "/unused/resource".into()),
        (
            "CONCERT_WORLD_SOURCE_BINDING_FILE",
            "/unused/binding".into(),
        ),
    ]);
    let error = WorldRuntimeSettings::settings(|n| Ok(values.get(n).cloned()))
        .err()
        .unwrap();
    assert!(error.to_string().contains("stable World owner identity"));
}

#[test]
fn world_opt_in_requires_and_verifies_actual_source_bound_map4_release() {
    use std::path::Path;
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    let bundle = root.join("backend/releases/operator-bundle/quality-v6");
    let spatial = root.join("backend/releases/reference-scene-v2/quality-v6");
    let bootstrap: serde_json::Value =
        serde_json::from_slice(&std::fs::read(bundle.join("bootstrap.json")).unwrap()).unwrap();
    let owner = Uuid::new_v4().to_string();
    let mut values = BTreeMap::from([
        ("CONCERT_WORLD_LISTEN_ADDR", "127.0.0.1:0".to_string()),
        ("CONCERT_RUNTIME_MODE", "local".to_string()),
        (
            "CONCERT_WORLD_ALLOWED_ORIGINS",
            "[\"http://localhost:9000\"]".to_string(),
        ),
        ("CONCERT_WORLD_OWNER_ID", owner.clone()),
        ("CONCERT_WORLD_INVALIDATION_CONSUMER_ID", owner),
        (
            "CONCERT_WORLD_EVENT_ID",
            bootstrap["event"]["event_id"].as_str().unwrap().to_string(),
        ),
        (
            "CONCERT_WORLD_ROOMS_JSON",
            format!("[{{\"room_id\":\"{}\",\"capacity\":2}}]", Uuid::new_v4()),
        ),
        (
            "CONCERT_BOOTSTRAP_FILE",
            bundle.join("bootstrap.json").display().to_string(),
        ),
        (
            "CONCERT_AVATAR_MANIFEST_FILE",
            root.join("contracts/fixtures/assets/selected-static/avatar-manifest.json")
                .display()
                .to_string(),
        ),
        (
            "CONCERT_MAP_MANIFEST_FILE",
            spatial.join("map-manifest.json").display().to_string(),
        ),
        (
            "CONCERT_REVENGE_MANIFEST_FILE",
            bundle.join("theme-revenge.json").display().to_string(),
        ),
        (
            "CONCERT_PARADE_MANIFEST_FILE",
            bundle.join("theme-parade.json").display().to_string(),
        ),
        (
            "CONCERT_DANGER_MANIFEST_FILE",
            bundle.join("theme-danger.json").display().to_string(),
        ),
        (
            "CONCERT_WORLD_RESOURCE_ROOT",
            root.join("frontend/public").display().to_string(),
        ),
        (
            "CONCERT_WORLD_SOURCE_BINDING_FILE",
            spatial.join("source-binding.json").display().to_string(),
        ),
    ]);
    let binding = values.remove("CONCERT_WORLD_SOURCE_BINDING_FILE").unwrap();
    let missing = WorldRuntimeSettings::settings(|n| Ok(values.get(n).cloned()))
        .err()
        .unwrap();
    assert!(missing
        .to_string()
        .contains("CONCERT_WORLD_SOURCE_BINDING_FILE"));
    values.insert("CONCERT_WORLD_SOURCE_BINDING_FILE", binding);
    let settings = WorldRuntimeSettings::settings(|n| Ok(values.get(n).cloned()))
        .unwrap()
        .unwrap();
    let release = settings.validated_release();
    assert!(
        std::sync::Arc::ptr_eq(&release, &settings.validated_release()),
        "C03 must reuse the exact validated World release rather than rebuild feature defaults"
    );
    values.insert(
        "CONCERT_WORLD_SOURCE_BINDING_FILE",
        spatial.join("missing-binding.json").display().to_string(),
    );
    assert!(WorldRuntimeSettings::settings(|n| Ok(values.get(n).cloned())).is_err());
    let only_binding = WorldRuntimeSettings::settings(|n| {
        Ok((n == "CONCERT_WORLD_SOURCE_BINDING_FILE").then(|| "unused".into()))
    });
    assert!(only_binding.is_err());
}
