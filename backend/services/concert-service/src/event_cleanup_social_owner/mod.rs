//! Bounded canonical Social revocation on CleanupRuntime's native transaction.
//! No pool, authority provider, policy default, handle reveal or READ_ONLY effect.
use crate::event_cleanup_live::LocalOwner;
use async_trait::async_trait;
use concert_application::{commands::CommandError, event_cleanup::*};
use concert_domain::event_lifecycle::{Job, JobState, Retention, Step};
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgTransactionManager, PgConnection, Row, TransactionManager};
use uuid::Uuid;

pub struct SocialGrantsOwner;

// Keyset pagination includes terminal tombstones. A grant is one resource, not
// an entire request with an unbounded cascade. The final byte encodes platform.
const SOURCE: &str = "select decode('01','hex') || uuid_send(request_id) || uuid_send(owner_id) || uuid_send(viewer_id) || decode(case platform when 'INSTAGRAM' then '01' else '02' end,'hex') k, revision, state from public.concert_social_grants where event_id=$1 union all select decode('02','hex') || uuid_send(request_id) k, revision, state from public.concert_social_requests where event_id=$1";

struct Resource {
    key: Vec<u8>,
    revision: i64,
    state: String,
}
struct Page {
    rows: Vec<Resource>,
    more: bool,
}

#[async_trait]
impl LocalOwner for SocialGrantsOwner {
    fn owner(&self) -> Owner {
        Owner::SocialGrants
    }

    async fn apply_in(
        &self,
        c: &mut PgConnection,
        request: &BatchRequest,
    ) -> Result<Outcome, CommandError> {
        if scope_in(c, request).await? {
            // Complete replay belongs to verify_in, never apply a second effect.
            return Err(CommandError::InvalidState);
        }
        let page = page_in(c, request).await?;
        for row in &page.rows {
            if !live(&row.state) {
                continue;
            }
            if row.revision >= concert_domain::MAX_REVISION {
                return Err(CommandError::InvalidState);
            }
            let id = uuid(&row.key[1..17])?;
            let query = if row.key[0] == 1 {
                sqlx::query("update public.concert_social_grants set state='REVOKED',revision=revision+1 where event_id=$1 and request_id=$2 and owner_id=$3 and viewer_id=$4 and platform=$5 and state='ACTIVE' and revision=$6")
                    .bind(request.scope().event).bind(id)
                    .bind(uuid(&row.key[17..33])?).bind(uuid(&row.key[33..49])?)
                    .bind(if row.key[49] == 1 { "INSTAGRAM" } else { "X" })
                    .bind(row.revision)
            } else {
                sqlx::query("update public.concert_social_requests set state='REVOKED',revision=revision+1 where event_id=$1 and request_id=$2 and state in ('PENDING','APPROVED') and revision=$3")
                    .bind(request.scope().event).bind(id).bind(row.revision)
            };
            if query.execute(&mut *c).await.map_err(db)?.rows_affected() != 1 {
                return Err(CommandError::InvalidState);
            }
        }
        let checked = page_in(c, request).await?;
        let receipt = checked.receipt(request)?;
        self.verify_in(c, request, receipt).await?;
        if checked.more {
            let next = checked.rows.last().ok_or(CommandError::InvalidState)?;
            Ok(Outcome::Page {
                receipt,
                next: Cursor::new(next.key.clone()).map_err(invalid)?,
            })
        } else {
            Ok(Outcome::Converged { receipt })
        }
    }

