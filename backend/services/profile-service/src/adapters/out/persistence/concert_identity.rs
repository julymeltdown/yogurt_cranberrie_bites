//! One uncached, parameterized owner snapshot; no writes or portable admission proof.
use async_trait::async_trait;
use profile_application::concert_identity::{
    ConcertProfileError, ConcertProfileQuery, ConcertProfileRecord, ConcertProfileRepository,
};
use sqlx::{PgPool, Row};
use tokio::time::Instant;

#[derive(Clone)]
pub struct PgConcertProfileRepository {
    pool: PgPool,
}
impl PgConcertProfileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    async fn snapshot(
        &self,
        query: ConcertProfileQuery,
        stop: Instant,
    ) -> Result<Option<ConcertProfileRecord>, ConcertProfileError> {
        remaining_millis(stop)?;
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        // A dropped SQLx future does not cancel PostgreSQL. Install the bound in
        // a separate command BEFORE parsing/planning or waiting on owner locks.
        // LOCAL rolls back with this transaction, including cancellation/error.
        let millis = remaining_millis(stop)?;
        sqlx::query("select set_config('statement_timeout', $1, true)")
            .bind(millis.to_string())
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        remaining_millis(stop)?;
        // Statement snapshot covers both current content and the owning deletion fence.
        // Bound transfer before decoding: UTF-8 uses at most four bytes per Unicode scalar.
        // Character limits also exclude oversized ASCII without truncating stored content.
        // Private profiles remain readable by their authenticated self producer. Auth is
        // enforced by the signed RPC; this selector is not an actor proof.
        let row = sqlx::query(
            "select p.user_id, p.nickname, p.bio, statement_timestamp() as observed_at
             from profiles p
             where p.user_id = $1 and p.deleted_at is null
               and octet_length(p.nickname) <= 160 and char_length(p.nickname) <= 40
               and octet_length(p.bio) <= 640 and char_length(p.bio) <= 160
               and not exists (
                   select 1 from profile_user_deletion_tombstones d where d.user_id = p.user_id
               )",
        )
        .bind(query.user_id())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| ConcertProfileError::DependencyUnavailable)?;
        remaining_millis(stop)?;
        let record = row
            .map(|row| {
                Ok(ConcertProfileRecord {
                    user_id: row
                        .try_get("user_id")
                        .map_err(|_| ConcertProfileError::DependencyUnavailable)?,
                    nickname: row
                        .try_get("nickname")
                        .map_err(|_| ConcertProfileError::DependencyUnavailable)?,
                    bio: row
                        .try_get("bio")
                        .map_err(|_| ConcertProfileError::DependencyUnavailable)?,
                    observed_at: row
                        .try_get("observed_at")
                        .map_err(|_| ConcertProfileError::DependencyUnavailable)?,
                })
            })
            .transpose()?;
        tx.commit().await.map_err(unavailable)?;
        remaining_millis(stop)?;
        Ok(record)
    }
}

#[async_trait]
impl ConcertProfileRepository for PgConcertProfileRepository {
    async fn find_self_content(
        &self,
        query: ConcertProfileQuery,
    ) -> Result<Option<ConcertProfileRecord>, ConcertProfileError> {
        // Even a direct owner call uses the retained edge ceiling. The RPC scopes
        // its verified original/proof stop, which can only shorten this one stop.
        let ceiling = platform::grpc_deadline::local_deadline(platform::edge::Edge::GatewayProfile);
        let edge_stop = Instant::now() + ceiling;
        let stop = platform::call_deadline::current()
            .map(Instant::from_std)
            .map_or(edge_stop, |original| original.min(edge_stop));
        let result = tokio::time::timeout_at(stop, self.snapshot(query, stop))
            .await
            .map_err(|_| ConcertProfileError::DependencyUnavailable)?;
        remaining_millis(stop)?;
        result
    }
}

fn unavailable(_: sqlx::Error) -> ConcertProfileError {
    ConcertProfileError::DependencyUnavailable
}

fn remaining_millis(stop: Instant) -> Result<u128, ConcertProfileError> {
    // PostgreSQL interprets zero as DISABLED. Do not round a submillisecond or
    // expired budget up into fresh authority; refuse without issuing the read.
    let millis = stop.saturating_duration_since(Instant::now()).as_millis();
    if millis == 0 {
        return Err(ConcertProfileError::DependencyUnavailable);
    }
    Ok(millis)
}
