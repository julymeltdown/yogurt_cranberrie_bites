//! Transactional BE15 handoff to the content owner. No remote mutation occurs
//! inside Concert's transaction and PENDING never means content was published.
use crate::adapters::out::repository::receipt::{CommandProducer, PgCommandRepository};
use async_trait::async_trait;
use concert_application::{commands::*, feed::PreparedCommand};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

/// Implement only after qualifying a protocol that fences current Auth/Friend,
/// local membership/phase, parent/owner/CAS, media and actual linked target
/// authority through commit. A bounded remote observation cannot implement this.
#[async_trait]
pub trait FencedFeedAuthority: Send + Sync {
    async fn check_in(
        &self,
        connection: &mut PgConnection,
        command: &PreparedCommand,
    ) -> Result<(), CommandError>;
}
pub struct UnavailableFeedAuthority;
#[async_trait]
impl FencedFeedAuthority for UnavailableFeedAuthority {
    async fn check_in(
        &self,
        _: &mut PgConnection,
        _: &PreparedCommand,
    ) -> Result<(), CommandError> {
        Err(CommandError::Unavailable)
    }
}
pub struct FeedProducer<'a, A> {
    pub command: &'a PreparedCommand,
    pub authority: &'a A,
}
impl<A: FencedFeedAuthority> FeedProducer<'_, A> {
    fn binding(&self, scope: Scope, target: Uuid) -> Result<(), CommandError> {
        if self
            .command
            .scope()
            .map_err(|_| CommandError::InvalidInput)?
            != scope
            || self.command.target() != target
        {
            return Err(CommandError::Denied);
        }
        Ok(())
    }
}
#[async_trait]
impl<A: FencedFeedAuthority> CommandProducer for FeedProducer<'_, A> {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        self.binding(s, target)?;
        self.authority.check_in(c, self.command).await
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        self.authorize(c, s, target).await?;
        let payload =
            serde_json::to_value(self.command.command()).map_err(|_| CommandError::InvalidInput)?;
        // Scope and target are selected from the just-created receipt, not provided
        // by a separate request. Durable job and Pending receipt commit together.
        let n=sqlx::query("insert into public.concert_feed_owner_work(receipt_id,event_id,target_id,actor_id,operation_id,idempotency_key,session_family_id,request_id,authority_expires_at,payload) select receipt_id,event_id,target_id,actor_id,operation_id,idempotency_key,$6,$7,$8,$9 from public.concert_command_receipts where actor_id=$1 and event_id=$2 and operation_id=$3 and idempotency_key=$4 and target_id=$5 and state='PENDING'")
            .bind(s.actor).bind(s.event).bind(s.operation.id()).bind(s.key).bind(target)
            .bind(self.command.context().session_family).bind(self.command.context().request).bind(self.command.context().expires_at).bind(payload)
            .execute(c).await.map_err(|_|CommandError::Unavailable)?.rows_affected();
        if n != 1 {
            return Err(CommandError::InvalidState);
        }
        Ok((Completion::Pending, None))
    }
}
/// A metadata-only result from a verified, committed owner receipt. A transport
/// adapter must authenticate the owner and bind the full original scope; no raw
/// post or actor-supplied success is accepted by root composition.
#[derive(Clone, Copy, Debug)]
pub struct OwnerCommit {
    pub scope: Scope,
    pub resource: Uuid,
    pub revision: i64,
}
pub struct FeedQueue {
    pool: PgPool,
}
impl FeedQueue {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub async fn enqueue<A: FencedFeedAuthority>(
        &self,
        command: &PreparedCommand,
        keys: &FingerprintKeys,
        authority: &A,
    ) -> Result<SafeReceipt, CommandError> {
        let request = command
            .canonical()
            .map_err(|_| CommandError::InvalidInput)?;
        PgCommandRepository::new(self.pool.clone())
            .execute(
                &request,
                command.target(),
                keys,
                &FeedProducer { command, authority },
            )
            .await
    }
    /// Only called after a real owner commit. The command is reconstructed from
    /// protected work storage/current authenticated context, never from a browser.
    /// Receipt and metadata-only invalidation are completed in this one transaction.
    pub async fn acknowledge<A: FencedFeedAuthority>(
        &self,
        command: &PreparedCommand,
        commit: OwnerCommit,
        authority: &A,
    ) -> Result<(), CommandError> {
        let scope = command.scope().map_err(|_| CommandError::InvalidInput)?;
        if scope != commit.scope || command.target() != commit.resource {
            return Err(CommandError::Denied);
        }
        SafeReference {
            resource: commit.resource,
            revision: commit.revision,
        }
        .validate()?;
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        authority.check_in(&mut tx, command).await?;
        sqlx::query(
            "select singleton from public.concert_command_budget where singleton for update",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
        let row=sqlx::query("select r.receipt_id,r.state,r.resource_revision,r.expires_at,w.state work_state,w.payload from public.concert_command_receipts r join public.concert_feed_owner_work w using(receipt_id) where r.actor_id=$1 and r.event_id=$2 and r.operation_id=$3 and r.idempotency_key=$4 and r.target_id=$5 for update of r,w")
            .bind(scope.actor).bind(scope.event).bind(scope.operation.id()).bind(scope.key).bind(commit.resource).fetch_optional(&mut *tx).await.map_err(unavailable)?.ok_or(CommandError::Denied)?;
        // Compare intent as well as scope/target before a qualified acknowledgement.
        let stored: serde_json::Value = row.try_get("payload").map_err(unavailable)?;
        if stored
            != serde_json::to_value(command.command()).map_err(|_| CommandError::InvalidInput)?
        {
            return Err(CommandError::Conflict);
        }
        let expires: chrono::DateTime<chrono::Utc> =
            row.try_get("expires_at").map_err(unavailable)?;
        let expired: bool = sqlx::query_scalar("select clock_timestamp()>=$1")
            .bind(expires)
            .fetch_one(&mut *tx)
            .await
            .map_err(unavailable)?;
        if expired {
            return Err(CommandError::Expired);
        }
        let state: String = row.try_get("state").map_err(unavailable)?;
        let work: String = row.try_get("work_state").map_err(unavailable)?;
        if state == "SUCCEEDED"
            && work == "ACKNOWLEDGED"
            && row
                .try_get::<Option<i64>, _>("resource_revision")
                .map_err(unavailable)?
                == Some(commit.revision)
        {
            tx.commit().await.map_err(unavailable)?;
            return Ok(());
        }
        if state != "PENDING" || work != "QUEUED" {
            return Err(CommandError::InvalidState);
        }
        let receipt: Uuid = row.try_get("receipt_id").map_err(unavailable)?;
        let queued: i64 = sqlx::query_scalar(
            "select count(*) from public.concert_outbox where state in ('READY','LEASED')",
        )
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
        if queued >= 10000 {
            return Err(CommandError::Capacity);
        }
        authority.check_in(&mut tx, command).await?;
        sqlx::query("update public.concert_command_receipts set state='SUCCEEDED',resource_id=$2,resource_revision=$3,result_status=200 where receipt_id=$1").bind(receipt).bind(commit.resource).bind(commit.revision).execute(&mut *tx).await.map_err(unavailable)?;
        sqlx::query("update public.concert_feed_owner_work set state='ACKNOWLEDGED',acknowledged_at=clock_timestamp() where receipt_id=$1").bind(receipt).execute(&mut *tx).await.map_err(unavailable)?;
        sqlx::query("insert into public.concert_outbox(outbox_id,receipt_id,event_id,resource_id,resource_revision) values($1,$2,$3,$4,$5)").bind(Uuid::new_v4()).bind(receipt).bind(scope.event).bind(commit.resource).bind(commit.revision).execute(&mut *tx).await.map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)
    }
}
fn unavailable(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
