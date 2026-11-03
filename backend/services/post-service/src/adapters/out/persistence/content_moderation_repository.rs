use super::subject_locks::lock_post_subjects;
use crate::application::ports::ContentModerationRepositoryPort;
use crate::domain::post::{ContentModerationAction, ContentModerationTarget};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgContentModerationRepository {
    pool: PgPool,
}

impl PgContentModerationRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ContentModerationRepositoryPort for PgContentModerationRepository {
    async fn apply_content_moderation(
        &self,
        report_id: Uuid,
        target: ContentModerationTarget,
        target_id: Uuid,
        actor_id: Uuid,
        action: ContentModerationAction,
    ) -> Result<Option<Uuid>> {
        let mut tx = self.pool.begin().await?;
        let post_id = match target {
            ContentModerationTarget::Post => {
                sqlx::query_scalar::<_, Uuid>(
                    "select id from posts where id = $1 and deleted_at is null",
                )
                .bind(target_id)
                .fetch_optional(&mut *tx)
                .await?
            }
            ContentModerationTarget::Comment => {
                sqlx::query_scalar::<_, Uuid>(
                    "select post_id from comments where id = $1 and deleted_at is null",
                )
                .bind(target_id)
                .fetch_optional(&mut *tx)
                .await?
            }
        };
        let Some(post_id) = post_id else {
            tx.commit().await?;
            return Ok(None);
        };

        let mut subjects = vec![actor_id];
        if target == ContentModerationTarget::Comment {
            let author: Option<Uuid> =
                sqlx::query_scalar("select author_id from comments where id=$1")
                    .bind(target_id)
                    .fetch_optional(&mut *tx)
                    .await?;
            subjects.extend(author);
        }
        lock_post_subjects(&mut tx, post_id, &subjects).await?;

        let inserted = sqlx::query(
            r#"
            insert into post_moderation_actions (
                report_id, action, target_type, target_id, actor_id, occurred_at
            )
            values ($1, $2, $3, $4, $5, now())
            on conflict (report_id, action) do nothing
            returning report_id
            "#,
        )
        .bind(report_id)
        .bind(action.as_str())
        .bind(target.as_str())
        .bind(target_id)
        .bind(actor_id)
        .fetch_optional(&mut *tx)
        .await?
        .is_some();

        if !inserted {
            tx.commit().await?;
            return Ok(Some(post_id));
        }

        let changed = match (target, action) {
            (ContentModerationTarget::Post, ContentModerationAction::Hide) => {
                sqlx::query(
                    r#"
                update posts
                set moderation_hidden_at = now(),
                    moderation_report_id = $2,
                    moderation_actor_id = $3,
                    updated_at = now()
                where id = $1
                  and deleted_at is null
                  and moderation_hidden_at is null
                "#,
                )
                .bind(target_id)
                .bind(report_id)
                .bind(actor_id)
                .execute(&mut *tx)
                .await?
                .rows_affected()
                    > 0
            }
            (ContentModerationTarget::Post, ContentModerationAction::Restore) => {
                sqlx::query(
                    r#"
                update posts
                set moderation_hidden_at = null,
                    moderation_report_id = null,
                    moderation_actor_id = null,
                    updated_at = now()
                where id = $1
                  and deleted_at is null
                  and moderation_hidden_at is not null
                  and moderation_report_id = $2
                "#,
                )
                .bind(target_id)
                .bind(report_id)
                .execute(&mut *tx)
                .await?
                .rows_affected()
                    > 0
            }
            (ContentModerationTarget::Comment, ContentModerationAction::Hide) => {
                let row = sqlx::query(
                    r#"
                    update comments
                    set moderation_hidden_at = now(),
                        moderation_report_id = $2,
                        moderation_actor_id = $3,
                        updated_at = now()
                    where id = $1
                      and deleted_at is null
                      and moderation_hidden_at is null
                    returning post_id
                    "#,
                )
                .bind(target_id)
                .bind(report_id)
                .bind(actor_id)
                .fetch_optional(&mut *tx)
                .await?;
                if let Some(row) = row {
                    let changed_post_id: Uuid = row.get("post_id");
                    sqlx::query(
                        "update posts set comment_count = greatest(comment_count - 1, 0) where id = $1",
                    )
                    .bind(changed_post_id)
                    .execute(&mut *tx)
                    .await?;
                    true
                } else {
                    false
                }
            }
            (ContentModerationTarget::Comment, ContentModerationAction::Restore) => {
                let row = sqlx::query(
                    r#"
                    update comments
                    set moderation_hidden_at = null,
                        moderation_report_id = null,
                        moderation_actor_id = null,
                        updated_at = now()
                    where id = $1
                      and deleted_at is null
                      and moderation_hidden_at is not null
                      and moderation_report_id = $2
                    returning post_id
                    "#,
                )
                .bind(target_id)
                .bind(report_id)
                .fetch_optional(&mut *tx)
                .await?;
                if let Some(row) = row {
                    let changed_post_id: Uuid = row.get("post_id");
                    sqlx::query(
                        "update posts set comment_count = comment_count + 1 where id = $1 and deleted_at is null",
                    )
                    .bind(changed_post_id)
                    .execute(&mut *tx)
                    .await?;
                    true
                } else {
                    false
                }
            }
        };

        sqlx::query(
            r#"
            insert into post_audit_events (
                id, actor_id, post_id, comment_id, action, outcome,
                request_id, ip_address, user_agent, metadata_json, occurred_at
            )
            values ($1, $2, $3, $4, $5, $6, null, null, null, $7, now())
            "#,
        )
        .bind(Uuid::new_v4())
        .bind(actor_id)
        .bind(post_id)
        .bind(if target == ContentModerationTarget::Comment {
            Some(target_id)
        } else {
            None
        })
        .bind(format!(
            "{}_MODERATION_{}",
            target.as_str(),
            action.as_str()
        ))
        .bind(if changed { "SUCCESS" } else { "NOOP" })
        .bind(
            json!({
                "reportId": report_id,
                "targetType": target.as_str(),
                "targetId": target_id,
                "action": action.as_str()
            })
            .to_string(),
        )
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;
        Ok(Some(post_id))
    }
}
