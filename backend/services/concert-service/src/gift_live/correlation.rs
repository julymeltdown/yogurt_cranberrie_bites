//! Exact validated command intent, consumed only by the receipt INSERT trigger.
//! A transaction-local intent is correlation, never an authority grant.
use super::protocol::{GiftInput, GiftRequest};
use concert_application::{commands::CommandError, community::giveaway::GiftCommand};
use concert_domain::community::giveaway::GiftAction;
use sqlx::PgConnection;
use uuid::Uuid;

#[cfg(test)]
#[path = "native_targets.rs"]
pub(crate) mod native_tests;

#[derive(Clone)]
pub(crate) struct CommandTarget {
    actor: Uuid,
    event: Uuid,
    operation: &'static str,
    key: Uuid,
    target: Uuid,
    giveaway: Option<Uuid>,
}

pub(crate) async fn require_receipt(
    pool: &sqlx::PgPool,
    command: &GiftCommand,
    receipt: &concert_application::commands::SafeReceipt,
) -> Result<(), CommandError> {
    if receipt.scope != command.scope()? {
        return Err(CommandError::Unavailable);
    }
    let present: bool = sqlx::query_scalar("select exists(select 1 from concert_gift_command_targets t join concert_command_receipts r on r.receipt_id=t.receipt_id and r.actor_id=t.actor_id and r.event_id=t.event_id and r.operation_id=t.operation_id and r.idempotency_key=t.idempotency_key and r.target_id=t.target_id where t.actor_id=$1 and t.event_id=$2 and t.operation_id=$3 and t.idempotency_key=$4 and t.target_id=$5 and t.giveaway_id=$6 and t.receipt_id=$7)")
        .bind(command.actor).bind(command.event).bind(command.operation()).bind(command.key)
        .bind(command.target()).bind(command.giveaway_id()).bind(receipt.id)
        .fetch_one(pool).await.map_err(|_| CommandError::Unavailable)?;
    if present {
        Ok(())
    } else {
        Err(CommandError::Unavailable)
    }
}
impl CommandTarget {
    pub(crate) fn from_request(request: &GiftRequest) -> Result<Option<Self>, CommandError> {
        let GiftInput::Write {
            giveaway, action, ..
        } = request.input()
        else {
            return Ok(None);
        };
        let i = request.identity();
        // Only deterministic IDs are derived here. No request/body fingerprint or
        // replacement action is invented, and plaintext pickup code is not copied.
        let command = GiftCommand {
            actor: i.actor,
            event: i.event,
            key: request
                .idempotency_key()
                .ok_or(CommandError::InvalidInput)?,
            giveaway: giveaway.unwrap_or(Uuid::nil()),
            action: action.clone(),
            code: None,
        };
        let root = match action {
            GiftAction::Create(_)
            | GiftAction::Update { .. }
            | GiftAction::SetMeeting { .. }
            | GiftAction::Reserve { .. } => Some(command.giveaway_id()),
            _ => None,
        };
        if root.is_some_and(|id| id.is_nil()) || command.target().is_nil() {
            return Err(CommandError::InvalidInput);
        }
        Ok(Some(Self {
            actor: i.actor,
            event: i.event,
            operation: command.operation(),
            key: command.key,
            target: command.target(),
            giveaway: root,
        }))
    }
    /// Called after actual canonical/local subject authorization. It writes no
    /// durable row. Only a receipt INSERT in this same transaction materializes
    /// the mapping, before the command savepoint can record a safe FAILED result.
    pub(crate) async fn install_in(&self, c: &mut PgConnection) -> Result<(), CommandError> {
        // Fail before receipt admission if 0033 is absent/disabled. Hold these
        // ordinary table locks through commit so concurrent DDL cannot remove
        // the trigger after this check and before the actual receipt INSERT.
        sqlx::query("lock table concert_command_receipts in row exclusive mode")
            .execute(&mut *c)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        sqlx::query("lock table concert_gift_command_targets in row share mode")
            .execute(&mut *c)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        let enabled: bool = sqlx::query_scalar("select exists(select 1 from pg_trigger where tgrelid='concert_command_receipts'::regclass and tgname='concert_capture_gift_command_target' and tgenabled in ('O','A') and not tgisinternal)")
            .fetch_one(&mut *c).await.map_err(|_| CommandError::Unavailable)?;
        if !enabled {
            return Err(CommandError::Unavailable);
        }
        let root = match self.giveaway {
            Some(root) => root,
            None => sqlx::query_scalar::<_, Uuid>(
                "select giveaway_id from concert_community_reservations where event_id=$1 and reservation_id=$2",
            )
            .bind(self.event).bind(self.target).fetch_optional(&mut *c).await
            .map_err(|_| CommandError::Unavailable)?.ok_or(CommandError::Denied)?,
        };
        let context = serde_json::json!({
            "v":1,"actor_id":self.actor,"event_id":self.event,
            "operation_id":self.operation,"idempotency_key":self.key,
            "target_id":self.target,"giveaway_id":root,
        });
        let old: Option<String> = sqlx::query_scalar(
            "select nullif(current_setting('mcr.gift_live_command_target',true),'')",
        )
        .fetch_one(&mut *c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if let Some(old) = old {
            if serde_json::from_str::<serde_json::Value>(&old)
                .ok()
                .as_ref()
                != Some(&context)
            {
                return Err(CommandError::Denied);
            }
        }
        sqlx::query("select set_config('mcr.gift_live_command_target',$1,true)")
            .bind(context.to_string())
            .execute(c)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        Ok(())
    }
}
