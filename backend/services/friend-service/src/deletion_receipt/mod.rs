//! Owner-local deletion transaction, extracted from the canonical repository.
//! No second transaction, queued effect or caller-provided receipt can certify cleanup.
use crate::application::dto::friend_dto::CommittedDeletionReceiptDto as Receipt;
use anyhow::Result;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

pub async fn apply(
    pool: &PgPool,
    saga_id: Uuid,
    user_id: Uuid,
    actor_id: Uuid,
    retention_until: DateTime<Utc>,
) -> Result<crate::application::dto::friend_dto::CommittedDeletionReceiptDto> {
    anyhow::ensure!(
        !saga_id.is_nil() && !user_id.is_nil() && !actor_id.is_nil(),
        "Deletion identity is required"
    );
    anyhow::ensure!(
        retention_until.timestamp_subsec_nanos() < 1_000_000_000,
        "Invalid retention deadline"
    );
    let work = apply_transaction(pool, saga_id, user_id, actor_id, retention_until);
    if let Some(deadline) = platform::call_deadline::current() {
        return tokio::time::timeout_at(tokio::time::Instant::from_std(deadline), work)
            .await
            .map_err(|_| anyhow::anyhow!("Deletion deadline exceeded"))?;
    }
    work.await
}

async fn apply_transaction(
    pool: &PgPool,
    saga_id: Uuid,
    user_id: Uuid,
    actor_id: Uuid,
    retention_until: DateTime<Utc>,
) -> Result<crate::application::dto::friend_dto::CommittedDeletionReceiptDto> {
    let mut tx = pool.begin().await?;
    // Serialize deletion with every active Friend write involving this
    // identity. The database trigger takes the same advisory lock for
    // relationship and recommendation writes, so a late event cannot pass
    // a pre-delete check and commit after the tombstone.
    sqlx::query("select friend_lock_subject_ids($1::uuid[])")
        .bind(vec![user_id])
        .execute(&mut *tx)
        .await?;
    if let Some(receipt) = load(&mut tx, saga_id, user_id, actor_id, retention_until).await? {
        let result = receipt;
        tx.commit().await?;
        return Ok(result);
    }
    // Never upgrade an unproved historical tombstone/audit into COMMIT evidence.
    let occupied: bool = sqlx::query_scalar("select exists(select 1 from friend_user_deletion_tombstones where user_id=$1 or saga_id=$2) or exists(select 1 from friend_audit_events where id=$2)")
            .bind(user_id).bind(saga_id).fetch_one(&mut *tx).await?;
    anyhow::ensure!(
        !occupied,
        "Historical deletion evidence requires reconciliation"
    );
    sqlx::query(
        r#"
            insert into friend_user_deletion_tombstones (
                user_id, saga_id, actor_id, retention_until, applied_at
            ) values ($1, $2, $3, $4, now())
            on conflict (user_id) do nothing
            "#,
    )
    .bind(user_id)
    .bind(saga_id)
    .bind(actor_id)
    .bind(retention_until)
    .execute(&mut *tx)
    .await?;
    let receipt_matches: bool = sqlx::query_scalar(
        r#"
            select saga_id = $2 and actor_id = $3 and retention_until = $4
            from friend_user_deletion_tombstones
            where user_id = $1
            "#,
    )
    .bind(user_id)
    .bind(saga_id)
    .bind(actor_id)
    .bind(retention_until)
    .fetch_one(&mut *tx)
    .await?;
    anyhow::ensure!(
        receipt_matches,
        "Friend deletion identity was reused with different saga data"
    );

    sqlx::query(
        r#"
            update follows
            set deleted_at=coalesce(deleted_at, now()),
                deleted_by=$2,
                delete_reason='account_deletion',
                retention_until=$3
            where follower_id=$1 or followee_id=$1
            "#,
    )
    .bind(user_id)
    .bind(actor_id)
    .bind(retention_until)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
            r#"
            update follow_requests
            set status=case when status='PENDING' then 'CANCELLED' else status end,
                decided_at=case when status='PENDING' then coalesce(decided_at, now()) else decided_at end,
                deleted_at=coalesce(deleted_at, now()),
                deleted_by=$2,
                delete_reason='account_deletion',
                retention_until=$3
            where follower_id=$1 or followee_id=$1
            "#,
        )
        .bind(user_id)
        .bind(actor_id)
        .bind(retention_until)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        r#"
            update user_blocks
            set deleted_at=coalesce(deleted_at, now()),
                deleted_by=$2,
                delete_reason='account_deletion',
                retention_until=$3
            where blocker_id=$1 or blocked_id=$1
            "#,
    )
    .bind(user_id)
    .bind(actor_id)
    .bind(retention_until)
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        r#"
            update user_mutes
            set deleted_at=coalesce(deleted_at, now()),
                deleted_by=$2,
                delete_reason='account_deletion',
                retention_until=$3
            where muter_id=$1 or muted_id=$1
            "#,
    )
    .bind(user_id)
    .bind(actor_id)
    .bind(retention_until)
    .execute(&mut *tx)
    .await?;

    for query in [
            "delete from people_recommendation_impressions where viewer_id=$1 or candidate_user_id=$1",
            "delete from people_recommendation_snapshot_items where candidate_user_id=$1",
            "delete from people_recommendation_snapshots where viewer_id=$1",
            "delete from people_recommendation_candidates where viewer_id=$1 or candidate_user_id=$1",
            "delete from people_recommendation_events where viewer_id=$1 or actor_id=$1 or subject_id=$1",
            "delete from people_recommendation_refresh_jobs where viewer_id=$1",
            "delete from people_experiment_assignments where viewer_id=$1",
            "delete from people_embeddings where subject_id=$1",
        ] {
            sqlx::query(query).bind(user_id).execute(&mut *tx).await?;
        }
    sqlx::query(
        r#"
            insert into friend_audit_events (
                id, actor_id, follower_id, followee_id, action, outcome,
                request_id, metadata_json, occurred_at
            ) values ($1,$2,$3,null,'ACCOUNT_DELETION_APPLIED','SUCCESS',$4,$5,now())
            on conflict (id) do nothing
            "#,
    )
    .bind(saga_id)
    .bind(actor_id)
    .bind(user_id)
    .bind(saga_id.to_string())
    .bind(serde_json::json!({"retentionUntil": retention_until}).to_string())
    .execute(&mut *tx)
    .await?;
    let fence_revision: i64 = sqlx::query_scalar(
        "insert into friend_deletion_fences_v1(user_id) values($1) returning fence_revision",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;
    let metadata = json!({"proofVersion":1,"sagaId":saga_id,"retentionSeconds":retention_until.timestamp(),"retentionNanos":retention_until.timestamp_subsec_nanos(),"fenceRevision":fence_revision});
    sqlx::query("update friend_audit_events set metadata_json=(metadata_json::jsonb || $2::jsonb)::text where id=$1")
            .bind(saga_id).bind(metadata).execute(&mut *tx).await?;
    sqlx::query("insert into friend_deletion_receipts_v1(saga_id,user_id,actor_id,retention_until,retention_seconds,retention_nanos,fence_revision,audit_event_id) values($1,$2,$3,$4,$5,$6,$7,$1)")
            .bind(saga_id).bind(user_id).bind(actor_id).bind(retention_until)
            .bind(retention_until.timestamp()).bind(retention_until.timestamp_subsec_nanos() as i32)
            .bind(fence_revision).execute(&mut *tx).await?;
    let receipt = load(&mut tx, saga_id, user_id, actor_id, retention_until)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Deletion proof unavailable"))?;
    tx.commit().await?;
    Ok(receipt)
}

