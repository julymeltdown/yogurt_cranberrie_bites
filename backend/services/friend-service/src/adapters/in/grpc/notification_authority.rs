//! Actual owning notification policy and admitted selected-page projection.
use super::fanout_projection::proto;
use crate::application::{
    ports::{FollowRepositoryPort, RelationshipPolicyRepositoryPort},
    use_cases::{
        get_relationship_policy::GetRelationshipPolicyUseCase, list_followers::ListFollowersUseCase,
    },
};
use crate::domain::follow_edge::FollowCursor;
use platform::{
    actor_delegation::notification::{self, *},
    grpc_auth::{authorize_call, RpcAction, Workload},
    grpc_method_policy::MethodPolicy,
};
use prost::Message;
use std::{collections::HashSet, sync::Arc};
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub static NOTIFICATION_POLICY_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.v1.NotificationPolicyService",
    workload: Some(Workload::Friend),
    methods: &[("CheckNotificationRecipient", &[Workload::Gateway])],
    actor_fields: &[],
};
pub static NOTIFICATION_PROJECTION_POLICY: MethodPolicy = MethodPolicy {
    service: "authority.v1.NotificationProjectionService",
    workload: Some(Workload::Friend),
    methods: &[("ListNotificationFollowers", &[Workload::Post])],
    actor_fields: &[],
};
#[derive(Clone)]
pub struct NotificationPolicyGrpcServer {
    query: GetRelationshipPolicyUseCase<Arc<dyn RelationshipPolicyRepositoryPort>>,
    signer: NotificationFriendSigner,
}
impl NotificationPolicyGrpcServer {
    pub fn new(
        repository: Arc<dyn RelationshipPolicyRepositoryPort>,
        signer: NotificationFriendSigner,
    ) -> Self {
        Self {
            query: GetRelationshipPolicyUseCase::new(repository),
            signer,
        }
    }
}
#[tonic::async_trait]
impl proto::notification_policy_service_server::NotificationPolicyService
    for NotificationPolicyGrpcServer
{
    async fn check_notification_recipient(
        &self,
        request: Request<proto::CheckNotificationRecipientRequest>,
    ) -> Result<Response<proto::CheckNotificationRecipientResponse>, Status> {
        if authorize_call(&request, RpcAction::Read)? != Workload::Gateway {
            return Err(Status::permission_denied("Gateway policy query required"));
        }
        let r = request.into_inner();
        if r.query.len() > MAX_BYTES {
            return Err(Status::resource_exhausted("Notification query too large"));
        }
        let checked_at = now();
        let query = self
            .signer
            .verify_policy_query(&r.query, Workload::Gateway, POLICY_METHOD, checked_at)
            .map_err(notification::status)?;
        let policy = self
            .query
            .execute(query.recipient_id(), query.actor_id())
            .await
            .map_err(|_| Status::unavailable("Notification policy unavailable"))?;
        let evidence = self
            .signer
            .policy(
                query,
                policy.blocked_by_viewer || policy.blocked_viewer,
                policy.muted_by_viewer,
                policy.following,
                checked_at,
                now(),
            )
            .map_err(notification::status)?;
        Ok(Response::new(proto::CheckNotificationRecipientResponse {
            evidence,
        }))
    }
}

#[derive(Clone)]
pub struct NotificationProjectionGrpcServer {
    query: ListFollowersUseCase<Arc<dyn FollowRepositoryPort>>,
    signer: NotificationFriendSigner,
}
impl NotificationProjectionGrpcServer {
    pub fn new(
        repository: Arc<dyn FollowRepositoryPort>,
        signer: NotificationFriendSigner,
    ) -> Self {
        Self {
            query: ListFollowersUseCase::new(repository),
            signer,
        }
    }
}
pub fn claimed_notification_projection_service(
    repository: Arc<dyn FollowRepositoryPort>,
    signer: NotificationFriendSigner,
    verifier: NotificationVerifier,
    admission: Arc<dyn NotificationAdmissionCheck>,
) -> anyhow::Result<
    NotificationGuard<
        proto::notification_projection_service_server::NotificationProjectionServiceServer<
            NotificationProjectionGrpcServer,
        >,
    >,
