//! WP-06 T-501: the profile media upload lifecycle.
//!
//! Three steps, exactly as the concert media runtime has them:
//!
//! 1. `init` — declare the purpose, the content type, the byte length and the
//!    SHA-256 of the bytes about to be sent. The row is created `PENDING`.
//! 2. `put bytes` — send exactly those bytes. The row becomes `UPLOADED`.
//! 3. `finalize` — the sanitizer decodes, verifies the format and single frame,
//!    and re-encodes without metadata. The row becomes `READY` with width and
//!    height.
//!
//! Two rules are structural rather than conventional:
//!
//! * **Purpose/context mismatch is refused up front.** [`UploadPurpose`] is the
//!   closed set of purposes a given *context* admits. The profile-post context
//!   admits `PROFILE_POST` and `PROFILE_IMAGE`; the intro context admits
//!   `INTRO_POST`; the room context admits `ROOM_POSTER`. A `ROOM_POSTER` asset
//!   therefore cannot be finalized through the profile-post path — the request
//!   is refused before a row is even read, and independently the stored row's
//!   purpose is re-checked inside the finalize transaction.
//! * **No pixel conversion.** Nothing in this module quantizes, downsamples,
//!   posterizes or palette-reduces pixels. The only transformation is the shared
//!   [`ImageSanitizer`], whose contract is "re-encode the decoded frame without
//!   metadata" — the same one a concert upload goes through. The stored `READY`
//!   bytes keep the source dimensions.
//!
//! Orphan cleanup follows the foundation's retention window: a `PENDING` or
//! `UPLOADED` row that is older than `ORPHAN_SECONDS` and attached to nothing is
//! erased. A `READY` row is never orphaned by age: it is a published photograph
//! until its post is deleted, and the reference check refuses to erase media that
//! a live post still uses.
use crate::adapters::out::media::codec::BoundedImageSanitizer;
use concert_application::commands::CommandError;
use concert_application::media::{
    ImageSanitizer, InitUpload, Mime, Purpose, MAX_BYTES, MAX_DIMENSION, MAX_PIXELS, ORPHAN_SECONDS,
    UPLOAD_SECONDS,
};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

/// The closed error set of a profile media operation.
///
/// The HTTP projection is fixed by [`ProfileMediaError::status`] and
/// [`ProfileMediaError::reason`], so the route and the producer cannot disagree
/// about a status.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileMediaError {
    /// The request is not a well-formed upload command.
    Invalid,
    /// The purpose is not admitted in this context (purpose/context mismatch).
    PurposeMismatch,
    /// The bytes are not one of JPEG/PNG/WebP, or are not a single frame.
    UnsupportedMedia,
    /// The bytes exceed the foundation's byte limit.
    TooLarge,
    /// The declared bytes do not match the sent bytes.
    BytesMismatch,
    /// The asset exists but this caller may not see it, or it does not exist.
    /// Deliberately one variant: a caller must not learn which.
    Unavailable,
    /// The asset is not in the state the command needs.
    InvalidState,
    /// The caller is not the owner of the asset.
    Denied,
    /// The asset has expired or was erased.
    Expired,
    /// The store or the sanitizer is unavailable.
    Unavailable503,
}

impl ProfileMediaError {
    /// The HTTP status this error projects to.
    pub fn status(self) -> u16 {
        match self {
            Self::Invalid | Self::PurposeMismatch | Self::BytesMismatch => 400,
            Self::UnsupportedMedia => 415,
            Self::TooLarge => 413,
            Self::Denied => 403,
            Self::Unavailable | Self::Expired => 404,
            Self::InvalidState => 409,
            Self::Unavailable503 => 503,
        }
    }
    /// The frozen `details.reason` string.
    ///
    /// Every value here is in the operation register's closed error set for the
    /// profile media operations (`contracts/domain/v1/operations.json`). A
    /// purpose/context mismatch and a byte mismatch are both *client input*
    /// errors, so both project to `INVALID_INPUT`; a reason the contract does not
    /// declare would be dropped by the gateway's closed envelope and by the
    /// client's error decoder, turning a precise refusal into an opaque one.
    pub fn reason(self) -> &'static str {
        match self {
            Self::Invalid | Self::PurposeMismatch | Self::BytesMismatch => "INVALID_INPUT",
            Self::UnsupportedMedia => "INVALID_MEDIA_TYPE",
            Self::TooLarge => "PAYLOAD_TOO_LARGE",
            Self::Denied => "PERMISSION_DENIED",
            Self::Unavailable | Self::Expired => "RESOURCE_UNAVAILABLE",
            Self::InvalidState => "INVALID_TRANSITION",
            Self::Unavailable503 => "DEPENDENCY_UNAVAILABLE",
        }
    }
}

