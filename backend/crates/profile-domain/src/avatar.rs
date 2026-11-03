use anyhow::{bail, Result};
use std::fmt::{Debug, Formatter};

const ALLOWED_CONTENT_TYPES: &[&str] = &["image/png", "image/jpeg", "image/webp", "image/gif"];

/// Largest avatar accepted, before decoding and after canonical encoding.
pub const MAX_AVATAR_BYTES: usize = 1_048_576;

/// Largest decoded bitmap accepted, in pixels.
pub const MAX_AVATAR_PIXELS: u64 = 4096 * 4096;

#[derive(Clone, PartialEq, Eq)]
pub struct Avatar {
    pub content: Vec<u8>,
    pub content_type: String,
    pub filename: String,
}

impl Debug for Avatar {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Avatar")
            .field("content", &"[REDACTED]")
            .field("content_len", &self.content.len())
            .field("content_type", &self.content_type)
            .field("filename", &"[REDACTED]")
            .finish()
    }
}

impl Avatar {
    /// Build an avatar from bytes already validated and canonicalized by the
    /// media-processing adapter. This constructor enforces value-level rules;
    /// parsing image containers belongs outside the domain layer.
    pub fn from_processed(
        content: Vec<u8>,
        content_type: impl AsRef<str>,
        filename: impl AsRef<str>,
    ) -> Result<Self> {
        let content_type = normalize_avatar_content_type(content_type.as_ref())?;
        if content.is_empty() {
            bail!("Avatar content is empty");
        }
        if content.len() > MAX_AVATAR_BYTES {
            bail!(
                "Avatar exceeds the {MAX_AVATAR_BYTES} byte limit; received {} bytes",
                content.len()
            );
        }

        Ok(Self {
            content,
            content_type,
            filename: filename.as_ref().trim().to_owned(),
        })
    }
}

/// Normalize and validate the content type accepted for an avatar.
pub fn normalize_avatar_content_type(content_type: &str) -> Result<String> {
    let normalized = content_type.trim().to_ascii_lowercase();
    if normalized.is_empty() {
        bail!("Avatar content type is required");
    }
    if !ALLOWED_CONTENT_TYPES.contains(&normalized.as_str()) {
        bail!("Avatar content type is unsupported");
    }
    Ok(normalized)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalizes_supported_content_type_and_filename() {
        let avatar =
            Avatar::from_processed(vec![1], " IMAGE/PNG ", " face.png ").expect("processed avatar");

        assert_eq!(avatar.content_type, "image/png");
        assert_eq!(avatar.filename, "face.png");
    }

    #[test]
    fn rejects_empty_and_oversized_processed_content() {
        assert!(Avatar::from_processed(Vec::new(), "image/png", "face.png").is_err());
        assert!(
            Avatar::from_processed(vec![0; MAX_AVATAR_BYTES + 1], "image/png", "face.png").is_err()
        );
    }

    #[test]
    fn rejects_unsupported_content_types() {
        assert!(Avatar::from_processed(vec![1], "image/svg+xml", "face.svg").is_err());
    }

    #[test]
    fn debug_redacts_avatar_payload_and_filename() {
        let avatar = Avatar::from_processed(
            vec![222, 173, 190, 239],
            "image/png",
            "private-avatar-filename-marker.png",
        )
        .expect("valid avatar value");

        let rendered = format!("{avatar:?}");
        assert!(rendered.contains("content_len: 4"), "{rendered}");
        assert!(rendered.contains("image/png"), "{rendered}");
        assert!(rendered.contains("[REDACTED]"), "{rendered}");
        assert!(!rendered.contains("222, 173, 190, 239"), "{rendered}");
        assert!(
            !rendered.contains("private-avatar-filename-marker"),
            "{rendered}"
        );
    }
}
