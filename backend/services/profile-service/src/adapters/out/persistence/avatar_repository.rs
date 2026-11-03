use crate::adapters::out::errors::repository_error;
use crate::application::dto::profile_dto::AvatarDto;
use crate::application::errors::{ProfileError, ProfileResult as Result};
use crate::application::ports::AvatarRepositoryPort;
use async_trait::async_trait;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub struct PgAvatarRepository {
    pool: PgPool,
}

impl PgAvatarRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AvatarRepositoryPort for PgAvatarRepository {
    async fn save(&self, user_id: Uuid, avatar: &AvatarDto) -> Result<()> {
        let result: anyhow::Result<_> = async {
            let saved = sqlx::query(
                r#"
            insert into avatars (user_id, content, content_type, filename)
            select $1, $2, $3, $4
            from profiles
            where user_id = $1
              and deleted_at is null
            on conflict (user_id) do update
            set content = excluded.content,
                content_type = excluded.content_type,
                filename = excluded.filename,
                updated_at = now()
            where exists (
                select 1 from profiles
                where profiles.user_id = excluded.user_id
                  and profiles.deleted_at is null
            )
            returning user_id
            "#,
            )
            .bind(user_id)
            .bind(&avatar.content)
            .bind(&avatar.content_type)
            .bind(&avatar.filename)
            .fetch_optional(&self.pool)
            .await?;
            if saved.is_none() {
                return Err(ProfileError::ProfileNotFound.into());
            }
            Ok(())
        }
        .await;
        result.map_err(repository_error)
    }

    async fn find(&self, user_id: Uuid) -> Result<Option<AvatarDto>> {
        let result: anyhow::Result<_> = async {
            let row = sqlx::query(
                r#"
            select content, content_type, filename
            from avatars
            where user_id = $1
              and deleted_at is null
            "#,
            )
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;

            Ok(row.map(|row| AvatarDto {
                content: row.get("content"),
                content_type: row.get("content_type"),
                filename: row.get("filename"),
            }))
        }
        .await;
        result.map_err(repository_error)
    }
}
