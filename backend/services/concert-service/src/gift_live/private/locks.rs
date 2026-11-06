//! Local Gift owner locks and disclosure projection. No canonical allow fixture.
use super::{native, GiftPrivateRequest};
use crate::{
    adapters::out::relationship_authority::guarded::GiftPrivateKind,
    community::{
        giveaway::{IssuedCode, PickupView},
        policy::CommunityResources,
        CommunityAccess,
    },
    gift_live::{
        queries,
        resources::{current_settings_in, select_place, GiftMediaLink, PgGiftResources},
    },
};
use chrono::{DateTime, Utc};
use concert_application::{commands::CommandError, community::giveaway::PickupKeys};
use concert_domain::community::giveaway::ReservationState;
use sqlx::{PgConnection, Row};
use tonic::Status;
use uuid::Uuid;

struct NoMedia;
#[async_trait::async_trait]
impl GiftMediaLink for NoMedia {
    async fn link_in(
        &self,
        _: &mut PgConnection,
        _: super::GiftIdentity,
        _: Uuid,
        _: Uuid,
    ) -> Result<(), CommandError> {
        Err(CommandError::Denied)
    }
}
fn denied() -> Status {
    Status::permission_denied("Gift private disclosure denied")
}
fn db(_: sqlx::Error) -> Status {
    native::unavailable()
}
fn command(e: CommandError) -> Status {
    if e == CommandError::Denied {
        denied()
    } else {
        native::unavailable()
    }
}
pub(super) struct RootHeader {
    pub owner: Uuid,
    revision: i64,
    meeting_place: Uuid,
    meeting_map: i64,
    meeting_revision: i64,
    meeting_expires: DateTime<Utc>,
    pickup_starts: DateTime<Utc>,
}
pub(super) struct Header {
    pub until: i64,
    root: RootHeader,
    reservation_revision: i64,
    reservation_state: String,
    reservation_expires: DateTime<Utc>,
    code_expires: Option<DateTime<Utc>>,
    code_generation: i64,
}
pub(super) async fn root_in(
    c: &mut PgConnection,
    request: &GiftPrivateRequest,
    giveaway: Uuid,
) -> Result<RootHeader, Status> {
    let i = request.identity;
    let op = match request.kind {
        GiftPrivateKind::Pickup => super::GiftOperation::Pickup,
        GiftPrivateKind::Code => super::GiftOperation::IssueCode,
    };
    PgGiftResources::for_status(i, op, NoMedia)
        .feature_in(c, i.event, i.actor, CommunityAccess::ReadGifts)
        .await
        .map_err(command)?;
    // Metadata only. No aggregate, private instructions/identification or secret
    // hash enters memory before the durable typed child intent acknowledgement.
    let row=sqlx::query("select giveaway_id,owner_id,state,revision,(aggregate->'fields'->>'public_place_id')::uuid public_place_id,(aggregate->'fields'->>'pickup_starts_at')::timestamptz pickup_starts_at,(aggregate->'meeting'->>'place_id')::uuid place_id,(aggregate->'meeting'->>'map_revision')::bigint map_revision,(aggregate->'meeting'->>'place_revision')::bigint place_revision,(aggregate->'meeting'->>'expires_at')::timestamptz meeting_expires_at from concert_community_giveaways where event_id=$1 and giveaway_id=$2 for share")
        .bind(i.event).bind(giveaway).fetch_optional(c).await.map_err(db)?.ok_or_else(denied)?;
    let state: String = row.try_get("state").map_err(db)?;
    let owner: Uuid = row.try_get("owner_id").map_err(db)?;
    let meeting_place = row
        .try_get::<Option<Uuid>, _>("place_id")
        .map_err(db)?
        .ok_or_else(denied)?;
    if !matches!(state.as_str(), "OPEN" | "PAUSED")
        || owner == i.actor
        || owner.is_nil()
        || row.try_get::<Uuid, _>("public_place_id").map_err(db)? != meeting_place
    {
        return Err(denied());
    }
    Ok(RootHeader {
        owner,
        revision: row.try_get("revision").map_err(db)?,
        meeting_place,
        meeting_map: row
            .try_get::<Option<i64>, _>("map_revision")
            .map_err(db)?
            .ok_or_else(denied)?,
        meeting_revision: row
            .try_get::<Option<i64>, _>("place_revision")
            .map_err(db)?
            .ok_or_else(denied)?,
        meeting_expires: row
            .try_get::<Option<DateTime<Utc>>, _>("meeting_expires_at")
            .map_err(db)?
            .ok_or_else(denied)?,
        pickup_starts: row.try_get("pickup_starts_at").map_err(db)?,
    })
}
pub(super) async fn metadata_in(
    c: &mut PgConnection,
    request: &GiftPrivateRequest,
    giveaway: Uuid,
    root: RootHeader,
    issued: Option<&IssuedCode>,
) -> Result<Header, Status> {
    let i = request.identity;
    // A real reservation's mutation policy has already materialized this pair.
    // Missing metadata fails closed; a prepared read must not create pair state
    // or enter a generic mutation/deferred-commit authority path.
    local_visibility_in(c, i.event, i.actor, root.owner).await?;
    let r=sqlx::query("select recipient_id,state,revision,expires_at from concert_community_reservations where event_id=$1 and giveaway_id=$2 and reservation_id=$3 for share")
        .bind(i.event).bind(giveaway).bind(request.reservation).fetch_optional(&mut *c).await.map_err(db)?.ok_or_else(denied)?;
    let state: String = r.try_get("state").map_err(db)?;
    if r.try_get::<Uuid, _>("recipient_id").map_err(db)? != i.actor
        || !matches!(state.as_str(), "HELD" | "READY")
        || (request.kind == GiftPrivateKind::Code && state != "READY")
    {
        return Err(denied());
    }
    let revision: i64 = r.try_get("revision").map_err(db)?;
    let expires: DateTime<Utc> = r.try_get("expires_at").map_err(db)?;
    let metadata=sqlx::query("select (j.value->>'expires_at')::timestamptz expires_at,(j.value->>'code_expires_at')::timestamptz code_expires_at,(j.value->>'code_generation')::bigint code_generation from concert_community_giveaways g cross join lateral jsonb_array_elements(g.aggregate->'reservations') j where g.event_id=$1 and g.giveaway_id=$2 and (j.value->>'id')::uuid=$3")
        .bind(i.event).bind(giveaway).bind(request.reservation).fetch_all(&mut *c).await.map_err(db)?;
    if metadata.len() != 1
        || metadata[0]
            .try_get::<DateTime<Utc>, _>("expires_at")
            .map_err(db)?
            != expires
    {
        return Err(native::unavailable());
    }
    let code_expires: Option<DateTime<Utc>> = metadata[0].try_get("code_expires_at").map_err(db)?;
    let generation: i64 = metadata[0].try_get("code_generation").map_err(db)?;
    let settings = current_settings_in(c, i.event).await.map_err(command)?;
    if root.meeting_expires > settings.event_cutoff {
        return Err(denied());
    }
    select_place(
        c,
        i.event,
        root.meeting_place,
        Some((root.meeting_map, root.meeting_revision)),
    )
    .await
    .map_err(command)?;
    let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(&mut *c)
        .await
        .map_err(db)?;
    let mut until = i
        .deadline
        .expires_micros()
        .min(expires.timestamp_micros())
        .min(root.meeting_expires.timestamp_micros())
        .min(settings.event_cutoff.timestamp_micros());
    if request.kind == GiftPrivateKind::Code {
        let actual = code_expires.ok_or_else(denied)?;
        let issued = issued.ok_or_else(denied)?;
        if generation < 1
            || issued.revision != revision
            || issued.expires_at != actual
            || issued.reservation != request.reservation
            || now < root.pickup_starts
        {
            return Err(denied());
        }
        until = until.min(actual.timestamp_micros());
    } else if issued.is_some() {
        return Err(denied());
    }
    if until <= now.timestamp_micros() {
        return Err(Status::deadline_exceeded("Gift disclosure expired"));
    }
    native::budget(c, i.deadline, until).await?;
    Ok(Header {
        until,
        root,
        reservation_revision: revision,
        reservation_state: state,
        reservation_expires: expires,
        code_expires,
        code_generation: generation,
    })
}
/// Actual pair + admission visibility used by the Gift-private metadata read.
pub(super) async fn local_visibility_in(
    c: &mut PgConnection,
    event: Uuid,
    actor: Uuid,
    owner: Uuid,
) -> Result<(), Status> {
    let (low, high) = if actor < owner {
        (actor, owner)
    } else {
        (owner, actor)
    };
    let pair=sqlx::query("select revision,low_blocks_high,high_blocks_low from concert_relationship_pairs where event_id=$1 and low_user=$2 and high_user=$3 for share")
        .bind(event).bind(low).bind(high).fetch_one(&mut *c).await.map_err(db)?;
    if pair.try_get::<i64, _>("revision").map_err(db)? < 1
        || pair.try_get::<bool, _>("low_blocks_high").map_err(db)?
        || pair.try_get::<bool, _>("high_blocks_low").map_err(db)?
    {
        return Err(denied());
    }
    for user in [low, high] {
        let state: Option<String> = sqlx::query_scalar(
            "select state from concert_event_admissions where event_id=$1 and user_id=$2 for share",
        )
        .bind(event)
        .bind(user)
        .fetch_optional(&mut *c)
        .await
        .map_err(db)?;
        let member: Option<Uuid> = sqlx::query_scalar(
            "select user_id from concert_participants where event_id=$1 and user_id=$2 for share",
        )
        .bind(event)
        .bind(user)
        .fetch_optional(&mut *c)
        .await
        .map_err(db)?;
        if state.as_deref() != Some("ACTIVE") || member != Some(user) {
            return Err(denied());
        }
    }
    Ok(())
}

