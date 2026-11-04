//! SQL-serialized giveaway/reservation commands, one-shot codes and restart-safe bounded sweeps.
use super::{db, CommunityAccess, CommunityAuthority};
use crate::adapters::out::repository::{
    outbox_maintenance::enqueue_maintenance_in,
    receipt::{CommandProducer, PgCommandRepository},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    commands::maintenance::MaintenanceInvalidation,
    commands::*,
    community::{giveaway::*, resource_id},
};
use concert_domain::community::giveaway::*;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Mutex;
use uuid::Uuid;

pub struct GiftSettings {
    pub pickup_grace_seconds: i32,
    pub event_cutoff: DateTime<Utc>,
}
#[async_trait]
pub trait GiftAuthority: CommunityAuthority {
    /// Optional owning adapter witness on this exact receipt transaction.
    /// The default supplies no canonical committed-transaction evidence.
    async fn completed_in(
        &self,
        _c: &mut PgConnection,
        _scope: Scope,
        _target: Uuid,
        _receipt: &SafeReceipt,
    ) -> Result<(), CommandError> {
        Ok(())
    }
    async fn settings_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
    ) -> Result<GiftSettings, CommandError>;
    /// READY, owned, correct-purpose media; no raw browser URL is accepted.
    async fn media_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        actor: Uuid,
        giveaway: Uuid,
        media: Option<Uuid>,
    ) -> Result<(), CommandError>;
    /// Current reviewed BE08 map/place versions and allowed event cutoff are fenced here.
    async fn meeting_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        meeting: &PickupMeeting,
    ) -> Result<(), CommandError>;
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GiftServiceError {
    Command(CommandError),
    Gift(GiftError),
    EventReadOnly,
}
impl From<CommandError> for GiftServiceError {
    fn from(v: CommandError) -> Self {
        Self::Command(v)
    }
}
/// Integration derives this from the authenticated request's trusted network context,
/// using an injected privacy key. Never accept a browser field or retain raw IP.
pub struct TrustedPickupContext {
    ip_bucket: [u8; 32],
}
impl TrustedPickupContext {
    pub fn from_trusted_keyed_ip_bucket(bucket: [u8; 32]) -> Self {
        Self { ip_bucket: bucket }
    }
}
/// No Debug/Serialize; expose only in the authorized one-shot no-store transport response.
pub struct IssuedCode {
    pub code: PickupCode,
    pub reservation: Uuid,
    pub revision: i64,
    pub expires_at: DateTime<Utc>,
}
pub struct GiftResult {
    pub receipt: SafeReceipt,
    pub issued_code: Option<IssuedCode>,
}
pub struct PgGifts<A> {
    pool: PgPool,
    authority: A,
}
impl<A: GiftAuthority> PgGifts<A> {
    pub fn new(pool: PgPool, authority: A) -> Self {
        Self { pool, authority }
    }
    pub async fn execute(
        &self,
        command: &GiftCommand,
        receipt_keys: &FingerprintKeys,
        pickup_keys: &PickupKeys,
        context: Option<&TrustedPickupContext>,
    ) -> Result<GiftResult, GiftServiceError> {
        let request = command.request()?;
        let producer = GiftProducer {
            command,
            authority: &self.authority,
            pickup_keys,
            error: Mutex::new(None),
            issued: Mutex::new(None),
        };
        if matches!(command.action, GiftAction::Claim { .. }) {
            self.charge_attempt(&producer, &request, receipt_keys, context)
                .await?;
        }
        let result = PgCommandRepository::new(self.pool.clone())
            .execute(&request, command.target(), receipt_keys, &producer)
            .await;
        if let Some(e) = producer
            .error
            .lock()
            .map_err(|_| CommandError::Unavailable)?
            .take()
        {
            return Err(e);
        }
        let receipt = result?;
        if let Some(error) = receipt.failure.and_then(gift_failure_error) {
            return Err(GiftServiceError::Gift(error));
        }
        let issued_code = producer
            .issued
            .lock()
            .map_err(|_| CommandError::Unavailable)?
            .take();
        Ok(GiftResult {
            receipt,
            issued_code,
        })
    }
    pub async fn status(
        &self,
        command: &GiftCommand,
        pickup_keys: &PickupKeys,
    ) -> Result<Option<SafeReceipt>, GiftServiceError> {
        let producer = GiftProducer {
            command,
            authority: &self.authority,
            pickup_keys,
            error: Mutex::new(None),
            issued: Mutex::new(None),
        };
        PgCommandRepository::new(self.pool.clone())
            .lookup(command.scope()?, command.target(), &producer)
            .await
            .map_err(GiftServiceError::Command)
    }
    async fn charge_attempt(
        &self,
        p: &GiftProducer<'_, A>,
        request: &CanonicalRequest,
        keys: &FingerprintKeys,
        context: Option<&TrustedPickupContext>,
    ) -> Result<(), GiftServiceError> {
        let cmd = p.command;
        let mut tx = self.pool.begin().await.map_err(db)?;
        p.authorize(&mut tx, cmd.scope()?, cmd.target()).await?;

        let g = load(&mut tx, cmd.event, cmd.giveaway_id(), true, false)
            .await?
            .ok_or(CommandError::Denied)?;
        let r = g
            .reservations
            .iter()
            .find(|r| r.id == cmd.target())
            .ok_or(CommandError::Denied)?;
        // Resolve terminal/current receipt correlation before spending any new guess budget.
        // BE15 performs authoritative fingerprint/expiry projection after this transaction.
        sqlx::query("select singleton from concert_command_budget where singleton for update")
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
        let receipt_exists:bool=sqlx::query_scalar("select exists(select 1 from concert_command_receipts where actor_id=$1 and event_id=$2 and operation_id=$3 and idempotency_key=$4)").bind(cmd.actor).bind(cmd.event).bind(cmd.operation()).bind(cmd.key).fetch_one(&mut *tx).await.map_err(db)?;
        if receipt_exists {
            tx.commit().await.map_err(db)?;
            return Ok(());
        }
        if let Some(row)=sqlx::query("select reservation_id,request_hash,expires_at from concert_community_pickup_admissions where actor_id=$1 and event_id=$2 and idempotency_key=$3 for update").bind(cmd.actor).bind(cmd.event).bind(cmd.key).fetch_optional(&mut *tx).await.map_err(db)? {
            let expired:bool=sqlx::query_scalar("select clock_timestamp()>=$1").bind(row.try_get::<DateTime<Utc>,_>("expires_at").map_err(db)?).fetch_one(&mut *tx).await.map_err(db)?;
            if expired{return Err(CommandError::Expired.into())}
            let hash=Fingerprint::from_storage(&row.try_get::<String,_>("request_hash").map_err(db)?)?;
            if row.try_get::<Uuid,_>("reservation_id").map_err(db)?!=cmd.target() || !keys.verify(request,&hash)? {return Err(CommandError::Conflict.into())}
            tx.commit().await.map_err(db)?;return Ok(())
        }
        if r.state == ReservationState::Claimed {
            tx.commit().await.map_err(db)?;
            return Ok(());
        }
        p.write_guard(&mut tx, cmd.scope()?).await?;
        let context = context.ok_or(CommandError::Unavailable)?;
        let used: i64 =
            sqlx::query_scalar("select count(*) from concert_community_pickup_admissions")
                .fetch_one(&mut *tx)
                .await
                .map_err(db)?;
        let actor_used:i64=sqlx::query_scalar("select count(*) from concert_community_pickup_admissions where actor_id=$1 and event_id=$2").bind(cmd.actor).bind(cmd.event).fetch_one(&mut *tx).await.map_err(db)?;
        if used >= 1000000 || actor_used >= 1000 {
            return Err(CommandError::Capacity.into());
        }
        // Budget commit is independent from command savepoint rollback, so wrong guesses persist.
        let mut buckets = vec![
            (bucket(b"reservation", r.id.as_bytes()), 5),
            (bucket(b"owner", g.owner.as_bytes()), 25),
            (bucket(b"recipient", r.recipient.as_bytes()), 10),
            (bucket(b"ip", &context.ip_bucket), 25),
        ];
        buckets.sort_by_key(|x| x.0);
        for (key, limit) in buckets {
            let count:Option<i32>=sqlx::query_scalar("insert into concert_community_pickup_attempts(event_id,bucket,window_started_at,attempts) values($1,$2,clock_timestamp(),1) on conflict(event_id,bucket) do update set attempts=case when concert_community_pickup_attempts.window_started_at<=clock_timestamp()-interval '15 minutes' then 1 else concert_community_pickup_attempts.attempts+1 end,window_started_at=case when concert_community_pickup_attempts.window_started_at<=clock_timestamp()-interval '15 minutes' then clock_timestamp() else concert_community_pickup_attempts.window_started_at end where concert_community_pickup_attempts.window_started_at<=clock_timestamp()-interval '15 minutes' or concert_community_pickup_attempts.attempts<$3 returning attempts")
    .bind(cmd.event).bind(key.as_slice()).bind(limit).fetch_optional(&mut *tx).await.map_err(db)?;
            if count.is_none() {
                return Err(CommandError::Capacity.into());
            }
        }
        let fingerprint = keys.sign(request)?;
        sqlx::query("insert into concert_community_pickup_admissions(actor_id,event_id,idempotency_key,reservation_id,request_hash,created_at,expires_at) select $1,$2,$3,$4,$5,t,t+interval '86400 seconds' from (select clock_timestamp() t) q").bind(cmd.actor).bind(cmd.event).bind(cmd.key).bind(cmd.target()).bind(fingerprint.as_storage()).execute(&mut *tx).await.map_err(db)?;
        tx.commit().await.map_err(db)?;
        Ok(())
    }
    pub async fn detail(
        &self,
        actor: Uuid,
        event: Uuid,
        id: Uuid,
    ) -> Result<GiftSummary, GiftServiceError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let g = load(&mut tx, event, id, false, false)
            .await?
            .ok_or(CommandError::Denied)?;
        self.authority
            .authorize_in(&mut tx, actor, event, g.owner, CommunityAccess::ReadGifts)
            .await?;
        if g.state == GiftState::Draft {
            return Err(CommandError::Denied.into());
        }
        let result = GiftSummary::from(&g);
        tx.commit().await.map_err(db)?;
        Ok(result)
    }
    pub async fn reservation(
        &self,
        actor: Uuid,
        event: Uuid,
        giveaway: Uuid,
        reservation: Uuid,
    ) -> Result<Reservation, GiftServiceError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let mut g = load(&mut tx, event, giveaway, true, false)
            .await?
            .ok_or(CommandError::Denied)?;

        let r = g
            .reservations
            .iter()
            .find(|r| r.id == reservation)
            .ok_or(CommandError::Denied)?;
        if actor != g.owner && actor != r.recipient {
            return Err(CommandError::Denied.into());
        }

        let mut subjects = vec![g.owner, r.recipient];
        subjects.sort();
        subjects.dedup();
        for subject in subjects {
            self.authority
                .authorize_in(&mut tx, actor, event, subject, CommunityAccess::ReadGifts)
                .await?;
        }
        let now = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
        if g.advance_time(now, 100).map_err(GiftServiceError::Gift)? > 0 {
            save(&mut tx, &g).await?;
            enqueue_time(&mut tx, &g).await?;
        }
        let r = g
            .reservations
            .iter()
            .find(|r| r.id == reservation)
            .ok_or(CommandError::Denied)?
            .clone();
        tx.commit().await.map_err(db)?;
        Ok(r)
    }
    /// Bounded global due scan; each root has its own atomic transaction and SKIP LOCKED.
    /// No recipient impersonation, process-local timer truth or external side effect.
    pub async fn sweep(&self, limit: i64) -> Result<usize, GiftServiceError> {
        if !(1..=100).contains(&limit) {
            return Err(CommandError::InvalidInput.into());
        }
        let due=sqlx::query("select event_id,giveaway_id from concert_community_giveaways where next_action_at<=clock_timestamp() order by next_action_at,event_id,giveaway_id limit $1").bind(limit).fetch_all(&self.pool).await.map_err(db)?;
        let mut changed = 0;
        for row in due {
            let event: Uuid = row.try_get("event_id").map_err(db)?;
            let id: Uuid = row.try_get("giveaway_id").map_err(db)?;
            let mut tx = self.pool.begin().await.map_err(db)?;
            sqlx::query("select event_id from concert_events where event_id=$1 for share")
                .bind(event)
                .fetch_one(&mut *tx)
                .await
                .map_err(db)?;
            let Some(mut g) = load(&mut tx, event, id, true, true).await? else {
                continue;
            };
            let now = sqlx::query_scalar("select clock_timestamp()")
                .fetch_one(&mut *tx)
                .await
                .map_err(db)?;
            let count = g.advance_time(now, 100).map_err(GiftServiceError::Gift)?;
            if count > 0 {
                save(&mut tx, &g).await?;
                enqueue_time(&mut tx, &g).await?;
                changed += count;
            }
            tx.commit().await.map_err(db)?;
        }
        Ok(changed)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct GiftSummary {
    pub id: Uuid,
    pub event: Uuid,
    pub owner: Uuid,
    pub fields: GiftFields,
    pub state: GiftState,
    pub revision: i64,
    pub available: i32,
    pub pickup_grace_seconds: i32,
    pub created_at: DateTime<Utc>,
}
impl From<&Giveaway> for GiftSummary {
    fn from(g: &Giveaway) -> Self {
        Self {
            id: g.id,
            event: g.event,
            owner: g.owner,
            fields: g.fields.clone(),
            state: g.state,
            revision: g.revision,
            available: g.available(),
            pickup_grace_seconds: g.pickup_grace_seconds,
            created_at: g.created_at,
        }
    }
}
struct GiftProducer<'a, A> {
    command: &'a GiftCommand,
    authority: &'a A,
    pickup_keys: &'a PickupKeys,
    error: Mutex<Option<GiftServiceError>>,
    issued: Mutex<Option<IssuedCode>>,
}
impl<A> GiftProducer<'_, A> {
    fn domain(&self, e: GiftError) -> CommandError {
        if let Ok(mut s) = self.error.lock() {
            *s = Some(GiftServiceError::Gift(e))
        }
        CommandError::InvalidState
    }
}
#[async_trait]
impl<A: GiftAuthority> CommandProducer for GiftProducer<'_, A> {
    async fn completed(
        &self,
        c: &mut PgConnection,
        scope: Scope,
        target: Uuid,
        receipt: &SafeReceipt,
    ) -> Result<(), CommandError> {
        if scope != self.command.scope()? || target != self.command.target() {
            return Err(CommandError::Denied);
        }
        self.authority.completed_in(c, scope, target, receipt).await
    }
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
        let phase: Option<String> =
            sqlx::query_scalar("select phase from concert_events where event_id=$1 for share")
                .bind(scope.event)
                .fetch_optional(&mut *c)
                .await
                .map_err(db)?;
        let Some(phase) = phase else {
            return Err(CommandError::Denied);
        };
        if !matches!(phase.as_str(), "OPEN" | "LIVE" | "READ_ONLY") {
            return Err(CommandError::Denied);
        }
        let gift = load(c, scope.event, cmd.giveaway_id(), true, false).await?;
        let owner = gift.as_ref().map(|g| g.owner).unwrap_or(scope.actor);
        let mut subjects = vec![owner];
        if let Some(g) = gift {
            match cmd.action {
                GiftAction::Create(_)
                | GiftAction::Update { .. }
                | GiftAction::SetMeeting { .. } => {
                    if g.owner != scope.actor {
                        return Err(CommandError::Denied);
                    }
                }
                GiftAction::Reserve { .. } => {}
                GiftAction::Cancel { .. }
                | GiftAction::IssueCode { .. }
                | GiftAction::Claim { .. } => {
                    let r = g
                        .reservations
                        .iter()
                        .find(|r| r.id == target)
                        .ok_or(CommandError::Denied)?;
                    let permitted = match cmd.action {
                        GiftAction::Cancel { .. } => {
                            scope.actor == g.owner || scope.actor == r.recipient
                        }
                        GiftAction::IssueCode { .. } => scope.actor == r.recipient,
                        GiftAction::Claim { .. } => scope.actor == g.owner,
                        _ => false,
                    };
                    if !permitted {
                        return Err(CommandError::Denied);
                    }
                    subjects.push(r.recipient);
                }
            }
        } else if !matches!(cmd.action, GiftAction::Create(_)) {
            return Err(CommandError::Denied);
        }
        subjects.sort();
        subjects.dedup();
        for subject in subjects {
            self.authority
                .authorize_in(
                    c,
                    scope.actor,
                    scope.event,
                    subject,
                    CommunityAccess::ReadGifts,
                )
                .await?;
        }
        Ok(())
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        self.write_guard(c, scope).await?;
        let now = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(db)?;
        let cmd = self.command;
        let settings = self.authority.settings_in(c, scope.event).await?;
        let (gift, change) = if let GiftAction::Create(fields) = &cmd.action {
            let g = Giveaway::create(
                target,
                scope.event,
                scope.actor,
                fields.clone(),
                settings.pickup_grace_seconds,
                now,
            )
            .map_err(|e| self.domain(e))?;
            (
                g,
                GiftChange {
                    resource: target,
                    revision: 1,
                },
            )
        } else {
            let mut g = load(c, scope.event, cmd.giveaway_id(), true, false)
                .await?
                .ok_or(CommandError::Denied)?;
            if let GiftAction::Claim { reservation, .. } = cmd.action {
                let r = g
                    .reservations
                    .iter()
                    .find(|r| r.id == reservation)
                    .ok_or(CommandError::Denied)?;
                if r.state != ReservationState::Claimed {
                    let row=sqlx::query("select key_id,keyed_hash,generation,expires_at from concert_community_pickup_secrets where event_id=$1 and reservation_id=$2 for update").bind(scope.event).bind(reservation).fetch_optional(&mut *c).await.map_err(db)?;
                    let Some(row) = row else {
                        return Ok((Completion::Failed(SafeFailure::PickupCodeInvalid), None));
                    };
                    let expiry: DateTime<Utc> = row.try_get("expires_at").map_err(db)?;
                    let generation: i64 = row.try_get("generation").map_err(db)?;
                    if now >= r.expires_at {
                        return Ok((Completion::Failed(SafeFailure::ReservationExpired), None));
                    }
                    if now >= expiry
                        || r.code_expires_at != Some(expiry)
                        || r.code_generation != generation
                        || !self.pickup_keys.verify(
                            &row.try_get::<String, _>("key_id").map_err(db)?,
                            &row.try_get::<Vec<u8>, _>("keyed_hash").map_err(db)?,
                            scope.event,
                            reservation,
                            generation,
                            expiry,
                            cmd.code.as_ref().ok_or(CommandError::InvalidInput)?,
                        )?
                    {
                        return Ok((Completion::Failed(SafeFailure::PickupCodeInvalid), None));
                    }
                }
            }
            let change = match g.change(scope.actor, &cmd.action, cmd.created_id(), now) {
                Ok(v) => v,
                Err(e) => {
                    if let Some(f) = gift_safe_failure(e) {
                        return Ok((Completion::Failed(f), None));
                    } else {
                        return Err(self.domain(e));
                    }
                }
            };
            (g, change)
        };
        if gift.expires_at() > settings.event_cutoff {
            return Err(CommandError::InvalidInput);
        }
        if matches!(
            cmd.action,
            GiftAction::Create(_) | GiftAction::Update { .. }
        ) {
            self.authority
                .place_in(c, scope.event, gift.fields.public_place_id)
                .await?;
            self.authority
                .media_in(
                    c,
                    scope.event,
                    scope.actor,
                    gift.id,
                    gift.fields.image_media_id,
                )
                .await?;
        }
        if let GiftAction::SetMeeting { meeting, .. } = &cmd.action {
            if meeting.expires_at > settings.event_cutoff {
                return Err(CommandError::InvalidInput);
            }
            self.authority.meeting_in(c, scope.event, meeting).await?;
        }
        if matches!(cmd.action, GiftAction::Reserve { .. }) {
            super::policy::pin_effect_until(c, gift.fields.pickup_ends_at).await?;
        }
        if matches!(
            cmd.action,
            GiftAction::IssueCode { .. } | GiftAction::Claim { .. }
        ) {
            let previous = load(c, scope.event, cmd.giveaway_id(), true, false)
                .await?
                .ok_or(CommandError::Denied)?;
            let r = previous
                .reservations
                .iter()
                .find(|r| r.id == target)
                .ok_or(CommandError::Denied)?;
            if r.state != ReservationState::Claimed {
                super::policy::pin_effect_until(c, r.expires_at).await?;
                if let Some(expiry) = r.code_expires_at {
                    if matches!(cmd.action, GiftAction::Claim { .. }) {
                        super::policy::pin_effect_until(c, expiry).await?;
                    }
                }
            }
        }
        if matches!(cmd.action, GiftAction::IssueCode { .. }) {
            let r = gift
                .reservations
                .iter()
                .find(|r| r.id == target)
                .ok_or(CommandError::InvalidState)?;
            super::policy::pin_effect_until(
                c,
                r.code_expires_at.ok_or(CommandError::InvalidState)?,
            )
            .await?;
        }
        save(c, &gift).await?;
        if let GiftAction::IssueCode { reservation, .. } = cmd.action {
            let r = gift
                .reservations
                .iter()
                .find(|r| r.id == reservation)
                .ok_or(CommandError::InvalidState)?;
            let expiry = r.code_expires_at.ok_or(CommandError::InvalidState)?;
            let previous = sqlx::query("select key_id,keyed_hash,generation,expires_at from concert_community_pickup_secrets where event_id=$1 and reservation_id=$2 for update")
                .bind(scope.event).bind(reservation).fetch_optional(&mut *c).await.map_err(db)?;
            let code = loop {
                let candidate = PickupCode::generate();
                if let Some(old) = &previous {
                    if self.pickup_keys.verify(
                        &old.try_get::<String, _>("key_id").map_err(db)?,
                        &old.try_get::<Vec<u8>, _>("keyed_hash").map_err(db)?,
                        scope.event,
                        reservation,
                        old.try_get("generation").map_err(db)?,
                        old.try_get("expires_at").map_err(db)?,
                        &candidate,
                    )? {
                        continue;
                    }
                }
                break candidate;
            };
            let (key, hash) = self.pickup_keys.sign(
                scope.event,
                reservation,
                r.code_generation,
                expiry,
                &code,
            )?;
            sqlx::query("insert into concert_community_pickup_secrets(event_id,reservation_id,generation,key_id,keyed_hash,expires_at) values($1,$2,$3,$4,$5,$6) on conflict(event_id,reservation_id) do update set generation=excluded.generation,key_id=excluded.key_id,keyed_hash=excluded.keyed_hash,expires_at=excluded.expires_at").bind(scope.event).bind(reservation).bind(r.code_generation).bind(key).bind(hash).bind(expiry).execute(&mut *c).await.map_err(db)?;
            *self.issued.lock().map_err(|_| CommandError::Unavailable)? = Some(IssuedCode {
                code,
                reservation,
                revision: r.revision,
                expires_at: expiry,
            });
        }
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
fn bucket(tag: &[u8], value: &[u8]) -> [u8; 32] {
    let mut hash = Sha256::new();
    hash.update(b"mcr-pickup-attempt:v1");
    hash.update(tag);
    hash.update(value);
    hash.finalize().into()
}
async fn enqueue_time(c: &mut PgConnection, g: &Giveaway) -> Result<(), CommandError> {
    let id = resource_id(
        Uuid::nil(),
        g.event,
        g.id,
        &format!("maintenance:{}", g.revision),
    );
    enqueue_maintenance_in(
        c,
        MaintenanceInvalidation {
            id,
            event: g.event,
            reference: SafeReference {
                resource: g.id,
                revision: g.revision,
            },
        },
    )
    .await?;
    Ok(())
}
pub(super) async fn load(
    c: &mut PgConnection,
    event: Uuid,
    id: Uuid,
    write: bool,
    skip: bool,
) -> Result<Option<Giveaway>, CommandError> {
    // Every caller, including public/private readers, keeps event→root ordering.
    let event_exists: Option<Uuid> =
        sqlx::query_scalar("select event_id from concert_events where event_id=$1 for share")
            .bind(event)
            .fetch_optional(&mut *c)
            .await
            .map_err(db)?;
    if event_exists.is_none() {
        return Ok(None);
    }

    let sql = if !write {
        "select aggregate from concert_community_giveaways where event_id=$1 and giveaway_id=$2 for share"
    } else if skip {
        "select aggregate from concert_community_giveaways where event_id=$1 and giveaway_id=$2 for update skip locked"
    } else {
        "select aggregate from concert_community_giveaways where event_id=$1 and giveaway_id=$2 for update"
    };
    let row = sqlx::query(sql)
        .bind(event)
        .bind(id)
        .fetch_optional(c)
        .await
        .map_err(db)?;
    row.map(|r| {
        let g: Giveaway =
            serde_json::from_value(r.try_get::<serde_json::Value, _>("aggregate").map_err(db)?)
                .map_err(|_| CommandError::InvalidState)?;
        if g.event != event || g.id != id {
            return Err(CommandError::InvalidState);
        }
        g.validate().map_err(|_| CommandError::InvalidState)?;
        Ok(g)
    })
    .transpose()
}
pub(super) async fn save(c: &mut PgConnection, g: &Giveaway) -> Result<(), CommandError> {
    let next = g
        .reservations
        .iter()
        .filter(|r| r.state.active())
        .map(|r| {
            if r.state == ReservationState::Held {
                std::cmp::min(g.fields.pickup_starts_at, r.expires_at)
            } else {
                r.expires_at
            }
        })
        .min();
    sqlx::query("insert into concert_community_giveaways(event_id,giveaway_id,owner_id,revision,state,quantity_total,reserved_unclaimed,quantity_claimed,next_action_at,aggregate) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) on conflict(event_id,giveaway_id) do update set owner_id=excluded.owner_id,revision=excluded.revision,state=excluded.state,quantity_total=excluded.quantity_total,reserved_unclaimed=excluded.reserved_unclaimed,quantity_claimed=excluded.quantity_claimed,next_action_at=excluded.next_action_at,aggregate=excluded.aggregate")
  .bind(g.event).bind(g.id).bind(g.owner).bind(g.revision).bind(state_name(g.state)).bind(g.fields.quantity_total).bind(g.reserved_unclaimed).bind(g.quantity_claimed).bind(next).bind(serde_json::to_value(g).map_err(|_|CommandError::InvalidState)?).execute(&mut *c).await.map_err(db)?;
    for r in &g.reservations {
        sqlx::query("insert into concert_community_reservations(event_id,giveaway_id,reservation_id,recipient_id,state,revision,expires_at) values($1,$2,$3,$4,$5,$6,$7) on conflict(event_id,reservation_id) do update set state=excluded.state,revision=excluded.revision,expires_at=excluded.expires_at").bind(g.event).bind(g.id).bind(r.id).bind(r.recipient).bind(match r.state{ReservationState::Held=>"HELD",ReservationState::Ready=>"READY",ReservationState::Claimed=>"CLAIMED",ReservationState::Cancelled=>"CANCELLED",ReservationState::Expired=>"EXPIRED"}).bind(r.revision).bind(r.expires_at).execute(&mut *c).await.map_err(db)?;
        if !r.state.active() || r.code_expires_at.is_none() {
            sqlx::query("delete from concert_community_pickup_secrets where event_id=$1 and reservation_id=$2").bind(g.event).bind(r.id).execute(&mut *c).await.map_err(db)?;
        }
    }
    Ok(())
}

