//! Read-only persisted delivery ownership; no production listener or signer.
use super::event_outbox::PgEventOutbox;
use async_trait::async_trait;
use post_application::delivery_authority::{
    DeliveryAttemptProvenanceDto, DeliveryAuthorityErrorDto as Error,
    DeliveryAuthorityRepositoryPort, DeliveryClaimLookupDto, DeliveryLeaseKindDto,
    DeliveryReplayRestoreDto, OwnedDeliveryClaimDto,
};
use post_application::dto::{
    delivery_schedule_dto::TargetDeliveryClaimDto,
    timeline_projection_dto::TimelineProjectionEventDto,
};
use post_domain::post::DeliveryAttemptSource;
use sqlx::{PgPool, Row};

#[derive(Clone)]
pub struct PgDeliveryAuthorityRepository {
    pool: PgPool,
}
impl PgDeliveryAuthorityRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
fn unavailable(_: sqlx::Error) -> Error {
    Error::Unavailable
}

#[async_trait]
impl DeliveryAuthorityRepositoryPort for PgDeliveryAuthorityRepository {
    async fn resolve_claimed_delivery(
        &self,
        request: &DeliveryClaimLookupDto,
    ) -> Result<OwnedDeliveryClaimDto, Error> {
        let expected = request.claim();
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        // Same parent-before-target order as completion/replay. This generic
        // lookup never locks a Post or requires its continued existence: cache
        // deletion and stored-event delivery may outlive the resource.
        let parent = sqlx::query("select event_id,event_type,aggregate_id,aggregate_version,actor_id,attributes,payload,occurred_at,created_at,published,published_at,replay_resolution_id,origin_traceparent,delivery_required_targets,locked_at from post_event_outbox where event_id=$1 for share")
            .bind(expected.event.event_id).fetch_optional(&mut *tx).await.map_err(unavailable)?.ok_or(Error::Denied)?;
        let required: Option<Vec<String>> = parent
            .try_get("delivery_required_targets")
            .map_err(unavailable)?;
        let required = required.ok_or(Error::Denied)?;
        if parent
            .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("locked_at")
            .map_err(unavailable)?
            .is_some()
        {
            return Err(Error::Denied);
        }
        let stored = PgEventOutbox::map_event(parent);
        // Mutable publication/release metadata is not source identity. Retained
        // trace context, payload, attributes, actor, version and times are exact.
        if TimelineProjectionEventDto::from(&stored)
            != TimelineProjectionEventDto::from(&expected.event)
            || stored.origin_traceparent != expected.event.origin_traceparent
        {
            return Err(Error::Denied);
        }
        let job = sqlx::query(
            "select * from post_event_delivery_jobs where event_id=$1 and target=$2 for share",
        )
        .bind(stored.event_id)
        .bind(expected.target.as_str())
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?
        .ok_or(Error::Denied)?;
        if job.try_get::<String, _>("state").map_err(unavailable)? != "LEASED"
            || job
                .try_get::<Option<uuid::Uuid>, _>("lease_token")
                .map_err(unavailable)?
                != Some(expected.lease_token)
            || job
                .try_get::<Option<uuid::Uuid>, _>("attempt_id")
                .map_err(unavailable)?
                != Some(expected.attempt_id)
            || job
                .try_get::<i32, _>("attempt_count")
                .map_err(unavailable)?
                != expected.attempt_number
            || job
                .try_get::<Option<chrono::DateTime<chrono::Utc>>, _>("lease_expires_at")
                .map_err(unavailable)?
                != Some(expected.lease_expires_at)
        {
            return Err(Error::Denied);
        }
        let kind: String = job.try_get("lease_kind").map_err(unavailable)?;
        let restore: Option<String> = job.try_get("replay_restore_state").map_err(unavailable)?;
        let lease_kind = match (kind.as_str(), restore.as_deref()) {
            ("AUTO", None) => DeliveryLeaseKindDto::Automatic,
            ("REPLAY", Some(state)) => DeliveryLeaseKindDto::Replay(match state {
                "PENDING" => DeliveryReplayRestoreDto::Pending,
                "RETRY" => DeliveryReplayRestoreDto::Retry,
                "SUCCEEDED" => DeliveryReplayRestoreDto::Succeeded,
                "SKIPPED" => DeliveryReplayRestoreDto::Skipped,
                _ => return Err(Error::Denied),
            }),
            _ => return Err(Error::Denied),
        };
        let marker = sqlx::query("select source,resolution_id from post_event_delivery_attempts where attempt_id=$1 and event_id=$2 and target=$3 and phase='STARTED' and not exists(select 1 from post_event_delivery_attempts terminal where terminal.attempt_id=$1 and terminal.phase<>'STARTED')")
            .bind(expected.attempt_id).bind(stored.event_id).bind(expected.target.as_str())
            .fetch_optional(&mut *tx).await.map_err(unavailable)?.ok_or(Error::Denied)?;
        let resolution_id: Option<uuid::Uuid> =
            marker.try_get("resolution_id").map_err(unavailable)?;
        if resolution_id != job.try_get("release_resolution_id").map_err(unavailable)? {
            return Err(Error::Denied);
        }
        if let Some(resolution) = resolution_id {
            let same_event: bool = sqlx::query_scalar("select exists(select 1 from post_event_dead_letter_resolutions where resolution_id=$1 and event_id=$2)")
                .bind(resolution).bind(stored.event_id).fetch_one(&mut *tx).await.map_err(unavailable)?;
            if !same_event {
                return Err(Error::Denied);
            }
        }
        let source = match marker
            .try_get::<String, _>("source")
            .map_err(unavailable)?
            .as_str()
        {
            "OUTBOX_WORKER" => DeliveryAttemptSource::OutboxWorker,
            "ADMIN_REPLAY" => DeliveryAttemptSource::AdminReplay,
            _ => return Err(Error::Denied),
        };
        // A legacy checkpoint invalidates AUTO authority even before scheduler
        // reconciliation. Explicit completed replay legitimately retains it.
        let checkpoint: bool = sqlx::query_scalar(
            "select exists(select 1 from event_publications where event_id=$1 and target=$2)",
        )
        .bind(stored.event_id)
        .bind(expected.target.as_str())
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
        if lease_kind == DeliveryLeaseKindDto::Automatic && (stored.published || checkpoint) {
            return Err(Error::Denied);
        }
        let checked_at = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(unavailable)?;
        let result = OwnedDeliveryClaimDto::from_persisted_claim(
            TargetDeliveryClaimDto {
                event: stored,
                target: expected.target,
                lease_token: job.try_get("lease_token").map_err(unavailable)?,
                attempt_id: job.try_get("attempt_id").map_err(unavailable)?,
                attempt_number: job.try_get("attempt_count").map_err(unavailable)?,
                lease_expires_at: job.try_get("lease_expires_at").map_err(unavailable)?,
            },
            DeliveryAttemptProvenanceDto {
                lease_kind,
                source,
                resolution_id,
                max_attempts: job.try_get("max_attempts").map_err(unavailable)?,
                progress_credits: job.try_get("progress_credits").map_err(unavailable)?,
                target_required: required.iter().any(|v| v == expected.target.as_str()),
            },
            checked_at,
        )?;
        tx.commit().await.map_err(unavailable)?;
        // Successful returns await commit; errors retain SQLx queued rollback.
        // Independent issuance must still check fresh time after its own await.
        if result.expires_at() <= chrono::Utc::now() {
            return Err(Error::Denied);
        }
        Ok(result)
    }
}
