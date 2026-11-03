use crate::adapters::out::errors::repository_error;
use crate::adapters::out::persistence::mutation_receipts;
use crate::application::dto::mutation_dto::{
    MutationReceiptStatusDto, ProfileMutationOperationDto,
};
use crate::application::dto::profile_dto::{ProfileDeletionAuditDto, ProfileDto};
use crate::application::dto::repository_dto::{
    ProfileAuditEventDto, ProfileRecordDto, ProfileVisibilityDto,
};
use crate::application::errors::{ProfileError, ProfileResult as Result};
use crate::application::ports::{ProfileMutationReceiptMaintenancePort, ProfileRepositoryPort};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde_json::json;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgProfileRepository {
    pool: PgPool,
}

impl PgProfileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    fn map_profile(row: PgRow) -> ProfileRecordDto {
        ProfileRecordDto {
            user_id: row.get("user_id"),
            username: row.get::<String, _>("username"),
            nickname: row.get::<String, _>("nickname"),
            avatar_url: row.get::<String, _>("avatar_url"),
            bio: row.get::<String, _>("bio"),
            follower_count: row.get::<i64, _>("follower_count"),
            post_count: row.get::<i64, _>("post_count"),
            following_count: row.get::<i64, _>("following_count"),
            account_visibility: match row.get::<String, _>("account_visibility").as_str() {
                "PRIVATE" => ProfileVisibilityDto::Private,
                _ => ProfileVisibilityDto::Public,
            },
            lifecycle_status: "ACTIVE".to_owned(),
        }
    }
}

#[async_trait]
impl profile_application::author_existence::AuthorProfileLookupPort for PgProfileRepository {
    async fn find_existing_author(
        &self,
        query: profile_application::author_existence::AuthorProfileQueryDto,
    ) -> Result<Option<profile_application::author_existence::ExistingAuthorProfileDto>> {
        // Exactly the existing GetProfile lifecycle predicate. Private profiles
        // still exist; no content, counters or privacy values leave this query.
        let user_id = sqlx::query_scalar::<_, Uuid>(
            "select user_id from profiles where user_id=$1 and deleted_at is null",
        )
        .bind(query.user_id())
        .fetch_optional(&self.pool)
        .await
        .map_err(|error| repository_error(error.into()))?;
        user_id
            .map(profile_application::author_existence::ExistingAuthorProfileDto::new)
            .transpose()
    }
}

