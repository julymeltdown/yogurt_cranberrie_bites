//! BE15 -> typed sink -> real Notification-owned PostgreSQL. Owner auth is fixture-only.
#[path = "../../notification-service/tests/concert_inbox_support/mod.rs"]
mod support;
use async_trait::async_trait;
use concert_application::{
    commands::{maintenance::InvalidationKind, SafeReference},
    notification::*,
};
use concert_domain::notification::*;
use concert_service::{
    adapters::out::{notification::*, repository::outbox::*},
    workers::outbox::{run_batch, IdempotentInvalidationSink},
};
use notification_service::concert::{PgConcertInbox, UnavailableAuthority};
use sqlx::PgPool;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use uuid::Uuid;

struct Routing(Recipient);
#[async_trait]
impl source::ProjectionRouting for Routing {
    async fn route_in(
        &self,
        _: &mut sqlx::PgConnection,
        source: &source::Source,
        _: chrono::DateTime<chrono::Utc>,
    ) -> Result<Vec<Recipient>> {
        if source.operation != "updateConcertNotice"
            || Some(source.actor_id) != self.0.source_actor_id
            || source.envelope.resource_id != self.0.target.unwrap().target_id
        {
            return Err(NotificationError::PermissionDenied);
        }
        Ok(vec![self.0])
    }
}
struct Owner {
    inbox: PgConcertInbox<UnavailableAuthority>,
    lose_ack: AtomicBool,
}
#[async_trait]
impl DurableNotificationOwner for Owner {
    async fn deliver(&self, e: Envelope, r: Vec<Recipient>) -> Result<()> {
        self.inbox.ingest(e, r).await?;
        if self.lose_ack.swap(false, Ordering::SeqCst) {
            Err(NotificationError::Unavailable)
        } else {
            Ok(())
        }
    }
}
async fn seed() -> anyhow::Result<(PgPool, PgPool, Envelope, Recipient)> {
    let concert = support::owned_pool("mcr_concert_notification_source").await?;
    let notification = support::owned_pool("mcr_concert_notification").await?;
    let e = Envelope {
        outbox_id: Uuid::new_v4(),
        receipt_id: Uuid::new_v4(),
        event_id: Uuid::new_v4(),
        resource_id: Uuid::new_v4(),
        resource_revision: 1,
    };
    let r = Recipient {
        user_id: Uuid::new_v4(),
        source_actor_id: Some(Uuid::new_v4()),
        kind: Kind::Notice,
        target: Some(Target {
            target_type: TargetType::Notice,
            target_id: e.resource_id,
        }),
    };
    sqlx::query(
        "insert into concert_events(event_id,slug,title,phase) values($1,$2,'BE33 fixture','LIVE')",
    )
    .bind(e.event_id)
    .bind(e.event_id.to_string())
    .execute(&concert)
    .await?;
    sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state,resource_id,resource_revision,result_status) values($1,$2,$3,'updateConcertNotice',$4,$5,$6,'SUCCEEDED',$6,1,200)")
        .bind(e.receipt_id).bind(r.source_actor_id).bind(e.event_id).bind(Uuid::new_v4()).bind(format!("hmac-sha256:v1:fixture:{}","00".repeat(32))).bind(e.resource_id).execute(&concert).await?;
    sqlx::query("insert into concert_outbox(outbox_id,receipt_id,event_id,resource_id,resource_revision) values($1,$2,$3,$4,1)").bind(e.outbox_id).bind(e.receipt_id).bind(e.event_id).bind(e.resource_id).execute(&concert).await?;
    Ok((concert, notification, e, r))
}
#[tokio::test]
async fn lost_ack_redelivery_deduplicates_before_outbox_ack() -> anyhow::Result<()> {
    let (c, n, e, r) = seed().await?;
    let outbox = PgOutbox::new(c.clone());
    let sink = Arc::new(NotificationSink {
        projection: source::PgProjectionOwner::new(c.clone(), Routing(r)),
        owner: Owner {
            inbox: PgConcertInbox::new(n.clone(), UnavailableAuthority),
            lose_ack: AtomicBool::new(true),
        },
    });
    let first = run_batch(&outbox, sink.clone(), 1).await?;
    assert_eq!((first.failed, first.acknowledged), (1, 0));
    sqlx::query("update concert_outbox set available_at=clock_timestamp() where outbox_id=$1")
        .bind(e.outbox_id)
        .execute(&c)
        .await?;
    let second = run_batch(&outbox, sink, 1).await?;
    assert_eq!(second.acknowledged, 1);
    let count: i64 = sqlx::query_scalar("select count(*) from notifications where event_id=$1")
        .bind(e.outbox_id)
        .fetch_one(&n)
        .await?;
    assert_eq!(count, 1);
    let state: String = sqlx::query_scalar("select state from concert_outbox where outbox_id=$1")
        .bind(e.outbox_id)
        .fetch_one(&c)
        .await?;
    assert_eq!(state, "DELIVERED");
    Ok(())
}
#[tokio::test]
async fn expired_lease_fences_old_ack_after_durable_owner_commit() -> anyhow::Result<()> {
    let (c, n, e, r) = seed().await?;
    let outbox = PgOutbox::new(c.clone());
    let old = outbox.claim(1).await?.remove(0);
    PgConcertInbox::new(n.clone(), UnavailableAuthority)
        .ingest(e, vec![r])
        .await?;
    sqlx::query("update concert_outbox set lease_until=clock_timestamp()-interval '1 second' where outbox_id=$1").bind(e.outbox_id).execute(&c).await?;
    assert!(!outbox.acknowledge(&old).await?);
    let sink = Arc::new(NotificationSink {
        projection: source::PgProjectionOwner::new(c.clone(), Routing(r)),
        owner: Owner {
            inbox: PgConcertInbox::new(n.clone(), UnavailableAuthority),
            lose_ack: AtomicBool::new(false),
        },
    });
    assert_eq!(run_batch(&outbox, sink, 1).await?.acknowledged, 1);
    let count: i64 = sqlx::query_scalar("select count(*) from notifications where event_id=$1")
        .bind(e.outbox_id)
        .fetch_one(&n)
        .await?;
    assert_eq!(count, 1);
    Ok(())
}
#[tokio::test]
async fn missing_owner_never_acknowledges_and_invalid_envelope_rejects() -> anyhow::Result<()> {
    let (c, _n, e, r) = seed().await?;
    let sink = Arc::new(NotificationSink {
        projection: source::PgProjectionOwner::new(c.clone(), Routing(r)),
        owner: UnavailableNotificationOwner,
    });
    let result = run_batch(&PgOutbox::new(c.clone()), sink.clone(), 1).await?;
    assert_eq!((result.failed, result.acknowledged), (1, 0));
    let bad = Delivery {
        id: Uuid::nil(),
        receipt: Some(e.receipt_id),
        kind: InvalidationKind::ResourceInvalidated,
        event: e.event_id,
        reference: SafeReference {
            resource: e.resource_id,
            revision: 1,
        },
        attempt: 1,
        lease: Uuid::new_v4(),
    };
    assert_eq!(sink.deliver(&bad).await, Err(DeliveryFailure::Rejected));
    // Make this test's retry terminal so another test cannot claim it.
    sqlx::query(
        "update concert_outbox set state='DEAD',terminal_at=clock_timestamp() where outbox_id=$1",
    )
    .bind(e.outbox_id)
    .execute(&c)
    .await?;
    Ok(())
}

