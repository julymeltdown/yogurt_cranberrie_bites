//! BE39 durable saga. Only owned moderation ledger and BE15 receipt/outbox seams.
use super::{
    event_operations as ops,
    receipt::{CommandProducer, PgCommandRepository},
};
use crate::moderation_live::{ports::*, wire::change_value};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    admin::{
        moderation::{Change, OwnerStep},
        Action, Actor, AuthorityQuery, Role,
    },
    commands::*,
};
use prost::Message;
use serde_json::json;
use sqlx::{PgConnection, PgPool, Row};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use uuid::Uuid;
#[derive(Clone)]
pub struct PgModeration {
    pool: PgPool,
    owners: ModerationOwners,
    keys: Arc<FingerprintKeys>,
}
impl PgModeration {
    pub fn new(pool: PgPool, owners: ModerationOwners, keys: Arc<FingerprintKeys>) -> Self {
        Self { pool, owners, keys }
    }
    /// Identical replay returns the original receipt without calling an owner.
    pub async fn submit(
        &self,
        actor: Actor,
        key: Uuid,
        change: &Change,
        authority: &dyn ModerationAuthority,
    ) -> Result<SafeReceipt, CommandError> {
        let request = request(actor, key, change)?;
        let fresh = AtomicBool::new(false);
        let producer = Reservation {
            actor,
            change,
            authority,
            owners: &self.owners,
            fresh: &fresh,
        };
        let receipt = PgCommandRepository::new(self.pool.clone())
            .execute(&request, change.target(), &self.keys, &producer)
            .await?;
        if fresh.load(Ordering::Acquire) {
            self.advance(&receipt, change, authority).await?;
        }
        PgCommandRepository::new(self.pool.clone())
            .lookup(request.scope(), change.target(), &producer)
            .await?
            .ok_or(CommandError::Unavailable)
    }
    /// Explicit server continuation only. Unknown/expired keys never create work.
    /// Reacquires current authority and verifies the original HMAC/input/key.
    pub async fn resume(
        &self,
        actor: Actor,
        key: Uuid,
        change: &Change,
        authority: &dyn ModerationAuthority,
    ) -> Result<SafeReceipt, CommandError> {
        let request = request(actor, key, change)?;
        let fresh = AtomicBool::new(false);
        let producer = Reservation {
            actor,
            change,
            authority,
            owners: &self.owners,
            fresh: &fresh,
        };
        let store = PgCommandRepository::new(self.pool.clone());
        if store
            .lookup(request.scope(), change.target(), &producer)
            .await?
            .is_none()
        {
            return Err(CommandError::Denied);
        }
        let receipt = store
            .execute(&request, change.target(), &self.keys, &producer)
            .await?;
        if receipt.state == ReceiptState::Pending {
            self.advance(&receipt, change, authority).await?;
        }
        store
            .lookup(request.scope(), change.target(), &producer)
            .await?
            .ok_or(CommandError::Unavailable)
    }
    async fn advance(
        &self,
        receipt: &SafeReceipt,
        change: &Change,
        authority: &dyn ModerationAuthority,
    ) -> Result<(), CommandError> {
        if receipt.state != ReceiptState::Pending {
            return Ok(());
        }
        let actor = Actor {
            actor: receipt.scope.actor,
            event: receipt.scope.event,
        };
        for step in change.steps() {
            let intent = OwnerIntent {
                receipt: receipt.id,
                actor,
                target: change.target(),
                operation: change.action().operation().into(),
                step,
                expected_revision: change.revision(),
            };
            let mut tx = self.pool.begin().await.map_err(db)?;
            authorize(&mut tx, authority, actor, change.action(), change.target()).await?;
            let state:String=sqlx::query_scalar("select state from public.concert_command_receipts where receipt_id=$1 and expires_at>clock_timestamp() for update").bind(receipt.id).fetch_optional(&mut *tx).await.map_err(db)?.ok_or(CommandError::Expired)?;
            if state != "PENDING" {
                tx.commit().await.map_err(db)?;
                return Ok(());
            }
            let terminal:Option<String>=sqlx::query_scalar("select outcome from public.concert_moderation_owner_results where receipt_id=$1 and step=$2 and outcome in ('APPLIED','FAILED')").bind(receipt.id).bind(step.as_str()).fetch_optional(&mut *tx).await.map_err(db)?;
            if terminal.as_deref() == Some("APPLIED") {
                tx.commit().await.map_err(db)?;
                continue;
            }
            if terminal.as_deref() == Some("FAILED") {
                tx.commit().await.map_err(db)?;
                break;
            }
            let seen:bool=sqlx::query_scalar("select exists(select 1 from public.concert_moderation_owner_results where receipt_id=$1 and step=$2 and outcome='PENDING')").bind(receipt.id).bind(step.as_str()).fetch_one(&mut *tx).await.map_err(db)?;
            if !seen {
                sqlx::query("insert into public.concert_moderation_owner_results(result_id,receipt_id,step,intent_digest,outcome) values($1,$2,$3,$4,'PENDING')").bind(Uuid::new_v4()).bind(receipt.id).bind(step.as_str()).bind(intent.digest().as_slice()).execute(&mut *tx).await.map_err(db)?;
            }
            authority.check_disclosure().await?;
            finish(&mut tx).await?;
            tx.commit().await.map_err(db)?;
            // No external side effect runs inside a Concert database transaction.
            let port = self.owners.port(step)?;
            authority.check_disclosure().await?;
            let outcome = if seen {
                port.recover(authority, &intent, change).await
            } else {
                port.apply(authority, &intent, change).await
            };
            let outcome = match outcome {
                Ok(v) => v,
                Err(_) => return Ok(()),
            }; // lost/error ACK stays PENDING, never inferred FAILED/APPLIED
            let (kind, owner_id, revision, native, failure) = match outcome {
                OwnerOutcome::Pending { intent: actual } => {
                    if actual != intent {
                        return Err(CommandError::Unavailable);
                    };
                    return Ok(());
                }
                OwnerOutcome::Failed {
                    intent: actual,
                    reason,
                } => {
                    if actual != intent || !safe_failure(reason) {
                        return Err(CommandError::Unavailable);
                    };
                    ("FAILED", None, None, None, Some(reason.as_str()))
                }
                OwnerOutcome::Applied {
                    intent: actual,
                    owner_receipt,
                    revision,
                    transaction,
                } => {
                    use crate::adapters::out::relationship_authority::guarded::ProducerKind;
                    let producer = if step == OwnerStep::PostContent {
                        ProducerKind::Post
                    } else {
                        ProducerKind::Concert
                    };
                    if actual != intent
                        || owner_receipt.is_nil()
                        || !(1..=concert_domain::MAX_REVISION).contains(&revision)
                        || transaction.effect_binding_digest != intent.digest()
                        || transaction.producer != producer
                        || (step == change.steps()[0] && revision <= change.revision())
                    {
                        return Err(CommandError::Unavailable);
                    }
                    let identity = transaction
                        .wire()
                        .map_err(|_| CommandError::Unavailable)?
                        .encode_to_vec();
                    (
                        "APPLIED",
                        Some(owner_receipt),
                        Some(revision),
                        Some(identity),
                        None,
                    )
                }
            };
            let mut tx = self.pool.begin().await.map_err(db)?;
            authorize(&mut tx, authority, actor, change.action(), change.target()).await?;
            let state:String=sqlx::query_scalar("select state from public.concert_command_receipts where receipt_id=$1 and expires_at>clock_timestamp() for update").bind(receipt.id).fetch_optional(&mut *tx).await.map_err(db)?.ok_or(CommandError::Expired)?;
            if state != "PENDING" {
                tx.commit().await.map_err(db)?;
                return Ok(());
            }
            let prior=sqlx::query("select outcome,owner_receipt_id,owner_revision,native_identity,failure_reason from public.concert_moderation_owner_results where receipt_id=$1 and step=$2 and outcome in ('APPLIED','FAILED')").bind(receipt.id).bind(step.as_str()).fetch_optional(&mut *tx).await.map_err(db)?;
            if let Some(row) = prior {
                if row.try_get::<String, _>("outcome").map_err(db)? != kind
                    || row
                        .try_get::<Option<Uuid>, _>("owner_receipt_id")
                        .map_err(db)?
                        != owner_id
                    || row
                        .try_get::<Option<i64>, _>("owner_revision")
                        .map_err(db)?
                        != revision
                    || row
                        .try_get::<Option<Vec<u8>>, _>("native_identity")
                        .map_err(db)?
                        != native
                    || row
                        .try_get::<Option<String>, _>("failure_reason")
                        .map_err(db)?
                        .as_deref()
                        != failure
                {
                    return Err(CommandError::Unavailable);
                }
            } else {
                sqlx::query("insert into public.concert_moderation_owner_results(result_id,receipt_id,step,intent_digest,outcome,owner_receipt_id,owner_revision,native_identity,failure_reason) values($1,$2,$3,$4,$5,$6,$7,$8,$9)").bind(Uuid::new_v4()).bind(receipt.id).bind(step.as_str()).bind(intent.digest().as_slice()).bind(kind).bind(owner_id).bind(revision).bind(native).bind(failure).execute(&mut *tx).await.map_err(db)?;
            }
            authority.check_disclosure().await?;
            finish(&mut tx).await?;
            tx.commit().await.map_err(db)?;
            if kind == "FAILED" {
                break;
            }
        }
        self.finalize(receipt, change, authority).await
    }
    async fn finalize(
        &self,
        receipt: &SafeReceipt,
        change: &Change,
        authority: &dyn ModerationAuthority,
    ) -> Result<(), CommandError> {
        let actor = Actor {
            actor: receipt.scope.actor,
            event: receipt.scope.event,
        };
        let mut tx = self.pool.begin().await.map_err(db)?;
        authorize(&mut tx, authority, actor, change.action(), change.target()).await?;
        let state:String=sqlx::query_scalar("select state from public.concert_command_receipts where receipt_id=$1 and expires_at>clock_timestamp() for update").bind(receipt.id).fetch_optional(&mut *tx).await.map_err(db)?.ok_or(CommandError::Expired)?;
        if state != "PENDING" {
            tx.commit().await.map_err(db)?;
            return Ok(());
        }
        let failure:Option<String>=sqlx::query_scalar("select failure_reason from public.concert_moderation_owner_results where receipt_id=$1 and outcome='FAILED' order by created_at,result_id limit 1").bind(receipt.id).fetch_optional(&mut *tx).await.map_err(db)?;
        let rows=sqlx::query("select step,owner_revision from public.concert_moderation_owner_results where receipt_id=$1 and outcome='APPLIED'").bind(receipt.id).fetch_all(&mut *tx).await.map_err(db)?;
        let complete = change.steps().iter().all(|s| {
            rows.iter().any(|r| {
                r.try_get::<String, _>("step")
                    .is_ok_and(|x| x == s.as_str())
            })
        });
        let outcome = if let Some(reason) = failure {
            sqlx::query("update public.concert_command_receipts set state='FAILED',failure_reason=$2 where receipt_id=$1").bind(receipt.id).bind(reason).execute(&mut *tx).await.map_err(db)?;
            "FAILED"
        } else if complete {
            let first = change.steps()[0];
            let revision: i64 = rows
                .iter()
                .find(|r| {
                    r.try_get::<String, _>("step")
                        .is_ok_and(|s| s == first.as_str())
                })
                .ok_or(CommandError::InvalidState)?
                .try_get("owner_revision")
                .map_err(db)?;
            sqlx::query(
                "select singleton from public.concert_command_budget where singleton for update",
            )
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
            let queued: i64 = sqlx::query_scalar(
                "select count(*) from public.concert_outbox where state in ('READY','LEASED')",
            )
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
            if queued >= 10000 {
                return Err(CommandError::Capacity);
            }
            sqlx::query("update public.concert_command_receipts set state='SUCCEEDED',resource_id=$2,resource_revision=$3,result_status=200 where receipt_id=$1").bind(receipt.id).bind(change.target()).bind(revision).execute(&mut *tx).await.map_err(db)?;
            sqlx::query("insert into public.concert_outbox(outbox_id,receipt_id,event_id,resource_id,resource_revision) values($1,$2,$3,$4,$5)").bind(Uuid::new_v4()).bind(receipt.id).bind(actor.event).bind(change.target()).bind(revision).execute(&mut *tx).await.map_err(db)?;
            "SUCCEEDED"
        } else {
            finish(&mut tx).await?;
            tx.commit().await.map_err(db)?;
            return Ok(());
        };
        insert_audit(&mut tx, receipt.id, actor, change, outcome).await?;
        authority.check_disclosure().await?;
        finish(&mut tx).await?;
        tx.commit().await.map_err(db)?;
        Ok(())
    }
    pub async fn audit_page(
        &self,
        actor: Actor,
        authority: &dyn ModerationAuthority,
        query: &crate::moderation_live::wire::Query,
        after: Option<&str>,
        snapshot: DateTime<Utc>,
    ) -> Result<Vec<serde_json::Value>, CommandError> {
        let after = after
            .map(|s| {
                s.split_once('/')
                    .ok_or(CommandError::InvalidInput)
                    .and_then(|(t, id)| {
                        Ok((
                            DateTime::parse_from_rfc3339(t)
                                .map_err(|_| CommandError::InvalidInput)?
                                .with_timezone(&Utc),
                            Uuid::parse_str(id).map_err(|_| CommandError::InvalidInput)?,
                        ))
                    })
            })
            .transpose()?;
        let mut tx = self.pool.begin().await.map_err(db)?;
        authorize(&mut tx, authority, actor, Action::AuditRead, actor.event).await?;
        let filter = |k: &str| query.filters.get(k).cloned();
        let rows=sqlx::query("select audit_id,event_id,actor_id,operation_id,target_id,outcome,receipt_id,created_at from (select audit_id,event_id,actor_id,operation_id,target_id,outcome,receipt_id,created_at from public.concert_moderation_audit union all select audit_id,event_id,actor_id,operation_id,target_id,outcome,receipt_id,created_at from public.concert_operations_audit where receipt_id is not null) audit where event_id=$1 and created_at<=$2 and ($3::timestamptz is null or (created_at,audit_id)<($3,$4)) and ($5::text is null or actor_id::text=$5) and ($6::text is null or operation_id=$6) and ($7::text is null or outcome=$7) and ($8::timestamptz is null or created_at>=$8) and ($9::timestamptz is null or created_at<$9) order by created_at desc,audit_id desc limit $10")
            .bind(actor.event).bind(snapshot).bind(after.map(|v|v.0)).bind(after.map(|v|v.1)).bind(filter("actor_id")).bind(filter("operation_id")).bind(filter("outcome")).bind(filter("from").map(|v|DateTime::parse_from_rfc3339(&v).map(|d|d.with_timezone(&Utc))).transpose().map_err(|_|CommandError::InvalidInput)?).bind(filter("until").map(|v|DateTime::parse_from_rfc3339(&v).map(|d|d.with_timezone(&Utc))).transpose().map_err(|_|CommandError::InvalidInput)?).bind(i64::from(query.limit)+1).fetch_all(&mut *tx).await.map_err(db)?;
        let mut values = Vec::new();
        for row in rows {
            values.push(json!({"audit_id":row.try_get::<Uuid,_>("audit_id").map_err(db)?,"event_id":row.try_get::<Uuid,_>("event_id").map_err(db)?,"actor_id":row.try_get::<Uuid,_>("actor_id").map_err(db)?,"operation_id":row.try_get::<String,_>("operation_id").map_err(db)?,"target_id":row.try_get::<Option<Uuid>,_>("target_id").map_err(db)?,"outcome":row.try_get::<String,_>("outcome").map_err(db)?,"receipt_id":row.try_get::<Uuid,_>("receipt_id").map_err(db)?,"created_at":row.try_get::<DateTime<Utc>,_>("created_at").map_err(db)?}));
        }
        authority.check_disclosure().await?;
        finish(&mut tx).await?;
        tx.commit().await.map_err(db)?;
        Ok(values)
    }
}
struct Reservation<'a> {
    actor: Actor,
    change: &'a Change,
    authority: &'a dyn ModerationAuthority,
    owners: &'a ModerationOwners,
    fresh: &'a AtomicBool,
}
#[async_trait]
impl CommandProducer for Reservation<'_> {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if scope.actor != self.actor.actor
            || scope.event != self.actor.event
            || scope.operation.id() != self.change.action().operation()
            || target != self.change.target()
        {
            return Err(CommandError::Denied);
        };
        authorize(c, self.authority, self.actor, self.change.action(), target).await
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        self.change.validate(self.actor, ops::now(c).await?)?;
        for step in self.change.steps() {
            self.owners.port(step)?;
        }
        let receipt:Uuid=sqlx::query_scalar("select receipt_id from public.concert_command_receipts where actor_id=$1 and event_id=$2 and operation_id=$3 and idempotency_key=$4").bind(scope.actor).bind(scope.event).bind(scope.operation.id()).bind(scope.key).fetch_one(&mut *c).await.map_err(db)?;
        sqlx::query("insert into public.concert_moderation_commands(receipt_id,event_id,actor_id,target_id,operation_id,expected_revision,input,steps) values($1,$2,$3,$4,$5,$6,$7,$8)").bind(receipt).bind(scope.event).bind(scope.actor).bind(target).bind(scope.operation.id()).bind(self.change.revision()).bind(change_value(self.change)).bind(self.change.steps().iter().map(|s|s.as_str()).collect::<Vec<_>>()).execute(&mut *c).await.map_err(db)?;
        insert_audit(c, receipt, self.actor, self.change, "PENDING").await?;
        self.fresh.store(true, Ordering::Release);
        Ok((Completion::Pending, None))
    }
}
pub async fn authorize(
    c: &mut PgConnection,
    a: &dyn ModerationAuthority,
    actor: Actor,
    action: Action,
    target: Uuid,
) -> Result<(), CommandError> {
    if actor.actor.is_nil() || actor.event.is_nil() || target.is_nil() {
        return Err(CommandError::Denied);
    }
    sqlx::query("select event_id from public.concert_events where event_id=$1 for update")
        .bind(actor.event)
        .fetch_optional(&mut *c)
        .await
        .map_err(db)?
        .ok_or(CommandError::Denied)?;
    let started = ops::now(c).await?;
    let q = AuthorityQuery {
        actor,
        action,
        target,
    };
    let lease = a.check_in(c, q, started).await?;
    lease.validate(q, started, ops::now(c).await?)?;
    let mut until = lease.expires_at;
    if action == Action::ModerateUser {
        let at = ops::now(c).await?;
        let peer = a.target_role_in(c, actor, target, at).await?;
        let end = ops::now(c).await?;
        if peer.event != actor.event
            || peer.target != target
            || !(1..=concert_domain::MAX_REVISION).contains(&peer.revision)
            || peer.checked_at < at
            || peer.checked_at > end
            || peer.expires_at <= end
        {
            return Err(CommandError::Unavailable);
        }
        if lease.role != Role::Operator && peer.role == Role::Operator {
            return Err(CommandError::Denied);
        }
        until = until.min(peer.expires_at);
    }
    a.check_disclosure().await?;
    ops::pin(c, until).await?;
    sqlx::query("select set_config('mcr.moderation_authority_until',least(coalesce(nullif(current_setting('mcr.moderation_authority_until',true),''),'infinity')::timestamptz,$1)::text,true)").bind(until).execute(&mut *c).await.map_err(db)?;
    Ok(())
}
async fn finish(c: &mut PgConnection) -> Result<(), CommandError> {
    let valid:bool=sqlx::query_scalar("select clock_timestamp()<nullif(current_setting('mcr.moderation_authority_until',true),'')::timestamptz").fetch_one(c).await.map_err(db)?;
    if valid {
        Ok(())
    } else {
        Err(CommandError::Unavailable)
    }
}
async fn insert_audit(
    c: &mut PgConnection,
    receipt: Uuid,
    actor: Actor,
    change: &Change,
    outcome: &str,
) -> Result<(), CommandError> {
    sqlx::query("insert into public.concert_moderation_audit(audit_id,receipt_id,event_id,actor_id,target_id,operation_id,outcome) values($1,$2,$3,$4,$5,$6,$7)").bind(Uuid::new_v4()).bind(receipt).bind(actor.event).bind(actor.actor).bind(change.target()).bind(change.action().operation()).bind(outcome).execute(c).await.map_err(db)?;
    Ok(())
}
fn request(actor: Actor, key: Uuid, c: &Change) -> Result<CanonicalRequest, CommandError> {
    let event = actor.event.to_string();
    let target = c.target().to_string();
    let mut params = vec![("eventId", event.as_str())];
    if matches!(c, Change::User { .. }) {
        params.push(("userId", target.as_str()));
    }
    CanonicalRequest::json(
        Scope {
            actor: actor.actor,
            event: actor.event,
            operation: RegisteredOperation::find(c.action().operation())?,
            key,
        },
        &params,
        &[],
        Some(&c.canonical()),
    )
}
fn safe_failure(f: SafeFailure) -> bool {
    matches!(
        f,
        SafeFailure::InvalidTransition
            | SafeFailure::VersionConflict
            | SafeFailure::DependencyUnavailable
            | SafeFailure::InternalError
            | SafeFailure::ResourceUnavailable
            | SafeFailure::InvalidInput
            | SafeFailure::EventReadOnly
    )
}
fn db(_: impl std::fmt::Display) -> CommandError {
    CommandError::Unavailable
}
