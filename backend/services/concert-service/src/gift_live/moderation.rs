//! BE39 Giveaway owner. Preparation is not an effect; Root completion is not EOF.
//! All native identities are captured on the actual protected connection.
use super::{GiftInput, GiftOperation, GiftRequest};
use crate::{
    adapters::out::{
        relationship_authority::guarded::{OwnerGuardScope, ProducerKind, ProducerTransaction},
        repository::audit,
    },
    moderation_live::{ports::*, wire::{self, Query}},
    product_rpc::{
        scope_ledger::ProducerTransaction as StoredNative,
        settlement::{NativeTransactionResolution, NativeTransactionResolver},
    },
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    admin::{moderation::{Change, ContentAction, ContentKind, OwnerStep}, Action, Actor},
    commands::{CommandError, SafeFailure},
};
use platform::actor_delegation::owner_binding::OwnerBinding;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

const EFFECT_KIND: &str = "GIFT_MODERATION_OWNER_V1";
const READ_KIND: &str = "GIFT_VISIBILITY_READ_V1";

fn unavailable(_: impl std::fmt::Display) -> CommandError { CommandError::Unavailable }
fn status(_: impl std::fmt::Display) -> Status { Status::unavailable("Gift moderation unavailable") }
fn hex(bytes: [u8;32]) -> String {
    const DIGITS: &[u8;16] = b"0123456789abcdef";
    let mut out=String::with_capacity(64);
    for byte in bytes { out.push(DIGITS[(byte>>4) as usize] as char); out.push(DIGITS[(byte&15) as usize] as char); }
    out
}
fn stored(n: &ProducerTransaction) -> StoredNative {
    StoredNative {
        producer: "CONCERT".into(), system_identifier: n.system_identifier.clone(),
        database_oid: n.database_oid, full_xid: n.full_xid.clone(), backend_pid: n.backend_pid,
        backend_started_unix_micros: n.backend_started.timestamp_micros(),
        effect_binding_digest: n.effect_binding_digest, producer_instance: n.producer_instance_id,
        settlement_reference: n.settlement_reference.clone(),
    }
}
fn restored(n: StoredNative) -> Result<ProducerTransaction, CommandError> {
    n.digest().map_err(unavailable)?;
    if n.producer != "CONCERT" { return Err(CommandError::Unavailable); }
    Ok(ProducerTransaction {
        producer: ProducerKind::Concert, system_identifier: n.system_identifier,
        database_oid: n.database_oid, full_xid: n.full_xid, backend_pid: n.backend_pid,
        backend_started: DateTime::from_timestamp_micros(n.backend_started_unix_micros)
            .ok_or(CommandError::Unavailable)?,
        effect_binding_digest: n.effect_binding_digest, producer_instance_id: n.producer_instance,
        settlement_reference: n.settlement_reference,
    })
}
fn scope_key(scope: &OwnerGuardScope, instance: Uuid) -> Result<Uuid, CommandError> {
    scope.binding().check().map_err(unavailable)?;
    let p = scope.producer_scope();
    if p.instance_id != instance.to_string() { return Err(CommandError::Denied); }
    Uuid::parse_str(&p.scope_id).ok().filter(|v| !v.is_nil()).ok_or(CommandError::Unavailable)
}
async fn limits(c: &mut PgConnection) -> Result<(), CommandError> {
    sqlx::query("set local lock_timeout='200ms'").execute(&mut *c).await.map_err(unavailable)?;
    sqlx::query("set local statement_timeout='1000ms'").execute(c).await.map_err(unavailable)?;
    Ok(())
}
async fn event_lock(c: &mut PgConnection, event: Uuid) -> Result<(), CommandError> {
    sqlx::query("select pg_advisory_xact_lock(hashtextextended('mcr-gift-visibility:'||$1::text,0))")
        .bind(event).execute(c).await.map_err(unavailable)?;
    Ok(())
}
pub(crate) async fn visible_in(c: &mut PgConnection, event: Uuid, gift: Uuid) -> Result<(), CommandError> {
    // Lock the actual aggregate, not only an optional visibility row. An absent
    // visibility row must serialize with the first HIDE too.
    let visible: Option<bool> = sqlx::query_scalar("select not exists(select 1 from public.concert_gift_visibility v where v.event_id=g.event_id and v.giveaway_id=g.giveaway_id and v.hidden) from public.concert_community_giveaways g where g.event_id=$1 and g.giveaway_id=$2 for share of g")
        .bind(event).bind(gift).fetch_optional(c).await.map_err(unavailable)?;
    if visible != Some(true) { return Err(CommandError::Denied); }
    Ok(())
}

