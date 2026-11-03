use crate::adapters::out::errors::media_error;
use crate::application::dto::profile_dto::AvatarDto;
use crate::application::errors::{ProfileError, ProfileResult};
use crate::application::ports::AvatarProcessorPort;
use crate::domain::avatar::{
    normalize_avatar_content_type, Avatar, MAX_AVATAR_BYTES, MAX_AVATAR_PIXELS,
};
use anyhow::{bail, Context, Result};
use async_trait::async_trait;
use image::ImageFormat;
use platform::cpu::{AdmissionClass, AdmissionStats};
use std::io::{self, Cursor, Seek, SeekFrom, Write};

/// Image decoding and canonical re-encoding behind the application-owned port.
/// CPU work is admitted to a bounded worker class rather than Tokio's async
/// workers. Re-encoded output is bounded while writing to avoid a second
/// unbounded allocation after the decoded bitmap has been validated.
#[derive(Clone)]
pub struct ImageAvatarProcessor {
    admission: AdmissionClass,
}

impl ImageAvatarProcessor {
    pub fn new(admission: AdmissionClass) -> Self {
        Self { admission }
    }

    pub fn admission_stats(&self) -> AdmissionStats {
        self.admission.stats()
    }
}

#[async_trait]
impl AvatarProcessorPort for ImageAvatarProcessor {
    async fn process(
        &self,
        content: Vec<u8>,
        content_type: &str,
        filename: &str,
    ) -> ProfileResult<AvatarDto> {
        if content.is_empty() {
            return Err(ProfileError::InvalidInput("Avatar content is empty"));
        }
        if content.len() > MAX_AVATAR_BYTES {
            return Err(ProfileError::AvatarContentTooLarge {
                received: content.len(),
            });
        }

        let content_type = normalize_avatar_content_type(content_type).map_err(media_error)?;
        let filename = filename.to_owned();
        self.admission
            .run(move || {
                let format = declared_format(&content_type)?;
                let encoded = decode_and_reencode(&content, &content_type, format)?;
                let canonical_type = content_type_for_format(format)?;
                Avatar::from_processed(encoded, canonical_type, filename).map(Into::into)
            })
            .await
            .map_err(media_error)
    }
}

fn decode_and_reencode(
    content: &[u8],
    declared_content_type: &str,
    format: ImageFormat,
) -> Result<Vec<u8>> {
    ensure_content_matches_type(content, declared_content_type)?;

    let detected = image::guess_format(content)
        .map_err(|_| anyhow::anyhow!("Avatar is not a decodable image"))?;
    anyhow::ensure!(
        detected == format,
        ProfileError::InvalidInput("Avatar content does not match its declared image type")
    );

    // Check header dimensions before allocating a decoded bitmap. An encoded
    // byte cap by itself does not bound the memory required by decompression.
    let dimensions = image::ImageReader::with_format(Cursor::new(content), format)
        .into_dimensions()
        .context("Avatar dimensions cannot be read")?;
    anyhow::ensure!(
        dimensions.0 > 0 && dimensions.1 > 0,
        "Avatar has no usable dimensions"
    );
    let pixels = u64::from(dimensions.0) * u64::from(dimensions.1);
    anyhow::ensure!(
        pixels <= MAX_AVATAR_PIXELS,
        ProfileError::AvatarPixelsExceeded {
            width: dimensions.0,
            height: dimensions.1
        }
    );

    let decoded = image::load_from_memory_with_format(content, format)
        .context("Avatar could not be decoded as the image type it declares")?;
    let mut encoded = BoundedOutput::new(MAX_AVATAR_BYTES);
    decoded
        .write_to(&mut encoded, format)
        .context("Avatar could not be re-encoded within the configured size limit")?;
    let reencoded = encoded.into_inner();
    anyhow::ensure!(!reencoded.is_empty(), "Avatar re-encoding produced no data");
    Ok(reencoded)
}

fn declared_format(content_type: &str) -> Result<ImageFormat> {
    Ok(match content_type {
        "image/png" => ImageFormat::Png,
        "image/jpeg" => ImageFormat::Jpeg,
        "image/gif" => ImageFormat::Gif,
        "image/webp" => ImageFormat::WebP,
        other => bail!("Avatar content type is unsupported: {other}"),
    })
}

fn content_type_for_format(format: ImageFormat) -> Result<&'static str> {
    match format {
        ImageFormat::Png => Ok("image/png"),
        ImageFormat::Jpeg => Ok("image/jpeg"),
        ImageFormat::Gif => Ok("image/gif"),
        ImageFormat::WebP => Ok("image/webp"),
        _ => bail!("Avatar format is unsupported"),
    }
}

