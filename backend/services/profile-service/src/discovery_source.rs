//! The profile service's internal discovery-source read.
//!
//! Discovery lives in the concert service, but the facts it mirrors are
//! profile-owned and live in `mcr_profile`. `domain-contract.md` §1.1 forbids a
//! cross-database join, so the concert service's projection writer asks this
//! service for the facts instead of reading its tables.
//!
//! This is a **pull** read for a bounded reconciliation job. It answers one
//! question — *which profiles changed after this cursor?* — in a stable order so
//! the caller can advance a watermark without skipping or repeating a row.
//!
//! Three rules the read obeys:
//!
//! * **No fabricated facts.** A column that is null in the source is returned
//!   absent. `profile_identities` has no `interaction_style` column at all, so
//!   that field is always absent here; the projection then stores null, and the
//!   discovery card omits the line rather than inventing one.
//! * **Tombstones are reported.** A soft-deleted profile is returned with
//!   `deleted = true` rather than silently vanishing, because a silent omission
//!   would leave a stale projection row behind forever.
//! * **No score, rank or count.** The read has nothing to copy, which is the
//!   simplest guarantee that the projection cannot acquire a column the
//!   discovery query forbids.
use chrono::{DateTime, Utc};
use platform::grpc_method_policy::MethodPolicy;
use platform::grpc_auth::{VerifiedCaller, Workload};
use sqlx::{PgPool, Row};
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("profile.discovery_source.v1");
}

/// The server-side page cap. A caller asking for more receives this.
pub const MAX_PAGE: i32 = 100;
/// The default page size when the caller names none.
pub const DEFAULT_PAGE: i32 = 50;

/// Only the concert service may read the discovery source, and only as itself.
///
/// This is an internal service-to-service read: it carries no actor delegation
/// because it acts for no user. It is not a product route and is never mounted
/// on the Gateway.
pub static DISCOVERY_SOURCE_POLICY: MethodPolicy = MethodPolicy {
    service: "profile.discovery_source.v1.DiscoverySourceService",
    workload: Some(Workload::Profile),
    methods: &[("ReadChanged", &[Workload::Concert])],
    actor_fields: &[],
};

/// One profile's discovery-relevant facts, as the store found them.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceProfile {
    pub profile_id: Uuid,
    pub account_id: Uuid,
    pub nickname: String,
    pub bio: Option<String>,
    pub profile_image_media_id: Option<Uuid>,
    /// Always `None` today: `profile_identities` has no such column. It is
    /// carried so a future column needs no proto change, and it is never
    /// defaulted to a string.
    pub interaction_style: Option<String>,
    pub visibility: String,
    pub favorites: Vec<SourceFavorite>,
    pub updated_at: DateTime<Utc>,
    pub deleted: bool,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceFavorite {
    pub category: String,
    pub catalog_item_id: Uuid,
    pub label: String,
    pub position: i16,
}

/// One page of the read.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcePage {
    pub profiles: Vec<SourceProfile>,
    pub next: Option<(DateTime<Utc>, Uuid)>,
    pub more: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceError {
    /// The caller's cursor was malformed.
    InvalidCursor,
    /// The store is unavailable.
    Unavailable,
}

impl std::fmt::Display for SourceError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "discovery source error: {self:?}")
    }
}

impl std::error::Error for SourceError {}

impl From<sqlx::Error> for SourceError {
    fn from(_: sqlx::Error) -> Self {
        Self::Unavailable
    }
}