impl std::fmt::Display for ProfileMediaError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "profile media error: {}", self.reason())
    }
}

impl std::error::Error for ProfileMediaError {}

impl From<CommandError> for ProfileMediaError {
    fn from(error: CommandError) -> Self {
        match error {
            CommandError::InvalidInput => Self::Invalid,
            CommandError::Denied => Self::Denied,
            CommandError::Unavailable | CommandError::Capacity => Self::Unavailable503,
            CommandError::Conflict => Self::InvalidState,
            CommandError::Expired => Self::Expired,
            CommandError::InvalidState => Self::InvalidState,
        }
    }
}

fn store(_: sqlx::Error) -> ProfileMediaError {
    ProfileMediaError::Unavailable503
}

fn decode(_: sqlx::Error) -> ProfileMediaError {
    ProfileMediaError::InvalidState
}

/// The purpose a given upload *context* admits.
///
/// This is the purpose/context rule in one type. A context is not a free-form
/// string: each variant names exactly the purposes it may create, and
/// [`UploadPurpose::admits`] is the only way to ask. The profile-post context
/// deliberately cannot create an `INTRO_POST` or a `ROOM_POSTER`, and the room
/// context cannot create a `PROFILE_POST`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UploadPurpose {
    /// The profile-post composer: ordinary photographs and artwork.
    ProfilePost,
    /// The profile image (avatar photograph) slot.
    ProfileImage,
    /// The mutuals-intro composer.
    IntroPost,
    /// The room poster importer. Poster *conversion* happens in the browser;
    /// this context only stores the resulting PNG.
    RoomPoster,
}

impl UploadPurpose {
    /// The purposes this context admits, in contract order.
    pub fn admitted(self) -> &'static [Purpose] {
        match self {
            Self::ProfilePost => &[Purpose::ProfilePost],
            Self::ProfileImage => &[Purpose::ProfileImage],
            Self::IntroPost => &[Purpose::IntroPost],
            Self::RoomPoster => &[Purpose::RoomPoster],
        }
    }
    /// The single purpose this context stores.
    pub fn purpose(self) -> Purpose {
        self.admitted()[0]
    }
    /// Whether this context may create an asset with `purpose`.
    ///
    /// A purpose from the other scope (a concert purpose) is never admitted.
    pub fn admits(self, purpose: Purpose) -> bool {
        purpose.is_profile() && self.admitted().contains(&purpose)
    }
    /// Resolve the context from a requested purpose. A purpose that no context
    /// admits is a mismatch, never a default.
    pub fn for_purpose(purpose: Purpose) -> Option<Self> {
        match purpose {
            Purpose::ProfilePost => Some(Self::ProfilePost),
            Purpose::ProfileImage => Some(Self::ProfileImage),
            Purpose::IntroPost => Some(Self::IntroPost),
            Purpose::RoomPoster => Some(Self::RoomPoster),
            _ => None,
        }
    }
}

/// The canonical profile media id. A nil UUID is never an asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct ProfileMediaId(Uuid);

impl ProfileMediaId {
    pub fn from_uuid(value: Uuid) -> Result<Self, ProfileMediaError> {
        if value.is_nil() {
            return Err(ProfileMediaError::Invalid);
        }
        Ok(Self(value))
    }
    pub fn uuid(self) -> Uuid {
        self.0
    }
}

/// The committed state of one profile media asset.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileMediaState {
    Pending,
    Uploaded,
    Ready,
    Deleted,
}

impl ProfileMediaState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "PENDING",
            Self::Uploaded => "UPLOADED",
            Self::Ready => "READY",
            Self::Deleted => "DELETED",
        }
    }
    pub fn parse(value: &str) -> Result<Self, ProfileMediaError> {
        match value {
            "PENDING" => Ok(Self::Pending),
            "UPLOADED" => Ok(Self::Uploaded),
            "READY" => Ok(Self::Ready),
            "DELETED" => Ok(Self::Deleted),
            _ => Err(ProfileMediaError::InvalidState),
        }
    }
}

