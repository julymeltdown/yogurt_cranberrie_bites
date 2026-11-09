//! The discovery projection writer.
//!
//! Migration `0086` created `discovery_profiles`, `discovery_favorite_items` and
//! `discovery_blocks`, and `discovery::repository` reads them. Nothing wrote
//! them, so production discovery returned an empty list. This module is the
//! writer.
//!
//! ## The mechanism, and why
//!
//! A **bounded incremental reconciliation job**. The full reasoning is in
//! `docs/mcr/integration/discovery-projection.md`; the short version is that the
//! `discovery_*` tables are concert-owned, so the concert service must be their
//! only writer, and the projection has three sources written by three different
//! services (profile facts and favorites in the profile service, blocks in the
//! friend service). A pull read covers all three through one path; an outbox
//! would need three producers, and a synchronous call would make a profile
//! mutation depend on a derived read.
//!
//! ## Correctness properties
//!
//! * **Idempotent.** Every write is an upsert keyed by `profile_id`, and a
//!   profile's favorites are replaced wholesale inside the same transaction as
//!   its facts. Applying one page twice produces the same rows.
//! * **No lost updates.** The watermark advances only after a page is committed,
//!   and it advances to the cursor the *server returned*, never to a value this
//!   module computed. A crash mid-page re-reads the same page. A source change
//!   that lands after the read moves the row's `updated_at`, so the next pass
//!   sees it.
//! * **Deletions propagate.** A tombstone removes the projection rows.
//! * **No fabricated facts.** A source field that is absent is written absent.
//! * **No score column.** This module writes no numeric ranking field, and the
//!   discovery query's "no score column" test still passes.
//!
//! ## Failure behaviour
//!
//! If the profile service is unreachable the job writes nothing and records the
//! failure. It does **not** clear the projection: an empty projection is
//! indistinguishable from "nobody has favorites", which would be a fabricated
//! fact. A stale-but-consistent projection is the honest failure.
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use tonic::transport::Channel;
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("profile.discovery_source.v1");
}

/// The page size the job asks for. The server caps it independently.
pub const PAGE_SIZE: i32 = 100;
/// The most pages one pass applies before yielding, so a large backlog cannot
/// hold one pass open indefinitely. The next tick continues from the watermark.
pub const MAX_PAGES_PER_PASS: usize = 20;

/// One profile as the source reported it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourceProfile {
    pub profile_id: Uuid,
    pub account_id: Uuid,
    pub nickname: String,
    pub bio: Option<String>,
    pub profile_image_media_id: Option<Uuid>,
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

/// One page from the source.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SourcePage {
    pub profiles: Vec<SourceProfile>,
    pub next: Option<(DateTime<Utc>, Uuid)>,
    pub more: bool,
}

/// The source port. The production implementation is the profile service's
/// gRPC read; the tests use an in-memory implementation so the writer's
/// behaviour can be exercised without a second service.
#[async_trait::async_trait]
pub trait DiscoverySource: Send + Sync {
    /// Read the profiles that changed after `after`.
    async fn read_changed(
        &self,
        after: Option<(DateTime<Utc>, Uuid)>,
        limit: i32,
    ) -> Result<SourcePage, SourceFailure>;
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SourceFailure {
    /// The source is unreachable. The job must not treat this as "no changes".
    Unavailable,
    /// The source returned something this module cannot interpret.
    Invalid,
}

impl std::fmt::Display for SourceFailure {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "discovery source failure: {self:?}")
    }
}

impl std::error::Error for SourceFailure {}

/// The gRPC-backed source: the profile service's `DiscoverySourceService`.
pub struct GrpcDiscoverySource {
    client: proto::discovery_source_service_client::DiscoverySourceServiceClient<Channel>,
}

impl GrpcDiscoverySource {
    pub fn new(
        client: proto::discovery_source_service_client::DiscoverySourceServiceClient<Channel>,
    ) -> Self {
        Self { client }
    }
}

