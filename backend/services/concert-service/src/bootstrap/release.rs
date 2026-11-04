//! Explicit product release parsing. The frozen C03 document supplies all
//! contract-owned structure; bootstrap changes only request-bound fields.
use super::{
    BootstrapAdmission, BootstrapDefaults, BootstrapError, BootstrapFeatures, BootstrapResult,
    ReleasePolicy,
};
use chrono::SecondsFormat;
use concert_application::bootstrap::{BootstrapRelease, ValidatedProductRelease};
use concert_domain::{AssetId, AvatarSelection, Theme};
use serde_json::{json, Map, Value};
use sha2::{Digest, Sha256};
use std::{collections::BTreeSet, fs, path::Path, sync::Arc};

const MAX_RELEASE_BYTES: usize = 4 * 1024 * 1024;
const CONTRACT_VERSION: &str = "0.1.0";
const ASSET_VERSION: &str = "1.0.0";
const THEMES: [&str; 3] = ["REVENGE", "PARADE", "DANGER"];

pub fn validate_product_files(
    bootstrap: &Path,
    avatar_path: &Path,
    map_path: &Path,
    theme_paths: [&Path; 3],
    features: BootstrapFeatures,
    policy: ReleasePolicy,
) -> Result<Arc<ValidatedProductRelease>, super::BootstrapServiceError> {
    let mut documents = Vec::with_capacity(6);
    documents.push(read_document(bootstrap)?);
    documents.push(read_document(avatar_path)?);
    documents.push(read_document(map_path)?);
    for path in theme_paths {
        documents.push(read_document(path)?);
    }
    let doc = &documents[0];
    let avatar = &documents[1];
    let map = &documents[2];
    let themes = &documents[3..];

    super::schema::validate(doc)?;
    validate_template(doc)?;
    validate_avatar(avatar)?;
    validate_map(map)?;
    validate_themes(themes, avatar, map)?;
    validate_release_provenance(&documents, policy)?;

    let template_event = object(doc, "event")?;
    let template_event_id = template_event
        .get("event_id")
        .and_then(Value::as_str)
        .ok_or(super::BootstrapServiceError::ReleaseUnavailable)?
        .to_owned();
    if template_event.get("phase").and_then(Value::as_str) != Some("OPEN") {
        return Err(super::BootstrapServiceError::ReleaseUnavailable);
    }
    let template_avatar_value = doc
        .get("avatar")
        .ok_or(super::BootstrapServiceError::ReleaseUnavailable)?;
    let config = object(template_avatar_value, "config")?;
    let config = Value::Object(config.clone());
    let defaults = BootstrapDefaults {
        theme: string(&doc["participant"]["preferences"], "theme")?
            .parse::<Theme>()
            .map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)?,
        avatar: parse_avatar_config(&config)?,
    };
    validate_default(defaults.avatar.clone(), avatar)?;
    let fallback = string(avatar, default_preset_key(avatar))?;
    if !avatar["presets"]
        .as_array()
        .ok_or(super::BootstrapServiceError::ReleaseUnavailable)?
        .iter()
        .any(|preset| preset["preset_id"].as_str() == Some(fallback) && preset["config"] == config)
    {
        return Err(super::BootstrapServiceError::ReleaseUnavailable);
    }
    if string(
        &Value::Object(object(doc, "avatar_manifest")?.clone()),
        "manifest_id",
    )? != string(avatar, "manifest_id")?
    {
        return Err(super::BootstrapServiceError::ReleaseUnavailable);
    }
    let mut capability_template = doc.clone();
    let capabilities = object(&capability_template, "capabilities")?;
    for capability in capabilities.keys() {
        if !matches!(
            capability.as_str(),
            "world" | "feed" | "meet" | "gifts" | "wall" | "social_sharing" | "mutations"
        ) {
            return Err(super::BootstrapServiceError::ReleaseUnavailable);
        }
    }
    let world_resources = avatar["atlas"]["texture"]["image_state"] == "PROVIDED"
        && map["tilemap"]["resource_state"] == "PROVIDED"
        && themes
            .iter()
            .all(|theme| theme["texture"]["image_state"] == "PROVIDED");
    let feature = |_name: &str, enabled: bool| json!({"available": enabled, "reason": if enabled { Value::Null } else { Value::String("FEATURE_DISABLED".into()) }});
    capability_template["capabilities"] = json!({
        "world": if features.world && !world_resources {
            json!({"available":false,"reason":"DEPENDENCY_UNAVAILABLE"})
        } else { feature("world", features.world) },
        "feed": feature("feed", features.feed),
        "meet": feature("meet", features.meet),
        "gifts": feature("gifts", features.gifts),
        "wall": feature("wall", features.wall),
        "social_sharing": feature("social_sharing", features.social_sharing),
        "mutations": feature("mutations", false),
    });
    capability_template["event"]["event_id"] = Value::String(template_event_id.clone());
    capability_template["map_manifest"] = map.clone();
    capability_template["avatar_manifest"] = avatar.clone();
    let mut ordered = themes.to_vec();
    ordered.sort_by_key(|v| string(v, "theme_id").unwrap_or_default().to_owned());
    capability_template["theme_manifests"] = Value::Array(ordered);
    capability_template["participant"]["age_status"] = Value::String("UNKNOWN".into());
    super::schema::validate(&capability_template)?;
    let mut bound_sources = documents.clone();
    bound_sources.push(capability_template.clone());
    let digest = release_digest(&bound_sources)?;
    let bytes = serde_json::to_vec(&capability_template)
        .map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)?;
    let release = BootstrapRelease::new(defaults, bytes)
        .map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)?;
    ValidatedProductRelease::from_validated(release, digest)
        .map(Arc::new)
        .map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)
}

