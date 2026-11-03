//! BE36 private reports. No target/public API can return the privileged projection.
use super::{
    event_operations::{self as ops, FencedOperationsAuthority},
    receipt::{CommandProducer, PgCommandRepository},
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{admin::*, commands::*, report::ReportFilter};
use concert_domain::{report::*, EventPhase};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

/// The target owner must verify current target access OR qualified evidence of a
/// recent target; media owner must verify every attachment is READY, REPORT purpose,
/// actor-owned and event-scoped. Bind exact input; retain all revocation/media/target
/// revisions until transaction commit. This is not a client accessibility flag.
#[async_trait]
pub trait FencedReportEvidence: Send + Sync {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        actor: Actor,
        input: &CreateReport,
        now: DateTime<Utc>,
    ) -> Result<EvidenceLease, CommandError>;
}
/// A mismatched or stale owner response is unavailable, never an authorization grant.
#[derive(Clone)]
pub struct EvidenceLease {
    pub actor: Actor,
    pub input_digest: [u8; 32],
    pub revision: i64,
    pub checked_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
}
pub fn evidence_digest(input: &CreateReport) -> Result<[u8; 32], CommandError> {
    Ok(Sha256::digest(serde_json::to_vec(input).map_err(|_| CommandError::InvalidInput)?).into())
}
impl EvidenceLease {
    fn validate(
        &self,
        actor: Actor,
        input: &CreateReport,
        started: DateTime<Utc>,
        finished: DateTime<Utc>,
    ) -> Result<(), CommandError> {
        if self.actor != actor
            || self.input_digest != evidence_digest(input)?
            || !(1..=concert_domain::MAX_REVISION).contains(&self.revision)
            || self.checked_at < started
            || self.checked_at > finished
            || self.expires_at <= finished
        {
            Err(CommandError::Unavailable)
        } else {
            Ok(())
        }
    }
}
pub struct UnavailableEvidence;
#[async_trait]
impl FencedReportEvidence for UnavailableEvidence {
    async fn check_in(
        &self,
        _: &mut PgConnection,
        _: Actor,
        _: &CreateReport,
        _: DateTime<Utc>,
    ) -> Result<EvidenceLease, CommandError> {
        Err(CommandError::Unavailable)
    }
}
pub struct PgReports<A, E> {
    pool: PgPool,
    authority: A,
    evidence: E,
}
impl<A: FencedOperationsAuthority, E: FencedReportEvidence> PgReports<A, E> {
    pub fn new(pool: PgPool, authority: A, evidence: E) -> Self {
        Self {
            pool,
            authority,
            evidence,
        }
    }
    pub async fn create(
        &self,
        actor: Actor,
        key: Uuid,
        input: CreateReport,
        keys: &FingerprintKeys,
    ) -> Result<SafeReceipt, CommandError> {
        input.validate().map_err(|_| CommandError::InvalidInput)?;
        let target = ops::stable_id(actor, key, Action::CreateReport);
        let body = serde_json::to_value(&input).map_err(|_| CommandError::InvalidInput)?;
        let CanonicalValue::Object(body) = ops::canonical_json(body)? else {
            return Err(CommandError::InvalidInput);
        };
        let request = ops::request(actor, key, Action::CreateReport, target, body)?;
        let producer = ReportProducer {
            repository: self,
            actor,
            target,
            input,
        };
        PgCommandRepository::new(self.pool.clone())
            .execute(&request, target, keys, &producer)
            .await
    }
    /// Safe own status only. Even the reporter cannot retrieve evidence via this method.
    pub async fn own(
        &self,
        actor: Actor,
        filter: ReportFilter,
    ) -> Result<Vec<Report>, CommandError> {
        if filter.target_type.is_some() {
            return Err(CommandError::InvalidInput);
        }
        self.list(actor, filter, false).await
    }
    pub async fn queue(
        &self,
        actor: Actor,
        filter: ReportFilter,
    ) -> Result<Vec<Report>, CommandError> {
        self.list(actor, filter, true).await
    }
    async fn list(
        &self,
        actor: Actor,
        filter: ReportFilter,
        privileged: bool,
    ) -> Result<Vec<Report>, CommandError> {
        let filter = filter.validate()?;
        let action = if privileged {
            Action::ReviewReports
        } else {
            Action::OwnReports
        };
        let mut tx = self.pool.begin().await.map_err(ops::unavailable)?;
        ops::authorize(
            &mut tx,
            &self.authority,
            AuthorityQuery {
                actor,
                action,
                target: actor.event,
            },
            false,
        )
        .await?;
        // Select only safe columns. Reporter/evidence cannot be accidentally serialized here.
        let rows=sqlx::query("select report_id,event_id,target_type,target_id,state,revision,created_at from concert_reports where event_id=$1 and ($2::uuid is null or reporter_id=$2) and ($3::text is null or state=$3) and ($4::text is null or target_type=$4) and ($5::uuid is null or report_id>$5) order by report_id limit $6")
   .bind(actor.event).bind(if privileged{None}else{Some(actor.actor)}).bind(filter.state.map(ReportState::as_str)).bind(filter.target_type.map(TargetType::as_str)).bind(filter.after).bind(i64::from(filter.limit)).fetch_all(&mut *tx).await.map_err(ops::unavailable)?;
        let result = rows.iter().map(safe).collect::<Result<_, _>>()?;
        if privileged {
            ops::audit(&mut tx, actor, action, actor.event, None, json!({})).await?;
        }
        ops::finish_read(&mut tx).await?;
        tx.commit().await.map_err(ops::unavailable)?;
        Ok(result)
    }
    pub async fn detail(
        &self,
        actor: Actor,
        id: Uuid,
    ) -> Result<Option<ReportDetail>, CommandError> {
        let mut tx = self.pool.begin().await.map_err(ops::unavailable)?;
        ops::authorize(
            &mut tx,
            &self.authority,
            AuthorityQuery {
                actor,
                action: Action::ReportDetail,
                target: id,
            },
            false,
        )
        .await?;
        let row = sqlx::query(
            "select * from concert_reports where event_id=$1 and report_id=$2 for share",
        )
        .bind(actor.event)
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(ops::unavailable)?;
        let result = row
            .as_ref()
            .map(|r| {
                Ok(ReportDetail {
                    report: safe(r)?,
                    reporter_id: r.try_get("reporter_id").map_err(ops::unavailable)?,
                    reason: Reason::parse(
                        &r.try_get::<String, _>("reason").map_err(ops::unavailable)?,
                    )
                    .map_err(|_| CommandError::InvalidState)?,
                    text: r.try_get("text").map_err(ops::unavailable)?,
                    evidence_media_ids: r
                        .try_get("evidence_media_ids")
                        .map_err(ops::unavailable)?,
                })
            })
            .transpose()?;
        // Every authorized evidence read (including a missing scoped ID) is audited in
        // the same transaction; audit failure prevents returning any privileged content.
        ops::audit(
            &mut tx,
            actor,
            Action::ReportDetail,
            id,
            None,
            json!({"found":result.is_some()}),
        )
        .await?;
        ops::finish_read(&mut tx).await?;
        tx.commit().await.map_err(ops::unavailable)?;
        Ok(result)
    }
}
fn safe(r: &sqlx::postgres::PgRow) -> Result<Report, CommandError> {
    let revision: i64 = r.try_get("revision").map_err(ops::unavailable)?;
    concert_domain::Revision::new(revision).map_err(|_| CommandError::InvalidState)?;
    Ok(Report {
        report_id: r.try_get("report_id").map_err(ops::unavailable)?,
        event_id: r.try_get("event_id").map_err(ops::unavailable)?,
        target_type: TargetType::parse(
            &r.try_get::<String, _>("target_type")
                .map_err(ops::unavailable)?,
        )
        .map_err(|_| CommandError::InvalidState)?,
        target_id: r.try_get("target_id").map_err(ops::unavailable)?,
        state: ReportState::parse(&r.try_get::<String, _>("state").map_err(ops::unavailable)?)
            .map_err(|_| CommandError::InvalidState)?,
        revision,
        created_at: r.try_get("created_at").map_err(ops::unavailable)?,
    })
}
struct ReportProducer<'a, A, E> {
    repository: &'a PgReports<A, E>,
    actor: Actor,
    target: Uuid,
    input: CreateReport,
}
#[async_trait]
impl<A: FencedOperationsAuthority, E: FencedReportEvidence> CommandProducer
    for ReportProducer<'_, A, E>
{
    async fn authorize(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if s.actor != self.actor.actor
            || s.event != self.actor.event
            || target != self.target
            || s.operation.id() != Action::CreateReport.operation()
        {
            return Err(CommandError::Denied);
        }
        ops::authorize(
            c,
            &self.repository.authority,
            AuthorityQuery {
                actor: self.actor,
                action: Action::CreateReport,
                target,
            },
            true,
        )
        .await?;
        let started = ops::now(c).await?;
        let lease = self
            .repository
            .evidence
            .check_in(c, self.actor, &self.input, started)
            .await?;
        lease.validate(self.actor, &self.input, started, ops::now(c).await?)?;
        ops::pin(c, lease.expires_at).await?;
        Ok(())
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        let phase: String =
            sqlx::query_scalar("select phase from concert_events where event_id=$1")
                .bind(s.event)
                .fetch_one(&mut *c)
                .await
                .map_err(ops::unavailable)?;
        // SAFETY remains available during READ_ONLY, but not draft/archived.
        let phase: EventPhase = phase.parse().map_err(|_| CommandError::InvalidState)?;
        if matches!(phase, EventPhase::Draft | EventPhase::Archived) {
            return Ok(ops::failure(SafeFailure::EventReadOnly));
        }
        let duplicate:bool=sqlx::query_scalar("select exists(select 1 from concert_reports where event_id=$1 and reporter_id=$2 and target_type=$3 and target_id=$4 and state in ('OPEN','IN_REVIEW'))").bind(s.event).bind(s.actor).bind(self.input.target_type.as_str()).bind(self.input.target_id).fetch_one(&mut *c).await.map_err(ops::unavailable)?;
        if duplicate {
            return Ok(ops::failure(SafeFailure::ResourceUnavailable));
        }
        // Fixed server quota, serialized by the event/BE15 locks. No rejected input or
        // private report text is logged or used in public errors.
        let recent:i64=sqlx::query_scalar("select count(*) from concert_reports where event_id=$1 and reporter_id=$2 and created_at>clock_timestamp()-interval '1 hour'").bind(s.event).bind(s.actor).fetch_one(&mut *c).await.map_err(ops::unavailable)?;
        if recent >= 10 {
            return Err(CommandError::Capacity);
        }
        sqlx::query("insert into concert_reports(report_id,event_id,reporter_id,target_type,target_id,reason,text,evidence_media_ids) values($1,$2,$3,$4,$5,$6,$7,$8)").bind(target).bind(s.event).bind(s.actor).bind(self.input.target_type.as_str()).bind(self.input.target_id).bind(self.input.reason.as_str()).bind(&self.input.text).bind(&self.input.evidence_media_ids).execute(&mut *c).await.map_err(ops::unavailable)?;
        ops::audit(
            c,
            self.actor,
            Action::CreateReport,
            target,
            Some(s.key),
            json!({"state":"OPEN"}),
        )
        .await?;
        Ok(ops::success(target, 1))
    }
}
