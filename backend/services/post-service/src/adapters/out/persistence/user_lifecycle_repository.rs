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
        crate::deletion_receipt::apply(&self.pool, saga_id, user_id, actor_id, retention_until)
            .await
            .map(|(_, ids)| ids)
    }

    async fn apply_user_deletion_with_receipt(
        &self,
        saga_id: Uuid,
        user_id: Uuid,
        actor_id: Uuid,
        retention_until: DateTime<Utc>,
    ) -> Result<(
        crate::application::dto::post_dto::CommittedDeletionReceiptDto,
        Vec<Uuid>,
    )> {
        crate::deletion_receipt::apply(&self.pool, saga_id, user_id, actor_id, retention_until)
            .await
    }
}
