//! WP-07 T-601: the personal room layout value and its structural validation.
//!
//! A room belongs to a profile. It is subordinate to the profile and completely
//! independent of every mutuals-intro post, so nothing in this module can name an
//! intro post, a concert event or a participant. The owner is supplied by the
//! authenticated session; the wire shape is exactly `social.RoomLayout`
//! (domain-contract.md §5, http-contract.md §3.7–§3.9).
//!
//! The server validates the layout **structurally and by bounds**. It does not
//! judge aesthetics: any admitted wall, floor, furniture item and placement is
//! accepted as long as it is inside the fixed grid, inside the closed style and
//! item sets, and does not collide within a layer.
//!
//! The grid is the frozen contract's grid, not a product choice:
//!
//! * a cell coordinate is `0..=31` on both axes (`Domain_social_FurniturePlacement`,
//!   `Domain_social_CharacterPlacement`, `Domain_social_PosterPlacement`);
//! * a furniture layer is `0..=3`;
//! * a poster slot is `0..=3`, and a poster spans `1..=32` cells on each axis;
//! * at most 4 posters and at most 32 furniture items.
//!
//! The style and item identifiers are closed sets. An unknown identifier is
//! rejected rather than stored: a stored layout the renderer cannot draw is a
//! room a visitor sees half-empty, and the contract's "known style IDs" rule
//! exists to make that impossible.
use crate::{AssetId, Revision};
use serde::Deserialize;
use serde_json::{json, Value};
use std::collections::BTreeSet;
use std::fmt;
use std::str::FromStr;
use uuid::Uuid;

/// The grid is fixed: 32 cells on each axis, indexed from zero.
pub const GRID_CELLS: u16 = 32;
/// At most four posters stand in a room.
pub const MAX_POSTERS: usize = 4;
/// At most 32 furniture items stand in a room.
pub const MAX_FURNITURE: usize = 32;
/// A furniture layer is one of four draw bands.
pub const MAX_LAYER: u8 = 3;
/// A poster slot is one of four wall slots.
pub const MAX_POSTER_SLOT: u8 = 3;
/// A poster spans at most the whole grid width/height.
pub const MAX_POSTER_SPAN: u16 = 32;

/// The largest accepted save/visibility/poster request body.
///
/// A poster body is base64 of at most 1 MiB, which is 1_398_104 characters, plus
/// the closed envelope. The bound is generous enough for the frozen maximum and
/// small enough that a caller cannot make the service buffer an arbitrary body.
pub const MAX_REQUEST_BYTES: usize = 1_500_000;

/// A poster image is a PNG of at most 128 px per side and at most 1 MiB.
pub const MAX_POSTER_SIDE: u32 = 128;
pub const MAX_POSTER_BYTES: usize = 1024 * 1024;

/// The closed wall styles. One fixed room with a small set of wall styles.
pub const WALL_STYLES: [&str; 4] = ["plaster", "brick", "wallpaper", "panel"];
/// The closed floor styles.
pub const FLOOR_STYLES: [&str; 4] = ["wood", "tile", "rug", "concrete"];
/// The closed default furniture set. Small by product design (product-plan.md §6).
pub const FURNITURE_ITEMS: [&str; 8] = [
    "bed",
    "chair",
    "desk",
    "lamp",
    "plant",
    "shelf",
    "sofa",
    "table",
];

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum RoomError {
    #[error("room layout input is invalid")]
    Invalid,
    #[error("room layout was written by a newer version")]
    UnsupportedVersion,
}

/// The canonical internal profile identifier (domain-contract.md §1.1).
///
/// Re-exported from the profile avatar module so a room and an avatar are keyed
/// by exactly the same owner type; there is one definition, not two.
pub use crate::profile_avatar::ProfileId;

/// The room's visibility. A private room is `404` for every visitor; the profile
/// still reports `room_available: false`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum RoomVisibility {
    Public,
    Private,
}

impl RoomVisibility {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Public => "PUBLIC",
            Self::Private => "PRIVATE",
        }
    }

    /// The default a profile that has never chosen starts from.
    ///
    /// http-contract.md §2.4 declares `public_read` for `getProfileRoom` and
    /// product-plan.md §6 says authorized visitors see the saved room, so the
    /// product default is `PUBLIC`. The `profile_rooms` column default (`PRIVATE`,
    /// WP-02) is only the storage default for a row inserted without a choice;
    /// every write in this package states the visibility explicitly.
    pub fn default_for_new_room() -> Self {
        Self::Public
    }
}

