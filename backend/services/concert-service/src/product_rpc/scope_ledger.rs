//! Durable root lifecycle, independent of process-memory registry membership.
//! ROOT_FINISHED means no further root work; owner services must still resolve
//! their actual registered transactions before releasing durable exclusions.
use platform::{
    actor_delegation::owner_binding::{identity_digest, secret_hash, secret_matches, OwnerBinding},
    grpc_auth::Workload,
};
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Postgres, Transaction};
use tonic::Status;
use uuid::Uuid;

const CAPACITY: i64 = 16;
fn invalid() -> Status {
    Status::invalid_argument("Invalid product scope lifecycle binding")
}
fn unavailable(_: sqlx::Error) -> Status {
    Status::unavailable("Product scope lifecycle unavailable")
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScopeKey {
    pub instance: Uuid,
    pub scope: Uuid,
    pub binding_digest: [u8; 32],
}
impl ScopeKey {
    fn validate(&self, instance: Uuid) -> Result<(), Status> {
        if self.instance != instance || self.scope.is_nil() || self.binding_digest == [0; 32] {
            return Err(invalid());
        }
        Ok(())
    }
}
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ProducerTransaction {
    pub producer: String,
    pub system_identifier: String,
    pub database_oid: u32,
    pub full_xid: String,
    pub backend_pid: i32,
    pub backend_started_unix_micros: i64,
    pub effect_binding_digest: [u8; 32],
    pub producer_instance: Uuid,
    pub settlement_reference: Vec<u8>,
}
impl ProducerTransaction {
    pub fn digest(&self) -> Result<[u8; 32], Status> {
        identity_digest(
            &self.producer,
            &self.system_identifier,
            self.database_oid,
            &self.full_xid,
            self.backend_pid,
            self.backend_started_unix_micros,
            &self.effect_binding_digest,
            self.producer_instance,
            &self.settlement_reference,
        )
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ScopePhase {
    Unknown,
    Active,
    Sealed,
    RootFinished,
}
fn phase(value: &str) -> Result<ScopePhase, Status> {
    match value {
        "ACTIVE" => Ok(ScopePhase::Active),
        "SEALED" => Ok(ScopePhase::Sealed),
        "ROOT_FINISHED" => Ok(ScopePhase::RootFinished),
        _ => Err(Status::unavailable("Unknown product scope lifecycle")),
    }
}

#[derive(Clone)]
pub struct PgScopeLedger {
    pool: PgPool,
    instance: Uuid,
}
impl PgScopeLedger {
    pub fn new(pool: PgPool, instance: Uuid) -> Result<Self, Status> {
        if instance.is_nil() {
            return Err(invalid());
        }
        Ok(Self { pool, instance })
    }
    pub fn instance(&self) -> Uuid {
        self.instance
    }
    async fn transaction(&self) -> Result<Transaction<'_, Postgres>, Status> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        sqlx::raw_sql("SET LOCAL lock_timeout='250ms'; SET LOCAL statement_timeout='750ms'")
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        Ok(tx)
    }
    async fn lock(
        &self,
        tx: &mut Transaction<'_, Postgres>,
        key: &ScopeKey,
    ) -> Result<ScopePhase, Status> {
        key.validate(self.instance)?;
        let row: Option<(Vec<u8>, String)> = sqlx::query_as(
            "SELECT binding_digest,phase FROM concert_product_owner_scopes WHERE instance_id=$1 AND scope_id=$2 FOR UPDATE",
        )
        .bind(key.instance).bind(key.scope).fetch_optional(&mut **tx).await.map_err(unavailable)?;
        let (digest, value) =
            row.ok_or_else(|| Status::failed_precondition("Unknown product scope"))?;
        if digest != key.binding_digest {
            return Err(Status::permission_denied("Product scope mismatch"));
        }
        phase(&value)
    }
    pub async fn register(&self, key: &ScopeKey, binding: &OwnerBinding) -> Result<(), Status> {
        key.validate(self.instance)?;
        if binding.digest()? != key.binding_digest
            || binding.expires_unix_micros <= chrono::Utc::now().timestamp_micros()
        {
            return Err(invalid());
        }
        let value = serde_json::to_value(binding).map_err(|_| invalid())?;
        let mut tx = self.transaction().await?;
        // Bound pending plus active scopes atomically across all callers of this
        // configured producer instance. Never evict expired/unknown scopes.
        sqlx::query("SELECT pg_advisory_xact_lock(hashtextextended($1,0))")
            .bind(format!("mcr-product-scope:{}", self.instance))
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        let existing: Option<(Vec<u8>, serde_json::Value, String)> = sqlx::query_as(
            "SELECT binding_digest,binding,phase FROM concert_product_owner_scopes WHERE instance_id=$1 AND scope_id=$2 FOR UPDATE",
        ).bind(key.instance).bind(key.scope).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        if let Some((digest, old, state)) = existing {
            if digest != key.binding_digest || old != value || state != "ACTIVE" {
                return Err(Status::failed_precondition("Product scope cannot reopen"));
            }
        } else {
            let count: i64 = sqlx::query_scalar(
                "SELECT count(*) FROM concert_product_owner_scopes WHERE instance_id=$1 AND phase<>'ROOT_FINISHED'",
            ).bind(key.instance).fetch_one(&mut *tx).await.map_err(unavailable)?;
            if count >= CAPACITY {
                return Err(Status::resource_exhausted(
                    "Product scope capacity unavailable",
                ));
            }
            sqlx::query("INSERT INTO concert_product_owner_scopes(instance_id,scope_id,binding_digest,binding) VALUES($1,$2,$3,$4)")
                .bind(key.instance).bind(key.scope).bind(key.binding_digest.as_slice()).bind(value)
                .execute(&mut *tx).await.map_err(unavailable)?;
        }
        tx.commit().await.map_err(unavailable)
    }
    pub async fn bind_delivery(&self, key: &ScopeKey, secret: &[u8]) -> Result<(), Status> {
        let hash = secret_hash(secret)?;
        let mut tx = self.transaction().await?;
        if self.lock(&mut tx, key).await? != ScopePhase::Active {
            return Err(Status::failed_precondition("Product scope is sealed"));
        }
        let old: Option<Vec<u8>> = sqlx::query_scalar("SELECT delivery_secret_hash FROM concert_product_owner_scopes WHERE instance_id=$1 AND scope_id=$2")
            .bind(key.instance).bind(key.scope).fetch_one(&mut *tx).await.map_err(unavailable)?;
        match old {
            Some(old) if old != hash => {
                return Err(Status::permission_denied("Product delivery mismatch"))
            }
            Some(_) => (),
            None => {
                sqlx::query("UPDATE concert_product_owner_scopes SET delivery_secret_hash=$3 WHERE instance_id=$1 AND scope_id=$2")
                    .bind(key.instance).bind(key.scope).bind(hash.as_slice()).execute(&mut *tx).await.map_err(unavailable)?;
            }
        }
        tx.commit().await.map_err(unavailable)
    }
    pub async fn verify_delivery(
        &self,
        key: &ScopeKey,
        secret: &[u8],
    ) -> Result<ScopePhase, Status> {
        key.validate(self.instance)?;
        let row: Option<(Vec<u8>, Option<Vec<u8>>, String, bool)> = sqlx::query_as(
            "SELECT binding_digest,delivery_secret_hash,phase,serialized_at IS NOT NULL FROM concert_product_owner_scopes WHERE instance_id=$1 AND scope_id=$2",
        ).bind(key.instance).bind(key.scope).fetch_optional(&self.pool).await.map_err(unavailable)?;
        let (digest, hash, state, serialized) =
            row.ok_or_else(|| Status::permission_denied("Unknown product delivery"))?;
        if digest != key.binding_digest || !hash.is_some_and(|hash| secret_matches(secret, &hash)) {
            return Err(Status::permission_denied("Product delivery mismatch"));
        }
        if !serialized {
            return Err(Status::failed_precondition(
                "Product response not serialized",
            ));
        }
        phase(&state)
    }
    pub async fn register_transaction(
        &self,
        key: &ScopeKey,
        identity: &ProducerTransaction,
    ) -> Result<(), Status> {
        if identity.effect_binding_digest != key.binding_digest {
            return Err(invalid());
        }
        let digest = identity.digest()?;
        let value = serde_json::to_value(identity).map_err(|_| invalid())?;
        let mut tx = self.transaction().await?;
        if self.lock(&mut tx, key).await? != ScopePhase::Active {
            return Err(Status::failed_precondition("Product scope is sealed"));
        }
        let existing: Option<serde_json::Value> = sqlx::query_scalar("SELECT identity FROM concert_product_owner_transactions WHERE instance_id=$1 AND scope_id=$2 AND identity_digest=$3")
            .bind(key.instance).bind(key.scope).bind(digest.as_slice()).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        if let Some(old) = existing {
            if old != value {
                return Err(Status::permission_denied("Product transaction mismatch"));
            }
        } else {
            sqlx::query("INSERT INTO concert_product_owner_transactions(instance_id,scope_id,binding_digest,identity_digest,identity) VALUES($1,$2,$3,$4,$5)")
                .bind(key.instance).bind(key.scope).bind(key.binding_digest.as_slice()).bind(digest.as_slice()).bind(value)
                .execute(&mut *tx).await.map_err(unavailable)?;
        }
        tx.commit().await.map_err(unavailable)
    }
    pub async fn record_tracking(
        &self,
        key: &ScopeKey,
        identity: &ProducerTransaction,
        owner: Workload,
        registration: Uuid,
    ) -> Result<(), Status> {
        if registration.is_nil() || identity.effect_binding_digest != key.binding_digest {
            return Err(invalid());
        }
        let column = match owner {
            Workload::Auth => "auth_registration_id",
            Workload::Friend => "friend_registration_id",
            _ => return Err(Status::permission_denied("Invalid product tracking owner")),
        };
        let digest = identity.digest()?;
        let mut tx = self.transaction().await?;
        if self.lock(&mut tx, key).await? != ScopePhase::Active {
            return Err(Status::failed_precondition("Product scope is sealed"));
        }
        let query = format!("SELECT {column} FROM concert_product_owner_transactions WHERE instance_id=$1 AND scope_id=$2 AND identity_digest=$3");
        let old: Option<Option<Uuid>> = sqlx::query_scalar(&query)
            .bind(key.instance)
            .bind(key.scope)
            .bind(digest.as_slice())
            .fetch_optional(&mut *tx)
            .await
            .map_err(unavailable)?;
        match old {
            Some(Some(old)) if old == registration => (),
            Some(None) => {
                let update = format!("UPDATE concert_product_owner_transactions SET {column}=$4 WHERE instance_id=$1 AND scope_id=$2 AND identity_digest=$3");
                sqlx::query(&update)
                    .bind(key.instance)
                    .bind(key.scope)
                    .bind(digest.as_slice())
                    .bind(registration)
                    .execute(&mut *tx)
                    .await
                    .map_err(unavailable)?;
            }
            _ => {
                return Err(Status::failed_precondition(
                    "Unknown product tracking registration",
                ))
            }
        }
        tx.commit().await.map_err(unavailable)
    }
    pub async fn confirm_tracking(
        &self,
        key: &ScopeKey,
        identity: &ProducerTransaction,
        auth: Uuid,
        friend: Uuid,
    ) -> Result<(), Status> {
        if auth.is_nil() || friend.is_nil() || identity.effect_binding_digest != key.binding_digest
        {
            return Err(invalid());
        }
        let digest = identity.digest()?;
        let mut tx = self.transaction().await?;
        if self.lock(&mut tx, key).await? != ScopePhase::Active {
            return Err(Status::failed_precondition("Product scope is sealed"));
        }
        let old: Option<(Option<Uuid>, Option<Uuid>, bool)> = sqlx::query_as("SELECT auth_registration_id,friend_registration_id,confirmed_at IS NOT NULL FROM concert_product_owner_transactions WHERE instance_id=$1 AND scope_id=$2 AND identity_digest=$3")
            .bind(key.instance).bind(key.scope).bind(digest.as_slice()).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let (old_auth, old_friend, confirmed) =
            old.ok_or_else(|| Status::failed_precondition("Unknown product transaction"))?;
        if old_auth != Some(auth) || old_friend != Some(friend) {
            return Err(Status::permission_denied("Product tracking mismatch"));
        }
        if !confirmed {
            sqlx::query("UPDATE concert_product_owner_transactions SET confirmed_at=clock_timestamp() WHERE instance_id=$1 AND scope_id=$2 AND identity_digest=$3")
                .bind(key.instance).bind(key.scope).bind(digest.as_slice()).execute(&mut *tx).await.map_err(unavailable)?;
        }
        tx.commit().await.map_err(unavailable)
    }
    pub async fn mark_serialized(&self, key: &ScopeKey) -> Result<(), Status> {
        let mut tx = self.transaction().await?;
        if self.lock(&mut tx, key).await? != ScopePhase::Active {
            return Err(Status::failed_precondition("Product scope is sealed"));
        }
        sqlx::query("UPDATE concert_product_owner_scopes SET serialized_at=clock_timestamp() WHERE instance_id=$1 AND scope_id=$2 AND serialized_at IS NULL AND delivery_secret_hash IS NOT NULL")
            .bind(key.instance).bind(key.scope).execute(&mut *tx).await.map_err(unavailable)?;
        let ready: bool = sqlx::query_scalar("SELECT serialized_at IS NOT NULL FROM concert_product_owner_scopes WHERE instance_id=$1 AND scope_id=$2")
            .bind(key.instance).bind(key.scope).fetch_one(&mut *tx).await.map_err(unavailable)?;
        if !ready {
            return Err(Status::failed_precondition(
                "Product delivery not registered",
            ));
        }
        tx.commit().await.map_err(unavailable)
    }
    pub async fn seal(&self, key: &ScopeKey) -> Result<(), Status> {
        let mut tx = self.transaction().await?;
        if self.lock(&mut tx, key).await? == ScopePhase::Active {
            sqlx::query("UPDATE concert_product_owner_scopes SET phase='SEALED',sealed_at=clock_timestamp() WHERE instance_id=$1 AND scope_id=$2")
                .bind(key.instance).bind(key.scope).execute(&mut *tx).await.map_err(unavailable)?;
        }
        tx.commit().await.map_err(unavailable)
    }
    /// Only the canonical registry calls this after sealing and draining actual
    /// root/callback work plus the authenticated terminal HTTP delivery callback.
    pub async fn finish_root_work(&self, key: &ScopeKey) -> Result<(), Status> {
        let mut tx = self.transaction().await?;
        match self.lock(&mut tx, key).await? {
            ScopePhase::RootFinished => (),
            ScopePhase::Sealed => {
                sqlx::query("UPDATE concert_product_owner_scopes SET phase='ROOT_FINISHED',root_finished_at=clock_timestamp() WHERE instance_id=$1 AND scope_id=$2")
                    .bind(key.instance).bind(key.scope).execute(&mut *tx).await.map_err(unavailable)?;
            }
            _ => return Err(Status::failed_precondition("Product root work not sealed")),
        }
        tx.commit().await.map_err(unavailable)
    }
    pub async fn resolve_scope(&self, key: &ScopeKey) -> Result<ScopePhase, Status> {
        key.validate(self.instance)?;
        let row: Option<(Vec<u8>, String)> = sqlx::query_as("SELECT binding_digest,phase FROM concert_product_owner_scopes WHERE instance_id=$1 AND scope_id=$2")
            .bind(key.instance).bind(key.scope).fetch_optional(&self.pool).await.map_err(unavailable)?;
        match row {
            Some((digest, value)) if digest == key.binding_digest => phase(&value),
            Some(_) => Err(Status::permission_denied("Product scope mismatch")),
            None => Ok(ScopePhase::Unknown),
        }
    }
    /// Called only after the actual canonical scope completed both owners and
    /// every child successfully. ROOT_FINISHED alone never writes this receipt.
    pub async fn record_delivery_completed(&self, key: &ScopeKey) -> Result<(), Status> {
        let mut tx = self.transaction().await?;
        if self.lock(&mut tx, key).await? != ScopePhase::RootFinished {
            return Err(Status::failed_precondition(
                "Product delivery root unfinished",
            ));
        }
        sqlx::query("UPDATE concert_product_owner_scopes SET delivery_completed_at=clock_timestamp() WHERE instance_id=$1 AND scope_id=$2 AND delivery_completed_at IS NULL")
            .bind(key.instance).bind(key.scope).execute(&mut *tx).await.map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)
    }
    pub async fn delivery_completed(&self, key: &ScopeKey) -> Result<bool, Status> {
        key.validate(self.instance)?;
        let value: Option<bool> = sqlx::query_scalar("SELECT delivery_completed_at IS NOT NULL FROM concert_product_owner_scopes WHERE instance_id=$1 AND scope_id=$2 AND binding_digest=$3")
            .bind(key.instance).bind(key.scope).bind(key.binding_digest.as_slice()).fetch_optional(&self.pool).await.map_err(unavailable)?;
        Ok(value == Some(true))
    }
    /// Return only exact owner-recorded evidence. A lost Track reply with no
    /// durable registration remains unknown; a caller-supplied UUID cannot fill it.
    pub async fn registration_binding(
        &self,
        owner: Workload,
        registration: Uuid,
        identity: &ProducerTransaction,
    ) -> Result<OwnerBinding, Status> {
        if registration.is_nil() {
            return Err(invalid());
        }
        let column = match owner {
            Workload::Auth => "auth_registration_id",
            Workload::Friend => "friend_registration_id",
            _ => {
                return Err(Status::permission_denied(
                    "Invalid product settlement owner",
                ))
            }
        };
        let digest = identity.digest()?;
        let query = format!("SELECT s.binding,t.identity FROM concert_product_owner_transactions t JOIN concert_product_owner_scopes s USING(instance_id,scope_id,binding_digest) WHERE t.instance_id=$1 AND t.{column}=$2 AND t.identity_digest=$3");
        let row: Option<(serde_json::Value, serde_json::Value)> = sqlx::query_as(&query)
            .bind(self.instance)
            .bind(registration)
            .bind(digest.as_slice())
            .fetch_optional(&self.pool)
            .await
            .map_err(unavailable)?;
        let (binding, transaction) = row.ok_or_else(|| {
            Status::failed_precondition("Unknown product settlement registration")
        })?;
        let actual: ProducerTransaction =
            serde_json::from_value(transaction).map_err(|_| invalid())?;
        let binding: OwnerBinding = serde_json::from_value(binding).map_err(|_| invalid())?;
        if &actual != identity || binding.digest()? != identity.effect_binding_digest {
            return Err(Status::permission_denied("Product settlement mismatch"));
        }
        Ok(binding)
    }
}