/// A canonical lowercase non-nil UUID, or `None`.
fn uuid(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
}

#[async_trait::async_trait]
impl DiscoverySource for GrpcDiscoverySource {
    async fn read_changed(
        &self,
        after: Option<(DateTime<Utc>, Uuid)>,
        limit: i32,
    ) -> Result<SourcePage, SourceFailure> {
        let request = proto::ReadChangedRequest {
            after_updated_at_unix_micros: after.map(|(at, _)| at.timestamp_micros()),
            after_profile_id: after.map(|(_, id)| id.to_string()),
            limit,
        };
        let reply = self
            .client
            .clone()
            .read_changed(request)
            .await
            .map_err(|status| {
                if status.code() == tonic::Code::InvalidArgument {
                    SourceFailure::Invalid
                } else {
                    SourceFailure::Unavailable
                }
            })?
            .into_inner();
        let mut profiles = Vec::with_capacity(reply.profiles.len());
        for profile in reply.profiles {
            let profile_id = uuid(&profile.profile_id).ok_or(SourceFailure::Invalid)?;
            let account_id = uuid(&profile.account_id).ok_or(SourceFailure::Invalid)?;
            let updated_at = DateTime::from_timestamp_micros(profile.updated_at_unix_micros)
                .ok_or(SourceFailure::Invalid)?;
            let mut favorites = Vec::with_capacity(profile.favorites.len());
            for favorite in profile.favorites {
                favorites.push(SourceFavorite {
                    category: favorite.category,
                    catalog_item_id: uuid(&favorite.catalog_item_id)
                        .ok_or(SourceFailure::Invalid)?,
                    label: favorite.label,
                    position: i16::try_from(favorite.position)
                        .map_err(|_| SourceFailure::Invalid)?,
                });
            }
            profiles.push(SourceProfile {
                profile_id,
                account_id,
                nickname: profile.nickname,
                bio: profile.bio,
                profile_image_media_id: profile
                    .profile_image_media_id
                    .as_deref()
                    .and_then(uuid),
                interaction_style: profile.interaction_style,
                visibility: profile.visibility,
                favorites,
                updated_at,
                deleted: profile.deleted,
            });
        }
        let next = match (
            reply.next_updated_at_unix_micros,
            reply.next_profile_id.as_deref(),
        ) {
            (Some(micros), Some(id)) => Some((
                DateTime::from_timestamp_micros(micros).ok_or(SourceFailure::Invalid)?,
                uuid(id).ok_or(SourceFailure::Invalid)?,
            )),
            (None, None) => None,
            _ => return Err(SourceFailure::Invalid),
        };
        Ok(SourcePage {
            profiles,
            next,
            more: reply.more,
        })
    }
}

/// What one pass did.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct PassReport {
    /// Source pages applied.
    pub pages: usize,
    /// Profiles written or removed.
    pub profiles: usize,
    /// Whether the pass reached the end of the source.
    pub caught_up: bool,
    /// The watermark after the pass.
    pub watermark: Option<(DateTime<Utc>, Uuid)>,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ProjectionError {
    /// The source was unavailable. The projection is unchanged.
    SourceUnavailable,
    /// The source returned something uninterpretable. The projection is
    /// unchanged for that page.
    SourceInvalid,
    /// The projection store is unavailable.
    StoreUnavailable,
}

impl std::fmt::Display for ProjectionError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "discovery projection error: {self:?}")
    }
}

impl std::error::Error for ProjectionError {}

impl From<sqlx::Error> for ProjectionError {
    fn from(_: sqlx::Error) -> Self {
        Self::StoreUnavailable
    }
}

impl From<SourceFailure> for ProjectionError {
    fn from(failure: SourceFailure) -> Self {
        match failure {
            SourceFailure::Unavailable => Self::SourceUnavailable,
            SourceFailure::Invalid => Self::SourceInvalid,
        }
    }
}

