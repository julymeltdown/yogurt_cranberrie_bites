//! Durable operation identity and native outcome checks. No time/absence decision.
use super::{
    identity::{native_status, prepared_gid, NativeStatus, TransactionIdentity},
    model::{Error, Version},
    store::exact_receipt,
};
use chrono::{DateTime, Utc};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use sqlx::{PgPool, Row};
use uuid::Uuid;

#[derive(Clone)]
pub(super) struct Operation {
    pub id: Uuid,
    pub actor: Uuid,
    pub event: Uuid,
    pub family: Uuid,
    pub binding_digest: [u8; 32],
    pub binding_proto: Vec<u8>,
    pub command_digest: [u8; 32],
    pub expected: Version,
    pub identity: TransactionIdentity,
}
impl std::fmt::Debug for Operation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Operation([REDACTED])")
    }
}
impl Operation {
    pub async fn register(
        &self,
        pool: &PgPool,
        deadline: AuthorityRpcDeadline,
    ) -> Result<(), Error> {
        self.identity.validate()?;
        if [self.id, self.actor, self.event, self.family]
            .iter()
            .any(Uuid::is_nil)
            || self.binding_digest == [0; 32]
            || self.command_digest == [0; 32]
            || self.binding_proto.is_empty()
            || self.binding_proto.len() > 32768
        {
            return Err(Error::Invalid);
        }
        let work = async {
            let result=sqlx::query("insert into concert_self_profile_operations(operation_id,actor_id,event_id,session_family_id,request_id,binding_digest,binding_proto,command_digest,expected_epoch,expected_revision,prepared_gid,full_xid,system_identifier,database_oid,backend_pid,backend_started_at,producer_instance_id,effect_binding_digest) values($1,$2,$3,$4,$1,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16,$17) on conflict(operation_id) do nothing")
                .bind(self.id).bind(self.actor).bind(self.event).bind(self.family)
                .bind(self.binding_digest.as_slice()).bind(&self.binding_proto).bind(self.command_digest.as_slice())
                .bind(self.expected.epoch()).bind(self.expected.revision()).bind(prepared_gid(self.id)?)
                .bind(&self.identity.full_xid).bind(&self.identity.system_identifier).bind(i64::from(self.identity.database_oid))
                .bind(self.identity.backend_pid).bind(self.identity.backend_started).bind(self.identity.producer_instance).bind(self.identity.effect_digest.as_slice())
                .execute(pool).await.map_err(unavailable)?;
            // A duplicate operation is resolved against its original transaction;
            // it must never allocate another protected effect under a fresh XID.
            if result.rows_affected() != 1 {
                return Err(Error::Conflict);
            }
            Ok(())
        };
        bounded(deadline, work).await
    }
    pub async fn load(
        pool: &PgPool,
        id: Uuid,
        deadline: AuthorityRpcDeadline,
    ) -> Result<Option<Self>, Error> {
        bounded(deadline,async {
            let row=sqlx::query("select operation_id,actor_id,event_id,session_family_id,binding_digest,binding_proto,command_digest,expected_epoch,expected_revision,full_xid,system_identifier,database_oid,backend_pid,backend_started_at,producer_instance_id,effect_binding_digest from concert_self_profile_operations where operation_id=$1").bind(id).fetch_optional(pool).await.map_err(unavailable)?;
            row.map(|r| {
                let bytes=|name:&str|->Result<[u8;32],Error>{let v:Vec<u8>=r.try_get(name).map_err(unavailable)?;v.try_into().map_err(|_|Error::Unavailable)};
                let oid:i64=r.try_get("database_oid").map_err(unavailable)?;
                let identity=TransactionIdentity{full_xid:r.try_get("full_xid").map_err(unavailable)?,system_identifier:r.try_get("system_identifier").map_err(unavailable)?,database_oid:u32::try_from(oid).map_err(|_|Error::Unavailable)?,backend_pid:r.try_get("backend_pid").map_err(unavailable)?,backend_started:r.try_get::<DateTime<Utc>,_>("backend_started_at").map_err(unavailable)?,producer_instance:r.try_get("producer_instance_id").map_err(unavailable)?,effect_digest:bytes("effect_binding_digest")?};
                identity.validate()?;
                Ok(Self{id:r.try_get("operation_id").map_err(unavailable)?,actor:r.try_get("actor_id").map_err(unavailable)?,event:r.try_get("event_id").map_err(unavailable)?,family:r.try_get("session_family_id").map_err(unavailable)?,binding_digest:bytes("binding_digest")?,binding_proto:r.try_get("binding_proto").map_err(unavailable)?,command_digest:bytes("command_digest")?,expected:Version::new(r.try_get("expected_epoch").map_err(unavailable)?,r.try_get("expected_revision").map_err(unavailable)?)?,identity})
            }).transpose()
        }).await
    }
    pub async fn status(
        &self,
        pool: &PgPool,
        recovery: AuthorityRpcDeadline,
    ) -> Result<NativeStatus, Error> {
        bounded(recovery, async {
            let status = native_status(pool, &self.identity, self.id, recovery).await?;
            if status == NativeStatus::Committed {
                let mut c = pool.acquire().await.map_err(unavailable)?;
                if exact_receipt(
                    &mut c,
                    self.id,
                    &self.identity.effect_digest,
                    &self.command_digest,
                )
                .await?
                .is_none()
                {
                    return Ok(NativeStatus::Unknown);
                }
            }
            Ok(status)
        })
        .await
    }
}
async fn bounded<T>(
    deadline: AuthorityRpcDeadline,
    future: impl std::future::Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    super::identity::remaining(deadline)?;
    let result = tokio::time::timeout_at(deadline.stop(), future)
        .await
        .map_err(|_| Error::Unavailable)?;
    super::identity::remaining(deadline)?;
    result
}
fn unavailable(_: sqlx::Error) -> Error {
    Error::Unavailable
}