/// Registered before any ordinary Gift representation work. A committed read
/// fence remains until the actual Root HTTP delivery/cancellation terminal.
pub(crate) async fn retain_representation(pool: &PgPool, scope: &Arc<OwnerGuardScope>, request: &GiftRequest) -> Result<(), Status> {
    super::authority::validate_binding(request, scope)?;
    let p = scope.producer_scope();
    let instance = Uuid::parse_str(&p.instance_id).map_err(status)?;
    let scope_id = scope_key(scope, instance).map_err(status)?;
    let binding_digest = scope.binding().digest();
    let event = request.identity().event;
    let reference = serde_json::to_vec(&json!({"kind":READ_KIND,"instance_id":instance,
        "scope_id":scope_id,"event_id":event,"binding_digest":hex(binding_digest)})).map_err(status)?;
    let mut h = Sha256::new(); h.update(b"MCR-gift-visibility-read-v1\0"); h.update(binding_digest);
    let effect: [u8;32] = h.finalize().into();
    let mut tx = pool.begin().await.map_err(status)?;
    limits(&mut tx).await.map_err(status)?;
    // Track cannot run while this connection already holds Root witness locks.
    let native = ProducerTransaction::capture_concert_in(&mut tx, effect, instance, reference.clone()).await?;
    let n = stored(&native); let digest = n.digest()?;
    scope.track_transaction(native).await?;
    event_lock(&mut tx, event).await.map_err(status)?;
    sqlx::query("insert into public.concert_gift_visibility_read_scopes(instance_id,scope_id,event_id,binding_digest,identity_digest,native_identity,settlement_reference) values($1,$2,$3,$4,$5,$6,$7)")
        .bind(instance).bind(scope_id).bind(event).bind(binding_digest.as_slice()).bind(digest.as_slice())
        .bind(sqlx::types::Json(&n)).bind(reference).execute(&mut *tx).await.map_err(status)?;
    // A direct targeted request fails before its business or private read. The
    // same check also runs inside the business resource transaction below.
    if request.operation() != GiftOperation::Cancel {
        let target = match request.input() {
            GiftInput::Read {target, ..} => match request.operation() {
                GiftOperation::Detail | GiftOperation::Management => *target,
                _ => None,
            },
            GiftInput::Write {giveaway, ..} => *giveaway,
        };
        if let Some(target) = target { visible_in(&mut tx, event, target).await.map_err(status)?; }
    }
    scope.binding().check()?;
    tx.commit().await.map_err(status)
}

