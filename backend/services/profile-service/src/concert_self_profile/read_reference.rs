//! A read has an actual rollback outcome, never a dummy write receipt/version.
use super::{
    identity::{native_status, NativeStatus, TransactionIdentity},
    model::Error,
};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use sqlx::{PgPool, Row};
use uuid::Uuid;
pub(super) struct ReadReference {
    pub id: Uuid,
    pub binding_proto: Vec<u8>,
    pub binding_digest: [u8; 32],
    pub identity: TransactionIdentity,
}
impl ReadReference {
    pub async fn register(&self, pool: &PgPool) -> Result<(), Error> {
        self.identity.validate()?;
        if self.id.is_nil()
            || self.binding_digest == [0; 32]
            || self.binding_proto.is_empty()
            || self.binding_proto.len() > 32768
        {
            return Err(Error::Invalid);
        }
        sqlx::query("insert into concert_self_profile_read_references(reference_id,binding_proto,binding_digest,transaction_identity) values($1,$2,$3,$4)")
            .bind(self.id).bind(&self.binding_proto).bind(self.binding_digest.as_slice())
            .bind(serde_json::to_value(&self.identity).map_err(|_|Error::Unavailable)?)
            .execute(pool).await.map_err(|_|Error::Unavailable)?;
        Ok(())
    }
    pub async fn load(pool: &PgPool, id: Uuid) -> Result<Option<Self>, Error> {
        let row=sqlx::query("select binding_proto,binding_digest,transaction_identity from concert_self_profile_read_references where reference_id=$1").bind(id).fetch_optional(pool).await.map_err(|_|Error::Unavailable)?;
        row.map(|r| {
            let identity: serde_json::Value = r
                .try_get("transaction_identity")
                .map_err(|_| Error::Unavailable)?;
            let identity: TransactionIdentity =
                serde_json::from_value(identity).map_err(|_| Error::Unavailable)?;
            identity.validate()?;
            let digest: Vec<u8> = r
                .try_get("binding_digest")
                .map_err(|_| Error::Unavailable)?;
            Ok(Self {
                id,
                binding_proto: r.try_get("binding_proto").map_err(|_| Error::Unavailable)?,
                binding_digest: digest.try_into().map_err(|_| Error::Unavailable)?,
                identity,
            })
        })
        .transpose()
    }
    pub async fn status(
        &self,
        pool: &PgPool,
        deadline: AuthorityRpcDeadline,
    ) -> Result<NativeStatus, Error> {
        // A committed read reference is anomalous, not a committed write effect.
        match native_status(pool, &self.identity, self.id, deadline).await? {
            NativeStatus::Aborted => Ok(NativeStatus::Aborted),
            NativeStatus::InProgress => Ok(NativeStatus::InProgress),
            _ => Ok(NativeStatus::Unknown),
        }
    }
}