/// The `feed.UploadIntent` projection of a freshly created asset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileMediaIntent {
    pub media_id: Uuid,
    pub revision: i64,
    pub expires_at: chrono::DateTime<chrono::Utc>,
}

impl ProfileMediaIntent {
    /// The frozen profile upload path. It has no event segment.
    pub fn upload_path(&self) -> String {
        format!("/api/me/media/{}/bytes", self.media_id)
    }
    pub fn to_wire(&self) -> serde_json::Value {
        serde_json::json!({
            "media_id": self.media_id,
            "state": "PENDING",
            "upload_path": self.upload_path(),
            "max_bytes": MAX_BYTES,
            "expires_at": self.expires_at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            "upload_method": "PUT",
            "revision": self.revision,
        })
    }
}

/// The `feed.Media` projection of a `READY` asset.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileMediaUpload {
    pub media_id: Uuid,
    pub purpose: Purpose,
    pub mime: Mime,
    pub width: u32,
    pub height: u32,
    pub revision: i64,
}

impl ProfileMediaUpload {
    /// The frozen profile read path. It has no event segment.
    pub fn read_path(&self) -> String {
        super::authorization::media_read_path(self.media_id)
    }
    pub fn to_wire(&self) -> serde_json::Value {
        serde_json::json!({
            "media_id": self.media_id,
            "state": "READY",
            "revision": self.revision,
            "width": self.width,
            "height": self.height,
            "content_type": self.mime.as_str(),
            "purpose": self.purpose.as_str(),
            "read_path": self.read_path(),
        })
    }
}

/// A stored row, as the state machine needs it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Stored {
    owner: Uuid,
    purpose: Purpose,
    mime: Mime,
    state: ProfileMediaState,
    revision: i64,
    expired: bool,
}

async fn stored_in(
    connection: &mut sqlx::PgConnection,
    id: Uuid,
    lock: bool,
) -> Result<Option<Stored>, ProfileMediaError> {
    const LOOKUP: &str = "select owner_profile_id,purpose,content_type,state,revision,\
                          created_at + make_interval(secs => $2) <= clock_timestamp() expired \
                          from public.profile_media where media_id=$1";
    let query = if lock {
        format!("{LOOKUP} for update")
    } else {
        LOOKUP.into()
    };
    let row = sqlx::query(&query)
        .bind(id)
        .bind(UPLOAD_SECONDS as f64)
        .fetch_optional(connection)
        .await
        .map_err(store)?;
    row.map(|row| {
        Ok(Stored {
            owner: row.try_get("owner_profile_id").map_err(decode)?,
            purpose: Purpose::parse(&row.try_get::<String, _>("purpose").map_err(decode)?)
                .map_err(|_| ProfileMediaError::InvalidState)?,
            mime: Mime::parse(&row.try_get::<String, _>("content_type").map_err(decode)?)
                .map_err(|_| ProfileMediaError::InvalidState)?,
            state: ProfileMediaState::parse(&row.try_get::<String, _>("state").map_err(decode)?)?,
            revision: row.try_get("revision").map_err(decode)?,
            expired: row.try_get("expired").map_err(decode)?,
        })
    })
    .transpose()
}

/// The profile media store.
///
/// One pool, one sanitizer. There is no event, no participant and no policy
/// fence: the owner is the authenticated profile the caller passes in, and it is
/// never read from a request body.
#[derive(Clone)]
pub struct ProfileMediaStore {
    pool: PgPool,
    sanitizer: Arc<dyn ImageSanitizer>,
    /// The visibility decision used to authorize a read through the attached
    /// post. `None` keeps the fail-closed default, under which only the owner may
    /// read an asset.
    visibility: Option<crate::intro_posts::visibility::VisibilityPolicy>,
}

impl ProfileMediaStore {
    pub fn new(pool: PgPool, sanitizer: Arc<dyn ImageSanitizer>) -> Self {
        Self {
            pool,
            sanitizer,
            visibility: None,
        }
    }

    /// Attach the real visibility policy, so a media read is authorized through
    /// the post the asset is attached to (T-307).
    pub fn with_visibility(
        mut self,
        visibility: crate::intro_posts::visibility::VisibilityPolicy,
    ) -> Self {
        self.visibility = Some(visibility);
        self
    }