pub struct GiftModerationOwner { pool: PgPool, instance: Uuid }
impl GiftModerationOwner {
    pub fn new(pool: PgPool, instance: Uuid) -> Result<Self, CommandError> {
        if instance.is_nil() || pool.is_closed() { return Err(CommandError::InvalidInput); }
        Ok(Self {pool, instance})
    }
    fn context(&self, a: &dyn ModerationAuthority, actor: Actor, operation: &str) -> Result<Arc<OwnerGuardScope>, CommandError> {
        let scope = a.canonical_scope().ok_or(CommandError::Denied)?;
        scope_key(&scope, self.instance)?;
        let b = scope.binding();
        if b.actor()!=actor.actor || b.event()!=actor.event || b.operation()!=operation {
            return Err(CommandError::Denied);
        }
        Ok(scope)
    }
    async fn execute(&self, a: &dyn ModerationAuthority, i: &OwnerIntent, change: &Change) -> Result<OwnerOutcome, CommandError> {
        let (action, change_digest) = validate_intent(i, change)?;
        let scope = self.context(a, i.actor, "moderateConcertContent")?;
        if scope.binding().http_method()!="POST" { return Err(CommandError::Denied); }
        a.check_disclosure().await?;
        let scope_id = scope_key(&scope, self.instance)?;
        let binding_digest = scope.binding().digest();
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        limits(&mut tx).await?;
        sqlx::query("select pg_advisory_xact_lock(hashtextextended('mcr-gift-moderation:'||$1::text,0))")
            .bind(i.receipt).execute(&mut *tx).await.map_err(unavailable)?;
        // Resolve the stored identity BEFORE capturing a new effect identity.
        let old = sqlx::query("select e.*,s.binding original_binding from public.concert_gift_moderation_effects e join public.concert_product_owner_scopes s using(instance_id,scope_id,binding_digest) where e.command_receipt_id=$1 and e.owner_step='GIVEAWAY_CONTENT'")
            .bind(i.receipt).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        if let Some(row) = old {
            let same = row.try_get::<Vec<u8>,_>("intent_digest").map_err(unavailable)? == i.digest()
                && row.try_get::<Vec<u8>,_>("change_digest").map_err(unavailable)? == change_digest
                && row.try_get::<String,_>("state").map_err(unavailable)? == "COMPLETE";
            if !same { return Err(CommandError::Conflict); }
            let n: StoredNative = serde_json::from_value(row.try_get("native_identity").map_err(unavailable)?).map_err(unavailable)?;
            let original: OwnerBinding = serde_json::from_value(row.try_get("original_binding").map_err(unavailable)?).map_err(unavailable)?;
            let owner_receipt = row.try_get("owner_receipt_id").map_err(unavailable)?;
            let revision = row.try_get("revision").map_err(unavailable)?;
            // Current authority is reacquired on this fresh original POST; it
            // cannot authorize a new effect under command-status GET.
            audit::authorize(&mut tx, a, i.actor, Action::ModerateContent, i.target).await?;
            a.check_disclosure().await?;
            let proof = native_resolution(&mut tx, self.instance, &original, &n, false).await.map_err(unavailable)?;
            if proof.status != transaction_status::Committed {
                tx.rollback().await.map_err(unavailable)?;
                return Ok(OwnerOutcome::Pending {intent:i.clone()});
            }
            tx.commit().await.map_err(unavailable)?;
            return Ok(OwnerOutcome::Applied {intent:i.clone(), owner_receipt, revision, transaction:restored(n)?});
        }
        event_lock(&mut tx, i.actor.event).await?;
        let active: bool = sqlx::query_scalar("select exists(select 1 from public.concert_gift_visibility_read_scopes r join public.concert_product_owner_scopes s using(instance_id,scope_id,binding_digest) where r.event_id=$1 and s.delivery_completed_at is null and s.cancellation_settled_at is null)")
            .bind(i.actor.event).fetch_one(&mut *tx).await.map_err(unavailable)?;
        if active {
            tx.rollback().await.map_err(unavailable)?;
            return Ok(OwnerOutcome::Pending {intent:i.clone()});
        }
        let owner_receipt = Uuid::new_v4();
        let reference = serde_json::to_vec(&json!({"kind":EFFECT_KIND,"owner_receipt_id":owner_receipt,
            "command_receipt_id":i.receipt,"actor_id":i.actor.actor,"event_id":i.actor.event,
            "giveaway_id":i.target,"operation_id":i.operation,"owner_step":i.step.as_str(),
            "expected_revision":i.expected_revision,"action":action.as_str(),"change_digest":hex(change_digest),
            "instance_id":self.instance,"scope_id":scope_id,"binding_digest":hex(binding_digest)})).map_err(unavailable)?;
        let n = ProducerTransaction::capture_concert_in(&mut tx, i.digest(), self.instance, reference.clone()).await.map_err(unavailable)?;
        let native = stored(&n); let identity_digest = native.digest().map_err(unavailable)?;
        scope.track_transaction(n.clone()).await.map_err(unavailable)?;
        audit::authorize(&mut tx, a, i.actor, Action::ModerateContent, i.target).await?;
        sqlx::query("select public.concert_moderation_require_native_authority($1,$2,$3,$4,$5,$6)")
            .bind(self.instance).bind(scope_id).bind(binding_digest.as_slice()).bind(i.actor.actor).bind(i.actor.event)
            .bind(&i.operation).execute(&mut *tx).await.map_err(unavailable)?;
        let row = sqlx::query("select g.* from public.concert_community_giveaways g where g.event_id=$1 and g.giveaway_id=$2 and not exists(select 1 from public.concert_gift_withdrawn_roots w where w.event_id=g.event_id and w.giveaway_id=g.giveaway_id) for update of g")
            .bind(i.actor.event).bind(i.target).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let Some(row) = row else {
            tx.rollback().await.map_err(unavailable)?;
            return Ok(OwnerOutcome::Failed {intent:i.clone(), reason:SafeFailure::ResourceUnavailable});
        };
        // Parse and validate the actual owner aggregate as well as denormalized
        // columns; the creator's role is not a content-moderation hierarchy.
        let g = super::queries::parse(&row, i.actor.event).map_err(|_|CommandError::Unavailable)?;
        if g.revision != i.expected_revision {
            tx.rollback().await.map_err(unavailable)?;
            return Ok(OwnerOutcome::Failed {intent:i.clone(), reason:SafeFailure::VersionConflict});
        }
        let hidden: Option<bool> = sqlx::query_scalar("select hidden from public.concert_gift_visibility where event_id=$1 and giveaway_id=$2 for update")
            .bind(i.actor.event).bind(i.target).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        if hidden.unwrap_or(false) == (action==ContentAction::Hide) {
            tx.rollback().await.map_err(unavailable)?;
            return Ok(OwnerOutcome::Failed {intent:i.clone(), reason:SafeFailure::InvalidTransition});
        }
        let revision = i.expected_revision.checked_add(1).filter(|v|*v<=concert_domain::MAX_REVISION).ok_or(CommandError::InvalidInput)?;
        sqlx::query("insert into public.concert_gift_moderation_effects(owner_receipt_id,command_receipt_id,actor_id,event_id,giveaway_id,operation_id,owner_step,expected_revision,revision,action,intent_digest,change_digest,instance_id,scope_id,binding_digest,identity_digest,native_identity,settlement_reference) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17,$18)")
            .bind(owner_receipt).bind(i.receipt).bind(i.actor.actor).bind(i.actor.event).bind(i.target).bind(&i.operation)
            .bind(i.step.as_str()).bind(i.expected_revision).bind(revision).bind(action.as_str()).bind(i.digest().as_slice())
            .bind(change_digest.as_slice()).bind(self.instance).bind(scope_id).bind(binding_digest.as_slice()).bind(identity_digest.as_slice())
            .bind(sqlx::types::Json(&native)).bind(reference).execute(&mut *tx).await.map_err(unavailable)?;
        sqlx::query("select set_config('mcr.gift_moderation_owner_receipt',$1,true)").bind(owner_receipt.to_string()).execute(&mut *tx).await.map_err(unavailable)?;
        let changed = sqlx::query("update public.concert_community_giveaways set revision=$3,aggregate=jsonb_set(aggregate,'{revision}',to_jsonb($3::bigint),false) where event_id=$1 and giveaway_id=$2 and revision=$4")
            .bind(i.actor.event).bind(i.target).bind(revision).bind(i.expected_revision).execute(&mut *tx).await.map_err(unavailable)?.rows_affected();
        if changed!=1 { return Err(CommandError::Unavailable); }
        if hidden.is_some() {
            sqlx::query("update public.concert_gift_visibility set hidden=$3,revision=$4,owner_receipt_id=$5 where event_id=$1 and giveaway_id=$2")
                .bind(i.actor.event).bind(i.target).bind(action==ContentAction::Hide).bind(revision).bind(owner_receipt)
                .execute(&mut *tx).await.map_err(unavailable)?;
        } else {
            sqlx::query("insert into public.concert_gift_visibility(event_id,giveaway_id,hidden,revision,owner_receipt_id) values($1,$2,true,$3,$4)")
                .bind(i.actor.event).bind(i.target).bind(revision).bind(owner_receipt).execute(&mut *tx).await.map_err(unavailable)?;
        }
        sqlx::query("update public.concert_gift_moderation_effects set state='COMPLETE',completed_at=clock_timestamp() where owner_receipt_id=$1 and state='PREPARED'")
            .bind(owner_receipt).execute(&mut *tx).await.map_err(unavailable)?;
        a.check_disclosure().await?;
        scope.binding().check().map_err(unavailable)?;
        // Commit errors are uncertain. Never convert them to a safe FAILED or
        // claim APPLIED; a later identical POST resolves the stored native TX.
        if tx.commit().await.is_err() { return Ok(OwnerOutcome::Pending {intent:i.clone()}); }
        Ok(OwnerOutcome::Applied {intent:i.clone(), owner_receipt, revision, transaction:n})
    }
}
fn validate_intent(i: &OwnerIntent, c: &Change) -> Result<(ContentAction,[u8;32]),CommandError> {
    c.validate(i.actor,Utc::now())?;
    let Change::Content {kind:ContentKind::Giveaway,target,expected_revision,action,..}=c else { return Err(CommandError::Denied); };
    if i.receipt.is_nil() || *target!=i.target || *expected_revision!=i.expected_revision
        || i.step!=OwnerStep::GiveawayContent || i.operation!="moderateConcertContent"
        || i.expected_revision>=concert_domain::MAX_REVISION { return Err(CommandError::InvalidInput); }
    let bytes=serde_json::to_vec(&wire::change_value(c)).map_err(unavailable)?;
    let mut h=Sha256::new();h.update(b"MCR-gift-moderation-change-v1\0");h.update(bytes);
    Ok((*action,h.finalize().into()))
}