    async fn verify_in(
        &self,
        c: &mut PgConnection,
        request: &BatchRequest,
        receipt: Receipt,
    ) -> Result<(), CommandError> {
        let complete = scope_in(c, request).await?;
        let page = page_in(c, request).await?;
        if page.receipt(request)? != receipt {
            return Err(CommandError::InvalidState);
        }
        // Every earlier key must remain terminal. At convergence this covers the
        // entire event, including any unexpected live row behind the cursor.
        let sql = format!("select exists(select 1 from ({SOURCE}) s where state in ('ACTIVE','PENDING','APPROVED') and ($2::bytea is null or k<=$2))");
        let end = if page.more {
            Some(
                page.rows
                    .last()
                    .ok_or(CommandError::InvalidState)?
                    .key
                    .as_slice(),
            )
        } else {
            None
        };
        let unfinished: bool = sqlx::query_scalar(&sql)
            .bind(request.scope().event)
            .bind(end)
            .fetch_one(&mut *c)
            .await
            .map_err(db)?;
        if unfinished {
            return Err(CommandError::InvalidState);
        }
        // First verification precedes append. On terminal replay require the
        // exact immutable engine row as well as the actual source postimage.
        let prior = sqlx::query("select * from public.concert_event_cleanup_batches where job_id=$1 and step='EXPIRE_GRANTS' and owner='SOCIAL_GRANTS' and ordinal=$2 for share")
            .bind(request.scope().job).bind(request.ordinal() as i64)
            .fetch_optional(&mut *c).await.map_err(db)?;
        if let Some(row) = prior {
            if !complete
                || page.more
                || row.try_get::<Uuid, _>("event_id").map_err(db)? != request.scope().event
                || row.try_get::<Uuid, _>("actor_id").map_err(db)? != request.scope().actor
                || row.try_get::<Uuid, _>("operation_id").map_err(db)? != receipt.operation
                || row.try_get::<Vec<u8>, _>("binding").map_err(db)? != receipt.binding
                || row.try_get::<Uuid, _>("receipt_id").map_err(db)? != receipt.id
                || row.try_get::<i64, _>("owner_revision").map_err(db)? != receipt.revision
                || row.try_get::<i16, _>("processed").map_err(db)? != receipt.processed as i16
                || row.try_get::<&str, _>("outcome").map_err(db)? != "CONVERGED"
                || row
                    .try_get::<Option<Vec<u8>>, _>("cursor_in")
                    .map_err(db)?
                    .as_deref()
                    != request.cursor().map(Cursor::bytes)
                || row
                    .try_get::<Option<Vec<u8>>, _>("cursor_out")
                    .map_err(db)?
                    .is_some()
            {
                return Err(CommandError::InvalidState);
            }
        } else if complete {
            return Err(CommandError::InvalidState);
        }
        deadlines(c).await
    }
}

fn live(state: &str) -> bool {
    matches!(state, "ACTIVE" | "PENDING" | "APPROVED")
}
fn valid_key(key: &[u8]) -> bool {
    matches!(key, [2, ..] if key.len()==17)
        || matches!(key, [1, ..] if key.len()==50 && matches!(key[49],1|2))
}
fn uuid(bytes: &[u8]) -> Result<Uuid, CommandError> {
    Uuid::from_slice(bytes).map_err(invalid)
}
fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
fn invalid<T>(_: T) -> CommandError {
    CommandError::InvalidState
}

impl Page {
    fn receipt(&self, r: &BatchRequest) -> Result<Receipt, CommandError> {
        let mut hash = Sha256::new();
        hash.update(b"mcr-social-closure-postimage-v1\0");
        hash.update(r.binding());
        hash.update([u8::from(self.more)]);
        hash.update((self.rows.len() as u16).to_be_bytes());
        for row in &self.rows {
            if !valid_key(&row.key)
                || live(&row.state)
                || !(1..=concert_domain::MAX_REVISION).contains(&row.revision)
                || !matches!(
                    row.state.as_str(),
                    "REVOKED" | "EXPIRED" | "REJECTED" | "CANCELLED"
                )
            {
                return Err(CommandError::InvalidState);
            }
            hash.update([row.key.len() as u8]);
            hash.update(&row.key);
            hash.update(row.revision.to_be_bytes());
            hash.update(row.state.as_bytes());
            hash.update([0]);
        }
        let digest = hash.finalize();
        let mut id: [u8; 16] = digest[..16].try_into().map_err(invalid)?;
        id[6] = (id[6] & 15) | 0x50;
        id[8] = (id[8] & 63) | 0x80;
        Ok(Receipt {
            binding: r.binding(),
            operation: r.operation(),
            id: Uuid::from_bytes(id),
            // Receipt profile revision, not a fabricated aggregate Social revision.
            revision: 1,
            processed: self.rows.len() as u16,
        })
    }
}