impl FromStr for RoomVisibility {
    type Err = RoomError;
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "PUBLIC" => Ok(Self::Public),
            "PRIVATE" => Ok(Self::Private),
            _ => Err(RoomError::Invalid),
        }
    }
}

impl fmt::Display for RoomVisibility {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// The closed wall style identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct WallStyleId(&'static str);

impl WallStyleId {
    pub fn parse(value: &str) -> Result<Self, RoomError> {
        WALL_STYLES
            .iter()
            .copied()
            .find(|style| *style == value)
            .map(Self)
            .ok_or(RoomError::Invalid)
    }
    pub fn as_str(self) -> &'static str {
        self.0
    }
}

/// The closed floor style identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FloorStyleId(&'static str);

impl FloorStyleId {
    pub fn parse(value: &str) -> Result<Self, RoomError> {
        FLOOR_STYLES
            .iter()
            .copied()
            .find(|style| *style == value)
            .map(Self)
            .ok_or(RoomError::Invalid)
    }
    pub fn as_str(self) -> &'static str {
        self.0
    }
}

/// The closed furniture item identifier.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FurnitureId(&'static str);

impl FurnitureId {
    pub fn parse(value: &str) -> Result<Self, RoomError> {
        FURNITURE_ITEMS
            .iter()
            .copied()
            .find(|item| *item == value)
            .map(Self)
            .ok_or(RoomError::Invalid)
    }
    pub fn as_str(self) -> &'static str {
        self.0
    }
}

/// One grid-aligned furniture item.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FurniturePlacement {
    pub item: FurnitureId,
    pub cell_x: u16,
    pub cell_y: u16,
    pub layer: u8,
}

/// One poster on the wall.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PosterPlacement {
    pub poster_media_id: Uuid,
    pub slot: u8,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

/// Which way the character faces. Purely presentation; it never implies presence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Facing {
    Left,
    Right,
}

impl Facing {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Left => "LEFT",
            Self::Right => "RIGHT",
        }
    }
    fn parse(value: &str) -> Result<Self, RoomError> {
        match value {
            "LEFT" => Ok(Self::Left),
            "RIGHT" => Ok(Self::Right),
            _ => Err(RoomError::Invalid),
        }
    }
}

/// Where the profile's character stands.
///
/// A character standing in a room is a decoration of the room. It carries no
/// presence, session or activity field, so it cannot be read as "online".
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CharacterPlacement {
    pub cell_x: u16,
    pub cell_y: u16,
    pub facing: Facing,
}

/// The complete, validated room layout.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomLayout {
    pub wall: WallStyleId,
    pub floor: FloorStyleId,
    pub furniture: Vec<FurniturePlacement>,
    pub posters: Vec<PosterPlacement>,
    pub character: CharacterPlacement,
}

impl RoomLayout {
    /// The layout a profile that has never saved starts from.
    ///
    /// It is a real, drawable room: a plaster wall, a wood floor, one plant, one
    /// chair, no poster, and the character standing near the middle of the floor.
    pub fn initial() -> Self {
        Self {
            wall: WallStyleId::parse("plaster").expect("admitted wall style"),
            floor: FloorStyleId::parse("wood").expect("admitted floor style"),
            furniture: vec![
                FurniturePlacement {
                    item: FurnitureId::parse("plant").expect("admitted item"),
                    cell_x: 2,
                    cell_y: 20,
                    layer: 0,
                },
                FurniturePlacement {
                    item: FurnitureId::parse("chair").expect("admitted item"),
                    cell_x: 12,
                    cell_y: 22,
                    layer: 0,
                },
            ],
            posters: Vec::new(),
            character: CharacterPlacement {
                cell_x: 16,
                cell_y: 24,
                facing: Facing::Right,
            },
        }
    }

