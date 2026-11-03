pub mod audience;
// Internal transactional community adapter. Public transport and runtime composition are separate.
use crate::adapters::out::repository::receipt::{CommandProducer, PgCommandRepository};
use async_trait::async_trait;
use concert_application::{commands::*, community::PartyCommand};
use concert_domain::community::party::*;
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Mutex;
use uuid::Uuid;

/// Temporary composition port, not a second policy engine. An implementation must call
/// BE37 authorize_in for each exact actor/subject/event and hold its fence through commit.
/// It must also fence current canonical session eligibility/feature policy. No default grant.
#[async_trait]
pub trait CommunityAuthority: Send + Sync {
    async fn authorize_in(
        &self,
        connection: &mut PgConnection,
        actor: Uuid,
        event: Uuid,
        subject: Uuid,
        safety: bool,
    ) -> Result<(), CommandError>;
    /// Resolve BE08's current published/open reviewed place, holding its binding/state locks
    /// through commit. A profile or a place UUID's existence is never permission.
    async fn place_in(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        place: Uuid,
    ) -> Result<(), CommandError>;
}
pub struct UnavailableAuthority;
#[async_trait]
impl CommunityAuthority for UnavailableAuthority {
    async fn authorize_in(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        _: Uuid,
        _: Uuid,
        _: bool,
    ) -> Result<(), CommandError> {
        Err(CommandError::Unavailable)
    }
    async fn place_in(&self, _: &mut PgConnection, _: Uuid, _: Uuid) -> Result<(), CommandError> {
        Err(CommandError::Unavailable)
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CommunityError {
    Command(CommandError),
    Party(PartyError),
    EventReadOnly,
}
impl From<CommandError> for CommunityError {
    fn from(v: CommandError) -> Self {
        Self::Command(v)
    }
}
pub struct PgCommunity<A> {
    pool: PgPool,
    authority: A,
}
impl<A: CommunityAuthority> PgCommunity<A> {
    pub fn new(pool: PgPool, authority: A) -> Self {
        Self { pool, authority }
    }
    pub async fn execute_party(
        &self,
        command: &PartyCommand,
        keys: &FingerprintKeys,
    ) -> Result<SafeReceipt, CommunityError> {
        let request = command.request()?;
        let producer = PartyProducer {
            command,
            authority: &self.authority,
            error: Mutex::new(None),
        };
        let result = PgCommandRepository::new(self.pool.clone())
            .execute(&request, command.target(), keys, &producer)
            .await;
        if let Some(error) = producer
            .error
            .lock()
            .map_err(|_| CommandError::Unavailable)?
            .take()
        {
            return Err(error);
        }
        result.map_err(CommunityError::Command)
    }
    /// Safe internal public projection. Caller still maps canonical public owner profile via owner service.
    /// No application notes, member IDs or physical meeting guidance is exposed.
    pub async fn party(
        &self,
        actor: Uuid,
        event: Uuid,
        id: Uuid,
    ) -> Result<PublicParty, CommunityError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let party = load(&mut tx, event, id, false)
            .await?
            .ok_or(CommunityError::Party(PartyError::Unavailable))?;
        self.authority
            .authorize_in(&mut tx, actor, event, party.owner, true)
            .await?;
        let result = PublicParty {
            id: party.id,
            event: party.event,
            owner: party.owner,
            fields: party.fields.clone(),
            state: party.state,
            revision: party.revision,
            meeting_revision: party.meeting_revision,
            member_count: party.member_count(),
            created_at: party.created_at,
        };
        tx.commit().await.map_err(db)?;
        Ok(result)
    }
    /// Synchronous access fence for BE09/BE19 and subscription authorization. Outbox lag
    /// never extends membership. Consumers must recheck before each private read/send.
    pub async fn party_audience(
        &self,
        actor: Uuid,
        event: Uuid,
        id: Uuid,
        expected: i64,
    ) -> Result<(), CommunityError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let party = load(&mut tx, event, id, false)
            .await?
            .ok_or(CommandError::Denied)?;
        self.authority
            .authorize_in(&mut tx, actor, event, party.owner, true)
            .await?;
        if !party.member_access(actor) || party.audience_revision != expected {
            return Err(CommandError::Denied.into());
        }
        tx.commit().await.map_err(db)?;
        Ok(())
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PublicParty {
    pub id: Uuid,
    pub event: Uuid,
    pub owner: Uuid,
    pub fields: PartyFields,
    pub state: PartyState,
    pub revision: i64,
    pub meeting_revision: i64,
    pub member_count: i32,
    pub created_at: chrono::DateTime<chrono::Utc>,
}
struct PartyProducer<'a, A> {
    command: &'a PartyCommand,
    authority: &'a A,
    error: Mutex<Option<CommunityError>>,
}
impl<A> PartyProducer<'_, A> {
    fn domain(&self, error: PartyError) -> CommandError {
        if let Ok(mut slot) = self.error.lock() {
            *slot = Some(CommunityError::Party(error))
        }
        CommandError::InvalidState
    }
}
#[async_trait]
impl<A: CommunityAuthority> CommandProducer for PartyProducer<'_, A> {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        let cmd = self.command;
        if scope != cmd.scope()? || target != cmd.target() {
            return Err(CommandError::Denied);
        }
        // All producers use event -> root -> shared pair -> BE15 budget lock order.
        let phase: Option<String> =
            sqlx::query_scalar("select phase from concert_events where event_id=$1 for share")
                .bind(scope.event)
                .fetch_optional(&mut *c)
                .await
                .map_err(db)?;
        let Some(phase) = phase else {
            return Err(CommandError::Denied);
        };
        if !matches!(phase.as_str(), "OPEN" | "LIVE") && !(cmd.safety() && phase == "READ_ONLY") {
            if let Ok(mut slot) = self.error.lock() {
                *slot = Some(CommunityError::EventReadOnly)
            }
            return Err(CommandError::Denied);
        }
        let party = load(c, scope.event, cmd.party_id(), true).await?;
        let owner = party.as_ref().map(|p| p.owner).unwrap_or(scope.actor);
        self.authority
            .authorize_in(c, scope.actor, scope.event, owner, cmd.safety())
            .await?;
        if let Some(p) = party {
            let subject = match cmd.action {
                PartyAction::Decide { application, .. }
                | PartyAction::CancelApplication { application, .. } => Some(
                    p.applications
                        .iter()
                        .find(|a| a.id == application)
                        .ok_or(CommandError::Denied)?
                        .user,
                ),
                PartyAction::Remove { user, .. } | PartyAction::Transfer { user, .. } => Some(user),
                _ => None,
            };
            match cmd.action {
                PartyAction::Create(_) => {
                    if p.owner != scope.actor {
                        return Err(CommandError::Denied);
                    }
                }
                PartyAction::Update { .. }
                | PartyAction::Cancel { .. }
                | PartyAction::Transfer { .. }
                | PartyAction::Decide { .. } => {
                    if p.owner != scope.actor {
                        return Err(CommandError::Denied);
                    }
                }
                PartyAction::CancelApplication { application, .. } => {
                    if !p
                        .applications
                        .iter()
                        .any(|a| a.id == application && a.user == scope.actor)
                    {
                        return Err(CommandError::Denied);
                    }
                }
                PartyAction::Remove { user, .. } => {
                    if scope.actor != p.owner && scope.actor != user {
                        return Err(CommandError::Denied);
                    }
                }
                PartyAction::Apply { .. } => {}
            }
            if let Some(subject) = subject {
                self.authority
                    .authorize_in(c, scope.actor, scope.event, subject, cmd.safety())
                    .await?;
            }
        } else if !matches!(cmd.action, PartyAction::Create(_)) {
            return Err(CommandError::Denied);
        }
        Ok(())
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        let now = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(db)?;
        let (party, change) = if let PartyAction::Create(fields) = &self.command.action {
            let party = Party::create(target, scope.event, scope.actor, fields.clone(), now)
                .map_err(|e| self.domain(e))?;
            (
                party,
                PartyChange {
                    resource: target,
                    revision: 1,
                },
            )
        } else {
            let mut p = load(c, scope.event, self.command.party_id(), true)
                .await?
                .ok_or(CommandError::Denied)?;
            let change = p
                .change(
                    scope.actor,
                    &self.command.action,
                    self.command.created_id(),
                    now,
                )
                .map_err(|e| self.domain(e))?;
            (p, change)
        };
        if matches!(
            self.command.action,
            PartyAction::Create(_) | PartyAction::Update { .. }
        ) {
            self.authority
                .place_in(c, scope.event, party.fields.public_place_id)
                .await?;
        }
        save(c, &party).await?;
        let reference = SafeReference {
            resource: change.resource,
            revision: change.revision,
        };
        Ok((
            Completion::Succeeded {
                reference,
                status: 200,
            },
            Some(Invalidation { reference }),
        ))
    }
}
async fn load(
    c: &mut PgConnection,
    event: Uuid,
    id: Uuid,
    write: bool,
) -> Result<Option<Party>, CommandError> {
    let sql = if write {
        "select aggregate from concert_community_parties where event_id=$1 and party_id=$2 for update"
    } else {
        "select aggregate from concert_community_parties where event_id=$1 and party_id=$2 for share"
    };
    let row = sqlx::query(sql)
        .bind(event)
        .bind(id)
        .fetch_optional(c)
        .await
        .map_err(db)?;
    row.map(|r| {
        let value: serde_json::Value = r.try_get("aggregate").map_err(db)?;
        let p: Party = serde_json::from_value(value).map_err(|_| CommandError::InvalidState)?;
        if p.id != id || p.event != event {
            return Err(CommandError::InvalidState);
        }
        p.validate().map_err(|_| CommandError::InvalidState)?;
        Ok(p)
    })
    .transpose()
}
async fn save(c: &mut PgConnection, p: &Party) -> Result<(), CommandError> {
    let aggregate = serde_json::to_value(p).map_err(|_| CommandError::InvalidState)?;
    sqlx::query("insert into concert_community_parties(event_id,party_id,owner_id,revision,state,member_count,capacity,meeting_revision,audience_revision,aggregate) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) on conflict(event_id,party_id) do update set owner_id=excluded.owner_id,revision=excluded.revision,state=excluded.state,member_count=excluded.member_count,capacity=excluded.capacity,meeting_revision=excluded.meeting_revision,audience_revision=excluded.audience_revision,aggregate=excluded.aggregate")
        .bind(p.event).bind(p.id).bind(p.owner).bind(p.revision).bind(concert_application::community::state_name(p.state)).bind(p.member_count()).bind(p.fields.capacity).bind(p.meeting_revision).bind(p.audience_revision).bind(aggregate).execute(&mut *c).await.map_err(db)?;
    // Demote first so owner transfer never transiently violates the unique-owner index.
    sqlx::query("update concert_community_members set is_owner=false where event_id=$1 and party_id=$2 and is_owner and user_id<>$3").bind(p.event).bind(p.id).bind(p.owner).execute(&mut *c).await.map_err(db)?;
    for m in &p.members {
        sqlx::query("insert into concert_community_members(event_id,party_id,user_id,is_owner,state,revision) values($1,$2,$3,$4,$5,$6) on conflict(event_id,party_id,user_id) do update set is_owner=excluded.is_owner,state=excluded.state,revision=excluded.revision")
            .bind(p.event).bind(p.id).bind(m.user).bind(m.owner).bind(match m.state{MemberState::Active=>"ACTIVE",MemberState::Removed=>"REMOVED",MemberState::Left=>"LEFT"}).bind(m.revision).execute(&mut *c).await.map_err(db)?;
    }
    for a in &p.applications {
        sqlx::query("insert into concert_community_applications(event_id,party_id,application_id,user_id,state,revision) values($1,$2,$3,$4,$5,$6) on conflict(event_id,application_id) do update set state=excluded.state,revision=excluded.revision")
            .bind(p.event).bind(p.id).bind(a.id).bind(a.user).bind(match a.state{ApplicationState::Pending=>"PENDING",ApplicationState::Approved=>"APPROVED",ApplicationState::Rejected=>"REJECTED",ApplicationState::Cancelled=>"CANCELLED"}).bind(a.revision).execute(&mut *c).await.map_err(db)?;
    }
    Ok(())
}
fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