    /// The production sanitizer: the same bounded codec the concert media runtime
    /// uses, so a profile photograph and a concert photograph are normalized by
    /// identical code.
    pub fn with_bounded_codec(
        pool: PgPool,
        admission: platform::cpu::AdmissionClass,
    ) -> Self {
        Self::new(pool, Arc::new(BoundedImageSanitizer::new(admission)))
    }

    /// The pool this store writes to. Used by the post commands so a post and its
    /// attachment check see the same database.
    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    /// Step 1: create a `PENDING` asset.
    ///
    /// `context` is the upload context. A requested purpose the context does not
    /// admit is [`ProfileMediaError::PurposeMismatch`] and writes nothing. A
    /// purpose from the concert scope is never admitted here.
    pub async fn init(
        &self,
        owner: Uuid,
        context: UploadPurpose,
        input: InitUpload,
    ) -> Result<ProfileMediaIntent, ProfileMediaError> {
        if owner.is_nil() {
            return Err(ProfileMediaError::Denied);
        }
        // The purpose/context rule is checked first, so a `ROOM_POSTER` request
        // on the profile-post path is a purpose mismatch even when its declared
        // size is also wrong.
        if !context.admits(input.purpose) {
            return Err(ProfileMediaError::PurposeMismatch);
        }
        // A byte length above the foundation limit is refused as a size problem,
        // not as a generic invalid input: the client must be able to tell the
        // difference between "too big" and "malformed".
        if input.byte_length == 0 {
            return Err(ProfileMediaError::Invalid);
        }
        if input.byte_length > MAX_BYTES {
            return Err(ProfileMediaError::TooLarge);
        }
        input.validate().map_err(|_| ProfileMediaError::Invalid)?;
        let mut tx = self.pool.begin().await.map_err(store)?;
        // The per-owner cap is the same shape the concert media producer uses:
        // bounded retained assets so one account cannot fill the table.
        let retained: i64 = sqlx::query_scalar(
            "select count(*) from public.profile_media \
             where owner_profile_id=$1 and state<>'DELETED'",
        )
        .bind(owner)
        .fetch_one(&mut *tx)
        .await
        .map_err(store)?;
        if retained >= 512 {
            return Err(ProfileMediaError::Unavailable503);
        }
        let id = Uuid::new_v4();
        sqlx::query(
            "insert into public.profile_media(media_id,owner_profile_id,purpose,content_type,\
             state,declared_bytes,input_sha256) values($1,$2,$3,$4,'PENDING',$5,$6)",
        )
        .bind(id)
        .bind(owner)
        .bind(input.purpose.as_str())
        .bind(input.mime.as_str())
        .bind(input.byte_length as i32)
        .bind(input.sha256.as_slice())
        .execute(&mut *tx)
        .await
        .map_err(store)?;
        let row = sqlx::query(
            "select revision, created_at + make_interval(secs => $2) expires_at \
             from public.profile_media where media_id=$1",
        )
        .bind(id)
        .bind(UPLOAD_SECONDS as f64)
        .fetch_one(&mut *tx)
        .await
        .map_err(store)?;
        let intent = ProfileMediaIntent {
            media_id: id,
            revision: row.try_get("revision").map_err(decode)?,
            expires_at: row.try_get("expires_at").map_err(decode)?,
        };
        tx.commit().await.map_err(store)?;
        Ok(intent)
    }