#[async_trait]
impl ProfileRepositoryPort for PgProfileRepository {
    async fn find_by_user_id(&self, user_id: Uuid) -> Result<Option<ProfileRecordDto>> {
        let result: anyhow::Result<_> = async {
        let row = sqlx::query(
            r#"
            select user_id, username, nickname, avatar_url, bio, follower_count, post_count, following_count, account_visibility
            from profiles
            where user_id = $1
              and deleted_at is null
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Self::map_profile))

        }.await;
        result.map_err(repository_error)
    }

    async fn find_by_username(&self, username: &str) -> Result<Option<ProfileRecordDto>> {
        let result: anyhow::Result<_> = async {
        let row = sqlx::query(
            r#"
            select user_id, username, nickname, avatar_url, bio, follower_count, post_count, following_count, account_visibility
            from profiles
            where username = $1
              and deleted_at is null
            "#,
        )
        .bind(username)
        .fetch_optional(&self.pool)
        .await?;
        Ok(row.map(Self::map_profile))

        }.await;
        result.map_err(repository_error)
    }

    async fn find_username_owner(&self, username: &str) -> Result<Option<Uuid>> {
        let result: anyhow::Result<_> = async {
            // Deliberately does not filter soft-deleted rows: the unique constraint on
            // the column does not either, so the reservation outlives the deletion.
            let owner =
                sqlx::query_scalar::<_, Uuid>("select user_id from profiles where username = $1")
                    .bind(username)
                    .fetch_optional(&self.pool)
                    .await?;
            Ok(owner)
        }
        .await;
        result.map_err(repository_error)
    }

    async fn find_by_user_ids(&self, user_ids: &[Uuid]) -> Result<Vec<ProfileRecordDto>> {
        let result: anyhow::Result<_> = async {
            if user_ids.is_empty() {
                return Ok(Vec::new());
            }
            let rows = sqlx::query(
                r#"
            select user_id, username, nickname, avatar_url, bio,
                   follower_count, post_count, following_count, account_visibility
            from profiles
            where user_id = any($1)
              and deleted_at is null
            "#,
            )
            .bind(user_ids)
            .fetch_all(&self.pool)
            .await?;
            Ok(rows.into_iter().map(Self::map_profile).collect())
        }
        .await;
        result.map_err(repository_error)
    }

    async fn save(&self, profile: &ProfileRecordDto) -> Result<ProfileRecordDto> {
        let result: anyhow::Result<_> = async {
        let row = sqlx::query(
            r#"
            insert into profiles (
                user_id, username, nickname, avatar_url, bio, follower_count, post_count, following_count
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8)
            on conflict (user_id) do update
            set username = excluded.username,
                nickname = excluded.nickname,
                avatar_url = excluded.avatar_url,
                bio = excluded.bio,
                follower_count = excluded.follower_count,
                post_count = excluded.post_count,
                following_count = excluded.following_count,
                updated_at = now()
            where profiles.deleted_at is null
            returning user_id, username, nickname, avatar_url, bio, follower_count, post_count, following_count, account_visibility
            "#,
        )
        .bind(profile.user_id)
        .bind(&profile.username)
        .bind(&profile.nickname)
        .bind(&profile.avatar_url)
        .bind(&profile.bio)
        .bind(profile.follower_count)
        .bind(profile.post_count)
        .bind(profile.following_count)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(ProfileError::InvalidInput("Profile is deleted"))?;
        Ok(Self::map_profile(row))

        }.await;
        result.map_err(repository_error)
    }

    async fn update_visibility(
        &self,
        user_id: Uuid,
        visibility: ProfileVisibilityDto,
    ) -> Result<Option<ProfileRecordDto>> {
        let result: anyhow::Result<_> = async {
            let row = sqlx::query(
                r#"
            update profiles
            set account_visibility = $2, updated_at = now()
            where user_id = $1 and deleted_at is null
            returning user_id, username, nickname, avatar_url, bio,
                      follower_count, post_count, following_count, account_visibility
            "#,
            )
            .bind(user_id)
            .bind(visibility.as_str())
            .fetch_optional(&self.pool)
            .await?;
            Ok(row.map(Self::map_profile))
        }
        .await;
        result.map_err(repository_error)
    }

    async fn lookup_privacy_mutation(
        &self,
        operation: &ProfileMutationOperationDto,
    ) -> Result<Option<ProfileDto>> {
        self.lookup_profile_mutation(operation).await
    }

    async fn lookup_profile_mutation(
        &self,
        operation: &ProfileMutationOperationDto,
    ) -> Result<Option<ProfileDto>> {
        let result: anyhow::Result<_> = async {
            let mut tx = self.pool.begin().await?;
            let outcome = mutation_receipts::lookup(&mut tx, operation).await?;
            tx.commit().await?;
            Ok(outcome)
        }
        .await;
        result.map_err(repository_error)
    }

    async fn update_visibility_with_receipt(
        &self,
        user_id: Uuid,
        visibility: ProfileVisibilityDto,
        operation: &ProfileMutationOperationDto,
    ) -> Result<Option<ProfileDto>> {
        let result: anyhow::Result<_> = async {
            let mut tx = self.pool.begin().await?;
            if let Some(profile) = mutation_receipts::claim(&mut tx, operation).await? {
                tx.commit().await?;
                return Ok(Some(profile));
            }

            let row = sqlx::query(
                r#"
            update profiles
            set account_visibility = $2, updated_at = now()
            where user_id = $1 and deleted_at is null
            returning user_id, username, nickname, avatar_url, bio,
                      follower_count, post_count, following_count, account_visibility
            "#,
            )
            .bind(user_id)
            .bind(visibility.as_str())
            .fetch_optional(&mut *tx)
            .await?;
            let Some(row) = row else {
                tx.rollback().await?;
                return Ok(None);
            };
            let profile = ProfileDto::from(Self::map_profile(row));
            let metadata = serde_json::json!({
                "accountVisibility": visibility.as_str(),
                "operationId": operation.context.operation_id,
            })
            .to_string();
            sqlx::query(
                r#"
            insert into profile_audit_events (
                id, actor_id, profile_user_id, action, outcome,
                request_id, metadata_json
            )
            values ($1, $2, $3, 'UPDATE_PRIVACY', 'SUCCESS', $4, $5)
            "#,
            )
            .bind(Uuid::new_v4())
            .bind(operation.context.actor_id)
            .bind(user_id)
            .bind(operation.context.operation_id.to_string())
            .bind(metadata)
            .execute(&mut *tx)
            .await?;
            mutation_receipts::complete(&mut tx, operation, &profile).await?;
            tx.commit().await?;
            Ok(Some(profile))
        }
        .await;
        result.map_err(repository_error)
    }

    async fn update_profile_with_receipt(
        &self,
        user_id: Uuid,
        username: &str,
        nickname: &str,
        avatar_url: &str,
        bio: &str,
        operation: &ProfileMutationOperationDto,
    ) -> Result<Option<ProfileDto>> {
        let result: anyhow::Result<_> = async {
            let mut tx = self.pool.begin().await?;
            if let Some(profile) = mutation_receipts::claim(&mut tx, operation).await? {
                tx.commit().await?;
                return Ok(Some(profile));
            }

            let username_owner = sqlx::query_scalar::<_, Uuid>(
                "select user_id from profiles where username = $1 for update",
            )
            .bind(username)
            .fetch_optional(&mut *tx)
            .await?;
            if username_owner.is_some_and(|owner| owner != user_id) {
                return Err(ProfileError::InvalidInput("Username already taken").into());
            }

            let row = sqlx::query(
                r#"
            insert into profiles (user_id, username, nickname, avatar_url, bio)
            values ($1, $2, $3, $4, $5)
            on conflict (user_id) do update
            set username = excluded.username,
                nickname = excluded.nickname,
                avatar_url = excluded.avatar_url,
                bio = excluded.bio,
                updated_at = now()
            where profiles.deleted_at is null
            returning user_id, username, nickname, avatar_url, bio,
                      follower_count, post_count, following_count, account_visibility
            "#,
            )
            .bind(user_id)
            .bind(username)
            .bind(nickname)
            .bind(avatar_url)
            .bind(bio)
            .fetch_optional(&mut *tx)
            .await;
            let row = match row {
                Ok(row) => row,
                Err(sqlx::Error::Database(error)) if error.code().as_deref() == Some("23505") => {
                    return Err(ProfileError::InvalidInput("Username already taken").into());
                }
                Err(error) => return Err(error.into()),
            };
            let Some(row) = row else {
                tx.rollback().await?;
                return Ok(None);
            };
            let profile = ProfileDto::from(Self::map_profile(row));
            mutation_receipts::complete(&mut tx, operation, &profile).await?;
            tx.commit().await?;
            Ok(Some(profile))
        }
        .await;
        result.map_err(repository_error)
    }

    async fn mutation_receipt_status(
        &self,
        operation_id: Uuid,
    ) -> Result<Option<MutationReceiptStatusDto>> {
        let result: anyhow::Result<_> =
            async { mutation_receipts::status(&self.pool, operation_id).await }.await;
        result.map_err(repository_error)
    }

    async fn search(
        &self,
        needle: &str,
        cursor: Option<&str>,
        limit: i32,
    ) -> Result<(Vec<ProfileRecordDto>, Option<String>)> {
        let result: anyhow::Result<_> = async {
        // Escaped so a literal % or _ matches literally rather than every row.
        let search_term = platform::search::contains_pattern(needle);
        let resolved_limit = i64::from(limit.max(1));
        let rows = sqlx::query(
            r#"
            select user_id, username, nickname, avatar_url, bio, follower_count, post_count, following_count, account_visibility
            from profiles
            where (
                lower(username) like $1 escape '\'
                or lower(nickname) like $1 escape '\'
                or lower(bio) like $1 escape '\'
            )
            and deleted_at is null
            and ($2::text is null or username > $2)
            order by username asc
            limit $3
            "#,
        )
        .bind(search_term)
        .bind(cursor)
        .bind(resolved_limit + 1)
        .fetch_all(&self.pool)
        .await?;

        let mut profiles: Vec<ProfileRecordDto> = rows.into_iter().map(Self::map_profile).collect();
        let next_cursor = if profiles.len() as i64 > resolved_limit {
            let extra = profiles.pop();
            extra.map(|_| {
                profiles
                    .last()
                    .map(|profile| profile.username.clone())
                    .unwrap_or_default()
            })
        } else {
            None
        };

        Ok((profiles, next_cursor.filter(|value| !value.is_empty())))

        }.await;
        result.map_err(repository_error)
    }

    async fn soft_delete_profile(
        &self,
        user_id: Uuid,
        actor_id: Uuid,
        reason: &str,
        retention_until: DateTime<Utc>,
        audit: &ProfileDeletionAuditDto,
    ) -> Result<bool> {
        let result: anyhow::Result<_> = async {
            let mut tx = self.pool.begin().await?;
            let result = sqlx::query(
                r#"
            update profiles
            set updated_at = now(),
                deleted_at = now(),
                deleted_by = $2,
                delete_reason = $3,
                retention_until = $4
            where user_id = $1
              and deleted_at is null
            "#,
            )
            .bind(user_id)
            .bind(actor_id)
            .bind(reason)
            .bind(retention_until)
            .execute(&mut *tx)
            .await?;

            if result.rows_affected() > 0 {
                sqlx::query(
                    r#"
                update avatars
                set updated_at = now(),
                    deleted_at = now(),
                    deleted_by = $2,
                    delete_reason = $3,
                    retention_until = $4
                where user_id = $1
                  and deleted_at is null
                "#,
                )
                .bind(user_id)
                .bind(actor_id)
                .bind(reason)
                .bind(retention_until)
                .execute(&mut *tx)
                .await?;

                // Required success audit shares the owning transaction with both
                // profile and avatar changes. An unavailable audit rolls them back.
                sqlx::query(
                    r#"
                insert into profile_audit_events (
                    id, actor_id, profile_user_id, action, outcome,
                    request_id, ip_address, user_agent, metadata_json, occurred_at
                )
                values ($1, $2, $3, 'PROFILE_DELETED', 'SUCCESS', $4, $5, $6, $7, $8)
                "#,
                )
                .bind(audit.id)
                .bind(actor_id)
                .bind(user_id)
                .bind(&audit.request_id)
                .bind(&audit.ip_address)
                .bind(&audit.user_agent)
                .bind(&audit.metadata_json)
                .bind(audit.occurred_at)
                .execute(&mut *tx)
                .await?;
            }

            tx.commit().await?;
            Ok(result.rows_affected() > 0)
        }
        .await;
        result.map_err(repository_error)
    }

    async fn record_audit_event(&self, event: &ProfileAuditEventDto) -> Result<()> {
        let result: anyhow::Result<_> = async {
            sqlx::query(
                r#"
            insert into profile_audit_events (
                id, actor_id, profile_user_id, action, outcome,
                request_id, ip_address, user_agent, metadata_json, occurred_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
            "#,
            )
            .bind(event.id)
            .bind(event.actor_id)
            .bind(event.profile_user_id)
            .bind(&event.action)
            .bind(&event.outcome)
            .bind(&event.request_id)
            .bind(&event.ip_address)
            .bind(&event.user_agent)
            .bind(&event.metadata_json)
            .bind(event.occurred_at)
            .execute(&self.pool)
            .await?;
            Ok(())
        }
        .await;
        result.map_err(repository_error)
    }

    async fn apply_user_deletion(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> Result<()> {
        let result: anyhow::Result<_> = async {
            let mut tx = self.pool.begin().await?;
            // Use the same subject lock as profile/avatar writes. A late writer
            // either commits before this step and is removed below, or wakes after
            // the tombstone and is rejected by the database trigger.
            sqlx::query("select profile_lock_subject_ids($1::uuid[])")
                .bind(vec![user_id])
                .execute(&mut *tx)
                .await?;
            sqlx::query(
                r#"
            insert into profile_user_deletion_tombstones (
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
            from profile_user_deletion_tombstones
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
                ProfileError::InvalidInput(
                    "Profile deletion identity was reused with different saga data"
                )
            );

            sqlx::query(
                r#"
            update profiles
            set deleted_at = coalesce(deleted_at, now()),
                deleted_by = $2,
                delete_reason = 'account_deletion',
                retention_until = $3,
                updated_at = now()
            where user_id = $1
            "#,
            )
            .bind(user_id)
            .bind(actor_id)
            .bind(retention_until)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
            update avatars
            set deleted_at = coalesce(deleted_at, now()),
                deleted_by = $2,
                delete_reason = 'account_deletion',
                retention_until = $3,
                updated_at = now()
            where user_id = $1
            "#,
            )
            .bind(user_id)
            .bind(actor_id)
            .bind(retention_until)
            .execute(&mut *tx)
            .await?;
            sqlx::query(
                r#"
            insert into profile_audit_events (
                id, actor_id, profile_user_id, action, outcome,
                request_id, metadata_json, occurred_at
            ) values ($1,$2,$3,'ACCOUNT_DELETION_APPLIED','SUCCESS',$4,$5,now())
            on conflict (id) do nothing
            "#,
            )
            .bind(saga_id)
            .bind(actor_id)
            .bind(user_id)
            .bind(saga_id.to_string())
            .bind(json!({"retentionUntil": retention_until}).to_string())
            .execute(&mut *tx)
            .await?;
            tx.commit().await?;
            Ok(())
        }
        .await;
        result.map_err(repository_error)
    }

    async fn purge_expired_profiles(
        &self,
        cutoff: DateTime<Utc>,
        actor_id: Uuid,
        request_id: Option<String>,
        ip_address: Option<String>,
        user_agent: Option<String>,
        dry_run: bool,
    ) -> Result<Vec<Uuid>> {
        let result: anyhow::Result<_> = async {
            let mut tx = self.pool.begin().await?;
            let rows = sqlx::query(
                r#"
            select user_id
            from profiles
            where deleted_at is not null
              and retention_until is not null
              and retention_until <= $1
            order by retention_until asc, user_id asc
            "#,
            )
            .bind(cutoff)
            .fetch_all(&mut *tx)
            .await?;
            let profile_ids: Vec<Uuid> = rows.into_iter().map(|row| row.get("user_id")).collect();

            if profile_ids.is_empty() {
                tx.commit().await?;
                return Ok(profile_ids);
            }

            let outcome = if dry_run { "DRY_RUN" } else { "SUCCESS" };
            for profile_id in &profile_ids {
                sqlx::query(
                    r#"
                insert into profile_audit_events (
                    id, actor_id, profile_user_id, action, outcome,
                    request_id, ip_address, user_agent, metadata_json, occurred_at
                )
                values ($1, $2, $3, 'PROFILE_PURGED', $4, $5, $6, $7, $8, now())
                "#,
                )
                .bind(Uuid::new_v4())
                .bind(actor_id)
                .bind(*profile_id)
                .bind(outcome)
                .bind(&request_id)
                .bind(&ip_address)
                .bind(&user_agent)
                .bind(
                    json!({
                        "cutoff": cutoff,
                        "dryRun": dry_run
                    })
                    .to_string(),
                )
                .execute(&mut *tx)
                .await?;
            }

            if dry_run {
                tx.commit().await?;
                return Ok(profile_ids);
            }

            sqlx::query("delete from avatars where user_id = any($1)")
                .bind(&profile_ids)
                .execute(&mut *tx)
                .await?;
            sqlx::query("delete from profiles where user_id = any($1)")
                .bind(&profile_ids)
                .execute(&mut *tx)
                .await?;

            tx.commit().await?;
            Ok(profile_ids)
        }
        .await;
        result.map_err(repository_error)
    }
}

#[async_trait]
impl ProfileMutationReceiptMaintenancePort for PgProfileRepository {
    async fn purge_expired_mutation_outcomes(&self, limit: i32) -> Result<i64> {
        let result: anyhow::Result<_> =
            async { mutation_receipts::purge_expired_outcomes(&self.pool, limit).await }.await;
        result.map_err(repository_error)
    }
}
