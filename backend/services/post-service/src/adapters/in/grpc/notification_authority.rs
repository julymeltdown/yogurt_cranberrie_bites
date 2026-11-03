//! Explicit owning notification RPC. Composition must supply authenticated
//! transport and this method policy; no production listener is selected here.
use super::{fanout_authority::frame, resource_authority::proto};
use crate::adapters::out::grpc::notification_client::proto::CreateNotificationRequest;
use platform::{
    actor_delegation::notification::{self, *},
    grpc_auth::{authorize_call, RpcAction, Workload},
    grpc_method_policy::MethodPolicy,
};
use post_application::{delivery_authority::DeliveryAuthorityErrorDto, notification_authority::*};
use prost::Message;
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub static NOTIFICATION_OWNER_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.v1.NotificationOwnerAuthorityService",
    workload: Some(Workload::Post),
    methods: &[("ResolveNotificationSource", &[Workload::Gateway])],
    actor_fields: &[],
};

#[derive(Clone)]
pub struct NotificationOwnerGrpcServer {
    repository: Arc<dyn NotificationNetworkAuthorityRepositoryPort>,
    signer: NotificationOwnerSigner,
}
impl NotificationOwnerGrpcServer {
    pub fn new(
        repository: Arc<dyn NotificationNetworkAuthorityRepositoryPort>,
        signer: NotificationOwnerSigner,
    ) -> Self {
        Self { repository, signer }
    }
}

pub fn page_message(
    page: &NotificationPage,
) -> Result<proto::ListNotificationFollowersRequest, platform::actor_delegation::Denial> {
    let c = &page.claim;
    Ok(proto::ListNotificationFollowersRequest {
        claim: Some(proto::NotificationClaim {
            event_id: c.event_id.to_string(),
            aggregate_id: c.aggregate_id.to_string(),
            event_digest: NotificationClaim::decode_digest(&c.event_digest)?.to_vec(),
            attempt_digest: NotificationClaim::decode_digest(&c.attempt_digest)?.to_vec(),
            lease_digest: NotificationClaim::decode_digest(&c.lease_digest)?.to_vec(),
            attempt_id: c.attempt_id.to_string(),
            attempt: c.attempt,
            lease_expires_micros: c.lease_expires_micros,
        }),
        author_id: page.author_id.to_string(),
        cursor: page.cursor.clone(),
        limit: page.limit,
    })
}

#[allow(clippy::result_large_err)]
fn lookup(
    binding: &NotificationBinding,
    body: &[u8],
) -> Result<NotificationNetworkReferenceDto, Status> {
    let invalid = || Status::invalid_argument("Invalid notification binding");
    if binding.frame().map_err(|_| invalid())? != body {
        return Err(invalid());
    }
    let scope = match binding {
        NotificationBinding::FollowerPage(p) => {
            // The hand-written protocol frame must equal generated Prost bytes.
            if frame(&page_message(p).map_err(|_| invalid())?) != body {
                return Err(invalid());
            }
            NotificationScopeDto::FollowerPage {
                author_id: p.author_id,
                cursor: p.cursor.clone(),
                limit: p.limit,
            }
        }
        NotificationBinding::Delivery(d) => {
            let message = CreateNotificationRequest::decode(&body[5..]).map_err(|_| invalid())?;
            if frame(&message) != body {
                return Err(invalid());
            }
            NotificationScopeDto::RecipientPolicy {
                recipient_id: d.recipient_id,
            }
        }
    };
    let c = binding.claim();
    let r = NotificationNetworkReferenceDto {
        event_id: c.event_id,
        aggregate_id: c.aggregate_id,
        event_digest: NotificationClaim::decode_digest(&c.event_digest).map_err(|_| invalid())?,
        attempt_digest: NotificationClaim::decode_digest(&c.attempt_digest)
            .map_err(|_| invalid())?,
        lease_digest: NotificationClaim::decode_digest(&c.lease_digest).map_err(|_| invalid())?,
        attempt_id: c.attempt_id,
        attempt_number: c.attempt,
        lease_expires_at: chrono::DateTime::from_timestamp_micros(c.lease_expires_micros)
            .ok_or_else(invalid)?,
        scope,
    };
    r.validate().map_err(|_| invalid())?;
    Ok(r)
}