/// The projection writer.
pub struct ProjectionWriter {
    pool: PgPool,
    source: std::sync::Arc<dyn DiscoverySource>,
}

impl ProjectionWriter {
    pub fn new(pool: PgPool, source: std::sync::Arc<dyn DiscoverySource>) -> Self {
        Self { pool, source }
    }

    /// The current watermark, or `None` before the first pass.
    pub async fn watermark(&self) -> Result<Option<(DateTime<Utc>, Uuid)>, ProjectionError> {
        let row = sqlx::query(
            "select source_updated_at, source_profile_id from discovery_projection_state \
             where singleton",
        )
        .fetch_one(&self.pool)
        .await?;
        let at: Option<DateTime<Utc>> = row.try_get("source_updated_at")?;
        let id: Option<Uuid> = row.try_get("source_profile_id")?;
        Ok(match (at, id) {
            (Some(at), Some(id)) => Some((at, id)),
            _ => None,
        })
    }

    /// Run one bounded pass.
    ///
    /// The pass applies at most [`MAX_PAGES_PER_PASS`] pages and stops early if
    /// the source reports it has caught up. Each page is applied in its own
    /// transaction that also advances the watermark, so the two can never
    /// disagree.
    pub async fn run_pass(&self) -> Result<PassReport, ProjectionError> {
        let started = Utc::now();
        let mut watermark = self.watermark().await?;
        let mut report = PassReport {
            watermark,
            ..PassReport::default()
        };
        for _ in 0..MAX_PAGES_PER_PASS {
            let page = self.source.read_changed(watermark, PAGE_SIZE).await?;
            if page.profiles.is_empty() {
                report.caught_up = !page.more;
                break;
            }
            let applied = page.profiles.len();
            self.apply_page(&page).await?;
            // The watermark advances to the cursor the SERVER returned, never to
            // a value computed here. If the server returned no cursor for a
            // non-empty page that is a protocol violation, not a silent success.
            let next = page.next.ok_or(ProjectionError::SourceInvalid)?;
            watermark = Some(next);
            report.pages += 1;
            report.profiles += applied;
            report.watermark = watermark;
            report.caught_up = !page.more;
            if !page.more {
                break;
            }
        }
        self.record_run(&started, &report).await?;
        Ok(report)
    }

