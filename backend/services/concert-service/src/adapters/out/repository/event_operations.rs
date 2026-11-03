//! BE38 transactional producers. Production composition must supply qualified owner ports.
use super::receipt::{CommandProducer, PgCommandRepository};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    admin::{event::*, notices::*, places::*, *},
    commands::*,
};
use concert_domain::{Event, EventId, EventPhase, EventSlug, EventTitle, Revision};
use serde::Serialize;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

/// Request-bound canonical Auth + event-role owner. Lock/fence revocation and the
/// returned revision through commit. Never implement from request role booleans,
/// bootstrap/profile presence or an expired/cached lease. Test authorities stay in tests.
#[async_trait]
pub trait FencedOperationsAuthority: Send + Sync {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        query: AuthorityQuery,
        now: DateTime<Utc>,
    ) -> Result<RoleLease, CommandError>;
}
pub struct UnavailableAuthority;
#[async_trait]
impl FencedOperationsAuthority for UnavailableAuthority {
    async fn check_in(
        &self,
        _: &mut PgConnection,
        _: AuthorityQuery,
        _: DateTime<Utc>,
    ) -> Result<RoleLease, CommandError> {
        Err(CommandError::Unavailable)
    }
}
/// Review must bind exact content/source/confirmation/state to this actor/event/notice,
/// and retain its source/review revision fence in this transaction. An operator role
/// alone is not proof that a source was reviewed. No network side effects here.
#[async_trait]
pub trait FencedNoticeReview: Send + Sync {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        notice: Uuid,
        input: &NoticeInput,
        now: DateTime<Utc>,
    ) -> Result<NoticeReviewLease, CommandError>;
}
#[derive(Clone)]
pub struct NoticeReviewLease {
    pub actor: Actor,
    pub notice_id: Uuid,
    pub input_digest: [u8; 32],
    pub revision: i64,
    pub checked_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
pub fn notice_review_digest(input: &NoticeInput) -> [u8; 32] {
    Sha256::digest(serde_json::to_vec(&json!({"title":input.title,"text":input.text,"source_uri":input.source_uri,"confirmed_at":input.confirmed_at,"state":input.state.as_str()})).expect("serializable typed notice")).into()
}
impl NoticeReviewLease {
    fn validate(
        &self,
        actor: Actor,
        id: Uuid,
        input: &NoticeInput,
        started: DateTime<Utc>,
        finished: DateTime<Utc>,
    ) -> Result<(), CommandError> {
        if self.actor != actor
            || self.notice_id != id
            || self.input_digest != notice_review_digest(input)
            || !(1..=concert_domain::MAX_REVISION).contains(&self.revision)
            || self.checked_at < started
            || self.checked_at > finished
            || self.expires_at <= finished
        {
            Err(CommandError::Unavailable)
        } else {
            Ok(())
        }
    }
}
pub struct UnavailableReview;
#[async_trait]
impl FencedNoticeReview for UnavailableReview {
    async fn check_in(
        &self,
        _: &mut PgConnection,
        _: Actor,
        _: Uuid,
        _: &NoticeInput,
        _: DateTime<Utc>,
    ) -> Result<NoticeReviewLease, CommandError> {
        Err(CommandError::Unavailable)
    }
}
#[derive(Clone, Debug)]
pub struct AdminPlace {
    pub place: concert_domain::place::Place,
    pub revision: Revision,
}
/// Existing place/map owner supplies local SQL mutation under the caller transaction.
/// It validates event/map binding, CAS, review/provenance and confirmation, and
/// immediately fences physical selection. A pending/uncertain external call is NOT Applied.
#[async_trait]
pub trait FencedPlaceOperations: Send + Sync {
    async fn list_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        after: Option<Uuid>,
        closed: Option<bool>,
        limit: u16,
    ) -> Result<Vec<AdminPlace>, CommandError>;
    async fn update_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        change: &PlaceChange,
    ) -> Result<Result<AdminPlace, SafeFailure>, CommandError>;
}
pub struct UnavailablePlaces;
#[async_trait]
impl FencedPlaceOperations for UnavailablePlaces {
    async fn list_in(
        &self,
        _: &mut PgConnection,
        _: Actor,
        _: Option<Uuid>,
        _: Option<bool>,
        _: u16,
    ) -> Result<Vec<AdminPlace>, CommandError> {
        Err(CommandError::Unavailable)
    }
    async fn update_in(
        &self,
        _: &mut PgConnection,
        _: Actor,
        _: &PlaceChange,
    ) -> Result<Result<AdminPlace, SafeFailure>, CommandError> {
        Err(CommandError::Unavailable)
    }
}

