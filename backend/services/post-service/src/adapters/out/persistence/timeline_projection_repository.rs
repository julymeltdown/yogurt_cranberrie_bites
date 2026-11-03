use super::event_outbox::PgEventOutbox;
use super::feed_recommendation_repository::{
    engage_projection, enqueue_projection, remove_projection,
};
use crate::application::dto::timeline_projection_dto::*;
use crate::application::ports::TimelineProjectionRepositoryPort;
use anyhow::{ensure, Result};
use async_trait::async_trait;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgTimelineProjectionRepository {
    pool: PgPool,
}
impl PgTimelineProjectionRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

const EVENT_COLUMNS: &str = "event_id,event_type,aggregate_id,aggregate_version,actor_id,attributes,payload,occurred_at,created_at,published,published_at,replay_resolution_id,origin_traceparent";

async fn fingerprint(
    tx: &mut Transaction<'_, Postgres>,
    event: &TimelineProjectionEventDto,
) -> Result<String> {
    Ok(
        sqlx::query_scalar("select encode(sha256(convert_to($1,'UTF8')),'hex')")
            .bind(serde_json::to_string(event)?)
            .fetch_one(&mut **tx)
            .await?,
    )
}
async fn duplicate(
    tx: &mut Transaction<'_, Postgres>,
    event: &TimelineProjectionEventDto,
    hash: &str,
) -> Result<bool> {
    let receipt = sqlx::query("select aggregate_id,aggregate_version,identity_sha256 from timeline_projection_receipts where event_id=$1")
        .bind(event.event_id).fetch_optional(&mut **tx).await?;
    if let Some(receipt) = receipt {
        ensure!(
            receipt.get::<Uuid, _>("aggregate_id") == event.aggregate_id
                && Some(receipt.get::<i64, _>("aggregate_version")) == event.aggregate_version
                && receipt.get::<String, _>("identity_sha256") == hash,
            "Conflicting timeline event identity"
        );
        return Ok(true);
    }
    Ok(false)
}
async fn source(
    tx: &mut Transaction<'_, Postgres>,
    event: &TimelineProjectionEventDto,
) -> Result<bool> {
    let sql = format!("select {EVENT_COLUMNS} from post_event_outbox where event_id=$1 for share");
    let row = sqlx::query(&sql)
        .bind(event.event_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Unproven timeline history: owner event missing"))?;
    let stored = PgEventOutbox::map_event(row);
    ensure!(
        TimelineProjectionEventDto::from(&stored) == *event,
        "Timeline event differs from authoritative owner source"
    );
    // Old target acknowledgements are not proof of effects. Even published=true
    // can have resulted from the old optional-repository silent no-op path.
    let checkpoint: bool = sqlx::query_scalar(
        "select exists(select 1 from event_publications where event_id=$1 and target='TIMELINE')",
    )
    .bind(event.event_id)
    .fetch_one(&mut **tx)
    .await?;
    Ok(stored.published || checkpoint)
}
async fn legacy_absent(tx: &mut Transaction<'_, Postgres>, aggregate: Uuid) -> Result<()> {
    let present: bool = sqlx::query_scalar("select exists(select 1 from post_event_outbox where aggregate_id=$1 and aggregate_version is null)")
        .bind(aggregate).fetch_one(&mut **tx).await?;
    ensure!(
        !present,
        "Unproven timeline history: legacy adoption required"
    );
    Ok(())
}

#[async_trait]
impl TimelineProjectionRepositoryPort for PgTimelineProjectionRepository {
    async fn prepare(
        &self,
        event: &TimelineProjectionEventDto,
        limit: i32,
    ) -> Result<TimelineProjectionPlanDto> {
        ensure!(
            (1..=100).contains(&limit),
            "Invalid timeline recovery limit"
        );
        let mut tx = self.pool.begin().await?;
        let hash = fingerprint(&mut tx, event).await?;
        if duplicate(&mut tx, event, &hash).await? {
            return Ok(TimelineProjectionPlanDto::Duplicate);
        }
        source(&mut tx, event).await?;
        let requested = event
            .aggregate_version
            .filter(|v| *v > 0)
            .ok_or_else(|| anyhow::anyhow!("Unproven timeline history: unsequenced event"))?;
        legacy_absent(&mut tx, event.aggregate_id).await?;
        let last: i64 = sqlx::query_scalar(
            "select last_version from timeline_projection_cursors where aggregate_id=$1",
        )
        .bind(event.aggregate_id)
        .fetch_optional(&mut *tx)
        .await?
        .unwrap_or(0);
        if requested <= last && duplicate(&mut tx, event, &hash).await? {
            return Ok(TimelineProjectionPlanDto::Duplicate);
        }
        ensure!(requested > last, "Unseen stale timeline event");
        let sql = format!("select {EVENT_COLUMNS} from post_event_outbox where aggregate_id=$1 and aggregate_version>$2 and aggregate_version<=$3 order by aggregate_version limit $4");
        let rows = sqlx::query(&sql)
            .bind(event.aggregate_id)
            .bind(last)
            .bind(requested)
            .bind(limit)
            .fetch_all(&mut *tx)
            .await?;
        ensure!(
            !rows.is_empty(),
            "Unproven timeline history: missing predecessor"
        );
        let mut works = Vec::with_capacity(rows.len());
        let mut expected = last + 1;
        for row in rows {
            let predecessor = TimelineProjectionEventDto::from(&PgEventOutbox::map_event(row));
            ensure!(
                predecessor.aggregate_version == Some(expected),
                "Unproven timeline history: sequence gap"
            );
            let effect = predecessor.effect()?;
            if expected == 1 {
                ensure!(
                    matches!(effect, TimelineProjectionEffectDto::Create { .. }),
                    "Unproven timeline history: creation genesis missing"
                );
            }
            expected += 1;
            works.push(TimelineProjectionWorkDto {
                event: predecessor,
                effect,
            });
        }
        tx.commit().await?;
        Ok(TimelineProjectionPlanDto::Events(works))
    }

    async fn apply(
        &self,
        request: &TimelineProjectionApplyDto,
    ) -> Result<TimelineProjectionOutcomeDto> {
        use TimelineProjectionEffectDto::*;
        let event = &request.work.event;
        let effect = &request.work.effect;
        ensure!(
            *effect == event.effect()?,
            "Projection effect differs from owner event"
        );
        ensure!(
            request.high_fanout_threshold > 0 && request.follower_count.is_none_or(|n| n >= 0),
            "Invalid projection fanout policy"
        );
        let mut tx = self.pool.begin().await?;
        // Reuse the lifecycle's sorted subject locks BEFORE the post lock. Read
        // actual owner too, including no-op events; retry if it changes meanwhile.
        let observed_author: Option<Uuid> =
            sqlx::query_scalar("select author_id from posts where id=$1")
                .bind(event.aggregate_id)
                .fetch_optional(&mut *tx)
                .await?;
        let mut subjects = effect.subjects();
        subjects.extend(observed_author);
        subjects.extend(
            super::affinity_provenance::projection_subjects(&mut tx, event.event_id).await?,
        );
        sqlx::query("select post_lock_subject_ids($1::uuid[])")
            .bind(&subjects)
            .execute(&mut *tx)
            .await?;
        let post = sqlx::query(
            "select author_id,deleted_at is not null as deleted from posts where id=$1 for update",
        )
        .bind(event.aggregate_id)
        .fetch_optional(&mut *tx)
        .await?;
        ensure!(
            post.as_ref().map(|p| p.get::<Uuid, _>("author_id")) == observed_author,
            "Projection owner changed during locking"
        );
        sqlx::query("insert into timeline_projection_cursors(aggregate_id) values ($1) on conflict do nothing")
            .bind(event.aggregate_id).execute(&mut *tx).await?;
        let last: i64 = sqlx::query_scalar(
            "select last_version from timeline_projection_cursors where aggregate_id=$1 for update",
        )
        .bind(event.aggregate_id)
        .fetch_one(&mut *tx)
        .await?;
        let hash = fingerprint(&mut tx, event).await?;
        if duplicate(&mut tx, event, &hash).await? {
            tx.commit().await?;
            return Ok(TimelineProjectionOutcomeDto::Duplicate);
        }
        let authority = request.authority.as_ref().ok_or_else(|| {
            anyhow::anyhow!("Live timeline claim required; historical imports are not authorized")
        })?;
        ensure!(
            authority.target == crate::domain::post::DeliveryTarget::Timeline
                && authority.event.aggregate_id == event.aggregate_id
                && event
                    .aggregate_version
                    .zip(authority.event.aggregate_version)
                    .is_some_and(|(v, limit)| v > 0 && v <= limit),
            "Invalid timeline claim scope"
        );
        source(&mut tx, &TimelineProjectionEventDto::from(&authority.event)).await?;
        let owned: bool = sqlx::query_scalar("select state='LEASED' and lease_token=$2 and attempt_id=$3 and attempt_count=$4 and lease_expires_at>clock_timestamp() from post_event_delivery_jobs where event_id=$1 and target='TIMELINE' for update")
            .bind(authority.event.event_id).bind(authority.lease_token).bind(authority.attempt_id).bind(authority.attempt_number).fetch_optional(&mut *tx).await?.unwrap_or(false);
        ensure!(owned, "Timeline delivery authority lost");
        let started: bool = sqlx::query_scalar("select exists(select 1 from post_event_delivery_attempts where attempt_id=$1 and event_id=$2 and target='TIMELINE' and phase='STARTED')")
            .bind(authority.attempt_id).bind(authority.event.event_id).fetch_one(&mut *tx).await?;
        ensure!(started, "Timeline claim has no immutable STARTED marker");
        let acknowledged = source(&mut tx, event).await?;
        legacy_absent(&mut tx, event.aggregate_id).await?;
        let version = event
            .aggregate_version
            .ok_or_else(|| anyhow::anyhow!("Unsequenced timeline event"))?;
        ensure!(
            version == last + 1,
            "Timeline predecessor is missing or event is stale"
        );
        ensure!(
            !matches!(effect, Create { .. }) || version == 1,
            "Repeated timeline creation requires an explicit epoch"
        );
        if version == 1 {
            ensure!(
                matches!(effect, Create { .. }),
                "Unproven timeline creation genesis"
            );
        }
        if let Some(post) = &post {
            let author = post.get::<Uuid, _>("author_id");
            let expected = match *effect {
                Create { author_id } | Delete { author_id } | Engagement { author_id, .. } => {
                    Some(author_id)
                }
                StoredUnlike { .. } | Noop => None,
            };
            ensure!(
                expected.is_none_or(|id| id == author),
                "Projection author differs from post owner"
            );
        }
        let deleted_subject: bool = sqlx::query_scalar("select exists(select 1 from post_user_deletion_tombstones where user_id=any($1::uuid[]))")
            .bind(&subjects).fetch_one(&mut *tx).await?;
        super::affinity_provenance::require_projection_resolver(
            &mut tx,
            event.event_id,
            authority,
            &hash,
        )
        .await?;
        let retired_post = post.as_ref().is_none_or(|p| p.get::<bool, _>("deleted"));
        let removed = deleted_subject || retired_post;
        let known_effect = match *effect {
            Engagement { .. } | StoredUnlike { .. } => sqlx::query_scalar::<_,bool>("select exists(select 1 from feed_projection_inbox where consumer='feed-engagement' and event_id=$1)")
                .bind(event.event_id).fetch_one(&mut *tx).await?,
            Create { author_id } => sqlx::query_scalar::<_,bool>("select exists(select 1 from feed_fanout_jobs where event_id=$1 and post_id=$2 and author_id=$3 and post_created_at=$4)")
                .bind(event.event_id).bind(event.aggregate_id).bind(author_id).bind(event.occurred_at).fetch_one(&mut *tx).await?,
            _ => false,
        };
        if matches!(effect, Create { .. }) && !known_effect && !retired_post {
            let prior: bool = sqlx::query_scalar("select exists(select 1 from home_timeline_entries where post_id=$1) or exists(select 1 from high_fanout_posts where post_id=$1) or exists(select 1 from feed_fanout_jobs where post_id=$1)")
                .bind(event.aggregate_id).fetch_one(&mut *tx).await?;
            ensure!(!prior, "Unproven timeline creation state requires adoption");
        }
        // The old inbox prevents duplicate deltas but cannot prove their original
        // clamp/decay ordering. Hold rather than silently claiming repair.
        ensure!(
            !known_effect
                || !matches!(effect, Engagement { .. } | StoredUnlike { .. })
                || retired_post,
            "Unproven timeline history: engagement adoption requires ordering evidence"
        );
        ensure!(
            !acknowledged || known_effect || matches!(effect, Noop) || removed,
            "Unproven timeline history: acknowledgement has no effect provenance"
        );
        let (disposition, outcome) = if known_effect {
            ("ADOPTED", TimelineProjectionOutcomeDto::Adopted)
        } else if removed && !matches!(effect, Delete { .. } | Noop) {
            ("SUPPRESSED", TimelineProjectionOutcomeDto::Suppressed)
        } else {
            match *effect {
                Create { author_id } => {
                    let count = request
                        .follower_count
                        .ok_or_else(|| anyhow::anyhow!("Missing prepared follower count"))?;
                    let now = sqlx::query_scalar("select clock_timestamp()")
                        .fetch_one(&mut *tx)
                        .await?;
                    enqueue_projection(
                        &mut tx,
                        event.event_id,
                        event.aggregate_id,
                        author_id,
                        event.occurred_at,
                        count,
                        request.high_fanout_threshold,
                        now,
                    )
                    .await?;
                }
                Delete { .. } => remove_projection(&mut tx, event.aggregate_id).await?,
                Engagement {
                    actor_id,
                    author_id,
                    action,
                } => {
                    engage_projection(
                        &mut tx,
                        event.event_id,
                        event.aggregate_id,
                        actor_id,
                        author_id,
                        action,
                        event.occurred_at,
                    )
                    .await?
                }
                StoredUnlike { actor_id } => {
                    let author_id = post
                        .as_ref()
                        .ok_or_else(|| anyhow::anyhow!("Stored unlike owner unavailable"))?
                        .get("author_id");
                    engage_projection(
                        &mut tx,
                        event.event_id,
                        event.aggregate_id,
                        actor_id,
                        author_id,
                        crate::domain::ranking::FeedEngagementAction::Unlike,
                        event.occurred_at,
                    )
                    .await?;
                }
                Noop => {}
            }
            (
                if matches!(effect, Noop) {
                    "NOOP"
                } else {
                    "APPLIED"
                },
                TimelineProjectionOutcomeDto::Applied,
            )
        };
        sqlx::query("insert into timeline_projection_receipts(event_id,aggregate_id,aggregate_version,identity_sha256,disposition,delivery_event_id,delivery_attempt_id) values ($1,$2,$3,$4,$5,$6,$7)")
            .bind(event.event_id).bind(event.aggregate_id).bind(version).bind(hash).bind(disposition).bind(authority.event.event_id).bind(authority.attempt_id).execute(&mut *tx).await?;
        sqlx::query("update timeline_projection_cursors set last_version=$2,updated_at=clock_timestamp() where aggregate_id=$1")
            .bind(event.aggregate_id).bind(version).execute(&mut *tx).await?;
        tx.commit().await?;
        Ok(outcome)
    }
}