fn read_document(path: &Path) -> Result<Value, super::BootstrapServiceError> {
    let metadata =
        fs::metadata(path).map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)?;
    if !metadata.is_file() || metadata.len() > MAX_RELEASE_BYTES as u64 {
        return Err(super::BootstrapServiceError::ReleaseUnavailable);
    }
    let bytes = fs::read(path).map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)?;
    if bytes.len() > MAX_RELEASE_BYTES {
        return Err(super::BootstrapServiceError::ReleaseUnavailable);
    }
    let text = std::str::from_utf8(&bytes)
        .map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)?;
    let value: Value = super::json::parse(text.as_bytes())
        .map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)?;
    assert_public_release_value(&value)?;
    Ok(value)
}

fn assert_public_release_value(value: &Value) -> Result<(), super::BootstrapServiceError> {
    fn walk(value: &Value) -> bool {
        match value {
            Value::Object(map) => map.iter().all(|(key, value)| {
                !matches!(
                    key.as_str(),
                    "email"
                        | "password"
                        | "token"
                        | "refresh_token"
                        | "provider_subject"
                        | "age"
                        | "exact_age"
                        | "avatar_url"
                        | "username"
                ) && walk(value)
            }),
            Value::Array(values) => values.iter().all(walk),
            Value::Null | Value::Bool(_) | Value::Number(_) | Value::String(_) => true,
        }
    }
    if walk(value) {
        Ok(())
    } else {
        Err(super::BootstrapServiceError::ReleaseUnavailable)
    }
}

fn release_digest(values: &[Value]) -> Result<String, super::BootstrapServiceError> {
    let mut hasher = Sha256::new();
    hasher.update(b"mcr bootstrap release v1\0");
    for value in values {
        let bytes =
            canonical_bytes(value).map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)?;
        hasher.update((bytes.len() as u64).to_be_bytes());
        hasher.update(bytes);
    }
    Ok(hasher
        .finalize()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect())
}

fn canonical_bytes(value: &Value) -> Result<Vec<u8>, serde_json::Error> {
    let mut output = Vec::new();
    encode(value, &mut output)?;
    Ok(output)
}
fn encode(value: &Value, output: &mut Vec<u8>) -> Result<(), serde_json::Error> {
    match value {
        Value::Null => output.extend(b"null"),
        Value::Bool(value) => {
            if *value {
                output.extend_from_slice(b"true")
            } else {
                output.extend_from_slice(b"false")
            }
        }
        Value::Number(value) if value.is_i64() || value.is_u64() => {
            output.extend(value.to_string().as_bytes())
        }
        Value::Number(_) => return Err(serde_json::Error::io(std::io::Error::other("float"))),
        Value::String(value) => encode_string(value, output)?,
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                encode(value, output)?;
            }
            output.push(b']');
        }
        Value::Object(values) => {
            output.push(b'{');
            let mut keys: Vec<_> = values.keys().collect();
            keys.sort();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                encode_string(key, output)?;
                output.push(b':');
                encode(&values[key], output)?;
            }
            output.push(b'}');
        }
    }
    Ok(())
}

