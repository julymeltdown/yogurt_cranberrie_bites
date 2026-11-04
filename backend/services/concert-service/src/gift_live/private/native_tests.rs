//! Native SQL exclusion/recovery ONLY. Synthetic correlation metadata below is
//! not an Auth/Friend grant, OwnerGuardScope, or durable Root intent fixture.
use super::native;
use crate::{
    adapters::out::relationship_authority::guarded::{
        ChildNativeIdentity, ChildQuery, ChildRecoveryReference, GiftPrivateKind, HeldChildGuard,
        ProducerTransaction,
    },
    gift_live::queries::native_tests::Fixture,
};
use chrono::Utc;
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use sqlx::{PgPool, Postgres, Transaction};
use std::{io::Write, os::unix::fs::OpenOptionsExt, time::Duration};
use uuid::Uuid;

async fn capture(
    tx: &mut Transaction<'_, Postgres>,
    f: &Fixture,
    gift: Uuid,
    until: i64,
) -> anyhow::Result<ChildRecoveryReference> {
    let identity =
        ProducerTransaction::capture_concert_in(tx, [73; 32], Uuid::new_v4(), vec![]).await?;
    // Real native identity captured before any local target lock. Only this test
    // directly constructs correlation metadata; production requires actual scope.
    sqlx::query("select giveaway_id from concert_community_giveaways where event_id=$1 and giveaway_id=$2 for share")
        .bind(f.event).bind(gift).fetch_one(&mut **tx).await?;
    native::reserve(tx).await?;
    Ok(ChildRecoveryReference::GiftPrivate {
        query: ChildQuery {
            actor: f.recipients[0],
            event: f.event,
            subject: f.owner,
            family: Uuid::new_v4(),
            request: Uuid::new_v4(),
            expires_unix_micros: until,
            binding_digest: [73; 32],
        },
        child_nonce: Uuid::new_v4(),
        native: ChildNativeIdentity {
            system_identifier: identity.system_identifier,
            database_oid: identity.database_oid,
            full_xid: identity.full_xid,
            backend_pid: identity.backend_pid,
            backend_started_unix_micros: identity.backend_started.timestamp_micros(),
            producer_instance_id: identity.producer_instance_id,
        },
        private_kind: GiftPrivateKind::Pickup,
        giveaway_id: gift,
        reservation_id: Uuid::new_v4(),
        valid_until_unix_micros: until,
    })
}
async fn held(pool: &PgPool, reference: &ChildRecoveryReference) -> anyhow::Result<bool> {
    Ok(sqlx::query_scalar("select exists(select 1 from pg_prepared_xacts where gid=$1 and database=current_database())")
        .bind(reference.local_prepared_gid()?).fetch_one(pool).await?)
}
async fn write(pool: &PgPool, event: Uuid, gift: Uuid) -> anyhow::Result<bool> {
    let mut tx = pool.begin().await?;
    sqlx::query("set local lock_timeout='60ms'")
        .execute(&mut *tx)
        .await?;
    let result=sqlx::query("update concert_community_giveaways set updated_at=clock_timestamp() where event_id=$1 and giveaway_id=$2")
        .bind(event).bind(gift).execute(&mut *tx).await;
    let allowed = match result {
        Ok(r) => r.rows_affected() == 1,
        Err(sqlx::Error::Database(e)) if e.code().as_deref() == Some("55P03") => false,
        Err(e) => return Err(e.into()),
    };
    tx.rollback().await?;
    Ok(allowed)
}
async fn prepared(
    f: &Fixture,
    gift: Uuid,
    until: i64,
) -> anyhow::Result<(native::NativeChild, ChildRecoveryReference)> {
    let mut tx = f.pool.begin().await?;
    let reference = capture(&mut tx, f, gift, until).await?;
    let deadline = AuthorityRpcDeadline::new(until, Duration::from_secs(5))?;
    // Native helper qualification only: this bypass does NOT certify Root's
    // durable-intent ordering. The public production factory cannot bypass it.
    let child =
        native::prepare_after_intent(f.pool.clone(), tx, reference.clone(), deadline).await?;
    Ok((child, reference))
}

#[tokio::test]
#[ignore = "owned private PG; native exclusion only, canonical/Root NOT_RUN"]
async fn drop_keeps_exclusion_until_exact_idempotent_abort() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let gift = f.gift(Utc::now() - chrono::Duration::minutes(1)).await?;
    let (child, reference) = prepared(&f, gift, Utc::now().timestamp_micros() + 4_000_000).await?;
    child.check().await?;
    assert!(!write(&f.pool, f.event, gift).await?);
    drop(child);
    assert!(held(&f.pool, &reference).await?);
    assert!(!write(&f.pool, f.event, gift).await?);
    native::recover_one(&f.pool, &reference).await?;
    native::recover_one(&f.pool, &reference).await?;
    assert!(!held(&f.pool, &reference).await?);
    assert!(write(&f.pool, f.event, gift).await?);
    f.pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "owned private PG; native exclusion only, canonical/Root NOT_RUN"]
