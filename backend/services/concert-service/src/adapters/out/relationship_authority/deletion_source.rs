//! Auth-only authenticated source consumer. A hash/raw command is never input
//! authority. No cache, retry, plaintext, actor impersonation or positive default.
use crate::world::account_deletion::{
    check_deadline, DeletionAuthority, DeletionFailure, VerifiedDeletionSource,
    WorldDeletionCommand, WorldDeletionTarget,
};
use platform::{
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use tokio::time::Instant;
use tonic::{service::interceptor::InterceptedService, transport::Channel};
pub mod proto {
    include!(concat!(
        env!("OUT_DIR"),
        "/deletion_source/authority.deletion.v1.rs"
    ));
}
type Client = proto::auth_deletion_source_service_client::AuthDeletionSourceServiceClient<
    InterceptedService<Channel, ServiceIdentityClientInterceptor>,
>;
#[derive(Clone)]
pub struct AuthDeletionSource {
    client: Client,
}
impl AuthDeletionSource {
    pub fn connect(endpoint: &str, tls: &TlsMaterial, credential: &str) -> anyhow::Result<Self> {
        anyhow::ensure!(
            endpoint.starts_with("https://"),
            "Auth deletion source requires HTTPS"
        );
        let channel = tls.endpoint(endpoint, Workload::Auth)?.connect_lazy();
        Ok(Self {
            client: Client::new(InterceptedService::new(
                channel,
                ServiceIdentityClientInterceptor::new(Workload::Concert, credential)?,
            ))
            .max_decoding_message_size(2048)
            .max_encoding_message_size(2048),
        })
    }
}
fn wire(c: &WorldDeletionCommand) -> proto::DeletionCommand {
    proto::DeletionCommand {
        saga_id: c.saga_id.to_string(),
        user_id: c.user_id.to_string(),
        actor_id: c.actor_id.to_string(),
        target: match c.target {
            WorldDeletionTarget::Tickets => 1,
            WorldDeletionTarget::Sessions => 2,
            WorldDeletionTarget::Sharing => 3,
            WorldDeletionTarget::PrivateData => 4,
        },
        retention_seconds: c.retention_until.timestamp(),
        retention_nanos: c.retention_until.timestamp_subsec_nanos() as i32,
    }
}
#[async_trait::async_trait]
impl DeletionAuthority for AuthDeletionSource {
    async fn verify(
        &self,
        c: &WorldDeletionCommand,
        original_deadline: Instant,
    ) -> Result<VerifiedDeletionSource, DeletionFailure> {
        c.validate()?;
        check_deadline(original_deadline)?;
        let stop = platform::call_deadline::current()
            .map_or(original_deadline, |p| original_deadline.min(p.into()));
        check_deadline(stop)?;
        let left = stop.saturating_duration_since(Instant::now());
        let nonce = uuid::Uuid::new_v4().as_bytes().to_vec();
        let expires = chrono::Utc::now()
            .timestamp_micros()
            .checked_add(left.as_micros() as i64)
            .ok_or(DeletionFailure::Unavailable)?;
        let command = wire(c);
        let mut request = tonic::Request::new(proto::VerifyDeletionSourceRequest {
            command: Some(command.clone()),
            request_nonce: nonce.clone(),
            expires_unix_micros: expires,
        });
        request.set_timeout(left);
        let response = tokio::time::timeout_at(stop, self.client.clone().verify_source(request))
            .await
            .map_err(|_| DeletionFailure::Unavailable)?
            .map_err(|_| DeletionFailure::Unavailable)?;
        check_deadline(stop)?;
        check_deadline(original_deadline)?;
        if response
            .metadata()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            != Some("no-store")
        {
            return Err(DeletionFailure::Unavailable);
        }
        let response = response.into_inner();
        if response.command.as_ref() != Some(&command)
            || response.request_nonce != nonce
            || response.expires_unix_micros != expires
            || chrono::Utc::now().timestamp_micros() >= expires
            || response.source_binding.len() != 32
            || response.source_binding.iter().all(|b| *b == 0)
        {
            return Err(DeletionFailure::Unavailable);
        }
        let source = response
            .source_binding
            .try_into()
            .map_err(|_| DeletionFailure::Unavailable)?;
        VerifiedDeletionSource::from_verified_auth_source(c, original_deadline, source)
    }
}