    /// Structural validation by bounds. See the module documentation for the
    /// exact rules; every rule here is also a rule of the frozen contract.
    pub fn validate(&self) -> Result<(), RoomError> {
        if self.furniture.len() > MAX_FURNITURE || self.posters.len() > MAX_POSTERS {
            return Err(RoomError::Invalid);
        }
        if !in_grid(self.character.cell_x) || !in_grid(self.character.cell_y) {
            return Err(RoomError::Invalid);
        }
        // A furniture cell is unique per layer. Two items in different layers may
        // share a cell: that is how a lamp stands on a table.
        let mut cells = BTreeSet::new();
        for placement in &self.furniture {
            if !in_grid(placement.cell_x)
                || !in_grid(placement.cell_y)
                || placement.layer > MAX_LAYER
            {
                return Err(RoomError::Invalid);
            }
            if !cells.insert((placement.layer, placement.cell_x, placement.cell_y)) {
                return Err(RoomError::Invalid);
            }
        }
        let mut slots = BTreeSet::new();
        let mut media = BTreeSet::new();
        for poster in &self.posters {
            if poster.slot > MAX_POSTER_SLOT
                || !in_grid(poster.x)
                || !in_grid(poster.y)
                || !(1..=MAX_POSTER_SPAN).contains(&poster.width)
                || !(1..=MAX_POSTER_SPAN).contains(&poster.height)
                || poster.x as u32 + poster.width as u32 > GRID_CELLS as u32
                || poster.y as u32 + poster.height as u32 > GRID_CELLS as u32
                || poster.poster_media_id.is_nil()
            {
                return Err(RoomError::Invalid);
            }
            // One poster per wall slot, and one placement per poster image: the
            // same image cannot hang twice.
            if !slots.insert(poster.slot) || !media.insert(poster.poster_media_id) {
                return Err(RoomError::Invalid);
            }
        }
        Ok(())
    }

    /// The canonical `social.RoomLayout` wire object.
    pub fn to_wire(&self) -> Value {
        json!({
            "version": 1,
            "wall": self.wall.as_str(),
            "floor": self.floor.as_str(),
            "furniture": self.furniture.iter().map(|placement| json!({
                "item_id": placement.item.as_str(),
                "cell_x": placement.cell_x,
                "cell_y": placement.cell_y,
                "layer": placement.layer,
            })).collect::<Vec<_>>(),
            "posters": self.posters.iter().map(|poster| json!({
                "poster_media_id": poster.poster_media_id.to_string(),
                "slot": poster.slot,
                "x": poster.x,
                "y": poster.y,
                "width": poster.width,
                "height": poster.height,
            })).collect::<Vec<_>>(),
            "character": {
                "cell_x": self.character.cell_x,
                "cell_y": self.character.cell_y,
                "facing": self.character.facing.as_str(),
            },
        })
    }

    /// Strict decode of a `social.RoomLayout` object.
    ///
    /// Every object is `deny_unknown_fields`, every number is an integer in its
    /// contract range, and the result is validated by [`RoomLayout::validate`]
    /// before it is returned. Duplicate JSON keys are refused by the shared strict
    /// visitor, so `{"wall":"plaster","wall":"brick"}` cannot be read two ways.
    pub fn parse(bytes: &[u8]) -> Result<Self, RoomError> {
        if bytes.is_empty() || bytes.len() > MAX_REQUEST_BYTES {
            return Err(RoomError::Invalid);
        }
        crate::avatar_motion_v2::strict_json::parse(bytes).map_err(|_| RoomError::Invalid)?;
        let wire: WireLayout = serde_json::from_slice(bytes).map_err(|_| RoomError::Invalid)?;
        let layout = wire.into_layout()?;
        layout.validate()?;
        Ok(layout)
    }

    /// Strict decode of a stored `jsonb` value.
    ///
    /// A stored layout is decoded through exactly the same parser as a submitted
    /// one. A row the current rules no longer admit fails closed; it is never
    /// silently replaced by [`RoomLayout::initial`].
    pub fn parse_value(value: &Value) -> Result<Self, RoomError> {
        let bytes = serde_json::to_vec(value).map_err(|_| RoomError::Invalid)?;
        Self::parse(&bytes)
    }
}

fn in_grid(value: u16) -> bool {
    value < GRID_CELLS
}

