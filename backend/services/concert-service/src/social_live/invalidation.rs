//! Durable, removal-only Social policy metadata. Root owns sockets and dispatch.
//! Database credentials are a trusted internal boundary, never client input.
use crate::{
    adapters::out::repository::outbox::{Delivery, DeliveryFailure},
    workers::outbox::IdempotentInvalidationSink,
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::commands::maintenance::InvalidationKind;
use sqlx::{postgres::PgRow, PgConnection, PgPool, Postgres, Row, Transaction};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PolicyReason {
    SourceChanged,
    ConsentChanged,
    BlockChanged,
    Expired,
    CanonicalPolicyChanged,
}
impl PolicyReason {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::SourceChanged => "SOURCE_CHANGED",
            Self::ConsentChanged => "CONSENT_CHANGED",
            Self::BlockChanged => "BLOCK_CHANGED",
            Self::Expired => "EXPIRED",
            Self::CanonicalPolicyChanged => "CANONICAL_POLICY_CHANGED",
        }
    }
    fn parse(s: &str) -> Result<Self, DeliveryFailure> {
        match s {
            "SOURCE_CHANGED" => Ok(Self::SourceChanged),
            "CONSENT_CHANGED" => Ok(Self::ConsentChanged),
            "BLOCK_CHANGED" => Ok(Self::BlockChanged),
            "EXPIRED" => Ok(Self::Expired),
            "CANONICAL_POLICY_CHANGED" => Ok(Self::CanonicalPolicyChanged),
            _ => Err(DeliveryFailure::Rejected),
        }
    }
}

/// Constructed only from a locked, source-verified durable signal. No Serialize:
/// the socket owner supplies streamId and excludes actor/internal signal IDs.
pub struct VerifiedSocialInvalidation {
    signal: Uuid,
    event: Uuid,
    actor: Uuid,
    revision: i64,
    reason: PolicyReason,
}
impl VerifiedSocialInvalidation {
    pub fn signal_id(&self) -> Uuid {
        self.signal
    }
    pub fn event(&self) -> Uuid {
        self.event
    }
    pub fn actor(&self) -> Uuid {
        self.actor
    }
    pub fn policy_revision(&self) -> i64 {
        self.revision
    }
    pub fn reason(&self) -> PolicyReason {
        self.reason
    }
}

/// Success means every matching current event/actor stream has applied this
/// invalidation or durably lost its private view. No-active-streams is success
/// only when a gated reconnect reads the durable baseline. Never grant access.
/// Dedup by actor/event/revision under the actual publication fence; late/older
/// retries cannot decrease a baseline. Gap/overflow fences private views until
/// fresh authoritative HTTP revalidation. Never send on a different tuple.
#[async_trait]
pub trait SocialPolicyPublisher: Send + Sync {
    async fn publish(&self, signal: &VerifiedSocialInvalidation) -> Result<(), DeliveryFailure>;
}

#[derive(Clone)]
pub struct SocialInvalidationSink {
    pool: PgPool,
    publisher: Arc<dyn SocialPolicyPublisher>,
}
/// Keep Root's stream gated until its matching tuple and baseline are installed.
/// Dropping/failed commit requires Root to withdraw the gated stream. No private
/// read may start before commit succeeds. Writers cannot advance this tuple
/// while the row's SHARE lock is held. Never hold across network or user waits.
pub struct BaselineGuard {
    tx: Transaction<'static, Postgres>,
    revision: i64,
}
impl BaselineGuard {
    pub fn revision(&self) -> i64 {
        self.revision
    }
    pub async fn commit(self) -> Result<(), DeliveryFailure> {
        self.tx.commit().await.map_err(retry)
    }
}
impl SocialInvalidationSink {
    pub fn new(pool: PgPool, publisher: Arc<dyn SocialPolicyPublisher>) -> Self {
        Self { pool, publisher }
    }

    /// Root must supply its actual canonical actor/event and hold its gated
    /// socket bootstrap until registration + this baseline are reconciled.
    /// This is an observation, not membership, authorization or a permit.
    pub async fn baseline(&self, event: Uuid, actor: Uuid) -> Result<i64, DeliveryFailure> {
        let guard = self.held_baseline(event, actor).await?;
        let revision = guard.revision();
        guard.commit().await?;
        Ok(revision)
    }

    pub async fn held_baseline(
        &self,
        event: Uuid,
        actor: Uuid,
    ) -> Result<BaselineGuard, DeliveryFailure> {
        if event.is_nil() || actor.is_nil() {
            return Err(DeliveryFailure::Rejected);
        }
        let mut tx = self.pool.begin().await.map_err(retry)?;
        bound(&mut tx).await?;
        sqlx::query("insert into public.concert_social_policy_generations(event_id,actor_id,revision) values($1,$2,1) on conflict do nothing")
            .bind(event).bind(actor).execute(&mut *tx).await.map_err(retry)?;
        let revision = sqlx::query_scalar("select revision from public.concert_social_policy_generations where event_id=$1 and actor_id=$2 for share")
            .bind(event).bind(actor).fetch_one(&mut *tx).await.map_err(retry)?;
        Ok(BaselineGuard { tx, revision })
    }

