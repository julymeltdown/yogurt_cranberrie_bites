//! Retains local source exclusions through Root's actual response lifetime.
//! This child is not a producer effect transaction or a positive authority grant.
use super::protocol::SocialOperation;
use crate::adapters::out::relationship_authority::guarded::{
    ChildNativeIdentity, ChildQuery, ChildRecoveryReference, HeldChildGuard, OwnerGuardScope,
};
use chrono::{DateTime, Utc};
use concert_domain::relationship::social::Platform;
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool, Postgres, Row, Transaction};
use std::{sync::Arc, time::Duration};
use tonic::Status;
use uuid::Uuid;

/// Private source identities, never serialized into the Root ledger or a DTO.
#[derive(Clone)]
pub(super) enum SocialReadSources {
    Metadata { subjects: Vec<Uuid> },
    Reveal { request: Uuid, platform: Platform },
}

/// Exact closed Root ABI. No other child kind can substitute for it. A missing
/// configured recovery owner still refuses settlement. The query's subject is the enrolled actor; all source
/// identities remain only in the in-memory source query above.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(tag = "kind", deny_unknown_fields)]
enum SocialReadRecovery {
    #[serde(rename = "SOCIAL_READ")]
    SocialRead {
        query: ChildQuery,
        child_nonce: Uuid,
        native: ChildNativeIdentity,
        valid_until_unix_micros: i64,
    },
}
impl SocialReadRecovery {
    fn parts(&self) -> (&ChildQuery, Uuid, &ChildNativeIdentity, i64) {
        let Self::SocialRead {
            query,
            child_nonce,
            native,
            valid_until_unix_micros,
        } = self;
        (query, *child_nonce, native, *valid_until_unix_micros)
    }
    fn gid(&self) -> Result<String, Status> {
        let (query, nonce, native, until) = self.parts();
        if nonce.is_nil()
            || [
                query.actor,
                query.event,
                query.subject,
                query.family,
                query.request,
            ]
            .iter()
            .any(Uuid::is_nil)
            || query.subject != query.actor
            || until <= 0
            || until != query.expires_unix_micros
        {
            return Err(unavailable());
        }
        platform::actor_delegation::owner_binding::identity_digest(
            "CONCERT",
            &native.system_identifier,
            native.database_oid,
            &native.full_xid,
            native.backend_pid,
            native.backend_started_unix_micros,
            &query.binding_digest,
            native.producer_instance_id,
            &[],
        )?;
        Ok(format!("mcr-social-read:{}", nonce.simple()))
    }
    fn root_reference(&self) -> Result<ChildRecoveryReference, Status> {
        let Self::SocialRead {
            query,
            child_nonce,
            native,
            valid_until_unix_micros,
        } = self;
        let reference = ChildRecoveryReference::SocialRead {
            query: query.clone(),
            child_nonce: *child_nonce,
            native: native.clone(),
            valid_until_unix_micros: *valid_until_unix_micros,
        };
        if reference.kind_name() != "SOCIAL_READ"
            || reference.local_prepared_gid()? != self.gid()?
        {
            return Err(unavailable());
        }
        let roundtrip: Self =
            serde_json::from_value(serde_json::to_value(&reference).map_err(|_| unavailable())?)
                .map_err(|_| unavailable())?;
        if &roundtrip != self {
            return Err(unavailable());
        }
        Ok(reference)
    }
}

/// Configure on Root's real CompositeChildIntentSettler::with_social. This
/// adapter neither authenticates a ledger reference nor completes native effects;
/// Root must perform those steps before invoking exact read-child recovery.
pub struct SocialReadSettler {
    pool: PgPool,
}
impl SocialReadSettler {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}
#[async_trait::async_trait]
impl crate::product_rpc::child_settlement::SocialReadIntentSettler for SocialReadSettler {
    async fn recover_social_read(&self, reference: &ChildRecoveryReference) -> Result<(), Status> {
        recover_social_read(&self.pool, reference).await
    }
}

pub(super) struct SocialReadChild {
    pool: PgPool,
    recovery: SocialReadRecovery,
    reference: ChildRecoveryReference,
    deadline: AuthorityRpcDeadline,
}

