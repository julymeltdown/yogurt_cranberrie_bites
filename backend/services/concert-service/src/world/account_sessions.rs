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
use concert_domain::{AssetId, AuthUserId, EventId, ParticipantKey, Revision};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{PgPool, Row};
use std::{collections::BTreeMap, sync::Arc};
use tokio::{sync::oneshot, time::Instant};
use uuid::Uuid;

const MAX_OWNERS: usize = 16;
const NATIVE_BATCH: usize = 128;
const MEMBERSHIP_BATCH: i64 = 128;
// Serialize the entire exact native binding, including resource/room/stream
// generations. Database journal contents remain private and are never logged.
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct NativeBinding {
    event: Uuid,
    user: Uuid,
    session: Uuid,
    connection: Uuid,
    owner: Uuid,
    room: Uuid,
    room_epoch: Uuid,
    stream: Uuid,
    map: String,
    map_revision: i64,
    avatar: String,
    avatar_revision: i64,
}
impl NativeBinding {
    fn capture(b: &Binding) -> Self {
        Self {
            event: b.key.event_id.uuid(),
            user: b.key.user_id.uuid(),
            session: b.session_id,
            connection: b.connection_id,
            owner: b.owner_id,
            room: b.room_id,
            room_epoch: b.room_epoch,
            stream: b.stream_id,
            map: b.resources.map_id.as_str().into(),
            map_revision: b.resources.map_revision.get(),
            avatar: b.resources.avatar_manifest_id.as_str().into(),
            avatar_revision: b.resources.avatar_manifest_version.get(),
        }
    }
    fn restore(self, user: Uuid, identity: (Uuid, Uuid, Uuid)) -> Result<Binding, DeletionFailure> {
        if self.user != user
            || self.event != identity.0
            || self.owner != identity.1
            || [
                self.event,
                self.user,
                self.session,
                self.connection,
                self.owner,
                self.room,
                self.room_epoch,
                self.stream,
            ]
            .iter()
            .any(Uuid::is_nil)
        {
            return Err(DeletionFailure::Unavailable);
        }
        Ok(Binding {
            key: ParticipantKey {
                event_id: EventId::from_uuid(self.event),
                user_id: AuthUserId::from_uuid(self.user),
            },
            session_id: self.session,
            connection_id: self.connection,
            owner_id: self.owner,
            room_id: self.room,
            room_epoch: self.room_epoch,
            stream_id: self.stream,
            resources: super::simulation::ResourcePin {
                map_id: AssetId::new(self.map).map_err(unavailable)?,
                map_revision: Revision::new(self.map_revision).map_err(unavailable)?,
                avatar_manifest_id: AssetId::new(self.avatar).map_err(unavailable)?,
                avatar_manifest_version: Revision::new(self.avatar_revision)
                    .map_err(unavailable)?,
            },
        })
    }
}
fn payload(b: &Binding) -> Result<(Vec<u8>, serde_json::Value), DeletionFailure> {
    let value = serde_json::to_value(NativeBinding::capture(b)).map_err(unavailable)?;
    let mut h = Sha256::new();
    h.update(b"MCR-SESSIONS-NATIVE-VICTIM-1\0");
    let canonical: BTreeMap<_, _> = value
        .as_object()
        .ok_or(DeletionFailure::Unavailable)?
        .iter()
        .collect();
    h.update(serde_json::to_vec(&canonical).map_err(unavailable)?);
    Ok((h.finalize().to_vec(), value))
}
fn unavailable(_: impl std::fmt::Debug) -> DeletionFailure {
    DeletionFailure::Unavailable
}