    /// A dispatcher must route other families to their actual owner. False is
    /// never an acknowledgement. Actual delivery rechecks the full live lease.
    pub async fn supports(&self, d: &Delivery) -> Result<bool, DeliveryFailure> {
        if d.kind != InvalidationKind::ResourceInvalidated || d.receipt.is_none() {
            return Ok(false);
        }
        sqlx::query_scalar("select exists(select 1 from public.concert_social_policy_signals where source_outbox=$1 and source_receipt=$2 and event_id=$3 and source_resource=$4 and source_revision=$5)")
            .bind(d.id).bind(d.receipt).bind(d.event).bind(d.reference.resource).bind(d.reference.revision)
            .fetch_one(&self.pool).await.map_err(retry)
    }

    async fn publish_row(&self, c: &mut PgConnection, row: PgRow) -> Result<(), DeliveryFailure> {
        let signal = VerifiedSocialInvalidation {
            signal: row.get("signal_id"),
            event: row.get("event_id"),
            actor: row.get("actor_id"),
            revision: row.get("policy_revision"),
            reason: PolicyReason::parse(row.get::<String, _>("reason").as_str())?,
        };
        if signal.event.is_nil()
            || signal.actor.is_nil()
            || !(1..=9_007_199_254_740_991).contains(&signal.revision)
        {
            return Err(DeliveryFailure::Rejected);
        }
        tokio::time::timeout(Duration::from_secs(2), self.publisher.publish(&signal))
            .await
            .map_err(retry)??;
        sqlx::query("update public.concert_social_policy_signals set published_at=clock_timestamp() where signal_id=$1 and published_at is null")
            .bind(signal.signal).execute(c).await.map_err(retry)?;
        Ok(())
    }

    /// Bounded server-time discovery. Only removes cached views; it does NOT
    /// mutate grants, fabricate a command receipt or authorize reveal. Each
    /// actual source deadline is durably deduplicated before socket delivery.
    pub async fn emit_expired(&self, limit: i64) -> Result<usize, DeliveryFailure> {
        if !(1..=100).contains(&limit) {
            return Err(DeliveryFailure::Rejected);
        }
        let mut emitted = 0;
        for _ in 0..limit {
            let mut tx = self.pool.begin().await.map_err(retry)?;
            bound(&mut tx).await?;
            let row = sqlx::query(EXPIRED)
                .fetch_optional(&mut *tx)
                .await
                .map_err(retry)?;
            let Some(row) = row else {
                break;
            };
            let event: Uuid = row.get("event_id");
            let request: Uuid = row.get("request_id");
            let deadline: DateTime<Utc> = row.get("deadline");
            let a: Uuid = row.get("requester_id");
            let b: Uuid = row.get("recipient_id");
            for actor in [a.min(b), a.max(b)] {
                let gen: i64 = sqlx::query_scalar("insert into public.concert_social_policy_generations as g(event_id,actor_id,revision) values($1,$2,2) on conflict(event_id,actor_id) do update set revision=g.revision+1 returning revision")
                    .bind(event).bind(actor).fetch_one(&mut *tx).await.map_err(retry)?;
                sqlx::query("insert into public.concert_social_policy_signals(signal_id,event_id,actor_id,policy_revision,reason,deadline_request,deadline_at) values($1,$2,$3,$4,'EXPIRED',$5,$6)")
                    .bind(Uuid::new_v4()).bind(event).bind(actor).bind(gen).bind(request).bind(deadline)
                    .execute(&mut *tx).await.map_err(retry)?;
            }
            tx.commit().await.map_err(retry)?;
            emitted += 1;
        }
        Ok(emitted)
    }

    /// Drain only server-deadline signals. Common outbox signals MUST use their
    /// actual active Delivery lease through deliver, never this alternate path.
    pub async fn drain_pending(&self, limit: i64) -> Result<usize, DeliveryFailure> {
        if !(1..=100).contains(&limit) {
            return Err(DeliveryFailure::Rejected);
        }
        let mut count = 0;
        for _ in 0..limit {
            let mut tx = self.pool.begin().await.map_err(retry)?;
            bound(&mut tx).await?;
            let row = sqlx::query("select * from public.concert_social_policy_signals where source_outbox is null and published_at is null order by created_at,signal_id limit 1 for update skip locked")
                .fetch_optional(&mut *tx).await.map_err(retry)?;
            let Some(row) = row else {
                break;
            };
            self.publish_row(&mut tx, row).await?;
            tx.commit().await.map_err(retry)?;
            count += 1;
        }
        Ok(count)
    }
}