async fn page_in(c: &mut PgConnection, r: &BatchRequest) -> Result<Page, CommandError> {
    let sql = format!("select k,revision,state from ({SOURCE}) s where ($2::bytea is null or k>$2) order by k limit $3");
    let rows = sqlx::query(&sql)
        .bind(r.scope().event)
        .bind(r.cursor().map(Cursor::bytes))
        .bind(i64::from(r.limit()) + 1)
        .fetch_all(c)
        .await
        .map_err(db)?;
    let more = rows.len() > usize::from(r.limit());
    let rows = rows
        .into_iter()
        .take(usize::from(r.limit()))
        .map(|row| {
            Ok(Resource {
                key: row.try_get("k").map_err(db)?,
                revision: row.try_get("revision").map_err(db)?,
                state: row.try_get("state").map_err(db)?,
            })
        })
        .collect::<Result<Vec<_>, CommandError>>()?;
    Ok(Page { rows, more })
}

async fn scope_in(c: &mut PgConnection, r: &BatchRequest) -> Result<bool, CommandError> {
    let s = r.scope();
    if PgTransactionManager::get_transaction_depth(c) == 0
        || s.owner != Owner::SocialGrants
        || s.step != Step::ExpireGrants
        || r.cursor().is_some_and(|v| !valid_key(v.bytes()))
    {
        return Err(CommandError::InvalidInput);
    }
    let phase: Option<String> =
        sqlx::query_scalar("select phase from public.concert_events where event_id=$1 for update")
            .bind(s.event)
            .fetch_optional(&mut *c)
            .await
            .map_err(db)?;
    if phase.as_deref() != Some("READ_ONLY") {
        return Err(CommandError::InvalidState);
    }
    let row = sqlx::query(
        "select * from public.concert_closure_jobs where event_id=$1 and job_id=$2 for update",
    )
    .bind(s.event)
    .bind(s.job)
    .fetch_optional(&mut *c)
    .await
    .map_err(db)?
    .ok_or(CommandError::Denied)?;
    let job: Job = serde_json::from_value(row.try_get("progress").map_err(db)?).map_err(invalid)?;
    job.validate().map_err(invalid)?;
    let retention = Retention {
        policy_revision: row.try_get("retention_revision").map_err(db)?,
        closed_at: row.try_get("closed_at").map_err(db)?,
        cleanup_after: row.try_get("cleanup_after").map_err(db)?,
    };
    if row.try_get::<Uuid, _>("actor_id").map_err(db)? != s.actor
        || job.job_id != s.job
        || job.event_id != s.event
        || job.revision != row.try_get::<i64, _>("revision").map_err(db)?
        || !matches!(job.state, JobState::Pending | JobState::Running)
        || job.next() != Some(Step::ExpireGrants)
        || retention != r.retention()
    {
        return Err(CommandError::InvalidState);
    }
    let head=sqlx::query("select * from public.concert_event_cleanup_progress where job_id=$1 and event_id=$2 and actor_id=$3 and step='EXPIRE_GRANTS' and owner='SOCIAL_GRANTS' for update")
        .bind(s.job).bind(s.event).bind(s.actor).fetch_optional(&mut *c).await.map_err(db)?.ok_or(CommandError::Unavailable)?;
    let state: &str = head.try_get("state").map_err(db)?;
    if head.try_get::<i16, _>("batch_limit").map_err(db)? != r.limit() as i16
        || head.try_get::<i64, _>("ordinal").map_err(db)? != r.ordinal() as i64
        || head
            .try_get::<Option<Vec<u8>>, _>("cursor")
            .map_err(db)?
            .as_deref()
            != r.cursor().map(Cursor::bytes)
        || !matches!(state, "PENDING" | "PARTIAL_FAILURE" | "COMPLETE")
    {
        return Err(CommandError::InvalidState);
    }
    deadlines(c).await?;
    Ok(state == "COMPLETE")
}

async fn deadlines(c: &mut PgConnection) -> Result<(), CommandError> {
    // Deadline GUCs are necessary commit guards, never privileged authority. The
    // trusted caller establishes current authority; this leaf cannot extend it.
    let valid:bool=sqlx::query_scalar("select coalesce(clock_timestamp()<nullif(current_setting('mcr.lifecycle_authority_until',true),'')::timestamptz,false) and coalesce(clock_timestamp()<nullif(current_setting('mcr.relationship_authority_until',true),'')::timestamptz,false)")
        .fetch_one(c).await.map_err(db)?;
    if valid {
        Ok(())
    } else {
        Err(CommandError::Unavailable)
    }
}
