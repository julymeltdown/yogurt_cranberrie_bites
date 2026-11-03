use crate::application::dto::delivery_schedule_dto::TargetDeliveryClaimDto;
use crate::application::dto::mutation_dto::MutationError;
use anyhow::Result;
use chrono::{DateTime, Utc};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

pub(super) struct LegacyAffinityMutationDto<'a> {
    pub namespace: &'a str,
    pub source_id: Uuid,
    pub viewer_id: Uuid,
    pub author_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub post_id: Uuid,
    pub action: &'a str,
    pub occurred_at: DateTime<Utc>,
    pub positive: f64,
    pub negative: f64,
}

/// A retained source still identifies its complete subjects when the Post row
/// has been removed or the stored unlike payload omits the owner. Discover them
/// before any Post/job/cursor locks; immutable capture plus subject locks then
/// prevents lifecycle erasure from changing this set during resolution.
pub(super) async fn projection_subjects(
    tx: &mut Transaction<'_, Postgres>,
    event_id: Uuid,
) -> Result<Vec<Uuid>> {
    let installed: bool = sqlx::query_scalar(
        "select to_regclass('public.feed_affinity_source_contributions') is not null",
    )
    .fetch_one(&mut **tx)
    .await?;
    if !installed {
        return Ok(Vec::new());
    }
    Ok(sqlx::query_scalar("select distinct subject from public.feed_affinity_source_contributions c cross join lateral unnest(array[c.viewer_id,c.author_id,c.actor_id]) subject where c.namespace='OUTBOX' and c.source_id=$1 and subject is not null order by subject")
        .bind(event_id).fetch_all(&mut **tx).await?)
}

/// A post receipt/cursor, including lifecycle suppression, cannot consume an
/// ordered contribution before its pair resolution exists. Account erasure
/// removes personal capture state; the existing tombstone path then owns the
/// terminal acknowledgement without reinitializing any pair.
pub(super) async fn require_projection_resolver(
    tx: &mut Transaction<'_, Postgres>,
    event_id: Uuid,
    authority: &TargetDeliveryClaimDto,
    projection_hash: &str,
) -> Result<()> {
    if ordered_installed(tx).await? {
        let resolved: bool = sqlx::query_scalar(
            "select public.feed_affinity_resolve_ordered('OUTBOX',$1,$2,$3,$4,$5,$6)",
        )
        .bind(event_id)
        .bind(authority.event.event_id)
        .bind(authority.lease_token)
        .bind(authority.attempt_id)
        .bind(authority.attempt_number)
        .bind(projection_hash)
        .fetch_one(&mut **tx)
        .await
        .map_err(ordered_error)?;
        if resolved {
            return Ok(());
        }
    }
    let installed: bool =
        sqlx::query_scalar("select to_regclass('public.feed_affinity_pair_baselines') is not null")
            .fetch_one(&mut **tx)
            .await?;
    if installed {
        let blocked: bool = sqlx::query_scalar(
            "select exists(select 1 from public.feed_affinity_source_contributions c \
             left join public.feed_affinity_pair_provenance o using(epoch_id,viewer_id,author_id) \
             left join public.feed_affinity_pair_baselines b using(epoch_id,viewer_id,author_id) \
             where c.namespace='OUTBOX' and c.source_id=$1 \
             and (o.origin_mode='ORDERED_FRESH_BLOCKED' or b.epoch_id is not null))",
        )
        .bind(event_id)
        .fetch_one(&mut **tx)
        .await?;
        if blocked {
            return Err(MutationError::Unavailable.into());
        }
    }
    Ok(())
}

async fn ordered_installed(tx: &mut Transaction<'_, Postgres>) -> Result<bool> {
    Ok(sqlx::query_scalar("select to_regprocedure('public.feed_affinity_resolve_ordered(text,uuid,uuid,uuid,uuid,integer,text)') is not null")
        .fetch_one(&mut **tx).await?)
}

fn ordered_error(error: sqlx::Error) -> anyhow::Error {
    if error
        .as_database_error()
        .and_then(|db| db.constraint())
        .is_some_and(|c| {
            matches!(
                c,
                "feed_affinity_ordered_writer_required" | "feed_affinity_resolution_authority"
            )
        })
    {
        MutationError::Unavailable.into()
    } else {
        error.into()
    }
}

/// Validate and apply the actual compatibility mutation together with its
/// captured-source observation. False requests the pre-0024 historical path.
pub(super) async fn apply_legacy_effect(
    tx: &mut Transaction<'_, Postgres>,
    mutation: &LegacyAffinityMutationDto<'_>,
) -> Result<bool> {
    if ordered_installed(tx).await? {
        let resolved = if mutation.namespace == "RECOMMENDATION" {
            sqlx::query_scalar::<_, bool>("select public.feed_affinity_resolve_ordered('RECOMMENDATION',$1,null,null,null,null,null)")
                .bind(mutation.source_id).fetch_one(&mut **tx).await.map_err(ordered_error)?
        } else {
            sqlx::query_scalar::<_, bool>("select exists(select 1 from public.feed_affinity_resolution_receipts r join public.feed_affinity_source_contributions c using(namespace,source_id) where r.namespace='OUTBOX' and r.source_id=$1 and r.identity_sha256=c.identity_sha256 and (c.viewer_id,c.author_id,c.actor_id,c.post_id,c.occurred_at,c.positive_delta,c.negative_delta) is not distinct from ($2,$3,$4,$5,$6,$7,$8))")
                .bind(mutation.source_id).bind(mutation.viewer_id).bind(mutation.author_id).bind(mutation.actor_id)
                .bind(mutation.post_id).bind(mutation.occurred_at).bind(mutation.positive).bind(mutation.negative)
                .fetch_one(&mut **tx).await?
        };
        if resolved {
            return Ok(true);
        }
    }
    let installed: bool = sqlx::query_scalar(
        "select to_regprocedure('feed_affinity_apply_legacy_effect(text,uuid,uuid,uuid,uuid,uuid,text,timestamp with time zone,double precision,double precision)') is not null",
    )
    .fetch_one(&mut **tx)
    .await?;
    if installed {
        sqlx::query("select feed_affinity_apply_legacy_effect($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
            .bind(mutation.namespace)
            .bind(mutation.source_id)
            .bind(mutation.viewer_id)
            .bind(mutation.author_id)
            .bind(mutation.actor_id)
            .bind(mutation.post_id)
            .bind(mutation.action)
            .bind(mutation.occurred_at)
            .bind(mutation.positive)
            .bind(mutation.negative)
            .execute(&mut **tx)
            .await
            .map_err(|error| {
                if error.as_database_error().and_then(|db| db.constraint())
                    == Some("feed_affinity_ordered_writer_required")
                {
                    anyhow::Error::from(MutationError::Unavailable)
                } else {
                    anyhow::Error::from(error)
                }
            })?;
    }
    Ok(installed)
}