// ---------------------------------------------------------------------------
// Wire shapes
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireLayout {
    version: i64,
    wall: String,
    floor: String,
    furniture: Vec<WireFurniture>,
    posters: Vec<WirePoster>,
    character: WireCharacter,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireFurniture {
    item_id: String,
    cell_x: u16,
    cell_y: u16,
    layer: u8,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WirePoster {
    poster_media_id: String,
    slot: u8,
    x: u16,
    y: u16,
    width: u16,
    height: u16,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireCharacter {
    cell_x: u16,
    cell_y: u16,
    facing: String,
}

impl WireLayout {
    fn into_layout(self) -> Result<RoomLayout, RoomError> {
        // `version` is the closed layout version. A different integer is a layout
        // this server cannot draw, so it is refused as unsupported rather than
        // treated as version 1.
        if self.version != 1 {
            return Err(RoomError::UnsupportedVersion);
        }
        if self.furniture.len() > MAX_FURNITURE || self.posters.len() > MAX_POSTERS {
            return Err(RoomError::Invalid);
        }
        Ok(RoomLayout {
            wall: WallStyleId::parse(&self.wall)?,
            floor: FloorStyleId::parse(&self.floor)?,
            furniture: self
                .furniture
                .into_iter()
                .map(|placement| {
                    Ok(FurniturePlacement {
                        item: FurnitureId::parse(&placement.item_id)?,
                        cell_x: placement.cell_x,
                        cell_y: placement.cell_y,
                        layer: placement.layer,
                    })
                })
                .collect::<Result<_, RoomError>>()?,
            posters: self
                .posters
                .into_iter()
                .map(|poster| {
                    Ok(PosterPlacement {
                        poster_media_id: canonical_uuid(&poster.poster_media_id)?,
                        slot: poster.slot,
                        x: poster.x,
                        y: poster.y,
                        width: poster.width,
                        height: poster.height,
                    })
                })
                .collect::<Result<_, RoomError>>()?,
            character: CharacterPlacement {
                cell_x: self.character.cell_x,
                cell_y: self.character.cell_y,
                facing: Facing::parse(&self.character.facing)?,
            },
        })
    }
}

/// A canonical lower-case UUID string, exactly as the contract's `Uuid` pattern.
fn canonical_uuid(value: &str) -> Result<Uuid, RoomError> {
    let id = Uuid::parse_str(value).map_err(|_| RoomError::Invalid)?;
    if id.to_string() != value || id.is_nil() {
        return Err(RoomError::Invalid);
    }
    Ok(id)
}

// ---------------------------------------------------------------------------
// Commands
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireSave {
    expected_revision: i64,
    layout: Value,
}

/// One validated `PUT /api/me/room` command (`social.SaveRoom`).
///
/// The owner is the authenticated profile and is not part of the body: there is
/// no field through which a caller could name another profile's room.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SaveRoom {
    pub expected: Revision,
    pub layout: RoomLayout,
}

impl SaveRoom {
    pub fn parse(bytes: &[u8]) -> Result<Self, RoomError> {
        if bytes.is_empty() || bytes.len() > MAX_REQUEST_BYTES {
            return Err(RoomError::Invalid);
        }
        crate::avatar_motion_v2::strict_json::parse(bytes).map_err(|_| RoomError::Invalid)?;
        let wire: WireSave = serde_json::from_slice(bytes).map_err(|_| RoomError::Invalid)?;
        Ok(Self {
            expected: Revision::new(wire.expected_revision).map_err(|_| RoomError::Invalid)?,
            layout: RoomLayout::parse_value(&wire.layout)?,
        })
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireVisibility {
    expected_revision: i64,
    visibility: String,
}

/// One validated `PATCH /api/me/room/visibility` command
/// (`social.SetRoomVisibility`).
///
/// It carries the visibility and the expected revision and nothing else. There is
/// deliberately no audience field: room visibility never reads or writes an
/// intro post's audience.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SetRoomVisibility {
    pub expected: Revision,
    pub visibility: RoomVisibility,
}

impl SetRoomVisibility {
    pub fn parse(bytes: &[u8]) -> Result<Self, RoomError> {
        if bytes.is_empty() || bytes.len() > MAX_REQUEST_BYTES {
            return Err(RoomError::Invalid);
        }
        crate::avatar_motion_v2::strict_json::parse(bytes).map_err(|_| RoomError::Invalid)?;
        let wire: WireVisibility = serde_json::from_slice(bytes).map_err(|_| RoomError::Invalid)?;
        Ok(Self {
            expected: Revision::new(wire.expected_revision).map_err(|_| RoomError::Invalid)?,
            visibility: RoomVisibility::from_str(&wire.visibility)?,
        })
    }
}

/// The two poster presets of the browser conversion (domain-contract.md §6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PosterPreset {
    /// 64 × 64, more detail.
    Fine,
    /// 32 × 32, bold and poster-like.
    Chunky,
}

impl PosterPreset {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Fine => "FINE",
            Self::Chunky => "CHUNKY",
        }
    }
    /// The native pixel size the browser conversion produces for this preset.
    pub fn side(self) -> u32 {
        match self {
            Self::Fine => 64,
            Self::Chunky => 32,
        }
    }
    pub fn parse(value: &str) -> Result<Self, RoomError> {
        match value {
            "FINE" => Ok(Self::Fine),
            "CHUNKY" => Ok(Self::Chunky),
            _ => Err(RoomError::Invalid),
        }
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct WireUploadPoster {
    preset: String,
    width: u32,
    height: u32,
    content_type: String,
    bytes_base64: String,
}

/// One validated `POST /api/me/room/posters` command (`social.UploadPoster`).
///
/// The declared dimensions are what the client says it produced; the server
/// decodes the PNG and refuses a mismatch, so a declared size is a claim to be
/// checked, never a fact to be trusted.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UploadPoster {
    pub preset: PosterPreset,
    pub width: u32,
    pub height: u32,
    /// The decoded PNG bytes. Base64 is decoded here so the repository never sees
    /// an encoded string it might store by accident.
    pub bytes: Vec<u8>,
}

impl UploadPoster {
    pub fn parse(bytes: &[u8]) -> Result<Self, RoomError> {
        if bytes.is_empty() || bytes.len() > MAX_REQUEST_BYTES {
            return Err(RoomError::Invalid);
        }
        crate::avatar_motion_v2::strict_json::parse(bytes).map_err(|_| RoomError::Invalid)?;
        let wire: WireUploadPoster =
            serde_json::from_slice(bytes).map_err(|_| RoomError::Invalid)?;
        // The contract's closed content type. A JPEG, WebP or GIF declaration is
        // refused here, before any byte is decoded.
        if wire.content_type != "image/png" {
            return Err(RoomError::Invalid);
        }
        let preset = PosterPreset::parse(&wire.preset)?;
        for side in [wire.width, wire.height] {
            if !(8..=MAX_POSTER_SIDE).contains(&side) {
                return Err(RoomError::Invalid);
            }
        }
        let decoded = decode_base64(&wire.bytes_base64).ok_or(RoomError::Invalid)?;
        if decoded.is_empty() || decoded.len() > MAX_POSTER_BYTES {
            return Err(RoomError::Invalid);
        }
        Ok(Self {
            preset,
            width: wire.width,
            height: wire.height,
            bytes: decoded,
        })
    }

    /// The declared size is the preset's native size.
    ///
    /// The conversion contract fixes the output size per preset, so a poster that
    /// declares any other size was not produced by the contract's conversion. The
    /// rule is enforced here so the client and the server cannot disagree about
    /// what "Fine" and "Chunky" mean.
    pub fn matches_preset(&self) -> bool {
        self.width == self.preset.side() && self.height == self.preset.side()
    }
}

/// Standard base64 (with padding) decode of a bounded string.
///
/// Hand-written rather than pulled from a new dependency: the crate already
/// depends on `base64ct`, but the decode here must also refuse non-canonical
/// trailing bits, which is exactly what a hand-written strict decoder does.
fn decode_base64(value: &str) -> Option<Vec<u8>> {
    let bytes = value.as_bytes();
    if bytes.len() % 4 != 0 || bytes.is_empty() {
        return None;
    }
    let mut out = Vec::with_capacity(bytes.len() / 4 * 3);
    let padding = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    if padding > 2 {
        return None;
    }
    for (index, chunk) in bytes.chunks(4).enumerate() {
        let last = index + 1 == bytes.len() / 4;
        let mut values = [0u8; 4];
        for (position, byte) in chunk.iter().enumerate() {
            values[position] = match byte {
                b'A'..=b'Z' => byte - b'A',
                b'a'..=b'z' => byte - b'a' + 26,
                b'0'..=b'9' => byte - b'0' + 52,
                b'+' => 62,
                b'/' => 63,
                // Padding is only legal in the final chunk, and only at the end.
                b'=' if last && position >= 4 - padding => 0,
                _ => return None,
            };
        }
        if last && padding > 0 {
            // Canonical trailing bits: the discarded bits must be zero, so the
            // same bytes cannot be spelled two ways.
            let mask = match padding {
                1 => values[2] & 0b11,
                2 => values[1] & 0b1111,
                _ => 0,
            };
            if mask != 0 {
                return None;
            }
        }
        out.push((values[0] << 2) | (values[1] >> 4));
        if !(last && padding >= 2) {
            out.push((values[1] << 4) | (values[2] >> 2));
        }
        if !(last && padding >= 1) {
            out.push((values[2] << 6) | values[3]);
        }
    }
    Some(out)
}

/// The closed set of poster moderation states (`social.PosterModeration`).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PosterModeration {
    Visible,
    Hidden,
    PendingReview,
}

