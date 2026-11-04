//! Durable producer evidence, written before each potentially ambiguous Track.
//! This never releases an owner guard or proves Concert HTTP body completion.
use super::{
    authority_proto as wire,
    read_admission::ReadAdmission,
    read_clients::ReadClients,
    read_transaction::{self, CapturedReadTransaction, ReadTransactionState},
    storage,
};
use platform::grpc_auth::Workload;
use post_domain::concert_feed::Error;
use prost::Message;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

#[derive(Clone, Copy)]
pub(crate) enum RegistrationOwner {
    Auth,
    Friend,
}
#[derive(Clone)]
pub(crate) struct TrackingStore {
    pool: PgPool,
    instance: Uuid,
}
pub(crate) struct TrackedRead {
    store: TrackingStore,
    identity: CapturedReadTransaction,
    binding_digest: Vec<u8>,
    witness: [u8; 32],
}
/// Constructed only after BOTH actual acknowledgements have been committed to
/// the durable producer table. Dropping it is never a completion operation.
pub(crate) struct RegisteredRead {
    tracked: TrackedRead,
}
impl TrackingStore {
    pub(crate) fn new(pool: PgPool, instance: Uuid) -> Result<Self, Error> {
        if instance.is_nil() {
            return Err(Error::Invalid);
        }
        Ok(Self { pool, instance })
    }
    pub(crate) fn instance(&self) -> Uuid {
        self.instance
    }
    /// A separate, committed control transaction. The protected connection is
    /// already open but has not read protected rows. Requires pool capacity >=2.
    pub(crate) async fn begin(
        &self,
        a: &ReadAdmission,
        identity: CapturedReadTransaction,
    ) -> Result<TrackedRead, Error> {
        a.check()?;
        if identity.instance() != self.instance {
            return Err(Error::Authority);
        }
        let binding = a.authority.binding.as_ref().ok_or(Error::Authority)?;
        let auth = a.authority.auth.as_ref().ok_or(Error::Authority)?;
        let friend = a.authority.friend.as_ref().ok_or(Error::Authority)?;
        let witness = super::digest(&(Uuid::new_v4(), Uuid::new_v4()))?;
        let mut tx = self.pool.begin().await.map_err(storage)?;
        // Serialize admission to enforce the bounded unresolved-work limit.
        sqlx::query("select pg_advisory_xact_lock(7728319491591)")
            .execute(&mut *tx)
            .await
            .map_err(storage)?;
        let pending: i64 = sqlx::query_scalar(
            "select count(*) from post_concert_read_tracking where terminal_status is null",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(storage)?;
        if pending >= 4096 {
            return Err(Error::Unavailable);
        }
        sqlx::query("insert into post_concert_read_tracking(settlement_reference,producer_instance_id,transaction_bytes,binding_bytes,binding_digest,auth_guard_id,friend_guard_id,expires_at,witness) values($1,$2,$3,$4,$5,$6,$7,$8,$9)")
            .bind(identity.settlement_reference().to_vec()).bind(self.instance).bind(identity.wire().encode_to_vec())
            .bind(binding.encode_to_vec()).bind(&auth.binding_digest)
            .bind(id(&auth.guard_id)?).bind(id(&friend.guard_id)?).bind(a.context.expires_at).bind(witness.to_vec())
            .execute(&mut *tx).await.map_err(storage)?;
        tx.commit().await.map_err(storage)?;
        a.check()?;
        Ok(TrackedRead {
            store: self.clone(),
            identity,
            binding_digest: auth.binding_digest.clone(),
            witness,
        })
    }
    /// Only a workload verified by the guarded settlement server may call this.
    /// A received UUID is a lookup binding, never the authority for an outcome.
    pub(crate) async fn resolve(
        &self,
        caller: Workload,
        request: wire::ResolveTransactionRequest,
        stop: tokio::time::Instant,
    ) -> Result<wire::ResolveTransactionResponse, Error> {
        let owner = match caller {
            Workload::Auth => RegistrationOwner::Auth,
            Workload::Friend => RegistrationOwner::Friend,
            _ => return Err(Error::Authority),
        };
        let incoming = request.transaction.ok_or(Error::Invalid)?;
        let binding = request.binding.ok_or(Error::Invalid)?;
        let registration = id(&request.registration_id)?;
        if incoming.settlement_reference.len() != 32
            || incoming.producer != wire::Producer::Post as i32
        {
            return Err(Error::Authority);
        }
        let row =
            sqlx::query("select * from post_concert_read_tracking where settlement_reference=$1")
                .bind(&incoming.settlement_reference)
                .fetch_optional(&self.pool)
                .await
                .map_err(storage)?
                .ok_or(Error::Authority)?;
        let bytes: Vec<u8> = row.try_get("transaction_bytes").map_err(storage)?;
        let stored_binding: Vec<u8> = row.try_get("binding_bytes").map_err(storage)?;
        let binding_digest: Vec<u8> = row.try_get("binding_digest").map_err(storage)?;
        let (attempted, ack): (bool, Option<Uuid>) = match owner {
            RegistrationOwner::Auth => (
                row.try_get("auth_attempted").map_err(storage)?,
                row.try_get("auth_registration_id").map_err(storage)?,
            ),
            RegistrationOwner::Friend => (
                row.try_get("friend_attempted").map_err(storage)?,
                row.try_get("friend_registration_id").map_err(storage)?,
            ),
        };
        if !attempted
            || ack.is_some_and(|ack| ack != registration)
            || bytes != incoming.encode_to_vec()
            || stored_binding != binding.encode_to_vec()
        {
            return Err(Error::Authority);
        }
        // An absent acknowledgement is allowed ONLY because the actual owner
        // presents its durable registration for this exact pre-recorded attempt.
        let identity = CapturedReadTransaction::from_record(&bytes)?;
        let state = read_transaction::resolve(&self.pool, &identity, self.instance, stop).await;
        let mut evidence = vec![];
        let status = match state {
            ReadTransactionState::InProgress => wire::TransactionStatus::InProgress,
            ReadTransactionState::Aborted => wire::TransactionStatus::Aborted,
            ReadTransactionState::Unknown => wire::TransactionStatus::Unknown,
            ReadTransactionState::Committed => {
                let expected: Vec<u8> = row.try_get("witness").map_err(storage)?;
                let found:Option<Vec<u8>>=sqlx::query_scalar("select witness from post_concert_read_terminal_witness where settlement_reference=$1 and transaction_bytes=$2 and binding_digest=$3 and witness=$4")
                    .bind(identity.settlement_reference().to_vec()).bind(&bytes).bind(&binding_digest).bind(&expected)
                    .fetch_optional(&self.pool).await.map_err(storage)?;
                match found {
                    Some(witness) if witness.len() == 32 => {
                        evidence = witness;
                        wire::TransactionStatus::Committed
                    }
                    _ => wire::TransactionStatus::Unknown,
                }
            }
        };
        if matches!(
            status,
            wire::TransactionStatus::Committed | wire::TransactionStatus::Aborted
        ) {
            let label = if status == wire::TransactionStatus::Committed {
                "COMMITTED"
            } else {
                "ABORTED"
            };
            sqlx::query("update post_concert_read_tracking set terminal_status=$1 where settlement_reference=$2 and producer_instance_id=$3 and (terminal_status is null or terminal_status=$1)")
                .bind(label).bind(identity.settlement_reference().to_vec()).bind(self.instance).execute(&self.pool).await.map_err(storage)?;
        }
        Ok(wire::ResolveTransactionResponse {
            transaction: Some(incoming),
            binding_digest,
            status: status as i32,
            receipt_evidence: evidence,
        })
    }
}
impl TrackedRead {
    pub(crate) async fn register(
        self,
        clients: &ReadClients,
        a: &ReadAdmission,
    ) -> Result<RegisteredRead, Error> {
        for owner in [RegistrationOwner::Auth, RegistrationOwner::Friend] {
            self.attempt(owner).await?;
            let ack = clients.track_owner(a, &self.identity, owner).await?;
            self.acknowledge(owner, ack).await?;
            a.check()?;
        }
        Ok(RegisteredRead { tracked: self })
    }
    async fn attempt(&self, owner: RegistrationOwner) -> Result<(), Error> {
        let sql=match owner {
            RegistrationOwner::Auth=>"update post_concert_read_tracking set auth_attempted=true where settlement_reference=$1 and producer_instance_id=$2 and terminal_status is null",
            RegistrationOwner::Friend=>"update post_concert_read_tracking set friend_attempted=true where settlement_reference=$1 and producer_instance_id=$2 and auth_registration_id is not null and terminal_status is null",
        };
        let result = sqlx::query(sql)
            .bind(self.identity.settlement_reference().to_vec())
            .bind(self.store.instance)
            .execute(&self.store.pool)
            .await
            .map_err(storage)?;
        if result.rows_affected() != 1 {
            return Err(Error::Authority);
        }
        Ok(())
    }
    async fn acknowledge(&self, owner: RegistrationOwner, registration: Uuid) -> Result<(), Error> {
        let sql=match owner {
            RegistrationOwner::Auth=>"update post_concert_read_tracking set auth_registration_id=$1 where settlement_reference=$2 and auth_attempted and (auth_registration_id is null or auth_registration_id=$1)",
            RegistrationOwner::Friend=>"update post_concert_read_tracking set friend_registration_id=$1 where settlement_reference=$2 and friend_attempted and (friend_registration_id is null or friend_registration_id=$1)",
        };
        let result = sqlx::query(sql)
            .bind(registration)
            .bind(self.identity.settlement_reference().to_vec())
            .execute(&self.store.pool)
            .await
            .map_err(storage)?;
        if result.rows_affected() != 1 {
            return Err(Error::Authority);
        }
        Ok(())
    }
}
impl RegisteredRead {
    pub(crate) fn binding_digest(&self) -> &[u8] {
        &self.tracked.binding_digest
    }
    /// The witness is part of the SAME SQL transaction used by query::*_in.
    /// Only its actual COMMITTED outcome can make this row visible to settlement.
    pub(crate) async fn witness(&self, tx: &mut Transaction<'_, Postgres>) -> Result<(), Error> {
        let t = &self.tracked;
        let result=sqlx::query("insert into post_concert_read_terminal_witness(settlement_reference,transaction_bytes,binding_digest,witness) select $1,$2,$3,$4 where pg_current_xact_id()::text=$5 and pg_backend_pid()=$6")
            .bind(t.identity.settlement_reference().to_vec()).bind(t.identity.wire().encode_to_vec()).bind(&t.binding_digest).bind(t.witness.to_vec())
            .bind(t.identity.full_xid()).bind(t.identity.backend_pid()).execute(&mut **tx).await.map_err(storage)?;
        if result.rows_affected() != 1 {
            return Err(Error::Authority);
        }
        Ok(())
    }
}
fn id(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or(Error::Authority)
}
