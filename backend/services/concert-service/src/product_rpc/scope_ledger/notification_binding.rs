//! Completion-only restoration of the immutable original root binding.
//! This lookup grants no content, transaction settlement or owner authority.
use super::PgScopeLedger;
use crate::{
    adapters::out::relationship_authority::guarded::{proto, ChildQuery},
    notification_live::NotificationRecoveryBinding,
};
use platform::actor_delegation::owner_binding::OwnerBinding;
use std::time::Duration;
use tonic::Status;
use uuid::Uuid;

fn unavailable() -> Status {
    Status::unavailable("Notification durable root binding unavailable")
}

// Match the four existing Notification operation/path pairs. Keep the original
// query bytes in the returned target; they remain covered by the fingerprint.
fn notification_operation(binding: &OwnerBinding) -> bool {
    let base = format!("/api/concert/events/{}/me/notifications", binding.event_id);
    let path = binding
        .http_target
        .split_once('?')
        .map_or(binding.http_target.as_str(), |v| v.0);
    if binding.parent_method != "/concert.product.v1.ProductService/Dispatch" {
        return false;
    }
    match (binding.operation.as_str(), binding.http_method.as_str()) {
        ("listConcertNotifications", "GET") => path == base,
        ("getConcertNotificationUnreadCount", "GET") => path == format!("{base}/unread-count"),
        ("markAllConcertNotifications", "PATCH") => path == format!("{base}/read-all"),
        ("markConcertNotification", "PATCH") => path
            .strip_prefix(&format!("{base}/"))
            .and_then(|v| v.strip_suffix("/read"))
            .and_then(|v| Uuid::parse_str(v).ok().map(|id| (v, id)))
            .is_some_and(|(v, id)| !id.is_nil() && id.to_string() == v),
        _ => false,
    }
}

#[async_trait::async_trait]
impl NotificationRecoveryBinding for PgScopeLedger {
    async fn load_binding(&self, query: &ChildQuery) -> Result<proto::GuardBinding, Status> {
        if query.subject != query.actor
            || [query.actor, query.event, query.family, query.request]
                .iter()
                .any(Uuid::is_nil)
            || query.binding_digest == [0; 32]
        {
            return Err(unavailable());
        }
        // Bound pool acquisition as well as SQL. This is a mechanical recovery
        // budget, not a renewal of the stored (possibly expired) content expiry.
        let budget = platform::call_deadline::bounded(Duration::from_millis(750));
        if budget.is_zero() {
            return Err(unavailable());
        }
        tokio::time::timeout(budget, async {
            let mut tx = self.transaction().await.map_err(|_| unavailable())?;
            // ChildQuery has no scope ID. Refuse ambiguity across ALL phases;
            // filtering ROOT_FINISHED here could hide another matching scope.
            let rows: Vec<(String, serde_json::Value)> = sqlx::query_as(
                "SELECT phase,binding FROM public.concert_product_owner_scopes WHERE instance_id=$1 AND binding_digest=$2 LIMIT 2",
            )
            .bind(self.instance)
            .bind(query.binding_digest.as_slice())
            .fetch_all(&mut *tx)
            .await
            .map_err(|_| unavailable())?;
            let [(phase, value)] = rows.as_slice() else {
                return Err(unavailable());
            };
            if phase != "ROOT_FINISHED" {
                return Err(unavailable());
            }
            let binding: OwnerBinding =
                serde_json::from_value(value.clone()).map_err(|_| unavailable())?;
            // digest() validates every stored field without checking wall time.
            if binding.digest().map_err(|_| unavailable())? != query.binding_digest
                || binding.actor_id != query.actor
                || binding.event_id != query.event
                || binding.session_family_id != query.family
                || binding.request_id != query.request
                || binding.expires_unix_micros != query.expires_unix_micros
                || !notification_operation(&binding)
            {
                return Err(unavailable());
            }
            // Original OwnerGuardScope::wire_binding() uses wire([]); enrolled
            // subjects are not part of the immutable root fingerprint.
            let wire = proto::GuardBinding {
                actor_id: binding.actor_id.to_string(),
                event_id: binding.event_id.to_string(),
                session_family_id: binding.session_family_id.to_string(),
                request_id: binding.request_id.to_string(),
                operation: binding.operation,
                subject_ids: Vec::new(),
                expires_unix_micros: binding.expires_unix_micros,
                parent_digest: binding.parent_digest.to_vec(),
                parent_body_digest: binding.parent_body_digest.to_vec(),
                parent_method: binding.parent_method,
                http_method: binding.http_method,
                http_target: binding.http_target,
                body_digest: binding.body_digest.to_vec(),
            };
            tx.commit().await.map_err(|_| unavailable())?;
            Ok(wire)
        })
        .await
        .map_err(|_| unavailable())?
    }
}