    /// Step 2: stage the bytes.
    ///
    /// The bytes must be exactly the declared length and exactly the declared
    /// SHA-256, and the content type must be the declared one. Anything else is
    /// refused and the row stays `PENDING`.
    pub async fn put(
        &self,
        owner: Uuid,
        context: UploadPurpose,
        id: ProfileMediaId,
        mime: Mime,
        bytes: Vec<u8>,
    ) -> Result<i64, ProfileMediaError> {
        if owner.is_nil() {
            return Err(ProfileMediaError::Denied);
        }
        if bytes.is_empty() {
            return Err(ProfileMediaError::Invalid);
        }
        if bytes.len() > MAX_BYTES {
            return Err(ProfileMediaError::TooLarge);
        }
        let mut tx = self.pool.begin().await.map_err(store)?;
        let Some(value) = stored_in(&mut tx, id.uuid(), true).await? else {
            return Err(ProfileMediaError::Unavailable);
        };
        // Ownership first: a foreign asset is indistinguishable from an absent
        // one, so a caller cannot probe for another profile's media ids.
        if value.owner != owner {
            return Err(ProfileMediaError::Unavailable);
        }
        if !context.admits(value.purpose) {
            return Err(ProfileMediaError::PurposeMismatch);
        }
        if value.state == ProfileMediaState::Deleted || value.expired {
            return Err(ProfileMediaError::Expired);
        }
        if value.state != ProfileMediaState::Pending {
            return Err(ProfileMediaError::InvalidState);
        }
        let (declared, digest): (i32, Vec<u8>) = sqlx::query_as(
            "select declared_bytes, input_sha256 from public.profile_media where media_id=$1",
        )
        .bind(id.uuid())
        .fetch_one(&mut *tx)
        .await
        .map_err(store)?;
        if mime != value.mime || bytes.len() != declared as usize {
            return Err(ProfileMediaError::BytesMismatch);
        }
        if Sha256::digest(&bytes).as_slice() != digest.as_slice() {
            return Err(ProfileMediaError::BytesMismatch);
        }
        sqlx::query("update public.profile_media set sanitized=$2, state='UPLOADED' where media_id=$1")
            .bind(id.uuid())
            .bind(&bytes)
            .execute(&mut *tx)
            .await
            .map_err(store)?;
        let revision: i64 =
            sqlx::query_scalar("select revision from public.profile_media where media_id=$1")
                .bind(id.uuid())
                .fetch_one(&mut *tx)
                .await
                .map_err(store)?;
        tx.commit().await.map_err(store)?;
        Ok(revision)
    }

    /// Step 3: sanitize and publish.
    ///
    /// The sanitizer verifies the detected format matches the declared one,
    /// refuses an animation, fully decodes under the foundation's dimension and
    /// pixel limits, and re-encodes without metadata. The stored bytes are that
    /// re-encoded frame — the normalized photograph. **No pixel conversion is
    /// applied**: the width and height are the decoded source dimensions and the
    /// pixels are the decoded source pixels.
    pub async fn finalize(
        &self,
        owner: Uuid,
        context: UploadPurpose,
        id: ProfileMediaId,
        expected_revision: i64,
    ) -> Result<ProfileMediaUpload, ProfileMediaError> {
        if owner.is_nil() {
            return Err(ProfileMediaError::Denied);
        }
        if !(1..=9007199254740991).contains(&expected_revision) {
            return Err(ProfileMediaError::Invalid);
        }
        // Read the staged bytes without holding a transaction open across the
        // sanitizer: decode is bounded CPU work, not SQL.
        let mut tx = self.pool.begin().await.map_err(store)?;
        let Some(value) = stored_in(&mut tx, id.uuid(), false).await? else {
            return Err(ProfileMediaError::Unavailable);
        };
        if value.owner != owner {
            return Err(ProfileMediaError::Unavailable);
        }
        if !context.admits(value.purpose) {
            return Err(ProfileMediaError::PurposeMismatch);
        }
        if value.state == ProfileMediaState::Deleted || value.expired {
            return Err(ProfileMediaError::Expired);
        }
        if value.state != ProfileMediaState::Uploaded {
            return Err(ProfileMediaError::InvalidState);
        }
        if value.revision != expected_revision {
            return Err(ProfileMediaError::InvalidState);
        }
        let staged: Option<Vec<u8>> =
            sqlx::query_scalar("select sanitized from public.profile_media where media_id=$1")
                .bind(id.uuid())
                .fetch_one(&mut *tx)
                .await
                .map_err(store)?;
        tx.rollback().await.map_err(store)?;
        let staged = staged.ok_or(ProfileMediaError::InvalidState)?;
        let clean = self
            .sanitizer
            .sanitize(staged, value.mime)
            .await
            .map_err(ProfileMediaError::from)?;
        if clean.mime != value.mime {
            return Err(ProfileMediaError::UnsupportedMedia);
        }
        if clean.bytes.is_empty() || clean.bytes.len() > MAX_BYTES {
            return Err(ProfileMediaError::TooLarge);
        }
        if clean.width == 0
            || clean.height == 0
            || clean.width > MAX_DIMENSION
            || clean.height > MAX_DIMENSION
            || u64::from(clean.width) * u64::from(clean.height) > MAX_PIXELS
        {
            return Err(ProfileMediaError::UnsupportedMedia);
        }
        // Decoding is not permission. Re-read and re-check inside the writing
        // transaction: the row could have been erased or re-pointed meanwhile.
        let mut tx = self.pool.begin().await.map_err(store)?;
        let Some(current) = stored_in(&mut tx, id.uuid(), true).await? else {
            return Err(ProfileMediaError::Expired);
        };
        if current.owner != owner
            || current.purpose != value.purpose
            || current.state != ProfileMediaState::Uploaded
            || current.revision != value.revision
            || current.expired
        {
            return Err(ProfileMediaError::InvalidState);
        }
        sqlx::query(
            "update public.profile_media set sanitized=$2, width=$3, height=$4, state='READY' \
             where media_id=$1",
        )
        .bind(id.uuid())
        .bind(&clean.bytes)
        .bind(clean.width as i32)
        .bind(clean.height as i32)
        .execute(&mut *tx)
        .await
        .map_err(store)?;
        let revision: i64 =
            sqlx::query_scalar("select revision from public.profile_media where media_id=$1")
                .bind(id.uuid())
                .fetch_one(&mut *tx)
                .await
                .map_err(store)?;
        tx.commit().await.map_err(store)?;
        Ok(ProfileMediaUpload {
            media_id: id.uuid(),
            purpose: value.purpose,
            mime: clean.mime,
            width: clean.width,
            height: clean.height,
            revision,
        })
    }

