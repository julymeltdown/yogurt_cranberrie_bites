//! Owner-local deletion transaction, extracted from the canonical repository.
//! No second transaction, queued effect or caller-provided receipt can certify cleanup.
use crate::application::dto::post_dto::CommittedDeletionReceiptDto as Receipt;
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
) -> Result<(
    crate::application::dto::post_dto::CommittedDeletionReceiptDto,
    Vec<Uuid>,
)> {
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
) -> Result<(
    crate::application::dto::post_dto::CommittedDeletionReceiptDto,
    Vec<Uuid>,
)> {
    // Deleting this actor can update other authors' post counters. Gather
    // ALL owners before taking sorted subject locks; retry discovery if a
    // producer won and introduced a new owner before our actor lock.
    let mut tx = loop {
        let mut candidate = pool.begin().await?;
        let subjects: Vec<Uuid> = sqlx::query_scalar(DELETION_SUBJECTS_SQL)
            .bind(user_id)
            .fetch_all(&mut *candidate)
            .await?;
        sqlx::query("select post_lock_subject_ids($1::uuid[])")
            .bind(&subjects)
            .execute(&mut *candidate)
            .await?;
        let current: Vec<Uuid> = sqlx::query_scalar(DELETION_SUBJECTS_SQL)
            .bind(user_id)
            .fetch_all(&mut *candidate)
            .await?;
        if current.iter().all(|id| subjects.contains(id)) {
            break candidate;
        }
        candidate.rollback().await?;
    };
    if let Some(receipt) = load(&mut tx, saga_id, user_id, actor_id, retention_until).await? {
        let result = (
            receipt,
            sqlx::query_scalar("select id from posts where author_id=$1 order by id")
                .bind(user_id)
                .fetch_all(&mut *tx)
                .await?,
        );
        tx.commit().await?;
        return Ok(result);
    }
    // Never upgrade an unproved historical tombstone/audit into COMMIT evidence.
    let occupied: bool = sqlx::query_scalar("select exists(select 1 from post_user_deletion_tombstones where user_id=$1 or saga_id=$2) or exists(select 1 from post_audit_events where id=$2)")
            .bind(user_id).bind(saga_id).fetch_one(&mut *tx).await?;
    anyhow::ensure!(
        !occupied,
        "Historical deletion evidence requires reconciliation"
    );
    sqlx::query(
        r#"
            insert into post_user_deletion_tombstones (
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
            from post_user_deletion_tombstones
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
        "Post deletion identity was reused with different saga data"
    );

    sqlx::query("update comments set content='' where author_id=$1 and deleted_at is not null")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let post_ids =
        sqlx::query_scalar::<_, Uuid>("select id from posts where author_id=$1 order by id")
            .bind(user_id)
            .fetch_all(&mut *tx)
            .await?;

    sqlx::query(
        r#"
            with removed as (
                update comments
                set content='', deleted_at=coalesce(deleted_at, now()),
                    deleted_by=$2,
                    delete_reason='account_deletion',
                    retention_until=$3,
                    updated_at=now()
                where author_id=$1 and deleted_at is null
                returning post_id
            ), totals as (
                select post_id, count(*)::bigint as removed_count
                from removed group by post_id
            )
            update posts p
            set comment_count=greatest(0, p.comment_count-t.removed_count), updated_at=now()
            from totals t
            where p.id=t.post_id and p.author_id<>$1
            "#,
    )
    .bind(user_id)
    .bind(actor_id)
    .bind(retention_until)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
            with removed as (
                update post_likes
                set deleted_at=coalesce(deleted_at, now()),
                    deleted_by=$2,
                    delete_reason='account_deletion',
                    retention_until=$3
                where user_id=$1 and deleted_at is null
                returning post_id
            ), totals as (
                select post_id, count(*)::bigint as removed_count
                from removed group by post_id
            )
            update posts p
            set like_count=greatest(0, p.like_count-t.removed_count), updated_at=now()
            from totals t
            where p.id=t.post_id and p.author_id<>$1
            "#,
    )
    .bind(user_id)
    .bind(actor_id)
    .bind(retention_until)
    .execute(&mut *tx)
    .await?;

    sqlx::query(
        r#"
            update posts
            set content='', deleted_at=coalesce(deleted_at, now()),
                deleted_by=$2,
                delete_reason='account_deletion',
                retention_until=$3,
                updated_at=now()
            where author_id=$1
            "#,
    )
    .bind(user_id)
    .bind(actor_id)
    .bind(retention_until)
    .execute(&mut *tx)
    .await?;

    // Exact retained affinity cleanup; no new helper visibility or pool path.
    let installed: bool = sqlx::query_scalar(
        "select to_regprocedure('feed_affinity_forget_subject(uuid)') is not null",
    )
    .fetch_one(&mut *tx)
    .await?;
    if installed {
        sqlx::query("select feed_affinity_forget_subject($1)")
            .bind(user_id)
            .execute(&mut *tx)
            .await?;
    }
    for query in [
            "delete from feed_recommendation_impressions where viewer_id=$1 or author_id=$1",
            "delete from feed_recommendation_snapshot_items where author_id=$1 or post_id in (select id from posts where author_id=$1)",
            "delete from feed_recommendation_snapshots where viewer_id=$1",
            "delete from feed_recommendation_events where viewer_id=$1 or actor_id=$1 or (subject_type='USER' and subject_id=$1) or (subject_type='POST' and subject_id in (select id from posts where author_id=$1)) or (subject_type='COMMENT' and subject_id in (select id from comments where author_id=$1))",
            "delete from feed_author_affinities where viewer_id=$1 or author_id=$1",
            "delete from feed_recent_actions where viewer_id=$1 or post_id in (select id from posts where author_id=$1)",
            "delete from feed_experiment_assignments where viewer_id=$1",
            "delete from feed_embeddings where subject_id=$1 or subject_id in (select id from posts where author_id=$1)",
            "delete from post_engagement_velocity where post_id in (select id from posts where author_id=$1)",
            "delete from feed_fanout_jobs where author_id=$1",
            "delete from home_timeline_entries where viewer_id=$1 or author_id=$1",
            "delete from high_fanout_posts where author_id=$1",
            "delete from feed_snapshots where viewer_id=$1",
            "delete from feed_snapshot_items where author_id=$1 or post_id in (select id from posts where author_id=$1)",
            "delete from feed_impressions where viewer_id=$1 or author_id=$1 or post_id in (select id from posts where author_id=$1)",
            "delete from feed_author_affinity where viewer_id=$1 or author_id=$1",
        ] {
            sqlx::query(query).bind(user_id).execute(&mut *tx).await?;
        }
    sqlx::query(
        r#"
            insert into post_audit_events (
                id, actor_id, post_id, comment_id, action, outcome,
                request_id, metadata_json, occurred_at
            ) values ($1,$2,null,null,'ACCOUNT_DELETION_APPLIED','SUCCESS',$3,$4,now())
            on conflict (id) do nothing
            "#,
    )
    .bind(saga_id)
    .bind(actor_id)
    .bind(saga_id.to_string())
    .bind(
        serde_json::json!({
            "userId": user_id,
            "retentionUntil": retention_until,
            "postCount": post_ids.len()
        })
        .to_string(),
    )
    .execute(&mut *tx)
    .await?;
    // Canonical legacy events embed raw post/comment content. Preserve event,
    // ordering, attempts and genuine publication receipts; erase only the body.
    sqlx::query("update post_event_outbox set payload=(payload::jsonb-'content')::text where actor_id=$1 and payload::jsonb ? 'content'")
        .bind(user_id).execute(&mut *tx).await?;
    sqlx::query("update event_publications p set payload=e.payload from post_event_outbox e where p.event_id=e.event_id and e.actor_id=$1")
        .bind(user_id).execute(&mut *tx).await?;
    // Raw legacy bodies and replay outcomes must not survive soft deletion.
    sqlx::query("update posts set content='' where author_id=$1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("update comments set content='' where author_id=$1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    sqlx::query("update post_mutation_receipts set outcome_json=null,replay_until=least(replay_until,now()) where actor_id=$1")
            .bind(user_id).execute(&mut *tx).await?;
    sqlx::query(include_str!("concert_cleanup.sql"))
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let fence_revision: i64 = sqlx::query_scalar(
        "insert into post_deletion_fences_v1(user_id) values($1) returning fence_revision",
    )
    .bind(user_id)
    .fetch_one(&mut *tx)
    .await?;
    let metadata = json!({"proofVersion":1,"sagaId":saga_id,"retentionSeconds":retention_until.timestamp(),"retentionNanos":retention_until.timestamp_subsec_nanos(),"fenceRevision":fence_revision});
    sqlx::query("update post_audit_events set metadata_json=(metadata_json::jsonb || $2::jsonb)::text where id=$1")
            .bind(saga_id).bind(metadata).execute(&mut *tx).await?;
    sqlx::query("insert into post_deletion_receipts_v1(saga_id,user_id,actor_id,retention_until,retention_seconds,retention_nanos,fence_revision,audit_event_id) values($1,$2,$3,$4,$5,$6,$7,$1)")
            .bind(saga_id).bind(user_id).bind(actor_id).bind(retention_until)
            .bind(retention_until.timestamp()).bind(retention_until.timestamp_subsec_nanos() as i32)
            .bind(fence_revision).execute(&mut *tx).await?;
    let receipt = load(&mut tx, saga_id, user_id, actor_id, retention_until)
        .await?
        .ok_or_else(|| anyhow::anyhow!("Deletion proof unavailable"))?;
    tx.commit().await?;
    Ok((receipt, post_ids))
}

async fn load(
    tx: &mut Transaction<'_, Postgres>,
    saga_id: Uuid,
    user_id: Uuid,
    actor_id: Uuid,
    retention_until: DateTime<Utc>,
) -> Result<Option<Receipt>> {
    let rows = sqlx::query("select r.*,r.effect_xid::text as full_xid from post_deletion_receipts_v1 r join post_deletion_fences_v1 f using(user_id,fence_revision) where r.saga_id=$1 or r.user_id=$2")
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
        owner: "POST".to_owned(),
    }))
}

const DELETION_SUBJECTS_SQL: &str = "select $1::uuid union select p.author_id from posts p join comments c on c.post_id=p.id where c.author_id=$1 union select p.author_id from posts p join post_likes l on l.post_id=p.id where l.user_id=$1 union select p.author_id from post_concert_posts p join post_concert_comments c on c.post_id=p.post_id where c.author_id=$1 union select p.author_id from post_concert_posts p join post_concert_reactions r on r.post_id=p.post_id where r.actor_id=$1";