#[async_trait]
impl IdempotentInvalidationSink for SocialInvalidationSink {
    async fn deliver(&self, d: &Delivery) -> Result<(), DeliveryFailure> {
        if d.kind != InvalidationKind::ResourceInvalidated || d.receipt.is_none() {
            return Err(DeliveryFailure::Rejected);
        }
        let mut tx = self.pool.begin().await.map_err(retry)?;
        bound(&mut tx).await?;
        // Source snapshot was generated only by the successful receipt trigger.
        // Retirement may clear result projection, never immutable receipt tuple.
        let round_until = Instant::now() + Duration::from_secs(6);
        let valid: bool = sqlx::query_scalar("select exists(select 1 from public.concert_outbox o join public.concert_command_receipts r on r.receipt_id=o.receipt_id where o.outbox_id=$1 and o.receipt_id=$2 and o.event_id=$3 and o.resource_id=$4 and o.resource_revision=$5 and o.kind='RESOURCE_INVALIDATED' and o.state='LEASED' and o.lease_token=$6 and o.attempts=$7 and o.lease_until>clock_timestamp()+interval '10 seconds' and r.event_id=o.event_id and r.target_id=o.resource_id and r.state in ('SUCCEEDED','RETIRED') and exists(select 1 from public.concert_social_policy_signals s where s.source_outbox=o.outbox_id and s.source_receipt=r.receipt_id and s.source_operation=r.operation_id and s.source_resource=o.resource_id and s.source_revision=o.resource_revision and s.event_id=o.event_id))")
            .bind(d.id).bind(d.receipt).bind(d.event).bind(d.reference.resource).bind(d.reference.revision).bind(d.lease).bind(d.attempt)
            .fetch_one(&mut *tx).await.map_err(retry)?;
        if !valid {
            return Err(DeliveryFailure::Retryable);
        }
        // Keep this round bounded irrespective of historical audience size.
        let rows = sqlx::query("select * from public.concert_social_policy_signals where source_outbox=$1 and published_at is null order by actor_id limit 4096 for update")
            .bind(d.id).fetch_all(&mut *tx).await.map_err(retry)?;
        for row in rows {
            if Instant::now() >= round_until {
                break;
            }
            self.publish_row(&mut tx, row).await?;
        }
        let pending: bool = sqlx::query_scalar("select exists(select 1 from public.concert_social_policy_signals where source_outbox=$1 and published_at is null)")
            .bind(d.id).fetch_one(&mut *tx).await.map_err(retry)?;
        let live: bool = sqlx::query_scalar("select exists(select 1 from public.concert_outbox where outbox_id=$1 and state='LEASED' and lease_token=$2 and attempts=$3 and lease_until>clock_timestamp())")
            .bind(d.id).bind(d.lease).bind(d.attempt).fetch_one(&mut *tx).await.map_err(retry)?;
        if !live {
            return Err(DeliveryFailure::Retryable);
        }
        tx.commit().await.map_err(retry)?;
        if pending {
            Err(DeliveryFailure::Retryable)
        } else {
            Ok(())
        }
    }
}

async fn bound(c: &mut PgConnection) -> Result<(), DeliveryFailure> {
    sqlx::query("set local idle_in_transaction_session_timeout='3s'")
        .execute(&mut *c)
        .await
        .map_err(retry)?;
    sqlx::query("set local statement_timeout='2s'")
        .execute(&mut *c)
        .await
        .map_err(retry)?;
    sqlx::query("set local lock_timeout='500ms'")
        .execute(c)
        .await
        .map_err(retry)?;
    Ok(())
}
fn retry<T>(_: T) -> DeliveryFailure {
    DeliveryFailure::Retryable
}

// Select canonical claim before considering its deadline (never resurrect an
// older verified row). Birthday is a policy boundary even if the claim lives.
// Request expiry is immutable and always present. No caller-supplied clock.
const EXPIRED: &str = "select r.event_id,r.request_id,r.requester_id,r.recipient_id,d.deadline from public.concert_social_requests r cross join lateral (select min(t) deadline from (select r.expires_at t union all select expires_at from public.concert_social_grants where event_id=r.event_id and request_id=r.request_id and state='ACTIVE' union all select expires_at from public.concert_intros where event_id=r.event_id and intro_id=r.intro_id union all select q.expires_at from (select expires_at from public.concert_age_claims where user_id=r.requester_id order by verified_at desc,expires_at desc,claim_id desc limit 1) q union all select q.expires_at from (select expires_at from public.concert_age_claims where user_id=r.recipient_id order by verified_at desc,expires_at desc,claim_id desc limit 1) q union all select q.birthday from (select (next_birthday_at::timestamp at time zone 'Asia/Seoul') birthday from public.concert_age_claims where user_id=r.requester_id order by verified_at desc,expires_at desc,claim_id desc limit 1) q union all select q.birthday from (select (next_birthday_at::timestamp at time zone 'Asia/Seoul') birthday from public.concert_age_claims where user_id=r.recipient_id order by verified_at desc,expires_at desc,claim_id desc limit 1) q) deadlines where t is not null and not exists(select 1 from public.concert_social_policy_signals s where s.event_id=r.event_id and s.deadline_request=r.request_id and s.deadline_at=t)) d where r.state in ('PENDING','APPROVED') and d.deadline<=clock_timestamp() and not exists(select 1 from public.concert_social_policy_signals s where s.event_id=r.event_id and s.deadline_request=r.request_id and s.deadline_at=d.deadline) order by d.deadline,r.event_id,r.request_id limit 1 for update of r skip locked";