    /// The metadata of a `READY` asset, for the owner only.
    ///
    /// A caller that is not the owner gets [`ProfileMediaError::Unavailable`],
    /// which is the same answer an absent id gets.
    /// Authorize one media read (T-307, E-03 metadata and E-04 bytes).
    ///
    /// The asset is authorized **through the post it is attached to**, never
    /// through file ownership alone:
    ///
    /// * the owner always sees their own asset;
    /// * a non-owner sees it only when a live attachment is visible to them, and
    ///   an intro attachment re-evaluates that post's audience on every request;
    /// * an asset attached to nothing is readable by its owner only.
    ///
    /// Every refusal is `Unavailable`, so a caller cannot tell "not yours" from
    /// "does not exist".
    async fn authorize_read(
        &self,
        viewer: Uuid,
        media_id: Uuid,
    ) -> Result<(), ProfileMediaError> {
        use crate::profile_media::authorization::{can_view_media, MediaViewer};
        let now = chrono::Utc::now();
        // The fail-closed default policy denies every restricted intro, so an
        // unconfigured store still never widens visibility.
        let policy = self
            .visibility
            .clone()
            .unwrap_or_else(crate::intro_posts::visibility::VisibilityPolicy::deny_restricted);
        let facts = crate::profile_media::resolver::read_facts(
            &self.pool,
            &policy,
            media_id,
            viewer,
            now,
        )
        .await
        .map_err(store)?;
        let Some(facts) = facts else {
            return Err(ProfileMediaError::Unavailable);
        };
        if can_view_media(MediaViewer { profile_id: viewer }, &facts) {
            Ok(())
        } else {
            Err(ProfileMediaError::Unavailable)
        }
    }