#[async_trait]
impl ModerationOwner for GiftModerationOwner {
    async fn apply(&self,a:&dyn ModerationAuthority,i:&OwnerIntent,c:&Change)->Result<OwnerOutcome,CommandError>{self.execute(a,i,c).await}
    async fn recover(&self,a:&dyn ModerationAuthority,i:&OwnerIntent,c:&Change)->Result<OwnerOutcome,CommandError>{self.execute(a,i,c).await}
    async fn review(&self,a:&dyn ModerationAuthority,actor:Actor,target:Uuid)->Result<Value,CommandError>{
        self.context(a,actor,"moderateConcertContent")?;
        let mut tx=self.pool.begin().await.map_err(unavailable)?;limits(&mut tx).await?;
        audit::authorize(&mut tx,a,actor,Action::ModerateContent,target).await?;
        let row=sqlx::query("select g.revision,coalesce(v.hidden,false) hidden from public.concert_community_giveaways g left join public.concert_gift_visibility v using(event_id,giveaway_id) where g.event_id=$1 and g.giveaway_id=$2 and not exists(select 1 from public.concert_gift_withdrawn_roots w where w.event_id=g.event_id and w.giveaway_id=g.giveaway_id) for share of g")
            .bind(actor.event).bind(target).fetch_optional(&mut *tx).await.map_err(unavailable)?.ok_or(CommandError::Denied)?;
        let view=review_row(target,row.try_get("revision").map_err(unavailable)?,row.try_get("hidden").map_err(unavailable)?);
        wire::validate_value("Domain_admin_ContentReview",&view).map_err(unavailable)?;
        a.check_disclosure().await?;tx.commit().await.map_err(unavailable)?;Ok(view)
    }
}
fn review_row(target:Uuid,revision:i64,hidden:bool)->Value{
    json!({"target_type":"GIVEAWAY","target_id":target,"state":if hidden{"HIDDEN"}else{"VISIBLE"},"revision":revision})
}
#[async_trait]
impl ModerationDirectory for GiftModerationOwner {
    async fn page_in(&self,a:&dyn ModerationAuthority,c:&mut PgConnection,actor:Actor,operation:&str,q:&Query,after:Option<&str>,snapshot:DateTime<Utc>)->Result<Vec<Value>,CommandError>{
        self.context(a,actor,"listConcertAdminContent")?;
        if operation!="listConcertAdminContent" || !(1..=100).contains(&q.limit) {return Err(CommandError::Denied);}
        if q.filters.get("target_type").is_some_and(|v|v!="GIVEAWAY"){return Ok(vec![]);}
        if q.filters.keys().any(|k|!matches!(k.as_str(),"target_type"|"state")){return Err(CommandError::InvalidInput);}
        audit::authorize(c,a,actor,Action::AdminContent,actor.event).await?;
        let rows=sqlx::query("select g.giveaway_id,g.revision,coalesce(v.hidden,false) hidden from public.concert_community_giveaways g left join public.concert_gift_visibility v using(event_id,giveaway_id) where g.event_id=$1 and (g.aggregate->>'created_at')::timestamptz<=$2 and ($3::text is null or ('GIVEAWAY/'||g.giveaway_id::text)>$3) and ($4::text is null or (case when coalesce(v.hidden,false) then 'HIDDEN' else 'VISIBLE' end)=$4) and not exists(select 1 from public.concert_gift_withdrawn_roots w where w.event_id=g.event_id and w.giveaway_id=g.giveaway_id) order by g.giveaway_id::text limit $5 for share of g")
            .bind(actor.event).bind(snapshot).bind(after).bind(q.filters.get("state")).bind(i64::from(q.limit)+1)
            .fetch_all(&mut *c).await.map_err(unavailable)?;
        let mut out=Vec::with_capacity(rows.len());
        for r in rows {
            let value=review_row(r.try_get("giveaway_id").map_err(unavailable)?,r.try_get("revision").map_err(unavailable)?,r.try_get("hidden").map_err(unavailable)?);
            wire::validate_value("Domain_admin_ContentReview",&value).map_err(unavailable)?;out.push(value);
        }
        a.check_disclosure().await?;Ok(out)
    }
}