fn encode_string(value: &str, output: &mut Vec<u8>) -> Result<(), serde_json::Error> {
    let encoded = serde_json::to_string(value)?;
    for character in encoded.chars() {
        if character <= '\u{7e}' {
            output.push(character as u8);
        } else {
            for unit in character.encode_utf16(&mut [0; 2]).iter() {
                output.extend(format!("\\u{unit:04x}").as_bytes());
            }
        }
    }
    Ok(())
}

fn object<'a>(
    value: &'a Value,
    key: &str,
) -> Result<&'a Map<String, Value>, super::BootstrapServiceError> {
    value
        .get(key)
        .and_then(Value::as_object)
        .ok_or(super::BootstrapServiceError::ReleaseUnavailable)
}
fn string<'a>(value: &'a Value, key: &str) -> Result<&'a str, super::BootstrapServiceError> {
    value
        .get(key)
        .and_then(Value::as_str)
        .ok_or(super::BootstrapServiceError::ReleaseUnavailable)
}
fn positive(value: &Value, key: &str) -> Result<i64, super::BootstrapServiceError> {
    value
        .get(key)
        .and_then(Value::as_i64)
        .filter(|v| (1..=9_007_199_254_740_991).contains(v))
        .ok_or(super::BootstrapServiceError::ReleaseUnavailable)
}
fn manifest_id(value: &Value, key: &str) -> Result<String, super::BootstrapServiceError> {
    let text = string(value, key)?;
    AssetId::new(text.to_owned())
        .map(|v| v.as_str().to_owned())
        .map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)
}
fn parse_avatar_config(value: &Value) -> Result<AvatarSelection, super::BootstrapServiceError> {
    let bytes =
        serde_json::to_vec(value).map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)?;
    concert_domain::avatar::parse_config(&bytes)
        .map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)
}
fn default_preset_key(avatar: &Value) -> &'static str {
    if avatar["representation"] == "whole_character" {
        "default_preset_id"
    } else {
        "fallback_preset_id"
    }
}

fn validate_template(value: &Value) -> Result<(), super::BootstrapServiceError> {
    let object = value
        .as_object()
        .ok_or(super::BootstrapServiceError::ReleaseUnavailable)?;
    if object.get("contract_version").and_then(Value::as_str) != Some(CONTRACT_VERSION)
        || !object.contains_key("event")
        || !object.contains_key("participant")
        || !object.contains_key("avatar")
        || !object.contains_key("capabilities")
        || !object.contains_key("map_manifest")
        || !object.contains_key("avatar_manifest")
        || !object.contains_key("server_time")
        || !object.contains_key("theme_manifests")
    {
        return Err(super::BootstrapServiceError::ReleaseUnavailable);
    }
    let server_time = object
        .get("server_time")
        .and_then(Value::as_str)
        .ok_or(super::BootstrapServiceError::ReleaseUnavailable)?;
    chrono::DateTime::parse_from_rfc3339(server_time)
        .map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)?;
    Ok(())
}
pub(super) fn validate_avatar(value: &Value) -> Result<(), super::BootstrapServiceError> {
    if string(value, "kind")? != "avatar_manifest"
        || !matches!(string(value, "contract_version")?, "1.0.0" | "1.1.0")
    {
        return Err(super::BootstrapServiceError::ReleaseUnavailable);
    }
    let _ = manifest_id(value, "manifest_id")?;
    let _ = positive(value, "manifest_version")?;
    validate_manifest_hash(value)?;
    let presets = value
        .get("presets")
        .and_then(Value::as_array)
        .ok_or(super::BootstrapServiceError::ReleaseUnavailable)?;
    let fallback = string(value, default_preset_key(value))?;
    if !presets
        .iter()
        .any(|v| string(v, "preset_id").ok() == Some(fallback))
    {
        return Err(super::BootstrapServiceError::ReleaseUnavailable);
    }
    Ok(())
}
pub(super) fn validate_map(value: &Value) -> Result<(), super::BootstrapServiceError> {
    if string(value, "kind")? != "map_manifest"
        || string(value, "contract_version")? != ASSET_VERSION
    {
        return Err(super::BootstrapServiceError::ReleaseUnavailable);
    }
    let geometry = value
        .get("geometry")
        .ok_or(super::BootstrapServiceError::ReleaseUnavailable)?;
    let expected = format!(
        "sha256:{}",
        hex_digest(
            canonical_bytes(geometry)
                .map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)?
        )
    );
    if string(value, "geometry_hash")? != expected {
        return Err(super::BootstrapServiceError::ReleaseUnavailable);
    }
    validate_manifest_hash(value)
}
fn validate_manifest_hash(value: &Value) -> Result<(), super::BootstrapServiceError> {
    let mut without_hash = value.clone();
    without_hash
        .as_object_mut()
        .ok_or(super::BootstrapServiceError::ReleaseUnavailable)?
        .remove("content_hash");
    let bytes = canonical_bytes(&without_hash)
        .map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)?;
    let expected = format!("sha256:{}", hex_digest(bytes));
    if string(value, "content_hash")? != expected {
        return Err(super::BootstrapServiceError::ReleaseUnavailable);
    }
    Ok(())
}
fn hex_digest(value: Vec<u8>) -> String {
    Sha256::digest(value)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}
