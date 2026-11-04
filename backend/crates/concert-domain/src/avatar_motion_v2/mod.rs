//! Closed selected-source motion 2.0.0 candidate contract.
//!
//! Decoding and byte verification establish a pinned candidate, never production
//! admission. The old AvatarSelection/HTTP/persistence dispatch remains separate.
use serde::{Deserialize, Deserializer, Serialize};
use sha2::{Digest, Sha256};

pub const CONTRACT_VERSION: &str = "2.0.0";
pub const MANIFEST_ID: &str = "mcr_selected_motion_20261103";
pub const MANIFEST_VERSION: u8 = 1;
pub const MANIFEST_JSON: &str =
    include_str!("../../../../releases/selected-character-motion-v2/manifest.json");

macro_rules! closed_strings {
    ($name:ident { $($variant:ident => $wire:literal),+ $(,)? }) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
        pub enum $name { $(#[serde(rename = $wire)] $variant),+ }
        impl $name { pub fn as_str(self) -> &'static str { match self { $(Self::$variant => $wire),+ } } }
    };
}
closed_strings!(ContractVersion { V2 => "2.0.0" });
closed_strings!(ConfigKind { AvatarConfig => "avatar_config" });
closed_strings!(ManifestId { SelectedMotion => "mcr_selected_motion_20261103" });
closed_strings!(Direction { Down => "down", Left => "left", Right => "right", Up => "up" });
closed_strings!(SourceDirection { Front => "front", Left => "left", Right => "right", Rear => "rear" });
closed_strings!(Slot { Body => "body", Eyes => "eyes", Hair => "hair", Outfit => "outfit", Accessory => "accessory" });
closed_strings!(PresetId {
    P01 => "motion_v2_photo01_front_01", P02 => "motion_v2_photo01_front_02",
    P03 => "motion_v2_photo01_front_03", P04 => "motion_v2_photo01_front_04",
    P05 => "motion_v2_photo01_front_05", P06 => "motion_v2_photo01_front_06"
});
closed_strings!(BodyId {
    B0 => "motion_v2_body_0", B1 => "motion_v2_body_1", B2 => "motion_v2_body_2", B3 => "motion_v2_body_3",
    B4 => "motion_v2_body_4", B5 => "motion_v2_body_5", B6 => "motion_v2_body_6", B7 => "motion_v2_body_7"
});
closed_strings!(EyesId { E0 => "motion_v2_eyes_0", E1 => "motion_v2_eyes_1", E2 => "motion_v2_eyes_2", E3 => "motion_v2_eyes_3" });
closed_strings!(HairId {
    H0 => "motion_v2_hair_0", H1 => "motion_v2_hair_1", H2 => "motion_v2_hair_2",
    H3 => "motion_v2_hair_3", H4 => "motion_v2_hair_4", H5 => "motion_v2_hair_5"
});
closed_strings!(OutfitId {
    O0 => "motion_v2_outfit_0", O1 => "motion_v2_outfit_1", O2 => "motion_v2_outfit_2",
    O3 => "motion_v2_outfit_3", O4 => "motion_v2_outfit_4", O5 => "motion_v2_outfit_5"
});
closed_strings!(AccessoryId { A0 => "motion_v2_accessory_0", A1 => "motion_v2_accessory_1", A2 => "motion_v2_accessory_2", A3 => "motion_v2_accessory_3" });

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct ManifestVersion(u8);
impl TryFrom<u8> for ManifestVersion {
    type Error = &'static str;
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        if value == MANIFEST_VERSION {
            Ok(Self(value))
        } else {
            Err("unknown motion manifest version")
        }
    }
}
impl From<ManifestVersion> for u8 {
    fn from(value: ManifestVersion) -> Self {
        value.0
    }
}
impl ManifestVersion {
    pub fn current() -> Self {
        Self(MANIFEST_VERSION)
    }
}

// deserialize_with deliberately makes nullable fields REQUIRED. serde's ordinary
// Option<T> permits missing fields, which would silently widen the frozen schema.
fn required_nullable<'de, D: Deserializer<'de>, T: Deserialize<'de>>(
    d: D,
) -> Result<Option<T>, D::Error> {
    Option::<T>::deserialize(d)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ItemIds {
    pub body: BodyId,
    pub eyes: EyesId,
    pub hair: HairId,
    #[serde(deserialize_with = "required_nullable")]
    pub outfit: Option<OutfitId>,
    #[serde(deserialize_with = "required_nullable")]
    pub accessory: Option<AccessoryId>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "representation", deny_unknown_fields)]
