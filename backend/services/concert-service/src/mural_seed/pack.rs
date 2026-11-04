use super::SeedError;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    io::{Cursor, Read},
    path::{Component, Path},
};

const REFERENCE: &str = "d9a8fae2f96a82253542019ab673589b3938627956eace15c8594c702b5c4617";
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FilePin {
    path: String,
    sha256: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Files {
    source: FilePin,
    pixels: FilePin,
    palette: FilePin,
    preview: FilePin,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Provenance {
    origin: String,
    reference_path: String,
    reference_sha256: String,
    rights_statement: String,
    qualification_evidence: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Manifest {
    format: String,
    asset_id: String,
    asset_revision: u32,
    event_binding: String,
    width: u32,
    height: u32,
    layout: String,
    background_index: u8,
    files: Files,
    provenance: Provenance,
}
/// Frozen actual bytes from one selected source revision, not browser arrays.
/// Evidence bytes must be those at the manifest's exact relative path.
pub struct PackBytes<'a> {
    pub manifest: &'a [u8],
    pub source: &'a [u8],
    pub pixels: &'a [u8],
    pub palette: &'a [u8],
    pub preview: &'a [u8],
    pub qualification_path: &'a str,
    pub qualification: &'a [u8],
}
/// Only constructed after complete manifest/file/pixel/PNG validation. Owning
/// operator must separately select the real qualified revision and authority.
pub struct ValidatedPack {
    pub(super) manifest_sha256: [u8; 32],
    pub(super) qualification_sha256: [u8; 32],
    pub(super) pixels: Vec<u8>,
    pub(super) palette: Vec<String>,
}
impl ValidatedPack {
    pub fn manifest_sha256(&self) -> [u8; 32] {
        self.manifest_sha256
    }
    pub fn pixels(&self) -> &[u8] {
        &self.pixels
    }
    pub fn palette(&self) -> &[String] {
        &self.palette
    }
}
fn bad(reason: &'static str) -> SeedError {
    SeedError::InvalidPack(reason)
}
fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}
fn pin(pin: &FilePin, path: &str, bytes: &[u8]) -> Result<(), SeedError> {
    if pin.path != path || pin.sha256 != hex(&Sha256::digest(bytes)) {
        return Err(bad("File path or exact SHA-256 mismatch"));
    }
    Ok(())
}
fn relative(path: &str) -> bool {
    !path.is_empty()
        && path.len() <= 512
        && !path.contains('\\')
        && Path::new(path)
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
}
pub fn validate_pack(
    bytes: PackBytes<'_>,
    selected_manifest_sha256: [u8; 32],
) -> Result<ValidatedPack, SeedError> {
    if bytes.manifest.is_empty()
        || bytes.manifest.len() > 65536
        || bytes.source.is_empty()
        || bytes.source.len() > 4 * 1024 * 1024
        || bytes.pixels.len() != 16384
        || bytes.palette.len() > 4096
        || bytes.preview.len() > 262144
        || bytes.qualification.is_empty()
        || bytes.qualification.len() > 262144
    {
        return Err(bad("Pack byte bounds"));
    }
    let manifest_sha256: [u8; 32] = Sha256::digest(bytes.manifest).into();
    if manifest_sha256 != selected_manifest_sha256 {
        return Err(bad("Selected manifest mismatch"));
    }
    let m: Manifest =
        serde_json::from_slice(bytes.manifest).map_err(|_| bad("Closed manifest required"))?;
    if m.format != "mcr-mural-initial-seed-art/1"
        || m.asset_id != "wall-initial-motif-art-01"
        || m.asset_revision != 1
        || m.event_binding != "OPERATOR_BOUND_AT_PROVISION"
        || m.width != 128
        || m.height != 128
        || m.layout != "ROW_MAJOR_TOP_LEFT_U8"
        || m.background_index != 0
        || m.provenance.origin != "ORIGINAL_MCR_ARTWORK"
        || m.provenance.reference_path != "docs/mcr/design/reference-target-2026-11-04.png"
        || m.provenance.reference_sha256 != REFERENCE
        || m.provenance.rights_statement.trim().is_empty()
        || !relative(&m.provenance.qualification_evidence)
        || bytes.qualification_path != m.provenance.qualification_evidence
    {
        return Err(bad("Frozen art contract or provenance mismatch"));
    }
    pin(&m.files.source, "source.svg", bytes.source)?;
    pin(&m.files.pixels, "pixels.bin", bytes.pixels)?;
    pin(&m.files.palette, "palette.json", bytes.palette)?;
    pin(&m.files.preview, "preview.png", bytes.preview)?;
    // Source is never executed or rendered by the server. Its complete bytes
    // are pinned; the art owner's SVG/visual qualification remains separate.
    std::str::from_utf8(bytes.source).map_err(|_| bad("UTF-8 editable source required"))?;
    std::str::from_utf8(bytes.qualification)
        .map_err(|_| bad("UTF-8 qualification evidence required"))?;
    let palette: Vec<String> =
        serde_json::from_slice(bytes.palette).map_err(|_| bad("Palette JSON"))?;
    if palette.len() != 16
        || palette.iter().any(|p| {
            p.len() != 7
                || !p.starts_with('#')
                || !p.as_bytes()[1..]
                    .iter()
                    .all(|b| b.is_ascii_digit() || (b'A'..=b'F').contains(b))
        })
        || bytes.pixels.iter().any(|p| *p > 15)
    {
        return Err(bad("Palette or pixel index invalid"));
    }
    let mut reader =
        image::ImageReader::with_format(Cursor::new(bytes.preview), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(128);
    limits.max_image_height = Some(128);
    limits.max_alloc = Some(4 * 1024 * 1024);
    reader.limits(limits);
    let image = reader
        .decode()
        .map_err(|_| bad("Native bounded PNG required"))?;
    if image.width() != 128 || image.height() != 128 {
        return Err(bad("Native PNG dimensions"));
    }
    let colors: Vec<[u8; 3]> = palette
        .iter()
        .map(|p| {
            [
                u8::from_str_radix(&p[1..3], 16).unwrap(),
                u8::from_str_radix(&p[3..5], 16).unwrap(),
                u8::from_str_radix(&p[5..7], 16).unwrap(),
            ]
        })
        .collect();
    for (index, pixel) in image.to_rgba8().pixels().enumerate() {
        if pixel.0[3] != 255 || pixel.0[..3] != colors[bytes.pixels[index] as usize] {
            return Err(bad(
                "PNG differs from opaque row-major palette reconstruction",
            ));
        }
    }
    Ok(ValidatedPack {
        manifest_sha256,
        qualification_sha256: Sha256::digest(bytes.qualification).into(),
        pixels: bytes.pixels.to_vec(),
        palette,
    })
}
fn read(root: &Path, relative_path: &str, cap: usize) -> Result<Vec<u8>, SeedError> {
    if !relative(relative_path) {
        return Err(bad("Unsafe pack-relative path"));
    }
    let mut path = root.to_path_buf();
    for component in Path::new(relative_path).components() {
        path.push(component);
        if std::fs::symlink_metadata(&path)?.file_type().is_symlink() {
            return Err(bad("Symlink in pack path"));
        }
    }
    if !std::fs::metadata(&path)?.is_file() {
        return Err(bad("Regular pack file required"));
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)?
        .take((cap + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > cap {
        return Err(bad("Pack file exceeds byte bound"));
    }
    Ok(bytes)
}
/// Local filesystem convenience. Freezes bounded bytes before validation; no
/// database operation occurs until all selected digests and the PNG agree.
pub fn load_pack(
    root: &Path,
    selected_manifest_sha256: [u8; 32],
) -> Result<ValidatedPack, SeedError> {
    let root = root.canonicalize()?;
    let manifest = read(&root, "manifest.json", 65536)?;
    let m: Manifest =
        serde_json::from_slice(&manifest).map_err(|_| bad("Closed manifest required"))?;
    let source = read(&root, "source.svg", 4 * 1024 * 1024)?;
    let pixels = read(&root, "pixels.bin", 16384)?;
    let palette = read(&root, "palette.json", 4096)?;
    let preview = read(&root, "preview.png", 262144)?;
    let qualification = read(&root, &m.provenance.qualification_evidence, 262144)?;
    validate_pack(
        PackBytes {
            manifest: &manifest,
            source: &source,
            pixels: &pixels,
            palette: &palette,
            preview: &preview,
            qualification_path: &m.provenance.qualification_evidence,
            qualification: &qualification,
        },
        selected_manifest_sha256,
    )
}