    pub async fn metadata(
        &self,
        owner: Uuid,
        id: ProfileMediaId,
    ) -> Result<ProfileMediaUpload, ProfileMediaError> {
        let row = sqlx::query(
            "select owner_profile_id,purpose,content_type,width,height,revision,state \
             from public.profile_media where media_id=$1",
        )
        .bind(id.uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(store)?;
        let Some(row) = row else {
            return Err(ProfileMediaError::Unavailable);
        };
        self.authorize_read(owner, id.uuid()).await?;
        let state = ProfileMediaState::parse(&row.try_get::<String, _>("state").map_err(decode)?)?;
        if state != ProfileMediaState::Ready {
            return Err(ProfileMediaError::Unavailable);
        }
        Ok(ProfileMediaUpload {
            media_id: id.uuid(),
            purpose: Purpose::parse(&row.try_get::<String, _>("purpose").map_err(decode)?)
                .map_err(|_| ProfileMediaError::InvalidState)?,
            mime: Mime::parse(&row.try_get::<String, _>("content_type").map_err(decode)?)
                .map_err(|_| ProfileMediaError::InvalidState)?,
            width: row.try_get::<i32, _>("width").map_err(decode)? as u32,
            height: row.try_get::<i32, _>("height").map_err(decode)? as u32,
            revision: row.try_get("revision").map_err(decode)?,
        })
    }

    /// The sanitized bytes of a `READY` asset plus its content type.
    ///
    /// Authorization is the caller's: the route layer decides whether the viewer
    /// may see this asset (see [`super::authorization`]) and only then calls
    /// this. The owner check here is a second, independent guard.
    pub async fn content(
        &self,
        owner: Uuid,
        id: ProfileMediaId,
    ) -> Result<(Mime, Vec<u8>), ProfileMediaError> {
        let row = sqlx::query(
            "select owner_profile_id,content_type,sanitized,state \
             from public.profile_media where media_id=$1",
        )
        .bind(id.uuid())
        .fetch_optional(&self.pool)
        .await
        .map_err(store)?;
        let Some(row) = row else {
            return Err(ProfileMediaError::Unavailable);
        };
        self.authorize_read(owner, id.uuid()).await?;
        let state = ProfileMediaState::parse(&row.try_get::<String, _>("state").map_err(decode)?)?;
        if state != ProfileMediaState::Ready {
            return Err(ProfileMediaError::Unavailable);
        }
        let mime = Mime::parse(&row.try_get::<String, _>("content_type").map_err(decode)?)
            .map_err(|_| ProfileMediaError::InvalidState)?;
        let bytes: Option<Vec<u8>> = row.try_get("sanitized").map_err(decode)?;
        Ok((mime, bytes.ok_or(ProfileMediaError::Unavailable)?))
    }
}

/// Erase abandoned uploads.
///
/// A `PENDING` or `UPLOADED` row older than the foundation's orphan window
/// (`ORPHAN_SECONDS`, 24 hours) that no post references is erased. A `READY` row
/// is never erased by age: it is a published photograph until its post is
/// deleted, and `profile_media_erase_unreferenced` refuses to erase a file a live
/// post still uses.
///
/// `limit` is bounded so a scheduled run cannot hold one transaction over the
/// whole table. This is maintenance, not a browser route.
///
/// The candidate set is ordered oldest-first and the limit applies to that
/// order, so a bounded run always makes progress on the oldest orphans rather
/// than starving them behind newer ones.
pub async fn cleanup_orphans(
    pool: &PgPool,
    limit: i64,
) -> Result<u64, ProfileMediaError> {
    if !(1..=100).contains(&limit) {
        return Err(ProfileMediaError::Invalid);
    }
    let erased = sqlx::query(
        "with abandoned as ( \
             select media_id from public.profile_media m \
             where m.state in ('PENDING','UPLOADED') \
               and m.created_at + make_interval(secs => $2) <= clock_timestamp() \
               and not exists (select 1 from public.profile_post_attachments a \
                               where a.media_id = m.media_id) \
               and not exists (select 1 from public.intro_attachments a \
                               where a.media_id = m.media_id) \
             order by m.created_at, m.media_id \
             limit $1) \
         update public.profile_media m \
            set state='DELETED', sanitized=null, width=null, height=null \
           from abandoned a where m.media_id = a.media_id",
    )
    .bind(limit)
    .bind(ORPHAN_SECONDS as f64)
    .execute(pool)
    .await
    .map_err(store)?
    .rows_affected();
    Ok(erased)
}

/// Erase exactly the abandoned uploads this owner created.
///
/// This is the per-owner form of [`cleanup_orphans`]: same window, same
/// reference checks, plus the owner. It exists so a test can prove the orphan
/// rule for one account without a bounded maintenance run touching another
/// test's rows.
pub async fn cleanup_owner_orphans(
    pool: &PgPool,
    owner: Uuid,
    limit: i64,
) -> Result<u64, ProfileMediaError> {
    if owner.is_nil() || !(1..=100).contains(&limit) {
        return Err(ProfileMediaError::Invalid);
    }
    let erased = sqlx::query(
        "with abandoned as ( \
             select media_id from public.profile_media m \
             where m.owner_profile_id = $3 \
               and m.state in ('PENDING','UPLOADED') \
               and m.created_at + make_interval(secs => $2) <= clock_timestamp() \
               and not exists (select 1 from public.profile_post_attachments a \
                               where a.media_id = m.media_id) \
               and not exists (select 1 from public.intro_attachments a \
                               where a.media_id = m.media_id) \
             order by m.created_at, m.media_id \
             limit $1) \
         update public.profile_media m \
            set state='DELETED', sanitized=null, width=null, height=null \
           from abandoned a where m.media_id = a.media_id",
    )
    .bind(limit)
    .bind(ORPHAN_SECONDS as f64)
    .bind(owner)
    .execute(pool)
    .await
    .map_err(store)?
    .rows_affected();
    Ok(erased)
}