/// Called only after the actual registered producer transaction has authorized
/// and projected the result, while that transaction still holds its sources.
pub(super) async fn retain_projected_in(
    pool: &PgPool,
    scope: &Arc<OwnerGuardScope>,
    projected: &mut PgConnection,
    operation: SocialOperation,
    sources: SocialReadSources,
    projected_until: Option<DateTime<Utc>>,
) -> Result<(), Status> {
    scope.binding().check()?;
    let binding = scope.binding();
    if binding.operation() != operation.id() {
        return Err(unavailable());
    }
    // The retained service pins owner/claim/birthday/Intro/consent validity into
    // this actual transaction. Never extend it by opening the child transaction.
    let until: Option<DateTime<Utc>> = sqlx::query_scalar(
        "select nullif(current_setting('mcr.relationship_authority_until',true),'')::timestamptz",
    )
    .fetch_one(&mut *projected)
    .await
    .map_err(|_| unavailable())?;
    let until = until.ok_or_else(unavailable)?;
    let until = projected_until.map_or(until, |value| value.min(until));
    // Conservative admission: Root's original body deadline must fit entirely
    // inside source validity. A shorter-lived grant requires a new request;
    // neither this module nor an unqualified SDK silently extends delivery.
    if until.timestamp_micros() < binding.deadline().expires_micros() {
        return Err(Status::deadline_exceeded(
            "Social source expires before delivery deadline",
        ));
    }
    let query = ChildQuery {
        actor: binding.actor(),
        event: binding.event(),
        subject: binding.actor(),
        family: binding.session_family(),
        request: binding.request_id(),
        expires_unix_micros: binding.deadline().expires_micros(),
        binding_digest: binding.digest(),
    };
    let mut tx = pool.begin().await.map_err(|_| unavailable())?;
    budget(&mut tx, binding.deadline()).await?;
    lock_sources(&mut tx, &query, sources).await?;
    capacity(&mut tx).await?;
    let native = ChildNativeIdentity::capture_concert_in(&mut tx, scope).await?;
    let recovery = SocialReadRecovery::SocialRead {
        query,
        child_nonce: Uuid::new_v4(),
        native,
        valid_until_unix_micros: binding.deadline().expires_micros(),
    };
    let reference = recovery.root_reference()?;
    reference.validate_for(binding)?;
    // Durable intent MUST precede PREPARE. Cancellation after this point leaves
    // recoverable exclusion, not an unrecorded prepared transaction.
    scope.register_child_intent(&reference).await?;
    budget(&mut tx, binding.deadline()).await?;
    let gid = recovery.gid()?;
    sqlx::query(&format!("prepare transaction '{gid}'"))
        .execute(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
    tx.commit().await.map_err(|_| unavailable())?;
    let child = Arc::new(SocialReadChild {
        pool: pool.clone(),
        recovery,
        reference,
        deadline: binding.deadline(),
    });
    // Failure/cancellation here deliberately preserves the durable intent and
    // prepared locks. Only Root's settled completion/recovery may release them.
    scope.retain_child(child.clone()).await?;
    child.check().await
}

async fn lock_sources(
    c: &mut PgConnection,
    query: &ChildQuery,
    sources: SocialReadSources,
) -> Result<(), Status> {
    // NOWAIT avoids waiting behind a queued writer that is itself waiting for
    // the projecting transaction's compatible SHARE lock.
    sqlx::query("select event_id from concert_events where event_id=$1 for share nowait")
        .bind(query.event)
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(unavailable)?;
    let (mut subjects, reveal) = match sources {
        SocialReadSources::Metadata { subjects } => (subjects, None),
        SocialReadSources::Reveal { request, platform } => {
            let row = sqlx::query("select requester_id,recipient_id,intro_id,intro_revision from concert_social_requests where event_id=$1 and request_id=$2 and state='APPROVED' and (requester_id=$3 or recipient_id=$3)")
                .bind(query.event).bind(request).bind(query.actor)
                .fetch_optional(&mut *c).await.map_err(|_| unavailable())?.ok_or_else(unavailable)?;
            (
                vec![row.get("requester_id"), row.get("recipient_id")],
                Some((
                    request,
                    platform,
                    row.get::<Uuid, _>("intro_id"),
                    row.get::<i64, _>("intro_revision"),
                )),
            )
        }
    };
    subjects.push(query.actor);
    subjects.sort();
    subjects.dedup();
    if subjects.len() > 101 || subjects.iter().any(Uuid::is_nil) {
        return Err(unavailable());
    }
    for subject in &subjects {
        sqlx::query("select user_id from concert_participants where event_id=$1 and user_id=$2 for share nowait")
            .bind(query.event).bind(subject).fetch_optional(&mut *c).await.map_err(|_| unavailable())?
            .ok_or_else(unavailable)?;
    }
    if let Some((request, platform, intro, revision)) = reveal {
        // Claim writers take this canonical per-user root FOR UPDATE before
        // replacement. Locks stay in canonical UUID order, before the Intro.
        for subject in &subjects {
            sqlx::query("select user_id from concert_age_owners where user_id=$1 for share nowait")
                .bind(subject)
                .fetch_optional(&mut *c)
                .await
                .map_err(|_| unavailable())?
                .ok_or_else(unavailable)?;
        }
        sqlx::query("select intro_id from concert_intros where event_id=$1 and intro_id=$2 and revision=$3 and state='PUBLISHED' for share nowait")
            .bind(query.event).bind(intro).bind(revision).fetch_optional(&mut *c).await.map_err(|_| unavailable())?
            .ok_or_else(unavailable)?;
        // These sources are SHARE-locked by the projector, hence compatible.
        // Request/pair roots are intentionally not locked: the projector takes
        // UPDATE there. All retained writers of those roots take event UPDATE.
        let grants: Vec<Uuid> = sqlx::query_scalar("select owner_id from concert_social_grants where event_id=$1 and request_id=$2 and platform=$3 and state='ACTIVE' order by owner_id for share nowait")
            .bind(query.event).bind(request).bind(platform.as_str()).fetch_all(&mut *c).await.map_err(|_| unavailable())?;
        if grants != subjects {
            return Err(unavailable());
        }
        for subject in &subjects {
            sqlx::query("select user_id from concert_social_handles where event_id=$1 and user_id=$2 and platform=$3 and ciphertext is not null for share nowait")
                .bind(query.event).bind(subject).bind(platform.as_str())
                .fetch_optional(&mut *c).await.map_err(|_| unavailable())?.ok_or_else(unavailable)?;
        }
    }
    Ok(())
}

async fn budget(
    tx: &mut Transaction<'_, Postgres>,
    deadline: AuthorityRpcDeadline,
) -> Result<(), Status> {
    let remaining = deadline.remaining(Duration::from_secs(5), deadline.expires_micros())?;
    let ms = remaining.as_millis().max(1).to_string();
    sqlx::query(
        "select set_config('statement_timeout',$1,true),set_config('lock_timeout',$1,true)",
    )
    .bind(ms)
    .execute(&mut **tx)
    .await
    .map_err(|_| unavailable())?;
    Ok(())
}

async fn capacity(c: &mut PgConnection) -> Result<(), Status> {
    let slots: i32 = sqlx::query_scalar("select current_setting('max_prepared_transactions')::int")
        .fetch_one(&mut *c)
        .await
        .map_err(|_| unavailable())?;
    if slots < 256 {
        return Err(unavailable());
    }
    // Share the existing local prepared-child capacity namespace; this is a
    // resource budget, not LocalPublic authority or its recovery/GID protocol.
    for slot in 0i32..101 {
        let ok: bool =
            sqlx::query_scalar("select pg_try_advisory_xact_lock(1296257617::int,$1::int)")
                .bind(slot)
                .fetch_one(&mut *c)
                .await
                .map_err(|_| unavailable())?;
        if ok {
            return Ok(());
        }
    }
    Err(Status::resource_exhausted("Social response fence capacity"))
}

#[async_trait::async_trait]
impl HeldChildGuard for SocialReadChild {
    fn recovery_reference(&self) -> Result<ChildRecoveryReference, Status> {
        Ok(self.reference.clone())
    }
    async fn check(&self) -> Result<(), Status> {
        let remaining = self
            .deadline
            .remaining(Duration::from_secs(5), self.recovery.parts().3)?;
        let (_, _, native, _) = self.recovery.parts();
        let held: bool = tokio::time::timeout(remaining, sqlx::query_scalar(
            "select exists(select 1 from pg_prepared_xacts where database=current_database() and owner=current_user and gid=$1 and transaction::text::numeric=mod($2::text::numeric,4294967296)) and (pg_control_system()).system_identifier::text=$3 and (select oid::bigint from pg_database where datname=current_database())=$4",
        ).bind(self.recovery.gid()?).bind(&native.full_xid).bind(&native.system_identifier)
            .bind(i64::from(native.database_oid)).fetch_one(&self.pool))
            .await.map_err(|_| unavailable())?.map_err(|_| unavailable())?;
        if held {
            Ok(())
        } else {
            Err(unavailable())
        }
    }
    async fn complete_settled(&self) -> Result<(), Status> {
        recover_social_read(&self.pool, &self.reference).await
    }
}

/// Root's CompositeChildSettler routes only the explicit SOCIAL_READ kind here.
/// Native effects must already be settled before this read-only child releases.
pub async fn recover_social_read(
    pool: &PgPool,
    reference: &ChildRecoveryReference,
) -> Result<(), Status> {
    let recovery: SocialReadRecovery =
        serde_json::from_value(serde_json::to_value(reference).map_err(|_| unavailable())?)
            .map_err(|_| unavailable())?;
    recover_closed(pool, &recovery).await
}
async fn recover_closed(pool: &PgPool, recovery: &SocialReadRecovery) -> Result<(), Status> {
    let gid = recovery.gid()?;
    let (_, _, native, _) = recovery.parts();
    tokio::time::timeout(Duration::from_secs(2), async {
        let current = sqlx::query("select (pg_control_system()).system_identifier::text system_identifier,(select oid::bigint from pg_database where datname=current_database()) database_oid")
            .fetch_one(pool).await.map_err(|_| unavailable())?;
        if current.get::<String,_>("system_identifier") != native.system_identifier
            || current.get::<i64,_>("database_oid") != i64::from(native.database_oid) { return Err(unavailable()); }
        let actual: Option<String> = sqlx::query_scalar("select transaction::text from pg_prepared_xacts where database=current_database() and owner=current_user and gid=$1")
            .bind(&gid).fetch_optional(pool).await.map_err(|_| unavailable())?;
        if let Some(actual) = actual {
            let low = native.full_xid.parse::<u64>().map_err(|_| unavailable())? % 4_294_967_296;
            if actual != low.to_string() { return Err(unavailable()); }
            let _ = sqlx::query(&format!("rollback prepared '{gid}'")).execute(pool).await;
        }
        let state: Option<String> = sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
            .bind(&native.full_xid).fetch_one(pool).await.map_err(|_| unavailable())?;
        if state.as_deref() == Some("aborted") { Ok(()) } else { Err(unavailable()) }
    }).await.map_err(|_| unavailable())?
}
fn unavailable() -> Status {
    Status::unavailable("Social response fence unavailable")
}

#[cfg(test)]
#[path = "../../tests/social_live_support/read_child.rs"]
mod native_tests;

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_social_recovery_rejects_other_kinds_and_arbitrary_sql_identifiers() {
        let query = ChildQuery {
            actor: Uuid::from_u128(1),
            event: Uuid::from_u128(2),
            subject: Uuid::from_u128(1),
            family: Uuid::from_u128(3),
            request: Uuid::from_u128(4),
            expires_unix_micros: 1000,
            binding_digest: [1; 32],
        };
        let native = ChildNativeIdentity {
            system_identifier: "123".into(),
            database_oid: 1,
            full_xid: "77".into(),
            backend_pid: 123,
            backend_started_unix_micros: 1,
            producer_instance_id: Uuid::from_u128(5),
        };
        let r = SocialReadRecovery::SocialRead {
            query,
            child_nonce: Uuid::from_u128(6),
            native,
            valid_until_unix_micros: 1000,
        };
        assert_eq!(
            r.gid().unwrap(),
            format!("mcr-social-read:{}", Uuid::from_u128(6).simple())
        );
        assert_eq!(r.root_reference().unwrap().kind_name(), "SOCIAL_READ");
        let mut value = serde_json::to_value(&r).unwrap();
        value["kind"] = serde_json::json!("LOCAL_PUBLIC");
        assert!(serde_json::from_value::<SocialReadRecovery>(value).is_err());
        let mut value = serde_json::to_value(&r).unwrap();
        value["gid"] = serde_json::json!("arbitrary SQL");
        assert!(serde_json::from_value::<SocialReadRecovery>(value).is_err());
        let mut value = serde_json::to_value(r).unwrap();
        value["valid_until_unix_micros"] = serde_json::json!(1001);
        assert!(serde_json::from_value::<SocialReadRecovery>(value)
            .unwrap()
            .gid()
            .is_err());
    }
}
