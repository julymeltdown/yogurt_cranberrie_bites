//! Current local constraints only. Missing rows never prove a fenced absence.
use chrono::{DateTime, Utc};
use concert_application::notification::{Candidate, Context};
use concert_domain::notification::{NotificationError, Result, TargetType};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

pub(super) fn db(_: impl std::fmt::Display) -> NotificationError {
    NotificationError::Unavailable
}
pub(super) fn revision(value: i64) -> Result<()> {
    if (1..=concert_domain::MAX_REVISION).contains(&value) {
        Ok(())
    } else {
        Err(NotificationError::Unavailable)
    }
}
pub(super) fn id(candidate: Candidate) -> Result<Uuid> {
    candidate
        .target
        .map(|target| target.target_id)
        .ok_or(NotificationError::Unavailable)
}
pub(super) async fn current(c: &mut PgConnection, context: Context) -> Result<DateTime<Utc>> {
    let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(c)
        .await
        .map_err(db)?;
    context
        .check(now)
        .map_err(|_| NotificationError::Unavailable)?;
    context
        .check(Utc::now())
        .map_err(|_| NotificationError::Unavailable)?;
    Ok(now)
}
pub(super) async fn bound(c: &mut PgConnection, context: Context) -> Result<()> {
    let remaining = (context.expires_at - Utc::now()).num_milliseconds();
    if remaining < 1 {
        return Err(NotificationError::Unavailable);
    }
    // SET LOCAL only narrows existing limits; no authorization/RLS GUC is set.
    sqlx::query("select set_config('statement_timeout',(extract(epoch from least(coalesce(nullif(current_setting('statement_timeout')::interval,interval '0'),$1::text::interval),$1::text::interval))*1000)::bigint::text||'ms',true),set_config('lock_timeout',(extract(epoch from least(coalesce(nullif(current_setting('lock_timeout')::interval,interval '0'),$1::text::interval),$1::text::interval))*1000)::bigint::text||'ms',true)")
        .bind(format!("{remaining} milliseconds")).execute(&mut *c).await.map_err(db)?;
    current(c, context).await?;
    Ok(())
}
pub(super) async fn event(c: &mut PgConnection, context: Context) -> Result<bool> {
    let row =
        sqlx::query("select phase,revision from public.concert_events where event_id=$1 for share")
            .bind(context.event_id)
            .fetch_optional(c)
            .await
            .map_err(db)?
            .ok_or(NotificationError::Unavailable)?;
    revision(row.try_get("revision").map_err(db)?)?;
    match row.try_get::<String, _>("phase").map_err(db)?.as_str() {
        "OPEN" | "LIVE" | "READ_ONLY" => Ok(true),
        "DRAFT" | "ARCHIVED" => Ok(false),
        _ => Err(NotificationError::Unavailable),
    }
}
pub(super) async fn feature(
    c: &mut PgConnection,
    context: Context,
    kind: TargetType,
) -> Result<bool> {
    let row = sqlx::query("select parties,giveaways,social from public.concert_event_settings where event_id=$1 for share")
        .bind(context.event_id).fetch_optional(c).await.map_err(db)?.ok_or(NotificationError::Unavailable)?;
    let name = match kind {
        TargetType::Party | TargetType::Application => "parties",
        TargetType::Giveaway | TargetType::Reservation => "giveaways",
        TargetType::SocialRequest => "social",
        _ => return Err(NotificationError::Unavailable),
    };
    row.try_get(name).map_err(db)
}
/// Lock sorted existing admissions/participants and normalized pair rows. No
/// insert-on-read creates a missing policy row; Root's canonical fences remain
/// required even when every local constraint is positive.
pub(super) async fn audience(
    c: &mut PgConnection,
    context: Context,
    candidate: Candidate,
    mut subjects: Vec<Uuid>,
) -> Result<bool> {
    subjects.push(context.actor_id);
    subjects.extend(candidate.source_actor_id);
    subjects.sort();
    subjects.dedup();
    for subject in &subjects {
        if subject.is_nil() {
            return Err(NotificationError::Unavailable);
        }
        let admission = sqlx::query("select state,restriction_revision from public.concert_event_admissions where event_id=$1 and user_id=$2 for share")
            .bind(context.event_id).bind(subject).fetch_optional(&mut *c).await.map_err(db)?.ok_or(NotificationError::Unavailable)?;
        revision(admission.try_get("restriction_revision").map_err(db)?)?;
        match admission
            .try_get::<String, _>("state")
            .map_err(db)?
            .as_str()
        {
            "ACTIVE" => {}
            "SUSPENDED" => return Ok(false),
            _ => return Err(NotificationError::Unavailable),
        }
        let member: Option<i64> = sqlx::query_scalar("select revision from public.concert_participants where event_id=$1 and user_id=$2 for share")
            .bind(context.event_id).bind(subject).fetch_optional(&mut *c).await.map_err(db)?;
        revision(member.ok_or(NotificationError::Unavailable)?)?;
    }
    let mut pairs: Vec<_> = subjects
        .into_iter()
        .filter(|id| *id != context.actor_id)
        .map(|id| (context.actor_id.min(id), context.actor_id.max(id)))
        .collect();
    pairs.sort();
    pairs.dedup();
    for (low, high) in pairs {
        let row = sqlx::query("select revision,low_blocks_high,high_blocks_low from public.concert_relationship_pairs where event_id=$1 and low_user=$2 and high_user=$3 for share")
            .bind(context.event_id).bind(low).bind(high).fetch_optional(&mut *c).await.map_err(db)?.ok_or(NotificationError::Unavailable)?;
        revision(row.try_get("revision").map_err(db)?)?;
        if row.try_get::<bool, _>("low_blocks_high").map_err(db)?
            || row.try_get::<bool, _>("high_blocks_low").map_err(db)?
        {
            return Ok(false);
        }
    }
    Ok(true)
}
