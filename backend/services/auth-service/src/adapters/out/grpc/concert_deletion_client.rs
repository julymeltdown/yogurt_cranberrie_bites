//! Auth background transport. Typed success requires an exact authenticated
//! Concert owner receipt, never an accepted job or a boolean response.
use auth_application::concert_deletion::*;
use platform::{
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use std::{sync::Arc, time::Duration};
use tokio::time::Instant;
use tonic::{service::interceptor::InterceptedService, transport::Channel};
pub mod proto {
    tonic::include_proto!("concert.account_deletion.v1");
}
type Client = proto::account_deletion_service_client::AccountDeletionServiceClient<
    InterceptedService<Channel, ServiceIdentityClientInterceptor>,
>;
#[derive(Clone)]
pub struct ConcertDeletionClient {
    client: Client,
}
fn unavailable() -> ConcertDeletionFailure {
    ConcertDeletionFailure::Unavailable
}
fn deadline(stop: Instant) -> Result<(), ConcertDeletionFailure> {
    let now = Instant::now();
    if stop <= now || stop.duration_since(now) > Duration::from_secs(5) {
        return Err(unavailable());
    }
    Ok(())
}
fn wire(c: &ConcertDeletionCommand) -> proto::Command {
    proto::Command {
        saga_id: c.key.saga_id.to_string(),
        user_id: c.key.user_id.to_string(),
        actor_id: c.actor_id.to_string(),
        target: match c.key.target {
            ConcertDeletionTarget::Tickets => 1,
            ConcertDeletionTarget::Sessions => 2,
            ConcertDeletionTarget::Sharing => 3,
            ConcertDeletionTarget::PrivateData => 4,
        },
        retention_seconds: c.retention_until.timestamp(),
        retention_nanos: c.retention_until.timestamp_subsec_nanos() as i32,
    }
}
impl ConcertDeletionClient {
    pub fn connect(endpoint: &str, tls: &TlsMaterial, credential: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(
            endpoint.starts_with("https://"),
            "Concert deletion requires HTTPS"
        );
        let channel = tls.endpoint(endpoint, Workload::Concert)?.connect_lazy();
        Ok(Self {
            client: Client::new(InterceptedService::new(
                channel,
                ServiceIdentityClientInterceptor::new(Workload::Auth, credential)?,
            ))
            .max_decoding_message_size(2048)
            .max_encoding_message_size(2048),
        })
    }
    /// Four existing application ports for the canonical composite dispatcher.
    /// Root must scope its ORIGINAL absolute clock around that dispatch; absence
    /// is unavailable rather than silently starting a fresh five-second budget.
    pub fn owners(&self) -> crate::concert_deletion::ConcertDeletionOwners {
        let client = Arc::new(self.clone());
        crate::concert_deletion::ConcertDeletionOwners {
            tickets: Some(client.clone()),
            sessions: Some(client.clone()),
            sharing: Some(client.clone()),
            private_data: Some(client),
        }
    }
    pub async fn apply_with_deadline(
        &self,
        command: ConcertDeletionCommand,
        original_deadline: Instant,
    ) -> Result<ConcertDeletionAcknowledgement, ConcertDeletionFailure> {
        command.validate()?;
        deadline(original_deadline)?;
        let stop = platform::call_deadline::current()
            .map_or(original_deadline, |p| original_deadline.min(p.into()));
        deadline(stop)?;
        let left = stop.saturating_duration_since(Instant::now());
        let expires = chrono::Utc::now()
            .timestamp_micros()
            .checked_add(left.as_micros() as i64)
            .ok_or_else(unavailable)?;
        let nonce = uuid::Uuid::new_v4().as_bytes().to_vec();
        let exact = wire(&command);
        let mut q = tonic::Request::new(proto::ApplyRequest {
            command: Some(exact.clone()),
            request_nonce: nonce.clone(),
            expires_unix_micros: expires,
        });
        q.set_timeout(left);
        let response = tokio::time::timeout_at(stop, self.client.clone().apply(q))
            .await
            .map_err(|_| unavailable())?
            .map_err(|status| match status.code() {
                tonic::Code::FailedPrecondition => ConcertDeletionFailure::Incomplete,
                tonic::Code::InvalidArgument => ConcertDeletionFailure::InvalidCommand,
                _ => unavailable(),
            })?;
        deadline(stop)?;
        deadline(original_deadline)?;
        if response
            .metadata()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            != Some("no-store")
        {
            return Err(unavailable());
        }
        let r = response.into_inner();
        if r.command.as_ref() != Some(&exact)
            || r.request_nonce != nonce
            || r.expires_unix_micros != expires
            || chrono::Utc::now().timestamp_micros() >= expires
            || r.fence_revision == 0
            || r.source_binding.len() != 32
            || r.source_binding.iter().all(|b| *b == 0)
        {
            return Err(unavailable());
        }
        // Source binding is an authenticated Concert assertion corroborated by
        // its real Auth-source checks, not a client-supplied portable authority.
        Ok(ConcertDeletionAcknowledgement {
            command,
            fence_revision: r.fence_revision,
        })
    }
    async fn typed(
        &self,
        command: ConcertDeletionCommand,
        target: ConcertDeletionTarget,
    ) -> Result<ConcertDeletionAcknowledgement, ConcertDeletionFailure> {
        if command.key.target != target {
            return Err(ConcertDeletionFailure::InvalidCommand);
        }
        let original = platform::call_deadline::current().ok_or_else(unavailable)?;
        self.apply_with_deadline(command, original.into()).await
    }
}
macro_rules! port {
    ($port:ident, $target:ident) => {
        #[async_trait::async_trait]
        impl $port for ConcertDeletionClient {
            async fn apply(
                &self,
                command: ConcertDeletionCommand,
            ) -> Result<ConcertDeletionAcknowledgement, ConcertDeletionFailure> {
                self.typed(command, ConcertDeletionTarget::$target).await
            }
        }
    };
}
port!(ConcertTicketDeletionPort, Tickets);
port!(ConcertSessionDeletionPort, Sessions);
port!(ConcertSharingDeletionPort, Sharing);
port!(ConcertPrivateDataDeletionPort, PrivateData);
