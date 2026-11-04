//! Source-bound SOCIAL-only moderation leaf. Never wire as the whole private owner.
use super::invalidation::SocialInvalidationSink;
use crate::{
    adapters::out::{
        relationship_authority::guarded::{proto, OwnerGuardScope, ProducerTransaction},
        repository::audit,
    },
    moderation_live::ports::{ModerationAuthority, OwnerIntent},
    product_rpc::{
        scope_ledger::ProducerTransaction as LedgerIdentity,
        settlement::{NativeTransactionResolution, NativeTransactionResolver},
    },
};
use chrono::{DateTime, Utc};
use concert_application::{
    admin::{
        moderation::{Change, OwnerStep, UserAction},
        Action,
    },
    commands::CommandError,
};
use platform::actor_delegation::owner_binding::OwnerBinding;
use prost::Message;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

pub const COVERAGE: &str = "SOCIAL_ONLY";
const PAGE: i64 = 32;
const CAPACITY: usize = 4096;
fn unavailable(_: impl std::fmt::Display) -> CommandError {
    CommandError::Unavailable
}
fn status(_: impl std::fmt::Display) -> Status {
    Status::unavailable("Social moderation unavailable")
}

pub enum SocialRevocationOutcome {
    Pending,
    Applied(SealedSocialRevocation),
}
/// Private construction is intentional. Metadata publication cannot mint this receipt.
pub struct SealedSocialRevocation {
    receipt: Uuid,
    revision: i64,
    transaction: ProducerTransaction,
}
impl SealedSocialRevocation {
    pub fn owner_receipt(&self) -> Uuid {
        self.receipt
    }
    pub fn source_revision(&self) -> i64 {
        self.revision
    }
    pub fn transaction(&self) -> &ProducerTransaction {
        &self.transaction
    }
    pub fn coverage(&self) -> &'static str {
        COVERAGE
    }
}
#[derive(Clone)]
pub struct SocialModerationRevocation {
    pool: PgPool,
    sink: SocialInvalidationSink,
}
impl SocialModerationRevocation {
    pub(crate) fn new(pool: PgPool, sink: SocialInvalidationSink) -> Self {
        Self { pool, sink }
    }
    pub async fn apply(
        &self,
        authority: &dyn ModerationAuthority,
        intent: &OwnerIntent,
        change: &Change,
    ) -> Result<SocialRevocationOutcome, CommandError> {
        let scope = validate(authority, intent, change)?;
        // The original monotonic Root deadline bounds acquisition, SQL, Track,
        // publication and commit. Failure never becomes a successful receipt.
        match tokio::time::timeout_at(
            scope.binding().deadline().stop(),
            self.round(authority, &scope, intent, change),
        )
        .await
        {
            Ok(Ok(result)) => Ok(result),
            Ok(Err(CommandError::Denied)) => Err(CommandError::Denied),
            _ => Ok(SocialRevocationOutcome::Pending),
        }
    }
    pub async fn recover(
        &self,
        authority: &dyn ModerationAuthority,
        intent: &OwnerIntent,
        change: &Change,
    ) -> Result<SocialRevocationOutcome, CommandError> {
        // round resolves every durable old witness before retrying removal or
        // publication. It never repeats restriction/World effects or restores.
        self.apply(authority, intent, change).await
    }
    async fn round(
        &self,
        authority: &dyn ModerationAuthority,
        scope: &Arc<OwnerGuardScope>,
        intent: &OwnerIntent,
        change: &Change,
    ) -> Result<SocialRevocationOutcome, CommandError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        begin(&mut tx, authority, scope, intent).await?;
        let source = source(&mut tx, intent, change).await?;
        let prior = sqlx::query("select * from concert_social_moderation_revocations where command_receipt=$1 for update")
            .bind(intent.receipt).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let receipt = prior
            .as_ref()
            .map(|r| r.get::<Uuid, _>("owner_receipt"))
            .unwrap_or_else(Uuid::new_v4);
        if let Some(row) = &prior {
            exact_journal(row, intent, &source)?;
            // Committed native outcome AND exact same-transaction witness are
            // required; a missing/uncertain identity cannot authorize replay.
            let old: Vec<sqlx::types::Json<LedgerIdentity>> = sqlx::query_scalar("select native_identity from concert_social_moderation_witnesses where owner_receipt=$1 order by producer_instance,scope_id,identity_digest limit 4097")
                .bind(receipt).fetch_all(&mut *tx).await.map_err(unavailable)?;
            if old.len() > CAPACITY {
                return Ok(SocialRevocationOutcome::Pending);
            }
            for identity in old {
                if !committed(&mut tx, &identity.0).await? {
                    return Ok(SocialRevocationOutcome::Pending);
                }
            }
        } else {
            sqlx::query("insert into concert_social_moderation_revocations(owner_receipt,command_receipt,event_id,actor_id,target_id,expected_revision,intent_digest,source_revision,restriction_result,world_result,restriction_receipt,world_receipt,restriction_native,world_native) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14)")
                .bind(receipt).bind(intent.receipt).bind(intent.actor.event).bind(intent.actor.actor).bind(intent.target).bind(intent.expected_revision).bind(intent.digest().as_slice()).bind(source.revision)
                .bind(source.restriction_result).bind(source.world_result).bind(source.restriction_receipt).bind(source.world_receipt).bind(&source.restriction_native).bind(&source.world_native).execute(&mut *tx).await.map_err(unavailable)?;
        }
        let claim = claim(&mut tx, scope, receipt).await?;
        witness(
            &mut tx,
            scope,
            intent,
            receipt,
            source.revision,
            claim,
            "PREPARE",
            None,
        )
        .await?;
        if prior.is_none() {
            // Event UPDATE held by begin excludes ordinary writers and the
            // retained Social read children through their real HTTP terminal.
            let requests: Vec<(Uuid,Uuid,Uuid)> = sqlx::query_as("select request_id,requester_id,recipient_id from concert_social_requests where event_id=$1 and $2 in(requester_id,recipient_id) order by request_id limit 4097 for update")
                .bind(intent.actor.event).bind(intent.target).fetch_all(&mut *tx).await.map_err(unavailable)?;
            if requests.len() > CAPACITY {
                return Ok(SocialRevocationOutcome::Pending);
            }
            let mut actors = vec![intent.target];
            for (_, a, b) in &requests {
                actors.extend([*a, *b]);
            }
            actors.sort();
            actors.dedup();
            if actors.len() > CAPACITY {
                return Ok(SocialRevocationOutcome::Pending);
            }
            // Permanent-deletion guards take only TRY common locks here; a
            // concurrent deletion refuses the round instead of introducing a
            // waiting event/common-lock cycle. No deletion context is minted.
            for actor in actors {
                let revision: i64 = sqlx::query_scalar("insert into concert_social_policy_generations as g(event_id,actor_id,revision) values($1,$2,2) on conflict(event_id,actor_id) do update set revision=g.revision+1 returning revision")
                    .bind(intent.actor.event).bind(actor).fetch_one(&mut *tx).await.map_err(unavailable)?;
                sqlx::query("insert into concert_social_moderation_audience(owner_receipt,event_id,actor_id,policy_revision) values($1,$2,$3,$4)")
                    .bind(receipt).bind(intent.actor.event).bind(actor).bind(revision).execute(&mut *tx).await.map_err(unavailable)?;
            }
            sqlx::query("update concert_social_grants set state='REVOKED',revision=revision+1 where event_id=$1 and $2 in(owner_id,viewer_id) and state='ACTIVE'")
                .bind(intent.actor.event).bind(intent.target).execute(&mut *tx).await.map_err(unavailable)?;
            sqlx::query("update concert_social_requests set state='REVOKED',revision=revision+1 where event_id=$1 and $2 in(requester_id,recipient_id) and state in('PENDING','APPROVED')")
                .bind(intent.actor.event).bind(intent.target).execute(&mut *tx).await.map_err(unavailable)?;
        }
        finish(&mut tx, authority, scope, claim, receipt).await?;
        tx.commit().await.map_err(unavailable)?;
        // The source/generation commit precedes all external publication.
        let actors: Vec<Uuid> = sqlx::query_scalar("select actor_id from concert_social_moderation_audience where owner_receipt=$1 and published_at is null order by actor_id limit $2")
            .bind(receipt).bind(PAGE).fetch_all(&self.pool).await.map_err(unavailable)?;
        for actor in actors {
            let mut tx = self.pool.begin().await.map_err(unavailable)?;
            begin(&mut tx, authority, scope, intent).await?;
            source_check(&mut tx, intent, change, &source).await?;
            lock_claim(&mut tx, scope, receipt, claim).await?;
            witness(
                &mut tx,
                scope,
                intent,
                receipt,
                source.revision,
                claim,
                "PUBLISH",
                Some(actor),
            )
            .await?;
            self.sink
                .publish_moderation_in(&mut tx, receipt, claim, actor)
                .await
                .map_err(unavailable)?;
            finish(&mut tx, authority, scope, claim, receipt).await?;
            tx.commit().await.map_err(unavailable)?;
        }
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        begin(&mut tx, authority, scope, intent).await?;
        source_check(&mut tx, intent, change, &source).await?;
        lock_claim(&mut tx, scope, receipt, claim).await?;
        let pending: bool = sqlx::query_scalar("select exists(select 1 from concert_social_moderation_audience where owner_receipt=$1 and published_at is null)")
            .bind(receipt).fetch_one(&mut *tx).await.map_err(unavailable)?;
        if pending {
            return Ok(SocialRevocationOutcome::Pending);
        }
        let native = if prior
            .as_ref()
            .is_some_and(|r| r.get::<Option<DateTime<Utc>>, _>("completed_at").is_some())
        {
            // An already completed leaf still needs a current, tracked native
            // transaction and current role/source checks for its new response.
            witness(
                &mut tx,
                scope,
                intent,
                receipt,
                source.revision,
                claim,
                "COMPLETE",
                None,
            )
            .await?
        } else {
            let identity = witness(
                &mut tx,
                scope,
                intent,
                receipt,
                source.revision,
                claim,
                "COMPLETE",
                None,
            )
            .await?;
            sqlx::query("update concert_social_moderation_revocations set completed_at=clock_timestamp() where owner_receipt=$1 and completed_at is null")
                .bind(receipt).execute(&mut *tx).await.map_err(unavailable)?;
            identity
        };
        finish(&mut tx, authority, scope, claim, receipt).await?;
        tx.commit().await.map_err(unavailable)?;
        authority.check_disclosure().await?;
        scope.binding().check().map_err(unavailable)?;
        Ok(SocialRevocationOutcome::Applied(SealedSocialRevocation {
            receipt,
            revision: source.revision,
            transaction: native,
        }))
    }
}

