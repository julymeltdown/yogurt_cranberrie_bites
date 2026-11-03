//! One uncached, parameterized owner snapshot; no writes or portable admission proof.
use async_trait::async_trait;
use profile_application::concert_identity::{
    ConcertProfileError, ConcertProfileQuery, ConcertProfileRecord, ConcertProfileRepository,
};
use sqlx::{PgPool, Row};

#[derive(Clone)]
pub struct PgConcertProfileRepository {
    pool: PgPool,
}
impl PgConcertProfileRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
#[async_trait]
impl ConcertProfileRepository for PgConcertProfileRepository {
    async fn find_self_content(
        &self,
        query: ConcertProfileQuery,
    ) -> Result<Option<ConcertProfileRecord>, ConcertProfileError> {
        // Statement snapshot covers both current content and the owning deletion fence.
        // Bound transfer before decoding: UTF-8 uses at most four bytes per Unicode scalar.
        // Character limits also exclude oversized ASCII without truncating stored content.
        // Private profiles remain readable by their authenticated self producer. Auth is
        // enforced by the future RPC, not by copying this selector into an actor proof.
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
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| ConcertProfileError::DependencyUnavailable)?;
        row.map(|row| {
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
        .transpose()
    }
}
