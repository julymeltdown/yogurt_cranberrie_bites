//! WP-08 T-702: profile-owned avatar persistence.
//!
//! This is the profile re-keying of
//! `adapters::out::repository::avatar`. What is kept: the revision discipline
//! (a save carries `expected_revision`, is checked inside the command
//! transaction, and a mismatch is a conflict that returns the current config),
//! the release-bound catalog projection, and the strict row decoder. What is
//! dropped: the event phase gate, the participant-admission gate, the effect
//! witness and the concert key. A profile avatar has no concert to be open.
//!
//! Ownership is decided here, server-side, from the authenticated profile id.
//! The write path never accepts an owner from a caller body: the only owner a
//! statement binds is the one this module was handed by the authenticated
//! request, so a non-owner write cannot be expressed, and the trigger refuses
//! re-pointing a stored avatar at another profile.
use concert_application::bootstrap::ValidatedProductRelease;
use concert_domain::profile_avatar::{
    config_value, parse_config, ProfileAvatarCatalog, ProfileId, SetProfileAvatar,
    MAX_REQUEST_BYTES,
};
use concert_domain::{AvatarSelection, Revision};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgRow, PgPool, Row};
use std::sync::Arc;

/// The closed error set of a profile avatar operation.
///
/// The HTTP projection is fixed: `Invalid`/`InvalidState` are `400`,
/// `Unauthenticated` is `401`, `Denied` is `403`, `Missing` is `404`,
/// `Conflict` is `409` with the current config, and `Unavailable` is `503`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileAvatarError {
    Invalid,
    Unauthenticated,
    Denied,
    Missing,
    Conflict,
    Unavailable,
    InvalidState,
}

/// The committed state of one profile avatar (`core.AvatarState`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileAvatarState {
    pub profile: ProfileId,
    pub revision: Revision,
    pub selection: AvatarSelection,
}

impl ProfileAvatarState {
    /// `{ "revision": n, "config": {...} }` — exactly the frozen wire shape.
    pub fn to_wire(&self) -> Value {
        json!({"revision": self.revision.get(), "config": config_value(&self.selection)})
    }
}

/// The authenticated actor of a profile avatar request.
///
/// `owner` is the profile the session belongs to; it is never read from the
/// request body. `actor` is the canonical Auth account backing that session and
/// is recorded only so the ownership check has two independent inputs.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileAvatarActor {
    pub owner: ProfileId,
}

impl ProfileAvatarActor {
    pub fn new(owner: ProfileId) -> Self {
        Self { owner }
    }
}

fn unavailable(_: sqlx::Error) -> ProfileAvatarError {
    ProfileAvatarError::Unavailable
}

fn invalid(_: impl std::fmt::Debug) -> ProfileAvatarError {
    ProfileAvatarError::InvalidState
}

/// Profile avatar persistence bound to one already validated C03/C06 release.
///
/// The catalog is projected from the release exactly as the concert avatar
/// projects it, including the content-hash rebinding, so a tampered release is
/// refused before it can admit a profile write.
pub struct ProfileAvatarPersistence {
    pool: PgPool,
    catalog: ProfileAvatarCatalog,
    /// The release default a profile that has never saved starts from.
    default: AvatarSelection,
    release_digest: String,
}

impl ProfileAvatarPersistence {
    /// Product construction. The release must be a fully validated product
    /// release; a synthetic metadata fixture is refused.
    pub fn from_product(
        pool: PgPool,
        release: Arc<ValidatedProductRelease>,
    ) -> Result<Self, ProfileAvatarError> {
        Self::build(pool, release, false)
    }

    /// Explicit metadata-test seam. Never used by product construction.
    pub fn from_synthetic_test(
        pool: PgPool,
        release: Arc<ValidatedProductRelease>,
        allow_synthetic: bool,
    ) -> Result<Self, ProfileAvatarError> {
        if !allow_synthetic {
            return Err(ProfileAvatarError::Unavailable);
        }
        Self::build(pool, release, true)
    }

