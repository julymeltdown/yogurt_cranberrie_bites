//! Concert's private mTLS client. Success requires an exact post-commit echo;
//! transport ambiguity is retryable with the original immutable outbox identity.
use crate::notification_live::proto;
use async_trait::async_trait;
use concert_application::notification::DurableNotificationOwner;
use concert_domain::notification::*;
use platform::grpc_auth::{ServiceIdentityClientInterceptor, Workload};
use std::{collections::BTreeSet, time::Duration};
use tonic::{service::interceptor::InterceptedService, transport::Channel, Request};

type Transport = InterceptedService<Channel, ServiceIdentityClientInterceptor>;
#[derive(Clone)]
pub struct NotificationDeliveryClient {
    client: proto::concert_delivery_service_client::ConcertDeliveryServiceClient<Transport>,
}
impl NotificationDeliveryClient {
    pub fn connect(
        endpoint: &str,
        token: &str,
        tls: &platform::grpc_tls::TlsMaterial,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            endpoint.starts_with("https://"),
            "Notification delivery requires HTTPS"
        );
        let channel = tls
            .endpoint(endpoint, Workload::Notification)?
            .connect_lazy();
        let identity = ServiceIdentityClientInterceptor::new(Workload::Concert, token)?;
        Ok(Self {
            client: proto::concert_delivery_service_client::ConcertDeliveryServiceClient::new(
                InterceptedService::new(channel, identity),
            )
            .max_encoding_message_size(512 * 1024)
            .max_decoding_message_size(1024),
        })
    }
}
#[async_trait]
impl DurableNotificationOwner for NotificationDeliveryClient {
    async fn deliver(&self, envelope: Envelope, recipients: Vec<Recipient>) -> Result<()> {
        envelope.validate()?;
        if recipients.len() > 1000 {
            return Err(NotificationError::InvalidInput);
        }
        let mut users = BTreeSet::new();
        for recipient in &recipients {
            recipient.validate()?;
            if !users.insert(recipient.user_id) {
                return Err(NotificationError::InvalidInput);
            }
        }
        let expected = proto::DeliveryEnvelope {
            outbox_id: envelope.outbox_id.to_string(),
            receipt_id: envelope.receipt_id.to_string(),
            event_id: envelope.event_id.to_string(),
            resource_id: envelope.resource_id.to_string(),
            resource_revision: envelope.resource_revision,
        };
        let recipients = recipients
            .into_iter()
            .map(|r| proto::DeliveryRecipient {
                user_id: r.user_id.to_string(),
                source_actor_id: r.source_actor_id.map(|id| id.to_string()),
                kind: match r.kind {
                    Kind::Party => 1,
                    Kind::Giveaway => 2,
                    Kind::Social => 3,
                    Kind::Moderation => 4,
                    Kind::Notice => 5,
                    Kind::Feed => 6,
                },
                target: r.target.map(|t| proto::Target {
                    target_id: t.target_id.to_string(),
                    target_type: match t.target_type {
                        TargetType::Party => 1,
                        TargetType::Application => 2,
                        TargetType::Giveaway => 3,
                        TargetType::Reservation => 4,
                        TargetType::SocialRequest => 5,
                        TargetType::Post => 6,
                        TargetType::Notice => 7,
                        TargetType::Report => 8,
                    },
                }),
            })
            .collect();
        let budget = platform::call_deadline::bounded(Duration::from_secs(3));
        if budget.is_zero() {
            return Err(NotificationError::Unavailable);
        }
        let stop = tokio::time::Instant::now() + budget;
        let mut request = Request::new(proto::DeliverRequest {
            envelope: Some(expected.clone()),
            recipients,
        });
        request.set_timeout(budget);
        let reply = tokio::time::timeout_at(stop, self.client.clone().deliver(request))
            .await
            .map_err(|_| NotificationError::Unavailable)?
            .map_err(|error| match error.code() {
                tonic::Code::InvalidArgument => NotificationError::InvalidInput,
                tonic::Code::AlreadyExists => NotificationError::IdempotencyConflict,
                _ => NotificationError::Unavailable,
            })?
            .into_inner();
        if reply.committed_envelope != Some(expected) {
            return Err(NotificationError::Unavailable);
        }
        Ok(())
    }
}
