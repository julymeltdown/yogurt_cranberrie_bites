//! Frozen C03/C06 avatar input and release-bound inventory. No URLs or sheet IDs.
use crate::{AssetId, AvatarSelection, Revision};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::{BTreeMap, BTreeSet};

pub const MAX_REQUEST_BYTES: usize = 4096;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConfigError {
    Invalid,
    Catalog,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireItems {
    body: String,
    eyes: String,
    hair: String,
    outfit: String,
    accessory: Value,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireConfig {
    contract_version: String,
    kind: String,
    manifest_id: String,
    manifest_version: i64,
    item_ids: WireItems,
    skin_tone_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRequest {
    expected_revision: i64,
    config: WireConfig,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SetAvatar {
    pub expected: Revision,
    pub selection: AvatarSelection,
}
impl SetAvatar {
    pub fn parse(bytes: &[u8]) -> Result<Self, ConfigError> {
        if bytes.is_empty() || bytes.len() > MAX_REQUEST_BYTES {
            return Err(ConfigError::Invalid);
        }
        // Struct deserialization rejects duplicate fields, unknown keys, floats,
        // booleans as integers, and missing required (including nullable) slots.
        let wire: WireRequest = serde_json::from_slice(bytes).map_err(|_| ConfigError::Invalid)?;
        Ok(Self {
            expected: revision(wire.expected_revision)?,
            selection: wire.config.selection()?,
        })
    }
}
fn asset(value: String) -> Result<AssetId, ConfigError> {
    AssetId::new(value).map_err(|_| ConfigError::Invalid)
}
fn revision(value: i64) -> Result<Revision, ConfigError> {
    Revision::new(value).map_err(|_| ConfigError::Invalid)
}
impl WireConfig {
    fn selection(self) -> Result<AvatarSelection, ConfigError> {
        if self.contract_version != "1.0.0" || self.kind != "avatar_config" {
            return Err(ConfigError::Invalid);
        }
        let accessory = match self.item_ids.accessory {
            Value::Null => None,
            Value::String(value) => Some(asset(value)?),
            _ => return Err(ConfigError::Invalid),
        };
        Ok(AvatarSelection {
            manifest_id: asset(self.manifest_id)?,
            manifest_version: revision(self.manifest_version)?,
            body: asset(self.item_ids.body)?,
            eyes: asset(self.item_ids.eyes)?,
            hair: asset(self.item_ids.hair)?,
            outfit: asset(self.item_ids.outfit)?,
            accessory,
            skin_tone: asset(self.skin_tone_id)?,
        })
    }
}
pub fn config_value(s: &AvatarSelection) -> Value {
    json!({"contract_version":"1.0.0","kind":"avatar_config","manifest_id":s.manifest_id.as_str(),
        "manifest_version":s.manifest_version.get(),"item_ids":{"body":s.body.as_str(),"eyes":s.eyes.as_str(),
        "hair":s.hair.as_str(),"outfit":s.outfit.as_str(),"accessory":s.accessory.as_ref().map(AssetId::as_str)},
        "skin_tone_id":s.skin_tone.as_str()})
}
/// Inventory projected from an already fully validated C06 release. This is
/// intentionally not a replacement for its schema, hash and resource checks.
#[derive(Clone, Debug)]
pub struct AvatarCatalog {
    manifest_id: AssetId,
    version: Revision,
    items: BTreeMap<String, String>,
    tones: BTreeSet<String>,
}
impl AvatarCatalog {
    pub fn from_validated_manifest(value: &Value) -> Result<Self, ConfigError> {
        let text = |v: &Value, key: &str| {
            v[key]
                .as_str()
                .map(str::to_owned)
                .ok_or(ConfigError::Catalog)
        };
        let mut items = BTreeMap::new();
        let mut layers = BTreeSet::new();
        for item in value["atlas"]["items"]
            .as_array()
            .ok_or(ConfigError::Catalog)?
        {
            let id = text(item, "item_id")?;
            asset(id.clone())?;
            let layer = text(item, "layer_id")?;
            if !["body", "eyes", "hair", "outfit", "accessory"].contains(&layer.as_str())
                || items.insert(id, layer.clone()).is_some()
            {
                return Err(ConfigError::Catalog);
            }
            layers.insert(layer);
        }
        if layers.len() != 5 {
            return Err(ConfigError::Catalog);
        }
        let mut tones = BTreeSet::new();
        for tone in value["skin_tones"].as_array().ok_or(ConfigError::Catalog)? {
            let id = text(tone, "skin_tone_id")?;
            asset(id.clone())?;
            if !tones.insert(id) {
                return Err(ConfigError::Catalog);
            }
        }
        if tones.is_empty() {
            return Err(ConfigError::Catalog);
        }
        Ok(Self {
            manifest_id: asset(text(value, "manifest_id")?)?,
            version: revision(
                value["manifest_version"]
                    .as_i64()
                    .ok_or(ConfigError::Catalog)?,
            )?,
            items,
            tones,
        })
    }
    pub fn validate(&self, s: &AvatarSelection) -> Result<(), ConfigError> {
        if s.manifest_id != self.manifest_id
            || s.manifest_version != self.version
            || !self.tones.contains(s.skin_tone.as_str())
        {
            return Err(ConfigError::Invalid);
        }
        for (id, layer) in [
            (&s.body, "body"),
            (&s.eyes, "eyes"),
            (&s.hair, "hair"),
            (&s.outfit, "outfit"),
        ] {
            if self.items.get(id.as_str()).map(String::as_str) != Some(layer) {
                return Err(ConfigError::Invalid);
            }
        }
        if let Some(id) = &s.accessory {
            if self.items.get(id.as_str()).map(String::as_str) != Some("accessory") {
                return Err(ConfigError::Invalid);
            }
        }
        Ok(())
    }
}
