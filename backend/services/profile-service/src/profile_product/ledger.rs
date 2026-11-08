//! Durable profile scope lifecycle.
//!
//! The profile transport must behave like the concert transport under a lost
//! reply: a scope that was registered and published but never settled stays
//! visible as unresolved rather than being silently completed. This ledger is
//! the durable record of that lifecycle.
//!
//! Phases:
//!
//! * `ACTIVE` — registered by `acquire`, before any producer work;
//! * `SEALED` — the delivery capability has been published. Owner exclusion is
//!   held and the producer may run; the scope still cannot be read as complete;
//! * `ROOT_FINISHED` — the Gateway reported the actual HTTP body terminal and
//!   the delivery secret matched. This is the only terminal phase.
use crate::adapters::out::profile_owner_guard::{ProfileGuardBinding, ProfileProducerScope};
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use tonic::Status;

fn unavailable() -> Status {
    Status::unavailable("Profile scope lifecycle unavailable")
}

/// Hash a delivery secret before it is stored. The raw secret never reaches the
/// database, so a ledger read cannot replay a completion.
pub fn secret_hash(secret: &[u8]) -> Result<[u8; 32], Status> {
    if secret.len() != 32 {
        return Err(Status::permission_denied(
            "Invalid profile delivery capability",
        ));
    }
    let mut hash = Sha256::new();
    hash.update(b"mcr profile delivery capability v1\0");
    hash.update(secret);
    Ok(hash.finalize().into())
}

#[derive(Clone)]
pub struct ProfileScopeLedger {
    pool: PgPool,
}

impl ProfileScopeLedger {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub async fn register(
        &self,
        scope: &ProfileProducerScope,
        binding: &ProfileGuardBinding,
    ) -> Result<(), Status> {
        scope.validate()?;
        // The ledger row is keyed by the exact binding fingerprint, so a second
        // registration of a different request under the same scope id is a
        // conflict rather than an overwrite.
        sqlx::query(
            "insert into profile_product_scopes(instance_id,scope_id,binding_digest,actor_id,\
             profile_id,operation,http_method,http_target,phase) \
             values($1,$2,$3,$4,$5,$6,$7,$8,'ACTIVE') \
             on conflict(instance_id,scope_id) do nothing",
        )
        .bind(scope.instance_id)
        .bind(scope.scope_id)
        .bind(scope.binding_digest.as_slice())
        .bind(binding.actor())
        .bind(binding.profile())
        .bind(binding.operation())
        .bind(&*binding.http_method)
        .bind(&*binding.http_target)
        .execute(&self.pool)
        .await
        .map_err(|_| unavailable())?;
        let stored: Option<Vec<u8>> = sqlx::query_scalar(
            "select binding_digest from profile_product_scopes where instance_id=$1 and scope_id=$2",
        )
        .bind(scope.instance_id)
        .bind(scope.scope_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| unavailable())?;
        if stored.as_deref() != Some(scope.binding_digest.as_slice()) {
            return Err(Status::permission_denied("Profile scope rebound"));
        }
        Ok(())
    }

    pub async fn bind_delivery(
        &self,
        scope: &ProfileProducerScope,
        secret: &[u8; 32],
    ) -> Result<(), Status> {
        let hash = secret_hash(secret)?;
        // The scope lifecycle guard refuses an UPDATE that does not change the
        // phase, so publication advances ACTIVE -> SEALED. SEALED is the state
        // the concert ledger calls "producer work ended, owner exclusion still
        // held"; only CompleteResponse may move it to ROOT_FINISHED.
        let updated = sqlx::query(
            "update profile_product_scopes set delivery_secret_hash=$3,phase='SEALED',\
             updated_at=clock_timestamp() \
             where instance_id=$1 and scope_id=$2 and phase='ACTIVE' and delivery_secret_hash is null",
        )
        .bind(scope.instance_id)
        .bind(scope.scope_id)
        .bind(hash.as_slice())
        .execute(&self.pool)
        .await
        .map_err(|_| unavailable())?
        .rows_affected();
        if updated != 1 {
            return Err(Status::permission_denied(
                "Profile delivery already published",
            ));
        }
        Ok(())
    }

    pub async fn verify_delivery(
        &self,
        scope: &ProfileProducerScope,
        secret: &[u8],
    ) -> Result<(), Status> {
        let expected: Option<Vec<u8>> = sqlx::query_scalar(
            "select delivery_secret_hash from profile_product_scopes \
             where instance_id=$1 and scope_id=$2 and binding_digest=$3",
        )
        .bind(scope.instance_id)
        .bind(scope.scope_id)
        .bind(scope.binding_digest.as_slice())
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| unavailable())?;
        let expected = expected.ok_or_else(|| Status::permission_denied("Unknown profile scope"))?;
        if expected.as_slice() != secret_hash(secret)? {
            return Err(Status::permission_denied(
                "Invalid profile delivery capability",
            ));
        }
        Ok(())
    }

    pub async fn delivery_completed(&self, scope: &ProfileProducerScope) -> Result<bool, Status> {
        sqlx::query_scalar(
            "select phase='ROOT_FINISHED' from profile_product_scopes \
             where instance_id=$1 and scope_id=$2",
        )
        .bind(scope.instance_id)
        .bind(scope.scope_id)
        .fetch_optional(&self.pool)
        .await
        .map_err(|_| unavailable())?
        .ok_or_else(|| Status::permission_denied("Unknown profile scope"))
    }

    pub async fn record_delivery_completed(
        &self,
        scope: &ProfileProducerScope,
    ) -> Result<(), Status> {
        let updated = sqlx::query(
            "update profile_product_scopes set phase='ROOT_FINISHED',completed_at=clock_timestamp(),\
             updated_at=clock_timestamp() where instance_id=$1 and scope_id=$2 \
             and phase in ('ACTIVE','SEALED') and delivery_secret_hash is not null",
        )
        .bind(scope.instance_id)
        .bind(scope.scope_id)
        .execute(&self.pool)
        .await
        .map_err(|_| unavailable())?
        .rows_affected();
        if updated != 1 {
            return Err(Status::unavailable(
                "Profile delivery completion unresolved",
            ));
        }
        Ok(())
    }

}

/// Canonical scope key parser used by the delivery service.
pub fn scope_key(instance: &str, scope: &str, digest: &[u8]) -> Result<ProfileProducerScope, Status> {
    let value = ProfileProducerScope {
        instance_id: super::canonical_uuid(instance)?,
        scope_id: super::canonical_uuid(scope)?,
        binding_digest: digest
            .try_into()
            .map_err(|_| Status::permission_denied("Invalid profile delivery capability"))?,
    };
    value.validate()?;
    Ok(value)
}

