//! BE42 Sessions effects. The canonical Auth adapter and common account identity
//! are injected; raw commands, empty registries and queued jobs never imply ACK.
use super::{
    account_deletion::{
        self as common, DeletionAcknowledgement, DeletionAuthority, DeletionFailure,
        WorldDeletionCommand, WorldDeletionTarget,
    },
    admission_bridge::AdmissionBridge,
    runtime::{Command, RuntimeHandle},
    simulation::Binding,
};
use crate::adapters::r#in::world_ws::{SocketTransport, WorldSocketState};
use sqlx::{PgPool, Row};
use std::{
    collections::BTreeMap,
    sync::{Arc, Mutex},
};
use tokio::{sync::oneshot, time::Instant};
use uuid::Uuid;

const MAX_OWNERS: usize = 16;
const NATIVE_BATCH: usize = 128;
const MEMBERSHIP_BATCH: i64 = 128;
type StreamKey = (Uuid, Uuid, Uuid);
fn stream_key(b: &Binding) -> StreamKey {
    (b.room_id, b.connection_id, b.stream_id)
}
fn unavailable(_: impl std::fmt::Debug) -> DeletionFailure {
    DeletionFailure::Unavailable
}

/// Construct from the *serving* socket state. No default/empty transport exists.
/// A cancelled dispatch keeps exact native cleanup obligations in this owner;
/// restarting the process destroys the corresponding native state itself.
pub struct SessionOwner {
    bridge: Arc<AdmissionBridge>,
    runtime: Arc<RuntimeHandle>,
    transport: SocketTransport,
    pending: Mutex<BTreeMap<StreamKey, Binding>>,
}
impl SessionOwner {
    pub fn new(
        bridge: Arc<AdmissionBridge>,
        state: &WorldSocketState,
    ) -> Result<Self, DeletionFailure> {
        let (transport, runtime) = state.account_session_parts(&bridge).map_err(unavailable)?;
        Ok(Self {
            bridge,
            runtime,
            transport,
            pending: Mutex::default(),
        })
    }
    fn identity(&self) -> (Uuid, Uuid, Uuid) {
        let (_, process, epoch, event) = self.bridge.invalidation_context();
        (event, process, epoch)
    }
    async fn withdraw(&self, user: Uuid, budget: &mut usize) -> Result<bool, DeletionFailure> {
        let _gate = self.bridge.invalidation_admission_gate().lock().await;
        let bindings = self
            .bridge
            .withdraw_account_views(user)
            .map_err(unavailable)?;
        // No await separates local withdrawal and retained exact obligations.
        let batch: Vec<_> = {
            let mut pending = self.pending.lock().map_err(unavailable)?;
            for binding in bindings {
                pending.insert(stream_key(&binding), binding);
            }
            for binding in self.transport.withdraw_account(user).map_err(unavailable)? {
                pending.insert(stream_key(&binding), binding);
            }
            if pending.len() > 4096 {
                return Err(DeletionFailure::Unavailable);
            }
            pending
                .values()
                .filter(|b| b.key.user_id.uuid() == user)
                .take(*budget)
                .cloned()
                .collect()
        };
        for binding in batch {
            let (reply, answer) = oneshot::channel();
            self.runtime
                .try_submit(Command::Disconnect {
                    binding: binding.clone(),
                    reply,
                })
                .map_err(unavailable)?;
            // Exact false means this generation is already absent; a newer
            // generation is independently captured and permanently fenced.
            answer.await.map_err(unavailable)?.map_err(unavailable)?;
            self.pending
                .lock()
                .map_err(unavailable)?
                .remove(&stream_key(&binding));
            *budget -= 1;
        }
        Ok(!self
            .pending
            .lock()
            .map_err(unavailable)?
            .values()
            .any(|b| b.key.user_id.uuid() == user))
    }
}

