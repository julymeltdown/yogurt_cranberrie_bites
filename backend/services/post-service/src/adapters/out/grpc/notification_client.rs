use crate::application::delivery_effect::DeliveryEffectFailure;
use crate::application::ports::NotificationClientPort;
use crate::domain::post::NotificationRequest;
use anyhow::Result;
use async_trait::async_trait;
use platform::edge::Edge;
use platform::edge_metrics::EdgeDeadlineMetrics;
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use platform::grpc_client_trace::GrpcClientTraceService;
use platform::grpc_deadline::EdgeDeadlineCallExt;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;

pub mod proto {
    tonic::include_proto!("notification.v1");
}

#[derive(Clone)]
pub struct GrpcNotificationClient {
    client: proto::notification_service_client::NotificationServiceClient<
        InterceptedService<GrpcClientTraceService<Channel>, ServiceIdentityClientInterceptor>,
    >,
    edge_deadlines: EdgeDeadlineMetrics,
}

impl GrpcNotificationClient {
    /// Build the client without waiting for the peer to answer.
    ///
    /// The channel connects on first use, so a dependency that is down
    /// does not stop this service from starting. Configuration is still
    /// validated here: an invalid endpoint or TLS material fails now.
    pub async fn connect(
        endpoint: String,
        caller: Workload,
        service_token: &str,
        tls_material: Option<&platform::grpc_tls::TlsMaterial>,
    ) -> Result<Self> {
        Self::connect_with_metrics(
            endpoint,
            caller,
            service_token,
            tls_material,
            EdgeDeadlineMetrics::default(),
        )
        .await
    }

    pub async fn connect_with_metrics(
        endpoint: String,
        caller: Workload,
        service_token: &str,
        tls_material: Option<&platform::grpc_tls::TlsMaterial>,
        edge_deadlines: EdgeDeadlineMetrics,
    ) -> Result<Self> {
        // The peer is the notification service; `caller` is this workload's identity.
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls_material,
            &endpoint,
            Workload::Notification,
            platform::edge::Edge::PostNotification.budget(),
        )?;
        Ok(Self {
            client: proto::notification_service_client::NotificationServiceClient::with_interceptor(
                GrpcClientTraceService::new(channel),
                ServiceIdentityClientInterceptor::new(caller, service_token)?,
            ),
            edge_deadlines,
        })
    }
}

#[async_trait]
impl NotificationClientPort for GrpcNotificationClient {
    async fn create_notification(&self, request: NotificationRequest) -> Result<()> {
        let mut client = self.client.clone();
        client
            .create_notification(proto::CreateNotificationRequest {
                user_id: request.user_id.to_string(),
                r#type: request.notification_type,
                payload_json: request.payload_json,
                event_id: request.event_id.to_string(),
                actor_id: request
                    .actor_id
                    .map(|value| value.to_string())
                    .unwrap_or_default(),
                occurred_at: request.occurred_at,
            })
            .with_edge_deadline(Edge::PostNotification, self.edge_deadlines.clone())
            .await
            .map_err(|error| DeliveryEffectFailure::uncertain(error.into()))?;
        Ok(())
    }
}
