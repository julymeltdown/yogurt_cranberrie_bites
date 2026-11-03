//! Immutable witness persistence. Lookup proves observed stage membership only;
//! the resource RPC must also refresh authoritative access state before signing.
use async_trait::async_trait;
use post_application::authority_witness::{
    AuthorityWitnessDto, AuthorityWitnessRepositoryPort, PurgeWitnessesDto, ResolveWitnessDto,
    WitnessCleanupOutcomeDto, WitnessErrorDto, WitnessMemberDto, WitnessParentDto,
    WitnessPurposeDto, WitnessQueryDto, WitnessRootDto, WitnessSubsetDto,
};
use sqlx::{PgPool, Postgres, Row, Transaction};

#[derive(Clone)]
pub struct PgAuthorityWitnessRepository {
    pool: PgPool,
}
impl PgAuthorityWitnessRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
fn unavailable(error: impl std::fmt::Display) -> WitnessErrorDto {
    tracing::warn!(error = %error, "authority witness storage failed");
    WitnessErrorDto::Unavailable
}
fn map_witness(row: &sqlx::postgres::PgRow) -> Result<AuthorityWitnessDto, WitnessErrorDto> {
    let array = |name| -> Result<[u8; 32], WitnessErrorDto> {
        row.try_get::<Vec<u8>, _>(name)
            .map_err(unavailable)?
            .try_into()
            .map_err(|_| WitnessErrorDto::Unavailable)
    };
    let created_at = row.try_get("created_at").map_err(unavailable)?;
    let parent = WitnessParentDto::from_admitted_request(
        array("parent_digest")?,
        array("request_digest")?,
        row.try_get("actor_id").map_err(unavailable)?,
        WitnessRootDto::parse(&row.try_get::<String, _>("root_kind").map_err(unavailable)?)?,
        row.try_get("root_resources").map_err(unavailable)?,
        row.try_get("parent_expires_at").map_err(unavailable)?,
        created_at,
    )?;
    let sequence: i64 = row.try_get("batch_sequence").map_err(unavailable)?;
    let query = WitnessQueryDto::new(
        row.try_get("stage_id").map_err(unavailable)?,
        WitnessPurposeDto::parse(&row.try_get::<String, _>("purpose").map_err(unavailable)?)?,
        array("query_digest")?,
        u32::try_from(sequence).map_err(unavailable)?,
    )?;
    let posts: Vec<uuid::Uuid> = row.try_get("post_ids").map_err(unavailable)?;
    let authors: Vec<uuid::Uuid> = row.try_get("author_ids").map_err(unavailable)?;
    if posts.len() != authors.len() {
        return Err(WitnessErrorDto::Unavailable);
    }
    let members = posts
        .into_iter()
        .zip(authors)
        .map(|(post, author)| WitnessMemberDto::from_owned_query(post, author))
        .collect::<Result<Vec<_>, _>>()?;
    AuthorityWitnessDto::capture(
        parent,
        query,
        members,
        row.try_get("missing_post_ids").map_err(unavailable)?,
        created_at,
    )
}
// Keep insertion and the post-wait check in one transaction, so a statement
// whose admission check preceded a lock wait cannot publish an expired stage.
async fn publish_transaction(
    transaction: &mut Transaction<'_, Postgres>,
    witness: &AuthorityWitnessDto,
) -> Result<(), WitnessErrorDto> {
    let parent = witness.parent();
    let query = witness.query();
    let posts: Vec<_> = witness
        .members()
        .iter()
        .map(WitnessMemberDto::post_id)
        .collect();
    let authors: Vec<_> = witness
        .members()
        .iter()
        .map(WitnessMemberDto::author_id)
        .collect();
    // One insert publishes the complete stage. A collision cannot append,
    // replace metadata, extend expiry or merge another query's membership.
    sqlx::query("insert into post_authority_resource_witnesses
            (stage_id,parent_digest,request_digest,actor_id,root_kind,root_resources,
             purpose,query_digest,batch_sequence,post_ids,author_ids,missing_post_ids,created_at,parent_expires_at)
            select $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14
            where $14 > clock_timestamp()
            on conflict(stage_id) do nothing")
            .bind(query.stage_id()).bind(parent.digest().as_slice()).bind(parent.request_digest().as_slice())
            .bind(parent.actor_id()).bind(parent.root().as_str()).bind(parent.resources())
            .bind(query.purpose().as_str()).bind(query.digest().as_slice()).bind(i64::from(query.sequence()))
            .bind(&posts).bind(&authors).bind(witness.missing()).bind(witness.created_at()).bind(parent.expires_at())
            .execute(&mut **transaction).await.map_err(unavailable)?;
    // Even a successful insertion may have waited for a unique-index lock.
    // Read a fresh database clock after the write before acknowledging it.
    let row = sqlx::query(
        "select * from post_authority_resource_witnesses
            where stage_id=$1 and parent_expires_at > clock_timestamp()",
    )
    .bind(query.stage_id())
    .fetch_optional(&mut **transaction)
    .await
    .map_err(unavailable)?
    .ok_or(WitnessErrorDto::MissingOrExpired)?;
    if witness.parent().expires_at() <= chrono::Utc::now() {
        return Err(WitnessErrorDto::MissingOrExpired);
    }
    if map_witness(&row)? == *witness {
        Ok(())
    } else {
        Err(WitnessErrorDto::StageConflict)
    }
}

#[async_trait]
impl AuthorityWitnessRepositoryPort for PgAuthorityWitnessRepository {
    async fn publish(&self, witness: &AuthorityWitnessDto) -> Result<(), WitnessErrorDto> {
        let mut transaction = self.pool.begin().await.map_err(unavailable)?;
        if let Err(error) = publish_transaction(&mut transaction, witness).await {
            transaction.rollback().await.map_err(unavailable)?;
            return Err(error);
        }
        transaction.commit().await.map_err(unavailable)?;
        // Network/commit waits cannot turn a live pre-commit check into a grant.
        // A row that expires during commit is already unusable by lookup.
        if witness.parent().expires_at() <= chrono::Utc::now() {
            return Err(WitnessErrorDto::MissingOrExpired);
        }
        Ok(())
    }
    async fn resolve(
        &self,
        request: &ResolveWitnessDto,
    ) -> Result<WitnessSubsetDto, WitnessErrorDto> {
        let row = sqlx::query(
            "select *, clock_timestamp() as observed_at
            from post_authority_resource_witnesses
            where stage_id=$1 and parent_digest=$2 and parent_expires_at>clock_timestamp()",
        )
        .bind(request.stage_id())
        .bind(request.parent_digest().as_slice())
        .fetch_optional(&self.pool)
        .await
        .map_err(unavailable)?
        .ok_or(WitnessErrorDto::MissingOrExpired)?;
        let observed_at = row.try_get("observed_at").map_err(unavailable)?;
        // Refresh after the await as well: pool/lock/network waits may outlive
        // the parent, even when the selected row was live at evaluation time.
        map_witness(&row)?.resolve(request, chrono::Utc::now().max(observed_at))
    }
    async fn resolve_current(
        &self,
        request: &ResolveWitnessDto,
    ) -> Result<WitnessSubsetDto, WitnessErrorDto> {
        let subset = self.resolve(request).await?;
        let ids: Vec<_> = subset
            .members()
            .iter()
            .map(WitnessMemberDto::post_id)
            .chain(subset.missing().iter().copied())
            .collect();
        let rows = sqlx::query(
            "select id,author_id from posts where id=any($1)
            and deleted_at is null and moderation_hidden_at is null",
        )
        .bind(&ids)
        .fetch_all(&self.pool)
        .await
        .map_err(unavailable)?;
        let live = rows
            .into_iter()
            .map(|row| {
                Ok((
                    row.try_get::<uuid::Uuid, _>("id").map_err(unavailable)?,
                    row.try_get::<uuid::Uuid, _>("author_id")
                        .map_err(unavailable)?,
                ))
            })
            .collect::<Result<std::collections::HashMap<_, _>, WitnessErrorDto>>()?;
        let database_now: chrono::DateTime<chrono::Utc> =
            sqlx::query_scalar("select clock_timestamp()")
                .fetch_one(&self.pool)
                .await
                .map_err(unavailable)?;
        if subset.expires_at() <= chrono::Utc::now().max(database_now) {
            return Err(WitnessErrorDto::MissingOrExpired);
        }
        if subset
            .members()
            .iter()
            .any(|row| live.get(&row.post_id()) != Some(&row.author_id()))
            || subset.missing().iter().any(|id| live.contains_key(id))
        {
            return Err(WitnessErrorDto::NotAdmitted);
        }
        Ok(subset)
    }
    async fn purge_expired(
        &self,
        request: &PurgeWitnessesDto,
    ) -> Result<WitnessCleanupOutcomeDto, WitnessErrorDto> {
        let result = sqlx::query(
            "delete from post_authority_resource_witnesses witness
            using (select stage_id from post_authority_resource_witnesses
                where parent_expires_at <= clock_timestamp()
                order by parent_expires_at,stage_id limit $1 for update skip locked) expired
            where witness.stage_id=expired.stage_id",
        )
        .bind(i64::from(request.limit()))
        .execute(&self.pool)
        .await
        .map_err(unavailable)?;
        WitnessCleanupOutcomeDto::from_affected_rows(result.rows_affected())
    }
}
