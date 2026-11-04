//! Bounded PrivateData core erase over the canonical World deletion command.

use crate::world::account_deletion::{
    lock_user_in, verify_source, DeletionAuthority, DeletionFailure, VerifiedDeletionSource,
    WorldDeletionCommand, WorldDeletionTarget,
};
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Postgres, Row, Transaction};
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;
use uuid::Uuid;

pub const MAX_DISPATCH: Duration = Duration::from_secs(5);
pub const PAGE_SIZE: i64 = 128;
pub const NEUTRAL_THEME: &str = "PARADE";

#[derive(Clone, PartialEq, Eq)]
pub struct PrivateDataCoreAcknowledgement {
    pub command: WorldDeletionCommand,
    pub fence_revision: u64,
    pub scanned_events: u64,
    pub neutralized_participants: u64,
    pub removed_avatars: u64,
}

impl PrivateDataCoreAcknowledgement {
    pub fn verifies(&self, command: &WorldDeletionCommand) -> bool {
        self.fence_revision > 0 && &self.command == command
    }
}

impl std::fmt::Debug for PrivateDataCoreAcknowledgement {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("PrivateDataCoreAcknowledgement([REDACTED])")
    }
}

pub fn check_deadline(deadline: Instant) -> Result<(), DeletionFailure> {
    let now = Instant::now();
    if deadline <= now || deadline.duration_since(now) > MAX_DISPATCH {
        return Err(DeletionFailure::Unavailable);
    }
    Ok(())
}

pub struct PrivateDataCoreDeletion {
    pool: PgPool,
    authority: Arc<dyn DeletionAuthority>,
}

impl PrivateDataCoreDeletion {
    pub fn new(pool: PgPool, authority: Arc<dyn DeletionAuthority>) -> Self {
        Self { pool, authority }
    }
}

fn db_error(_: sqlx::Error) -> DeletionFailure {
    DeletionFailure::Unavailable
}

async fn bound_transaction(
    tx: &mut Transaction<'_, Postgres>,
    deadline: Instant,
) -> Result<(), DeletionFailure> {
    check_deadline(deadline)?;
    let millis = deadline
        .saturating_duration_since(Instant::now())
        .as_millis()
        .max(1)
        .min(5000);
    let value = format!("{millis}ms");
    sqlx::query(
        "select set_config('statement_timeout',$1,true),set_config('lock_timeout',$1,true)",
    )
    .bind(value)
    .execute(&mut **tx)
    .await
    .map_err(db_error)?;
    Ok(())
}

impl PrivateDataCoreDeletion {
    pub async fn apply(
        &self,
        command: WorldDeletionCommand,
        original_deadline: Instant,
    ) -> Result<PrivateDataCoreAcknowledgement, DeletionFailure> {
        command.validate()?;
        if command.target != WorldDeletionTarget::PrivateData {
            return Err(DeletionFailure::InvalidCommand);
        }
        check_deadline(original_deadline)?;
        tokio::time::timeout_at(
            original_deadline,
            self.apply_in_deadline(command, original_deadline),
        )
        .await
        .map_err(|_| DeletionFailure::Unavailable)?
    }

    async fn apply_in_deadline(
        &self,
        command: WorldDeletionCommand,
        deadline: Instant,
    ) -> Result<PrivateDataCoreAcknowledgement, DeletionFailure> {
        let verified = verify_source(self.authority.as_ref(), &command, deadline).await?;
        // The command binding is checked once. The durable receipt supplies the
        // exact saga/actor/source identity and the cursor for every page.
        let mut result = None;
        // Completion requires the cursor to reach the end of the user's rows.
        loop {
            let (done, scanned, neutralized, removed) =
                self.apply_page(&command, &verified, deadline).await?;
            if done {
                result = Some((scanned, neutralized, removed));
                break;
            }
        }
        let (scanned, neutralized, removed) = result.ok_or(DeletionFailure::Unavailable)?;
        Ok(PrivateDataCoreAcknowledgement {
            command,
            fence_revision: 1,
            scanned_events: scanned as u64,
            neutralized_participants: neutralized as u64,
            removed_avatars: removed as u64,
        })
    }