pub(crate) fn unavailable(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
pub(crate) async fn now(c: &mut PgConnection) -> Result<DateTime<Utc>, CommandError> {
    sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(c)
        .await
        .map_err(unavailable)
}
pub(crate) async fn pin(c: &mut PgConnection, until: DateTime<Utc>) -> Result<(), CommandError> {
    sqlx::query("select set_config('mcr.operations_authority_until',least(coalesce(nullif(current_setting('mcr.operations_authority_until',true),'')::timestamptz,'infinity'::timestamptz),$1::timestamptz)::text,true)").bind(until).execute(c).await.map_err(unavailable)?;
    Ok(())
}
pub(crate) async fn authorize<A: FencedOperationsAuthority>(
    c: &mut PgConnection,
    a: &A,
    q: AuthorityQuery,
    write: bool,
) -> Result<(EventPhase, i64), CommandError> {
    // All command producers lock event UPDATE first, avoiding SHARE-to-UPDATE deadlock
    // across the BE15 admission lock. Reads/feature gates retain SHARE through commit.
    let row = sqlx::query(if write {
        "select phase,revision from concert_events where event_id=$1 for update"
    } else {
        "select phase,revision from concert_events where event_id=$1 for share"
    })
    .bind(q.actor.event)
    .fetch_optional(&mut *c)
    .await
    .map_err(unavailable)?
    .ok_or(CommandError::Denied)?;
    let started = now(c).await?;
    let lease = a.check_in(c, q, started).await?;
    let finished = now(c).await?;
    lease.validate(q, started, finished)?;
    pin(c, lease.expires_at).await?;
    let phase: EventPhase = row
        .try_get::<String, _>("phase")
        .map_err(unavailable)?
        .parse()
        .map_err(|_| CommandError::InvalidState)?;
    let rev = row.try_get("revision").map_err(unavailable)?;
    Ok((phase, rev))
}
pub(crate) async fn finish_read(c: &mut PgConnection) -> Result<(), CommandError> {
    let valid:bool=sqlx::query_scalar("select clock_timestamp()<nullif(current_setting('mcr.operations_authority_until',true),'')::timestamptz").fetch_one(c).await.map_err(unavailable)?;
    if valid {
        Ok(())
    } else {
        Err(CommandError::Unavailable)
    }
}
pub(crate) async fn audit(
    c: &mut PgConnection,
    actor: Actor,
    action: Action,
    target: Uuid,
    key: Option<Uuid>,
    change: Value,
) -> Result<(), CommandError> {
    let receipt:Option<Uuid>=match key{Some(key)=>Some(sqlx::query_scalar("select receipt_id from concert_command_receipts where actor_id=$1 and event_id=$2 and operation_id=$3 and idempotency_key=$4").bind(actor.actor).bind(actor.event).bind(action.operation()).bind(key).fetch_one(&mut *c).await.map_err(unavailable)?),None=>None};
    sqlx::query("insert into concert_operations_audit(audit_id,event_id,actor_id,operation_id,target_id,receipt_id,outcome,change) values($1,$2,$3,$4,$5,$6,'SUCCEEDED',$7)").bind(Uuid::new_v4()).bind(actor.event).bind(actor.actor).bind(action.operation()).bind(target).bind(receipt).bind(change).execute(c).await.map_err(unavailable)?;
    Ok(())
}
pub(crate) fn success(id: Uuid, revision: i64) -> (Completion, Option<Invalidation>) {
    let reference = SafeReference {
        resource: id,
        revision,
    };
    (
        Completion::Succeeded {
            reference,
            status: 200,
        },
        Some(Invalidation { reference }),
    )
}
pub(crate) fn failure(reason: SafeFailure) -> (Completion, Option<Invalidation>) {
    (Completion::Failed(reason), None)
}
pub(crate) fn stable_id(actor: Actor, key: Uuid, action: Action) -> Uuid {
    let mut hash = Sha256::new();
    hash.update(b"mcr-be36-38-v1");
    hash.update(actor.event.as_bytes());
    hash.update(actor.actor.as_bytes());
    hash.update(key.as_bytes());
    hash.update(action.operation());
    let bytes = hash.finalize();
    let mut raw = [0; 16];
    raw.copy_from_slice(&bytes[..16]);
    raw[6] = (raw[6] & 15) | 0x50;
    raw[8] = (raw[8] & 63) | 0x80;
    Uuid::from_bytes(raw)
}
pub(crate) fn request(
    actor: Actor,
    key: Uuid,
    action: Action,
    id: Uuid,
    body: Vec<(String, CanonicalValue)>,
) -> Result<CanonicalRequest, CommandError> {
    let operation = RegisteredOperation::find(action.operation())?;
    let event = actor.event.to_string();
    let resource = id.to_string();
    let mut params = vec![("eventId", event.as_str())];
    match action {
        Action::PlaceWrite => params.push(("placeId", &resource)),
        Action::NoticeUpdate => params.push(("noticeId", &resource)),
        _ => {}
    }
    CanonicalRequest::json(
        Scope {
            actor: actor.actor,
            event: actor.event,
            operation,
            key,
        },
        &params,
        &[],
        Some(&CanonicalValue::Object(body)),
    )
}
#[derive(Clone)]
pub enum EventCommand {
    Settings {
        expected: Revision,
        settings: Settings,
        reason: String,
    },
    Phase {
        expected: Revision,
        phase: EventPhase,
        reason: String,
    },
    NoticeCreate(NoticeInput),
    NoticeUpdate {
        expected: Revision,
        input: NoticeInput,
    },
    Place(PlaceChange),
}
impl EventCommand {
    fn action(&self) -> Action {
        match self {
            Self::Settings { .. } => Action::SettingsWrite,
            Self::Phase { .. } => Action::PhaseWrite,
            Self::NoticeCreate(_) => Action::NoticeCreate,
            Self::NoticeUpdate { .. } => Action::NoticeUpdate,
            Self::Place(_) => Action::PlaceWrite,
        }
    }
    fn body(&self) -> Result<Vec<(String, CanonicalValue)>, CommandError> {
        let (mut body, expected, why) = match self {
            Self::Settings {
                expected,
                settings,
                reason,
            } => {
                settings.validate()?;
                (settings.canonical(), Some(*expected), Some(reason))
            }
            Self::Phase {
                expected,
                phase,
                reason,
            } => {
                if !matches!(phase, EventPhase::Open | EventPhase::Live) {
                    return Err(CommandError::InvalidInput);
                };
                (
                    vec![(
                        "phase".into(),
                        CanonicalValue::String(phase.as_str().into()),
                    )],
                    Some(*expected),
                    Some(reason),
                )
            }
            Self::NoticeCreate(input) => (input.canonical(), None, None),
            Self::NoticeUpdate { expected, input } => (input.canonical(), Some(*expected), None),
            Self::Place(change) => {
                change.validate()?;
                let p = &change.place;
                let v = &p.provenance;
                let place = json!({"place_id":p.id.uuid(),"label":p.label,"place_kind":p.kind.as_str(),"verification_state":p.verification.as_str(),"closure_state":p.closure.as_str(),"accessibility_notes":p.accessibility_notes,"provenance":{"source_id":v.source_id,"source_kind":v.source_kind.as_str(),"source_revision":v.source_revision,"source_uri":v.source_uri,"author":v.author,"license":v.license,"review_state":v.review_state.as_str(),"reviewed_by":v.reviewed_by,"reviewed_at":v.reviewed_at,"photo_permission":v.photo_permission.as_str()}});
                (
                    vec![("place".into(), canonical_json(place)?)],
                    Some(change.expected),
                    Some(&change.reason),
                )
            }
        };
        if let Some(rev) = expected {
            body.push((
                "expected_revision".into(),
                CanonicalValue::Integer(rev.get()),
            ))
        }
        if let Some(why) = why {
            reason(why)?;
            body.push(("reason".into(), CanonicalValue::String(why.clone())))
        }
        Ok(body)
    }
}
pub struct PgEventOperations<A, R = UnavailableReview, P = UnavailablePlaces> {
    pool: PgPool,
    authority: A,
    review: R,
    places: P,
}
impl<A: FencedOperationsAuthority, R: FencedNoticeReview, P: FencedPlaceOperations>
    PgEventOperations<A, R, P>
{
    pub fn new(pool: PgPool, authority: A, review: R, places: P) -> Self {
        Self {
            pool,
            authority,
            review,
            places,
        }
    }
    pub async fn execute(
        &self,
        actor: Actor,
        key: Uuid,
        id: Option<Uuid>,
        command: EventCommand,
        keys: &FingerprintKeys,
    ) -> Result<SafeReceipt, CommandError> {
        let action = command.action();
        let target = match &command {
            EventCommand::Settings { .. } | EventCommand::Phase { .. } => {
                if id.is_some_and(|id| id != actor.event) {
                    return Err(CommandError::InvalidInput);
                }
                actor.event
            }
            EventCommand::NoticeCreate(_) => {
                if id.is_some() {
                    return Err(CommandError::InvalidInput);
                }
                stable_id(actor, key, action)
            }
            _ => id.ok_or(CommandError::InvalidInput)?,
        };
        if let EventCommand::Place(change) = &command {
            if change.place.id.uuid() != target {
                return Err(CommandError::InvalidInput);
            }
        }
        let request = request(actor, key, action, target, command.body()?)?;
        let producer = EventProducer {
            operations: self,
            actor,
            target,
            command,
        };
        PgCommandRepository::new(self.pool.clone())
            .execute(&request, target, keys, &producer)
            .await
    }
    pub async fn settings(&self, actor: Actor) -> Result<SettingsSnapshot, CommandError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        authorize(
            &mut tx,
            &self.authority,
            AuthorityQuery {
                actor,
                action: Action::SettingsRead,
                target: actor.event,
            },
            false,
        )
        .await?;
        let event = read_event(&mut tx, actor.event).await?;
        let settings = read_settings(&mut tx, actor.event).await?;
        audit(
            &mut tx,
            actor,
            Action::SettingsRead,
            actor.event,
            None,
            json!({}),
        )
        .await?;
        finish_read(&mut tx).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(SettingsSnapshot { event, settings })
    }
    pub async fn places(
        &self,
        actor: Actor,
        after: Option<Uuid>,
        closed: Option<bool>,
        limit: u16,
    ) -> Result<Vec<AdminPlace>, CommandError> {
        if !(1..=100).contains(&limit) {
            return Err(CommandError::InvalidInput);
        }
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        authorize(
            &mut tx,
            &self.authority,
            AuthorityQuery {
                actor,
                action: Action::PlacesRead,
                target: actor.event,
            },
            false,
        )
        .await?;
        let result = self
            .places
            .list_in(&mut tx, actor, after, closed, limit)
            .await?;
        if result.len() > limit as usize {
            return Err(CommandError::InvalidState);
        }
        audit(
            &mut tx,
            actor,
            Action::PlacesRead,
            actor.event,
            None,
            json!({}),
        )
        .await?;
        finish_read(&mut tx).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(result)
    }
    pub async fn admin_notices(
        &self,
        actor: Actor,
        after: Option<Uuid>,
        state: Option<NoticeState>,
        limit: u16,
    ) -> Result<Vec<Notice>, CommandError> {
        if !(1..=100).contains(&limit) {
            return Err(CommandError::InvalidInput);
        }
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        authorize(
            &mut tx,
            &self.authority,
            AuthorityQuery {
                actor,
                action: Action::NoticeAdminRead,
                target: actor.event,
            },
            false,
        )
        .await?;
        let rows=sqlx::query("select * from concert_operation_notices where event_id=$1 and ($2::uuid is null or notice_id>$2) and ($3::text is null or state=$3) order by notice_id limit $4").bind(actor.event).bind(after).bind(state.map(NoticeState::as_str)).bind(i64::from(limit)).fetch_all(&mut *tx).await.map_err(unavailable)?;
        let result = rows.iter().map(notice).collect::<Result<_, _>>()?;
        audit(
            &mut tx,
            actor,
            Action::NoticeAdminRead,
            actor.event,
            None,
            json!({}),
        )
        .await?;
        finish_read(&mut tx).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(result)
    }
    pub async fn public_notice(
        &self,
        actor: Actor,
        id: Uuid,
    ) -> Result<Option<Notice>, CommandError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        let (phase, _) = authorize(
            &mut tx,
            &self.authority,
            AuthorityQuery {
                actor,
                action: Action::NoticeDetail,
                target: id,
            },
            false,
        )
        .await?;
        if phase == EventPhase::Draft {
            return Err(CommandError::Denied);
        }
        let row=sqlx::query("select * from concert_operation_notices where event_id=$1 and notice_id=$2 and state='PUBLISHED' for share").bind(actor.event).bind(id).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let result = row.as_ref().map(notice).transpose()?;
        finish_read(&mut tx).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(result)
    }
    pub async fn notices(
        &self,
        actor: Actor,
        after: Option<Uuid>,
        limit: u16,
    ) -> Result<Vec<Notice>, CommandError> {
        if !(1..=100).contains(&limit) {
            return Err(CommandError::InvalidInput);
        }
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        let (phase, _) = authorize(
            &mut tx,
            &self.authority,
            AuthorityQuery {
                actor,
                action: Action::NoticeRead,
                target: actor.event,
            },
            false,
        )
        .await?;
        if matches!(phase, EventPhase::Draft) {
            return Err(CommandError::Denied);
        }
        let rows=sqlx::query("select * from concert_operation_notices where event_id=$1 and state='PUBLISHED' and ($2::uuid is null or notice_id>$2) order by notice_id limit $3").bind(actor.event).bind(after).bind(i64::from(limit)).fetch_all(&mut *tx).await.map_err(unavailable)?;
        let result = rows.iter().map(notice).collect::<Result<_, _>>()?;
        finish_read(&mut tx).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(result)
    }
}
struct EventProducer<'a, A, R, P> {
    operations: &'a PgEventOperations<A, R, P>,
    actor: Actor,
    target: Uuid,
    command: EventCommand,
}
#[async_trait]
impl<A: FencedOperationsAuthority, R: FencedNoticeReview, P: FencedPlaceOperations> CommandProducer
    for EventProducer<'_, A, R, P>
{
    async fn authorize(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if s.actor != self.actor.actor
            || s.event != self.actor.event
            || s.operation.id() != self.command.action().operation()
        {
            return Err(CommandError::Denied);
        }
        authorize(
            c,
            &self.operations.authority,
            AuthorityQuery {
                actor: self.actor,
                action: self.command.action(),
                target,
            },
            true,
        )
        .await?;
        Ok(())
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        if target != self.target {
            return Err(CommandError::InvalidState);
        }
        let row = sqlx::query("select phase,revision from concert_events where event_id=$1")
            .bind(s.event)
            .fetch_one(&mut *c)
            .await
            .map_err(unavailable)?;
        let phase: EventPhase = row
            .try_get::<String, _>("phase")
            .map_err(unavailable)?
            .parse()
            .map_err(|_| CommandError::InvalidState)?;
        let revision: i64 = row.try_get("revision").map_err(unavailable)?;
        if matches!(phase, EventPhase::ReadOnly | EventPhase::Archived) {
            return Ok(failure(SafeFailure::EventReadOnly));
        }
        let change;
        let next = match &self.command {
            EventCommand::Settings {
                expected, settings, ..
            } => {
                if expected.get() != revision || expected.next().is_err() {
                    return Ok(failure(SafeFailure::VersionConflict));
                }
                let exists: bool = sqlx::query_scalar(
                    "select exists(select 1 from concert_event_settings where event_id=$1)",
                )
                .bind(s.event)
                .fetch_one(&mut *c)
                .await
                .map_err(unavailable)?;
                let old = if exists {
                    Some(read_settings(c, s.event).await?)
                } else {
                    None
                };
                sqlx::query("insert into concert_event_settings(event_id,parties,giveaways,intros,social,mural,beacon_ttl_seconds,pickup_grace_seconds,social_grant_ttl_seconds,mural_cooldown_ms) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) on conflict(event_id) do update set parties=$2,giveaways=$3,intros=$4,social=$5,mural=$6,beacon_ttl_seconds=$7,pickup_grace_seconds=$8,social_grant_ttl_seconds=$9,mural_cooldown_ms=$10").bind(s.event).bind(settings.features.parties).bind(settings.features.giveaways).bind(settings.features.intros).bind(settings.features.social).bind(settings.features.mural).bind(settings.beacon_ttl_seconds).bind(settings.pickup_grace_seconds).bind(settings.social_grant_ttl_seconds).bind(settings.mural_cooldown_ms).execute(&mut *c).await.map_err(unavailable)?;
                change = json!({"before":old.map(settings_json),"after":settings_json(*settings)});
                sqlx::query("update concert_events set revision=revision+1 where event_id=$1")
                    .bind(s.event)
                    .execute(&mut *c)
                    .await
                    .map_err(unavailable)?;
                revision + 1
            }
            EventCommand::Phase {
                expected,
                phase: next,
                ..
            } => {
                if expected.get() != revision || expected.next().is_err() {
                    return Ok(failure(SafeFailure::VersionConflict));
                }
                if !phase_transition(phase, *next) {
                    return Ok(failure(SafeFailure::InvalidTransition));
                }
                sqlx::query(
                    "update concert_events set phase=$2,revision=revision+1 where event_id=$1",
                )
                .bind(s.event)
                .bind(next.as_str())
                .execute(&mut *c)
                .await
                .map_err(unavailable)?;
                change = json!({"before":phase.as_str(),"after":next.as_str()});
                revision + 1
            }
            EventCommand::NoticeCreate(input) | EventCommand::NoticeUpdate { input, .. } => {
                let time = now(c).await?;
                input.validate(time)?;
                let lease = self
                    .operations
                    .review
                    .check_in(c, self.actor, target, input, time)
                    .await?;
                lease.validate(self.actor, target, input, time, now(c).await?)?;
                pin(c, lease.expires_at).await?;
                if let EventCommand::NoticeUpdate { expected, .. } = &self.command {
                    let previous=sqlx::query("select revision,state from concert_operation_notices where event_id=$1 and notice_id=$2 for update").bind(s.event).bind(target).fetch_optional(&mut *c).await.map_err(unavailable)?;
                    let Some(previous) = previous else {
                        return Ok(failure(SafeFailure::ResourceUnavailable));
                    };
                    let previous_revision: i64 =
                        previous.try_get("revision").map_err(unavailable)?;
                    if previous_revision != expected.get() || expected.next().is_err() {
                        return Ok(failure(SafeFailure::VersionConflict));
                    }
                    sqlx::query("update concert_operation_notices set title=$3,text=$4,source_uri=$5,confirmed_at=$6,state=$7,revision=revision+1 where event_id=$1 and notice_id=$2").bind(s.event).bind(target).bind(&input.title).bind(&input.text).bind(&input.source_uri).bind(input.confirmed_at).bind(input.state.as_str()).execute(&mut *c).await.map_err(unavailable)?;
                    change = json!({"before_state":previous.try_get::<String,_>("state").map_err(unavailable)?,"after_state":input.state.as_str(),"content_replaced":true});
                    previous_revision + 1
                } else {
                    sqlx::query("insert into concert_operation_notices(notice_id,event_id,title,text,source_uri,confirmed_at,state) values($1,$2,$3,$4,$5,$6,$7)").bind(target).bind(s.event).bind(&input.title).bind(&input.text).bind(&input.source_uri).bind(input.confirmed_at).bind(input.state.as_str()).execute(&mut *c).await.map_err(unavailable)?;
                    change = json!({"after_state":input.state.as_str()});
                    1
                }
            }
            EventCommand::Place(input) => {
                match self
                    .operations
                    .places
                    .update_in(c, self.actor, input)
                    .await?
                {
                    Ok(value) => {
                        if value.place != input.place
                            || value.revision
                                != input
                                    .expected
                                    .next()
                                    .map_err(|_| CommandError::InvalidState)?
                        {
                            return Err(CommandError::InvalidState);
                        }
                        change = json!({"place_revision":value.revision.get(),"provenance_rechecked":true});
                        value.revision.get()
                    }
                    Err(f) => return Ok(failure(f)),
                }
            }
        };
        audit(
            c,
            self.actor,
            self.command.action(),
            target,
            Some(s.key),
            change,
        )
        .await?;
        Ok(success(target, next))
    }
}
fn settings_json(s: Settings) -> Value {
    json!({"features":{"parties":s.features.parties,"giveaways":s.features.giveaways,"intros":s.features.intros,"social":s.features.social,"mural":s.features.mural},"beacon_ttl_seconds":s.beacon_ttl_seconds,"pickup_grace_seconds":s.pickup_grace_seconds,"social_grant_ttl_seconds":s.social_grant_ttl_seconds,"mural_cooldown_ms":s.mural_cooldown_ms})
}
async fn read_settings(c: &mut PgConnection, event: Uuid) -> Result<Settings, CommandError> {
    let r = sqlx::query("select * from concert_event_settings where event_id=$1 for share")
        .bind(event)
        .fetch_optional(c)
        .await
        .map_err(unavailable)?
        .ok_or(CommandError::Unavailable)?;
    let s = Settings {
        features: Features {
            parties: r.try_get("parties").map_err(unavailable)?,
            giveaways: r.try_get("giveaways").map_err(unavailable)?,
            intros: r.try_get("intros").map_err(unavailable)?,
            social: r.try_get("social").map_err(unavailable)?,
            mural: r.try_get("mural").map_err(unavailable)?,
        },
        beacon_ttl_seconds: r.try_get("beacon_ttl_seconds").map_err(unavailable)?,
        pickup_grace_seconds: r.try_get("pickup_grace_seconds").map_err(unavailable)?,
        social_grant_ttl_seconds: r.try_get("social_grant_ttl_seconds").map_err(unavailable)?,
        mural_cooldown_ms: r.try_get("mural_cooldown_ms").map_err(unavailable)?,
    };
    s.validate().map_err(|_| CommandError::InvalidState)
}
async fn read_event(c: &mut PgConnection, id: Uuid) -> Result<Event, CommandError> {
    let r = sqlx::query("select * from concert_events where event_id=$1")
        .bind(id)
        .fetch_one(c)
        .await
        .map_err(unavailable)?;
    Ok(Event {
        id: EventId::from_uuid(id),
        slug: EventSlug::new(r.try_get("slug").map_err(unavailable)?)
            .map_err(|_| CommandError::InvalidState)?,
        title: EventTitle::new(r.try_get("title").map_err(unavailable)?)
            .map_err(|_| CommandError::InvalidState)?,
        phase: r
            .try_get::<String, _>("phase")
            .map_err(unavailable)?
            .parse()
            .map_err(|_| CommandError::InvalidState)?,
        revision: Revision::new(r.try_get("revision").map_err(unavailable)?)
            .map_err(|_| CommandError::InvalidState)?,
        created_at: r.try_get("created_at").map_err(unavailable)?,
        updated_at: r.try_get("updated_at").map_err(unavailable)?,
    })
}
/// Every feature mutation producer must call this in its existing authority transaction
/// before applying effects. Missing settings are unavailable; hidden UI never grants access.
pub struct FeatureGuard<'a> {
    connection: &'a mut PgConnection,
    settings: Settings,
    revision: Revision,
}
impl std::fmt::Debug for FeatureGuard<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("FeatureGuard")
            .field("revision", &self.revision)
            .finish_non_exhaustive()
    }
}
impl FeatureGuard<'_> {
    pub fn connection(&mut self) -> &mut PgConnection {
        self.connection
    }
    pub fn settings(&self) -> Settings {
        self.settings
    }
    pub fn revision(&self) -> Revision {
        self.revision
    }
}
pub async fn require_feature_in(
    c: &mut PgConnection,
    event: Uuid,
    feature: Feature,
    expected: Option<Revision>,
) -> Result<Result<FeatureGuard<'_>, SafeFailure>, CommandError> {
    let r = sqlx::query("select phase,revision from concert_events where event_id=$1 for share")
        .bind(event)
        .fetch_optional(&mut *c)
        .await
        .map_err(unavailable)?
        .ok_or(CommandError::Denied)?;
    let phase: String = r.try_get("phase").map_err(unavailable)?;
    if !matches!(phase.as_str(), "OPEN" | "LIVE") {
        return Ok(Err(SafeFailure::EventReadOnly));
    }
    let revision: i64 = r.try_get("revision").map_err(unavailable)?;
    if expected.is_some_and(|e| e.get() != revision) {
        return Ok(Err(SafeFailure::VersionConflict));
    }
    let settings = read_settings(c, event).await?;
    if !settings.features.enabled(feature) {
        return Ok(Err(SafeFailure::EventReadOnly));
    }
    Ok(Ok(FeatureGuard {
        connection: c,
        settings,
        revision: Revision::new(revision).map_err(|_| CommandError::InvalidState)?,
    }))
}
#[derive(Clone, Serialize)]
pub struct Notice {
    pub notice_id: Uuid,
    pub event_id: Uuid,
    pub title: String,
    pub text: String,
    pub source_uri: String,
    pub confirmed_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub state: String,
    pub revision: i64,
}
fn notice(r: &sqlx::postgres::PgRow) -> Result<Notice, CommandError> {
    Ok(Notice {
        notice_id: r.try_get("notice_id").map_err(unavailable)?,
        event_id: r.try_get("event_id").map_err(unavailable)?,
        title: r.try_get("title").map_err(unavailable)?,
        text: r.try_get("text").map_err(unavailable)?,
        source_uri: r.try_get("source_uri").map_err(unavailable)?,
        confirmed_at: r.try_get("confirmed_at").map_err(unavailable)?,
        created_at: r.try_get("created_at").map_err(unavailable)?,
        state: r.try_get("state").map_err(unavailable)?,
        revision: r.try_get("revision").map_err(unavailable)?,
    })
}

pub(crate) fn canonical_json(value: Value) -> Result<CanonicalValue, CommandError> {
    Ok(match value {
        Value::Null => CanonicalValue::Null,
        Value::Bool(v) => CanonicalValue::Bool(v),
        Value::Number(v) => CanonicalValue::Integer(v.as_i64().ok_or(CommandError::InvalidInput)?),
        Value::String(v) => CanonicalValue::String(v),
        Value::Array(v) => CanonicalValue::Array(
            v.into_iter()
                .map(canonical_json)
                .collect::<Result<_, _>>()?,
        ),
        Value::Object(v) => CanonicalValue::Object(
            v.into_iter()
                .map(|(k, v)| Ok((k, canonical_json(v)?)))
                .collect::<Result<_, CommandError>>()?,
        ),
    })
}
