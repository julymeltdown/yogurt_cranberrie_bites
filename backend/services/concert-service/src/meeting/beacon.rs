//! Locked current-place selection and DB-time privacy projection.
use super::{grant, invalid, storage, MeetingAuthority};
use crate::adapters::out::repository::place::PgPlaceRepository;
use concert_application::{beacon::*, place::SelectionOutcome};
use concert_domain::{beacon::*, place::PlaceId, EventId, Revision};
use sqlx::{postgres::PgRow, Acquire, PgConnection, PgPool, Row};
use uuid::Uuid;

pub struct PgMeetingRepository<A> {
    pool: PgPool,
    authority: A,
}
impl<A: MeetingAuthority> PgMeetingRepository<A> {
    pub fn new(pool: PgPool, authority: A) -> Self {
        Self { pool, authority }
    }
    pub async fn read(&self, actor: MeetingActor, id: Uuid) -> Result<MeetBeacon, MeetingError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let value = read_in(&mut tx, &self.authority, actor, id).await?;
        tx.commit().await.map_err(storage)?;
        Ok(value)
    }
}

pub(crate) async fn load(
    c: &mut PgConnection,
    event: Uuid,
    id: Uuid,
    write: bool,
) -> Result<MeetBeacon, MeetingError> {
    let lock = if write { "for update" } else { "for share" };
    let row = sqlx::query(&format!(
        "select * from public.concert_meeting_beacons where event_id=$1 and beacon_id=$2 {lock}"
    ))
    .bind(event)
    .bind(id)
    .fetch_optional(c)
    .await
    .map_err(storage)?
    .ok_or(MeetingError::ResourceUnavailable)?;
    project(row)
}
fn project(row: PgRow) -> Result<MeetBeacon, MeetingError> {
    let audience =
        serde_json::from_value(row.try_get("audience").map_err(invalid)?).map_err(invalid)?;
    let meeting = MeetingDescription {
        place_id: row.try_get("place_id").map_err(invalid)?,
        map_revision: row.try_get("map_revision").map_err(invalid)?,
        place_revision: row.try_get("place_revision").map_err(invalid)?,
        instructions: row.try_get("instructions").map_err(invalid)?,
        identification: row.try_get("identification").map_err(invalid)?,
        audience,
        expires_at: row.try_get("expires_at").map_err(invalid)?,
    };
    if !meeting.valid() {
        return Err(MeetingError::InvalidState);
    }
    Ok(MeetBeacon {
        beacon_id: row.try_get("beacon_id").map_err(invalid)?,
        event_id: row.try_get("event_id").map_err(invalid)?,
        owner_id: row.try_get("owner_id").map_err(invalid)?,
        meeting,
        created_at: row.try_get("created_at").map_err(invalid)?,
        updated_at: row.try_get("updated_at").map_err(invalid)?,
        revision: row.try_get("revision").map_err(invalid)?,
        state: match row.try_get::<String, _>("state").map_err(invalid)?.as_str() {
            "ACTIVE" => BeaconState::Active,
            "REVOKED" => BeaconState::Revoked,
            _ => return Err(MeetingError::InvalidState),
        },
    })
}
pub(crate) async fn current(c: &mut PgConnection, value: &MeetBeacon) -> Result<(), MeetingError> {
    let active: bool = sqlx::query_scalar("select $1::timestamptz > clock_timestamp()")
        .bind(value.meeting.expires_at)
        .fetch_one(&mut *c)
        .await
        .map_err(storage)?;
    if value.state != BeaconState::Active || !active {
        return Err(MeetingError::ResourceUnavailable);
    }
    let settings = sqlx::query(
        "select event_cutoff from public.concert_meeting_settings where event_id=$1 for share",
    )
    .bind(value.event_id)
    .fetch_optional(&mut *c)
    .await
    .map_err(storage)?
    .ok_or(MeetingError::PolicyUnavailable)?;
    let before_cutoff: bool = sqlx::query_scalar("select clock_timestamp() < $1")
        .bind(
            settings
                .try_get::<chrono::DateTime<chrono::Utc>, _>("event_cutoff")
                .map_err(invalid)?,
        )
        .fetch_one(&mut *c)
        .await
        .map_err(storage)?;
    if !before_cutoff {
        return Err(MeetingError::ResourceUnavailable);
    }
    let catalog = select_place(c, value.event_id, &value.meeting)
        .await
        .map_err(|error| match error {
            MeetingError::StorageUnavailable => error,
            _ => MeetingError::ResourceUnavailable,
        })?;
    let selected: i64 = sqlx::query_scalar("select selected_catalog_revision from public.concert_meeting_beacons where event_id=$1 and beacon_id=$2")
        .bind(value.event_id).bind(value.beacon_id).fetch_one(c).await.map_err(storage)?;
    if catalog == selected {
        Ok(())
    } else {
        Err(MeetingError::ResourceUnavailable)
    }
}
async fn select_place(
    c: &mut PgConnection,
    event: Uuid,
    m: &MeetingDescription,
) -> Result<i64, MeetingError> {
    // A nested savepoint adapts the concrete BE08 Transaction API to the BE15
    // connection; releasing it keeps SHARE locks until the outer command commits.
    let mut savepoint = c.begin().await.map_err(storage)?;
    let catalog: i64 = sqlx::query_scalar("select catalog_revision from public.concert_events_map_releases where event_id=$1 for share")
        .bind(event).fetch_optional(&mut *savepoint).await.map_err(storage)?.ok_or(MeetingError::ResourceUnavailable)?;
    let outcome = PgPlaceRepository::select_physical_in_transaction(
        &mut savepoint,
        EventId::from_uuid(event),
        PlaceId::from_uuid(m.place_id).map_err(invalid)?,
        Revision::new(catalog).map_err(invalid)?,
        m.map_revision,
    )
    .await
    .map_err(|_| MeetingError::StorageUnavailable)?;
    match outcome {
        SelectionOutcome::Eligible(_) => {}
        SelectionOutcome::StaleCatalog => return Err(MeetingError::RevisionConflict),
        SelectionOutcome::MapMismatch => return Err(MeetingError::MapMismatch),
        _ => return Err(MeetingError::ResourceUnavailable),
    }
    let place_revision: i64 = sqlx::query_scalar("select s.revision from public.concert_place_current_state s join public.concert_events_map_releases b on b.map_id=s.map_id and b.map_revision=s.map_revision where b.event_id=$1 and s.place_id=$2 for share of s")
        .bind(event).bind(m.place_id).fetch_one(&mut *savepoint).await.map_err(storage)?;
    if place_revision != m.place_revision {
        return Err(MeetingError::RevisionConflict);
    }
    savepoint.commit().await.map_err(storage)?;
    Ok(catalog)
}
pub(crate) async fn stored_fence(
    c: &mut PgConnection,
    event: Uuid,
    id: Uuid,
) -> Result<Option<PartyFence>, MeetingError> {
    let row=sqlx::query("select audience->>'party_id' as party,selected_party_meeting_revision as meeting,selected_party_audience_revision as audience from public.concert_meeting_beacons where event_id=$1 and beacon_id=$2")
        .bind(event).bind(id).fetch_one(c).await.map_err(storage)?;
    match (
        row.try_get::<Option<String>, _>("party").map_err(invalid)?,
        row.try_get::<Option<i64>, _>("meeting").map_err(invalid)?,
        row.try_get::<Option<i64>, _>("audience").map_err(invalid)?,
    ) {
        (None, None, None) => Ok(None),
        (Some(party), Some(meeting_revision), Some(audience_revision)) => Ok(Some(PartyFence {
            party: Uuid::parse_str(&party).map_err(invalid)?,
            meeting_revision,
            audience_revision,
        })),
        _ => Err(MeetingError::InvalidState),
    }
}
pub async fn read_in<A: MeetingAuthority>(
    c: &mut PgConnection,
    a: &A,
    actor: MeetingActor,
    id: Uuid,
) -> Result<MeetBeacon, MeetingError> {
    // Actor eligibility is checked before existence and private rows are never projected early.
    a.actor_in(c, actor).await.map_err(|e| match e {
        MeetingError::ResourceUnavailable => e,
        _ => MeetingError::PolicyUnavailable,
    })?;
    let value = load(c, actor.event, id, false).await?;
    if actor.actor != value.owner_id
        && matches!(&value.meeting.audience, Audience::ExplicitUsers { .. })
        && !value.meeting.audience.explicitly_includes(actor.actor)
    {
        return Err(MeetingError::ResourceUnavailable);
    }
    let expected_party = stored_fence(c, actor.event, id).await?;
    grant(
        a,
        c,
        AudienceQuery {
            actor,
            owner: value.owner_id,
            audience: value.meeting.audience.clone(),
            operation: MeetingOperation::Read,
            expected_party,
        },
    )
    .await?;
    current(c, &value).await?;
    // Recheck DB expiry after all potentially blocking locks/authority calls.
    live_time(c, &value).await?;
    Ok(value)
}
pub(crate) async fn live_time(
    c: &mut PgConnection,
    value: &MeetBeacon,
) -> Result<(), MeetingError> {
    let active: bool = sqlx::query_scalar("select clock_timestamp() < least($1,event_cutoff) from public.concert_meeting_settings where event_id=$2")
        .bind(value.meeting.expires_at).bind(value.event_id)
        .fetch_optional(c).await.map_err(storage)?.unwrap_or(false);
    if active && value.state == BeaconState::Active {
        Ok(())
    } else {
        Err(MeetingError::ResourceUnavailable)
    }
}
async fn write_settings(
    c: &mut PgConnection,
    event: Uuid,
    meeting: &MeetingDescription,
) -> Result<(), MeetingError> {
    let phase: Option<String> =
        sqlx::query_scalar("select phase from public.concert_events where event_id=$1 for share")
            .bind(event)
            .fetch_optional(&mut *c)
            .await
            .map_err(storage)?;
    match phase.as_deref() {
        Some("OPEN" | "LIVE") => {}
        Some(_) => return Err(MeetingError::EventReadOnly),
        None => return Err(MeetingError::ResourceUnavailable),
    }
    let row = sqlx::query("select beacon_ttl_seconds,event_cutoff from public.concert_meeting_settings where event_id=$1 for share")
        .bind(event).fetch_optional(&mut *c).await.map_err(storage)?.ok_or(MeetingError::PolicyUnavailable)?;
    let valid: bool = sqlx::query_scalar("select clock_timestamp() < $1 and $1 <= clock_timestamp()+make_interval(secs=>$2::integer) and $1 <= $3")
        .bind(meeting.expires_at).bind(row.try_get::<i32,_>("beacon_ttl_seconds").map_err(invalid)?)
        .bind(row.try_get::<chrono::DateTime<chrono::Utc>,_>("event_cutoff").map_err(invalid)?)
        .fetch_one(c).await.map_err(storage)?;
    if valid {
        Ok(())
    } else {
        Err(MeetingError::InvalidInput)
    }
}

