//! Test-only synthetic roles/owners in a private marker/inode-checked database.
//! The native identity is captured from an actual local fixture transaction;
//! this is not evidence of a real World/Post/private-authority RPC.
use super::*;
use sqlx::postgres::{PgConnectOptions, PgPoolOptions};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
};
pub async fn owned_pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(
        std::env::var("CONCERT_TEST_OWNER_ROOT")
            .map_err(|_| anyhow::anyhow!("owned fixture root required"))?,
    );
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        info.is_dir() && !info.file_type().is_symlink() && info.permissions().mode() & 0o077 == 0,
        "private owned root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("CONCERT_TEST_OWNER_IDENTITY")?,
        "fixture inode changed"
    );
    let marker_path = root.join("mcr-owner.json");
    let marker_info = std::fs::symlink_metadata(&marker_path)?;
    anyhow::ensure!(
        marker_info.is_file()
            && !marker_info.file_type().is_symlink()
            && marker_info.permissions().mode() & 0o077 == 0,
        "private marker required"
    );
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(marker_path)?)?;
    anyhow::ensure!(
        marker["token"].as_str() == Some(std::env::var("CONCERT_TEST_OWNER_TOKEN")?.as_str())
            && marker["uid"].as_u64() == Some(info.uid() as u64),
        "fixture owner mismatch"
    );
    let options = PgConnectOptions::from_str(&std::env::var("CONCERT_TEST_DATABASE_URL")?)
        .map_err(|_| anyhow::anyhow!("invalid owned fixture options"))?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_concert_be_moderation")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await
        .map_err(|_| anyhow::anyhow!("owned fixture connection failed"))?;
    let actual: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        actual == root.join("data").to_string_lossy(),
        "foreign data directory refused"
    );
    Ok(pool)
}