    fn build(
        pool: PgPool,
        release: Arc<ValidatedProductRelease>,
        synthetic: bool,
    ) -> Result<Self, ProfileAvatarError> {
        let doc: Value =
            serde_json::from_slice(release.release().bootstrap_json()).map_err(invalid)?;
        let avatar = &doc["avatar_manifest"];
        // This is an already validated immutable C03/C06 release. Rebind the
        // exact avatar content hash before projecting its inventory.
        let mut unhashed = avatar.clone();
        unhashed
            .as_object_mut()
            .ok_or(ProfileAvatarError::Unavailable)?
            .remove("content_hash");
        let bytes = concert_domain::place::canonical_json_bytes(
            &serde_json::to_vec(&unhashed).map_err(invalid)?,
        )
        .map_err(invalid)?;
        let hash = format!("sha256:{:x}", Sha256::digest(bytes));
        if avatar["content_hash"].as_str() != Some(hash.as_str()) {
            return Err(ProfileAvatarError::Unavailable);
        }
        if !synthetic {
            let whole = avatar["representation"] == "whole_character";
            let motion = if avatar["contract_version"] == "2.0.0" {
                concert_domain::avatar_motion_v2::reviewed::VisuallyReviewedCatalog::from_manifest(
                    avatar,
                )
                .map_err(invalid)?;
                true
            } else {
                false
            };
            let mut resources = vec![&doc["map_manifest"]];
            if !whole && !motion {
                resources.push(avatar);
            }
            if whole
                && avatar["content_hash"]
                    != "sha256:0627c7d178b22535a561e1270db6d8bf9e2383ba84d03a63962e48e565c59b2b"
            {
                return Err(ProfileAvatarError::Unavailable);
            }
            resources.extend(
                doc["theme_manifests"]
                    .as_array()
                    .ok_or(ProfileAvatarError::Unavailable)?,
            );
            if resources.len() != if whole || motion { 4 } else { 5 }
                || resources.iter().any(|v| {
                    v["provenance"]["review_state"] != "REVIEWED"
                        || v["provenance"]["source_kind"] != "ORIGINAL_ART"
                })
                || (!motion && avatar["atlas"]["texture"]["image_state"] != "PROVIDED")
                || doc["map_manifest"]["tilemap"]["resource_state"] != "PROVIDED"
                || doc["theme_manifests"]
                    .as_array()
                    .unwrap()
                    .iter()
                    .any(|v| v["texture"]["image_state"] != "PROVIDED")
            {
                return Err(ProfileAvatarError::Unavailable);
            }
        }
        let catalog = ProfileAvatarCatalog::from_validated_manifest(avatar).map_err(invalid)?;
        let default = release.release().defaults().avatar.clone();
        catalog.validate(&default).map_err(invalid)?;
        Ok(Self {
            pool,
            catalog,
            default,
            release_digest: release.source_digest().to_owned(),
        })
    }

    pub fn release_digest(&self) -> &str {
        &self.release_digest
    }

    /// The manifest this persistence admits, for diagnostics and tests.
    pub fn manifest(&self) -> (&str, i64) {
        self.catalog.manifest()
    }

    /// Read the actor's own avatar.
    ///
    /// A profile that has never saved an avatar has no row yet. It is reported
    /// as the release default at revision 1 — the same starting point the
    /// concert avatar uses — without creating a row, so a read can never
    /// authorize or fabricate a write.
    pub async fn read(
        &self,
        actor: ProfileAvatarActor,
    ) -> Result<ProfileAvatarState, ProfileAvatarError> {
        let row = sqlx::query("select revision,config from public.profile_avatars where owner_profile_id=$1")
            .bind(actor.owner.uuid())
            .fetch_optional(&self.pool)
            .await
            .map_err(unavailable)?;
        let Some(row) = row else {
            return Ok(ProfileAvatarState {
                profile: actor.owner,
                revision: Revision::initial(),
                selection: self.default.clone(),
            });
        };
        let state = state_from_row(row, actor.owner)?;
        self.catalog
            .validate(&state.selection)
            .map_err(|_| ProfileAvatarError::InvalidState)?;
        Ok(state)
    }

    /// Set the actor's own avatar.
    ///
    /// `bytes` is the raw `core.SetAvatarRequest` body. The owner is the
    /// authenticated profile; there is no parameter through which a caller can
    /// name a different one. A revision mismatch is `Conflict` and leaves the
    /// stored row untouched; the caller re-reads to learn the current config.
    pub async fn set_avatar(
        &self,
        actor: ProfileAvatarActor,
        bytes: &[u8],
    ) -> Result<ProfileAvatarState, ProfileAvatarError> {
        if bytes.len() > MAX_REQUEST_BYTES {
            return Err(ProfileAvatarError::Invalid);
        }
        let command = SetProfileAvatar::parse(bytes).map_err(|_| ProfileAvatarError::Invalid)?;
        self.catalog
            .validate(&command.selection)
            .map_err(|_| ProfileAvatarError::Invalid)?;
        self.commit(actor, command.expected, command.selection).await
    }

    /// The release default, used only for a profile that has never saved.
    pub fn default_selection(&self) -> &AvatarSelection {
        &self.default
    }

