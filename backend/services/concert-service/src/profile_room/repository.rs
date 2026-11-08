//! WP-07 T-602: room persistence.
//!
//! The room is a profile-owned row (`profile_rooms`, WP-02 T-107). This module is
//! the only writer of it, and it keeps three disciplines:
//!
//! * **Ownership is decided server-side.** Every statement binds the owner the
//!   authenticated request was verified for. There is no parameter through which
//!   a caller could name another profile's room, and the schema's revision guard
//!   refuses re-pointing a stored room at another profile. A save by a non-owner
//!   therefore cannot be expressed, and the route layer's refusal is the frozen
//!   `403 PERMISSION_DENIED` for a caller that names a room it does not own.
//! * **A revision conflict writes nothing and returns the current room.** The
//!   compare-and-set happens inside one transaction; a mismatch rolls back and
//!   the caller receives `Conflict`, then reads the current room so the client can
//!   offer an explicit resolution instead of silently discarding its layout.
//! * **A stored layout is decoded strictly, or fails closed.** A row the current
//!   rules no longer admit is reported as an invalid state; it is never replaced
//!   by the default layout behind the owner's back.
//!
//! Nothing here reads or writes an intro post. The room is subordinate to the
//! profile and independent of every mutuals-intro post (domain-contract.md §7).
use concert_domain::profile_room::{
    own_room_wire, profile_room_wire, room_poster_wire, FurnitureId, PosterModeration,
    ProfileId, RoomError, RoomLayout, RoomVisibility, SaveRoom, SetRoomVisibility, StoredPoster,
    StoredRoom, UploadPoster, MAX_REQUEST_BYTES,
};
use concert_domain::Revision;
use serde_json::{json, Value};
use sqlx::{postgres::PgRow, PgPool, Row};
use std::sync::Arc;

/// The closed error set of a room operation.
///
/// The HTTP projection is fixed: `Invalid`/`InvalidState` are `400`,
/// `Unauthenticated` is `401`, `Denied` is `403`, `Missing` is `404`, `Conflict`
/// is `409`, `TooLarge` is `413`, `UnsupportedMedia` is `415`, and `Unavailable`
/// is `503`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomErrorKind {
    Invalid,
    InvalidState,
    Unauthenticated,
    Denied,
    Missing,
    Conflict,
    TooLarge,
    UnsupportedMedia,
    Unavailable,
}

impl From<RoomError> for RoomErrorKind {
    fn from(error: RoomError) -> Self {
        match error {
            RoomError::Invalid => Self::Invalid,
            RoomError::UnsupportedVersion => Self::InvalidState,
        }
    }
}

/// The authenticated actor of a room request.
///
/// `owner` is the profile the session belongs to. It is never read from a request
/// body.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomActor {
    pub owner: ProfileId,
}

impl RoomActor {
    pub fn new(owner: ProfileId) -> Self {
        Self { owner }
    }
}

/// Room persistence bound to one database pool.
///
/// The pool is the only dependency: unlike the avatar, a room has no release-bound
/// catalog to project. The closed style and item sets live in the domain value.
#[derive(Clone)]
pub struct RoomPersistence {
    pool: PgPool,
}

impl RoomPersistence {
    pub fn new(pool: PgPool) -> Arc<Self> {
        Arc::new(Self { pool })
    }

    /// The owner's own room (`social.OwnRoom`), or the default room at revision 1
    /// for a profile that has never saved.
    ///
    /// A read never creates a row, so a read cannot authorize or fabricate a write.
    pub async fn read_own(
        &self,
        actor: RoomActor,
    ) -> Result<Value, RoomErrorKind> {
        let (room, updated_at) = self.load(actor.owner).await?;
        self.project_own(&room, &updated_at).await
    }