#[derive(Clone)]
pub struct Authority {
    pub actor: Actor,
    pub role: Role,
    pub target_role: Role,
    pub mode: u8,
}
impl Authority {
    pub fn operator(actor: Actor) -> Self {
        Self {
            actor,
            role: Role::Operator,
            target_role: Role::Member,
            mode: 0,
        }
    }
}
#[async_trait]
impl ModerationAuthority for Authority {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        q: AuthorityQuery,
        now: DateTime<Utc>,
    ) -> Result<RoleLease, CommandError> {
        if q.actor != self.actor {
            return Err(CommandError::Denied);
        }
        let allowed: bool = sqlx::query_scalar(
            "select allowed from be39_fixture_roles where actor=$1 and event=$2 for share",
        )
        .bind(q.actor.actor)
        .bind(q.actor.event)
        .fetch_optional(c)
        .await
        .map_err(|_| CommandError::Unavailable)?
        .ok_or(CommandError::Denied)?;
        if !allowed {
            return Err(CommandError::Denied);
        }
        let mut lease = RoleLease {
            query: q,
            role: self.role,
            revision: 1,
            checked_at: now,
            expires_at: now + chrono::Duration::seconds(10),
        };
        match self.mode {
            1 => lease.query.actor.event = Uuid::new_v4(),
            2 => lease.expires_at = now,
            3 => lease.revision = 0,
            _ => {}
        }
        Ok(lease)
    }
    async fn target_role_in(
        &self,
        _: &mut PgConnection,
        actor: Actor,
        target: Uuid,
        now: DateTime<Utc>,
    ) -> Result<TargetRoleLease, CommandError> {
        if actor != self.actor {
            return Err(CommandError::Denied);
        }
        Ok(TargetRoleLease {
            event: actor.event,
            target,
            role: self.target_role,
            revision: 1,
            checked_at: now,
            expires_at: now + chrono::Duration::seconds(10),
        })
    }
    async fn check_disclosure(&self) -> Result<(), CommandError> {
        Ok(())
    }
}
pub async fn fixture() -> anyhow::Result<(PgPool, Actor)> {
    let pool = owned_pool().await?;
    sqlx::raw_sql("create table if not exists be39_fixture_roles(actor uuid,event uuid,allowed boolean not null,primary key(actor,event)); create table if not exists be39_fixture_owner(receipt uuid,step text,target uuid,digest bytea,native bytea,owner_receipt uuid,revision bigint,primary key(receipt,step)); create table if not exists be39_fixture_targets(event_id uuid,target_id uuid,state text,revision bigint,epoch bigint,sessions_live boolean,grants_live boolean,primary key(event_id,target_id));").execute(&pool).await?;
    let actor = Actor {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
    };
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Synthetic moderation fixture','OPEN')").bind(actor.event).bind(format!("be39-{}",actor.event.simple())).execute(&pool).await?;
    sqlx::query("insert into be39_fixture_roles values($1,$2,true)")
        .bind(actor.actor)
        .bind(actor.event)
        .execute(&pool)
        .await?;
    Ok((pool, actor))
}
pub fn keys() -> Arc<FingerprintKeys> {
    Arc::new(
        FingerprintKeys::new(SigningKey::new("be39-fixture", &[39; 32]).unwrap(), vec![]).unwrap(),
    )
}
pub fn user(target: Uuid, action: UserAction, revision: i64) -> Change {
    Change::User {
        target,
        expected_revision: revision,
        action,
        expires_at: None,
        reason: "Private synthetic moderation reason".into(),
    }
}
pub fn content(target: Uuid, action: ContentAction) -> Change {
    Change::Content {
        kind: ContentKind::Party,
        target,
        expected_revision: 1,
        action,
        reason: "Private synthetic content reason".into(),
    }
}
#[derive(Clone)]
pub struct Owner {
    pub pool: PgPool,
    pub mode: u8,
    pub calls: Arc<std::sync::atomic::AtomicUsize>,
    pub recoveries: Arc<std::sync::atomic::AtomicUsize>,
}
impl Owner {
    pub fn new(pool: &PgPool, mode: u8) -> Self {
        Self {
            pool: pool.clone(),
            mode,
            calls: Default::default(),
            recoveries: Default::default(),
        }
    }
    async fn stored(&self, i: &OwnerIntent) -> Result<Option<OwnerOutcome>, CommandError> {
        let row=sqlx::query("select digest,native,owner_receipt,revision from be39_fixture_owner where receipt=$1 and step=$2").bind(i.receipt).bind(i.step.as_str()).fetch_optional(&self.pool).await.map_err(|_|CommandError::Unavailable)?;
        row.map(|r|{
            let digest:Vec<u8>=r.try_get("digest").map_err(|_|CommandError::Unavailable)?;if digest!=i.digest(){return Err(CommandError::Conflict)}
            let native:Vec<u8>=r.try_get("native").map_err(|_|CommandError::Unavailable)?;
            let w=concert_service::adapters::out::relationship_authority::guarded::proto::TransactionIdentity::decode(native.as_slice()).map_err(|_|CommandError::Unavailable)?;
            let transaction=ProducerTransaction{producer:ProducerKind::Concert,system_identifier:w.system_identifier,database_oid:w.database_oid,full_xid:w.full_xid,backend_pid:w.backend_pid,backend_started:DateTime::from_timestamp_micros(w.backend_started_unix_micros).ok_or(CommandError::Unavailable)?,effect_binding_digest:w.effect_binding_digest.try_into().map_err(|_|CommandError::Unavailable)?,producer_instance_id:Uuid::parse_str(&w.producer_instance_id).map_err(|_|CommandError::Unavailable)?,settlement_reference:w.settlement_reference};
            Ok(OwnerOutcome::Applied{intent:i.clone(),owner_receipt:r.try_get("owner_receipt").map_err(|_|CommandError::Unavailable)?,revision:r.try_get("revision").map_err(|_|CommandError::Unavailable)?,transaction})
        }).transpose()
    }
}
#[async_trait]
impl ModerationOwner for Owner {
    async fn apply(
        &self,
        _: &dyn ModerationAuthority,
        i: &OwnerIntent,
        c: &Change,
    ) -> Result<OwnerOutcome, CommandError> {
        self.calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        if self.mode == 2 {
            return Ok(OwnerOutcome::Failed {
                intent: i.clone(),
                reason: SafeFailure::InvalidTransition,
            });
        }
        if self.mode == 4 {
            return Ok(OwnerOutcome::Pending { intent: i.clone() });
        }
        if let Some(v) = self.stored(i).await? {
            return Ok(v);
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| CommandError::Unavailable)?;
        let proof = ProducerTransaction::capture_concert_in(
            &mut tx,
            i.digest(),
            Uuid::new_v4(),
            i.receipt.as_bytes().to_vec(),
        )
        .await
        .map_err(|_| CommandError::Unavailable)?;
        let initial = match c {
            Change::User {
                action: UserAction::Unsuspend,
                ..
            } => "SUSPENDED",
            Change::User { .. } => "ACTIVE",
            _ => "VISIBLE",
        };
        sqlx::query("insert into be39_fixture_targets values($1,$2,$3,1,1,true,true) on conflict do nothing").bind(i.actor.event).bind(i.target).bind(initial).execute(&mut *tx).await.map_err(|_|CommandError::Unavailable)?;
        let target=sqlx::query("select state,revision from be39_fixture_targets where event_id=$1 and target_id=$2 for update").bind(i.actor.event).bind(i.target).fetch_one(&mut *tx).await.map_err(|_|CommandError::Unavailable)?;
        if i.step == c.steps()[0] {
            let current: i64 = target
                .try_get("revision")
                .map_err(|_| CommandError::Unavailable)?;
            let state: String = target
                .try_get("state")
                .map_err(|_| CommandError::Unavailable)?;
            let failure = if current != c.revision() {
                Some(SafeFailure::VersionConflict)
            } else if state == "DELETED" {
                Some(SafeFailure::InvalidTransition)
            } else {
                None
            };
            if let Some(reason) = failure {
                return Ok(OwnerOutcome::Failed {
                    intent: i.clone(),
                    reason,
                });
            }
            let (next, advance_epoch) = match c {
                Change::User {
                    action: UserAction::Suspend,
                    ..
                } => ("SUSPENDED", true),
                Change::User { .. } => ("ACTIVE", false),
                Change::Content {
                    action: ContentAction::Hide,
                    ..
                } => ("HIDDEN", false),
                Change::Content { .. } => ("VISIBLE", false),
            };
            sqlx::query("update be39_fixture_targets set state=$3,revision=revision+1,epoch=epoch+case when $4 then 1 else 0 end where event_id=$1 and target_id=$2").bind(i.actor.event).bind(i.target).bind(next).bind(advance_epoch).execute(&mut *tx).await.map_err(|_|CommandError::Unavailable)?;
        } else if i.step == OwnerStep::EventWorldTermination {
            sqlx::query("update be39_fixture_targets set sessions_live=false where event_id=$1 and target_id=$2").bind(i.actor.event).bind(i.target).execute(&mut *tx).await.map_err(|_|CommandError::Unavailable)?;
        } else if i.step == OwnerStep::EventPrivateAuthorityRevocation {
            sqlx::query("update be39_fixture_targets set grants_live=false where event_id=$1 and target_id=$2").bind(i.actor.event).bind(i.target).execute(&mut *tx).await.map_err(|_|CommandError::Unavailable)?;
        }
        let receipt = Uuid::new_v4();
        sqlx::query("insert into be39_fixture_owner values($1,$2,$3,$4,$5,$6,$7)")
            .bind(i.receipt)
            .bind(i.step.as_str())
            .bind(i.target)
            .bind(i.digest().as_slice())
            .bind(
                proof
                    .wire()
                    .map_err(|_| CommandError::Unavailable)?
                    .encode_to_vec(),
            )
            .bind(receipt)
            .bind(c.revision() + 1)
            .execute(&mut *tx)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        tx.commit().await.map_err(|_| CommandError::Unavailable)?;
        if self.mode == 1 {
            return Err(CommandError::Unavailable);
        } // Commit happened; ACK lost.
        if self.mode == 5 {
            let mut v = i.clone();
            v.target = Uuid::new_v4();
            return Ok(OwnerOutcome::Applied {
                intent: v,
                owner_receipt: receipt,
                revision: c.revision() + 1,
                transaction: proof,
            });
        }
        self.stored(i).await?.ok_or(CommandError::Unavailable)
    }
    async fn recover(
        &self,
        _: &dyn ModerationAuthority,
        i: &OwnerIntent,
        _: &Change,
    ) -> Result<OwnerOutcome, CommandError> {
        self.recoveries
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        Ok(self
            .stored(i)
            .await?
            .unwrap_or(OwnerOutcome::Pending { intent: i.clone() }))
    }
    async fn review(
        &self,
        _: &dyn ModerationAuthority,
        _: Actor,
        _: Uuid,
    ) -> Result<serde_json::Value, CommandError> {
        Err(CommandError::Unavailable)
    }
}
pub fn owners(owner: &Owner) -> ModerationOwners {
    ModerationOwners {
        event_restriction: Some(Arc::new(owner.clone())),
        event_world_termination: Some(Arc::new(owner.clone())),
        event_private_authority_revocation: Some(Arc::new(owner.clone())),
        party_content: Some(Arc::new(owner.clone())),
        ..Default::default()
    }
}
pub async fn outcomes(pool: &PgPool, receipt: Uuid) -> Vec<String> {
    sqlx::query_scalar("select outcome from concert_moderation_audit where receipt_id=$1 order by created_at,audit_id").bind(receipt).fetch_all(pool).await.unwrap()
}

pub mod capture;
