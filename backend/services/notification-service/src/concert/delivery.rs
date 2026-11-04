//! Workload-authenticated durable system ingress. User reads use the separate
//! canonical inbox authority; background delivery never fabricates a user grant.
use super::{transport::proto, PgConcertInbox, UnavailableAuthority};
use concert_domain::notification::*;
use platform::{
    grpc_auth::{VerifiedCaller, Workload},
    grpc_method_policy::MethodPolicy,
};
use sqlx::PgPool;
use std::{collections::BTreeSet, time::Duration};
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub static DELIVERY_POLICY: MethodPolicy = MethodPolicy {
    service: "notification.concert.v1.ConcertDeliveryService",
    workload: Some(Workload::Notification),
    methods: &[("Deliver", &[Workload::Concert])],
    actor_fields: &[],
};
#[derive(Clone)]
pub struct NotificationDeliveryIngress {
    pool: PgPool,
}
impl NotificationDeliveryIngress {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
    pub fn server(
        self,
    ) -> proto::concert_delivery_service_server::ConcertDeliveryServiceServer<Self> {
        proto::concert_delivery_service_server::ConcertDeliveryServiceServer::new(self)
            .max_decoding_message_size(512 * 1024)
            .max_encoding_message_size(1024)
    }
}
#[tonic::async_trait]
impl proto::concert_delivery_service_server::ConcertDeliveryService
    for NotificationDeliveryIngress
{
    async fn deliver(
        &self,
        request: Request<proto::DeliverRequest>,
    ) -> std::result::Result<Response<proto::DeliverResponse>, Status> {
        if !matches!(
            request.extensions().get::<VerifiedCaller>(),
            Some(VerifiedCaller(Workload::Concert))
        ) || request.peer_certs().is_none_or(|certs| certs.is_empty())
        {
            return Err(Status::permission_denied(
                "Concert delivery workload required",
            ));
        }
        let original = platform::call_deadline::current()
            .ok_or_else(|| Status::invalid_argument("Original delivery deadline required"))?;
        let stop = tokio::time::Instant::from_std(original)
            .min(tokio::time::Instant::now() + Duration::from_secs(3));
        let request = request.into_inner();
        let wire = request.envelope.ok_or_else(invalid)?;
        let envelope = envelope(&wire)?;
        if request.recipients.len() > 1000 {
            return Err(invalid());
        }
        let mut users = BTreeSet::new();
        let recipients = request
            .recipients
            .into_iter()
            .map(|wire| {
                let recipient = recipient(wire)?;
                if !users.insert(recipient.user_id) {
                    return Err(invalid());
                }
                Ok(recipient)
            })
            .collect::<std::result::Result<Vec<_>, Status>>()?;
        let inbox = PgConcertInbox::new(self.pool.clone(), UnavailableAuthority);
        // Cancellation before commit rolls back; ambiguous lost acknowledgements
        // retry the same immutable envelope and cannot recreate/purge-resurrect it.
        tokio::time::timeout_at(stop, inbox.ingest(envelope, recipients))
            .await
            .map_err(|_| Status::deadline_exceeded("Concert delivery deadline"))?
            .map_err(status)?;
        let mut response = Response::new(proto::DeliverResponse {
            committed_envelope: Some(wire),
        });
        response
            .metadata_mut()
            .insert("cache-control", "no-store".parse().unwrap());
        Ok(response)
    }
}
fn id(value: &str) -> std::result::Result<Uuid, Status> {
    let id = Uuid::parse_str(value).map_err(|_| invalid())?;
    if id.is_nil() || id.to_string() != value {
        return Err(invalid());
    }
    Ok(id)
}
fn envelope(wire: &proto::DeliveryEnvelope) -> std::result::Result<Envelope, Status> {
    let value = Envelope {
        outbox_id: id(&wire.outbox_id)?,
        receipt_id: id(&wire.receipt_id)?,
        event_id: id(&wire.event_id)?,
        resource_id: id(&wire.resource_id)?,
        resource_revision: wire.resource_revision,
    };
    value.validate().map_err(status)?;
    Ok(value)
}
fn recipient(wire: proto::DeliveryRecipient) -> std::result::Result<Recipient, Status> {
    let kind = match wire.kind {
        1 => Kind::Party,
        2 => Kind::Giveaway,
        3 => Kind::Social,
        4 => Kind::Moderation,
        5 => Kind::Notice,
        6 => Kind::Feed,
        _ => return Err(invalid()),
    };
    let target = wire
        .target
        .map(|wire| {
            Ok(Target {
                target_type: match wire.target_type {
                    1 => TargetType::Party,
                    2 => TargetType::Application,
                    3 => TargetType::Giveaway,
                    4 => TargetType::Reservation,
                    5 => TargetType::SocialRequest,
                    6 => TargetType::Post,
                    7 => TargetType::Notice,
                    8 => TargetType::Report,
                    _ => return Err(invalid()),
                },
                target_id: id(&wire.target_id)?,
            })
        })
        .transpose()?;
    let value = Recipient {
        user_id: id(&wire.user_id)?,
        source_actor_id: wire.source_actor_id.as_deref().map(id).transpose()?,
        kind,
        target,
    };
    value.validate().map_err(status)?;
    Ok(value)
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid Concert delivery")
}
fn status(error: NotificationError) -> Status {
    match error {
        NotificationError::InvalidInput => invalid(),
        NotificationError::IdempotencyConflict => {
            Status::already_exists("Concert delivery identity conflict")
        }
        _ => Status::unavailable("Concert delivery unavailable"),
    }
}