pub struct GiftFilter {
    pub state: Option<GiftState>,
    pub in_stock: Option<bool>,
    pub pickup_after: Option<DateTime<Utc>>,
    pub pickup_before: Option<DateTime<Utc>>,
    pub public_place_id: Option<Uuid>,
    pub after: Option<(DateTime<Utc>, Uuid)>,
}
pub struct GiftPage {
    pub items: Vec<GiftSummary>,
    pub next: Option<(DateTime<Utc>, Uuid)>,
}
pub struct PickupView {
    pub reservation: Reservation,
    pub meeting: PickupMeeting,
    pub meeting_revision: i64,
}
impl<A: GiftAuthority> PgGifts<A> {
    pub async fn list(
        &self,
        actor: Uuid,
        event: Uuid,
        filter: GiftFilter,
        limit: i64,
    ) -> Result<GiftPage, GiftServiceError> {
        if !(1..=100).contains(&limit)
            || filter.state == Some(GiftState::Draft)
            || filter
                .pickup_after
                .zip(filter.pickup_before)
                .is_some_and(|(a, b)| a >= b)
        {
            return Err(CommandError::InvalidInput.into());
        }
        let mut tx = self.pool.begin().await.map_err(db)?;
        self.authority
            .actor_in(&mut tx, actor, event, CommunityAccess::ReadGifts)
            .await?;
        let rows=sqlx::query("select giveaway_id from concert_community_giveaways where event_id=$1 and state<>'DRAFT' and ($2::text is null or state=$2) and ($3::boolean is null or (quantity_total-reserved_unclaimed-quantity_claimed>0)=$3) and ($4::timestamptz is null or (aggregate->'fields'->>'pickup_starts_at')::timestamptz>=$4) and ($5::timestamptz is null or (aggregate->'fields'->>'pickup_starts_at')::timestamptz<$5) and ($6::uuid is null or (aggregate->'fields'->>'public_place_id')::uuid=$6) and ($7::timestamptz is null or ((aggregate->>'created_at')::timestamptz,giveaway_id)>($7,$8)) order by (aggregate->>'created_at')::timestamptz,giveaway_id limit $9")
   .bind(event).bind(filter.state.map(state_name)).bind(filter.in_stock).bind(filter.pickup_after).bind(filter.pickup_before).bind(filter.public_place_id).bind(filter.after.map(|x|x.0)).bind(filter.after.map(|x|x.1)).bind(limit).fetch_all(&mut *tx).await.map_err(db)?;
        tx.commit().await.map_err(db)?;
        let mut items = vec![];
        let mut next = None;
        for row in rows {
            let mut tx = self.pool.begin().await.map_err(db)?;
            let id: Uuid = row.try_get("giveaway_id").map_err(db)?;
            let g = load(&mut tx, event, id, false, false)
                .await?
                .ok_or(CommandError::InvalidState)?;
            next = Some((g.created_at, id));
            match self
                .authority
                .authorize_in(&mut tx, actor, event, g.owner, CommunityAccess::ReadGifts)
                .await
            {
                Ok(()) => items.push(GiftSummary::from(&g)),
                Err(CommandError::Denied) => continue,
                Err(e) => return Err(e.into()),
            }
            tx.commit().await.map_err(db)?;
        }
        Ok(GiftPage { items, next })
    }
    pub async fn pickup(
        &self,
        actor: Uuid,
        event: Uuid,
        giveaway: Uuid,
        reservation: Uuid,
    ) -> Result<PickupView, GiftServiceError> {
        // Lazy DB-clock readiness/expiry is a durable maintenance transition before projection.
        self.reservation(actor, event, giveaway, reservation)
            .await?;
        let mut tx = self.pool.begin().await.map_err(db)?;
        let g = load(&mut tx, event, giveaway, false, false)
            .await?
            .ok_or(CommandError::Denied)?;
        self.authority
            .authorize_in(&mut tx, actor, event, g.owner, CommunityAccess::ReadGifts)
            .await?;
        let r = g
            .reservations
            .iter()
            .find(|r| r.id == reservation && r.recipient == actor)
            .ok_or(CommandError::Denied)?;
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
        if !r.state.active() || now >= r.expires_at || !g.state.active() {
            return Err(CommandError::Denied.into());
        }
        let meeting = g.meeting.as_ref().ok_or(CommandError::Denied)?;
        if now >= meeting.expires_at {
            return Err(CommandError::Denied.into());
        }
        self.authority.meeting_in(&mut tx, event, meeting).await?;
        let result = PickupView {
            reservation: r.clone(),
            meeting: meeting.clone(),
            meeting_revision: g.meeting_revision,
        };
        tx.commit().await.map_err(db)?;
        Ok(result)
    }
}

