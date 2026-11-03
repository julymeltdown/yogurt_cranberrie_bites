//! Owning friend database only. All facts and the coarse revision share one MVCC
//! statement snapshot; no claim of authority surviving later revocation.
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use friend_application::concert_relationship::*;
use sqlx::{PgPool, Row};
use std::{fmt, future::Future, time::Duration};
#[derive(Clone)]
pub struct PgCurrentRelationship {
    pool: PgPool,
}
impl PgCurrentRelationship {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    async fn snapshot(
        &self,
        request: RelationshipRead,
        budget: &ReadBudget,
    ) -> Result<CurrentRelationship, ReadError> {
        budget.check()?;
        request.check_at(Utc::now())?;
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        budget.check()?;
        sqlx::query("SET TRANSACTION READ ONLY")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        budget.check()?;
        sqlx::query("select set_config('statement_timeout',$1,true)")
            .bind(budget.remaining()?.as_millis().max(1).to_string())
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        budget.check()?;
        let row=sqlx::query(r#"WITH owner_clock AS MATERIALIZED (SELECT clock_timestamp() AS observed_at)
SELECT r.owner_epoch,r.revision,c.observed_at,
  EXISTS(SELECT 1 FROM public.user_blocks WHERE blocker_id=$1 AND blocked_id=$2 AND deleted_at IS NULL) AS actor_blocks_subject,
  EXISTS(SELECT 1 FROM public.user_blocks WHERE blocker_id=$2 AND blocked_id=$1 AND deleted_at IS NULL) AS subject_blocks_actor,
  EXISTS(SELECT 1 FROM public.friend_user_deletion_tombstones WHERE user_id=$1) AS actor_deleted,
  EXISTS(SELECT 1 FROM public.friend_user_deletion_tombstones WHERE user_id=$2) AS subject_deleted
FROM public.friend_concert_relationship_revision r CROSS JOIN owner_clock c WHERE r.singleton"#).bind(request.actor_id()).bind(request.subject_id()).fetch_optional(&mut *tx).await.map_err(unavailable)?.ok_or(ReadError::Unavailable)?;
        budget.check()?;
        let observed_at: DateTime<Utc> = row.try_get("observed_at").map_err(unavailable)?;
        request.check_at(observed_at)?;
        let valid_until = request.expires_at().min(observed_at + MAX_OBSERVATION);
        let owner_epoch = row.try_get("owner_epoch").map_err(unavailable)?;
        let revision = row.try_get("revision").map_err(unavailable)?;
        let result = CurrentRelationship {
            query: request,
            observed_at,
            valid_until,
            owner_epoch,
            revision,
            actor_blocks_subject: row.try_get("actor_blocks_subject").map_err(unavailable)?,
            subject_blocks_actor: row.try_get("subject_blocks_actor").map_err(unavailable)?,
            actor_deleted: row.try_get("actor_deleted").map_err(unavailable)?,
            subject_deleted: row.try_get("subject_deleted").map_err(unavailable)?,
        };
        let result = result.validate(request, Utc::now())?;
        tx.commit().await.map_err(unavailable)?;
        budget.check()?;
        result.validate(request, Utc::now())
    }
}
impl fmt::Debug for PgCurrentRelationship {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("PgCurrentRelationship([REDACTED])")
    }
}
fn unavailable(error: sqlx::Error) -> ReadError {
    if error.as_database_error().and_then(|e| e.code()).as_deref() == Some("57014") {
        ReadError::DeadlineExceeded
    } else {
        ReadError::Unavailable
    }
}
async fn bounded<T>(
    budget: &ReadBudget,
    request: RelationshipRead,
    future: impl Future<Output = Result<T, ReadError>>,
) -> Result<T, ReadError> {
    budget.check()?;
    request.check_at(Utc::now())?;
    let poll = async {
        loop {
            budget.check()?;
            request.check_at(Utc::now())?;
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    };
    tokio::select! {biased;
        _=tokio::time::sleep_until(budget.deadline().into())=>Err(ReadError::DeadlineExceeded),
        result=poll=>result,
        result=future=>{budget.check()?;request.check_at(Utc::now())?;result}
    }
}
#[async_trait]
impl CurrentRelationshipPort for PgCurrentRelationship {
    async fn read_current(
        &self,
        request: RelationshipRead,
        budget: &ReadBudget,
    ) -> Result<CurrentRelationship, ReadError> {
        bounded(budget, request, self.snapshot(request, budget)).await
    }
}