#[tokio::test]
async fn source_receipt_verifies_full_envelope_and_unavailable_routing() -> anyhow::Result<()> {
    let (c, _n, e, r) = seed().await?;
    let p = source::PgProjectionOwner::new(c.clone(), Routing(r));
    assert_eq!(p.resolve(e).await?, vec![r]);
    assert_eq!(
        p.resolve(Envelope {
            event_id: Uuid::new_v4(),
            ..e
        })
        .await,
        Err(NotificationError::IdempotencyConflict)
    );
    assert_eq!(
        p.resolve(Envelope {
            receipt_id: Uuid::new_v4(),
            ..e
        })
        .await,
        Err(NotificationError::IdempotencyConflict)
    );
    assert_eq!(
        source::PgProjectionOwner::new(c.clone(), source::UnavailableRouting)
            .resolve(e)
            .await,
        Err(NotificationError::Unavailable)
    );
    sqlx::query(
        "update concert_outbox set state='DEAD',terminal_at=clock_timestamp() where outbox_id=$1",
    )
    .bind(e.outbox_id)
    .execute(&c)
    .await?;
    Ok(())
}

#[tokio::test]
async fn maintenance_without_command_receipt_is_not_acknowledged() -> anyhow::Result<()> {
    let (c, n, e, _) = seed().await?;
    sqlx::query(
        "update concert_outbox set state='DEAD',terminal_at=clock_timestamp() where outbox_id=$1",
    )
    .bind(e.outbox_id)
    .execute(&c)
    .await?;
    let maintenance = Uuid::new_v4();
    sqlx::query("insert into concert_maintenance_invalidations(maintenance_id,event_id,resource_id,resource_revision) values($1,$2,$3,1)")
        .bind(maintenance).bind(e.event_id).bind(e.resource_id).execute(&c).await?;
    sqlx::query("insert into concert_outbox(outbox_id,maintenance_id,event_id,resource_id,resource_revision,kind) values($1,$1,$2,$3,1,'MAINTENANCE_INVALIDATED')")
        .bind(maintenance).bind(e.event_id).bind(e.resource_id).execute(&c).await?;
    let sink = Arc::new(NotificationSink {
        projection: UnavailableProjectionOwner,
        owner: UnavailableNotificationOwner,
    });
    let result = run_batch(&PgOutbox::new(c.clone()), sink.clone(), 1).await?;
    assert_eq!((result.failed, result.acknowledged), (1, 0));
    let state: (String, Option<Uuid>, String) =
        sqlx::query_as("select state,receipt_id,last_error from concert_outbox where outbox_id=$1")
            .bind(maintenance)
            .fetch_one(&c)
            .await?;
    assert_eq!(state, ("READY".into(), None, "RETRYABLE".into()));
    let count: i64 = sqlx::query_scalar(
        "select count(*) from concert_notification_deliveries where outbox_id=$1",
    )
    .bind(maintenance)
    .fetch_one(&n)
    .await?;
    assert_eq!(count, 0);
    let malformed = Delivery {
        id: maintenance,
        receipt: None,
        kind: InvalidationKind::ResourceInvalidated,
        event: e.event_id,
        reference: SafeReference {
            resource: e.resource_id,
            revision: 1,
        },
        attempt: 1,
        lease: Uuid::new_v4(),
    };
    assert_eq!(
        sink.deliver(&malformed).await,
        Err(DeliveryFailure::Rejected)
    );
    let malformed = Delivery {
        receipt: Some(e.receipt_id),
        kind: InvalidationKind::MaintenanceInvalidated,
        ..malformed
    };
    assert_eq!(
        sink.deliver(&malformed).await,
        Err(DeliveryFailure::Rejected)
    );
    sqlx::query(
        "update concert_outbox set state='DEAD',terminal_at=clock_timestamp() where outbox_id=$1",
    )
    .bind(maintenance)
    .execute(&c)
    .await?;
    Ok(())
}