> {
    NotificationGuard::new(
        proto::notification_projection_service_server::NotificationProjectionServiceServer::new(
            NotificationProjectionGrpcServer::new(repository, signer),
        ),
        verifier,
        admission,
    )
}
fn now() -> i64 {
    chrono::Utc::now().timestamp_micros()
}
fn frame(message: &impl Message) -> Vec<u8> {
    let bytes = message.encode_to_vec();
    [vec![0], (bytes.len() as u32).to_be_bytes().to_vec(), bytes].concat()
}
#[tonic::async_trait]
impl proto::notification_projection_service_server::NotificationProjectionService
    for NotificationProjectionGrpcServer
{
    async fn list_notification_followers(
        &self,
        request: Request<proto::ListNotificationFollowersRequest>,
    ) -> Result<Response<proto::ListNotificationFollowersResponse>, Status> {
        if authorize_call(&request, RpcAction::Read)? != Workload::Post {
            return Err(Status::permission_denied("Post presenter required"));
        }
        let admitted = request
            .extensions()
            .get::<AdmittedNotification>()
            .ok_or_else(|| {
                Status::permission_denied("Fresh notification page admission required")
            })?;
        let NotificationBinding::FollowerPage(scope) = admitted.binding() else {
            return Err(Status::permission_denied("Notification page required"));
        };
        if now() >= admitted.expires_micros()
            || admitted.binding().frame().map_err(notification::status)? != frame(request.get_ref())
        {
            return Err(Status::permission_denied(
                "Notification page mismatch or expiry",
            ));
        }
        use proto::list_notification_followers_response::Result as Wire;
        if admitted.decision() == NotificationDecision::Suppressed {
            return Ok(Response::new(proto::ListNotificationFollowersResponse {
                result: Some(Wire::Suppressed(proto::NotificationSuppressed {
                    expires_micros: admitted.expires_micros(),
                })),
            }));
        }
        let cursor = scope
            .cursor
            .as_deref()
            .map(FollowCursor::parse)
            .transpose()
            .map_err(|_| Status::invalid_argument("Invalid notification cursor"))?
            .flatten();
        let checked_at = now();
        let page = self
            .query
            .execute(scope.author_id, cursor, scope.limit)
            .await
            .map_err(|_| Status::unavailable("Notification projection unavailable"))?;
        let invalid = || Status::unavailable("Invalid notification projection");
        let mut seen = HashSet::new();
        let mut ids = Vec::new();
        let mut last = None;
        if page.edges.len() > scope.limit as usize {
            return Err(invalid());
        }
        for edge in page.edges {
            let follower = Uuid::parse_str(&edge.follower_id).map_err(|_| invalid())?;
            let followee = Uuid::parse_str(&edge.followee_id).map_err(|_| invalid())?;
            let created = chrono::DateTime::parse_from_rfc3339(&edge.created_at)
                .map_err(|_| invalid())?
                .with_timezone(&chrono::Utc);
            let boundary = (created, follower);
            if follower.is_nil()
                || follower == scope.author_id
                || followee != scope.author_id
                || !seen.insert(follower)
                || last.is_some_and(|previous| boundary >= previous)
                || cursor.is_some_and(|previous| {
                    boundary >= (previous.created_at, previous.tie_breaker_id)
                })
            {
                return Err(invalid());
            }
            last = Some(boundary);
            ids.push(follower);
        }
        if let Some(next) = &page.next_cursor {
            let parsed = FollowCursor::parse(next)
                .map_err(|_| invalid())?
                .ok_or_else(invalid)?;
            if Some((parsed.created_at, parsed.tie_breaker_id)) != last {
                return Err(invalid());
            }
        }
        let membership = self
            .signer
            .membership_observed(
                admitted,
                ids.clone(),
                page.next_cursor.clone(),
                checked_at,
                now(),
            )
            .map_err(notification::status)?;
        Ok(Response::new(proto::ListNotificationFollowersResponse {
            result: Some(Wire::Page(proto::NotificationFollowerPage {
                author_id: scope.author_id.to_string(),
                follower_ids: ids.into_iter().map(|id| id.to_string()).collect(),
                next_cursor: page.next_cursor,
                membership: membership.token,
                expires_micros: membership.expires_micros,
            })),
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn notification_owning_method_policies_match_schema_and_exact_callers() {
        let schema = include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../proto/authority/v1/authority.proto"
        ));
        for (policy, service, caller) in [
            (
                &NOTIFICATION_POLICY_POLICY,
                "NotificationPolicyService",
                Workload::Gateway,
            ),
            (
                &NOTIFICATION_PROJECTION_POLICY,
                "NotificationProjectionService",
                Workload::Post,
            ),
        ] {
            let section = schema
                .split(&format!("service {service}"))
                .nth(1)
                .unwrap()
                .split('}')
                .next()
                .unwrap();
            assert_eq!(
                platform::grpc_method_policy::proto_rpc_names(section),
                policy.method_names().collect::<Vec<_>>()
            );
            for method in policy.method_names() {
                for candidate in Workload::ALL {
                    assert_eq!(
                        policy.allows(&format!("/authority.v1.{service}/{method}"), candidate),
                        candidate == caller
                    );
                }
            }
        }
    }
}