use crate::adapters::out::relationship_authority::guarded::proto::TransactionStatus as transaction_status;
/// Route by the closed reference kind before the ordinary Gift13 resolver.
pub fn handles_native(identity:&StoredNative)->bool{
    serde_json::from_slice::<Value>(&identity.settlement_reference).ok()
        .and_then(|v|v.get("kind").and_then(Value::as_str).map(str::to_owned))
        .is_some_and(|v|matches!(v.as_str(),EFFECT_KIND|READ_KIND))
}
pub struct GiftModerationNativeResolver {pool:PgPool,instance:Uuid}
impl GiftModerationNativeResolver {
    pub fn new(pool:PgPool,instance:Uuid)->Result<Self,Status>{
        if instance.is_nil() || pool.is_closed(){return Err(status("configuration"));}Ok(Self{pool,instance})
    }
}
#[async_trait]
impl NativeTransactionResolver for GiftModerationNativeResolver {
    async fn resolve(&self,b:&OwnerBinding,n:&StoredNative)->Result<NativeTransactionResolution,Status>{
        let mut tx=self.pool.begin().await.map_err(status)?;limits(&mut tx).await.map_err(status)?;
        let answer=native_resolution(&mut tx,self.instance,b,n,true).await?;
        tx.commit().await.map_err(status)?;Ok(answer)
    }
}
async fn native_resolution(c:&mut PgConnection,instance:Uuid,b:&OwnerBinding,n:&StoredNative,require_finished:bool)->Result<NativeTransactionResolution,Status>{
    let mut answer=NativeTransactionResolution{status:transaction_status::Unknown,receipt_evidence:vec![]};
    let digest=n.digest()?;let binding_digest=b.digest()?;
    if n.producer!="CONCERT" || n.producer_instance!=instance || !handles_native(n){return Ok(answer);}
    let actual:(String,i64)=sqlx::query_as("select s.system_identifier::text,d.oid::bigint from pg_control_system() s cross join pg_database d where d.datname=current_database()")
        .fetch_one(&mut *c).await.map_err(status)?;
    if actual!=(n.system_identifier.clone(),i64::from(n.database_oid)){return Ok(answer);}
    let state:Option<String>=sqlx::query_scalar("select pg_xact_status($1::text::xid8)").bind(&n.full_xid).fetch_one(&mut *c).await.map_err(status)?;
    match state.as_deref(){
        Some("aborted")=>answer.status=transaction_status::Aborted,
        Some("in progress")=>answer.status=transaction_status::InProgress,
        Some("committed")=>{
            let exact:bool=sqlx::query_scalar("select exists(select 1 from public.concert_product_owner_transactions t join public.concert_product_owner_scopes s using(instance_id,scope_id,binding_digest) where t.instance_id=$1 and t.identity_digest=$2 and t.binding_digest=$3 and t.identity=$4 and t.confirmed_at is not null and s.binding=$5 and s.delivery_secret_hash is not null and (not $6 or s.phase='ROOT_FINISHED') and (exists(select 1 from public.concert_gift_moderation_effects e where e.instance_id=t.instance_id and e.scope_id=t.scope_id and e.binding_digest=t.binding_digest and e.identity_digest=t.identity_digest and e.native_identity=t.identity and e.settlement_reference=$7 and e.intent_digest=$8 and e.state='COMPLETE' and e.giveaway_effect_token is not null and e.visibility_effect_token is not null) or exists(select 1 from public.concert_gift_visibility_read_scopes r where r.instance_id=t.instance_id and r.scope_id=t.scope_id and r.binding_digest=t.binding_digest and r.identity_digest=t.identity_digest and r.native_identity=t.identity and r.settlement_reference=$7)))")
                .bind(instance).bind(digest.as_slice()).bind(binding_digest.as_slice()).bind(sqlx::types::Json(n))
                .bind(sqlx::types::Json(b)).bind(require_finished).bind(&n.settlement_reference).bind(n.effect_binding_digest.as_slice())
                .fetch_one(c).await.map_err(status)?;
            if exact {
                let mut h=Sha256::new();h.update(b"MCR-gift-moderation-native-commit-v1\0");h.update(binding_digest);h.update(digest);
                answer.status=transaction_status::Committed;answer.receipt_evidence=h.finalize().to_vec();
            }
        }
        _=>(),
    }
    Ok(answer)
}