fn gift_safe_failure(e: GiftError) -> Option<SafeFailure> {
    match e {
        GiftError::SoldOut => Some(SafeFailure::GiveawaySoldOut),
        GiftError::DuplicateReservation => Some(SafeFailure::DuplicateActiveReservation),
        GiftError::Expired => Some(SafeFailure::ReservationExpired),
        GiftError::CodeInvalid => Some(SafeFailure::PickupCodeInvalid),
        GiftError::VersionConflict => Some(SafeFailure::VersionConflict),
        GiftError::InvalidTransition => Some(SafeFailure::InvalidTransition),
        _ => None,
    }
}
fn gift_failure_error(e: SafeFailure) -> Option<GiftError> {
    match e {
        SafeFailure::GiveawaySoldOut => Some(GiftError::SoldOut),
        SafeFailure::DuplicateActiveReservation => Some(GiftError::DuplicateReservation),
        SafeFailure::ReservationExpired => Some(GiftError::Expired),
        SafeFailure::PickupCodeInvalid => Some(GiftError::CodeInvalid),
        SafeFailure::VersionConflict => Some(GiftError::VersionConflict),
        SafeFailure::InvalidTransition => Some(GiftError::InvalidTransition),
        _ => None,
    }
}
impl<A: GiftAuthority> GiftProducer<'_, A> {
    async fn write_guard(&self, c: &mut PgConnection, scope: Scope) -> Result<(), CommandError> {
        let phase: String =
            sqlx::query_scalar("select phase from concert_events where event_id=$1 for share")
                .bind(scope.event)
                .fetch_one(&mut *c)
                .await
                .map_err(db)?;
        if !matches!(phase.as_str(), "OPEN" | "LIVE")
            && !(self.command.safety() && phase == "READ_ONLY")
        {
            if let Ok(mut e) = self.error.lock() {
                *e = Some(GiftServiceError::EventReadOnly)
            }
            return Err(CommandError::Denied);
        }
        let g = load(c, scope.event, self.command.giveaway_id(), true, false).await?;
        let owner = g.as_ref().map(|g| g.owner).unwrap_or(scope.actor);
        let access = CommunityAccess::gift(true, self.command.safety());
        let mut subjects = vec![owner];
        if let Some(g) = g {
            if matches!(
                self.command.action,
                GiftAction::Cancel { .. } | GiftAction::IssueCode { .. } | GiftAction::Claim { .. }
            ) {
                let r = g
                    .reservations
                    .iter()
                    .find(|r| r.id == self.command.target())
                    .ok_or(CommandError::Denied)?;
                subjects.push(r.recipient);
            }
        }
        subjects.sort();
        subjects.dedup();
        for subject in subjects {
            self.authority
                .authorize_in(c, scope.actor, scope.event, subject, access)
                .await?;
        }
        Ok(())
    }
}