    /// Apply one page atomically.
    ///
    /// Every profile in the page is written inside one transaction, and the
    /// watermark advances in that same transaction. A crash therefore either
    /// applies the whole page and advances, or applies nothing.
    async fn apply_page(&self, page: &SourcePage) -> Result<(), ProjectionError> {
        let mut tx = self.pool.begin().await?;
        for profile in &page.profiles {
            if profile.deleted {
                // A tombstone removes the projection row. The favorites cascade
                // with it (0086 declares `on delete cascade`).
                sqlx::query("delete from discovery_profiles where profile_id=$1")
                    .bind(profile.profile_id)
                    .execute(&mut *tx)
                    .await?;
                continue;
            }
            // The profile facts are an upsert keyed by `profile_id`, so applying
            // the same page twice is a no-op.
            sqlx::query(
                "insert into discovery_profiles(profile_id,account_id,nickname,bio,\
                 profile_image_media_id,interaction_style,visibility,updated_at) \
                 values($1,$2,$3,$4,$5,$6,$7,$8) \
                 on conflict (profile_id) do update set \
                   account_id=excluded.account_id, nickname=excluded.nickname, bio=excluded.bio, \
                   profile_image_media_id=excluded.profile_image_media_id, \
                   interaction_style=excluded.interaction_style, \
                   visibility=excluded.visibility, updated_at=excluded.updated_at",
            )
            .bind(profile.profile_id)
            .bind(profile.account_id)
            .bind(&profile.nickname)
            .bind(&profile.bio)
            .bind(profile.profile_image_media_id)
            .bind(&profile.interaction_style)
            .bind(&profile.visibility)
            .bind(profile.updated_at)
            .execute(&mut *tx)
            .await?;
            // The favorites are replaced wholesale: the source's set is the
            // truth, and a stale row must not survive a removal.
            sqlx::query("delete from discovery_favorite_items where profile_id=$1")
                .bind(profile.profile_id)
                .execute(&mut *tx)
                .await?;
            for favorite in &profile.favorites {
                sqlx::query(
                    "insert into discovery_favorite_items(profile_id,category,catalog_item_id,\
                     label,position) values($1,$2,$3,$4,$5)",
                )
                .bind(profile.profile_id)
                .bind(&favorite.category)
                .bind(favorite.catalog_item_id)
                .bind(&favorite.label)
                .bind(favorite.position)
                .execute(&mut *tx)
                .await?;
            }
        }
        if let Some((at, id)) = page.next {
            sqlx::query(
                "update discovery_projection_state set source_updated_at=$1, \
                 source_profile_id=$2, updated_at=clock_timestamp(), last_failure=null \
                 where singleton",
            )
            .bind(at)
            .bind(id)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;
        Ok(())
    }

    /// Record a completed pass. Diagnostic only; a failure here does not undo
    /// the pass, because the projection is already consistent.
    async fn record_run(
        &self,
        started: &DateTime<Utc>,
        report: &PassReport,
    ) -> Result<(), ProjectionError> {
        sqlx::query(
            "insert into discovery_projection_runs(started_at,pages,profiles,caught_up) \
             values($1,$2,$3,$4)",
        )
        .bind(*started)
        .bind(report.pages as i32)
        .bind(report.profiles as i32)
        .bind(report.caught_up)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Record a failure without touching the projection.
    pub async fn record_failure(&self, message: &str) -> Result<(), ProjectionError> {
        // Bounded and control-character free, per the column's constraint.
        let bounded: String = message
            .chars()
            .filter(|character| !character.is_control())
            .take(400)
            .collect();
        let bounded = if bounded.is_empty() {
            "source unavailable".to_owned()
        } else {
            bounded
        };
        sqlx::query(
            "update discovery_projection_state set last_failure=$1, updated_at=clock_timestamp() \
             where singleton",
        )
        .bind(bounded)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    /// Mirror one block state.
    ///
    /// The block **source** is the friend service's invalidation journal: it
    /// publishes `FRIEND_RELATIONSHIP_CHANGED` signals with a `Pair { actor,
    /// subject }` scope, and the concert service already consumes that stream
    /// for relationship authority. This method is the write half; the caller
    /// resolves a signal's pair to account ids and calls it.
    ///
    /// `active = true` restores the block (`deleted_at` null); `active = false`
    /// lifts it. Both are idempotent: replaying the same signal writes the same
    /// row.
    ///
    /// A block is **not** a profile fact, so a lifted block is recorded as a
    /// tombstone (`deleted_at`) rather than a delete. The discovery query already
    /// filters on `deleted_at is null`, so a restored profile needs no query
    /// change, and the history of the block survives for an operator to inspect.
    pub async fn mirror_block(
        &self,
        blocker_account_id: Uuid,
        blocked_account_id: Uuid,
        active: bool,
    ) -> Result<(), ProjectionError> {
        if blocker_account_id.is_nil()
            || blocked_account_id.is_nil()
            || blocker_account_id == blocked_account_id
        {
            return Err(ProjectionError::SourceInvalid);
        }
        sqlx::query(
            "insert into discovery_blocks(blocker_account_id,blocked_account_id,deleted_at) \
             values($1,$2,case when $3 then null else clock_timestamp() end) \
             on conflict (blocker_account_id,blocked_account_id) do update set \
               deleted_at=case when $3 then null else clock_timestamp() end",
        )
        .bind(blocker_account_id)
        .bind(blocked_account_id)
        .bind(active)
        .execute(&self.pool)
        .await?;
        Ok(())
    }
}