fn validate(
    a: &dyn ModerationAuthority,
    i: &OwnerIntent,
    c: &Change,
) -> Result<Arc<OwnerGuardScope>, CommandError> {
    validate_intent(i, c)?;
    let s = a.canonical_scope().ok_or(CommandError::Unavailable)?;
    let b = s.binding();
    b.check().map_err(unavailable)?;
    if b.actor() != i.actor.actor || b.event() != i.actor.event || b.operation() != i.operation {
        return Err(CommandError::Denied);
    }
    Ok(s)
}
fn validate_intent(i: &OwnerIntent, c: &Change) -> Result<(), CommandError> {
    c.validate(i.actor, Utc::now())?;
    if i.receipt.is_nil()
        || i.operation != "moderateConcertUser"
        || i.step != OwnerStep::EventPrivateAuthorityRevocation
        || i.target != c.target()
        || i.expected_revision != c.revision()
        || !matches!(
            c,
            Change::User {
                action: UserAction::Suspend,
                ..
            }
        )
    {
        return Err(CommandError::InvalidInput);
    }
    Ok(())
}
async fn begin(
    c: &mut PgConnection,
    a: &dyn ModerationAuthority,
    s: &OwnerGuardScope,
    i: &OwnerIntent,
) -> Result<(), CommandError> {
    s.binding().check().map_err(unavailable)?;
    sqlx::query("select set_config('statement_timeout','2000ms',true),set_config('lock_timeout','500ms',true)").execute(&mut *c).await.map_err(unavailable)?;
    audit::authorize(c, a, i.actor, Action::ModerateUser, i.target).await?;
    sqlx::query("select set_config('mcr.relationship_authority_until',current_setting('mcr.moderation_authority_until'),true)").execute(c).await.map_err(unavailable)?;
    Ok(())
}
#[derive(PartialEq, Eq)]
struct Source {
    revision: i64,
    restriction_result: Uuid,
    world_result: Uuid,
    restriction_receipt: Uuid,
    world_receipt: Uuid,
    restriction_native: Vec<u8>,
    world_native: Vec<u8>,
}
async fn source(
    c: &mut PgConnection,
    i: &OwnerIntent,
    change: &Change,
) -> Result<Source, CommandError> {
    let row=sqlx::query("select m.input,m.expected_revision,a.restriction_revision from concert_moderation_commands m join concert_command_receipts r using(receipt_id) join concert_event_admissions a on a.event_id=m.event_id and a.user_id=m.target_id where m.receipt_id=$1 and m.event_id=$2 and m.actor_id=$3 and m.target_id=$4 and m.operation_id='moderateConcertUser' and m.input->>'action'='SUSPEND' and m.steps=array['EVENT_RESTRICTION','EVENT_WORLD_TERMINATION','EVENT_PRIVATE_AUTHORITY_REVOCATION'] and r.state='PENDING' and r.expires_at>clock_timestamp() and (r.event_id,r.actor_id,r.target_id,r.operation_id)=(m.event_id,m.actor_id,m.target_id,m.operation_id) and a.state='SUSPENDED' for share of r,a")
        .bind(i.receipt).bind(i.actor.event).bind(i.actor.actor).bind(i.target).fetch_optional(&mut *c).await.map_err(unavailable)?.ok_or(CommandError::Unavailable)?;
    let input: serde_json::Value = row.get("input");
    if row.get::<i64, _>("expected_revision") != i.expected_revision
        || input != crate::moderation_live::wire::change_value(change)
    {
        return Err(CommandError::Denied);
    }
    let pending:bool=sqlx::query_scalar("select exists(select 1 from concert_moderation_owner_results where receipt_id=$1 and step='EVENT_PRIVATE_AUTHORITY_REVOCATION' and outcome='PENDING' and intent_digest=$2)")
        .bind(i.receipt).bind(i.digest().as_slice()).fetch_one(&mut *c).await.map_err(unavailable)?;
    if !pending {
        return Err(CommandError::Unavailable);
    }
    let revision: i64 = row.get("restriction_revision");
    let mut results = Vec::new();
    for step in [
        OwnerStep::EventRestriction,
        OwnerStep::EventWorldTermination,
    ] {
        let mut expected = i.clone();
        expected.step = step;
        let r=sqlx::query("select r.result_id,r.owner_receipt_id,r.native_identity,p.completion_identity,p.completion_identity_digest from concert_moderation_owner_results r join concert_moderation_public_owner_receipts p on p.owner_receipt=r.owner_receipt_id and p.command_receipt=r.receipt_id and p.step=r.step join concert_moderation_public_owner_witnesses w on w.owner_receipt=p.owner_receipt and w.identity_digest=p.completion_identity_digest and w.phase='COMPLETE' and w.identity=p.completion_identity where r.receipt_id=$1 and r.step=$2 and r.outcome='APPLIED' and r.intent_digest=$3 and r.owner_revision=$4 and p.event_id=$5 and p.actor_id=$6 and p.target_id=$7 and p.expected_revision=$8 and p.source_revision=$4 and p.intent_digest=$3 and p.completed_at is not null")
            .bind(i.receipt).bind(step.as_str()).bind(expected.digest().as_slice()).bind(revision).bind(i.actor.event).bind(i.actor.actor).bind(i.target).bind(i.expected_revision)
            .fetch_optional(&mut *c).await.map_err(unavailable)?.ok_or(CommandError::Unavailable)?;
        let bytes: Vec<u8> = r.get("native_identity");
        let wire = proto::TransactionIdentity::decode(bytes.as_slice()).map_err(unavailable)?;
        let identity = from_wire(wire)?;
        let completion: sqlx::types::Json<LedgerIdentity> = r.get("completion_identity");
        if r.get::<Vec<u8>, _>("completion_identity_digest")
            != identity.digest().map_err(unavailable)?
            || identity != completion.0
            || identity.effect_binding_digest != expected.digest()
            || !committed(c, &identity).await?
        {
            return Err(CommandError::Unavailable);
        }
        results.push((
            r.get::<Uuid, _>("result_id"),
            r.get::<Uuid, _>("owner_receipt_id"),
            bytes,
        ));
    }
    Ok(Source {
        revision,
        restriction_result: results[0].0,
        world_result: results[1].0,
        restriction_receipt: results[0].1,
        world_receipt: results[1].1,
        restriction_native: results[0].2.clone(),
        world_native: results[1].2.clone(),
    })
}
async fn source_check(
    c: &mut PgConnection,
    i: &OwnerIntent,
    change: &Change,
    expected: &Source,
) -> Result<(), CommandError> {
    if &source(c, i, change).await? != expected {
        return Err(CommandError::Unavailable);
    }
    Ok(())
}
fn exact_journal(
    r: &sqlx::postgres::PgRow,
    i: &OwnerIntent,
    s: &Source,
) -> Result<(), CommandError> {
    if r.get::<Uuid, _>("event_id") != i.actor.event
        || r.get::<Uuid, _>("actor_id") != i.actor.actor
        || r.get::<Uuid, _>("target_id") != i.target
        || r.get::<i64, _>("expected_revision") != i.expected_revision
        || r.get::<Vec<u8>, _>("intent_digest") != i.digest()
        || r.get::<i64, _>("source_revision") != s.revision
        || r.get::<Uuid, _>("restriction_result") != s.restriction_result
        || r.get::<Uuid, _>("world_result") != s.world_result
        || r.get::<Uuid, _>("restriction_receipt") != s.restriction_receipt
        || r.get::<Uuid, _>("world_receipt") != s.world_receipt
        || r.get::<Vec<u8>, _>("restriction_native") != s.restriction_native
        || r.get::<Vec<u8>, _>("world_native") != s.world_native
    {
        return Err(CommandError::Unavailable);
    }
    Ok(())
}
async fn claim(
    c: &mut PgConnection,
    s: &OwnerGuardScope,
    receipt: Uuid,
) -> Result<Uuid, CommandError> {
    let p = s.producer_scope();
    let instance = Uuid::parse_str(&p.instance_id).map_err(unavailable)?;
    let scope = Uuid::parse_str(&p.scope_id).map_err(unavailable)?;
    let busy:bool=sqlx::query_scalar("select exists(select 1 from concert_social_moderation_claims where owner_receipt=$1 and authority_until>clock_timestamp() and (producer_instance,scope_id)<>($2,$3))")
        .bind(receipt).bind(instance).bind(scope).fetch_one(&mut *c).await.map_err(unavailable)?;
    if busy {
        return Err(CommandError::Unavailable);
    }
    let id = Uuid::new_v4();
    sqlx::query("insert into concert_social_moderation_claims(claim_id,owner_receipt,scope_id,producer_instance,request_id,root_binding_digest,authority_until) values($1,$2,$3,$4,$5,$6,least(nullif(current_setting('mcr.moderation_authority_until',true),'')::timestamptz,$7)) on conflict(owner_receipt,producer_instance,scope_id) do nothing")
        .bind(id).bind(receipt).bind(scope).bind(instance).bind(s.binding().request_id()).bind(s.binding().digest().as_slice()).bind(DateTime::from_timestamp_micros(s.binding().deadline().expires_micros()).ok_or(CommandError::Unavailable)?).execute(&mut *c).await.map_err(unavailable)?;
    let id:Uuid=sqlx::query_scalar("select claim_id from concert_social_moderation_claims where owner_receipt=$1 and producer_instance=$2 and scope_id=$3")
        .bind(receipt).bind(instance).bind(scope).fetch_one(&mut *c).await.map_err(unavailable)?;
    lock_claim(c, s, receipt, id).await?;
    Ok(id)
}
async fn lock_claim(
    c: &mut PgConnection,
    s: &OwnerGuardScope,
    receipt: Uuid,
    claim: Uuid,
) -> Result<(), CommandError> {
    let p = s.producer_scope();
    let found:Option<Uuid>=sqlx::query_scalar("select claim_id from concert_social_moderation_claims where claim_id=$1 and owner_receipt=$2 and scope_id=$3 and producer_instance=$4 and request_id=$5 and root_binding_digest=$6 and authority_until>clock_timestamp() for share")
        .bind(claim).bind(receipt).bind(Uuid::parse_str(&p.scope_id).map_err(unavailable)?).bind(Uuid::parse_str(&p.instance_id).map_err(unavailable)?).bind(s.binding().request_id()).bind(s.binding().digest().as_slice()).fetch_optional(c).await.map_err(unavailable)?;
    if found.is_none() {
        return Err(CommandError::Unavailable);
    }
    Ok(())
}
async fn finish(
    c: &mut PgConnection,
    a: &dyn ModerationAuthority,
    s: &OwnerGuardScope,
    claim: Uuid,
    receipt: Uuid,
) -> Result<(), CommandError> {
    lock_claim(c, s, receipt, claim).await?;
    sqlx::query("set constraints all immediate")
        .execute(&mut *c)
        .await
        .map_err(unavailable)?;
    a.check_disclosure().await?;
    s.binding().check().map_err(unavailable)?;
    let valid:bool=sqlx::query_scalar("select clock_timestamp()<nullif(current_setting('mcr.moderation_authority_until',true),'')::timestamptz")
        .fetch_one(c).await.map_err(unavailable)?;
    if !valid {
        return Err(CommandError::Unavailable);
    }
    Ok(())
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Reference {
    kind: String,
    owner_receipt: Uuid,
    command_receipt: Uuid,
    intent_digest: String,
    source_revision: i64,
    scope_id: Uuid,
    request_id: Uuid,
}
async fn witness(
    c: &mut PgConnection,
    s: &OwnerGuardScope,
    i: &OwnerIntent,
    receipt: Uuid,
    revision: i64,
    claim: Uuid,
    phase: &str,
    actor: Option<Uuid>,
) -> Result<ProducerTransaction, CommandError> {
    let p = s.producer_scope();
    let instance = Uuid::parse_str(&p.instance_id).map_err(unavailable)?;
    let scope = Uuid::parse_str(&p.scope_id).map_err(unavailable)?;
    let reference = serde_json::to_vec(&Reference {
        kind: "MODERATION_PRIVATE_REVOCATION_V1".into(),
        owner_receipt: receipt,
        command_receipt: i.receipt,
        intent_digest: hex_digest(i.digest()),
        source_revision: revision,
        scope_id: scope,
        request_id: s.binding().request_id(),
    })
    .map_err(unavailable)?;
    let native = ProducerTransaction::capture_concert_in(c, i.digest(), instance, reference)
        .await
        .map_err(unavailable)?;
    s.track_transaction(native.clone())
        .await
        .map_err(unavailable)?;
    let identity = ledger(&native);
    let digest = identity.digest().map_err(unavailable)?;
    sqlx::query("insert into concert_social_moderation_witnesses(producer_instance,scope_id,identity_digest,claim_id,owner_receipt,phase,audience_actor,native_identity) values($1,$2,$3,$4,$5,$6,$7,$8)")
        .bind(instance).bind(scope).bind(digest.as_slice()).bind(claim).bind(receipt).bind(phase).bind(actor).bind(sqlx::types::Json(identity)).execute(c).await.map_err(unavailable)?;
    Ok(native)
}
fn hex_digest(d: [u8; 32]) -> String {
    d.iter().map(|b| format!("{b:02x}")).collect()
}
fn ledger(n: &ProducerTransaction) -> LedgerIdentity {
    LedgerIdentity {
        producer: "CONCERT".into(),
        system_identifier: n.system_identifier.clone(),
        database_oid: n.database_oid,
        full_xid: n.full_xid.clone(),
        backend_pid: n.backend_pid,
        backend_started_unix_micros: n.backend_started.timestamp_micros(),
        effect_binding_digest: n.effect_binding_digest,
        producer_instance: n.producer_instance_id,
        settlement_reference: n.settlement_reference.clone(),
    }
}
fn from_wire(n: proto::TransactionIdentity) -> Result<LedgerIdentity, CommandError> {
    if n.producer != 1 {
        return Err(CommandError::Unavailable);
    }
    let v = LedgerIdentity {
        producer: "CONCERT".into(),
        system_identifier: n.system_identifier,
        database_oid: n.database_oid,
        full_xid: n.full_xid,
        backend_pid: n.backend_pid,
        backend_started_unix_micros: n.backend_started_unix_micros,
        effect_binding_digest: n
            .effect_binding_digest
            .try_into()
            .map_err(|_| CommandError::Unavailable)?,
        producer_instance: Uuid::parse_str(&n.producer_instance_id).map_err(unavailable)?,
        settlement_reference: n.settlement_reference,
    };
    v.digest().map_err(unavailable)?;
    Ok(v)
}
async fn committed(c: &mut PgConnection, n: &LedgerIdentity) -> Result<bool, CommandError> {
    if native_status(c, n).await? != proto::TransactionStatus::Committed {
        return Ok(false);
    }
    sqlx::query_scalar("select exists(select 1 from concert_product_owner_transactions where instance_id=$1 and identity_digest=$2 and identity=$3 and confirmed_at is not null)")
        .bind(n.producer_instance).bind(n.digest().map_err(unavailable)?.as_slice()).bind(sqlx::types::Json(n)).fetch_one(c).await.map_err(unavailable)
}
async fn native_status(
    c: &mut PgConnection,
    n: &LedgerIdentity,
) -> Result<proto::TransactionStatus, CommandError> {
    n.digest().map_err(unavailable)?;
    let actual:(String,i64)=sqlx::query_as("select system_identifier::text,(select oid::bigint from pg_database where datname=current_database()) from pg_control_system()")
        .fetch_one(&mut *c).await.map_err(unavailable)?;
    if n.producer != "CONCERT" || actual != (n.system_identifier.clone(), i64::from(n.database_oid))
    {
        return Ok(proto::TransactionStatus::Unknown);
    }
    let outcome: Option<String> = sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
        .bind(&n.full_xid)
        .fetch_one(&mut *c)
        .await
        .map_err(unavailable)?;
    Ok(match outcome.as_deref() {
        Some("committed") => proto::TransactionStatus::Committed,
        Some("aborted") => proto::TransactionStatus::Aborted,
        Some("in progress") => proto::TransactionStatus::InProgress,
        _ => proto::TransactionStatus::Unknown,
    })
}

pub struct SocialModerationNativeResolver {
    pool: PgPool,
    instance: Uuid,
}
impl SocialModerationNativeResolver {
    pub fn new(pool: PgPool, instance: Uuid) -> Result<Self, Status> {
        if instance.is_nil() {
            return Err(status("instance"));
        }
        Ok(Self { pool, instance })
    }
}
#[async_trait::async_trait]
impl NativeTransactionResolver for SocialModerationNativeResolver {
    async fn resolve(
        &self,
        b: &OwnerBinding,
        n: &LedgerIdentity,
    ) -> Result<NativeTransactionResolution, Status> {
        let unknown = || NativeTransactionResolution {
            status: proto::TransactionStatus::Unknown,
            receipt_evidence: vec![],
        };
        b.validate()?;
        let digest = n.digest()?;
        let Ok(r) = serde_json::from_slice::<Reference>(&n.settlement_reference) else {
            return Ok(unknown());
        };
        if r.kind != "MODERATION_PRIVATE_REVOCATION_V1"
            || n.producer_instance != self.instance
            || n.producer != "CONCERT"
            || b.operation != "moderateConcertUser"
            || b.http_method != "POST"
            || b.parent_method != "/concert.product.v1.ProductService/Dispatch"
            || r.request_id != b.request_id
            || r.intent_digest != hex_digest(n.effect_binding_digest)
            || r.owner_receipt.is_nil()
            || r.command_receipt.is_nil()
            || r.scope_id.is_nil()
            || !(2..=concert_domain::MAX_REVISION).contains(&r.source_revision)
        {
            return Ok(unknown());
        }
        let mut tx = self.pool.begin().await.map_err(status)?;
        sqlx::query("set local statement_timeout='1500ms'")
            .execute(&mut *tx)
            .await
            .map_err(status)?;
        let native = native_status(&mut tx, n).await.map_err(status)?;
        if matches!(
            native,
            proto::TransactionStatus::Aborted | proto::TransactionStatus::InProgress
        ) {
            tx.commit().await.map_err(status)?;
            return Ok(NativeTransactionResolution {
                status: native,
                receipt_evidence: vec![],
            });
        }
        if !committed(&mut tx, n).await.map_err(status)? {
            return Ok(unknown());
        }
        let found:Option<String>=sqlx::query_scalar("select w.phase from concert_social_moderation_witnesses w join concert_social_moderation_revocations j using(owner_receipt) join concert_social_moderation_claims c using(claim_id) join concert_product_owner_transactions t on t.instance_id=w.producer_instance and t.scope_id=w.scope_id and t.identity_digest=w.identity_digest join concert_product_owner_scopes s on s.instance_id=w.producer_instance and s.scope_id=w.scope_id where w.producer_instance=$1 and w.scope_id=$2 and w.identity_digest=$3 and w.native_identity=$4 and w.owner_receipt=$5 and j.command_receipt=$6 and j.intent_digest=$7 and j.source_revision=$8 and c.scope_id=w.scope_id and c.producer_instance=w.producer_instance and c.owner_receipt=j.owner_receipt and c.root_binding_digest=$9 and c.request_id=$10 and s.binding=$11 and t.identity=$4 and t.binding_digest=$9 and t.confirmed_at is not null and s.phase='ROOT_FINISHED' and s.delivery_secret_hash is not null and (w.phase<>'COMPLETE' or j.completed_at is not null) and (w.phase<>'PUBLISH' or exists(select 1 from concert_social_moderation_audience a where a.owner_receipt=j.owner_receipt and a.actor_id=w.audience_actor and a.published_at is not null)) and j.actor_id=$12 and j.event_id=$13 and $14='/api/concert/events/'||j.event_id::text||'/admin/users/'||j.target_id::text||'/moderation'")
            .bind(self.instance).bind(r.scope_id).bind(digest.as_slice()).bind(sqlx::types::Json(n)).bind(r.owner_receipt).bind(r.command_receipt).bind(n.effect_binding_digest.as_slice()).bind(r.source_revision).bind(b.digest()?.as_slice()).bind(b.request_id).bind(sqlx::types::Json(b)).bind(b.actor_id).bind(b.event_id).bind(&b.http_target).fetch_optional(&mut *tx).await.map_err(status)?;
        tx.commit().await.map_err(status)?;
        if let Some(phase) = found {
            let mut h = Sha256::new();
            h.update(b"MCR Social moderation native witness v1\0");
            h.update(digest);
            h.update(r.owner_receipt.as_bytes());
            h.update(phase.as_bytes());
            return Ok(NativeTransactionResolution {
                status: proto::TransactionStatus::Committed,
                receipt_evidence: h.finalize().to_vec(),
            });
        }
        Ok(unknown())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn closed_native_reference_rejects_extra_or_missing_fields() {
        let value = serde_json::json!({"kind":"MODERATION_PRIVATE_REVOCATION_V1","owner_receipt":Uuid::new_v4(),"command_receipt":Uuid::new_v4(),"intent_digest":hex_digest([1;32]),"source_revision":2,"scope_id":Uuid::new_v4(),"request_id":Uuid::new_v4()});
        assert!(serde_json::from_value::<Reference>(value.clone()).is_ok());
        let mut extra = value.clone();
        extra["phase"] = serde_json::json!("COMPLETE");
        assert!(serde_json::from_value::<Reference>(extra).is_err());
        let mut missing = value;
        missing.as_object_mut().unwrap().remove("command_receipt");
        assert!(serde_json::from_value::<Reference>(missing).is_err());
    }
}
