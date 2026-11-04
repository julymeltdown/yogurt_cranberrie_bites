//! Real PostgreSQL witness guards/resolution with explicitly SYNTHETIC ledger
//! correlation. These tests do not acquire canonical owners or certify HTTP.
use super::*;
use crate::adapters::out::relationship_authority::guarded::proto::TransactionStatus;
use sqlx::{Postgres, Transaction};
#[path = "../../tests/gift_live_support/owned.rs"]
mod owned;

struct Case {
    pool: PgPool,
    binding: OwnerBinding,
    native: ProducerTransaction,
    scope: Uuid,
}
async fn case() -> anyhow::Result<(Case, Transaction<'static, Postgres>)> {
    let pool = owned::owned_pool().await?;
    let mut binding = super::tests::binding();
    binding.actor_id = Uuid::new_v4();
    binding.event_id = Uuid::new_v4();
    binding.request_id = Uuid::new_v4();
    binding.http_target = format!(
        "/api/concert/events/{}/reservations/{}/claim",
        binding.event_id,
        Uuid::from_u128(5)
    );
    let reference = serde_json::to_vec(&super::tests::reference(&binding))?;
    let effect = effect_digest(&binding, &reference)?;
    let instance = Uuid::new_v4();
    let scope = Uuid::new_v4();
    let mut tx = pool.begin().await?;
    let actual = NativeIdentity::capture_concert_in(&mut tx, effect, instance, reference).await?;
    let native = ProducerTransaction {
        producer: "CONCERT".into(),
        system_identifier: actual.system_identifier,
        database_oid: actual.database_oid,
        full_xid: actual.full_xid,
        backend_pid: actual.backend_pid,
        backend_started_unix_micros: actual.backend_started.timestamp_micros(),
        effect_binding_digest: effect,
        producer_instance: instance,
        settlement_reference: actual.settlement_reference,
    };
    // Direct registration rows below are local SQL test metadata, NOT actual
    // Auth/Friend acknowledgments. Production uses OwnerGuardScope::track.
    sqlx::query("insert into concert_product_owner_scopes(instance_id,scope_id,binding_digest,binding,delivery_secret_hash,serialized_at) values($1,$2,$3,$4,$5,clock_timestamp())")
        .bind(instance).bind(scope).bind(binding.digest()?.as_slice()).bind(sqlx::types::Json(&binding)).bind([17u8;32].as_slice())
        .execute(&pool).await?;
    sqlx::query("insert into concert_product_owner_transactions(instance_id,scope_id,binding_digest,identity_digest,identity,auth_registration_id,friend_registration_id,confirmed_at) values($1,$2,$3,$4,$5,$6,$7,clock_timestamp())")
        .bind(instance).bind(scope).bind(binding.digest()?.as_slice()).bind(native.digest()?.as_slice()).bind(sqlx::types::Json(&native))
        .bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&pool).await?;
    Ok((
        Case {
            pool,
            binding,
            native,
            scope,
        },
        tx,
    ))
}
async fn witness(c: &Case, tx: &mut Transaction<'_, Postgres>) -> anyhow::Result<()> {
    sqlx::query("insert into concert_gift_native_witnesses(producer_instance,scope_id,identity_digest,binding_digest,native_identity) values($1,$2,$3,$4,$5)")
        .bind(c.native.producer_instance).bind(c.scope).bind(c.native.digest()?.as_slice()).bind(c.binding.digest()?.as_slice())
        .bind(sqlx::types::Json(&c.native)).execute(&mut **tx).await?;
    Ok(())
}
async fn finished(c: &Case) -> anyhow::Result<()> {
    sqlx::query("update concert_product_owner_scopes set phase='SEALED',sealed_at=clock_timestamp() where instance_id=$1 and scope_id=$2")
        .bind(c.native.producer_instance).bind(c.scope).execute(&c.pool).await?;
    sqlx::query("update concert_product_owner_scopes set phase='ROOT_FINISHED',root_finished_at=clock_timestamp() where instance_id=$1 and scope_id=$2")
        .bind(c.native.producer_instance).bind(c.scope).execute(&c.pool).await?;
    Ok(())
}
async fn resolved(c: &Case) -> anyhow::Result<NativeTransactionResolution> {
    Ok(
        GiftNativeResolver::new(c.pool.clone(), c.native.producer_instance)?
            .resolve(&c.binding, &c.native)
            .await?,
    )
}
#[tokio::test]
#[ignore = "requires owned Gift PostgreSQL and allocated copied library binary"]
async fn exact_commit_requires_witness_and_finished_root_and_rejects_forgeries(
) -> anyhow::Result<()> {
    let (c, mut tx) = case().await?;
    witness(&c, &mut tx).await?;
    assert_eq!(resolved(&c).await?.status, TransactionStatus::InProgress);
    tx.commit().await?;
    assert_eq!(resolved(&c).await?.status, TransactionStatus::Unknown);
    finished(&c).await?;
    let result = resolved(&c).await?;
    assert_eq!(result.status, TransactionStatus::Committed);
    assert_eq!(result.receipt_evidence.len(), 32);
    let resolver = GiftNativeResolver::new(c.pool.clone(), c.native.producer_instance)?;
    let mut other = c.native.clone();
    other.database_oid += 1;
    assert_eq!(
        resolver.resolve(&c.binding, &other).await?.status,
        TransactionStatus::Unknown
    );
    other = c.native.clone();
    other.backend_pid += 1;
    assert_eq!(
        resolver.resolve(&c.binding, &other).await?.status,
        TransactionStatus::Unknown
    );
    let mut binding = c.binding.clone();
    binding.body_digest = [83; 32];
    assert_eq!(
        resolver.resolve(&binding, &c.native).await?.status,
        TransactionStatus::Unknown
    );
    let forged = GiftNativeResolver::new(c.pool.clone(), Uuid::new_v4())?;
    assert_eq!(
        forged.resolve(&c.binding, &c.native).await?.status,
        TransactionStatus::Unknown
    );
    assert!(
        sqlx::query("delete from concert_gift_native_witnesses where producer_instance=$1")
            .bind(c.native.producer_instance)
            .execute(&c.pool)
            .await
            .is_err()
    );
    c.pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires owned Gift PostgreSQL and allocated copied library binary"]
async fn committed_without_witness_stays_unknown_and_rollback_is_not_commit() -> anyhow::Result<()>
{
    let (c, tx) = case().await?;
    tx.commit().await?;
    finished(&c).await?;
    assert_eq!(resolved(&c).await?.status, TransactionStatus::Unknown);
    c.pool.close().await;
    let (c, mut tx) = case().await?;
    witness(&c, &mut tx).await?;
    tx.rollback().await?;
    finished(&c).await?;
    assert_eq!(resolved(&c).await?.status, TransactionStatus::Aborted);
    let count: i64 = sqlx::query_scalar(
        "select count(*) from concert_gift_native_witnesses where producer_instance=$1",
    )
    .bind(c.native.producer_instance)
    .fetch_one(&c.pool)
    .await?;
    assert_eq!(count, 0);
    c.pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires owned Gift PostgreSQL and allocated copied library binary"]
async fn foreign_transaction_cannot_insert_witness_or_mutate_immutable_evidence(
) -> anyhow::Result<()> {
    let (c, tx) = case().await?;
    tx.commit().await?;
    let mut other = c.pool.begin().await?;
    assert!(witness(&c, &mut other).await.is_err());
    other.rollback().await?;
    let (c, mut tx) = case().await?;
    witness(&c, &mut tx).await?;
    tx.commit().await?;
    assert!(sqlx::query(
        "update concert_gift_native_witnesses set binding_digest=$2 where producer_instance=$1"
    )
    .bind(c.native.producer_instance)
    .bind([44u8; 32].as_slice())
    .execute(&c.pool)
    .await
    .is_err());
    c.pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires two owned processes separated by actual PostgreSQL restart"]
async fn restart_preserves_exact_committed_snapshot_witness() -> anyhow::Result<()> {
    let pool = owned::owned_pool().await?;
    sqlx::query("create table if not exists gift_native_restart_fixture(binding jsonb not null,native jsonb not null,scope uuid not null)").execute(&pool).await?;
    use sqlx::Row;
    if let Some(row) = sqlx::query("select binding,native,scope from gift_native_restart_fixture")
        .fetch_optional(&pool)
        .await?
    {
        let c = Case {
            binding: serde_json::from_value(row.get("binding"))?,
            native: serde_json::from_value(row.get("native"))?,
            scope: row.get("scope"),
            pool,
        };
        assert_eq!(resolved(&c).await?.status, TransactionStatus::Committed);
        let count: i64 = sqlx::query_scalar(
            "select count(*) from concert_gift_native_witnesses where producer_instance=$1",
        )
        .bind(c.native.producer_instance)
        .fetch_one(&c.pool)
        .await?;
        assert_eq!(count, 1);
        c.pool.close().await;
    } else {
        let (c, mut tx) = case().await?;
        witness(&c, &mut tx).await?;
        tx.commit().await?;
        finished(&c).await?;
        sqlx::query("insert into gift_native_restart_fixture values($1,$2,$3)")
            .bind(sqlx::types::Json(&c.binding))
            .bind(sqlx::types::Json(&c.native))
            .bind(c.scope)
            .execute(&pool)
            .await?;
        c.pool.close().await;
        pool.close().await;
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires owned Gift PostgreSQL and allocated copied library binary"]
async fn receipt_snapshot_is_same_transaction_parent_bound_and_survives_retirement(
) -> anyhow::Result<()> {
    let (c, mut tx) = case().await?;
    witness(&c, &mut tx).await?;
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Native witness metadata only','OPEN')")
        .bind(c.binding.event_id).bind(format!("gift-witness-{}",c.binding.event_id)).execute(&mut *tx).await?;
    let receipt = Uuid::new_v4();
    sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state,failure_reason,created_at,expires_at) select $1,$2,$3,$4,$5,$6,$7,'FAILED','VERSION_CONFLICT',t-interval '2 days',t-interval '1 day' from (select clock_timestamp() t) q")
        .bind(receipt).bind(c.binding.actor_id).bind(c.binding.event_id).bind(&c.binding.operation).bind(Uuid::from_u128(6))
        .bind(format!("hmac-sha256:v1:fixture:{}","00".repeat(32))).bind(Uuid::from_u128(5)).execute(&mut *tx).await?;
    let snapshot: serde_json::Value = sqlx::query_scalar(
        "select to_jsonb(r)-'request_hash' from concert_command_receipts r where receipt_id=$1",
    )
    .bind(receipt)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("insert into concert_gift_receipt_witnesses(producer_instance,scope_id,identity_digest,receipt_id,receipt_snapshot) values($1,$2,$3,$4,$5)")
        .bind(c.native.producer_instance).bind(c.scope).bind(c.native.digest()?.as_slice()).bind(receipt).bind(&snapshot)
        .execute(&mut *tx).await?;
    tx.commit().await?;
    finished(&c).await?;
    let before = resolved(&c).await?;
    assert_eq!(before.status, TransactionStatus::Committed);
    sqlx::query("update concert_command_receipts set state='RETIRED',resource_id=null,resource_revision=null,result_status=null,failure_reason=null where receipt_id=$1")
        .bind(receipt).execute(&c.pool).await?;
    let retained: serde_json::Value = sqlx::query_scalar(
        "select receipt_snapshot from concert_gift_receipt_witnesses where receipt_id=$1",
    )
    .bind(receipt)
    .fetch_one(&c.pool)
    .await?;
    assert_eq!(retained, snapshot);
    assert_eq!(
        resolved(&c).await?.receipt_evidence,
        before.receipt_evidence
    );
    assert!(sqlx::query("update concert_gift_receipt_witnesses set receipt_snapshot='{}'::jsonb where receipt_id=$1")
        .bind(receipt).execute(&c.pool).await.is_err());
    // A new transaction cannot replay an old native identity into a new receipt
    // witness, even if it points to an otherwise genuine receipt ID.
    let (other, mut tx) = case().await?;
    witness(&other, &mut tx).await?;
    assert!(sqlx::query("insert into concert_gift_receipt_witnesses(producer_instance,scope_id,identity_digest,receipt_id,receipt_snapshot) values($1,$2,$3,$4,$5)")
        .bind(other.native.producer_instance).bind(other.scope).bind(other.native.digest()?.as_slice()).bind(receipt).bind(snapshot)
        .execute(&mut *tx).await.is_err());
    tx.rollback().await?;
    other.pool.close().await;
    c.pool.close().await;
    Ok(())
}
