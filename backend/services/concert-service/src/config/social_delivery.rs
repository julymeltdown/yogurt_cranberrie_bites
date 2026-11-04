//! Opt-in Social delivery. Only complete, explicitly owned fanout families are
//! claimed. This is not the global outbox dispatcher or an authority source.
use crate::world::invalidation::WorldInvalidationConsumer;
use crate::{
    adapters::out::{
        notification::{
            delivery::NotificationDeliveryClient, routing::SocialProjectionRouting,
            source::PgProjectionOwner, NotificationSink,
        },
        repository::outbox::{Delivery, DeliveryFailure, PgOutbox},
    },
    social_live::invalidation::SocialInvalidationSink,
    workers::outbox::{BatchResult, IdempotentInvalidationSink},
};
use concert_application::commands::{maintenance::InvalidationKind, SafeReference};
use sqlx::{PgConnection, PgPool, Row};
use std::{future::Future, sync::Arc, time::Duration};
use tokio::sync::oneshot;
use uuid::Uuid;

const BATCH: i64 = 8;
const DELIVERY_BUDGET: Duration = Duration::from_secs(10);
type Notification =
    NotificationSink<PgProjectionOwner<SocialProjectionRouting>, NotificationDeliveryClient>;

pub struct SocialDeliveryRuntime {
    worker: DeliveryWorker,
    world_receiver: Option<oneshot::Receiver<Arc<WorldInvalidationConsumer>>>,
}
#[derive(Clone)]
struct DeliveryWorker {
    pool: PgPool,
    social: SocialInvalidationSink,
    notification: Arc<Notification>,
    world: Option<Arc<WorldInvalidationConsumer>>,
}
impl SocialDeliveryRuntime {
    /// Root supplies the actual single-World publisher and mTLS Notification
    /// transport. Construction performs no IO and installs no fallback owner.
    pub fn new(
        pool: PgPool,
        sink: SocialInvalidationSink,
        notification: Arc<NotificationDeliveryClient>,
    ) -> Self {
        Self {
            world_receiver: None,
            worker: DeliveryWorker {
                world: None,
                notification: Arc::new(NotificationSink {
                    projection: PgProjectionOwner::new(pool.clone(), SocialProjectionRouting),
                    owner: notification.as_ref().clone(),
                }),
                pool,
                social: sink,
            },
        }
    }

    /// Supply only the actual consumer of Root's ready, single World runtime.
    /// No claim is possible until this receiver resolves. Closure is an error.
    pub fn with_world(
        mut self,
        receiver: oneshot::Receiver<Arc<WorldInvalidationConsumer>>,
    ) -> Self {
        self.world_receiver = Some(receiver);
        self
    }

    /// Root supervises this future. Unexpected DB/expiry failures are returned,
    /// never logged with raw database values or replaced by successful delivery.
    pub async fn run(self) -> anyhow::Result<()> {
        self.run_until(platform::shutdown::global().cancelled())
            .await
    }

    /// Cancellation drops in-flight transactions and aborts this batch's tasks.
    /// A possibly completed external effect retries under its original UUID;
    /// cancellation is never an ACK. No task or network wait escapes this future.
    pub async fn run_until(mut self, stop: impl Future<Output = ()>) -> anyhow::Result<()> {
        tokio::pin!(stop);
        if let Some(receiver) = self.world_receiver.take() {
            self.worker.world = Some(tokio::select! {
                biased;
                _ = &mut stop => return Ok(()),
                result = receiver => result.map_err(|_| unavailable())?,
            });
        }
        tokio::select! {
            biased;
            _ = &mut stop => Ok(()),
            result = async {
                tokio::try_join!(self.outbox_loop(), self.expiry_loop())?;
                Ok(())
            } => result,
        }
    }

