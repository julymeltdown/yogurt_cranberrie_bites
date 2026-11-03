//! Validated C03/C06 place and immutable map-release values.
use super::MAX_REVISION;
use chrono::{DateTime, Utc};
use serde_json::{Map, Value};
use sha2::Sha256;
use std::{collections::HashSet, fmt, str::FromStr};
use url::Url;
use uuid::Uuid;

pub const MAP_CONTRACT_VERSION: &str = "1.0.0";
pub const MAX_TEXT_SCALARS: usize = 512;
pub const MAX_MAP_BYTES: usize = 1024 * 1024;
pub const MAX_PLACES: usize = 128;
pub const MAX_GRID_CELLS: usize = 65_536;

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlaceError {
    #[error("place identifier is not a canonical nonnil lowercase UUID")]
    Identifier,
    #[error("place text is invalid")]
    Text,
    #[error("place classification or provenance is invalid")]
    Classification,
    #[error("map release is invalid")]
    Manifest,
    #[error("map release content hash does not match")]
    ContentHash,
    #[error("map release geometry hash does not match")]
    GeometryHash,
    #[error("map release identity is already bound to different bytes")]
    ImmutableRelease,
}

macro_rules! string_enum {
    ($name:ident, $error:ident, $( $variant:ident = $value:literal ),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq)]
        pub enum $name { $( $variant ),+ }
        impl $name {
            pub fn as_str(self) -> &'static str { match self { $( Self::$variant => $value ),+ } }
        }
        impl FromStr for $name {
            type Err = PlaceError;
            fn from_str(value: &str) -> Result<Self, PlaceError> { match value { $( $value => Ok(Self::$variant), )+ _ => Err(PlaceError::$error) } }
        }
    };
}

string_enum!(
    PlaceKind,
    Classification,
    VirtualConcept = "VIRTUAL_CONCEPT",
    Physical = "PHYSICAL"
);
string_enum!(
    VerificationState,
    Classification,
    ConceptOnly = "CONCEPT_ONLY",
    Unverified = "UNVERIFIED",
    Verified = "VERIFIED"
);
string_enum!(
    ClosureState,
    Classification,
    Open = "OPEN",
    Closed = "CLOSED"
);
string_enum!(
    SourceKind,
    Classification,
    OriginalFixture = "ORIGINAL_FIXTURE",
    OriginalArt = "ORIGINAL_ART",
    PlaceReference = "PLACE_REFERENCE"
);
string_enum!(
    ReviewState,
    Classification,
    Unreviewed = "UNREVIEWED",
    Reviewed = "REVIEWED",
    Rejected = "REJECTED"
);
string_enum!(
    PhotoPermission,
    Classification,
    NotApplicable = "NOT_APPLICABLE",
    Unreviewed = "UNREVIEWED",
    Granted = "GRANTED",
    Denied = "DENIED"
);
string_enum!(
    ResourceState,
    Manifest,
    NotProvided = "NOT_PROVIDED",
    Provided = "PROVIDED"
);

