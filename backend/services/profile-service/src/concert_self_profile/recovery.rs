//! Durable discovery after disconnect/restart. Fresh recovery time inspects only
//! existing effects; it can never extend mutation admission or create a write.
use super::{
    model::Error,
    operation::Operation,
    server::{SelfProfileService, BUDGET},
};
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use uuid::Uuid;
impl SelfProfileService {
    pub async fn reconcile_pending(&self) -> anyhow::Result<usize> {
        let deadline =
            AuthorityRpcDeadline::new(chrono::Utc::now().timestamp_micros() + 5_000_000, BUDGET)?;
        let work = async {
            let ids:Vec<Uuid>=sqlx::query_scalar("select o.operation_id from concert_self_profile_operations o left join concert_self_profile_settled_observations s on s.operation_id=o.operation_id where s.observed_status is null or s.observed_status='UNRESOLVED' order by s.observed_at nulls first,o.created_at,o.operation_id limit 16").fetch_all(&self.pool).await.map_err(|_|Error::Unavailable)?;
            let mut examined = 0;
            for id in ids {
                super::identity::remaining(deadline)?;
                if let Some(operation) = Operation::load(&self.pool, id, deadline).await? {
                    // One unavailable coordinator leaves this operation intact;
                    // it must not prevent unrelated existing effects progressing.
                    let _ = self.settle(&operation, deadline).await;
                    examined += 1;
                }
            }
            Ok::<_, Error>(examined)
        };
        Ok(tokio::time::timeout_at(deadline.stop(), work)
            .await
            .map_err(|_| Error::Unavailable)??)
    }
}