    async fn outbox_loop(&self) -> anyhow::Result<()> {
        let mut tick = tokio::time::interval(Duration::from_millis(500));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            self.run_batch().await?;
        }
    }
    async fn expiry_loop(&self) -> anyhow::Result<()> {
        let mut tick = tokio::time::interval(Duration::from_secs(1));
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tick.tick().await;
            self.run_expiry_batch().await?;
        }
    }

    /// Bounded PostgreSQL-clock expiry emission followed by actual publisher
    /// handoff. It touches deadline signals only, never command outbox ACKs.
    pub async fn run_expiry_batch(&self) -> anyhow::Result<()> {
        tokio::time::timeout(Duration::from_secs(30), async {
            self.worker
                .social
                .emit_expired(4)
                .await
                .map_err(|_| unavailable())?;
            self.worker
                .social
                .drain_pending(8)
                .await
                .map_err(|_| unavailable())?;
            Ok::<_, anyhow::Error>(())
        })
        .await
        .map_err(|_| unavailable())?
    }

    pub async fn run_batch(&self) -> anyhow::Result<BatchResult> {
        anyhow::ensure!(
            self.world_receiver.is_none(),
            "Social World delivery is not ready"
        );
        let deliveries = tokio::time::timeout(Duration::from_secs(5), self.claim())
            .await
            .map_err(|_| unavailable())??;
        let mut result = BatchResult {
            claimed: deliveries.len(),
            ..BatchResult::default()
        };
        let mut tasks = tokio::task::JoinSet::new();
        for delivery in deliveries {
            let owner = self.worker.clone();
            tasks.spawn(async move {
                let outcome = tokio::time::timeout(DELIVERY_BUDGET, owner.deliver(&delivery))
                    .await
                    .unwrap_or(Err(DeliveryFailure::Retryable));
                // The unchanged repository enforces the original token,
                // attempt and live DB-clock lease for both success and failure.
                let outbox = PgOutbox::new(owner.pool.clone());
                let success = outcome.is_ok();
                let accepted = tokio::time::timeout(Duration::from_secs(5), async {
                    match outcome {
                        Ok(()) => outbox.acknowledge(&delivery).await,
                        Err(error) => outbox.fail(&delivery, error).await,
                    }
                })
                .await
                .map_err(|_| unavailable())?
                .map_err(|_| unavailable())?;
                Ok::<_, anyhow::Error>((accepted, success))
            });
        }
        while let Some(outcome) = tasks.join_next().await {
            let (accepted, success) = outcome.map_err(|_| unavailable())??;
            if !accepted {
                result.stale += 1;
            } else if success {
                result.acknowledged += 1;
            } else {
                result.failed += 1;
            }
        }
        Ok(result)
    }

    async fn claim(&self) -> anyhow::Result<Vec<Delivery>> {
        claim_scoped(
            &self.worker.pool,
            self.worker.world.as_ref().map(|world| world.event_id()),
        )
        .await
    }
}

impl DeliveryWorker {
    async fn deliver(&self, d: &Delivery) -> Result<(), DeliveryFailure> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| DeliveryFailure::Retryable)?;
        bound(&mut tx)
            .await
            .map_err(|_| DeliveryFailure::Retryable)?;
        let source = source_predicate("$8");
        let query = format!("select r.operation_id from public.concert_outbox o join public.concert_command_receipts r on r.receipt_id=o.receipt_id where {source} and o.outbox_id=$1 and o.receipt_id=$2 and o.event_id=$3 and o.resource_id=$4 and o.resource_revision=$5 and o.state='LEASED' and o.lease_token=$6 and o.attempts=$7 and o.lease_until>clock_timestamp()+interval '10 seconds'");
        let op: Option<String> = sqlx::query_scalar(&query)
            .bind(d.id)
            .bind(d.receipt)
            .bind(d.event)
            .bind(d.reference.resource)
            .bind(d.reference.revision)
            .bind(d.lease)
            .bind(d.attempt)
            .bind(self.world.as_ref().map(|world| world.event_id()))
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| DeliveryFailure::Retryable)?;
        let op = op.ok_or(DeliveryFailure::Retryable)?;
        tx.commit().await.map_err(|_| DeliveryFailure::Retryable)?;
        // Source locks are released before either IO leg. Each real adapter
        // independently validates its source; final ACK still checks live lease.
        self.social.deliver(d).await?;
        if matches!(op.as_str(), "blockConcertUser" | "unblockConcertUser") {
            self.world
                .as_ref()
                .ok_or(DeliveryFailure::Retryable)?
                .apply(d)
                .await
                .map_err(DeliveryFailure::from)?;
        }
        if requires_notification(&op) {
            self.notification.deliver(d).await?;
        }
        Ok(())
    }
}