#[async_trait]
impl GiftAuthority for super::UnavailableAuthority {
    async fn settings_in(
        &self,
        _: &mut PgConnection,
        _: Uuid,
    ) -> Result<GiftSettings, CommandError> {
        Err(CommandError::Unavailable)
    }
    async fn media_in(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        _: Uuid,
        _: Uuid,
        _: Option<Uuid>,
    ) -> Result<(), CommandError> {
        Err(CommandError::Unavailable)
    }
    async fn meeting_in(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        _: &PickupMeeting,
    ) -> Result<(), CommandError> {
        Err(CommandError::Unavailable)
    }
}

pub struct GiftManagement {
    pub giveaway: GiftSummary,
    pub reserved_unclaimed: i32,
    pub quantity_claimed: i32,
    pub reservations: Vec<Reservation>,
    pub next: Option<Uuid>,
}
impl<A: GiftAuthority> PgGifts<A> {
    pub async fn management(
        &self,
        actor: Uuid,
        event: Uuid,
        giveaway: Uuid,
        state: Option<ReservationState>,
        after: Option<Uuid>,
        limit: usize,
    ) -> Result<GiftManagement, GiftServiceError> {
        if !(1..=100).contains(&limit) {
            return Err(CommandError::InvalidInput.into());
        }
        let mut tx = self.pool.begin().await.map_err(db)?;
        let g = load(&mut tx, event, giveaway, false, false)
            .await?
            .ok_or(CommandError::Denied)?;
        if actor != g.owner {
            return Err(CommandError::Denied.into());
        }
        let mut candidates: Vec<_> = g
            .reservations
            .iter()
            .filter(|r| state.is_none_or(|s| r.state == s) && after.is_none_or(|id| r.id > id))
            .collect();
        candidates.sort_by_key(|r| r.id);
        candidates.truncate(limit);
        let next = candidates.last().map(|r| r.id);
        let mut subjects: Vec<_> = candidates.iter().map(|r| r.recipient).collect();
        subjects.push(actor);
        subjects.sort();
        subjects.dedup();
        let mut allowed = std::collections::HashSet::new();
        for subject in subjects {
            match self
                .authority
                .authorize_in(&mut tx, actor, event, subject, CommunityAccess::ReadGifts)
                .await
            {
                Ok(()) => {
                    allowed.insert(subject);
                }
                Err(CommandError::Denied) if subject != actor => {}
                Err(e) => return Err(e.into()),
            }
        }
        let reservations = candidates
            .into_iter()
            .filter(|r| allowed.contains(&r.recipient))
            .cloned()
            .collect();
        let result = GiftManagement {
            giveaway: GiftSummary::from(&g),
            reserved_unclaimed: g.reserved_unclaimed,
            quantity_claimed: g.quantity_claimed,
            reservations,
            next,
        };
        tx.commit().await.map_err(db)?;
        Ok(result)
    }
}