impl PosterModeration {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Visible => "VISIBLE",
            Self::Hidden => "HIDDEN",
            Self::PendingReview => "PENDING_REVIEW",
        }
    }
}

/// The stored state of one room, before it is projected onto the wire.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredRoom {
    pub owner: ProfileId,
    pub visibility: RoomVisibility,
    pub layout: RoomLayout,
    pub revision: Revision,
}

/// The stored state of one room-poster media asset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct StoredPoster {
    pub media_id: Uuid,
    pub width: u32,
    pub height: u32,
    pub revision: Revision,
}

/// The `Domain_feed_Media` projection of a room poster.
///
/// `purpose` is `POST` because the frozen `Domain_feed_Media` enum has no
/// `ROOM_POSTER` member; the profile-scoped purpose is carried by the
/// `profile_media` row, whose closed set does include `ROOM_POSTER`. The
/// projection is a render hint, not an authorization: reading the bytes is
/// re-checked per request against the media row itself.
pub fn poster_media_wire(poster: &StoredPoster) -> Value {
    json!({
        "media_id": poster.media_id.to_string(),
        "state": "READY",
        "revision": poster.revision.get(),
        "width": poster.width,
        "height": poster.height,
        "content_type": "image/png",
        "purpose": "POST",
        "read_path": format!("/api/profiles/room/media/{}/content", poster.media_id),
    })
}