async fn load(
    tx: &mut Transaction<'_, Postgres>,
    saga_id: Uuid,
    user_id: Uuid,
    actor_id: Uuid,
    retention_until: DateTime<Utc>,
) -> Result<Option<Receipt>> {
    let rows = sqlx::query("select r.*,r.effect_xid::text as full_xid from friend_deletion_receipts_v1 r join friend_deletion_fences_v1 f using(user_id,fence_revision) where r.saga_id=$1 or r.user_id=$2")
        .bind(saga_id).bind(user_id).fetch_all(&mut **tx).await?;
    if rows.is_empty() {
        return Ok(None);
    }
    anyhow::ensure!(rows.len() == 1, "Deletion identity rebound");
    let row = &rows[0];
    anyhow::ensure!(
        row.get::<Uuid, _>("saga_id") == saga_id
            && row.get::<Uuid, _>("user_id") == user_id
            && row.get::<Uuid, _>("actor_id") == actor_id
            && row.get::<i64, _>("retention_seconds") == retention_until.timestamp()
            && row.get::<i32, _>("retention_nanos")
                == retention_until.timestamp_subsec_nanos() as i32,
        "Deletion identity rebound"
    );
    Ok(Some(Receipt {
        version: 1,
        saga_id,
        user_id,
        actor_id,
        retention_until,
        fence_revision: row.get("fence_revision"),
        effect_xid: row.get("full_xid"),
        recorded_at: row.get("recorded_at"),
        owner: "FRIEND".to_owned(),
    }))
}