    /// A visitor's view of a profile's room (`social.ProfileRoom`).
    ///
    /// A `PRIVATE` room is `Missing` for a visitor — the same answer as a room
    /// that does not exist — so the endpoint cannot be used to learn that a
    /// profile has a room at all. The owner always sees their own room.
    pub async fn read_for_visitor(
        &self,
        viewer: RoomActor,
        target: ProfileId,
    ) -> Result<Value, RoomErrorKind> {
        let (room, updated_at) = self.load(target).await?;
        if viewer.owner != target && room.visibility != RoomVisibility::Public {
            return Err(RoomErrorKind::Missing);
        }
        let posters = self.posters_for(&room).await?;
        Ok(profile_room_wire(&room, &posters, &updated_at))
    }

    /// The stored visibility of a profile's room, defaulting to the product
    /// default for a profile that has never chosen.
    pub async fn stored_visibility(
        &self,
        target: ProfileId,
    ) -> Result<RoomVisibility, RoomErrorKind> {
        let (room, _) = self.load(target).await?;
        Ok(room.visibility)
    }

    /// Whether a visitor may open the room at all. The profile projection reports
    /// this as `room_available`.
    pub async fn is_available_to(
        &self,
        viewer: ProfileId,
        target: ProfileId,
    ) -> Result<bool, RoomErrorKind> {
        let visibility = self.stored_visibility(target).await?;
        Ok(super::visibility::room_available(viewer, target, visibility))
    }

    /// `PUT /api/me/room` — replace the layout at `expected_revision`.
    ///
    /// On success the room is returned at the next revision. On a revision
    /// mismatch the stored room is untouched and `Conflict` is returned; the route
    /// layer then calls [`RoomPersistence::read_own`] so the `409` carries the
    /// current layout.
    pub async fn save(
        &self,
        actor: RoomActor,
        body: &[u8],
    ) -> Result<Value, RoomErrorKind> {
        if body.len() > MAX_REQUEST_BYTES {
            return Err(RoomErrorKind::TooLarge);
        }
        let command = SaveRoom::parse(body)?;
        self.commit_layout(actor.owner, command).await
    }

    /// `PATCH /api/me/room/visibility`.
    ///
    /// This statement touches `profile_rooms.visibility` and nothing else. It does
    /// not read or write `intro_posts`, so it cannot change any intro post's
    /// audience — and, symmetrically, an intro-audience write cannot change this.
    pub async fn set_visibility(
        &self,
        actor: RoomActor,
        body: &[u8],
    ) -> Result<Value, RoomErrorKind> {
        if body.len() > MAX_REQUEST_BYTES {
            return Err(RoomErrorKind::TooLarge);
        }
        let command = SetRoomVisibility::parse(body)?;
        let owner = actor.owner.uuid();
        let mut tx = self.pool.begin().await.map_err(|_| RoomErrorKind::Unavailable)?;
        let (current, _) = self.load_in(&mut tx, actor.owner).await?;
        if current.revision != command.expected {
            tx.rollback().await.map_err(|_| RoomErrorKind::Unavailable)?;
            return Err(RoomErrorKind::Conflict);
        }
        // The row exists once a revision is known; a profile that never saved has
        // no row, so there is nothing to update and the request is a conflict.
        let next = command
            .expected
            .next()
            .map_err(|_| RoomErrorKind::Conflict)?;
        let updated = sqlx::query(
            "update public.profile_rooms set visibility=$2, revision=$3 \
             where owner_profile_id=$1 and revision=$4",
        )
        .bind(owner)
        .bind(command.visibility.as_str())
        .bind(next.get())
        .bind(command.expected.get())
        .execute(&mut *tx)
        .await
        .map_err(|_| RoomErrorKind::Unavailable)?;
        if updated.rows_affected() != 1 {
            // A profile that never saved has no row to update, so a visibility
            // change at revision 1 has nothing to change: it is a conflict, not a
            // silent no-op.
            tx.rollback().await.map_err(|_| RoomErrorKind::Unavailable)?;
            return Err(RoomErrorKind::Conflict);
        }
        let (room, updated_at) = self.load_in(&mut tx, actor.owner).await?;
        if room.revision != next || room.visibility != command.visibility {
            return Err(RoomErrorKind::InvalidState);
        }
        tx.commit().await.map_err(|_| RoomErrorKind::Unavailable)?;
        self.project_own(&room, &updated_at).await
    }