#[cfg(test)]
pub(crate) async fn probe_visible_in(
    c: &mut PgConnection,
    event: Uuid,
    actor: Uuid,
    owner: Uuid,
) -> bool {
    let _ = sqlx::query("select set_config('mcr.relationship_authority_until',(clock_timestamp()+interval '1 minute')::text,false)").execute(&mut *c).await;
    local_visibility_in(c, event, actor, owner).await.is_ok()
}

/// Called only AFTER durable SDK register_child_intent has acknowledged. Every
/// row was already locked; this is the first private content/secret SELECT.
pub(super) async fn content_in(
    c: &mut PgConnection,
    request: &GiftPrivateRequest,
    giveaway: Uuid,
    header: &Header,
    issued: Option<&IssuedCode>,
    keys: &PickupKeys,
) -> Result<PickupView, Status> {
    let i = request.identity;
    let row = sqlx::query(
        "select * from concert_community_giveaways where event_id=$1 and giveaway_id=$2 for share",
    )
    .bind(i.event)
    .bind(giveaway)
    .fetch_one(&mut *c)
    .await
    .map_err(db)?;
    let g = queries::parse(&row, i.event).map_err(|_| native::unavailable())?;
    let r = g
        .reservations
        .iter()
        .find(|r| r.id == request.reservation && r.recipient == i.actor)
        .ok_or_else(denied)?;
    let meeting = g.meeting.as_ref().ok_or_else(denied)?;
    if g.owner != header.root.owner
        || g.revision != header.root.revision
        || !g.state.active()
        || r.revision != header.reservation_revision
        || r.expires_at != header.reservation_expires
        || serde_json::to_value(r.state).map_err(|_| native::unavailable())?
            != header.reservation_state
        || !r.state.active()
        || r.code_generation != header.code_generation
        || r.code_expires_at != header.code_expires
        || meeting.place_id != header.root.meeting_place
        || meeting.map_revision != header.root.meeting_map
        || meeting.place_revision != header.root.meeting_revision
        || meeting.expires_at != header.root.meeting_expires
    {
        return Err(native::unavailable());
    }
    if request.kind == GiftPrivateKind::Code {
        let issued = issued.ok_or_else(denied)?;
        if r.state != ReservationState::Ready {
            return Err(denied());
        }
        let secret=sqlx::query("select generation,key_id,keyed_hash,expires_at from concert_community_pickup_secrets where event_id=$1 and reservation_id=$2 for share")
            .bind(i.event).bind(request.reservation).fetch_optional(&mut *c).await.map_err(db)?.ok_or_else(denied)?;
        let expiry = secret
            .try_get::<DateTime<Utc>, _>("expires_at")
            .map_err(db)?;
        if secret.try_get::<i64, _>("generation").map_err(db)? != r.code_generation
            || Some(expiry) != r.code_expires_at
            || expiry != issued.expires_at
            || !keys
                .verify(
                    &secret.try_get::<String, _>("key_id").map_err(db)?,
                    &secret.try_get::<Vec<u8>, _>("keyed_hash").map_err(db)?,
                    i.event,
                    r.id,
                    r.code_generation,
                    expiry,
                    &issued.code,
                )
                .map_err(command)?
        {
            return Err(denied());
        }
    }
    native::budget(c, i.deadline, header.until).await?;
    Ok(PickupView {
        reservation: r.clone(),
        meeting: meeting.clone(),
        meeting_revision: g.meeting_revision,
    })
}