/// Construct from the *serving* socket state. No default/empty transport exists.
/// Exact native obligations are journaled durably before local withdrawal;
/// a re-created dispatcher resumes them without relying on a copied registry.
pub struct SessionOwner {
    bridge: Arc<AdmissionBridge>,
    runtime: Arc<RuntimeHandle>,
    transport: SocketTransport,
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
        })
    }
    fn identity(&self) -> (Uuid, Uuid, Uuid) {
        let (_, process, epoch, event) = self.bridge.invalidation_context();
        (event, process, epoch)
    }
    async fn withdraw(
        &self,
        deletion: &SessionDeletion,
        command: &WorldDeletionCommand,
        verified: &common::VerifiedDeletionSource,
        deadline: Instant,
        budget: &mut usize,
    ) -> Result<bool, DeletionFailure> {
        let user = command.user_id;
        let identity = self.identity();
        let _gate = self.bridge.invalidation_admission_gate().lock().await;
        let mut captured = BTreeMap::new();
        for binding in self
            .bridge
            .account_bindings(user)
            .map_err(unavailable)?
            .into_iter()
            .chain(self.transport.account_bindings(user).map_err(unavailable)?)
        {
            let (id, value) = payload(&binding)?;
            captured.insert(id, value);
        }
        // Both source registries are bounded (800 bridge + 864 socket entries).
        // Persist the complete bounded discovery page BEFORE removing any view.
        if captured.len() > 1664 {
            return Err(DeletionFailure::Unavailable);
        }
        if !captured.is_empty() {
            let mut tx = deletion.pool.begin().await.map_err(unavailable)?;
            common::bound_transaction_in(&mut tx, deadline).await?;
            common::lock_user(&mut tx, user)
                .await
                .map_err(unavailable)?;
            let mut entries = Vec::new();
            for (id, value) in &captured {
                serde_json::from_value::<NativeBinding>(value.clone())
                    .map_err(unavailable)?
                    .restore(user, identity)?;
                let hex = id
                    .iter()
                    .map(|b| format!("{b:02x}"))
                    .collect::<Vec<_>>()
                    .join("");
                entries.push(serde_json::json!({"id":hex,"binding":value}));
            }
            let entries = serde_json::Value::Array(entries);
            sqlx::query("insert into public.concert_world_session_deletion_victims(user_id,saga_id,victim_id,event_id,owner_process_id,owner_epoch,binding) select $1,$2,decode(x.id,'hex'),$4,$5,$6,x.binding from jsonb_to_recordset($3) as x(id text,binding jsonb) on conflict do nothing")
                .bind(user).bind(command.saga_id).bind(&entries).bind(identity.0).bind(identity.1).bind(identity.2)
                .execute(&mut *tx).await.map_err(unavailable)?;
            let exact:bool=sqlx::query_scalar("select count(*)=jsonb_array_length($3)::bigint from jsonb_to_recordset($3) as x(id text,binding jsonb) join public.concert_world_session_deletion_victims v on v.user_id=$1 and v.victim_id=decode(x.id,'hex') where v.saga_id=$2 and v.event_id=$4 and v.owner_process_id=$5 and v.owner_epoch=$6 and v.binding=x.binding")
                .bind(user).bind(command.saga_id).bind(&entries).bind(identity.0).bind(identity.1).bind(identity.2)
                .fetch_one(&mut *tx).await.map_err(unavailable)?;
            if !exact {
                return Err(DeletionFailure::Unavailable);
            }
            deletion.recheck(command, deadline, verified).await?;
            tx.commit().await.map_err(unavailable)?;
        }
        let removed = self
            .bridge
            .withdraw_account_views(user)
            .map_err(unavailable)?;
        let sockets = self.transport.withdraw_account(user).map_err(unavailable)?;
        for b in removed.into_iter().chain(sockets) {
            if !captured.contains_key(&payload(&b)?.0) {
                return Err(DeletionFailure::Unavailable);
            }
        }
        let rows=sqlx::query("select victim_id,binding from public.concert_world_session_deletion_victims where user_id=$1 and saga_id=$2 and event_id=$3 and owner_process_id=$4 and owner_epoch=$5 and completed_at is null order by victim_id limit $6")
            .bind(user).bind(command.saga_id).bind(identity.0).bind(identity.1).bind(identity.2).bind(*budget as i64)
            .fetch_all(&deletion.pool).await.map_err(unavailable)?;
        let mut completed = Vec::new();
        for row in rows {
            let id: Vec<u8> = row.try_get("victim_id").map_err(unavailable)?;
            let value: serde_json::Value = row.try_get("binding").map_err(unavailable)?;
            let binding = serde_json::from_value::<NativeBinding>(value)
                .map_err(unavailable)?
                .restore(user, identity)?;
            if payload(&binding)?.0 != id {
                return Err(DeletionFailure::Unavailable);
            }
            let (reply, answer) = oneshot::channel();
            self.runtime
                .try_submit(Command::Disconnect { binding, reply })
                .map_err(unavailable)?;
            answer.await.map_err(unavailable)?.map_err(unavailable)?;
            completed.push(id);
            *budget -= 1;
        }
        if !completed.is_empty() {
            let mut tx = deletion.pool.begin().await.map_err(unavailable)?;
            common::bound_transaction_in(&mut tx, deadline).await?;
            common::lock_user(&mut tx, user)
                .await
                .map_err(unavailable)?;
            for id in completed {
                sqlx::query("update public.concert_world_session_deletion_victims set completed_at=clock_timestamp() where user_id=$1 and victim_id=$2 and completed_at is null")
                    .bind(user).bind(id).execute(&mut *tx).await.map_err(unavailable)?;
            }
            deletion.recheck(command, deadline, verified).await?;
            tx.commit().await.map_err(unavailable)?;
        }
        let pending:bool=sqlx::query_scalar("select exists(select 1 from public.concert_world_session_deletion_victims where user_id=$1 and event_id=$2 and owner_process_id=$3 and owner_epoch=$4 and completed_at is null)")
            .bind(user).bind(identity.0).bind(identity.1).bind(identity.2).fetch_one(&deletion.pool).await.map_err(unavailable)?;
        Ok(!pending)
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
            drained &= owner
                .withdraw(self, &command, &verified, deadline, &mut budget)
                .await?;
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
        let rows = sqlx::query("select distinct m.event_id,m.owner_process_id,m.owner_epoch from public.concert_world_memberships m left join public.concert_world_owner_leases l on (l.event_id,l.room_id,l.owner_process_id,l.owner_epoch)=(m.event_id,m.room_id,m.owner_process_id,m.owner_epoch) where m.user_id=$1 and (m.lease_until>clock_timestamp() or l.lease_until>clock_timestamp()) union select event_id,owner_process_id::text,owner_epoch from public.concert_world_session_deletion_victims where user_id=$1 and completed_at is null limit 17")
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