pub enum AvatarConfig {
    #[serde(rename = "motion_preset")]
    Preset {
        contract_version: ContractVersion,
        kind: ConfigKind,
        manifest_id: ManifestId,
        manifest_version: ManifestVersion,
        preset_id: PresetId,
    },
    #[serde(rename = "motion_layered")]
    Layered {
        contract_version: ContractVersion,
        kind: ConfigKind,
        manifest_id: ManifestId,
        manifest_version: ManifestVersion,
        item_ids: ItemIds,
    },
}
impl AvatarConfig {
    pub fn preset(preset_id: PresetId) -> Self {
        Self::Preset {
            contract_version: ContractVersion::V2,
            kind: ConfigKind::AvatarConfig,
            manifest_id: ManifestId::SelectedMotion,
            manifest_version: ManifestVersion::current(),
            preset_id,
        }
    }
    pub fn layered(item_ids: ItemIds) -> Self {
        Self::Layered {
            contract_version: ContractVersion::V2,
            kind: ConfigKind::AvatarConfig,
            manifest_id: ManifestId::SelectedMotion,
            manifest_version: ManifestVersion::current(),
            item_ids,
        }
    }
}

#[derive(Debug, thiserror::Error)]
pub enum MotionError {
    #[error("invalid motion JSON: {0}")]
    Json(#[from] serde_json::Error),
    #[error("motion input exceeds its bounded contract size")]
    TooLarge,
    #[error("motion catalog differs from immutable release 1")]
    CatalogMismatch,
    #[error("resource path is outside the pinned catalog")]
    UnknownResource,
    #[error("resource bytes/hash/dimensions differ from the pin")]
    ResourceMismatch,
    #[error("selected-source motion candidate is not reviewed for production")]
    ArtNotReviewed,
}

/// Candidate/config validity only; callers must not treat this as art admission.
pub fn decode_config(bytes: &[u8]) -> Result<AvatarConfig, MotionError> {
    if bytes.len() > 4096 {
        return Err(MotionError::TooLarge);
    }
    Ok(serde_json::from_slice(bytes)?)
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Resource {
    pub path: String,
    pub sha256: String,
    pub bytes: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub width: Option<u32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub height: Option<u32>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Original {
    pub source_id: String,
    pub resource: Resource,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Source {
    pub pack_id: String,
    pub revision: u32,
    pub descriptor: Resource,
    pub originals: Vec<Original>,
    pub art_source_revision: String,
    pub art_receipt_revision: String,
    pub resources: Vec<Resource>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectionAliases {
    pub down: SourceDirection,
    pub left: SourceDirection,
    pub right: SourceDirection,
    pub up: SourceDirection,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Geometry {
    pub cell: [u16; 2],
    pub foot: [u16; 2],
    pub directions: [Direction; 4],
    pub source_direction_aliases: DirectionAliases,
    pub phase_order: [String; 5],
    pub output_cells: u8,
    pub physical_frames: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Composition {
    pub layer_order: [String; 6],
    pub dressed_body_clip_y_exclusive: u16,
    pub hand_palette_color_index: u8,
    pub skin_policy: String,
    pub rear_eyes: String,
    pub stationary_uv_sharing: bool,
    pub combined_indices_already_offset: bool,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DirectionFrames {
    pub direction: Direction,
    pub frames: [u16; 5],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hand_frames: Option<[u16; 5]>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visibility: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SourceFrame {
    pub preset: String,
    pub direction: SourceDirection,
    pub phase: u8,
    pub pose: String,
    pub attribution: String,
    pub source: String,
    #[serde(rename = "atlasIndex")]
    pub atlas_index: u16,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Preset {
    pub preset_id: PresetId,
    pub local_key: String,
    pub source_pointer: String,
    pub directions: [DirectionFrames; 4],
    pub source_frames: Vec<SourceFrame>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ItemId {
    Body(BodyId),
    Eyes(EyesId),
    Hair(HairId),
    Outfit(OutfitId),
    Accessory(AccessoryId),
}
impl ItemId {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Body(v) => v.as_str(),
            Self::Eyes(v) => v.as_str(),
            Self::Hair(v) => v.as_str(),
            Self::Outfit(v) => v.as_str(),
            Self::Accessory(v) => v.as_str(),
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub struct SkinPalette {
    pub source_id: String,
    pub body_source_index: u8,
    pub method: String,
    pub colors: [[u8; 3]; 4],
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Item {
    pub item_id: ItemId,
    pub slot: Slot,
    pub local_key: String,
    pub source_pointer: String,
    pub selected_source_ids: Vec<String>,
    pub attribution: String,
    pub phase_motion: String,
    pub directions: [DirectionFrames; 4],
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_location: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub authored_resource: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_row: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skin_palette: Option<SkinPalette>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cuff_policy: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Manifest {
    pub contract_version: ContractVersion,
    pub kind: String,
    pub manifest_id: ManifestId,
    pub manifest_version: ManifestVersion,
    pub representation: String,
    pub production_eligible: bool,
    pub qualification: String,
    pub fallback: String,
    pub source: Source,
    pub atlas: Resource,
    pub geometry: Geometry,
    pub composition: Composition,
    pub presets: Vec<Preset>,
    pub items: Vec<Item>,
}

/// A validated immutable catalog; construction cannot accept relabelled art.
#[derive(Clone, Debug)]
pub struct Catalog {
    manifest: Manifest,
}
#[derive(Debug)]
pub enum ResolvedSelection<'a> {
    Preset(&'a Preset),
    Layered {
        body: &'a Item,
        eyes: &'a Item,
        hair: &'a Item,
        outfit: Option<&'a Item>,
        accessory: Option<&'a Item>,
    },
}
impl Catalog {
    pub fn canonical_candidate() -> Self {
        Self {
            manifest: serde_json::from_str(MANIFEST_JSON)
                .expect("embedded motion catalog is checked by fixture tests"),
        }
    }
    pub fn decode_candidate(bytes: &[u8]) -> Result<Self, MotionError> {
        if bytes.len() > 256 * 1024 {
            return Err(MotionError::TooLarge);
        }
        // First parse typed fields to reject duplicates/unknown fields. Then compare
        // JSON values too: typed Option fields must not normalize explicit nulls,
        // missing keys or other extensions into this exact immutable catalog.
        let manifest: Manifest = serde_json::from_slice(bytes)?;
        let actual: serde_json::Value = serde_json::from_slice(bytes)?;
        let expected: serde_json::Value = serde_json::from_str(MANIFEST_JSON)?;
        if actual != expected {
            return Err(MotionError::CatalogMismatch);
        }
        Ok(Self { manifest })
    }
    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }
    pub fn manifest_sha256(&self) -> String {
        format!("{:x}", Sha256::digest(MANIFEST_JSON.as_bytes()))
    }
    pub fn resolve(&self, config: &AvatarConfig) -> ResolvedSelection<'_> {
        let item = |id: &str| {
            self.manifest
                .items
                .iter()
                .find(|v| v.item_id.as_str() == id)
                .expect("closed ID belongs to immutable catalog")
        };
        match config {
            AvatarConfig::Preset { preset_id, .. } => ResolvedSelection::Preset(
                self.manifest
                    .presets
                    .iter()
                    .find(|p| p.preset_id == *preset_id)
                    .expect("closed preset belongs to catalog"),
            ),
            AvatarConfig::Layered { item_ids, .. } => ResolvedSelection::Layered {
                body: item(item_ids.body.as_str()),
                eyes: item(item_ids.eyes.as_str()),
                hair: item(item_ids.hair.as_str()),
                outfit: item_ids.outfit.map(|v| item(v.as_str())),
                accessory: item_ids.accessory.map(|v| item(v.as_str())),
            },
        }
    }
    pub fn resources(&self) -> impl Iterator<Item = &Resource> {
        std::iter::once(&self.manifest.atlas)
            .chain(std::iter::once(&self.manifest.source.descriptor))
            .chain(self.manifest.source.resources.iter())
            .chain(self.manifest.source.originals.iter().map(|v| &v.resource))
    }
    /// Caller supplies bytes from a trusted local resource resolver. No client URL
    /// or path is opened by this pure domain API. Exact hash closes PNG contents.
    pub fn verify_resource(&self, path: &str, bytes: &[u8]) -> Result<(), MotionError> {
        let pin = self
            .resources()
            .find(|pin| pin.path == path)
            .ok_or(MotionError::UnknownResource)?;
        if bytes.len() as u64 != pin.bytes || format!("{:x}", Sha256::digest(bytes)) != pin.sha256 {
            return Err(MotionError::ResourceMismatch);
        }
        if let (Some(width), Some(height)) = (pin.width, pin.height) {
            if bytes.len() < 24
                || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
                || &bytes[12..16] != b"IHDR"
                || u32::from_be_bytes(bytes[16..20].try_into().unwrap()) != width
                || u32::from_be_bytes(bytes[20..24].try_into().unwrap()) != height
            {
                return Err(MotionError::ResourceMismatch);
            }
        }
        Ok(())
    }
    /// No bool, caller assertion or candidate flag can manufacture an art review.
    /// A future reviewed release needs a separately versioned admission policy.
    pub fn admit_production(&self) -> Result<(), MotionError> {
        Err(MotionError::ArtNotReviewed)
    }
}