fn validate_default(
    selection: AvatarSelection,
    avatar: &Value,
) -> Result<(), super::BootstrapServiceError> {
    concert_domain::avatar::AvatarCatalog::from_validated_manifest(avatar)
        .and_then(|catalog| catalog.validate(&selection))
        .map_err(|_| super::BootstrapServiceError::ReleaseUnavailable)
}

pub(super) fn validate_themes(
    themes: &[Value],
    avatar: &Value,
    map: &Value,
) -> Result<(), super::BootstrapServiceError> {
    if themes.len() != 3 {
        return Err(super::BootstrapServiceError::ReleaseUnavailable);
    }
    let mut seen = BTreeSet::new();
    for theme in themes {
        if string(theme, "kind")? != "theme_manifest"
            || string(theme, "contract_version")? != ASSET_VERSION
        {
            return Err(super::BootstrapServiceError::ReleaseUnavailable);
        }
        if !seen.insert(string(theme, "theme_id")?.to_owned()) {
            return Err(super::BootstrapServiceError::ReleaseUnavailable);
        }
        for (key, expected) in [
            ("map_id", string(map, "map_id")?),
            ("map_content_hash", string(map, "content_hash")?),
            ("geometry_hash", string(map, "geometry_hash")?),
            ("avatar_manifest_id", string(avatar, "manifest_id")?),
            ("avatar_content_hash", string(avatar, "content_hash")?),
        ] {
            if string(theme, key)? != expected {
                return Err(super::BootstrapServiceError::ReleaseUnavailable);
            }
        }
        if positive(theme, "map_revision")? != positive(map, "map_revision")?
            || positive(theme, "avatar_manifest_version")? != positive(avatar, "manifest_version")?
            || string(theme, "collision_id")? != string(&map["geometry"], "collision_id")?
        {
            return Err(super::BootstrapServiceError::ReleaseUnavailable);
        }
        validate_manifest_hash(theme)?;
    }
    if seen != THEMES.iter().map(|v| (*v).to_owned()).collect() {
        return Err(super::BootstrapServiceError::ReleaseUnavailable);
    }
    Ok(())
}
fn validate_release_provenance(
    values: &[Value],
    policy: ReleasePolicy,
) -> Result<(), super::BootstrapServiceError> {
    fn check(value: &Value, policy: ReleasePolicy) -> Result<(), super::BootstrapServiceError> {
        match value {
            Value::Object(map) => {
                if let Some(provenance) = map.get("provenance") {
                    // BE08/AR01/FE19 keep display-only concepts distinct from
                    // physical meeting places. Full C06 validation precedes
                    // this policy; no other object receives this exception.
                    let virtual_concept = map.get("place_id").is_some()
                        && map.get("place_kind").and_then(Value::as_str) == Some("VIRTUAL_CONCEPT")
                        && map.get("verification_state").and_then(Value::as_str)
                            == Some("CONCEPT_ONLY");
                    let review_state = string(provenance, "review_state")?;
                    if policy == ReleasePolicy::Product
                        && (string(provenance, "source_kind")? == "ORIGINAL_FIXTURE"
                            || !(review_state == "REVIEWED"
                                || (virtual_concept && review_state == "UNREVIEWED")))
                    {
                        return Err(super::BootstrapServiceError::ReleaseUnavailable);
                    }
                }
                for child in map.values() {
                    check(child, policy)?;
                }
            }
            Value::Array(array) => {
                for child in array {
                    check(child, policy)?;
                }
            }
            _ => (),
        }
        Ok(())
    }
    for value in &values[1..] {
        check(value, policy)?;
    }
    if policy == ReleasePolicy::Product
        && if values[1]["representation"] == "whole_character" {
            string(&values[1]["source"], "source_pack_id")? != "mcr_user_characters_20261103"
        } else {
            string(&values[1]["provenance"], "source_id")? != "mcr_user_characters_20261103"
        }
    {
        return Err(super::BootstrapServiceError::ReleaseUnavailable);
    }
    Ok(())
}

