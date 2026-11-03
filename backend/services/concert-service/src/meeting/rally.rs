//! Member-only current meeting reference and explicit self-state commands.
use super::{
    commands::{canonical, command_error, safe_failure},
    invalid, storage, MeetingAuthority,
};
use crate::adapters::out::repository::{
    place::PgPlaceRepository,
    receipt::{CommandProducer, PgCommandRepository},
};
use async_trait::async_trait;
use concert_application::{beacon::*, commands::*, place::SelectionOutcome, rally::*};
use concert_domain::{place::PlaceId, rally::*, EventId, Revision};
use sqlx::{postgres::PgRow, Acquire, PgConnection, PgPool, Row};
use uuid::Uuid;

#[async_trait]
pub trait RallyAuthority: MeetingAuthority {
    /// Resolve exact current party authority under community's caller-transaction
    /// seam; OPEN/CLOSED only, actor/owner active, full current member generations.
    /// Locks persist through commit. Never synthesize a beacon association.
    async fn party_in(
        &self,
        c: &mut PgConnection,
        q: RallyQuery,
    ) -> Result<PartyMeeting, MeetingError>;
    /// Fresh BE37 actor/subject policy plus current party membership.
    /// Denied peers are omitted; policy outages fail the entire projection closed.
    async fn peer_in(
        &self,
        c: &mut PgConnection,
        q: RallyQuery,
        peer: Uuid,
    ) -> Result<(), MeetingError>;
}
async fn party<A: RallyAuthority>(
    c: &mut PgConnection,
    a: &A,
    q: RallyQuery,
) -> Result<PartyMeeting, MeetingError> {
    a.actor_in(c, q.actor).await?;
    let grant = a.party_in(c, q).await?;
    if !grant.valid_for(q) {
        return Err(MeetingError::ResourceUnavailable);
    }
    a.peer_in(c, q, grant.owner).await?;
    Ok(grant)
}
async fn active_meeting(c: &mut PgConnection, g: &PartyMeeting) -> Result<(), MeetingError> {
    let mut tx = c.begin().await.map_err(storage)?;
    let binding=sqlx::query("select catalog_revision,map_revision from public.concert_events_map_releases where event_id=$1 for share")
        .bind(g.query.actor.event).fetch_optional(&mut *tx).await.map_err(storage)?.ok_or(MeetingError::ResourceUnavailable)?;
    let selected = PgPlaceRepository::select_physical_in_transaction(
        &mut tx,
        EventId::from_uuid(g.query.actor.event),
        PlaceId::from_uuid(g.public_place_id).map_err(invalid)?,
        Revision::new(binding.try_get("catalog_revision").map_err(invalid)?).map_err(invalid)?,
        binding.try_get("map_revision").map_err(invalid)?,
    )
    .await
    .map_err(|_| MeetingError::StorageUnavailable)?;
    if !matches!(selected, SelectionOutcome::Eligible(_)) {
        return Err(MeetingError::ResourceUnavailable);
    }
    let cutoff = sqlx::query(
        "select event_cutoff from public.concert_meeting_settings where event_id=$1 for share",
    )
    .bind(g.query.actor.event)
    .fetch_optional(&mut *tx)
    .await
    .map_err(storage)?
    .ok_or(MeetingError::PolicyUnavailable)?;
    let live: bool = sqlx::query_scalar("select clock_timestamp() < $1 and clock_timestamp() < $2")
        .bind(g.ends_at)
        .bind(
            cutoff
                .try_get::<chrono::DateTime<chrono::Utc>, _>("event_cutoff")
                .map_err(invalid)?,
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
    if !live {
        return Err(MeetingError::ResourceUnavailable);
    }
    tx.commit().await.map_err(storage)
}
fn entry(row: &PgRow) -> Result<RallyEntry, MeetingError> {
    let state = match row.try_get::<String, _>("state").map_err(invalid)?.as_str() {
        "ON_MY_WAY" => RallyState::OnMyWay,
        "HERE" => RallyState::Here,
        "MET" => RallyState::Met,
        "NEED_HELP" => RallyState::NeedHelp,
        "LEFT" => RallyState::Left,
        _ => return Err(MeetingError::InvalidState),
    };
    Ok(RallyEntry {
        user_id: row.try_get("user_id").map_err(invalid)?,
        state,
        meeting_revision: row.try_get("meeting_revision").map_err(invalid)?,
        revision: row.try_get("revision").map_err(invalid)?,
        self_reported_at: row.try_get("self_reported_at").map_err(invalid)?,
    })
}
pub struct PgRally<A> {
    pool: PgPool,
    authority: A,
}
impl<A: RallyAuthority> PgRally<A> {
    pub fn new(pool: PgPool, authority: A) -> Self {
        Self { pool, authority }
    }
    pub async fn read(&self, q: RallyQuery) -> Result<Rally, MeetingError> {
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let g = party(&mut tx, &self.authority, q).await?;
        active_meeting(&mut tx, &g).await?;
        let rows=sqlx::query("select * from public.concert_meeting_rally_states where event_id=$1 and party_id=$2 and user_id=any($3) order by user_id limit 51 for share")
            .bind(q.actor.event).bind(q.party).bind(g.members.iter().map(|m|m.user).collect::<Vec<_>>()).fetch_all(&mut *tx).await.map_err(storage)?;
        // Reports for removed or newly reapproved membership generations are never restored.
        let mut entries = vec![];
        for row in rows {
            let deadline: chrono::DateTime<chrono::Utc> =
                row.try_get("meeting_deadline").map_err(invalid)?;
            // Forward-upgraded legacy reports have no qualified physical deadline.
            // Preserve revision metadata but do not restore their arrival state.
            if deadline.timestamp() <= 0 {
                continue;
            }
            let value = entry(&row)?;
            if value.meeting_revision > g.meeting_revision {
                return Err(MeetingError::InvalidState);
            }
            let generation: i64 = row.try_get("membership_revision").map_err(invalid)?;
            if !g
                .members
                .iter()
                .any(|m| m.user == value.user_id && m.membership_revision == generation)
            {
                continue;
            }
            match self.authority.peer_in(&mut tx, q, value.user_id).await {
                Ok(()) => entries.push(value),
                Err(MeetingError::ResourceUnavailable) => {}
                Err(_) => return Err(MeetingError::PolicyUnavailable),
            }
        }
        // Expiry is checked after all potentially blocking peer policy checks.
        active_meeting(&mut tx, &g).await?;
        super::ensure_authority_current(&mut tx).await?;
        tx.commit().await.map_err(storage)?;
        Ok(Rally {
            party_id: q.party,
            event_id: q.actor.event,
            meeting_revision: g.meeting_revision,
            beacon_id: None,
            entries,
        })
    }
    pub async fn execute(
        &self,
        q: RallyQuery,
        key: Uuid,
        input: SetRally,
        keys: &FingerprintKeys,
    ) -> Result<SafeReceipt, CommandError> {
        if !input.valid() || q.party.is_nil() {
            return Err(CommandError::InvalidInput);
        }
        let scope = Scope {
            actor: q.actor.actor,
            event: q.actor.event,
            key,
            operation: RegisteredOperation::find("setConcertRally")?,
        };
        let event = q.actor.event.to_string();
        let party = q.party.to_string();
        let body =
            canonical(&serde_json::to_value(&input).map_err(|_| CommandError::InvalidInput)?)?;
        let request = CanonicalRequest::json(
            scope,
            &[("eventId", &event), ("partyId", &party)],
            &[],
            Some(&body),
        )?;
        PgCommandRepository::new(self.pool.clone())
            .execute(
                &request,
                q.actor.actor,
                keys,
                &RallyProducer {
                    authority: &self.authority,
                    q,
                    input,
                },
            )
            .await
    }
}
struct RallyProducer<'a, A> {
    authority: &'a A,
    q: RallyQuery,
    input: SetRally,
}
#[async_trait]
impl<A: RallyAuthority> CommandProducer for RallyProducer<'_, A> {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if scope.actor != self.q.actor.actor
            || scope.event != self.q.actor.event
            || target != self.q.actor.actor
            || scope.operation.id() != "setConcertRally"
        {
            return Err(CommandError::Denied);
        }
        party(c, self.authority, self.q)
            .await
            .map_err(command_error)?;
        Ok(())
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        _: Scope,
        _: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        match set_in(c, self.authority, self.q, &self.input).await {
            Ok(value) => {
                let reference = SafeReference {
                    resource: value.user_id,
                    revision: value.revision,
                };
                Ok((
                    Completion::Succeeded {
                        reference,
                        status: 200,
                    },
                    Some(Invalidation { reference }),
                ))
            }
            Err(error) => safe_failure(error),
        }
    }
}
/// Actor identity comes only from the authenticated context, never the request body.
pub async fn set_in<A: RallyAuthority>(
    c: &mut PgConnection,
    a: &A,
    q: RallyQuery,
    input: &SetRally,
) -> Result<RallyEntry, MeetingError> {
    if !input.valid() {
        return Err(MeetingError::InvalidInput);
    }
    let g = party(c, a, q).await?;
    let phase: String =
        sqlx::query_scalar("select phase from public.concert_events where event_id=$1 for share")
            .bind(q.actor.event)
            .fetch_one(&mut *c)
            .await
            .map_err(storage)?;
    if !matches!(phase.as_str(), "OPEN" | "LIVE") {
        return Err(MeetingError::EventReadOnly);
    }
    if g.meeting_revision != input.meeting_revision {
        return Err(MeetingError::RevisionConflict);
    }
    active_meeting(c, &g).await?;
    // Namespace absence locks by exact event+party+actor; concurrent first reports
    // cannot both observe revision zero, even outside the receipt admission lock.
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1,0))")
        .bind(format!(
            "mcr-rally:{}:{}:{}",
            q.actor.event, q.party, q.actor.actor
        ))
        .execute(&mut *c)
        .await
        .map_err(storage)?;
    let prior=sqlx::query("select * from public.concert_meeting_rally_states where event_id=$1 and party_id=$2 and user_id=$3 for update")
        .bind(q.actor.event).bind(q.party).bind(q.actor.actor).fetch_optional(&mut *c).await.map_err(storage)?;
    let revision = prior
        .as_ref()
        .map(|r| r.try_get::<i64, _>("revision"))
        .transpose()
        .map_err(invalid)?
        .unwrap_or(0);
    let membership = g
        .members
        .iter()
        .find(|m| m.user == q.actor.actor)
        .ok_or(MeetingError::ResourceUnavailable)?
        .membership_revision;
    let visible_revision = match &prior {
        Some(row)
            if row
                .try_get::<i64, _>("membership_revision")
                .map_err(invalid)?
                != membership
                || row
                    .try_get::<chrono::DateTime<chrono::Utc>, _>("meeting_deadline")
                    .map_err(invalid)?
                    .timestamp()
                    <= 0 =>
        {
            0
        }
        _ => revision,
    };
    if visible_revision != input.expected_revision {
        return Err(MeetingError::RevisionConflict);
    }
    // Recheck DB expiry after advisory/row locks; no waiting command can report against an expired meeting.
    active_meeting(c, &g).await?;
    let row=sqlx::query("insert into public.concert_meeting_rally_states(event_id,party_id,user_id,state,meeting_revision,membership_revision,meeting_deadline) values($1,$2,$3,$4,$5,$6,$7) on conflict(event_id,party_id,user_id) do update set state=excluded.state,meeting_revision=excluded.meeting_revision,membership_revision=excluded.membership_revision,meeting_deadline=excluded.meeting_deadline,revision=concert_meeting_rally_states.revision+1 returning *")
        .bind(q.actor.event).bind(q.party).bind(q.actor.actor).bind(input.state.as_str()).bind(input.meeting_revision).bind(membership).bind(g.ends_at).fetch_one(c).await.map_err(storage)?;
    entry(&row)
}
