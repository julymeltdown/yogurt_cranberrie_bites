//! Concert-owned social/beacon lifecycle SQL. No Auth/Post/service-table access.
//! Components are intentionally narrower than the composite EXPIRE_BEACONS/CLEANUP
//! steps: their receipts cannot stand in for world/community/other owner effects.
use super::{invalid, unavailable, Applied, EffectKey};
use chrono::{DateTime, Utc};
use concert_application::commands::CommandError;
use concert_domain::event_lifecycle::{JobState, Retention, Step};
use sqlx::{PgConnection, Row};
use uuid::Uuid;
pub const EXPIRE_GRANTS:&str="update concert_social_grants set state='REVOKED',revision=revision+1 where event_id=$1 and state='ACTIVE'";
pub const EXPIRE_REQUESTS:&str="update concert_social_requests set state='REVOKED',revision=revision+1 where event_id=$1 and state in ('PENDING','APPROVED')";
pub const EXPIRE_BEACONS:&str="update concert_meeting_beacons set state='REVOKED',revision=revision+1 where event_id=$1 and state='ACTIVE'";
#[derive(Clone, Copy, Debug)]
pub struct PrivacyComponentReceipt {
    pub event: Uuid,
    pub operation: Uuid,
    pub affected: i64,
}
async fn scope(c: &mut PgConnection, key: EffectKey) -> Result<(), CommandError> {
    let ok:bool=sqlx::query_scalar("select exists(select 1 from concert_closure_jobs j join concert_events e using(event_id) where j.event_id=$1 and j.job_id=$2 and e.phase='READ_ONLY')")
        .bind(key.event).bind(key.job).fetch_one(&mut *c).await.map_err(unavailable)?;
    if !ok {
        return Err(CommandError::Denied);
    }
    let row = sqlx::query(
        "select * from concert_closure_jobs where event_id=$1 and job_id=$2 for update",
    )
    .bind(key.event)
    .bind(key.job)
    .fetch_one(&mut *c)
    .await
    .map_err(unavailable)?;
    // The canonical relationship owner must pin its own checked authority. A
    // lifecycle deadline cannot manufacture permission to mutate consent data.
    let fenced:bool=sqlx::query_scalar("select coalesce(clock_timestamp()<nullif(current_setting('mcr.lifecycle_authority_until',true),'')::timestamptz and clock_timestamp()<nullif(current_setting('mcr.relationship_authority_until',true),'')::timestamptz,false)")
        .fetch_one(&mut *c).await.map_err(unavailable)?;
    if !fenced {
        return Err(CommandError::Unavailable);
    }
    let job = super::project(&row)?;
    if job.next() != Some(key.step) || !matches!(job.state, JobState::Pending | JobState::Running) {
        return Err(CommandError::InvalidState);
    }
    Ok(())
}
async fn prior(
    c: &mut PgConnection,
    key: EffectKey,
    component: &str,
) -> Result<Option<PrivacyComponentReceipt>, CommandError> {
    let row=sqlx::query("select event_id,component,affected from concert_closure_privacy_effects where operation_id=$1 for share").bind(component_id(key,component)).fetch_optional(c).await.map_err(unavailable)?;
    row.map(|r| {
        if r.try_get::<Uuid, _>("event_id").map_err(invalid)? != key.event
            || r.try_get::<String, _>("component").map_err(invalid)? != component
        {
            return Err(CommandError::InvalidState);
        }
        Ok(PrivacyComponentReceipt {
            event: key.event,
            operation: component_id(key, component),
            affected: r.try_get("affected").map_err(invalid)?,
        })
    })
    .transpose()
}
fn component_id(key: EffectKey, component: &str) -> Uuid {
    use sha2::{Digest, Sha256};
    let mut h = Sha256::new();
    h.update(b"mcr-be41-privacy-v1");
    h.update(key.operation_id().as_bytes());
    h.update(component);
    super::digest_uuid(h.finalize().as_slice())
}
async fn record(
    c: &mut PgConnection,
    key: EffectKey,
    component: &str,
    affected: u64,
) -> Result<PrivacyComponentReceipt, CommandError> {
    let affected = i64::try_from(affected).map_err(invalid)?;
    let operation = component_id(key, component);
    sqlx::query("insert into concert_closure_privacy_effects(event_id,job_id,operation_id,component,affected) values($1,$2,$3,$4,$5)")
        .bind(key.event).bind(key.job).bind(operation).bind(component).bind(affected).execute(c).await.map_err(unavailable)?;
    Ok(PrivacyComponentReceipt {
        event: key.event,
        operation,
        affected,
    })
}
/// Must run inside the lifecycle transaction after its event lock and authority fence.
pub async fn expire_grants_in(
    c: &mut PgConnection,
    key: EffectKey,
) -> Result<Applied, CommandError> {
    if key.step != Step::ExpireGrants {
        return Err(CommandError::InvalidInput);
    }
    scope(c, key).await?;
    let result = if let Some(r) = prior(c, key, "SOCIAL_EXPIRY").await? {
        r
    } else {
        let grants = sqlx::query(EXPIRE_GRANTS)
            .bind(key.event)
            .execute(&mut *c)
            .await
            .map_err(unavailable)?
            .rows_affected();
        let requests = sqlx::query(EXPIRE_REQUESTS)
            .bind(key.event)
            .execute(&mut *c)
            .await
            .map_err(unavailable)?
            .rows_affected();
        record(
            c,
            key,
            "SOCIAL_EXPIRY",
            grants
                .checked_add(requests)
                .ok_or(CommandError::InvalidState)?,
        )
        .await?
    };
    let remains:bool=sqlx::query_scalar("select exists(select 1 from concert_social_grants where event_id=$1 and state='ACTIVE') or exists(select 1 from concert_social_requests where event_id=$1 and state in ('PENDING','APPROVED'))").bind(key.event).fetch_one(c).await.map_err(unavailable)?;
    if remains {
        return Err(CommandError::InvalidState);
    }
    Ok(Applied {
        key,
        receipt: result.operation,
        revision: 1,
    })
}
pub async fn expire_beacons_in(
    c: &mut PgConnection,
    key: EffectKey,
) -> Result<PrivacyComponentReceipt, CommandError> {
    if key.step != Step::ExpireBeacons {
        return Err(CommandError::InvalidInput);
    }
    scope(c, key).await?;
    if let Some(r) = prior(c, key, "BEACON_EXPIRY").await? {
        return Ok(r);
    }
    let affected = sqlx::query(EXPIRE_BEACONS)
        .bind(key.event)
        .execute(&mut *c)
        .await
        .map_err(unavailable)?
        .rows_affected();
    record(c, key, "BEACON_EXPIRY", affected).await
}
/// Privacy retention only: other owners still must complete the composite cleanup.
/// Preserve command/correlation/audit tombstones; never touch another service DB.
pub async fn cleanup_privacy_in(
    c: &mut PgConnection,
    key: EffectKey,
    retention: Retention,
) -> Result<PrivacyComponentReceipt, CommandError> {
    if key.step != Step::Cleanup {
        return Err(CommandError::InvalidInput);
    }
    scope(c, key).await?;
    retention.validate().map_err(invalid)?;
    let at: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&mut *c)
        .await
        .map_err(unavailable)?;
    if at < retention.cleanup_after {
        return Err(CommandError::InvalidState);
    }
    let pinned:bool=sqlx::query_scalar("select retention_revision=$3 and closed_at=$4 and cleanup_after=$5 from concert_closure_jobs where event_id=$1 and job_id=$2 for share")
        .bind(key.event).bind(key.job).bind(retention.policy_revision).bind(retention.closed_at).bind(retention.cleanup_after).fetch_one(&mut *c).await.map_err(unavailable)?;
    if !pinned {
        return Err(CommandError::InvalidState);
    }
    if let Some(r) = prior(c, key, "PRIVACY_RETENTION").await? {
        return Ok(r);
    }
    // Terminal requests cascade only their Concert-owned bindings/grants. Active
    // rows are not silently deleted; expiry step must have completed first.
    let active:bool=sqlx::query_scalar("select exists(select 1 from concert_social_requests where event_id=$1 and state in ('PENDING','APPROVED')) or exists(select 1 from concert_social_grants where event_id=$1 and state='ACTIVE') or exists(select 1 from concert_meeting_beacons where event_id=$1 and state='ACTIVE')")
        .bind(key.event).fetch_one(&mut *c).await.map_err(unavailable)?;
    if active {
        return Err(CommandError::InvalidState);
    }
    let requests=sqlx::query("delete from concert_social_requests where event_id=$1 and created_at<=$2 and state in ('REJECTED','CANCELLED','REVOKED','EXPIRED')").bind(key.event).bind(retention.closed_at).execute(&mut *c).await.map_err(unavailable)?.rows_affected();
    let handles=sqlx::query("update concert_social_handles set key_id=null,nonce=null,ciphertext=null,revision=revision+1 where event_id=$1 and ciphertext is not null and updated_at<=$2").bind(key.event).bind(retention.closed_at).execute(&mut *c).await.map_err(unavailable)?.rows_affected();
    let beacons=sqlx::query("delete from concert_meeting_beacons where event_id=$1 and state='REVOKED' and created_at<=$2").bind(key.event).bind(retention.closed_at).execute(&mut *c).await.map_err(unavailable)?.rows_affected();
    record(
        c,
        key,
        "PRIVACY_RETENTION",
        requests
            .checked_add(handles)
            .and_then(|v| v.checked_add(beacons))
            .ok_or(CommandError::InvalidState)?,
    )
    .await
}
