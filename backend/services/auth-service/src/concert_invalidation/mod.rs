//! Auth-only durable invalidation hints. No network serving or authority grants.
use serde::Serialize;
use sqlx::{PgPool, Row};
use std::{fmt, time::Duration};
use uuid::Uuid;

pub const MAX_CURSOR: i64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayCursor {
    pub owner_epoch: Uuid,
    pub after: i64,
}

impl ReplayCursor {
    pub fn new(owner_epoch: Uuid, after: i64) -> Result<Self, ReplayError> {
        if owner_epoch.is_nil() || !(0..=MAX_CURSOR).contains(&after) {
            return Err(ReplayError::InvalidCursor);
        }
        Ok(Self { owner_epoch, after })
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum InvalidationKind {
    SessionChanged,
    UserAuthorityChanged,
    CredentialsChanged,
    UserRemoved,
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct InvalidationSignal {
    pub cursor: ReplayCursor,
    pub signal_id: Uuid,
    pub user_id: Uuid,
    pub family_id: Option<Uuid>,
    pub kind: InvalidationKind,
}

impl InvalidationSignal {
    /// Binding filter only. Matching a hint must invalidate/re-read current
    /// authority, never restore a revoked/expired lease or extend JWT expiry.
    pub fn affects(&self, user_id: Uuid, family_id: Uuid) -> bool {
        self.user_id == user_id && self.family_id.map_or(true, |family| family == family_id)
    }
}

#[derive(Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ReplayBatch {
    pub signals: Vec<InvalidationSignal>,
    pub next: ReplayCursor,
    pub high_water: ReplayCursor,
    pub has_more: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ReplayError {
    #[error("Invalid Auth invalidation cursor")]
    InvalidCursor,
    #[error("Auth invalidation owner epoch changed")]
    OwnerEpochMismatch,
    #[error("Invalid Auth invalidation batch size")]
    InvalidBatchSize,
    #[error("Auth invalidation journal unavailable")]
    Unavailable,
}

#[derive(Clone)]
pub struct PgInvalidationJournal {
    pool: PgPool,
    deadline: Duration,
}

impl PgInvalidationJournal {
    pub fn new(pool: PgPool, deadline: Duration) -> Result<Self, ReplayError> {
        if deadline.is_zero() || deadline > Duration::from_secs(5) {
            return Err(ReplayError::Unavailable);
        }
        Ok(Self { pool, deadline })
    }

    /// Owner-local bootstrap/checkpoint only; no acknowledgments. First fence old
    /// authority, then capture this baseline, reread current typed authority and
    /// apply intervening signals before becoming ready. Never skip cached authority.
    pub async fn checkpoint(&self) -> Result<ReplayCursor, ReplayError> {
        tokio::time::timeout(self.deadline, async {
            let mut tx = self.pool.begin().await.map_err(unavailable)?;
            sqlx::query("set transaction isolation level repeatable read read only")
                .execute(&mut *tx)
                .await
                .map_err(unavailable)?;
            set_timeout(&mut tx, self.deadline).await?;
            let head = read_head(&mut tx).await?;
            tx.commit().await.map_err(unavailable)?;
            Ok(head)
        })
        .await
        .map_err(|_| ReplayError::Unavailable)?
    }

    pub async fn replay(
        &self,
        after: ReplayCursor,
        limit: u16,
    ) -> Result<ReplayBatch, ReplayError> {
        ReplayCursor::new(after.owner_epoch, after.after)?;
        if !(1..=256).contains(&limit) {
            return Err(ReplayError::InvalidBatchSize);
        }
        tokio::time::timeout(self.deadline, async {
            let mut tx = self.pool.begin().await.map_err(unavailable)?;
            sqlx::query("set transaction isolation level repeatable read read only")
                .execute(&mut *tx).await.map_err(unavailable)?;
            set_timeout(&mut tx, self.deadline).await?;
            let head = read_head(&mut tx).await?;
            if after.owner_epoch != head.owner_epoch { return Err(ReplayError::OwnerEpochMismatch); }
            if after.after > head.after { return Err(ReplayError::InvalidCursor); }
            let rows = sqlx::query("select cursor,owner_epoch,signal_id,user_id,family_id,kind from public.auth_concert_invalidation_journal where cursor>$1 and cursor<=$2 order by cursor limit $3")
                .bind(after.after).bind(head.after).bind(i64::from(limit)).fetch_all(&mut *tx).await.map_err(unavailable)?;
            let mut expected = after.after;
            let mut signals = Vec::with_capacity(rows.len());
            for row in rows {
                expected += 1;
                let cursor = ReplayCursor::new(row.try_get("owner_epoch").map_err(unavailable)?, row.try_get("cursor").map_err(unavailable)?).map_err(|_| ReplayError::Unavailable)?;
                let kind = match row.try_get::<String,_>("kind").map_err(unavailable)?.as_str() {
                    "SESSION_CHANGED" => InvalidationKind::SessionChanged,
                    "USER_AUTHORITY_CHANGED" => InvalidationKind::UserAuthorityChanged,
                    "CREDENTIALS_CHANGED" => InvalidationKind::CredentialsChanged,
                    "USER_REMOVED" => InvalidationKind::UserRemoved,
                    _ => return Err(ReplayError::Unavailable),
                };
                let signal = InvalidationSignal {
                    cursor, signal_id: row.try_get("signal_id").map_err(unavailable)?,
                    user_id: row.try_get("user_id").map_err(unavailable)?,
                    family_id: row.try_get("family_id").map_err(unavailable)?, kind,
                };
                if cursor.owner_epoch != head.owner_epoch || cursor.after != expected || signal.signal_id.is_nil()
                    || signal.user_id.is_nil() || signal.family_id.is_some_and(|v| v.is_nil())
                    || (kind == InvalidationKind::SessionChanged) != signal.family_id.is_some() {
                    return Err(ReplayError::Unavailable);
                }
                signals.push(signal);
            }
            // No pruning exists in this increment: gaps or missing tail fail
            // closed rather than falsely advancing a consumer past unseen work.
            if signals.len() < usize::from(limit) && expected != head.after { return Err(ReplayError::Unavailable); }
            let next = ReplayCursor { owner_epoch: head.owner_epoch, after: expected };
            tx.commit().await.map_err(unavailable)?;
            Ok(ReplayBatch { signals, next, high_water: head, has_more: next.after < head.after })
        }).await.map_err(|_| ReplayError::Unavailable)?
    }
}

async fn set_timeout(c: &mut sqlx::PgConnection, deadline: Duration) -> Result<(), ReplayError> {
    sqlx::query("select set_config('statement_timeout',$1,true)")
        .bind(deadline.as_millis().max(1).to_string())
        .execute(c)
        .await
        .map_err(unavailable)?;
    Ok(())
}
async fn read_head(c: &mut sqlx::PgConnection) -> Result<ReplayCursor, ReplayError> {
    let row = sqlx::query("select h.owner_epoch,h.committed_cursor,exists(select 1 from public.auth_concert_invalidation_journal j where j.cursor=h.committed_cursor and j.owner_epoch=h.owner_epoch) as tail_exists from public.auth_concert_invalidation_head h where singleton")
        .fetch_optional(c).await.map_err(unavailable)?.ok_or(ReplayError::Unavailable)?;
    let head = ReplayCursor::new(
        row.try_get("owner_epoch").map_err(unavailable)?,
        row.try_get("committed_cursor").map_err(unavailable)?,
    )
    .map_err(|_| ReplayError::Unavailable)?;
    if head.after > 0 && !row.try_get::<bool, _>("tail_exists").map_err(unavailable)? {
        return Err(ReplayError::Unavailable);
    }
    Ok(head)
}
fn unavailable(_: sqlx::Error) -> ReplayError {
    ReplayError::Unavailable
}

macro_rules! redacted_debug {
    ($($ty:ty),+) => {$(impl fmt::Debug for $ty {
        fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result { f.write_str(concat!(stringify!($ty), "([REDACTED])")) }
    })+};
}
redacted_debug!(
    ReplayCursor,
    InvalidationSignal,
    ReplayBatch,
    PgInvalidationJournal
);
