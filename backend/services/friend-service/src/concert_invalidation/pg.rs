use super::*;
use sqlx::{PgPool, Row};

/// Owner-local reader only. Workload authentication and RPC composition are separate.
#[derive(Clone)]
pub struct PgJournal {
    pool: PgPool,
}
impl PgJournal {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub async fn read_head(
        &self,
        query: HeadQuery,
        budget: &ReadBudget,
    ) -> Result<Head, ReadError> {
        budget.run(async {
            let mut connection = self.pool.acquire().await.map_err(database_error)?;
            // A cancelled future must never return a connection carrying a pending
            // statement/error to the pool. Server timeout also bounds abandoned work.
            connection.close_on_drop();
            sqlx::query("BEGIN READ ONLY").execute(&mut *connection).await.map_err(database_error)?;
            timeout(&mut connection, budget).await?;
            let row = sqlx::query("SELECT owner_epoch,sequence FROM public.friend_world_invalidation_head WHERE singleton")
                .fetch_one(&mut *connection).await.map_err(database_error)?;
            let head = decode_cursor(row.try_get("owner_epoch").map_err(database_error)?, row.try_get("sequence").map_err(database_error)?)?;
            budget.check()?;
            let result=Head { query, head };
            result.to_wire()?;
            sqlx::query("COMMIT").execute(&mut *connection).await.map_err(database_error)?;
            Ok(result)
        }).await
    }
    pub async fn read_after(
        &self,
        query: PageQuery,
        budget: &ReadBudget,
    ) -> Result<Page, ReadError> {
        budget.run(async {
            let mut connection=self.pool.acquire().await.map_err(database_error)?;
            connection.close_on_drop();
            sqlx::query("BEGIN READ ONLY").execute(&mut *connection).await.map_err(database_error)?;
            timeout(&mut connection,budget).await?;
            // One statement, one MVCC snapshot: the head and page cannot straddle
            // another transaction's commit. No row locks and no permission grants.
            let rows=sqlx::query("WITH h AS MATERIALIZED (SELECT owner_epoch,sequence FROM public.friend_world_invalidation_head WHERE singleton), p AS MATERIALIZED (SELECT j.* FROM public.friend_world_invalidation_journal j CROSS JOIN h WHERE j.owner_epoch=h.owner_epoch AND j.sequence>$1 AND j.sequence<=h.sequence ORDER BY j.sequence LIMIT $2) SELECT h.owner_epoch AS head_epoch,h.sequence AS head_sequence,p.sequence AS signal_sequence,p.signal_id,p.reason,p.user_id,p.peer_id FROM h LEFT JOIN p ON true ORDER BY p.sequence")
                .bind(query.after().sequence()).bind(i64::from(query.limit()))
                .fetch_all(&mut *connection).await.map_err(database_error)?;
            let first=rows.first().ok_or(ReadError::Unavailable)?;
            let head=decode_cursor(first.try_get("head_epoch").map_err(database_error)?,first.try_get("head_sequence").map_err(database_error)?)?;
            if head.epoch()!=query.after().epoch() || query.after().sequence()>head.sequence() {return Err(ReadError::FailedPrecondition);}
            let mut signals=Vec::with_capacity(rows.len());
            for row in rows {
                let sequence:Option<i64>=row.try_get("signal_sequence").map_err(database_error)?;
                if let Some(sequence)=sequence {
                    let reason:String=row.try_get("reason").map_err(database_error)?;
                    let user:uuid::Uuid=row.try_get("user_id").map_err(database_error)?;
                    let peer:Option<uuid::Uuid>=row.try_get("peer_id").map_err(database_error)?;
                    let (reason,scope)=match (reason.as_str(),peer) {
                        ("FRIEND_RELATIONSHIP_CHANGED",Some(second))=>(Reason::RelationshipChanged,Scope::Pair{first:user,second}),
                        ("FRIEND_USER_REMOVED",None)=>(Reason::UserRemoved,Scope::User{user}),
                        _=>return Err(ReadError::Unavailable),
                    };
                    signals.push(Signal {cursor:decode_cursor(head.epoch(),sequence)?,id:row.try_get("signal_id").map_err(database_error)?,reason,scope});
                }
            }
            let next=signals.last().map_or(query.after(),|s|s.cursor());
            let page=Page {query,head,next,signals};
            page.validate()?;
            page.to_wire()?;
            budget.check()?;
            sqlx::query("COMMIT").execute(&mut *connection).await.map_err(database_error)?;
            Ok(page)
        }).await
    }
}
async fn timeout(
    connection: &mut sqlx::PgConnection,
    budget: &ReadBudget,
) -> Result<(), ReadError> {
    let millis = budget.remaining()?.as_millis().max(1);
    sqlx::query("SELECT set_config('statement_timeout',$1,true)")
        .bind(format!("{millis}ms"))
        .execute(connection)
        .await
        .map_err(database_error)?;
    budget.check()
}
fn decode_cursor(epoch: uuid::Uuid, sequence: i64) -> Result<Cursor, ReadError> {
    Cursor::new(epoch, sequence).map_err(|_| ReadError::Unavailable)
}
fn database_error(error: sqlx::Error) -> ReadError {
    if error
        .as_database_error()
        .is_some_and(|e| e.code().as_deref() == Some("57014"))
    {
        ReadError::DeadlineExceeded
    } else {
        ReadError::Unavailable
    }
}
impl std::fmt::Debug for PgJournal {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PgJournal([REDACTED])")
    }
}
