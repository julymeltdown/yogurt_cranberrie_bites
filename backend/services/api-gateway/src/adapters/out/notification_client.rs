use crate::adapters::out::grpc_call::{delegated, BoundedGrpcCallExt};
use crate::application::dto::notification_dto::{
    NotificationFeedResponse, NotificationMarkAllReadResponse, NotificationResponse,
};
use crate::application::ports::NotificationClientPort;
use crate::config::GatewayMetrics;
use async_trait::async_trait;
use platform::actor_delegation::{ActorDelegationClient, ActorDelegationSigner};
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use platform::grpc_client_trace::GrpcClientTraceService;
use tonic::service::interceptor::InterceptedService;
use tonic::transport::Channel;

pub mod proto {
    tonic::include_proto!("notification.v1");
}

#[derive(Clone)]
pub struct GrpcNotificationClient {
    client: proto::notification_service_client::NotificationServiceClient<
        InterceptedService<
            GrpcClientTraceService<ActorDelegationClient<Channel>>,
            ServiceIdentityClientInterceptor,
        >,
    >,
    metrics: GatewayMetrics,
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
    ) -> anyhow::Result<Self> {
        Self::connect_with_metrics(
            endpoint,
            caller,
            service_token,
            tls_material,
            ActorDelegationSigner::local_parity(caller),
            GatewayMetrics::default(),
        )
        .await
    }

    pub async fn connect_with_metrics(
        endpoint: String,
        caller: Workload,
        service_token: &str,
        tls_material: Option<&platform::grpc_tls::TlsMaterial>,
        signer: ActorDelegationSigner,
        metrics: GatewayMetrics,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls_material,
            &endpoint,
            Workload::Notification,
            platform::edge::Edge::GatewayNotification.budget(),
        )?;
        Ok(Self {
            client: proto::notification_service_client::NotificationServiceClient::with_interceptor(
                GrpcClientTraceService::new(ActorDelegationClient::new(
                    channel,
                    signer,
                    Workload::Notification,
                )),
                ServiceIdentityClientInterceptor::new(caller, service_token)?,
            ),
            metrics,
        })
    }
}

impl GrpcNotificationClient {
    async fn list_notifications_notification_client_port_inner(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> anyhow::Result<NotificationFeedResponse> {
        let cursor = cursor.unwrap_or_default();
        let response = crate::adapters::out::grpc_call::replay_safe_read(
            platform::edge::Edge::GatewayNotification,
            "/notification.v1.NotificationService/ListNotifications",
            &self.metrics,
            || {
                delegated(
                    proto::ListNotificationsRequest {
                        user_id: user_id.clone(),
                        cursor: cursor.clone(),
                        limit,
                    },
                    &user_id,
                )
            },
            |request| {
                let mut client = self.client.clone();
                async move { client.list_notifications(request).await }
            },
        )
        .await?
        .into_inner();
        Ok(NotificationFeedResponse {
            items: response.items.into_iter().map(map_notification).collect(),
            next_cursor: empty_to_none(response.next_cursor),
        })
    }

    async fn mark_read_notification_client_port_inner(
        &self,
        user_id: String,
        notification_id: String,
    ) -> anyhow::Result<NotificationResponse> {
        let mut client = self.client.clone();
        let actor = user_id.clone();
        let response = client
            .mark_notification_read(delegated(
                proto::MarkNotificationReadRequest {
                    user_id,
                    notification_id,
                },
                &actor,
            )?)
            .with_edge_deadline(
                platform::edge::Edge::GatewayNotification,
                self.metrics.clone(),
            )
            .await?
            .into_inner();
        Ok(map_notification(
            response.notification.expect("notification"),
        ))
    }

    async fn mark_all_read_notification_client_port_inner(
        &self,
        user_id: String,
    ) -> anyhow::Result<NotificationMarkAllReadResponse> {
        let mut client = self.client.clone();
        let response = client
            .mark_all_notifications_read(delegated(
                proto::MarkAllNotificationsReadRequest {
                    user_id: user_id.clone(),
                },
                &user_id,
            )?)
            .with_edge_deadline(
                platform::edge::Edge::GatewayNotification,
                self.metrics.clone(),
            )
            .await?
            .into_inner();
        Ok(NotificationMarkAllReadResponse {
            updated_count: response.updated_count,
        })
    }
}

#[async_trait]
impl NotificationClientPort for GrpcNotificationClient {
    async fn list_notifications(
        &self,
        user_id: String,
        cursor: Option<String>,
        limit: i32,
    ) -> crate::application::client_error::GatewayClientResult<NotificationFeedResponse> {
        self.list_notifications_notification_client_port_inner(user_id, cursor, limit)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn mark_read(
        &self,
        user_id: String,
        notification_id: String,
    ) -> crate::application::client_error::GatewayClientResult<NotificationResponse> {
        self.mark_read_notification_client_port_inner(user_id, notification_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
    async fn mark_all_read(
        &self,
        user_id: String,
    ) -> crate::application::client_error::GatewayClientResult<NotificationMarkAllReadResponse>
    {
        self.mark_all_read_notification_client_port_inner(user_id)
            .await
            .map_err(crate::adapters::out::client_error::classify_client_error)
    }
}

fn map_notification(value: proto::Notification) -> NotificationResponse {
    NotificationResponse {
        id: value.id,
        user_id: value.user_id,
        notification_type: value.r#type,
        payload_json: value.payload_json,
        created_at: value.created_at,
        read_at: value.read_at,
    }
}

fn empty_to_none(value: String) -> Option<String> {
    if value.trim().is_empty() {
        None
    } else {
        Some(value)
    }
}
