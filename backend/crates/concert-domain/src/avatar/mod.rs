//! Frozen C03/C06 avatar input and release-bound inventory. No URLs or sheet IDs.
use crate::{AssetId, AvatarAppearance, AvatarSelection, Revision};
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
#[serde(tag = "contract_version", deny_unknown_fields)]
enum WireConfig {
    #[serde(rename = "1.0.0")]
    Layered {
        kind: String,
        manifest_id: String,
        manifest_version: i64,
        item_ids: WireItems,
        skin_tone_id: String,
    },
    #[serde(rename = "1.1.0")]
    WholeCharacter {
        kind: String,
        manifest_id: String,
        manifest_version: i64,
        representation: String,
        preset_id: String,
    },
}
#[derive(Deserialize)]
#[serde(untagged)]
enum AllWireConfig {
    Legacy(WireConfig),
    Reviewed(crate::avatar_motion_v2::reviewed::Config),
}
impl AllWireConfig {
    fn selection(self) -> Result<AvatarSelection, ConfigError> {
        match self {
            Self::Legacy(v) => v.selection(),
            Self::Reviewed(v) => Ok(v.into_selection()),
        }
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireRequest {
    expected_revision: i64,
    config: AllWireConfig,
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
        crate::avatar_motion_v2::strict_json::parse(bytes).map_err(|_| ConfigError::Invalid)?;
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
pub const SELECTED_MANIFEST_ID: &str = "mcr_selected_static_20261103";
pub const SELECTED_PRESETS: [&str; 6] = [
    "photo01_front_01",
    "photo01_front_02",
    "photo01_front_03",
    "photo01_front_04",
    "photo01_front_05",
    "photo01_front_06",
];
impl WireConfig {
    fn selection(self) -> Result<AvatarSelection, ConfigError> {
        let (kind, manifest_id, manifest_version, appearance) = match self {
            Self::Layered {
                kind,
                manifest_id,
                manifest_version,
                item_ids,
                skin_tone_id,
            } => {
                let accessory = match item_ids.accessory {
                    Value::Null => None,
                    Value::String(value) => Some(asset(value)?),
                    _ => return Err(ConfigError::Invalid),
                };
                (
                    kind,
                    manifest_id,
                    manifest_version,
                    AvatarAppearance::Layered {
                        body: asset(item_ids.body)?,
                        eyes: asset(item_ids.eyes)?,
                        hair: asset(item_ids.hair)?,
                        outfit: asset(item_ids.outfit)?,
                        accessory,
                        skin_tone: asset(skin_tone_id)?,
                    },
                )
            }
            Self::WholeCharacter {
                kind,
                manifest_id,
                manifest_version,
                representation,
                preset_id,
            } => {
                if representation != "whole_character"
                    || manifest_id != SELECTED_MANIFEST_ID
                    || manifest_version != 1
                    || !SELECTED_PRESETS.contains(&preset_id.as_str())
                {
                    return Err(ConfigError::Invalid);
                }
                (
                    kind,
                    manifest_id,
                    manifest_version,
                    AvatarAppearance::WholeCharacter {
                        preset_id: asset(preset_id)?,
                    },
                )
            }
        };
        if kind != "avatar_config" {
            return Err(ConfigError::Invalid);
        }
        Ok(AvatarSelection {
            manifest_id: asset(manifest_id)?,
            manifest_version: revision(manifest_version)?,
            appearance,
        })
    }
}
/// Strict configuration decoder shared by validated release projection and HTTP commands.
pub fn parse_config(bytes: &[u8]) -> Result<AvatarSelection, ConfigError> {
    if bytes.is_empty() || bytes.len() > MAX_REQUEST_BYTES {
        return Err(ConfigError::Invalid);
    }
    crate::avatar_motion_v2::strict_json::parse(bytes).map_err(|_| ConfigError::Invalid)?;
    serde_json::from_slice::<AllWireConfig>(bytes)
        .map_err(|_| ConfigError::Invalid)?
        .selection()
}
pub fn config_value(s: &AvatarSelection) -> Value {
    match &s.appearance {
        AvatarAppearance::ReviewedMotion(appearance) => {
            let mut value = serde_json::to_value(
                crate::avatar_motion_v2::reviewed::Config::from_appearance(appearance.clone()),
            )
            .expect("closed config serialization");
            value["manifest_id"] = json!(s.manifest_id.as_str());
            value["manifest_version"] = json!(s.manifest_version.get());
            value
        }
        AvatarAppearance::Layered {
            body,
            eyes,
            hair,
            outfit,
            accessory,
            skin_tone,
        } => {
            json!({"contract_version":"1.0.0","kind":"avatar_config","manifest_id":s.manifest_id.as_str(),
                "manifest_version":s.manifest_version.get(),"item_ids":{"body":body.as_str(),"eyes":eyes.as_str(),
                "hair":hair.as_str(),"outfit":outfit.as_str(),"accessory":accessory.as_ref().map(AssetId::as_str)},
                "skin_tone_id":skin_tone.as_str()})
        }
        AvatarAppearance::WholeCharacter { preset_id } => {
            json!({"contract_version":"1.1.0","kind":"avatar_config","manifest_id":s.manifest_id.as_str(),
                "manifest_version":s.manifest_version.get(),"representation":"whole_character","preset_id":preset_id.as_str()})
        }
    }
}
/// Inventory projected from an already fully validated C06 release. This is
/// intentionally not a replacement for its schema, hash and resource checks.
#[derive(Clone, Debug)]
pub struct AvatarCatalog {
    motion: Option<crate::avatar_motion_v2::reviewed::VisuallyReviewedCatalog>,
    manifest_id: AssetId,
    version: Revision,
    items: BTreeMap<String, String>,
    tones: BTreeSet<String>,
    presets: Option<BTreeSet<String>>,
}
impl AvatarCatalog {
    pub fn from_validated_manifest(value: &Value) -> Result<Self, ConfigError> {
        if value["contract_version"] == "2.0.0" {
            let motion =
                crate::avatar_motion_v2::reviewed::VisuallyReviewedCatalog::from_manifest(value)
                    .map_err(|_| ConfigError::Catalog)?;
            return Ok(Self {
                motion: Some(motion),
                manifest_id: asset(crate::avatar_motion_v2::reviewed::MANIFEST_ID.into())?,
                version: revision(2)?,
                items: BTreeMap::new(),
                tones: BTreeSet::new(),
                presets: None,
            });
        }
        if value["contract_version"] == "1.1.0"
            && value["kind"] == "avatar_manifest"
            && value["representation"] == "whole_character"
        {
            let mut presets = BTreeSet::new();
            for preset in value["presets"].as_array().ok_or(ConfigError::Catalog)? {
                let selection = parse_config(
                    &serde_json::to_vec(&preset["config"]).map_err(|_| ConfigError::Catalog)?,
                )?;
                let AvatarAppearance::WholeCharacter { preset_id } = selection.appearance else {
                    return Err(ConfigError::Catalog);
                };
                if preset["preset_id"].as_str() != Some(preset_id.as_str())
                    || !presets.insert(preset_id.as_str().to_owned())
                {
                    return Err(ConfigError::Catalog);
                }
            }
            if value["manifest_id"] != SELECTED_MANIFEST_ID
                || value["manifest_version"] != 1
                || presets != SELECTED_PRESETS.into_iter().map(str::to_owned).collect()
                || !value["default_preset_id"]
                    .as_str()
                    .is_some_and(|id| presets.contains(id))
            {
                return Err(ConfigError::Catalog);
            }
            return Ok(Self {
                motion: None,
                manifest_id: asset(SELECTED_MANIFEST_ID.into())?,
                version: revision(1)?,
                items: BTreeMap::new(),
                tones: BTreeSet::new(),
                presets: Some(presets),
            });
        }
        if value["contract_version"] != "1.0.0"
            || value["kind"] != "avatar_manifest"
            || value["layer_order"] != json!(["body", "eyes", "hair", "outfit", "accessory"])
        {
            return Err(ConfigError::Catalog);
        }
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
            motion: None,
            manifest_id: asset(text(value, "manifest_id")?)?,
            version: revision(
                value["manifest_version"]
                    .as_i64()
                    .ok_or(ConfigError::Catalog)?,
            )?,
            items,
            tones,
            presets: None,
        })
    }
    pub fn validate(&self, s: &AvatarSelection) -> Result<(), ConfigError> {
        if let Some(motion) = &self.motion {
            return motion
                .validate_selection(s)
                .map_err(|_| ConfigError::Invalid);
        }
        if s.manifest_id != self.manifest_id || s.manifest_version != self.version {
            return Err(ConfigError::Invalid);
        }
        match (&self.presets, &s.appearance) {
            (Some(presets), AvatarAppearance::WholeCharacter { preset_id })
                if presets.contains(preset_id.as_str()) =>
            {
                Ok(())
            }
            (
                None,
                AvatarAppearance::Layered {
                    body,
                    eyes,
                    hair,
                    outfit,
                    accessory,
                    skin_tone,
                },
            ) => {
                if !self.tones.contains(skin_tone.as_str()) {
                    return Err(ConfigError::Invalid);
                }
                for (id, layer) in [
                    (body, "body"),
                    (eyes, "eyes"),
                    (hair, "hair"),
                    (outfit, "outfit"),
                ] {
                    if self.items.get(id.as_str()).map(String::as_str) != Some(layer) {
                        return Err(ConfigError::Invalid);
                    }
                }
                if let Some(id) = accessory {
                    if self.items.get(id.as_str()).map(String::as_str) != Some("accessory") {
                        return Err(ConfigError::Invalid);
                    }
                }
                Ok(())
            }
            _ => Err(ConfigError::Invalid),
        }
    }
}