    /// `POST /api/me/room/posters`.
    ///
    /// The bytes are validated as a PNG of the declared size and then stored
    /// **exactly as submitted** as a `ROOM_POSTER` media asset. The server never
    /// re-pixelates: the conversion belongs to the browser (domain-contract.md §6).
    pub async fn upload_poster(
        &self,
        actor: RoomActor,
        body: &[u8],
    ) -> Result<Value, RoomErrorKind> {
        if body.len() > MAX_REQUEST_BYTES {
            return Err(RoomErrorKind::TooLarge);
        }
        let command = UploadPoster::parse(body)?;
        if !command.matches_preset() {
            return Err(RoomErrorKind::Invalid);
        }
        let dimensions = validate_png(&command.bytes)?;
        if dimensions != (command.width, command.height) {
            // The declared size is a claim. A PNG whose real size differs is
            // refused: it was not produced by the contract's conversion, and
            // storing it would put a lie in the room's poster list.
            return Err(RoomErrorKind::Invalid);
        }
        let poster = self.insert_poster(actor.owner, &command).await?;
        Ok(room_poster_wire(&command, &poster))
    }

    // -----------------------------------------------------------------------
    // Internals
    // -----------------------------------------------------------------------

    async fn commit_layout(
        &self,
        owner: ProfileId,
        command: SaveRoom,
    ) -> Result<Value, RoomErrorKind> {
        let mut tx = self.pool.begin().await.map_err(|_| RoomErrorKind::Unavailable)?;
        let (current, _) = self.load_in(&mut tx, owner).await?;
        if current.revision != command.expected {
            tx.rollback().await.map_err(|_| RoomErrorKind::Unavailable)?;
            return Err(RoomErrorKind::Conflict);
        }
        let next = command.expected.next().map_err(|_| RoomErrorKind::Conflict)?;
        let layout = command.layout.to_wire();
        // A layout may only reference posters the owner actually holds as READY
        // ROOM_POSTER assets. This is the same ownership rule the upload applies,
        // checked again at placement so a save cannot point at another profile's
        // media or at an intro attachment.
        for poster in &command.layout.posters {
            let owned: bool = sqlx::query_scalar(
                "select exists(select 1 from public.profile_media \
                 where media_id=$1 and owner_profile_id=$2 and purpose='ROOM_POSTER' \
                   and state='READY')",
            )
            .bind(poster.poster_media_id)
            .bind(owner.uuid())
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| RoomErrorKind::Unavailable)?;
            if !owned {
                tx.rollback().await.map_err(|_| RoomErrorKind::Unavailable)?;
                return Err(RoomErrorKind::Invalid);
            }
        }
        // An update of a missing row affects zero rows without an error, so the
        // first save has to insert. The primary key makes a concurrent first save
        // fail rather than overwrite it, and the revision is re-read below.
        let updated = sqlx::query(
            "update public.profile_rooms set layout=$2, revision=$3 \
             where owner_profile_id=$1 and revision=$4",
        )
        .bind(owner.uuid())
        .bind(&layout)
        .bind(next.get())
        .bind(command.expected.get())
        .execute(&mut *tx)
        .await
        .map_err(|_| RoomErrorKind::Unavailable)?;
        if updated.rows_affected() == 0 {
            let inserted = sqlx::query(
                "insert into public.profile_rooms(owner_profile_id, layout, visibility, revision) \
                 values($1,$2,$3,$4) on conflict (owner_profile_id) do nothing",
            )
            .bind(owner.uuid())
            .bind(&layout)
            .bind(RoomVisibility::default_for_new_room().as_str())
            .bind(next.get())
            .execute(&mut *tx)
            .await
            .map_err(|_| RoomErrorKind::Unavailable)?;
            if inserted.rows_affected() != 1 {
                // Another writer created the row between the update and the
                // insert. That writer won this revision; this one conflicts.
                tx.rollback().await.map_err(|_| RoomErrorKind::Unavailable)?;
                return Err(RoomErrorKind::Conflict);
            }
        }
        let (room, updated_at) = self.load_in(&mut tx, owner).await?;
        if room.revision != next || room.layout != command.layout {
            return Err(RoomErrorKind::InvalidState);
        }
        tx.commit().await.map_err(|_| RoomErrorKind::Unavailable)?;
        self.project_own(&room, &updated_at).await
    }

    /// The stored room and its `updated_at`, or the default room at revision 1.
    async fn load(&self, owner: ProfileId) -> Result<(StoredRoom, String), RoomErrorKind> {
        let row = sqlx::query(
            "select layout, visibility, revision, updated_at from public.profile_rooms \
             where owner_profile_id=$1",
        )
        .bind(owner.uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| RoomErrorKind::Unavailable)?;
        match row {
            Some(row) => room_from_row(row, owner),
            None => Ok((default_room(owner), never_written())),
        }
    }

    async fn load_in(
        &self,
        connection: &mut sqlx::PgConnection,
        owner: ProfileId,
    ) -> Result<(StoredRoom, String), RoomErrorKind> {
        let row = sqlx::query(
            "select layout, visibility, revision, updated_at from public.profile_rooms \
             where owner_profile_id=$1 for update",
        )
        .bind(owner.uuid())
        .fetch_optional(&mut *connection)
        .await
        .map_err(|_| RoomErrorKind::Unavailable)?;
        match row {
            Some(row) => room_from_row(row, owner),
            None => Ok((default_room(owner), never_written())),
        }
    }

    async fn posters_for(&self, room: &StoredRoom) -> Result<Vec<StoredPoster>, RoomErrorKind> {
        let mut posters = Vec::new();
        for placement in &room.layout.posters {
            let row = sqlx::query(
                "select media_id, width, height, revision from public.profile_media \
                 where media_id=$1 and owner_profile_id=$2 and purpose='ROOM_POSTER' \
                   and state='READY'",
            )
            .bind(placement.poster_media_id)
            .bind(room.owner.uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(|_| RoomErrorKind::Unavailable)?;
            let Some(row) = row else {
                // A placement whose media is gone is not projected as a poster.
                // The layout is unchanged; the room simply renders that slot empty
                // rather than failing the whole read.
                continue;
            };
            posters.push(poster_from_row(&row)?);
        }
        Ok(posters)
    }

    async fn project_own(
        &self,
        room: &StoredRoom,
        updated_at: &str,
    ) -> Result<Value, RoomErrorKind> {
        let posters = self.posters_for(room).await?;
        let moderation: Vec<(uuid::Uuid, PosterModeration)> = posters
            .iter()
            .map(|poster| (poster.media_id, PosterModeration::Visible))
            .collect();
        Ok(own_room_wire(room, &posters, &moderation, updated_at))
    }

    async fn insert_poster(
        &self,
        owner: ProfileId,
        command: &UploadPoster,
    ) -> Result<StoredPoster, RoomErrorKind> {
        let media_id = uuid::Uuid::new_v4();
        let digest = {
            use sha2::{Digest, Sha256};
            Sha256::digest(&command.bytes).to_vec()
        };
        let bytes = command.bytes.len() as i32;
        let mut tx = self.pool.begin().await.map_err(|_| RoomErrorKind::Unavailable)?;
        // The profile-social write guard takes the account lock and refuses a
        // fenced account; taking it here first gives the same ordering as every
        // other profile writer instead of letting the trigger do it mid-statement.
        crate::world::account_deletion::lock_user_in(&mut tx, owner.uuid())
            .await
            .map_err(|_| RoomErrorKind::Unavailable)?;
        sqlx::query(
            "insert into public.profile_media(media_id, owner_profile_id, purpose, content_type, \
             state, declared_bytes, sanitized, width, height, input_sha256) \
             values($1,$2,'ROOM_POSTER','image/png','READY',$3,$4,$5,$6,$7)",
        )
        .bind(media_id)
        .bind(owner.uuid())
        .bind(bytes)
        .bind(&command.bytes)
        .bind(command.width as i32)
        .bind(command.height as i32)
        .bind(&digest)
        .execute(&mut *tx)
        .await
        .map_err(|_| RoomErrorKind::Unavailable)?;
        // The stored bytes are the submitted bytes. They are read back inside the
        // same transaction so the response describes what was actually stored.
        let stored: Vec<u8> = sqlx::query_scalar(
            "select sanitized from public.profile_media where media_id=$1",
        )
        .bind(media_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| RoomErrorKind::Unavailable)?;
        if stored != command.bytes {
            return Err(RoomErrorKind::InvalidState);
        }
        let row = sqlx::query(
            "select media_id, width, height, revision from public.profile_media where media_id=$1",
        )
        .bind(media_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(|_| RoomErrorKind::Unavailable)?;
        let poster = poster_from_row(&row)?;
        tx.commit().await.map_err(|_| RoomErrorKind::Unavailable)?;
        Ok(poster)
    }
}

fn default_room(owner: ProfileId) -> StoredRoom {
    StoredRoom {
        owner,
        visibility: RoomVisibility::default_for_new_room(),
        layout: RoomLayout::initial(),
        revision: Revision::initial(),
    }
}

fn room_from_row(row: PgRow, owner: ProfileId) -> Result<(StoredRoom, String), RoomErrorKind> {
    let layout: Value = row.try_get("layout").map_err(|_| RoomErrorKind::InvalidState)?;
    let visibility: String = row.try_get("visibility").map_err(|_| RoomErrorKind::InvalidState)?;
    let revision: i64 = row.try_get("revision").map_err(|_| RoomErrorKind::InvalidState)?;
    let updated_at: chrono::DateTime<chrono::Utc> =
        row.try_get("updated_at").map_err(|_| RoomErrorKind::InvalidState)?;
    Ok((
        StoredRoom {
            owner,
            visibility: visibility
                .parse()
                .map_err(|_| RoomErrorKind::InvalidState)?,
            layout: RoomLayout::parse_value(&layout).map_err(|_| RoomErrorKind::InvalidState)?,
            revision: Revision::new(revision).map_err(|_| RoomErrorKind::InvalidState)?,
        },
        // The frozen `UtcTimestamp` spelling: RFC 3339 in UTC with millisecond
        // precision and a `Z` suffix, never a local offset.
        updated_at.to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
    ))
}

fn poster_from_row(row: &PgRow) -> Result<StoredPoster, RoomErrorKind> {
    Ok(StoredPoster {
        media_id: row.try_get("media_id").map_err(|_| RoomErrorKind::InvalidState)?,
        width: u32::try_from(row.try_get::<i32, _>("width").map_err(|_| RoomErrorKind::InvalidState)?)
            .map_err(|_| RoomErrorKind::InvalidState)?,
        height: u32::try_from(row.try_get::<i32, _>("height").map_err(|_| RoomErrorKind::InvalidState)?)
            .map_err(|_| RoomErrorKind::InvalidState)?,
        revision: Revision::new(row.try_get("revision").map_err(|_| RoomErrorKind::InvalidState)?)
            .map_err(|_| RoomErrorKind::InvalidState)?,
    })
}

/// The `updated_at` of a room that has never been written.
///
/// The default room has no row, so it is reported at the Unix epoch: a timestamp
/// that is visibly "never written" rather than a fabricated recent one.
fn never_written() -> String {
    "1970-01-01T00:00:00.000Z".to_owned()
}

/// Validate a PNG and return its real pixel dimensions.
///
/// The checks are, in order: the eight-byte PNG signature, a well-formed IHDR
/// chunk with the fixed bit depth/colour type/compression/filter/interlace fields
/// the conversion produces, the declared dimensions inside the poster bound, and
/// a complete decode of every pixel so a truncated or corrupt file cannot be
/// stored as a poster. A JPEG, WebP or GIF is refused by the signature check
/// before any decoder runs.
pub fn validate_png(bytes: &[u8]) -> Result<(u32, u32), RoomErrorKind> {
    const SIGNATURE: [u8; 8] = [0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a];
    if bytes.len() < 8 + 25 || bytes[..8] != SIGNATURE {
        return Err(RoomErrorKind::UnsupportedMedia);
    }
    // The first chunk of every PNG is IHDR.
    if &bytes[12..16] != b"IHDR" {
        return Err(RoomErrorKind::Invalid);
    }
    let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
    let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
    let bit_depth = bytes[24];
    let colour_type = bytes[25];
    let compression = bytes[26];
    let filter = bytes[27];
    let interlace = bytes[28];
    if width == 0 || height == 0 || width > 128 || height > 128 {
        return Err(RoomErrorKind::Invalid);
    }
    if !matches!(bit_depth, 1 | 2 | 4 | 8 | 16)
        || !matches!(colour_type, 0 | 2 | 3 | 4 | 6)
        || compression != 0
        || filter != 0
        || !matches!(interlace, 0 | 1)
    {
        return Err(RoomErrorKind::Invalid);
    }
    // A full decode proves the file is a real image of exactly that size. The
    // decoder limits are the poster bound, so a malformed IHDR cannot make the
    // decoder allocate beyond it.
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(128);
    limits.max_image_height = Some(128);
    limits.max_alloc = Some(4 * 1024 * 1024);
    let decoder = image::codecs::png::PngDecoder::with_limits(
        std::io::Cursor::new(bytes),
        limits,
    )
    .map_err(|_| RoomErrorKind::Invalid)?;
    use image::ImageDecoder;
    if decoder.is_apng().unwrap_or(false) {
        return Err(RoomErrorKind::Invalid);
    }
    let (decoded_width, decoded_height) = decoder.dimensions();
    if (decoded_width, decoded_height) != (width, height) {
        return Err(RoomErrorKind::Invalid);
    }
    let decoded = image::DynamicImage::from_decoder(decoder).map_err(|_| RoomErrorKind::Invalid)?;
    if decoded.width() != width || decoded.height() != height {
        return Err(RoomErrorKind::Invalid);
    }
    Ok((width, height))
}

/// The HTTP projection of an error. Kept beside the error so the route and the
/// persistence cannot disagree about a status.
pub fn http_status(error: RoomErrorKind) -> u16 {
    match error {
        RoomErrorKind::Invalid | RoomErrorKind::InvalidState => 400,
        RoomErrorKind::Unauthenticated => 401,
        RoomErrorKind::Denied => 403,
        RoomErrorKind::Missing => 404,
        RoomErrorKind::Conflict => 409,
        RoomErrorKind::TooLarge => 413,
        RoomErrorKind::UnsupportedMedia => 415,
        RoomErrorKind::Unavailable => 503,
    }
}

/// The frozen reason string for an error (`details.reason`).
pub fn http_reason(error: RoomErrorKind) -> &'static str {
    match error {
        RoomErrorKind::Invalid | RoomErrorKind::InvalidState => "INVALID_INPUT",
        RoomErrorKind::Unauthenticated => "AUTHENTICATION_REQUIRED",
        RoomErrorKind::Denied => "PERMISSION_DENIED",
        RoomErrorKind::Missing => "RESOURCE_UNAVAILABLE",
        RoomErrorKind::Conflict => "VERSION_CONFLICT",
        RoomErrorKind::TooLarge => "PAYLOAD_TOO_LARGE",
        RoomErrorKind::UnsupportedMedia => "INVALID_MEDIA_TYPE",
        RoomErrorKind::Unavailable => "DEPENDENCY_UNAVAILABLE",
    }
}

/// The `409` body: the reason plus the current room, so the client can offer an
/// explicit resolution without a second round trip.
pub fn conflict_body(current: Value) -> Value {
    json!({
        "code": "ROOM_409",
        "message": "Room request unavailable",
        "details": {"reason": "VERSION_CONFLICT", "current": current},
    })
}

/// The closed furniture identifier set, exposed for the route layer's diagnostics.
pub fn furniture_ids() -> Vec<&'static str> {
    concert_domain::profile_room::FURNITURE_ITEMS.to_vec()
}

/// The admitted furniture identifier of a wire string, or `None`.
pub fn furniture_id(value: &str) -> Option<FurnitureId> {
    FurnitureId::parse(value).ok()
}