/// The `social.ProfileRoom` projection (a visitor's view).
pub fn profile_room_wire(room: &StoredRoom, posters: &[StoredPoster], updated_at: &str) -> Value {
    json!({
        "owner_profile_id": room.owner.to_string(),
        "visibility": room.visibility.as_str(),
        "layout": room.layout.to_wire(),
        "posters": posters.iter().map(poster_media_wire).collect::<Vec<_>>(),
        "revision": room.revision.get(),
        "updated_at": updated_at,
    })
}

/// The `social.OwnRoom` projection (the owner's view).
pub fn own_room_wire(
    room: &StoredRoom,
    posters: &[StoredPoster],
    moderation: &[(Uuid, PosterModeration)],
    updated_at: &str,
) -> Value {
    let mut value = profile_room_wire(room, posters, updated_at);
    if let Some(object) = value.as_object_mut() {
        object.insert("is_owner".to_owned(), Value::Bool(true));
        object.insert(
            "poster_moderation".to_owned(),
            Value::Array(
                moderation
                    .iter()
                    .map(|(media_id, state)| {
                        json!({"media_id": media_id.to_string(), "state": state.as_str()})
                    })
                    .collect(),
            ),
        );
    }
    value
}

/// The `social.RoomPoster` projection of a completed upload.
pub fn room_poster_wire(upload: &UploadPoster, poster: &StoredPoster) -> Value {
    json!({
        "media": poster_media_wire(poster),
        "preset": upload.preset.as_str(),
        "width": upload.width,
        "height": upload.height,
        "revision": poster.revision.get(),
    })
}

/// The canonical `AssetId` for a poster preset, for callers that need one.
pub fn preset_asset(preset: PosterPreset) -> Result<AssetId, RoomError> {
    AssetId::new(format!("room_poster_{}", preset.as_str().to_ascii_lowercase()))
        .map_err(|_| RoomError::Invalid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_canonical_input() {
        // "PNG" then a padded pair.
        assert_eq!(decode_base64("UE5H").as_deref(), Some(b"PNG".as_ref()));
        assert_eq!(decode_base64("UE4=").as_deref(), Some(b"PN".as_ref()));
        assert_eq!(decode_base64("UA==").as_deref(), Some(b"P".as_ref()));
    }

    #[test]
    fn base64_refuses_non_canonical_and_foreign_input() {
        // Non-zero discarded bits spell the same byte a second way.
        assert!(decode_base64("UB==").is_none());
        assert!(decode_base64("UE5=").is_none());
        assert!(decode_base64("UE5").is_none());
        assert!(decode_base64("UE5H=").is_none());
        assert!(decode_base64("U E5").is_none());
        assert!(decode_base64("UE5-").is_none());
        assert!(decode_base64("").is_none());
    }
}