/// Rebinds request identity, current persisted state and timestamps. No caller
/// can select user/event through response bytes; all response fields are checked.
pub fn bind_response(
    template: &[u8],
    admission: &BootstrapAdmission,
    result: &BootstrapResult,
    release_digest: &str,
) -> Result<Vec<u8>, BootstrapError> {
    if release_digest.len() != 64 {
        return Err(BootstrapError::ReleaseUnavailable);
    }
    let mut document: Value =
        serde_json::from_slice(template).map_err(|_| BootstrapError::ReleaseUnavailable)?;
    let key = admission.key();
    let event = key.event_id.uuid().to_string();
    let user = key.user_id.uuid().to_string();
    let template_event = document
        .get_mut("event")
        .and_then(Value::as_object_mut)
        .ok_or(BootstrapError::ReleaseUnavailable)?;
    let template_event_id = template_event
        .get("event_id")
        .and_then(Value::as_str)
        .ok_or(BootstrapError::ReleaseUnavailable)?
        .to_owned();
    if template_event_id != event {
        return Err(BootstrapError::ReleaseUnavailable);
    }
    template_event.insert(
        "slug".into(),
        Value::String(result.record.event_slug.clone()),
    );
    template_event.insert(
        "title".into(),
        Value::String(result.record.event_title.clone()),
    );
    template_event.insert(
        "timezone".into(),
        Value::String(result.record.event_timezone.clone()),
    );
    template_event.insert(
        "phase".into(),
        Value::String(result.record.phase.as_str().into()),
    );
    template_event.insert("revision".into(), Value::from(result.record.event_revision));
    if template_event.get("slug").and_then(Value::as_str).is_none()
        || template_event
            .get("title")
            .and_then(Value::as_str)
            .is_none()
        || template_event.get("timezone").and_then(Value::as_str) != Some("Asia/Seoul")
    {
        return Err(BootstrapError::ReleaseUnavailable);
    }
    let participant = document
        .get_mut("participant")
        .and_then(Value::as_object_mut)
        .ok_or(BootstrapError::ReleaseUnavailable)?;
    participant.insert(
        "event_id".into(),
        Value::String(key.event_id.uuid().to_string()),
    );
    participant.insert("user_id".into(), Value::String(user.clone()));
    participant.insert("age_status".into(), Value::String("UNKNOWN".into()));
    participant.insert(
        "revision".into(),
        Value::from(result.record.participant_revision),
    );
    let profile = participant
        .get_mut("profile")
        .and_then(Value::as_object_mut)
        .ok_or(BootstrapError::ReleaseUnavailable)?;
    profile.insert("user_id".into(), Value::String(user));
    profile.insert(
        "display_name".into(),
        Value::String(admission.display_name().into()),
    );
    profile.insert(
        "bio".into(),
        admission
            .bio()
            .map_or(Value::Null, |v| Value::String(v.into())),
    );
    let avatar = document
        .get_mut("avatar")
        .and_then(Value::as_object_mut)
        .ok_or(BootstrapError::ReleaseUnavailable)?;
    avatar.insert(
        "revision".into(),
        Value::from(result.record.avatar_revision),
    );
    avatar.insert("config".into(), avatar_config_value(&result.record.avatar));
    if let Some(profile_avatar) = document["participant"]["profile"]
        .as_object_mut()
        .ok_or(BootstrapError::ReleaseUnavailable)?
        .get_mut("avatar")
        .and_then(Value::as_object_mut)
    {
        profile_avatar.insert(
            "revision".into(),
            Value::from(result.record.avatar_revision),
        );
        profile_avatar.insert("config".into(), avatar_config_value(&result.record.avatar));
    } else {
        return Err(BootstrapError::ReleaseUnavailable);
    }
    let preferences = document["participant"]["preferences"]
        .as_object_mut()
        .ok_or(BootstrapError::ReleaseUnavailable)?;
    preferences.insert(
        "theme".into(),
        Value::String(result.record.preferences.theme.as_str().into()),
    );
    preferences.insert(
        "low_power".into(),
        Value::Bool(result.record.preferences.low_power),
    );
    preferences.insert(
        "reduced_motion".into(),
        Value::Bool(result.record.preferences.reduced_motion),
    );
    preferences.insert(
        "sound_enabled".into(),
        Value::Bool(result.record.preferences.sound_enabled),
    );
    document["server_time"] = Value::String(
        result
            .record
            .completed_at
            .to_rfc3339_opts(SecondsFormat::Secs, true),
    );
    if result.record.restriction_revision < 1
        || result.record.event_revision < 1
        || result.record.participant_revision < 1
        || result.record.avatar_revision < 1
    {
        return Err(BootstrapError::InvalidResponse);
    }
    let bytes = serde_json::to_vec(&document).map_err(|_| BootstrapError::InvalidResponse)?;
    let document: Value =
        serde_json::from_slice(&bytes).map_err(|_| BootstrapError::InvalidResponse)?;
    validate_persisted_binding(&document, admission, result)?;
    super::schema::validate(&document).map_err(|_| BootstrapError::ReleaseUnavailable)?;
    Ok(serde_json::to_vec(&document).map_err(|_| BootstrapError::InvalidResponse)?)
}