    async fn apply_page(
        &self,
        command: &WorldDeletionCommand,
        verified: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<(bool, i64, i64, i64), DeletionFailure> {
        verified.check(command, deadline)?;
        let mut tx = self.pool.begin().await.map_err(db_error)?;
        bound_transaction(&mut tx, deadline).await?;

        // The shared user lock must precede every event, participant and avatar
        // row. Writers use this identical lock before their personal rows.
        lock_user_in(&mut tx, command.user_id)
            .await
            .map_err(db_error)?;

        sqlx::query("insert into public.concert_private_data_deletion_core(user_id,saga_id,actor_id,retention_until,command_binding,source_binding) values($1,$2,$3,$4,$5,$6) on conflict(user_id) do nothing")
            .bind(command.user_id)
            .bind(command.saga_id)
            .bind(command.actor_id)
            .bind(command.retention_until)
            .bind(command.binding()?.as_slice())
            .bind(verified.source_binding().as_slice())
            .execute(&mut *tx)
            .await
            .map_err(db_error)?;

        let row = sqlx::query("select saga_id,actor_id,retention_until,command_binding,source_binding,next_event_id,state,scanned_events,neutralized_participants,removed_avatars,completed_at from public.concert_private_data_deletion_core where user_id=$1 for update")
            .bind(command.user_id)
            .fetch_one(&mut *tx)
            .await
            .map_err(db_error)?;
        if row.try_get::<Uuid, _>("saga_id").map_err(db_error)? != command.saga_id
            || row.try_get::<Uuid, _>("actor_id").map_err(db_error)? != command.actor_id
            || row
                .try_get::<DateTime<Utc>, _>("retention_until")
                .map_err(db_error)?
                != command.retention_until
            || row
                .try_get::<Vec<u8>, _>("command_binding")
                .map_err(db_error)?
                != command.binding()?
            || row
                .try_get::<Vec<u8>, _>("source_binding")
                .map_err(db_error)?
                != *verified.source_binding()
        {
            return Err(DeletionFailure::InvalidCommand);
        }

        let mut complete = row
            .try_get::<Option<DateTime<Utc>>, _>("completed_at")
            .map_err(db_error)?
            .is_some();
        let mut scanned = row.try_get::<i64, _>("scanned_events").map_err(db_error)?;
        let mut neutralized = row
            .try_get::<i64, _>("neutralized_participants")
            .map_err(db_error)?;
        let mut avatars = row.try_get::<i64, _>("removed_avatars").map_err(db_error)?;
        if !complete {
            sqlx::query("insert into public.concert_private_data_participant_fences(user_id,saga_id) values($1,$2) on conflict(user_id) do nothing")
                .bind(command.user_id)
                .bind(command.saga_id)
                .execute(&mut *tx)
                .await
                .map_err(db_error)?;

            // This setting is transaction-local and matches only the command's
            // exact user. Any caller can use it only after the trusted authority
            // and the shared user-first lock have both been established.
            sqlx::query("select set_config('mcr.private_data_deletion_user',$1,true)")
                .bind(command.user_id.to_string())
                .execute(&mut *tx)
                .await
                .map_err(db_error)?;

            let next_event: Option<Uuid> = row.try_get("next_event_id").map_err(db_error)?;
            let events = sqlx::query("select event_id from public.concert_participants where user_id=$1 and ($2::uuid is null or event_id>$2) order by event_id limit $3")
                .bind(command.user_id)
                .bind(next_event)
                .bind(PAGE_SIZE)
                .fetch_all(&mut *tx)
                .await
                .map_err(db_error)?;
            scanned += events.len() as i64;
            let no_more = events.is_empty();
            let mut page_end = next_event;
            for event in &events {
                let event_id: Uuid = event.try_get("event_id").map_err(db_error)?;
                page_end = Some(event_id);
                let participant = sqlx::query("update public.concert_participants set revision=revision+1,theme=$3,low_power=false,reduced_motion=false,sound_enabled=false where event_id=$1 and user_id=$2 returning revision")
                    .bind(event_id)
                    .bind(command.user_id)
                    .bind(NEUTRAL_THEME)
                    .fetch_optional(&mut *tx)
                    .await
                    .map_err(db_error)?;
                if participant.is_some() {
                    neutralized += 1;
                }
                let removed = sqlx::query(
                    "delete from public.concert_avatar_settings where event_id=$1 and user_id=$2",
                )
                .bind(event_id)
                .bind(command.user_id)
                .execute(&mut *tx)
                .await
                .map_err(db_error)?
                .rows_affected();
                avatars += removed as i64;
            }
            sqlx::query("select set_config('mcr.private_data_deletion_user','',true)")
                .execute(&mut *tx)
                .await
                .map_err(db_error)?;

            // A complete page is resumable; only an empty final page proves that
            // every event row was traversed. The participant keys remain because
            // dependent aggregates reference them, but carry no personal choice.
            complete = no_more;
            if !complete {
                page_end = Some(page_end.ok_or(DeletionFailure::Unavailable)?);
            }
            let next_state = if complete { "COMPLETE" } else { "INCOMPLETE" };
            let update = sqlx::query("update public.concert_private_data_deletion_core set state=$2,scanned_events=$3,neutralized_participants=$4,removed_avatars=$5,progress_revision=progress_revision+1,completed_at=case when $6 then clock_timestamp() else null end,next_event_id=$7 where user_id=$1")
                .bind(command.user_id)
                .bind(next_state)
                .bind(scanned)
                .bind(neutralized)
                .bind(avatars)
                .bind(complete)
                .bind(page_end)
                .execute(&mut *tx)
                .await;
            if let Err(error) = update {
                return Err(db_error(error));
            }
        }

        let again = verify_source(self.authority.as_ref(), command, deadline).await?;
        if &again != verified {
            return Err(DeletionFailure::Unavailable);
        }
        check_deadline(deadline)?;
        let state: String = sqlx::query_scalar(
            "select state from public.concert_private_data_deletion_core where user_id=$1",
        )
        .bind(command.user_id)
        .fetch_one(&mut *tx)
        .await
        .map_err(db_error)?;
        tx.commit().await.map_err(db_error)?;
        Ok((state == "COMPLETE", scanned, neutralized, avatars))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn command() -> WorldDeletionCommand {
        WorldDeletionCommand {
            saga_id: Uuid::from_u128(1),
            user_id: Uuid::from_u128(2),
            target: WorldDeletionTarget::PrivateData,
            actor_id: Uuid::from_u128(2),
            retention_until: "2070-01-01T00:00:00.123456789Z".parse().unwrap(),
        }
    }

    #[test]
    fn exact_command_binding_and_redaction() {
        let c = command();
        let mut other = c.clone();
        other.retention_until += chrono::Duration::nanoseconds(1);
        assert_ne!(c.binding().unwrap(), other.binding().unwrap());
        assert_eq!(format!("{c:?}"), "WorldDeletionCommand([REDACTED])");
    }

    #[test]
    fn deadline_and_source_are_bound() {
        let c = command();
        let d = Instant::now() + Duration::from_secs(4);
        assert!(VerifiedDeletionSource::from_verified_auth_source(&c, d, [0; 32]).is_err());
        let proof = VerifiedDeletionSource::from_verified_auth_source(&c, d, [1; 32]).unwrap();
        assert_eq!(
            proof.check(&c, d + Duration::from_millis(1)),
            Err(DeletionFailure::InvalidCommand)
        );
    }
}