#[tonic::async_trait]
impl proto::notification_owner_authority_service_server::NotificationOwnerAuthorityService
    for NotificationOwnerGrpcServer
{
    async fn resolve_notification_source(
        &self,
        request: Request<proto::ResolveNotificationSourceRequest>,
    ) -> Result<Response<proto::ResolveNotificationSourceResponse>, Status> {
        if authorize_call(&request, RpcAction::Read)? != Workload::Gateway {
            return Err(Status::permission_denied(
                "Independent Gateway lookup required",
            ));
        }
        let request = request.into_inner();
        let challenge = Uuid::parse_str(&request.challenge)
            .ok()
            .filter(|id| !id.is_nil())
            .filter(|id| id.to_string() == request.challenge)
            .ok_or_else(|| Status::invalid_argument("Invalid owner challenge"))?;
        let binding =
            NotificationBinding::decode(&request.binding).map_err(notification::status)?;
        let observed = self
            .repository
            .resolve_notification_source(&lookup(&binding, &request.framed_body)?)
            .await
            .map_err(|e| match e {
                DeliveryAuthorityErrorDto::Unavailable => {
                    Status::unavailable("Notification owner unavailable")
                }
                _ => Status::permission_denied("Notification owner denied"),
            })?;
        let now = chrono::Utc::now().timestamp_micros();
        let checked_at = observed.checked_at().timestamp_micros();
        // Retain the repository's validated reference even for suppression.
        // Lifecycle-before-payload parsing does not relax immutable identity.
        let r = observed.reference();
        let stored = observed.event();
        let owned_claim = NotificationClaim {
            event_id: r.event_id,
            aggregate_id: r.aggregate_id,
            event_digest: NotificationClaim::digest(
                &notification_event_identity(stored)
                    .map_err(|_| Status::permission_denied("Notification source denied"))?,
            ),
            attempt_digest: NotificationClaim::encode_digest(&r.attempt_digest),
            lease_digest: NotificationClaim::digest(r.lease_token.as_bytes()),
            attempt_id: r.attempt_id,
            attempt: r.attempt_number,
            lease_expires_micros: r.lease_expires_at.timestamp_micros(),
        };
        if binding.claim() != &owned_claim {
            return Err(Status::permission_denied("Notification source denied"));
        }
        let immutable_matches = match &binding {
            NotificationBinding::FollowerPage(p) => {
                stored.event_type == "POST_CREATED" && stored.actor_id == Some(p.author_id)
            }
            NotificationBinding::Delivery(d) => {
                let kind_matches = matches!(
                    (stored.event_type.as_str(), d.kind),
                    ("POST_CREATED", NotificationKind::Created)
                        | ("POST_LIKED", NotificationKind::Liked)
                        | ("POST_COMMENTED", NotificationKind::Commented)
                        | ("COMMENT", NotificationKind::LegacyComment)
                );
                kind_matches
                    && stored.actor_id == Some(d.actor_id)
                    && stored.occurred_at.to_rfc3339() == d.occurred_at
            }
        };
        if !immutable_matches {
            return Err(Status::permission_denied("Notification source denied"));
        }
        let evidence = if let Some(scope) = observed.scope() {
            // Persistence scope alone does not bind the destination write. Its
            // exact original intent must also equal the proposed request body.
            if let NotificationBinding::Delivery(d) = &binding {
                let n = scope
                    .intent()
                    .notification_for(d.recipient_id)
                    .map_err(|_| Status::permission_denied("Notification intent denied"))?;
                let actual = CreateNotificationRequest {
                    user_id: n.user_id.to_string(),
                    r#type: n.notification_type,
                    payload_json: n.payload_json,
                    event_id: n.event_id.to_string(),
                    actor_id: n.actor_id.map(|id| id.to_string()).unwrap_or_default(),
                    occurred_at: n.occurred_at,
                };
                if frame(&actual) != request.framed_body {
                    return Err(Status::permission_denied("Notification intent denied"));
                }
            }
            let visibility = scope.visibility().map(|v| match v {
                NotificationVisibilityDto::Public => SourceVisibility::Public,
                NotificationVisibilityDto::Friends => SourceVisibility::Friends,
            });
            self.signer.attest(
                &binding,
                &request.framed_body,
                challenge,
                visibility,
                checked_at,
                now,
            )
        } else {
            self.signer.attest_suppressed(
                &binding,
                &request.framed_body,
                challenge,
                checked_at,
                now,
            )
        }
        .map_err(notification::status)?;
        Ok(Response::new(proto::ResolveNotificationSourceResponse {
            evidence,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn method_policy_matches_proto_and_only_allows_gateway() {
        let section = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/authority/v1/authority.proto"
        ))
        .split("service NotificationOwnerAuthorityService")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
        assert_eq!(
            platform::grpc_method_policy::proto_rpc_names(section),
            vec!["ResolveNotificationSource"]
        );
        assert_eq!(
            NOTIFICATION_OWNER_POLICY.method_names().collect::<Vec<_>>(),
            vec!["ResolveNotificationSource"]
        );
        for caller in Workload::ALL {
            assert_eq!(
                NOTIFICATION_OWNER_POLICY.allows(
                    "/authority.v1.NotificationOwnerAuthorityService/ResolveNotificationSource",
                    caller
                ),
                caller == Workload::Gateway
            );
        }
    }
    #[test]
    fn generated_page_frame_matches_zero_count_and_exact_optional_cursor() {
        for cursor in [
            None,
            Some(format!("2026-10-14T00:00:00+00:00|{}", Uuid::new_v4())),
        ] {
            let p = NotificationPage {
                claim: NotificationClaim {
                    event_id: Uuid::new_v4(),
                    aggregate_id: Uuid::new_v4(),
                    event_digest: NotificationClaim::digest(b"source"),
                    attempt_digest: NotificationClaim::digest(b"REPLAY SKIPPED count0"),
                    lease_digest: NotificationClaim::digest(Uuid::new_v4().as_bytes()),
                    attempt_id: Uuid::new_v4(),
                    attempt: 0,
                    lease_expires_micros: 1_800_000_000_000_000,
                },
                author_id: Uuid::new_v4(),
                cursor,
                limit: 2,
            };
            let body = frame(&page_message(&p).unwrap());
            assert_eq!(p.frame().unwrap(), body);
            let binding = NotificationBinding::FollowerPage(p);
            assert!(lookup(&binding, &body).is_ok());
            let mut unknown = body;
            unknown.extend_from_slice(&[0x28, 1]);
            let len = (unknown.len() - 5) as u32;
            unknown[1..5].copy_from_slice(&len.to_be_bytes());
            assert!(lookup(&binding, &unknown).is_err());
        }
    }
}