/// The read itself. It is a function over a pool so the server and the tests
/// share one implementation.
///
/// The ordering is `(updated_at, profile_id)`, which is what makes the watermark
/// safe: `updated_at` is rewritten by the identity trigger on every update, so a
/// row that changes after the caller read it moves to the end of the stream and
/// is picked up by the next pass rather than being skipped.
pub async fn read_changed(
    pool: &PgPool,
    after: Option<(DateTime<Utc>, Uuid)>,
    limit: i32,
) -> Result<SourcePage, SourceError> {
    let limit = limit.clamp(1, MAX_PAGE);
    let (after_at, after_id) = match after {
        Some((at, id)) => {
            if id.is_nil() {
                return Err(SourceError::InvalidCursor);
            }
            (Some(at), Some(id))
        }
        None => (None, None),
    };
    // One row per (profile, favorite). The page is cut by profile, so the query
    // fetches one profile more than asked to prove whether more exist.
    //
    // The watermark column is `profile_identities.updated_at`, and migration
    // `0017_profile_favorites_touch` is what makes that correct: a favorites
    // insert *or delete* bumps the owning profile's revision, which the identity
    // trigger turns into a fresh `updated_at`. Without that trigger a removal
    // would leave no trace and the projection would silently stop tracking the
    // profile.
    let rows = sqlx::query(
        "with changed as ( \
             select i.profile_id, i.account_id, i.nickname, i.bio, i.profile_image_media_id, \
                    i.visibility, i.deleted_at, i.updated_at \
             from profile_identities i \
             where ($1::timestamptz is null or (i.updated_at, i.profile_id) > ($1, $2)) \
             order by i.updated_at, i.profile_id \
             limit $3 \
         ) \
         select p.profile_id, p.account_id, p.nickname, p.bio, p.profile_image_media_id, \
                p.visibility, p.updated_at, p.deleted_at, \
                f.category, f.catalog_item_id, c.label, f.position \
         from changed p \
         left join profile_favorites f on f.profile_id = p.profile_id \
         left join catalog_items c on c.catalog_item_id = f.catalog_item_id \
         order by p.updated_at, p.profile_id, f.category, f.position",
    )
    .bind(after_at)
    .bind(after_id)
    .bind(limit as i64 + 1)
    .fetch_all(pool)
    .await?;

    let mut profiles: Vec<SourceProfile> = Vec::new();
    for row in rows {
        let profile_id: Uuid = row.try_get("profile_id")?;
        let updated_at: DateTime<Utc> = row.try_get("updated_at")?;
        let deleted_at: Option<DateTime<Utc>> = row.try_get("deleted_at")?;
        // A favorite whose catalog row is missing has no label. That is a real
        // inconsistency in the source, and the projection must not invent a
        // label for it — but it must still project the *profile*. So the
        // unlabelled favorite is dropped and the profile is kept; `continue`
        // here would skip the whole profile row along with it.
        let favorite = match row.try_get::<Option<String>, _>("category")? {
            None => None,
            Some(category) => match (
                row.try_get::<Option<Uuid>, _>("catalog_item_id")?,
                row.try_get::<Option<String>, _>("label")?,
            ) {
                (Some(catalog_item_id), Some(label)) => Some(SourceFavorite {
                    category,
                    catalog_item_id,
                    label,
                    position: row.try_get("position")?,
                }),
                _ => None,
            },
        };
        match profiles.last_mut() {
            Some(profile) if profile.profile_id == profile_id => {
                if let Some(favorite) = favorite {
                    profile.favorites.push(favorite);
                }
            }
            _ => profiles.push(SourceProfile {
                profile_id,
                account_id: row.try_get("account_id")?,
                nickname: row.try_get("nickname")?,
                bio: row.try_get("bio")?,
                profile_image_media_id: row.try_get("profile_image_media_id")?,
                // No such column exists; never defaulted.
                interaction_style: None,
                visibility: row.try_get("visibility")?,
                favorites: favorite.into_iter().collect(),
                updated_at,
                deleted: deleted_at.is_some(),
            }),
        }
    }

    let more = profiles.len() as i32 > limit;
    if more {
        profiles.truncate(limit as usize);
    }
    let next = profiles
        .last()
        .map(|profile| (profile.updated_at, profile.profile_id));
    Ok(SourcePage {
        profiles,
        next,
        more,
    })
}

/// The gRPC server.
#[derive(Clone)]
pub struct DiscoverySourceServer {
    pool: PgPool,
}

impl DiscoverySourceServer {
    pub fn from_pool(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn canonical(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(|| Status::invalid_argument("Invalid discovery source cursor"))
}

#[tonic::async_trait]
impl proto::discovery_source_service_server::DiscoverySourceService for DiscoverySourceServer {
    async fn read_changed(
        &self,
        request: Request<proto::ReadChangedRequest>,
    ) -> Result<Response<proto::ReadChangedReply>, Status> {
        match request.extensions().get::<VerifiedCaller>() {
            Some(VerifiedCaller(Workload::Concert)) => {}
            Some(_) => {
                return Err(Status::permission_denied(
                    "Discovery source caller denied",
                ))
            }
            None => {
                return Err(Status::unauthenticated(
                    "Verified discovery source caller required",
                ))
            }
        }
        let message = request.into_inner();
        // Both cursor components are present or both are absent. A half-set
        // cursor is refused rather than defaulted, so a caller cannot silently
        // restart the stream from the beginning.
        let after = match (message.after_updated_at_unix_micros, message.after_profile_id) {
            (None, None) => None,
            (Some(micros), Some(id)) if !id.is_empty() => {
                let at = DateTime::from_timestamp_micros(micros)
                    .ok_or_else(|| Status::invalid_argument("Invalid discovery source cursor"))?;
                Some((at, canonical(&id)?))
            }
            _ => {
                return Err(Status::invalid_argument(
                    "Discovery source cursor must be complete",
                ))
            }
        };
        let page = read_changed(&self.pool, after, message.limit)
            .await
            .map_err(|error| match error {
                SourceError::InvalidCursor => {
                    Status::invalid_argument("Invalid discovery source cursor")
                }
                SourceError::Unavailable => {
                    Status::unavailable("Discovery source store unavailable")
                }
            })?;
        Ok(Response::new(proto::ReadChangedReply {
            profiles: page
                .profiles
                .into_iter()
                .map(|profile| proto::DiscoveryProfile {
                    profile_id: profile.profile_id.to_string(),
                    account_id: profile.account_id.to_string(),
                    nickname: profile.nickname,
                    bio: profile.bio,
                    profile_image_media_id: profile
                        .profile_image_media_id
                        .map(|id| id.to_string()),
                    interaction_style: profile.interaction_style,
                    visibility: profile.visibility,
                    favorites: profile
                        .favorites
                        .into_iter()
                        .map(|favorite| proto::DiscoveryFavorite {
                            category: favorite.category,
                            catalog_item_id: favorite.catalog_item_id.to_string(),
                            label: favorite.label,
                            position: favorite.position as i32,
                        })
                        .collect(),
                    updated_at_unix_micros: profile.updated_at.timestamp_micros(),
                    deleted: profile.deleted,
                })
                .collect(),
            next_updated_at_unix_micros: page.next.map(|(at, _)| at.timestamp_micros()),
            next_profile_id: page.next.map(|(_, id)| id.to_string()),
            more: page.more,
        }))
    }
}