async fn expiry_denies_content_without_releasing_exclusion() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let gift = f.gift(Utc::now() - chrono::Duration::minutes(1)).await?;
    let (child, reference) = prepared(&f, gift, Utc::now().timestamp_micros() + 500_000).await?;
    tokio::time::sleep(Duration::from_millis(550)).await;
    assert!(child.check().await.is_err());
    assert!(held(&f.pool, &reference).await?);
    assert!(!write(&f.pool, f.event, gift).await?);
    // Recovery deliberately accepts an expired, exact saved reference.
    child.complete_settled().await?;
    assert!(write(&f.pool, f.event, gift).await?);
    f.pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "owned private PG; native exclusion only, canonical/Root NOT_RUN"]
async fn mismatched_identity_cannot_rollback_and_commit_is_not_abort() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let gift = f.gift(Utc::now() - chrono::Duration::minutes(1)).await?;
    let (child, reference) = prepared(&f, gift, Utc::now().timestamp_micros() + 4_000_000).await?;
    for field in ["cluster", "database", "xid", "epoch"] {
        let mut wrong = reference.clone();
        if let ChildRecoveryReference::GiftPrivate { native, .. } = &mut wrong {
            match field {
                "cluster" => native.system_identifier = "1".into(),
                "database" => native.database_oid = native.database_oid.saturating_add(1),
                "epoch" => {
                    native.full_xid = (native.full_xid.parse::<u64>()? + 4_294_967_296).to_string()
                }
                _ => native.full_xid = (native.full_xid.parse::<u64>()? + 1).to_string(),
            }
        }
        assert!(native::recover_one(&f.pool, &wrong).await.is_err());
        assert!(held(&f.pool, &reference).await?);
    }
    let gid = reference.local_prepared_gid()?;
    sqlx::query(&format!("commit prepared '{gid}'"))
        .execute(&f.pool)
        .await?;
    assert!(native::recover_one(&f.pool, &reference).await.is_err());
    assert!(child.check().await.is_err());
    f.pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "owned private PG; native exclusion only, canonical/Root NOT_RUN"]
async fn missing_gid_requires_real_full_xid_abort() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let gift = f.gift(Utc::now() - chrono::Duration::minutes(1)).await?;
    let mut tx = f.pool.begin().await?;
    let reference = capture(&mut tx, &f, gift, Utc::now().timestamp_micros() + 4_000_000).await?;
    assert!(!held(&f.pool, &reference).await?);
    assert!(native::recover_one(&f.pool, &reference).await.is_err());
    tx.rollback().await?;
    native::recover_one(&f.pool, &reference).await?;
    native::recover_one(&f.pool, &reference).await?;
    f.pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "two-phase owned runner; prepared restart recovery only"]
async fn prepared_restart_recovery() -> anyhow::Result<()> {
    let phase = std::env::var("GIFT_PRIVATE_RESTART_PHASE")?;
    let f = Fixture::new().await?; // validates marker/inode/socket before file access
    let path = std::path::PathBuf::from(std::env::var("GIFT_LIVE_TEST_OWNER_ROOT")?)
        .join("gift-private-restart.json");
    match phase.as_str() {
        "prepare" => {
            let gift = f.gift(Utc::now() - chrono::Duration::minutes(1)).await?;
            let (child, reference) =
                prepared(&f, gift, Utc::now().timestamp_micros() + 4_000_000).await?;
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)?;
            file.write_all(&serde_json::to_vec(&reference)?)?;
            file.sync_all()?;
            drop(child);
            assert!(held(&f.pool, &reference).await?);
        }
        "recover" => {
            let info = std::fs::symlink_metadata(&path)?;
            anyhow::ensure!(
                info.is_file() && !info.file_type().is_symlink(),
                "invalid restart reference file"
            );
            let reference: ChildRecoveryReference = serde_json::from_slice(&std::fs::read(&path)?)?;
            assert!(held(&f.pool, &reference).await?);
            native::recover_one(&f.pool, &reference).await?;
            native::recover_one(&f.pool, &reference).await?;
            assert!(!held(&f.pool, &reference).await?);
            std::fs::remove_file(path)?;
        }
        _ => anyhow::bail!("invalid native restart phase"),
    }
    f.pool.close().await;
    Ok(())
}
