//! Distinct reviewed successor. Structural decoding, visual proof and verified
//! resources are separate types; none is a caller-supplied admission boolean.
use super::{ConfigKind, ContractVersion, ItemIds, MotionError, PresetId, Resource};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};

pub const MANIFEST_ID: &str = "mcr_selected_motion_reviewed_20261104";
pub const MANIFEST_VERSION: u8 = 2;
pub const MANIFEST_JSON: &str =
    include_str!("../../../../releases/selected-character-motion-v2/reviewed/manifest.json");
pub const REVIEW_JSON: &str =
    include_str!("../../../../../docs/mcr/contracts/avatar-motion-v2/review/visual-review.json");
closed_strings!(ReviewedId { Selected => "mcr_selected_motion_reviewed_20261104" });
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(try_from = "u8", into = "u8")]
pub struct ReviewedVersion(u8);
impl TryFrom<u8> for ReviewedVersion {
    type Error = &'static str;
    fn try_from(v: u8) -> Result<Self, Self::Error> {
        if v == 2 {
            Ok(Self(2))
        } else {
            Err("unknown reviewed revision")
        }
    }
}
impl From<ReviewedVersion> for u8 {
    fn from(v: ReviewedVersion) -> Self {
        v.0
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Appearance {
    Preset(PresetId),
    Layered(ItemIds),
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "representation", deny_unknown_fields)]
pub enum Config {
    #[serde(rename = "motion_preset")]
    Preset {
        contract_version: ContractVersion,
        kind: ConfigKind,
        manifest_id: ReviewedId,
        manifest_version: ReviewedVersion,
        preset_id: PresetId,
    },
    #[serde(rename = "motion_layered")]
    Layered {
        contract_version: ContractVersion,
        kind: ConfigKind,
        manifest_id: ReviewedId,
        manifest_version: ReviewedVersion,
        item_ids: ItemIds,
    },
}
impl Config {
    pub fn decode(bytes: &[u8]) -> Result<Self, MotionError> {
        if bytes.len() > 4096 {
            return Err(MotionError::TooLarge);
        }
        super::strict_json::parse(bytes)?;
        Ok(serde_json::from_slice(bytes)?)
    }
    pub fn appearance(&self) -> Appearance {
        match self {
            Self::Preset { preset_id, .. } => Appearance::Preset(*preset_id),
            Self::Layered { item_ids, .. } => Appearance::Layered(item_ids.clone()),
        }
    }
    pub fn from_appearance(a: Appearance) -> Self {
        match a {
            Appearance::Preset(preset_id) => Self::Preset {
                contract_version: ContractVersion::V2,
                kind: ConfigKind::AvatarConfig,
                manifest_id: ReviewedId::Selected,
                manifest_version: ReviewedVersion(2),
                preset_id,
            },
            Appearance::Layered(item_ids) => Self::Layered {
                contract_version: ContractVersion::V2,
                kind: ConfigKind::AvatarConfig,
                manifest_id: ReviewedId::Selected,
                manifest_version: ReviewedVersion(2),
                item_ids,
            },
        }
    }
    pub fn into_selection(self) -> crate::AvatarSelection {
        crate::AvatarSelection {
            manifest_id: crate::AssetId::new(MANIFEST_ID.into()).expect("fixed valid ID"),
            manifest_version: crate::Revision::new(2).expect("fixed valid revision"),
            appearance: crate::AvatarAppearance::ReviewedMotion(self.appearance()),
        }
    }
}

#[derive(Clone, Debug)]
pub struct StructuralCatalog {
    document: Value,
    candidate: super::Catalog,
}
#[derive(Clone, Debug)]
pub struct VisuallyReviewedCatalog {
    structure: StructuralCatalog,
}
#[derive(Clone, Debug)]
pub struct EligibleCatalog {
    reviewed: VisuallyReviewedCatalog,
}
fn invalid() -> MotionError {
    MotionError::ArtNotReviewed
}
impl StructuralCatalog {
    pub fn decode(bytes: &[u8]) -> Result<Self, MotionError> {
        if bytes.len() > 256 * 1024 {
            return Err(MotionError::TooLarge);
        }
        let document = super::strict_json::parse(bytes)?;
        let expected: Value = serde_json::from_str(MANIFEST_JSON)?;
        if document != expected {
            return Err(MotionError::CatalogMismatch);
        }
        let mut preimage = document.clone();
        preimage
            .as_object_mut()
            .ok_or_else(invalid)?
            .remove("content_hash");
        let canonical = crate::place::canonical_json_bytes(&serde_json::to_vec(&preimage)?)
            .map_err(|_| invalid())?;
        if document["content_hash"] != format!("sha256:{:x}", Sha256::digest(canonical)) {
            return Err(MotionError::CatalogMismatch);
        }
        // The successor changes authority/envelope, never the immutable art mapping.
        let mut original = document.clone();
        for k in [
            "content_hash",
            "review",
            "candidate_manifest",
            "default_preset_id",
        ] {
            original.as_object_mut().ok_or_else(invalid)?.remove(k);
        }
        original["manifest_id"] = json!(super::MANIFEST_ID);
        original["manifest_version"] = json!(1);
        original["representation"] = json!("selected_source_motion");
        original["qualification"] = json!("CANDIDATE_ONLY");
        original["production_eligible"] = json!(false);
        for preset in original["presets"].as_array_mut().ok_or_else(invalid)? {
            preset.as_object_mut().ok_or_else(invalid)?.remove("config");
        }
        let candidate = super::Catalog::decode_candidate(&serde_json::to_vec(&original)?)?;
        Ok(Self {
            document,
            candidate,
        })
    }
    pub fn bind_visual_review(self, bytes: &[u8]) -> Result<VisuallyReviewedCatalog, MotionError> {
        let pin: Resource = serde_json::from_value(self.document["review"].clone())?;
        verify_pin(&pin, bytes)?;
        let review = super::strict_json::parse(bytes)?;
        let expected = super::strict_json::parse(REVIEW_JSON.as_bytes())?;
        if review != expected
            || review["decision"] != "VISUAL_ACCEPTED"
            || review["originals_modified"] != false
            || review["art_generated"] != false
            || review["candidate_manifest"] != self.document["candidate_manifest"]
            || review["atlas_sha256"] != self.document["atlas"]["sha256"]
        {
            return Err(invalid());
        }
        Ok(VisuallyReviewedCatalog { structure: self })
    }
}
impl VisuallyReviewedCatalog {
    pub fn from_manifest(value: &Value) -> Result<Self, MotionError> {
        StructuralCatalog::decode(&serde_json::to_vec(value)?)?
            .bind_visual_review(REVIEW_JSON.as_bytes())
    }
    pub fn candidate(&self) -> &super::Catalog {
        &self.structure.candidate
    }
    pub fn document(&self) -> &Value {
        &self.structure.document
    }
    pub fn validate_selection(
        &self,
        selection: &crate::AvatarSelection,
    ) -> Result<(), MotionError> {
        if selection.manifest_id.as_str() != MANIFEST_ID || selection.manifest_version.get() != 2 {
            return Err(invalid());
        }
        let crate::AvatarAppearance::ReviewedMotion(appearance) = &selection.appearance else {
            return Err(invalid());
        };
        let config = match appearance {
            Appearance::Preset(p) => super::AvatarConfig::preset(*p),
            Appearance::Layered(i) => super::AvatarConfig::layered(i.clone()),
        };
        self.candidate().resolve(&config);
        Ok(())
    }
    pub fn verify_resources<F>(self, mut read: F) -> Result<EligibleCatalog, MotionError>
    where
        F: FnMut(&str) -> Result<Vec<u8>, MotionError>,
    {
        for pin in self.candidate().resources() {
            verify_pin(pin, &read(&pin.path)?)?;
        }
        let review = super::strict_json::parse(REVIEW_JSON.as_bytes())?;
        let mut pins = vec![
            self.document()["review"].clone(),
            self.document()["candidate_manifest"].clone(),
            review["additional_seam_report"].clone(),
            review["additional_seam_checker"].clone(),
        ];
        pins.extend(
            review["inspected_resources"]
                .as_array()
                .ok_or_else(invalid)?
                .iter()
                .cloned(),
        );
        for pin in pins {
            let pin: Resource = serde_json::from_value(pin)?;
            verify_pin(&pin, &read(&pin.path)?)?;
        }
        Ok(EligibleCatalog { reviewed: self })
    }
}
impl EligibleCatalog {
    pub fn reviewed(&self) -> &VisuallyReviewedCatalog {
        &self.reviewed
    }
}
fn verify_pin(pin: &Resource, bytes: &[u8]) -> Result<(), MotionError> {
    if pin.bytes != bytes.len() as u64 || pin.sha256 != format!("{:x}", Sha256::digest(bytes)) {
        return Err(MotionError::ResourceMismatch);
    }
    if let (Some(w), Some(h)) = (pin.width, pin.height) {
        if bytes.len() < 24
            || &bytes[..8] != b"\x89PNG\r\n\x1a\n"
            || &bytes[12..16] != b"IHDR"
            || u32::from_be_bytes(bytes[16..20].try_into().unwrap()) != w
            || u32::from_be_bytes(bytes[20..24].try_into().unwrap()) != h
        {
            return Err(MotionError::ResourceMismatch);
        }
    }
    Ok(())
}
