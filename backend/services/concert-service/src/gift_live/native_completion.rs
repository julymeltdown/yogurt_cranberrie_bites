//! Actual BE15 completion hook over PgGifts. Synthetic local ACL deliberately
//! does not stand in for canonical owner acquisition or HTTP acceptance.
use super::*;
use concert_application::commands::{ReceiptState, SafeReceipt, Scope};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Mutex,
};
#[derive(Clone)]
struct CompletionPolicy {
    base: SyntheticPolicy,
    fail: Arc<AtomicBool>,
    receipts: Arc<Mutex<Vec<SafeReceipt>>>,
}
#[async_trait]
impl CommunityAuthority for CompletionPolicy {
    async fn actor_in(
        &self,
        c: &mut PgConnection,
        a: Uuid,
        e: Uuid,
        x: CommunityAccess,
    ) -> Result<(), CommandError> {
        self.base.actor_in(c, a, e, x).await
    }
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        a: Uuid,
        e: Uuid,
        s: Uuid,
        x: CommunityAccess,
    ) -> Result<(), CommandError> {
        self.base.authorize_in(c, a, e, s, x).await
    }
    async fn place_in(&self, c: &mut PgConnection, e: Uuid, p: Uuid) -> Result<(), CommandError> {
        self.base.place_in(c, e, p).await
    }
}
#[async_trait]
impl GiftAuthority for CompletionPolicy {
    async fn settings_in(
        &self,
        c: &mut PgConnection,
        e: Uuid,
    ) -> Result<GiftSettings, CommandError> {
        self.base.settings_in(c, e).await
    }
    async fn media_in(
        &self,
        c: &mut PgConnection,
        e: Uuid,
        a: Uuid,
        g: Uuid,
        m: Option<Uuid>,
    ) -> Result<(), CommandError> {
        self.base.media_in(c, e, a, g, m).await
    }
    async fn meeting_in(
        &self,
        c: &mut PgConnection,
        e: Uuid,
        m: &PickupMeeting,
    ) -> Result<(), CommandError> {
        self.base.meeting_in(c, e, m).await
    }
    async fn completed_in(
        &self,
        _: &mut PgConnection,
        s: Scope,
        t: Uuid,
        r: &SafeReceipt,
    ) -> Result<(), CommandError> {
        assert_eq!(r.scope, s);
        if let Some(reference) = r.reference {
            assert_eq!(reference.resource, t);
        }
        self.receipts.lock().unwrap().push(r.clone());
        if self.fail.load(Ordering::SeqCst) {
            Err(CommandError::Unavailable)
        } else {
            Ok(())
        }
    }
}
#[tokio::test]
#[ignore = "requires owned Gift PostgreSQL and allocated copied library binary"]
async fn completion_hook_covers_success_failure_replay_and_atomic_rollback() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let gift = f.gift(Utc::now() - Duration::minutes(1)).await?;
    let policy = CompletionPolicy {
        base: f.authority.clone(),
        fail: Arc::new(AtomicBool::new(false)),
        receipts: Arc::new(Mutex::new(vec![])),
    };
    let service = PgGifts::new(f.pool.clone(), policy.clone());
    let command = f.command(
        f.recipients[0],
        gift,
        GiftAction::Reserve {
            giveaway_revision: 3,
        },
    );
    let first = service
        .execute(&command, &f.keys, &f.codes, None)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let outbox: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(f.event)
        .fetch_one(&f.pool)
        .await?;
    let replay = service
        .execute(&command, &f.keys, &f.codes, None)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    assert_eq!(first.receipt, replay.receipt);
    let after: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(f.event)
        .fetch_one(&f.pool)
        .await?;
    assert_eq!(outbox, after);
    let failed = f.command(
        f.recipients[1],
        gift,
        GiftAction::Reserve {
            giveaway_revision: 3,
        },
    );
    assert!(service
        .execute(&failed, &f.keys, &f.codes, None)
        .await
        .is_err());
    {
        let receipts = policy.receipts.lock().unwrap();
        assert_eq!(receipts.len(), 3);
        assert_eq!(receipts[0], receipts[1]);
        assert_eq!(receipts[2].state, ReceiptState::Failed);
    }
    let retry = f.command(
        f.recipients[1],
        gift,
        GiftAction::Reserve {
            giveaway_revision: 4,
        },
    );
    policy.fail.store(true, Ordering::SeqCst);
    assert!(service
        .execute(&retry, &f.keys, &f.codes, None)
        .await
        .is_err());
    let count:i64=sqlx::query_scalar("select count(*) from concert_command_receipts where actor_id=$1 and event_id=$2 and idempotency_key=$3")
        .bind(retry.actor).bind(retry.event).bind(retry.key).fetch_one(&f.pool).await?;
    assert_eq!(
        count, 0,
        "failed completion must roll back receipt with aggregate and outbox"
    );
    let reserved:i32=sqlx::query_scalar("select reserved_unclaimed from concert_community_giveaways where event_id=$1 and giveaway_id=$2")
        .bind(f.event).bind(gift).fetch_one(&f.pool).await?;
    assert_eq!(reserved, 1);
    let after: i64 = sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
        .bind(f.event)
        .fetch_one(&f.pool)
        .await?;
    assert_eq!(after, outbox);
    policy.fail.store(false, Ordering::SeqCst);
    service
        .execute(&retry, &f.keys, &f.codes, None)
        .await
        .map_err(|e| anyhow::anyhow!("{e:?}"))?;
    let reserved:i32=sqlx::query_scalar("select reserved_unclaimed from concert_community_giveaways where event_id=$1 and giveaway_id=$2")
        .bind(f.event).bind(gift).fetch_one(&f.pool).await?;
    assert_eq!(reserved, 2);
    f.pool.close().await;
    Ok(())
}