pub struct SessionDeletion {
    pool: PgPool,
    authority: Arc<dyn DeletionAuthority>,
    owners: Vec<SessionOwner>,
    dispatch: tokio::sync::Mutex<()>,
}
impl SessionDeletion {
    pub fn new(
        pool: PgPool,
        authority: Arc<dyn DeletionAuthority>,
        owners: Vec<SessionOwner>,
    ) -> Result<Self, DeletionFailure> {
        if owners.is_empty() || owners.len() > MAX_OWNERS {
            return Err(DeletionFailure::Unavailable);
        }
        let mut identities = std::collections::BTreeSet::new();
        for owner in &owners {
            let identity = owner.identity();
            if [identity.0, identity.1, identity.2]
                .iter()
                .any(Uuid::is_nil)
                || !identities.insert(identity)
            {
                return Err(DeletionFailure::Unavailable);
            }
        }
        Ok(Self {
            pool,
            authority,
            owners,
            dispatch: tokio::sync::Mutex::new(()),
        })
    }
    pub async fn apply(
        &self,
        command: WorldDeletionCommand,
        original_deadline: Instant,
    ) -> Result<DeletionAcknowledgement, DeletionFailure> {
        command.validate()?;
        if command.target != WorldDeletionTarget::Sessions {
            return Err(DeletionFailure::InvalidCommand);
        }
        common::check_deadline(original_deadline)?;
        tokio::time::timeout_at(
            original_deadline,
            self.apply_in_deadline(command, original_deadline),
        )
        .await
        .map_err(unavailable)?
    }
    async fn apply_in_deadline(
        &self,
        command: WorldDeletionCommand,
        deadline: Instant,
    ) -> Result<DeletionAcknowledgement, DeletionFailure> {
        let _dispatch = self.dispatch.lock().await;
        let verified = common::verify_source(self.authority.as_ref(), &command, deadline).await?;
        // Commit the permanent fence and resumable operation BEFORE native work.
        // Failure/cancellation after this point is a retry, never a rollback of
        // the user's permanent denial or an acknowledgement of accepted work.
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        let fence = common::establish_fence_in(&mut tx, &command, &verified).await?;
        sqlx::query("insert into public.concert_world_session_deletion_progress(user_id,saga_id,command_binding,source_binding) values($1,$2,$3,$4) on conflict do nothing")
            .bind(command.user_id).bind(command.saga_id).bind(command.binding()?.as_slice())
            .bind(verified.source_binding().as_slice()).execute(&mut *tx).await.map_err(unavailable)?;
        let row = sqlx::query("select saga_id,command_binding,source_binding from public.concert_world_session_deletion_progress where user_id=$1 for update")
            .bind(command.user_id).fetch_one(&mut *tx).await.map_err(unavailable)?;
        if row.try_get::<Uuid, _>("saga_id").map_err(unavailable)? != command.saga_id
            || row
                .try_get::<Vec<u8>, _>("command_binding")
                .map_err(unavailable)?
                != command.binding()?
            || row
                .try_get::<Vec<u8>, _>("source_binding")
                .map_err(unavailable)?
                != *verified.source_binding()
        {
            return Err(DeletionFailure::InvalidCommand);
        }
        self.recheck(&command, deadline, &verified).await?;
        tx.commit().await.map_err(unavailable)?;

        // Do not erase a foreign owner's durable victim identity to manufacture
        // a clean registry. Even disconnected rows can have a live native socket.
        self.require_owner_coverage(command.user_id).await?;
        let mut budget = NATIVE_BATCH;
        let mut drained = true;
        for owner in &self.owners {
            drained &= owner.withdraw(command.user_id, &mut budget).await?;
        }
        if !drained {
            return Err(DeletionFailure::Incomplete);
        }

        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        common::bound_transaction_in(&mut tx, deadline).await?;
        common::lock_user(&mut tx, command.user_id)
            .await
            .map_err(unavailable)?;
        let removed: i64 = sqlx::query_scalar("with batch as (select event_id from public.concert_world_memberships where user_id=$1 order by event_id limit $2 for update), removed as (delete from public.concert_world_memberships m using batch b where m.user_id=$1 and m.event_id=b.event_id returning 1) select count(*)::bigint from removed")
            .bind(command.user_id).bind(MEMBERSHIP_BATCH).fetch_one(&mut *tx).await.map_err(unavailable)?;
        let more: bool = sqlx::query_scalar(
            "select exists(select 1 from public.concert_world_memberships where user_id=$1)",
        )
        .bind(command.user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(unavailable)?;
        // Exact replay never mutates an already-completed immutable receipt.
        sqlx::query("update public.concert_world_session_deletion_progress set removed_count=removed_count+$2,progress_revision=progress_revision+1 where user_id=$1 and completed_at is null")
            .bind(command.user_id).bind(removed).execute(&mut *tx).await.map_err(unavailable)?;
        self.recheck(&command, deadline, &verified).await?;
        tx.commit().await.map_err(unavailable)?;
        if more {
            return Err(DeletionFailure::Incomplete);
        }

        for owner in &self.owners {
            // Only durable all-event absence permits retained native/recovery
            // cleanup identities to be retired. No ordinary lease is extended.
            owner
                .bridge
                .confirm_account_absence(command.user_id)
                .await
                .map_err(unavailable)?;
            owner.runtime.ready().await.map_err(unavailable)?;
        }
        self.require_owner_coverage(command.user_id).await?;
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        common::bound_transaction_in(&mut tx, deadline).await?;
        common::lock_user(&mut tx, command.user_id)
            .await
            .map_err(unavailable)?;
        self.recheck(&command, deadline, &verified).await?;
        sqlx::query("update public.concert_world_session_deletion_progress set completed_at=clock_timestamp(),progress_revision=progress_revision+1 where user_id=$1 and completed_at is null")
            .bind(command.user_id).execute(&mut *tx).await.map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        common::check_deadline(deadline)?;
        Ok(DeletionAcknowledgement {
            command,
            fence_revision: fence.revision,
        })
    }
    async fn recheck(
        &self,
        command: &WorldDeletionCommand,
        deadline: Instant,
        previous: &common::VerifiedDeletionSource,
    ) -> Result<(), DeletionFailure> {
        let current = common::verify_source(self.authority.as_ref(), command, deadline).await?;
        if &current != previous {
            return Err(DeletionFailure::Unavailable);
        }
        Ok(())
    }
    async fn require_owner_coverage(&self, user: Uuid) -> Result<(), DeletionFailure> {
        let rows = sqlx::query("select distinct m.event_id,m.owner_process_id,m.owner_epoch from public.concert_world_memberships m left join public.concert_world_owner_leases l on (l.event_id,l.room_id,l.owner_process_id,l.owner_epoch)=(m.event_id,m.room_id,m.owner_process_id,m.owner_epoch) where m.user_id=$1 and (m.lease_until>clock_timestamp() or l.lease_until>clock_timestamp()) limit 17")
            .bind(user).fetch_all(&self.pool).await.map_err(unavailable)?;
        for row in rows {
            let event: Uuid = row.try_get("event_id").map_err(unavailable)?;
            let process: String = row.try_get("owner_process_id").map_err(unavailable)?;
            let epoch: Uuid = row.try_get("owner_epoch").map_err(unavailable)?;
            if !self.owners.iter().any(|o| {
                let i = o.identity();
                i.0 == event && i.1.to_string() == process && i.2 == epoch
            }) {
                return Err(DeletionFailure::Incomplete);
            }
        }
        Ok(())
    }
}
