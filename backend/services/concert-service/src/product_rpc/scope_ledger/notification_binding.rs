//! Completion-only restoration of the immutable original root binding.
//! This lookup grants no content, transaction settlement or owner authority.
use super::PgScopeLedger;
use crate::{
    adapters::out::relationship_authority::guarded::{proto, ChildQuery},
    notification_live::NotificationRecoveryBinding,
};
use platform::actor_delegation::owner_binding::OwnerBinding;
use sqlx::{PgPool, Postgres, Transaction};
use std::time::Duration;
use tonic::Status;
use uuid::Uuid;

fn unavailable() -> Status {
    Status::unavailable("Notification durable root binding unavailable")
}

/// Completion-only database view, constructible before the child settler/core.
/// A pool clone shares the configured pool; no ledger, registry or authority is
/// created or retained. Keep this composition detail within the service crate.
#[derive(Clone)]
pub(crate) struct PgNotificationBindingReader {
    pool: PgPool,
    instance: Uuid,
}
impl PgNotificationBindingReader {
    pub(crate) fn new(pool: PgPool, instance: Uuid) -> Result<Self, Status> {
        if instance.is_nil() {
            return Err(super::invalid());
        }
        Ok(Self { pool, instance })
    }

    async fn transaction(&self) -> Result<Transaction<'static, Postgres>, Status> {
        let mut tx = self.pool.begin().await.map_err(|_| unavailable())?;
        sqlx::query("SET LOCAL lock_timeout='250ms'")
            .execute(&mut *tx)
            .await
            .map_err(|_| unavailable())?;
        sqlx::query("SET LOCAL statement_timeout='750ms'")
            .execute(&mut *tx)
            .await
            .map_err(|_| unavailable())?;
        Ok(tx)
    }
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
        PgNotificationBindingReader::new(self.pool.clone(), self.instance)?
            .load_binding(query)
            .await
    }
}

#[async_trait::async_trait]
impl NotificationRecoveryBinding for PgNotificationBindingReader {
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate as binding_test_types;
    use crate::{
        notification_live::NotificationClient,
        product_rpc::child_settlement::NotificationInboxIntentSettler,
    };
    use std::sync::Arc;
    use tonic::Code;

    mod support {
        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/notification_root_binding_support/mod.rs"
        ));
    }

    fn same_refusal(left: Status, right: Status) {
        assert_eq!(left.code(), Code::Unavailable);
        assert_eq!(left.code(), right.code());
        assert_eq!(left.message(), right.message());
    }

    #[tokio::test]
    async fn constructor_rejects_nil_and_shares_only_the_existing_pool() {
        let pool = support::pool().await;
        assert_eq!(
            PgNotificationBindingReader::new(pool.clone(), Uuid::nil())
                .err()
                .expect("nil instance must fail")
                .code(),
            Code::InvalidArgument
        );
        let instance = Uuid::new_v4();
        let size = pool.size();
        let reader = PgNotificationBindingReader::new(pool.clone(), instance).unwrap();
        assert_eq!(
            pool.size(),
            size,
            "construction must not acquire a connection"
        );
        // Exhaustive destructuring also keeps constructor dependencies explicit.
        let PgNotificationBindingReader {
            pool: shared,
            instance: actual,
        } = reader;
        assert_eq!(actual, instance);
        shared.close().await;
        assert!(
            pool.is_closed(),
            "reader must share, not recreate, the pool"
        );
    }

    #[tokio::test]
    async fn reader_and_ledger_share_exact_wire_and_fail_closed_results() {
        let pool = support::pool().await;
        let instance = Uuid::new_v4();
        let reader = PgNotificationBindingReader::new(pool.clone(), instance).unwrap();
        // Reader construction has no child keys/settler/ledger prerequisite.
        let ledger = support::ledger(&pool, instance);
        let b = support::binding();
        let q = support::query(&b);
        let missing = reader.load_binding(&q).await.unwrap_err();
        assert_eq!(missing.code(), Code::Unavailable);
        same_refusal(missing, ledger.load_binding(&q).await.unwrap_err());
        support::store(
            &pool,
            instance,
            q.binding_digest,
            serde_json::to_value(&b).unwrap(),
            "ROOT_FINISHED",
        )
        .await;
        let original = reader.load_binding(&q).await.unwrap();
        assert_eq!(original, ledger.load_binding(&q).await.unwrap());
        assert_eq!(original.expires_unix_micros, b.expires_unix_micros);
        assert!(b.expires_unix_micros < chrono::Utc::now().timestamp_micros());
        let mut foreign_subject = q.clone();
        foreign_subject.subject = Uuid::new_v4();
        same_refusal(
            reader.load_binding(&foreign_subject).await.unwrap_err(),
            ledger.load_binding(&foreign_subject).await.unwrap_err(),
        );
        // An unfinished duplicate must not be hidden by terminal filtering.
        support::store(
            &pool,
            instance,
            q.binding_digest,
            serde_json::to_value(&b).unwrap(),
            "ACTIVE",
        )
        .await;
        let ambiguous = reader.load_binding(&q).await.unwrap_err();
        assert_eq!(ambiguous.code(), Code::Unavailable);
        same_refusal(ambiguous, ledger.load_binding(&q).await.unwrap_err());
        pool.close().await;
    }

    #[tokio::test]
    async fn notification_client_can_own_reader_before_any_ledger_or_settler() {
        let pool = support::pool().await;
        let reader =
            Arc::new(PgNotificationBindingReader::new(pool.clone(), Uuid::new_v4()).unwrap());
        assert_eq!(Arc::strong_count(&reader), 1);
        // Real lazy transport construction only: no request, owner proof,
        // completion secret, positive response, ledger or registry is fabricated.
        let client: Arc<dyn NotificationInboxIntentSettler> = Arc::new(
            NotificationClient::connect(
                "http://127.0.0.1:1",
                "nb02-construction-only",
                None,
                Some(reader.clone()),
            )
            .unwrap(),
        );
        assert_eq!(Arc::strong_count(&reader), 2);
        drop(client);
        assert_eq!(
            Arc::strong_count(&reader),
            1,
            "client/reader ownership must not form a cycle"
        );
        drop(reader);
        pool.close().await;
    }
}
