use super::{model, owner, proto, OperationBudget, SettlementResolver};
use platform::grpc_auth::Workload;
use sqlx::{PgConnection, PgPool, Postgres, Row, Transaction};
use std::{collections::BTreeSet, sync::Arc};
use tonic::Status;
use uuid::Uuid;

pub const MAX_GUARDS: i64 = 16;
const MAX_PARTICIPANTS: i64 = 128;
#[derive(Clone)]
pub struct PgOwnerAuthority {
    pool: PgPool,
    resolver: Arc<dyn SettlementResolver>,
}
struct Lease {
    id: Uuid,
    digest: Vec<u8>,
    binding: proto::GuardBinding,
    subjects: Vec<Uuid>,
    scope: proto::ProducerScope,
    preparer: Option<(i32, chrono::DateTime<chrono::Utc>, String)>,
    view_hash: Vec<u8>,
    completion_hash: Vec<u8>,
    gid: String,
    state: String,
    revision: Option<proto::OwnerRevision>,
    scope_settlement: Option<serde_json::Value>,
}
impl Lease {
    fn row(row: sqlx::postgres::PgRow) -> Result<Self, Status> {
        let epoch: Option<Uuid> = row.try_get("owner_epoch").map_err(super::unavailable)?;
        let revision: Option<i64> = row.try_get("owner_revision").map_err(super::unavailable)?;
        let preparer_pid: Option<i32> = row.try_get("preparer_pid").map_err(super::unavailable)?;
        let preparer_started: Option<chrono::DateTime<chrono::Utc>> = row
            .try_get("preparer_started")
            .map_err(super::unavailable)?;
        let preparer_xid: Option<String> =
            row.try_get("preparer_xid").map_err(super::unavailable)?;
        let preparer = preparer_pid
            .zip(preparer_started)
            .zip(preparer_xid)
            .map(|((p, s), x)| (p, s, x));
        let value = Self {
            preparer,
            id: row.try_get("guard_id").map_err(super::unavailable)?,
            digest: row.try_get("binding_digest").map_err(super::unavailable)?,
            binding: model::decode(row.try_get("binding").map_err(super::unavailable)?)?,
            subjects: row
                .try_get("enrolled_subjects")
                .map_err(super::unavailable)?,
            scope: model::decode(row.try_get("response_scope").map_err(super::unavailable)?)?,
            view_hash: row.try_get("view_hash").map_err(super::unavailable)?,
            completion_hash: row.try_get("completion_hash").map_err(super::unavailable)?,
            gid: row.try_get("prepared_gid").map_err(super::unavailable)?,
            state: row.try_get("state").map_err(super::unavailable)?,
            revision: epoch
                .zip(revision)
                .map(|(epoch, revision)| proto::OwnerRevision {
                    epoch: epoch.to_string(),
                    revision,
                }),
            scope_settlement: row
                .try_get("scope_settlement")
                .map_err(super::unavailable)?,
        };
        if value.gid != format!("{}{}", owner::GID_PREFIX, value.id.simple())
            || !model::constant_eq(&value.digest, &model::binding_digest(&value.binding)?)
        {
            return Err(model::unavailable());
        }
        Ok(value)
    }
    fn authorize(
        &self,
        view: &proto::GuardView,
        binding: &proto::GuardBinding,
    ) -> Result<(), Status> {
        if model::view(view, binding)? != self.id
            || !model::constant_eq(&self.digest, &model::binding_digest(binding)?)
            || !model::secret_matches(&view.view_secret, &self.view_hash)
        {
            return Err(model::denied());
        }
        Ok(())
    }
}
impl PgOwnerAuthority {
    pub fn new(pool: PgPool, resolver: Arc<dyn SettlementResolver>) -> Result<Self, Status> {
        if !resolver.configured(proto::Producer::Concert) {
            return Err(model::unavailable());
        }
        Ok(Self { pool, resolver })
    }
    async fn transaction(
        &self,
        budget: &OperationBudget,
    ) -> Result<Transaction<'static, Postgres>, Status> {
        budget.remaining()?;
        let mut tx = self.pool.begin().await.map_err(super::unavailable)?;
        budget.sql(&mut tx).await?;
        Ok(tx)
    }
    /// Required at startup and before grants: durable prepare support, correct
    /// owner-local orphan accounting and no untracked prepared source fences.
    pub async fn verify_ready(&self, budget: &OperationBudget) -> Result<(), Status> {
        budget.run(async {
            let mut tx=self.transaction(budget).await?;
            let capacity:i32=sqlx::query_scalar("select current_setting('max_prepared_transactions')::int").fetch_one(&mut *tx).await.map_err(super::unavailable)?;
            if capacity<MAX_GUARDS as i32{return Err(model::unavailable());}
            let orphan:bool=sqlx::query_scalar("select exists(select 1 from pg_prepared_xacts p where p.database=current_database() and left(p.gid,9)=$1 and not exists(select 1 from public.concert_owner_guard_leases l where l.prepared_gid=p.gid and l.state not in ('COMPLETED','ABANDONED')))")
                .bind(owner::GID_PREFIX).fetch_one(&mut *tx).await.map_err(super::unavailable)?;
            if orphan{return Err(model::unavailable());}tx.commit().await.map_err(super::unavailable)
        }).await
    }
    pub async fn acquire(
        &self,
        request: proto::AcquireRequest,
        budget: &OperationBudget,
    ) -> Result<proto::AcquireResponse, Status> {
        budget.run(async {
            let binding=request.binding.as_ref().ok_or_else(model::invalid)?;let subjects=model::binding(binding)?;model::live(binding)?;
            if binding.expires_unix_micros>chrono::Utc::now().timestamp_micros()+5_000_000{return Err(model::invalid());}
            let scope=request.response_scope.as_ref().ok_or_else(model::invalid)?;model::scope(scope,binding)?;
            self.verify_ready(budget).await?;
            let id=Uuid::new_v4();let gid=format!("{}{}",owner::GID_PREFIX,id.simple());let view=model::secret();let completion=model::secret();
            let digest=model::binding_digest(binding)?;
            // The durable reservation precedes PREPARE. Cancellation can never
            // create an untracked prepared transaction. All pending rows count.
            let mut reserve=self.transaction(budget).await?;
            sqlx::query("select pg_advisory_xact_lock(733617291)").execute(&mut *reserve).await.map_err(super::unavailable)?;
            let (pending,prepared,capacity):(i64,i64,i32)=sqlx::query_as("select (select count(*) from public.concert_owner_guard_leases where state not in ('COMPLETED','ABANDONED')),(select count(*) from pg_prepared_xacts),current_setting('max_prepared_transactions')::int").fetch_one(&mut *reserve).await.map_err(super::unavailable)?;
            if pending>=MAX_GUARDS||prepared>=i64::from(capacity){return Err(Status::resource_exhausted("Owner guard capacity exhausted"));}
            budget.sql(&mut reserve).await?;
            sqlx::query("insert into public.concert_owner_guard_leases(guard_id,binding_digest,binding,enrolled_subjects,response_scope,view_hash,completion_hash,prepared_gid,state,expires_at) values($1,$2,$3,$4,$5,$6,$7,$8,'PREPARING',$9)")
                .bind(id).bind(digest.to_vec()).bind(model::json(binding)?).bind(&subjects).bind(model::json(scope)?).bind(model::secret_hash(&view)?.to_vec()).bind(model::secret_hash(&completion)?.to_vec()).bind(&gid).bind(chrono::DateTime::from_timestamp_micros(binding.expires_unix_micros).ok_or_else(model::invalid)?).execute(&mut *reserve).await.map_err(super::unavailable)?;
            reserve.commit().await.map_err(super::unavailable)?;
            // Detach: cancellation closes this physical session, rolling back
            // non-prepared work rather than returning an open transaction to pool.
            let mut source=self.pool.acquire().await.map_err(super::unavailable)?.detach();
            sqlx::query("begin isolation level read committed").execute(&mut source).await.map_err(super::unavailable)?;
            budget.sql(&mut source).await?;
            let preparer:(i32,chrono::DateTime<chrono::Utc>,String)=sqlx::query_as("select pg_backend_pid(),backend_start,pg_current_xact_id()::text from pg_stat_activity where pid=pg_backend_pid()")
                .fetch_one(&mut source).await.map_err(super::unavailable)?;
            let mut identity=self.transaction(budget).await?;let lease=self.lock(&mut identity,id,false).await?.ok_or_else(model::unavailable)?;
            if lease.state!="PREPARING"{return Err(model::unavailable());}
            sqlx::query("update public.concert_owner_guard_leases set preparer_pid=$2,preparer_started=$3,preparer_xid=$4 where guard_id=$1")
                .bind(id).bind(preparer.0).bind(preparer.1).bind(&preparer.2).execute(&mut *identity).await.map_err(super::unavailable)?;
            identity.commit().await.map_err(super::unavailable)?;
            // Durable identity precedes permission to PREPARE. Recovery cannot
            // infer abort from absence while a late PREPARE might still execute.
            let mut control=self.transaction(budget).await?;let lease=self.lock(&mut control,id,false).await?.ok_or_else(model::unavailable)?;
            if lease.state!="PREPARING"{return Err(model::unavailable());}
            budget.sql(&mut source).await?;
            let revision=sqlx::query(owner::REVISION_LOCK).fetch_one(&mut source).await.map_err(super::unavailable)?;
            let expected=proto::OwnerRevision{epoch:revision.try_get::<Uuid,_>("owner_epoch").map_err(super::unavailable)?.to_string(),revision:revision.try_get("revision").map_err(super::unavailable)?};
            budget.sql(&mut source).await?;model::live(binding)?;let facts=owner::facts(&mut source,binding,&subjects).await?;
            if facts.revision.as_ref()!=Some(&expected){return Err(model::unavailable());}
            budget.sql(&mut source).await?;
            sqlx::query(&format!("prepare transaction '{gid}'")).execute(&mut source).await.map_err(super::unavailable)?;drop(source);
            // After PREPARE every error leaves a visible PREPARING quarantine.
            model::live(binding)?;budget.sql(&mut control).await?;
            sqlx::query("update public.concert_owner_guard_leases set state='ACTIVE',owner_epoch=$2,owner_revision=$3 where guard_id=$1")
                .bind(id).bind(model::uuid(&expected.epoch)?).bind(expected.revision).execute(&mut *control).await.map_err(super::unavailable)?;
            control.commit().await.map_err(super::unavailable)?;model::live(binding)?;budget.remaining()?;
            Ok(proto::AcquireResponse{view:Some(proto::GuardView{guard_id:id.to_string(),binding_digest:digest.to_vec(),view_secret:view,expires_unix_micros:binding.expires_unix_micros}),completion_secret:completion,revision:Some(expected),session_live:facts.session_live,subjects:facts.subjects})
        }).await
    }
    async fn lock(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        id: Uuid,
        skip: bool,
    ) -> Result<Option<Lease>, Status> {
        let sql = if skip {
            "select * from public.concert_owner_guard_leases where guard_id=$1 for update skip locked"
        } else {
            "select * from public.concert_owner_guard_leases where guard_id=$1 for update"
        };
        sqlx::query(sql)
            .bind(id)
            .fetch_optional(&mut **tx)
            .await
            .map_err(super::unavailable)?
            .map(Lease::row)
            .transpose()
    }
    async fn prepared(&self, connection: &mut PgConnection, lease: &Lease) -> Result<bool, Status> {
        let found:Option<bool>=sqlx::query_scalar("select database=current_database() and owner=current_user from pg_prepared_xacts where gid=$1").bind(&lease.gid).fetch_optional(connection).await.map_err(super::unavailable)?;
        match found {
            None => Ok(false),
            Some(true) => Ok(true),
            Some(false) => Err(model::unavailable()),
        }
    }
    async fn active(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        view: &proto::GuardView,
        binding: &proto::GuardBinding,
        budget: &OperationBudget,
    ) -> Result<Lease, Status> {
        model::binding(binding)?;
        model::live(binding)?;
        let id = model::view(view, binding)?;
        budget.sql(tx).await?;
        let lease = self.lock(tx, id, false).await?.ok_or_else(model::denied)?;
        lease.authorize(view, binding)?;
        if lease.state != "ACTIVE" || !self.prepared(tx, &lease).await? {
            return Err(model::unavailable());
        }
        Ok(lease)
    }
    pub async fn check(
        &self,
        request: proto::CheckRequest,
        budget: &OperationBudget,
    ) -> Result<proto::CheckResponse, Status> {
        budget
            .run(async {
                let binding = request.binding.as_ref().ok_or_else(model::invalid)?;
                let view = request.view.as_ref().ok_or_else(model::invalid)?;
                let mut tx = self.transaction(budget).await?;
                let lease = self.active(&mut tx, view, binding, budget).await?;
                budget.sql(&mut tx).await?;
                let facts = owner::facts(&mut tx, &lease.binding, &lease.subjects).await?;
                if facts.revision != lease.revision {
                    return Err(model::unavailable());
                }
                tx.commit().await.map_err(super::unavailable)?;
                model::live(binding)?;
                Ok(facts)
            })
            .await
    }
    pub async fn enroll(
        &self,
        request: proto::EnrollRequest,
        budget: &OperationBudget,
    ) -> Result<proto::CheckResponse, Status> {
        budget.run(async {
            let binding=request.binding.as_ref().ok_or_else(model::invalid)?;let view=request.view.as_ref().ok_or_else(model::invalid)?;
            if request.subject_ids.len()>100{return Err(model::invalid());}
            let mut tx=self.transaction(budget).await?;let lease=self.active(&mut tx,view,binding,budget).await?;
            let mut subjects:BTreeSet<_>=lease.subjects.iter().copied().collect();let mut new=BTreeSet::new();
            for id in &request.subject_ids {let id=model::uuid(id)?;if !new.insert(id){return Err(model::invalid());}subjects.insert(id);}
            let actor=model::uuid(&binding.actor_id)?;if subjects.iter().filter(|id|**id!=actor).count()>100{return Err(model::invalid());}
            let subjects:Vec<_>=subjects.into_iter().collect();budget.sql(&mut tx).await?;
            let facts=owner::facts(&mut tx,&lease.binding,&subjects).await?;if facts.revision!=lease.revision{return Err(model::unavailable());}
            sqlx::query("update public.concert_owner_guard_leases set enrolled_subjects=$2 where guard_id=$1").bind(lease.id).bind(subjects).execute(&mut *tx).await.map_err(super::unavailable)?;
            tx.commit().await.map_err(super::unavailable)?;model::live(binding)?;Ok(facts)
        }).await
    }
    pub async fn track(
        &self,
        request: proto::TrackRequest,
        caller: Workload,
        budget: &OperationBudget,
    ) -> Result<proto::TrackResponse, Status> {
        budget.run(async {
            let binding=request.binding.as_ref().ok_or_else(model::invalid)?;let view=request.view.as_ref().ok_or_else(model::invalid)?;let identity=request.transaction.as_ref().ok_or_else(model::invalid)?;model::identity(identity)?;
            let producer=proto::Producer::try_from(identity.producer).map_err(|_|model::invalid())?;
            if !matches!((producer,caller),(proto::Producer::Concert,Workload::Concert)|(proto::Producer::Post,Workload::Post)|(proto::Producer::Profile,Workload::Profile)){return Err(model::denied());}
            if !self.resolver.configured(producer){return Err(model::unavailable());}
            let mut tx=self.transaction(budget).await?;let lease=self.active(&mut tx,view,binding,budget).await?;let digest=model::identity_digest(identity)?;
            let existing:Option<(Uuid,serde_json::Value)>=sqlx::query_as("select registration_id,identity from public.concert_owner_guard_participants where guard_id=$1 and producer=$2 and identity_digest=$3")
                .bind(lease.id).bind(producer.as_str_name()).bind(digest.to_vec()).fetch_optional(&mut *tx).await.map_err(super::unavailable)?;
            let registration=if let Some((id,stored))=existing {if model::decode::<proto::TransactionIdentity>(stored)?!=*identity{return Err(model::denied());}id}else{
                let count:i64=sqlx::query_scalar("select count(*) from public.concert_owner_guard_participants where guard_id=$1").bind(lease.id).fetch_one(&mut *tx).await.map_err(super::unavailable)?;
                if count>=MAX_PARTICIPANTS{return Err(Status::resource_exhausted("Owner participant capacity exhausted"));}
                let id=Uuid::new_v4();budget.sql(&mut tx).await?;
                sqlx::query("insert into public.concert_owner_guard_participants(registration_id,guard_id,producer,identity_digest,identity) values($1,$2,$3,$4,$5)")
                    .bind(id).bind(lease.id).bind(producer.as_str_name()).bind(digest.to_vec()).bind(model::json(identity)?).execute(&mut *tx).await.map_err(super::unavailable)?;id
            };
            tx.commit().await.map_err(super::unavailable)?;model::live(binding)?;Ok(proto::TrackResponse{registration_id:registration.to_string()})
        }).await
    }
    pub async fn complete(
        &self,
        request: proto::CompleteRequest,
        budget: &OperationBudget,
    ) -> Result<proto::CompleteResponse, Status> {
        budget.run(async {
            let binding=request.binding.as_ref().ok_or_else(model::invalid)?;model::binding(binding)?;let view=request.view.as_ref().ok_or_else(model::invalid)?;let id=model::view(view,binding)?;
            let mut tx=self.transaction(budget).await?;let lease=self.lock(&mut tx,id,false).await?.ok_or_else(model::denied)?;lease.authorize(view,binding)?;
            if !model::secret_matches(&request.completion_secret,&lease.completion_hash){return Err(model::denied());}
            if lease.state=="COMPLETED"{return Ok(proto::CompleteResponse{completed:true});}
            if lease.state=="ACTIVE"{sqlx::query("update public.concert_owner_guard_leases set state='SEALED',sealed_at=clock_timestamp() where guard_id=$1").bind(id).execute(&mut *tx).await.map_err(super::unavailable)?;}
            else if lease.state!="SEALED"{return Err(model::unavailable());}
            tx.commit().await.map_err(super::unavailable)?;
            Ok(proto::CompleteResponse{completed:self.settle(id,budget).await?})
        }).await
    }
    async fn release(&self, lease: &Lease, budget: &OperationBudget) -> Result<(), Status> {
        let mut connection = self
            .pool
            .acquire()
            .await
            .map_err(super::unavailable)?
            .detach();
        if self.prepared(&mut connection, lease).await? {
            budget.remaining()?;
            sqlx::query(&format!("rollback prepared '{}'", lease.gid))
                .execute(&mut connection)
                .await
                .map_err(super::unavailable)?;
        }
        Ok(())
    }
    async fn settle(&self, id: Uuid, budget: &OperationBudget) -> Result<bool, Status> {
        let mut tx = self.transaction(budget).await?;
        let lease = match self.lock(&mut tx, id, true).await? {
            Some(lease) => lease,
            None => return Ok(false),
        };
        if lease.state == "COMPLETED" || lease.state == "ABANDONED" {
            return Ok(true);
        }
        if lease.state != "SEALED" {
            return Ok(false);
        }
        if lease.scope_settlement.is_none() {
            let result = self
                .resolver
                .resolve_scope(
                    proto::ResolveScopeRequest {
                        scope: Some(lease.scope.clone()),
                    },
                    budget,
                )
                .await?;
            if result.scope.as_ref() != Some(&lease.scope) {
                return Err(model::unavailable());
            }
            if !result.sealed_and_finished {
                return Ok(false);
            }
            budget.sql(&mut tx).await?;
            sqlx::query("update public.concert_owner_guard_leases set scope_settlement=$2 where guard_id=$1").bind(id).bind(model::json(&result)?).execute(&mut *tx).await.map_err(super::unavailable)?;
        }
        let pending=sqlx::query("select registration_id,identity from public.concert_owner_guard_participants where guard_id=$1 and state='PENDING' order by registration_id").bind(id).fetch_all(&mut *tx).await.map_err(super::unavailable)?;
        // Commit scope proof and each terminal participant independently. A
        // slow later participant cannot discard earlier durable progress on the
        // two-second cleanup budget. The sealed lease serializes each step.
        tx.commit().await.map_err(super::unavailable)?;
        for row in pending {
            let mut tx = self.transaction(budget).await?;
            let current = self
                .lock(&mut tx, id, false)
                .await?
                .ok_or_else(model::unavailable)?;
            if current.state == "COMPLETED" {
                return Ok(true);
            }
            if current.state != "SEALED" || current.scope_settlement.is_none() {
                return Err(model::unavailable());
            }
            let registration: Uuid = row.try_get("registration_id").map_err(super::unavailable)?;
            let state:String=sqlx::query_scalar("select state from public.concert_owner_guard_participants where registration_id=$1 and guard_id=$2").bind(registration).bind(id).fetch_one(&mut *tx).await.map_err(super::unavailable)?;
            if state != "PENDING" {
                continue;
            }
            let identity: proto::TransactionIdentity =
                model::decode(row.try_get("identity").map_err(super::unavailable)?)?;
            let result = self
                .resolver
                .resolve_transaction(
                    proto::ResolveTransactionRequest {
                        binding: Some(lease.binding.clone()),
                        transaction: Some(identity.clone()),
                        registration_id: registration.to_string(),
                    },
                    budget,
                )
                .await?;
            if result.transaction.as_ref() != Some(&identity)
                || !model::constant_eq(&result.binding_digest, &lease.digest)
            {
                return Err(model::unavailable());
            }
            let state = match proto::TransactionStatus::try_from(result.status) {
                Ok(proto::TransactionStatus::Committed)
                    if !result.receipt_evidence.is_empty()
                        && result.receipt_evidence.len() <= 32768 =>
                {
                    "COMMITTED"
                }
                Ok(proto::TransactionStatus::Aborted) => "ABORTED",
                Ok(proto::TransactionStatus::InProgress | proto::TransactionStatus::Unknown) => {
                    return Ok(false)
                }
                _ => return Err(model::unavailable()),
            };
            budget.sql(&mut tx).await?;
            sqlx::query("update public.concert_owner_guard_participants set state=$2,settlement=$3,settled_at=clock_timestamp() where registration_id=$1").bind(registration).bind(state).bind(model::json(&result)?).execute(&mut *tx).await.map_err(super::unavailable)?;
            tx.commit().await.map_err(super::unavailable)?;
        }
        // Every actual terminal proof is durable BEFORE releasing source locks.
        let mut tx = self.transaction(budget).await?;
        let lease = self
            .lock(&mut tx, id, false)
            .await?
            .ok_or_else(model::unavailable)?;
        if lease.state == "COMPLETED" {
            return Ok(true);
        }
        let outstanding:i64=sqlx::query_scalar("select count(*) from public.concert_owner_guard_participants where guard_id=$1 and state='PENDING'").bind(id).fetch_one(&mut *tx).await.map_err(super::unavailable)?;
        if lease.state != "SEALED" || lease.scope_settlement.is_none() || outstanding != 0 {
            return Err(model::unavailable());
        }
        self.release(&lease, budget).await?;
        budget.sql(&mut tx).await?;
        sqlx::query("update public.concert_owner_guard_leases set state='COMPLETED',completed_at=clock_timestamp() where guard_id=$1").bind(id).execute(&mut *tx).await.map_err(super::unavailable)?;
        tx.commit().await.map_err(super::unavailable)?;
        Ok(true)
    }
    async fn abort_preparer(
        &self,
        control: &mut Transaction<'_, Postgres>,
        lease: &Lease,
        budget: &OperationBudget,
    ) -> Result<bool, Status> {
        if self.prepared(control, lease).await? {
            self.release(lease, budget).await?;
            return Ok(true);
        }
        let Some((pid, started, xid)) = &lease.preparer else {
            return Ok(true);
        };
        budget.sql(control).await?;
        // Exact service-owned backend identity only. PID absence alone is not an
        // outcome; termination racing PREPARE is resolved by xid/prepared state.
        sqlx::query("select pg_terminate_backend(pid) from pg_stat_activity where pid=$1 and backend_start=$2 and datname=current_database() and usename=current_user")
            .bind(pid).bind(started).execute(&mut **control).await.map_err(super::unavailable)?;
        loop {
            budget.sql(control).await?;
            if self.prepared(control, lease).await? {
                self.release(lease, budget).await?;
                return Ok(true);
            }
            let outcome: Option<String> =
                sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
                    .bind(xid)
                    .fetch_one(&mut **control)
                    .await
                    .map_err(super::unavailable)?;
            match outcome.as_deref() {
                Some("aborted") => return Ok(true),
                Some("in progress") => {
                    tokio::time::sleep(std::time::Duration::from_millis(5)).await
                }
                _ => return Ok(false),
            }
        }
    }
    /// One bounded recovery sweep. Busy acquisitions are skipped, unknown
    /// producer outcomes remain quarantined and all such rows consume capacity.
    pub async fn recover(&self, budget: &OperationBudget) -> Result<u32, Status> {
        budget.run(async {
            let mut tx=self.transaction(budget).await?;
            let ids:Vec<Uuid>=sqlx::query_scalar("select guard_id from public.concert_owner_guard_leases where state='SEALED' or (state in ('PREPARING','ACTIVE') and expires_at<=clock_timestamp()) order by created_at limit 16").fetch_all(&mut *tx).await.map_err(super::unavailable)?;
            tx.commit().await.map_err(super::unavailable)?;let mut finished=0;
            for id in ids {
                let mut tx=self.transaction(budget).await?;let lease=match self.lock(&mut tx,id,true).await?{Some(lease)=>lease,None=>continue};
                if lease.state=="PREPARING" {
                    if !self.abort_preparer(&mut tx,&lease,budget).await?{continue;}budget.sql(&mut tx).await?;
                    sqlx::query("update public.concert_owner_guard_leases set state='ABANDONED',completed_at=clock_timestamp() where guard_id=$1").bind(id).execute(&mut *tx).await.map_err(super::unavailable)?;
                    tx.commit().await.map_err(super::unavailable)?;finished+=1;continue;
                }
                if lease.state=="ACTIVE" {
                    sqlx::query("update public.concert_owner_guard_leases set state='SEALED',sealed_at=clock_timestamp() where guard_id=$1 and expires_at<=clock_timestamp()")
                        .bind(id).execute(&mut *tx).await.map_err(super::unavailable)?;
                }
                tx.commit().await.map_err(super::unavailable)?;
                match self.settle(id,budget).await {Ok(true)=>finished+=1,Ok(false)=>{},Err(e) if e.code()==tonic::Code::Unavailable=>{},Err(e)=>return Err(e)}
            }Ok(finished)
        }).await
    }
}