async fn claim_scoped(pool: &PgPool, world_event: Option<Uuid>) -> anyhow::Result<Vec<Delivery>> {
    let source = source_predicate("$2");
    let mut tx = pool.begin().await.map_err(|_| unavailable())?;
    bound(&mut tx).await?;
    // Apply exhaustion only to the same sealed, explicitly owned families.
    // Foreign/unknown/block/canonical rows retain state AND attempt budget.
    let exhausted = format!("with picked as (select o.outbox_id from public.concert_outbox o join public.concert_command_receipts r on r.receipt_id=o.receipt_id where {source} and o.state='LEASED' and o.lease_until<=clock_timestamp() and o.attempts>=8 order by o.lease_until,o.outbox_id for update of o skip locked limit $1) update public.concert_outbox o set state='DEAD',lease_token=null,lease_until=null,terminal_at=clock_timestamp(),last_error='ATTEMPTS_EXHAUSTED' from picked p where o.outbox_id=p.outbox_id");
    sqlx::query(&exhausted)
        .bind(BATCH)
        .bind(world_event)
        .execute(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    let source = source_predicate("$3");
    let claim = format!("with picked as (select o.outbox_id from public.concert_outbox o join public.concert_command_receipts r on r.receipt_id=o.receipt_id where {source} and o.attempts<8 and ((o.state='READY' and o.available_at<=clock_timestamp()) or (o.state='LEASED' and o.lease_until<=clock_timestamp())) order by o.available_at,o.created_at,o.outbox_id for update of o skip locked limit $1) update public.concert_outbox o set state='LEASED',attempts=o.attempts+1,lease_token=$2,lease_until=clock_timestamp()+interval '30 seconds' from picked p where o.outbox_id=p.outbox_id returning o.*");
    let rows = sqlx::query(&claim)
        .bind(BATCH)
        .bind(Uuid::new_v4())
        .bind(world_event)
        .fetch_all(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    let mut deliveries = Vec::with_capacity(rows.len());
    for row in rows {
        deliveries.push(Delivery {
            id: row.try_get("outbox_id").map_err(|_| unavailable())?,
            receipt: row.try_get("receipt_id").map_err(|_| unavailable())?,
            event: row.try_get("event_id").map_err(|_| unavailable())?,
            kind: InvalidationKind::ResourceInvalidated,
            reference: SafeReference {
                resource: row.try_get("resource_id").map_err(|_| unavailable())?,
                revision: row
                    .try_get("resource_revision")
                    .map_err(|_| unavailable())?,
            }
            .validate()
            .map_err(|_| unavailable())?,
            attempt: row.try_get("attempts").map_err(|_| unavailable())?,
            lease: row.try_get("lease_token").map_err(|_| unavailable())?,
        });
    }
    tx.commit().await.map_err(|_| unavailable())?;
    Ok(deliveries)
}

// Test invokes the exact production SQL without constructing a fake World owner.
// This can prove claim scope, never publication, World effects or queue ACK.
#[cfg(test)]
pub(crate) async fn scoped_claim_for_test(
    pool: &PgPool,
    event: Uuid,
) -> anyhow::Result<Vec<Delivery>> {
    claim_scoped(pool, Some(event)).await
}

fn requires_notification(op: &str) -> bool {
    matches!(
        op,
        "createConcertSocialRequest"
            | "decideConcertSocialRequest"
            | "cancelConcertSocialRequest"
            | "revokeConcertSocialRequest"
    )
}

// Compile-time SQL only. Never interpolate operations or values from input.
// The command operation and outbox tuple are immutable. The sealed source is
// retained even after receipt retirement and prevents receipt-only fabrication.
const SOURCE: &str = "o.kind='RESOURCE_INVALIDATED' and r.event_id=o.event_id and r.target_id=o.resource_id and r.state in ('SUCCEEDED','RETIRED') and (r.state='RETIRED' or (r.resource_id=o.resource_id and r.resource_revision=o.resource_revision and r.result_status=case when r.operation_id='createConcertSocialRequest' then 201 else 200 end)) and exists(select 1 from public.concert_social_policy_signals s where s.source_outbox=o.outbox_id and s.source_receipt=r.receipt_id and s.source_operation=r.operation_id and s.event_id=o.event_id and s.source_resource=o.resource_id and s.source_revision=o.resource_revision)";

fn source_predicate(event_bind: &'static str) -> String {
    // The caller passes a literal SQL parameter marker, never command input.
    format!("{SOURCE} and (r.operation_id in ('setConcertHandle','removeConcertHandle','setConcertIntro','publishConcertIntro','closeConcertIntro','createConcertSocialRequest','decideConcertSocialRequest','cancelConcertSocialRequest','revokeConcertSocialRequest') or (r.operation_id in ('blockConcertUser','unblockConcertUser') and o.event_id={event_bind}))")
}

async fn bound(c: &mut PgConnection) -> anyhow::Result<()> {
    for statement in [
        "set local statement_timeout='2s'",
        "set local lock_timeout='500ms'",
        "set local idle_in_transaction_session_timeout='3s'",
    ] {
        sqlx::query(statement)
            .execute(&mut *c)
            .await
            .map_err(|_| unavailable())?;
    }
    Ok(())
}
fn unavailable() -> anyhow::Error {
    anyhow::anyhow!("Social delivery is unavailable")
}