fn avatar_config_value(selection: &AvatarSelection) -> Value {
    concert_domain::avatar::config_value(selection)
}

fn validate_persisted_binding(
    document: &Value,
    admission: &BootstrapAdmission,
    result: &BootstrapResult,
) -> Result<(), BootstrapError> {
    let event = document["event"]["event_id"].as_str();
    let participant = document["participant"]["user_id"].as_str();
    let profile = document["participant"]["profile"]["user_id"].as_str();
    let event_expected = admission.key().event_id.uuid().to_string();
    let user_expected = admission.key().user_id.uuid().to_string();
    if event != Some(event_expected.as_str())
        || participant != Some(user_expected.as_str())
        || profile != participant
        || document["participant"]["event_id"].as_str() != event
        || document["avatar"] != document["participant"]["profile"]["avatar"]
        || document["event"]["phase"].as_str() != Some(result.record.phase.as_str())
    {
        return Err(BootstrapError::InvalidResponse);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn product_virtual_concepts_preserve_unreviewed_place_truth_without_physical_or_fixture_waivers(
    ) {
        let reviewed = json!({"source_kind":"ORIGINAL_ART","review_state":"REVIEWED","source_id":"mcr_user_characters_20261103"});
        let mut values = vec![
            Value::Null,
            json!({"provenance":reviewed}),
            json!({"provenance":reviewed,
            "places":[{"place_id":"6a5c196b-58e1-47d5-99bb-000000000002","place_kind":"VIRTUAL_CONCEPT",
                "verification_state":"CONCEPT_ONLY","provenance":{"source_kind":"ORIGINAL_ART","review_state":"UNREVIEWED"}}]}),
        ];
        validate_release_provenance(&values, ReleasePolicy::Product).unwrap();
        let original = values.clone();
        for (field, value) in [
            ("place_kind", "PHYSICAL"),
            ("verification_state", "VERIFIED"),
        ] {
            values = original.clone();
            values[2]["places"][0][field] = value.into();
            assert!(validate_release_provenance(&values, ReleasePolicy::Product).is_err());
        }
        values = original.clone();
        values[2]["places"][0]["provenance"]["source_kind"] = "ORIGINAL_FIXTURE".into();
        assert!(validate_release_provenance(&values, ReleasePolicy::Product).is_err());
        values = original.clone();
        values[2]["places"][0]["provenance"]["review_state"] = "REJECTED".into();
        assert!(validate_release_provenance(&values, ReleasePolicy::Product).is_err());
        values = original.clone();
        values[2]["provenance"]["review_state"] = "UNREVIEWED".into();
        assert!(validate_release_provenance(&values, ReleasePolicy::Product).is_err());
        values = original;
        values[2]["places"][0]["place_kind"] = "PHYSICAL".into();
        values[2]["places"][0]["verification_state"] = "VERIFIED".into();
        values[2]["places"][0]["provenance"]["review_state"] = "REVIEWED".into();
        validate_release_provenance(&values, ReleasePolicy::Product).unwrap();
    }
    #[test]
    fn canonical_ascii_and_surrogates_match_c06_python() {
        assert_eq!(
            canonical_bytes(&json!({"z":"é 🎸 \u{7f}","a":1})).unwrap(),
            br#"{"a":1,"z":"\u00e9 \ud83c\udfb8 \u007f"}"#
        );
        assert!(canonical_bytes(&json!(0.5)).is_err());
    }
}
