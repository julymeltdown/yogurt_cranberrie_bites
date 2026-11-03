//! Legacy router-only audit persistence. Durable journal writes are separate.
use crate::application::dto::legacy_audit_dto::LegacyAuditCommandDto;
use crate::application::legacy_audit_error::LegacyAuditStorageError;
use crate::application::ports::LegacyAuditRepositoryPort;
use async_trait::async_trait;
use sqlx::PgPool;

pub struct PgLegacyAuditRepository {
    pool: PgPool,
}

impl PgLegacyAuditRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl LegacyAuditRepositoryPort for PgLegacyAuditRepository {
    async fn record(&self, command: LegacyAuditCommandDto) -> Result<(), LegacyAuditStorageError> {
        sqlx::query(
            "insert into admin_audit_events (id, actor_id, action, resource_type, resource_id, outcome, request_id, ip_address, user_agent, metadata_json, occurred_at) values ($1, $2, $3, $4, null, $5, $6, $7, $8, '{}', now())"
        )
        .bind(command.event_id)
        .bind(command.actor_id)
        .bind(command.action)
        .bind(command.resource_path)
        .bind(command.outcome.as_str())
        .bind(command.request_id)
        .bind(command.ip_address)
        .bind(command.user_agent)
        .execute(&self.pool)
        .await
        .map(|_| ())
        .map_err(|error| {
            tracing::error!(error = ?error, "failed to record admin audit event");
            LegacyAuditStorageError
        })
    }
}