/// Internal effect seam. Caller owns a transaction and its receipt/outbox boundary.
/// Use BeaconCommands for idempotency, not this seam as a public mutation endpoint.
pub async fn mutate_in<A: MeetingAuthority>(
    c: &mut PgConnection,
    a: &A,
    actor: MeetingActor,
    id: Uuid,
    mutation: &BeaconMutation,
) -> Result<BeaconStatus, MeetingError> {
    if id.is_nil() || !mutation.valid() {
        return Err(MeetingError::InvalidInput);
    }
    a.actor_in(c, actor).await?;
    let prior = match mutation {
        BeaconMutation::Create(_) => None,
        _ => Some(load(c, actor.event, id, true).await?),
    };
    if let Some(prior) = &prior {
        if prior.owner_id != actor.actor {
            return Err(MeetingError::ResourceUnavailable);
        }
        let expected = match mutation {
            BeaconMutation::Update(v) => v.expected_revision,
            BeaconMutation::Revoke { expected_revision } => *expected_revision,
            _ => unreachable!(),
        };
        if expected != prior.revision {
            return Err(MeetingError::RevisionConflict);
        }
    }
    if let BeaconMutation::Revoke { .. } = mutation {
        let value = prior.ok_or(MeetingError::InvalidState)?;
        grant(
            a,
            c,
            AudienceQuery {
                actor,
                owner: value.owner_id,
                audience: value.meeting.audience,
                operation: MeetingOperation::Revoke,
                expected_party: None,
            },
        )
        .await?;
        if value.state == BeaconState::Revoked {
            return Err(MeetingError::RevisionConflict);
        }
        let revision: i64 = sqlx::query_scalar("update public.concert_meeting_beacons set state='REVOKED',revision=revision+1 where event_id=$1 and beacon_id=$2 returning revision")
            .bind(actor.event).bind(id).fetch_one(c).await.map_err(storage)?;
        return Ok(BeaconStatus {
            beacon_id: id,
            event_id: actor.event,
            state: BeaconState::Revoked,
            revision,
        });
    }
    let (meeting, op) = match mutation {
        BeaconMutation::Create(v) => (&v.meeting, MeetingOperation::Create),
        BeaconMutation::Update(v) => (&v.meeting, MeetingOperation::Update),
        _ => unreachable!(),
    };
    let party_fence = grant(
        a,
        c,
        AudienceQuery {
            actor,
            owner: actor.actor,
            audience: meeting.audience.clone(),
            operation: op,
            expected_party: None,
        },
    )
    .await?;
    write_settings(c, actor.event, meeting).await?;
    let catalog = select_place(c, actor.event, meeting).await?;
    if let Some(value) = &prior {
        live_time(c, value).await?;
    }
    let audience = serde_json::to_value(&meeting.audience).map_err(invalid)?;
    let query = if prior.is_some() {
        "update public.concert_meeting_beacons set place_id=$4,map_revision=$5,place_revision=$6,instructions=$7,identification=$8,audience=$9,expires_at=$10,selected_catalog_revision=$11,selected_party_meeting_revision=$12,selected_party_audience_revision=$13,revision=revision+1 where event_id=$2 and beacon_id=$1 and owner_id=$3 returning revision"
    } else {
        "insert into public.concert_meeting_beacons(beacon_id,event_id,owner_id,place_id,map_revision,place_revision,instructions,identification,audience,expires_at,selected_catalog_revision,selected_party_meeting_revision,selected_party_audience_revision) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) returning revision"
    };
    let revision: i64 = sqlx::query_scalar(query)
        .bind(id)
        .bind(actor.event)
        .bind(actor.actor)
        .bind(meeting.place_id)
        .bind(meeting.map_revision)
        .bind(meeting.place_revision)
        .bind(&meeting.instructions)
        .bind(&meeting.identification)
        .bind(audience)
        .bind(meeting.expires_at)
        .bind(catalog)
        .bind(party_fence.map(|f| f.meeting_revision))
        .bind(party_fence.map(|f| f.audience_revision))
        .fetch_one(c)
        .await
        .map_err(storage)?;
    Ok(BeaconStatus {
        beacon_id: id,
        event_id: actor.event,
        state: BeaconState::Active,
        revision,
    })
}
