//! Internal event-close effect, not an impersonated participant command.
//! Uses existing checked domain transitions and canonical root/child serializers.
use super::{giveaway, load, save};
use crate::event_lifecycle::{
    read_only::{require_new_barrier_in, require_scope_in},
    EffectKey,
};
use concert_application::commands::CommandError;
use concert_domain::community::{
    giveaway::{GiftAction, GiftState, ReservationState},
    party::{ApplicationState, PartyAction},
};
use sqlx::{PgConnection, Row};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CommunityReadOnlyReceipt {
    pub event: Uuid,
    pub job: Uuid,
    pub operation: Uuid,
    pub receipt_id: Uuid,
    pub revision: i64,
}
fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
fn invalid(_: impl std::fmt::Display) -> CommandError {
    CommandError::InvalidState
}
fn decode(
    row: &sqlx::postgres::PgRow,
    key: EffectKey,
) -> Result<CommunityReadOnlyReceipt, CommandError> {
    let result = CommunityReadOnlyReceipt {
        event: row.try_get("event_id").map_err(db)?,
        job: row.try_get("job_id").map_err(db)?,
        operation: row.try_get("operation_id").map_err(db)?,
        receipt_id: row.try_get("receipt_id").map_err(db)?,
        revision: row.try_get("revision").map_err(db)?,
    };
    if result.event != key.event
        || result.job != key.job
        || result.operation != key.operation_id()
        || result.receipt_id.is_nil()
        || result.revision != 1
    {
        return Err(CommandError::InvalidState);
    }
    Ok(result)
}
/// Caller owns the exclusive event transaction and actual operator authority.
/// No pool/commit, client owner claim, relationship grant or fabricated BE15 receipt.
pub async fn fence_event_read_only_in(
    c: &mut PgConnection,
    key: EffectKey,
) -> Result<CommunityReadOnlyReceipt, CommandError> {
    require_scope_in(c, key).await?;
    if let Some(row) =
        sqlx::query("select * from concert_closure_community_barriers where job_id=$1")
            .bind(key.job)
            .fetch_optional(&mut *c)
            .await
            .map_err(db)?
    {
        return decode(&row, key);
    }
    require_new_barrier_in(c, key).await?;
    // Propagate, never extend, the already verified lifecycle transaction deadline.
    sqlx::query("select set_config('mcr.community_authority_until',least(nullif(current_setting('mcr.lifecycle_authority_until',true),'')::timestamptz,coalesce(nullif(current_setting('mcr.community_authority_until',true),'')::timestamptz,'infinity'::timestamptz))::text,true)")
        .execute(&mut *c).await.map_err(db)?;
    let at = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&mut *c)
        .await
        .map_err(db)?;
    let parties: Vec<Uuid> = sqlx::query_scalar("select party_id from concert_community_parties where event_id=$1 order by party_id for update")
        .bind(key.event).fetch_all(&mut *c).await.map_err(db)?;
    let mut party_count = 0_i64;
    let mut application_count = 0_i64;
    for id in parties {
        let mut party = load(c, key.event, id, true)
            .await?
            .ok_or(CommandError::InvalidState)?;
        if !party.state.active() {
            continue;
        }
        application_count += party
            .applications
            .iter()
            .filter(|a| a.state == ApplicationState::Pending)
            .count() as i64;
        // Existing owner parameter enforces aggregate invariants only. The authority
        // here is the locked operator event-close command, not a synthetic owner session.
        party
            .change(
                party.owner,
                &PartyAction::Cancel {
                    expected: party.revision,
                },
                Uuid::nil(),
                at,
            )
            .map_err(invalid)?;
        save(c, &party).await?;
        party_count += 1;
    }
    let gifts: Vec<Uuid> = sqlx::query_scalar("select giveaway_id from concert_community_giveaways where event_id=$1 order by giveaway_id for update")
        .bind(key.event).fetch_all(&mut *c).await.map_err(db)?;
    let mut gift_count = 0_i64;
    let mut reservation_count = 0_i64;
    for id in gifts {
        let mut gift = giveaway::load(c, key.event, id, true, false)
            .await?
            .ok_or(CommandError::InvalidState)?;
        if matches!(gift.state, GiftState::Closed | GiftState::Cancelled) {
            continue;
        }
        reservation_count += gift
            .reservations
            .iter()
            .filter(|r| matches!(r.state, ReservationState::Held | ReservationState::Ready))
            .count() as i64;
        gift.change(
            gift.owner,
            &GiftAction::Update {
                expected: gift.revision,
                fields: gift.fields.clone(),
                state: GiftState::Cancelled,
            },
            Uuid::nil(),
            at,
        )
        .map_err(invalid)?;
        giveaway::save(c, &gift).await?;
        gift_count += 1;
    }
    // Fail closed on inconsistent/unserialized children; never silently certify
    // a partial scan or normalize counters independently of the owning aggregate.
    let residual: bool = sqlx::query_scalar("select exists(select 1 from concert_community_parties where event_id=$1 and state in ('OPEN','CLOSED')) or exists(select 1 from concert_community_applications where event_id=$1 and state='PENDING') or exists(select 1 from concert_community_giveaways where event_id=$1 and state not in ('CLOSED','CANCELLED')) or exists(select 1 from concert_community_reservations where event_id=$1 and state in ('HELD','READY')) or exists(select 1 from concert_community_giveaways where event_id=$1 and reserved_unclaimed<>0) or exists(select 1 from concert_community_pickup_secrets where event_id=$1)")
        .bind(key.event).fetch_one(&mut *c).await.map_err(db)?;
    if residual {
        return Err(CommandError::InvalidState);
    }
    let row = sqlx::query("insert into concert_closure_community_barriers(event_id,job_id,operation_id,receipt_id,revision,parties_cancelled,applications_cancelled,giveaways_cancelled,reservations_cancelled) values($1,$2,$3,$4,1,$5,$6,$7,$8) returning *")
        .bind(key.event).bind(key.job).bind(key.operation_id()).bind(Uuid::new_v4())
        .bind(party_count).bind(application_count).bind(gift_count).bind(reservation_count)
        .fetch_one(c).await.map_err(db)?;
    decode(&row, key)
}