impl ClosureState {
    pub fn permits_new_physical_selection(self) -> bool {
        self == Self::Open
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct PlaceId(Uuid);
impl PlaceId {
    pub fn from_uuid(value: Uuid) -> Result<Self, PlaceError> {
        if value.is_nil() {
            return Err(PlaceError::Identifier);
        }
        Ok(Self(value))
    }
    pub fn uuid(self) -> Uuid {
        self.0
    }
}
impl FromStr for PlaceId {
    type Err = PlaceError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let id = Uuid::parse_str(value).map_err(|_| PlaceError::Identifier)?;
        if id.to_string() != value {
            return Err(PlaceError::Identifier);
        }
        Self::from_uuid(id)
    }
}

fn text(value: &str) -> bool {
    !value.trim().is_empty()
        && value.chars().count() <= MAX_TEXT_SCALARS
        && !value.chars().any(char::is_control)
}
fn asset_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.as_bytes()[0].is_ascii_lowercase()
        && value
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}
fn uri(value: &str) -> bool {
    Url::parse(value)
        .is_ok_and(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())
}
fn hash_bytes(bytes: &[u8]) -> String {
    use sha2::Digest as _;
    format!(
        "sha256:{}",
        Sha256::digest(bytes)
            .iter()
            .map(|v| format!("{v:02x}"))
            .collect::<String>()
    )
}
fn digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|v| {
        v.len() == 64
            && v.bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    })
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Provenance {
    pub source_id: String,
    pub source_kind: SourceKind,
    pub source_revision: String,
    pub source_uri: Option<String>,
    pub author: String,
    pub license: String,
    pub review_state: ReviewState,
    pub reviewed_by: Option<String>,
    pub reviewed_at: Option<DateTime<Utc>>,
    pub photo_permission: PhotoPermission,
}
impl Provenance {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        source_id: String,
        source_kind: SourceKind,
        source_revision: String,
        source_uri: Option<String>,
        author: String,
        license: String,
        review_state: ReviewState,
        reviewed_by: Option<String>,
        reviewed_at: Option<DateTime<Utc>>,
        photo_permission: PhotoPermission,
    ) -> Result<Self, PlaceError> {
        if !asset_id(&source_id)
            || !text(&source_revision)
            || !text(&author)
            || !text(&license)
            || source_uri.as_deref().is_some_and(|v| !uri(v))
        {
            return Err(PlaceError::Classification);
        }
        match (review_state, reviewed_by.as_deref(), reviewed_at) {
            (ReviewState::Reviewed, Some(by), Some(_)) if text(by) => {}
            (ReviewState::Reviewed, _, _) => return Err(PlaceError::Classification),
            (_, None, None) => {}
            _ => return Err(PlaceError::Classification),
        }
        Ok(Self {
            source_id,
            source_kind,
            source_revision,
            source_uri,
            author,
            license,
            review_state,
            reviewed_by,
            reviewed_at,
            photo_permission,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Place {
    pub id: PlaceId,
    pub label: String,
    pub kind: PlaceKind,
    pub verification: VerificationState,
    pub closure: ClosureState,
    pub accessibility_notes: String,
    pub provenance: Provenance,
}
impl Place {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: PlaceId,
        label: String,
        kind: PlaceKind,
        verification: VerificationState,
        closure: ClosureState,
        accessibility_notes: String,
        provenance: Provenance,
    ) -> Result<Self, PlaceError> {
        if !text(&label) || !text(&accessibility_notes) {
            return Err(PlaceError::Text);
        }
        let value = Self {
            id,
            label,
            kind,
            verification,
            closure,
            accessibility_notes,
            provenance,
        };
        if !value.kind_and_provenance_valid() {
            return Err(PlaceError::Classification);
        }
        Ok(value)
    }
    pub fn kind_and_provenance_valid(&self) -> bool {
        match (self.kind, self.verification) {
            (PlaceKind::VirtualConcept, VerificationState::ConceptOnly) => true,
            (PlaceKind::Physical, VerificationState::Unverified) => {
                self.provenance.source_kind == SourceKind::PlaceReference
                    && self.provenance.source_uri.is_some()
            }
            (PlaceKind::Physical, VerificationState::Verified) => {
                self.provenance.source_kind == SourceKind::PlaceReference
                    && self.provenance.source_uri.is_some()
                    && self.provenance.review_state == ReviewState::Reviewed
            }
            _ => false,
        }
    }
    pub fn permits_new_physical_selection(&self) -> bool {
        self.kind == PlaceKind::Physical
            && self.verification == VerificationState::Verified
            && self.closure.permits_new_physical_selection()
            && self.kind_and_provenance_valid()
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Spawn {
    pub id: String,
    pub x: i32,
    pub y: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Interaction {
    pub id: String,
    pub place_id: PlaceId,
    pub x: i32,
    pub y: i32,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Tilemap {
    pub resource_state: ResourceState,
    pub path: Option<String>,
    pub sha256: Option<String>,
}
impl Tilemap {
    pub fn new(
        resource_state: ResourceState,
        path: Option<String>,
        sha256: Option<String>,
    ) -> Result<Self, PlaceError> {
        let valid = match resource_state {
            ResourceState::NotProvided => path.is_none() && sha256.is_none(),
            ResourceState::Provided => {
                path.as_deref().is_some_and(|v| {
                    let Some(stem) = v.strip_suffix(".json") else {
                        return false;
                    };
                    stem.bytes()
                        .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'/'))
                }) && sha256.as_deref().is_some_and(digest)
            }
        };
        if !valid {
            return Err(PlaceError::Manifest);
        }
        Ok(Self {
            resource_state,
            path,
            sha256,
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Geometry {
    pub width: i32,
    pub height: i32,
    pub tile_width: i32,
    pub tile_height: i32,
    pub collision_id: String,
    pub collision: Vec<Vec<u8>>,
    pub terrain: Vec<Vec<i32>>,
    pub overdraw: Vec<Vec<i32>>,
    pub spawns: Vec<Spawn>,
    pub interactions: Vec<Interaction>,
    pub default_spawn_id: String,
}
impl Geometry {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        width: i32,
        height: i32,
        tile_width: i32,
        tile_height: i32,
        collision_id: String,
        collision: Vec<Vec<u8>>,
        terrain: Vec<Vec<i32>>,
        overdraw: Vec<Vec<i32>>,
        spawns: Vec<Spawn>,
        interactions: Vec<Interaction>,
        default_spawn_id: String,
    ) -> Result<Self, PlaceError> {
        let cells = usize::try_from(width)
            .ok()
            .and_then(|w| usize::try_from(height).ok().and_then(|h| w.checked_mul(h)));
        if width < 1
            || height < 1
            || tile_width < 1
            || tile_height < 1
            || cells.is_none_or(|c| c > MAX_GRID_CELLS)
            || !asset_id(&collision_id)
            || !asset_id(&default_spawn_id)
            || spawns.is_empty()
            || interactions.is_empty()
            || !dimensions(&collision, width, height)
            || !dimensions(&terrain, width, height)
            || !dimensions(&overdraw, width, height)
            || collision.iter().flatten().any(|v| *v > 1)
            || terrain
                .iter()
                .flatten()
                .chain(overdraw.iter().flatten())
                .any(|v| *v < 0)
        {
            return Err(PlaceError::Manifest);
        }
        let mut spawn_ids = HashSet::new();
        for s in &spawns {
            if !asset_id(&s.id)
                || !spawn_ids.insert(&s.id)
                || !walkable(&collision, width, height, s.x, s.y)
            {
                return Err(PlaceError::Manifest);
            }
        }
        if !spawn_ids.contains(&default_spawn_id) {
            return Err(PlaceError::Manifest);
        }
        let mut interaction_ids = HashSet::new();
        for i in &interactions {
            if !asset_id(&i.id)
                || !interaction_ids.insert(&i.id)
                || !walkable(&collision, width, height, i.x, i.y)
            {
                return Err(PlaceError::Manifest);
            }
        }
        if spawns.iter().any(|spawn| {
            interactions
                .iter()
                .any(|interaction| !reachable(&collision, width, height, spawn, interaction))
        }) {
            return Err(PlaceError::Manifest);
        }
        Ok(Self {
            width,
            height,
            tile_width,
            tile_height,
            collision_id,
            collision,
            terrain,
            overdraw,
            spawns,
            interactions,
            default_spawn_id,
        })
    }
}
fn reachable(grid: &[Vec<u8>], w: i32, h: i32, spawn: &Spawn, target: &Interaction) -> bool {
    let width = w as usize;
    let height = h as usize;
    let mut seen = vec![false; width * height];
    let mut pending = vec![(spawn.x as usize, spawn.y as usize)];
    while let Some((x, y)) = pending.pop() {
        let index = y * width + x;
        if seen[index] || grid[y][x] != 0 {
            continue;
        }
        if x == target.x as usize && y == target.y as usize {
            return true;
        }
        seen[index] = true;
        if x > 0 {
            pending.push((x - 1, y));
        }
        if x + 1 < width {
            pending.push((x + 1, y));
        }
        if y > 0 {
            pending.push((x, y - 1));
        }
        if y + 1 < height {
            pending.push((x, y + 1));
        }
    }
    false
}
fn dimensions<T>(grid: &[Vec<T>], w: i32, h: i32) -> bool {
    grid.len() == usize::try_from(h).unwrap_or(0)
        && grid
            .iter()
            .all(|r| r.len() == usize::try_from(w).unwrap_or(0))
}
fn walkable(grid: &[Vec<u8>], w: i32, h: i32, x: i32, y: i32) -> bool {
    x >= 0 && y >= 0 && x < w && y < h && grid[y as usize][x as usize] == 0
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Manifest {
    pub map_id: String,
    pub map_revision: i64,
    pub content_hash: String,
    pub geometry_hash: String,
    pub provenance: Provenance,
    pub tilemap: Tilemap,
    pub geometry: Geometry,
    pub places: Vec<Place>,
}
impl Manifest {
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        map_id: String,
        map_revision: i64,
        content_hash: String,
        geometry_hash: String,
        provenance: Provenance,
        tilemap: Tilemap,
        geometry: Geometry,
        places: Vec<Place>,
    ) -> Result<Self, PlaceError> {
        if !asset_id(&map_id)
            || !(1..=MAX_REVISION).contains(&map_revision)
            || provenance.review_state == ReviewState::Rejected
            || !digest(&content_hash)
            || !digest(&geometry_hash)
            || places.is_empty()
            || places.len() > MAX_PLACES
        {
            return Err(PlaceError::Manifest);
        }
        let mut ids = HashSet::new();
        if places.iter().any(|p| !ids.insert(p.id)) {
            return Err(PlaceError::Manifest);
        }
        if geometry
            .interactions
            .iter()
            .any(|i| !ids.contains(&i.place_id))
        {
            return Err(PlaceError::Manifest);
        }
        Ok(Self {
            map_id,
            map_revision,
            content_hash,
            geometry_hash,
            provenance,
            tilemap,
            geometry,
            places,
        })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReviewStatus {
    Pending,
    Reviewed { current_closure: ClosureState },
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ReviewedPlace {
    pub place: Place,
    pub review: ReviewStatus,
}
impl ReviewedPlace {
    pub fn new(place: Place, review: ReviewStatus) -> Result<Self, PlaceError> {
        let accepted = match review {
            ReviewStatus::Pending => place.verification != VerificationState::Verified,
            ReviewStatus::Reviewed { current_closure } => {
                place.verification == VerificationState::Verified
                    && place.closure == current_closure
                    && place.provenance.review_state == ReviewState::Reviewed
            }
        };
        if !accepted {
            return Err(PlaceError::Classification);
        }
        Ok(Self { place, review })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogRelease {
    pub map_id: String,
    pub manifest: Manifest,
    pub canonical_bytes: Vec<u8>,
}
impl CatalogRelease {
    pub fn new(manifest: Manifest, canonical_bytes: Vec<u8>) -> Result<Self, PlaceError> {
        if canonical_bytes.is_empty() || canonical_bytes.len() > MAX_MAP_BYTES {
            return Err(PlaceError::Manifest);
        }
        let decoded = parse_stored_release(&canonical_bytes)?;
        if decoded.map_id != manifest.map_id
            || decoded.map_revision != manifest.map_revision
            || decoded.content_hash != manifest.content_hash
            || decoded.geometry_hash != manifest.geometry_hash
            || decoded.provenance != manifest.provenance
            || decoded.tilemap != manifest.tilemap
            || decoded.geometry != manifest.geometry
            || decoded.places != manifest.places
        {
            return Err(PlaceError::Manifest);
        }
        if manifest_digest(&canonical_bytes)? != manifest.content_hash {
            return Err(PlaceError::ContentHash);
        }
        if geometry_digest(&canonical_bytes)? != manifest.geometry_hash {
            return Err(PlaceError::GeometryHash);
        }
        Ok(Self {
            map_id: manifest.map_id.clone(),
            manifest,
            canonical_bytes,
        })
    }
    pub fn verify_immutable(&self, stored_bytes: &[u8]) -> Result<(), PlaceError> {
        if stored_bytes != self.canonical_bytes {
            Err(PlaceError::ImmutableRelease)
        } else if manifest_digest(stored_bytes)? != self.manifest.content_hash {
            Err(PlaceError::ContentHash)
        } else if geometry_digest(stored_bytes)? != self.manifest.geometry_hash {
            Err(PlaceError::GeometryHash)
        } else {
            Ok(())
        }
    }
    pub fn byte_digest(&self) -> String {
        hash_bytes(&self.canonical_bytes)
    }
}

fn canonicalize(value: &Value) -> Result<Vec<u8>, PlaceError> {
    let mut output = Vec::new();
    write_canonical(value, &mut output).map_err(|_| PlaceError::Manifest)?;
    Ok(output)
}
fn write_canonical(value: &Value, output: &mut Vec<u8>) -> Result<(), ()> {
    match value {
        Value::Object(object) => {
            output.push(b'{');
            let mut keys = object.keys().map(String::as_str).collect::<Vec<_>>();
            keys.sort_unstable();
            for (index, key) in keys.into_iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_canonical(&Value::String(key.into()), output)?;
                output.push(b':');
                write_canonical(object.get(key).ok_or(())?, output)?;
            }
            output.push(b'}');
        }
        Value::Array(values) => {
            output.push(b'[');
            for (index, value) in values.iter().enumerate() {
                if index > 0 {
                    output.push(b',');
                }
                write_canonical(value, output)?;
            }
            output.push(b']');
        }
        Value::String(text) => {
            output.push(b'"');
            for character in text.chars() {
                if let Some(literal) = match character {
                    '"' => Some("\\\""),
                    '\\' => Some("\\\\"),
                    '\u{08}' => Some("\\b"),
                    '\u{0c}' => Some("\\f"),
                    '\n' => Some("\\n"),
                    '\r' => Some("\\r"),
                    '\t' => Some("\\t"),
                    ' '..='~' => None,
                    _ => Some(""),
                } {
                    if literal.is_empty() {
                        let point = character as u32;
                        if point <= 0xffff {
                            output.extend_from_slice(format!("\\u{point:04x}").as_bytes());
                        } else {
                            let value = point - 0x1_0000;
                            let high = 0xd800 + (value >> 10);
                            let low = 0xdc00 + (value & 0x3ff);
                            output
                                .extend_from_slice(format!("\\u{high:04x}\\u{low:04x}").as_bytes());
                        }
                    } else {
                        output.extend_from_slice(literal.as_bytes());
                    }
                } else {
                    output.push(character as u8);
                }
            }
            output.push(b'"');
        }
        Value::Number(number) if number.is_i64() || number.is_u64() => {
            output.extend_from_slice(number.to_string().as_bytes());
        }
        Value::Number(_) => return Err(()),
        Value::Null => output.extend_from_slice(b"null"),
        Value::Bool(value) => output.extend_from_slice(if *value { b"true" } else { b"false" }),
    }
    Ok(())
}
fn manifest_digest(bytes: &[u8]) -> Result<String, PlaceError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| PlaceError::Manifest)?;
    let object = value.as_object().ok_or(PlaceError::Manifest)?;
    let mut without = object.clone();
    without.remove("content_hash");
    Ok(hash_bytes(&canonicalize(&Value::Object(without))?))
}
fn geometry_digest(bytes: &[u8]) -> Result<String, PlaceError> {
    let value: Value = serde_json::from_slice(bytes).map_err(|_| PlaceError::Manifest)?;
    let geometry = value.get("geometry").ok_or(PlaceError::Manifest)?;
    Ok(hash_bytes(&canonicalize(geometry)?))
}

/// Convert a noncanonical JSON document to exact C06 canonical bytes for
/// contract fixtures and consumer tests. Floats and non-JSON values fail closed.
pub fn canonical_json_bytes(source: &[u8]) -> Result<Vec<u8>, PlaceError> {
    let value: Value = serde_json::from_slice(source).map_err(|_| PlaceError::Manifest)?;
    canonicalize(&value)
}

/// Strictly decode the immutable stored release. The whole canonical document
/// must use C06 canonical bytes; only closed objects and typed values are accepted.
pub fn parse_stored_release(bytes: &[u8]) -> Result<Manifest, PlaceError> {
    if bytes.is_empty() || bytes.len() > MAX_MAP_BYTES {
        return Err(PlaceError::Manifest);
    }
    let root: Value = serde_json::from_slice(bytes).map_err(|_| PlaceError::Manifest)?;
    if canonicalize(&root)? != bytes {
        return Err(PlaceError::Manifest);
    }
    closed_object(
        &root,
        &[
            "contract_version",
            "kind",
            "map_id",
            "map_revision",
            "content_hash",
            "geometry_hash",
            "provenance",
            "tilemap",
            "geometry",
            "places",
        ],
    )?;
    if root.get("contract_version").and_then(|v| v.as_str()) != Some(MAP_CONTRACT_VERSION)
        || root.get("kind").and_then(|v| v.as_str()) != Some("map_manifest")
    {
        return Err(PlaceError::Manifest);
    }
    let map_id = root
        .get("map_id")
        .and_then(|v| v.as_str())
        .ok_or(PlaceError::Manifest)?
        .to_owned();
    let map_revision = root
        .get("map_revision")
        .and_then(|v| v.as_i64())
        .ok_or(PlaceError::Manifest)?;
    let content_hash = root
        .get("content_hash")
        .and_then(|v| v.as_str())
        .ok_or(PlaceError::Manifest)?
        .to_owned();
    let geometry_hash = root
        .get("geometry_hash")
        .and_then(|v| v.as_str())
        .ok_or(PlaceError::Manifest)?
        .to_owned();
    let provenance = parse_provenance(root.get("provenance").ok_or(PlaceError::Manifest)?)?;
    let tile = root.get("tilemap").ok_or(PlaceError::Manifest)?;
    closed_object(tile, &["format", "resource_state", "path", "sha256"])?;
    if tile.get("format").and_then(Value::as_str) != Some("TILED_JSON") {
        return Err(PlaceError::Manifest);
    }
    let tilemap = Tilemap::new(
        tile.get("resource_state")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .parse()
            .map_err(|_| PlaceError::Manifest)?,
        nullable_string(tile, "path")?,
        nullable_string(tile, "sha256")?,
    )?;
    let g = root.get("geometry").ok_or(PlaceError::Manifest)?;
    closed_object(
        g,
        &[
            "width_cells",
            "height_cells",
            "tile_width_px",
            "tile_height_px",
            "collision_id",
            "collision_grid",
            "terrain_layer",
            "overdraw_layer",
            "spawns",
            "interactions",
            "default_spawn_id",
        ],
    )?;
    let int = |name: &str| {
        g.get(name)
            .and_then(|v| v.as_i64())
            .and_then(|v| i32::try_from(v).ok())
            .ok_or(PlaceError::Manifest)
    };
    let grid = |name: &str| -> Result<Vec<Vec<i32>>, PlaceError> {
        g.get(name)
            .and_then(|v| v.as_array())
            .ok_or(PlaceError::Manifest)?
            .iter()
            .map(|row| {
                row.as_array()
                    .ok_or(PlaceError::Manifest)?
                    .iter()
                    .map(|v| {
                        v.as_i64()
                            .and_then(|v| i32::try_from(v).ok())
                            .ok_or(PlaceError::Manifest)
                    })
                    .collect()
            })
            .collect()
    };
    let spawns = g
        .get("spawns")
        .and_then(|v| v.as_array())
        .ok_or(PlaceError::Manifest)?
        .iter()
        .map(|v| {
            closed_object(v, &["spawn_id", "cell_x", "cell_y"])?;
            Ok(Spawn {
                id: v
                    .get("spawn_id")
                    .and_then(|v| v.as_str())
                    .ok_or(PlaceError::Manifest)?
                    .into(),
                x: v.get("cell_x")
                    .and_then(|v| v.as_i64())
                    .and_then(|v| i32::try_from(v).ok())
                    .ok_or(PlaceError::Manifest)?,
                y: v.get("cell_y")
                    .and_then(|v| v.as_i64())
                    .and_then(|v| i32::try_from(v).ok())
                    .ok_or(PlaceError::Manifest)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let interactions = g
        .get("interactions")
        .and_then(|v| v.as_array())
        .ok_or(PlaceError::Manifest)?
        .iter()
        .map(|v| {
            closed_object(v, &["interaction_id", "place_id", "cell_x", "cell_y"])?;
            Ok(Interaction {
                id: v
                    .get("interaction_id")
                    .and_then(|v| v.as_str())
                    .ok_or(PlaceError::Manifest)?
                    .into(),
                place_id: v
                    .get("place_id")
                    .and_then(|v| v.as_str())
                    .ok_or(PlaceError::Manifest)?
                    .parse()
                    .map_err(|_| PlaceError::Manifest)?,
                x: v.get("cell_x")
                    .and_then(|v| v.as_i64())
                    .and_then(|v| i32::try_from(v).ok())
                    .ok_or(PlaceError::Manifest)?,
                y: v.get("cell_y")
                    .and_then(|v| v.as_i64())
                    .and_then(|v| i32::try_from(v).ok())
                    .ok_or(PlaceError::Manifest)?,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    let collision = grid("collision_grid")?
        .into_iter()
        .map(|r| {
            r.into_iter()
                .map(|v| u8::try_from(v).map_err(|_| PlaceError::Manifest))
                .collect()
        })
        .collect::<Result<Vec<Vec<u8>>, _>>()?;
    let geometry = Geometry::new(
        int("width_cells")?,
        int("height_cells")?,
        int("tile_width_px")?,
        int("tile_height_px")?,
        g.get("collision_id")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .into(),
        collision,
        grid("terrain_layer")?,
        grid("overdraw_layer")?,
        spawns,
        interactions,
        g.get("default_spawn_id")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .into(),
    )?;
    let places = root
        .get("places")
        .and_then(|v| v.as_array())
        .ok_or(PlaceError::Manifest)?
        .iter()
        .map(parse_place)
        .collect::<Result<Vec<_>, _>>()?;
    Manifest::new(
        map_id,
        map_revision,
        content_hash,
        geometry_hash,
        provenance,
        tilemap,
        geometry,
        places,
    )
}
fn closed_object<'a>(
    value: &'a Value,
    fields: &[&str],
) -> Result<&'a Map<String, Value>, PlaceError> {
    let object = value.as_object().ok_or(PlaceError::Manifest)?;
    if object.len() != fields.len()
        || object.keys().any(|key| !fields.contains(&key.as_str()))
        || fields.iter().any(|field| !object.contains_key(*field))
    {
        return Err(PlaceError::Manifest);
    }
    Ok(object)
}
fn nullable_string(value: &Value, field: &str) -> Result<Option<String>, PlaceError> {
    match value.get(field) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(text)) => Ok(Some(text.clone())),
        _ => Err(PlaceError::Manifest),
    }
}
fn parse_provenance(v: &Value) -> Result<Provenance, PlaceError> {
    closed_object(
        v,
        &[
            "source_id",
            "source_kind",
            "source_revision",
            "source_uri",
            "author",
            "license",
            "review_state",
            "reviewed_by",
            "reviewed_at",
            "photo_permission",
        ],
    )?;
    let at = v
        .get("reviewed_at")
        .and_then(|v| v.as_str())
        .map(|s| s.parse().map_err(|_| PlaceError::Manifest))
        .transpose()?;
    Provenance::new(
        v.get("source_id")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .into(),
        v.get("source_kind")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .parse()
            .map_err(|_| PlaceError::Manifest)?,
        v.get("source_revision")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .into(),
        nullable_string(v, "source_uri")?,
        v.get("author")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .into(),
        v.get("license")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .into(),
        v.get("review_state")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .parse()
            .map_err(|_| PlaceError::Manifest)?,
        nullable_string(v, "reviewed_by")?,
        at,
        v.get("photo_permission")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .parse()
            .map_err(|_| PlaceError::Manifest)?,
    )
    .map_err(|_| PlaceError::Manifest)
}
fn parse_place(v: &Value) -> Result<Place, PlaceError> {
    closed_object(
        v,
        &[
            "place_id",
            "label",
            "place_kind",
            "verification_state",
            "closure_state",
            "accessibility_notes",
            "provenance",
        ],
    )?;
    Place::new(
        v.get("place_id")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .parse()
            .map_err(|_| PlaceError::Manifest)?,
        v.get("label")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .into(),
        v.get("place_kind")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .parse()
            .map_err(|_| PlaceError::Manifest)?,
        v.get("verification_state")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .parse()
            .map_err(|_| PlaceError::Manifest)?,
        v.get("closure_state")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .parse()
            .map_err(|_| PlaceError::Manifest)?,
        v.get("accessibility_notes")
            .and_then(|v| v.as_str())
            .ok_or(PlaceError::Manifest)?
            .into(),
        parse_provenance(v.get("provenance").ok_or(PlaceError::Manifest)?)?,
    )
    .map_err(|_| PlaceError::Manifest)
}

impl fmt::Display for SourceKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}
