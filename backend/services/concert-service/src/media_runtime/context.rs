//! Shared media policy capture. Produces a binding, never authorization or a lease.
use super::OwnerMediaIdentity;
use concert_application::{
    commands::CommandError,
    media::{Context, PolicyFence},
};
use sqlx::PgConnection;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum OwnerMediaContextMode {
    Mutation,
    OwnerReadOrRecovery,
}

/// Capture the uploader's exact immutable media policy semantics on the caller's
/// actual transaction: event revision, admission restriction revision, and real
/// per-actor security generation. A BE37 relationship-pair stamp is not this value.
///
/// The caller must separately retain canonical account/session exclusion through
/// commit, durably Track this transaction before effects, and implement qualified
/// MediaAuthority target/purpose/audience checks. This helper grants none of those.
/// SHARE locks remain on c through the caller's commit/rollback. Do not detach the
/// result for later use or replace authority_revision with a default/global counter.
pub async fn capture_owner_context_in(
    c: &mut PgConnection,
    identity: OwnerMediaIdentity,
    authority_revision: i64,
    mode: OwnerMediaContextMode,
) -> Result<Context, CommandError> {
    identity.check_current()?;
    if !(1..=9007199254740991).contains(&authority_revision) {
        return Err(CommandError::Denied);
    }
    let i = identity;
    // Same event-first order as PgMedia. SHARE locks exclude phase, suspension
    // and participant deletion changes through commit, not just a snapshot.
    let event: Option<(String, i64)> = sqlx::query_as(
        "select phase,revision from public.concert_events where event_id=$1 for share",
    )
    .bind(i.event)
    .fetch_optional(&mut *c)
    .await
    .map_err(|_| CommandError::Unavailable)?;
    let (phase, event_revision) = event.ok_or(CommandError::Denied)?;
    let readable = mode == OwnerMediaContextMode::OwnerReadOrRecovery;
    if !matches!(phase.as_str(), "OPEN" | "LIVE") && !(readable && phase == "READ_ONLY") {
        return Err(CommandError::Denied);
    }
    let admission: Option<(String,i64)> = sqlx::query_as("select state,restriction_revision from public.concert_event_admissions where event_id=$1 and user_id=$2 for share")
            .bind(i.event).bind(i.actor).fetch_optional(&mut *c).await.map_err(|_| CommandError::Unavailable)?;
    let (state, restriction) = admission.ok_or(CommandError::Denied)?;
    if state != "ACTIVE" {
        return Err(CommandError::Denied);
    }
    let participant: Option<i64> = sqlx::query_scalar("select revision from public.concert_participants where event_id=$1 and user_id=$2 for share")
            .bind(i.event).bind(i.actor).fetch_optional(&mut *c).await.map_err(|_| CommandError::Unavailable)?;
    if participant.is_none() {
        return Err(CommandError::Denied);
    }
    let context = Context {
        actor: i.actor,
        event: i.event,
        policy: PolicyFence {
            event_revision,
            relationship_revision: restriction,
            authority_revision,
        },
    }
    .validate()?;
    identity.check_current()?;
    Ok(context)
}
