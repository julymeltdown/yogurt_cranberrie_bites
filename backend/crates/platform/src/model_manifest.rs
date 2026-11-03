use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::error::Error;
use std::fmt::{Display, Formatter};

const MAX_EMBEDDING_DIMENSION: usize = 2_048;
const MIN_EMBEDDING_DIMENSION: usize = 16;
const MAX_NAME_LENGTH: usize = 128;
const MAX_URI_LENGTH: usize = 1_024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum ModelStatus {
    Draft,
    Shadow,
    Ready,
    Active,
    Retired,
    Revoked,
}

impl ModelStatus {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "DRAFT",
            Self::Shadow => "SHADOW",
            Self::Ready => "READY",
            Self::Active => "ACTIVE",
            Self::Retired => "RETIRED",
            Self::Revoked => "REVOKED",
        }
    }

    pub fn parse(value: &str) -> Result<Self, ModelManifestError> {
        match value.trim().to_ascii_uppercase().as_str() {
            "DRAFT" => Ok(Self::Draft),
            "SHADOW" => Ok(Self::Shadow),
            "READY" => Ok(Self::Ready),
            "ACTIVE" => Ok(Self::Active),
            "RETIRED" => Ok(Self::Retired),
            "REVOKED" => Ok(Self::Revoked),
            _ => Err(ModelManifestError::new("Unsupported model status")),
        }
    }

    pub fn can_serve(self) -> bool {
        matches!(self, Self::Ready | Self::Active)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelManifest {
    pub model_name: String,
    pub model_version: String,
    pub feature_schema_version: String,
    pub artifact_uri: String,
    pub sha256: String,
    pub embedding_dimension: Option<usize>,
    pub activated_at: Option<DateTime<Utc>>,
    pub fallback_version: String,
    pub status: ModelStatus,
}

impl ModelManifest {
    pub fn validate(&self, production: bool) -> Result<(), ModelManifestError> {
        validate_text(&self.model_name, "Model name")?;
        validate_text(&self.model_version, "Model version")?;
        validate_text(&self.feature_schema_version, "Feature schema version")?;
        validate_text(&self.fallback_version, "Fallback version")?;
        if self.model_version == self.fallback_version {
            return Err(ModelManifestError::new(
                "Model version and fallback version must differ",
            ));
        }
        if self.artifact_uri.len() > MAX_URI_LENGTH || self.artifact_uri.trim().is_empty() {
            return Err(ModelManifestError::new("Model artifact URI is invalid"));
        }
        if production && !is_remote_artifact(&self.artifact_uri) {
            return Err(ModelManifestError::new(
                "Production model artifact must use a remote immutable URI",
            ));
        }
        if self.sha256.len() != 64
            || !self
                .sha256
                .bytes()
                .all(|value| value.is_ascii_digit() || (b'a'..=b'f').contains(&value))
        {
            return Err(ModelManifestError::new(
                "Model SHA-256 must be 64 lowercase hexadecimal characters",
            ));
        }
        if self.embedding_dimension.is_some_and(|dimension| {
            !(MIN_EMBEDDING_DIMENSION..=MAX_EMBEDDING_DIMENSION).contains(&dimension)
        }) {
            return Err(ModelManifestError::new(
                "Model embedding dimension is out of range",
            ));
        }
        if self.status == ModelStatus::Active && self.activated_at.is_none() {
            return Err(ModelManifestError::new(
                "Active model requires an activation timestamp",
            ));
        }
        Ok(())
    }
}

fn validate_text(value: &str, label: &str) -> Result<(), ModelManifestError> {
    if value.len() > MAX_NAME_LENGTH || value.trim().is_empty() {
        return Err(ModelManifestError::new(format!("{label} is invalid")));
    }
    Ok(())
}

fn is_remote_artifact(value: &str) -> bool {
    let value = value.trim().to_ascii_lowercase();
    ["s3://", "gs://", "https://", "oci://"]
        .iter()
        .any(|prefix| value.starts_with(prefix))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModelManifestError {
    message: String,
}

impl ModelManifestError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

impl Display for ModelManifestError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl Error for ModelManifestError {}
