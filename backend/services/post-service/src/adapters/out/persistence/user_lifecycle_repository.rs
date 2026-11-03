use super::subject_locks::forget_affinity_sources;
use crate::application::ports::UserLifecycleRepositoryPort;
use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Clone)]
pub struct PgUserLifecycleRepository {
    pool: PgPool,
}

impl PgUserLifecycleRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserLifecycleRepositoryPort for PgUserLifecycleRepository {
    async fn apply_user_deletion(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> Result<Vec<Uuid>> {
        // Deleting this actor can update other authors' post counters. Gather
        // ALL owners before taking sorted subject locks; retry discovery if a
        // producer won and introduced a new owner before our actor lock.
        let mut tx = loop {
            let mut candidate = self.pool.begin().await?;
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

        let post_ids =
            sqlx::query_scalar::<_, Uuid>("select id from posts where author_id=$1 order by id")
                .bind(user_id)
                .fetch_all(&mut *tx)
                .await?;

        sqlx::query(
            r#"
            with removed as (
                update comments
                set deleted_at=coalesce(deleted_at, now()),
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
            set deleted_at=coalesce(deleted_at, now()),
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

        forget_affinity_sources(&mut tx, user_id).await?;
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
        tx.commit().await?;
        Ok(post_ids)
    }
}

const DELETION_SUBJECTS_SQL: &str = "select $1::uuid union select p.author_id from posts p join comments c on c.post_id=p.id where c.author_id=$1 union select p.author_id from posts p join post_likes l on l.post_id=p.id where l.user_id=$1";