fn ensure_content_matches_type(content: &[u8], content_type: &str) -> Result<()> {
    let matches = match content_type {
        "image/png" => content.starts_with(&[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]),
        "image/jpeg" => content.starts_with(&[0xff, 0xd8, 0xff]),
        "image/gif" => content.starts_with(b"GIF87a") || content.starts_with(b"GIF89a"),
        "image/webp" => {
            content.len() >= 12 && content.starts_with(b"RIFF") && &content[8..12] == b"WEBP"
        }
        _ => false,
    };
    if !matches {
        bail!("Avatar content does not match its declared image type");
    }
    Ok(())
}

/// Seekable encoder target that rejects growth before the backing Vec expands
/// beyond the avatar limit.
struct BoundedOutput {
    cursor: Cursor<Vec<u8>>,
    max_bytes: usize,
}

impl BoundedOutput {
    fn new(max_bytes: usize) -> Self {
        Self {
            cursor: Cursor::new(Vec::with_capacity(max_bytes.min(8192))),
            max_bytes,
        }
    }

    fn into_inner(self) -> Vec<u8> {
        self.cursor.into_inner()
    }

    fn exceeds_limit(&self) -> io::Error {
        io::Error::other(format!(
            "Avatar encoded output exceeds the {} byte limit",
            self.max_bytes
        ))
    }
}

impl Write for BoundedOutput {
    fn write(&mut self, buffer: &[u8]) -> io::Result<usize> {
        if buffer.is_empty() {
            return Ok(0);
        }
        let start = usize::try_from(self.cursor.position()).map_err(|_| self.exceeds_limit())?;
        let end = self
            .cursor
            .position()
            .checked_add(u64::try_from(buffer.len()).map_err(|_| self.exceeds_limit())?)
            .ok_or_else(|| self.exceeds_limit())?;
        if end > self.max_bytes as u64 {
            return Err(self.exceeds_limit());
        }
        let end = usize::try_from(end).map_err(|_| self.exceeds_limit())?;
        let output = self.cursor.get_mut();
        if end > output.capacity() {
            let target_capacity = end
                .max(output.capacity().saturating_mul(2))
                .min(self.max_bytes);
            output
                .try_reserve_exact(target_capacity.saturating_sub(output.len()))
                .map_err(io::Error::other)?;
        }
        if end > output.len() {
            output.resize(end, 0);
        }
        output[start..end].copy_from_slice(buffer);
        self.cursor.set_position(end as u64);
        Ok(buffer.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        self.cursor.flush()
    }
}

impl Seek for BoundedOutput {
    fn seek(&mut self, position: SeekFrom) -> io::Result<u64> {
        let current = i128::from(self.cursor.position());
        let end = self.cursor.get_ref().len() as i128;
        let target = match position {
            SeekFrom::Start(offset) => i128::from(offset),
            SeekFrom::Current(offset) => current + i128::from(offset),
            SeekFrom::End(offset) => end + i128::from(offset),
        };
        if target < 0 || target > self.max_bytes as i128 {
            return Err(self.exceeds_limit());
        }
        self.cursor.seek(position)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use image::codecs::jpeg::JpegEncoder;
    use image::{ImageEncoder, Rgb, RgbImage};

    #[test]
    fn bounded_output_refuses_growth_before_allocating_past_limit() {
        let mut output = BoundedOutput::new(4);
        output.write_all(b"1234").expect("write at limit");

        let error = output.write_all(b"5").expect_err("write beyond limit");

        assert!(error.to_string().contains("exceeds the 4 byte limit"));
        assert_eq!(output.cursor.get_ref(), b"1234");
    }

    #[test]
    fn rejects_an_encoded_image_that_expands_past_the_storage_limit() {
        let source = noisy_low_quality_jpeg();
        assert!(
            source.len() <= MAX_AVATAR_BYTES,
            "input is {} bytes",
            source.len()
        );

        let outcome = decode_and_reencode(&source, "image/jpeg", ImageFormat::Jpeg)
            .map(|encoded| encoded.len());
        let error =
            outcome.expect_err("canonical output must be refused when it exceeds the bound");

        assert!(
            format!("{error:#}").contains("exceeds the 1048576 byte limit"),
            "unexpected error: {error:#}"
        );
    }

    fn noisy_low_quality_jpeg() -> Vec<u8> {
        let mut state = 0x8a5c_91e3_u32;
        let mut image = RgbImage::new(3300, 3300);
        for pixel in image.pixels_mut() {
            state ^= state << 13;
            state ^= state >> 17;
            state ^= state << 5;
            *pixel = Rgb([state as u8, (state >> 8) as u8, (state >> 16) as u8]);
        }

        let mut output = Vec::new();
        JpegEncoder::new_with_quality(&mut output, 1)
            .write_image(
                image.as_raw(),
                image.width(),
                image.height(),
                image::ExtendedColorType::Rgb8,
            )
            .expect("encode low-quality fixture");
        output
    }
}