    async fn commit(
        &self,
        actor: ProfileAvatarActor,
        expected: Revision,
        selection: AvatarSelection,
    ) -> Result<ProfileAvatarState, ProfileAvatarError> {
        let owner = actor.owner.uuid();
        let config = config_value(&selection);
        let next = expected.next().map_err(|_| ProfileAvatarError::Conflict)?;
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        // Lock the row if it exists. `for update` on a missing row locks nothing,
        // so the insert path below re-checks the revision through the primary
        // key instead of assuming the row is absent.
        let row = sqlx::query(
            "select revision,config from public.profile_avatars where owner_profile_id=$1 for update",
        )
        .bind(owner)
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?;
        let current = match row {
            Some(row) => Some(state_from_row(row, actor.owner)?),
            None => None,
        };
        let current_revision = current
            .as_ref()
            .map_or_else(Revision::initial, |state| state.revision);
        if current_revision != expected {
            // A conflict writes nothing. The transaction is rolled back so even
            // the read lock is released before the caller re-reads.
            tx.rollback().await.map_err(unavailable)?;
            return Err(ProfileAvatarError::Conflict);
        }
        if current.is_some() {
            sqlx::query(
                "update public.profile_avatars set config=$2, revision=$3 \
                 where owner_profile_id=$1 and revision=$4",
            )
            .bind(owner)
            .bind(&config)
            .bind(next.get())
            .bind(expected.get())
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        } else {
            // The first save creates the row at the next revision. The primary
            // key makes a concurrent first save fail rather than overwrite it.
            let inserted = sqlx::query(
                "insert into public.profile_avatars(owner_profile_id, config, revision) \
                 values($1,$2,$3) on conflict (owner_profile_id) do nothing",
            )
            .bind(owner)
            .bind(&config)
            .bind(next.get())
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
            if inserted.rows_affected() != 1 {
                // Another writer created the row between the select and the
                // insert. That writer won this revision; this one conflicts.
                tx.rollback().await.map_err(unavailable)?;
                return Err(ProfileAvatarError::Conflict);
            }
        }
        let committed = sqlx::query(
            "select revision,config from public.profile_avatars where owner_profile_id=$1",
        )
        .bind(owner)
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
        let state = state_from_row(committed, actor.owner)?;
        self.catalog
            .validate(&state.selection)
            .map_err(|_| ProfileAvatarError::InvalidState)?;
        if state.revision != next {
            return Err(ProfileAvatarError::InvalidState);
        }
        tx.commit().await.map_err(unavailable)?;
        Ok(state)
    }
}

/// Decode a stored row through the canonical strict parser.
///
/// A stored config that the current catalog no longer admits fails closed as
/// `InvalidState`; it is never silently replaced by a default or a fallback.
fn state_from_row(row: PgRow, owner: ProfileId) -> Result<ProfileAvatarState, ProfileAvatarError> {
    let revision = Revision::new(row.try_get("revision").map_err(invalid)?).map_err(invalid)?;
    let config: Value = row.try_get("config").map_err(invalid)?;
    let bytes = serde_json::to_vec(&config).map_err(invalid)?;
    let selection = parse_config(&bytes).map_err(|_| ProfileAvatarError::InvalidState)?;
    Ok(ProfileAvatarState {
        profile: owner,
        revision,
        selection,
    })
}

/// The HTTP projection of an error. Kept beside the error so the route and the
/// persistence cannot disagree about a status.
pub fn http_status(error: ProfileAvatarError) -> u16 {
    match error {
        ProfileAvatarError::Invalid | ProfileAvatarError::InvalidState => 400,
        ProfileAvatarError::Unauthenticated => 401,
        ProfileAvatarError::Denied => 403,
        ProfileAvatarError::Missing => 404,
        ProfileAvatarError::Conflict => 409,
        ProfileAvatarError::Unavailable => 503,
    }
}

/// The frozen reason string for an error (`details.reason`).
pub fn http_reason(error: ProfileAvatarError) -> &'static str {
    match error {
        ProfileAvatarError::Invalid => "INVALID_INPUT",
        ProfileAvatarError::InvalidState => "INVALID_STATE",
        ProfileAvatarError::Unauthenticated => "AUTHENTICATION_REQUIRED",
        ProfileAvatarError::Denied => "PERMISSION_DENIED",
        ProfileAvatarError::Missing => "RESOURCE_UNAVAILABLE",
        ProfileAvatarError::Conflict => "VERSION_CONFLICT",
        ProfileAvatarError::Unavailable => "DEPENDENCY_UNAVAILABLE",
    }
}
