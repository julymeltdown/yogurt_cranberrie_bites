//! Newly authored codec. Retained Profile processor informed the bounded-output design;
//! unlike that processor, this pipeline rejects animation and applies decoder limits.
use concert_application::{commands::CommandError, media::*};
use image::{ImageDecoder, ImageFormat, Limits};
use platform::cpu::{AdmissionClass, CapacityExhausted};
use std::io::{self, Cursor, Seek, SeekFrom, Write};

#[derive(Clone)]
pub struct BoundedImageSanitizer {
    admission: AdmissionClass,
}
impl BoundedImageSanitizer {
    pub fn new(admission: AdmissionClass) -> Self {
        Self { admission }
    }
}
#[async_trait::async_trait]
impl ImageSanitizer for BoundedImageSanitizer {
    async fn sanitize(&self, bytes: Vec<u8>, mime: Mime) -> Result<Sanitized, CommandError> {
        if bytes.is_empty() || bytes.len() > MAX_BYTES {
            return Err(CommandError::InvalidInput);
        }
        // Preserve the typed safe decode error; capacity/outage never masquerades as bad media.
        self.admission
            .run(move || Ok(decode(&bytes, mime)))
            .await
            .map_err(|e| {
                if e.is::<CapacityExhausted>() {
                    CommandError::Capacity
                } else {
                    CommandError::Unavailable
                }
            })?
    }
}
fn decode(bytes: &[u8], mime: Mime) -> Result<Sanitized, CommandError> {
    let bad = |_| CommandError::InvalidInput;
    let format = match mime {
        Mime::Png => ImageFormat::Png,
        Mime::Jpeg => ImageFormat::Jpeg,
        Mime::Webp => ImageFormat::WebP,
    };
    if image::guess_format(bytes).map_err(bad)? != format {
        return Err(CommandError::InvalidInput);
    }
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_DIMENSION);
    limits.max_image_height = Some(MAX_DIMENSION);
    limits.max_alloc = Some(128 * 1024 * 1024);
    let mut decoder: Box<dyn ImageDecoder> = match mime {
        Mime::Png => {
            let d = image::codecs::png::PngDecoder::with_limits(Cursor::new(bytes), limits.clone())
                .map_err(bad)?;
            if d.is_apng().map_err(bad)? {
                return Err(CommandError::InvalidInput);
            }
            Box::new(d)
        }
        Mime::Webp => {
            let d = image::codecs::webp::WebPDecoder::new(Cursor::new(bytes)).map_err(bad)?;
            if d.has_animation() {
                return Err(CommandError::InvalidInput);
            }
            Box::new(d)
        }
        Mime::Jpeg => {
            Box::new(image::codecs::jpeg::JpegDecoder::new(Cursor::new(bytes)).map_err(bad)?)
        }
    };
    let (width, height) = decoder.dimensions();
    if width == 0
        || height == 0
        || width > MAX_DIMENSION
        || height > MAX_DIMENSION
        || u64::from(width) * u64::from(height) > MAX_PIXELS
        || decoder.total_bytes() > 128 * 1024 * 1024
    {
        return Err(CommandError::InvalidInput);
    }
    decoder.set_limits(limits).map_err(bad)?;
    let decoded = image::DynamicImage::from_decoder(decoder).map_err(bad)?;
    let mut output = BoundedOutput(Cursor::new(Vec::new()));
    // Fresh encoder receives pixels only. No EXIF, ICC, XMP, text, filenames or original chunks.
    decoded.write_to(&mut output, format).map_err(bad)?;
    let bytes = output.0.into_inner();
    if bytes.is_empty() {
        return Err(CommandError::InvalidInput);
    }
    Ok(Sanitized {
        bytes,
        mime,
        width,
        height,
    })
}
struct BoundedOutput(Cursor<Vec<u8>>);
impl Write for BoundedOutput {
    fn write(&mut self, buf: &[u8]) -> io::Result<usize> {
        if self
            .0
            .position()
            .checked_add(buf.len() as u64)
            .is_none_or(|end| end > MAX_BYTES as u64)
        {
            return Err(io::Error::other("image output limit"));
        }
        let end = self.0.position() as usize + buf.len();
        if end > self.0.get_ref().capacity() {
            let growth = end
                .max(self.0.get_ref().capacity().saturating_mul(2))
                .min(MAX_BYTES);
            let additional = growth.saturating_sub(self.0.get_ref().len());
            self.0
                .get_mut()
                .try_reserve_exact(additional)
                .map_err(io::Error::other)?;
        }
        self.0.write(buf)
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}
impl Seek for BoundedOutput {
    fn seek(&mut self, pos: SeekFrom) -> io::Result<u64> {
        let p = match pos {
            SeekFrom::Start(v) => v as i128,
            SeekFrom::Current(v) => self.0.position() as i128 + v as i128,
            SeekFrom::End(v) => self.0.get_ref().len() as i128 + v as i128,
        };
        if p < 0 || p > MAX_BYTES as i128 {
            return Err(io::Error::other("image output limit"));
        }
        self.0.seek(pos)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn output_limit_rejects_before_growth_or_seek() {
        let mut out = BoundedOutput(Cursor::new(Vec::new()));
        assert!(out.seek(SeekFrom::Start(MAX_BYTES as u64 + 1)).is_err());
        assert_eq!(out.0.get_ref().capacity(), 0);
        out.seek(SeekFrom::Start(MAX_BYTES as u64)).unwrap();
        assert!(out.write_all(b"x").is_err());
        assert_eq!(out.0.get_ref().capacity(), 0);
    }
}
