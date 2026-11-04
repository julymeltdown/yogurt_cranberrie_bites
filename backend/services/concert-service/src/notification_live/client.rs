//! Real Notification child transport. Root persists the private intent before
//! Execute and retains the child through its existing before-byte/finish gates.
use super::proto;
use crate::{
    adapters::out::relationship_authority::guarded::{
        self, ChildQuery, ChildRecoveryReference, HeldChildGuard, OwnerGuardScope,
    },
    product_rpc::child_settlement::NotificationInboxIntentSettler,
};
use async_trait::async_trait;
use platform::{
    actor_delegation::authority_deadline::AuthorityRpcDeadline,
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use std::{sync::Arc, time::Duration};
use tonic::{service::interceptor::InterceptedService, transport::Channel, Request, Status};
use uuid::Uuid;
const BUDGET: Duration = Duration::from_secs(5);
type Transport = InterceptedService<Channel, ServiceIdentityClientInterceptor>;
/// Root must load the exact original private binding from its durable scope
/// ledger; rebuilding HTTP/body/operation fields from a ChildQuery is forbidden.
#[async_trait]
pub trait NotificationRecoveryBinding: Send + Sync {
    async fn load_binding(
        &self,
        query: &ChildQuery,
    ) -> Result<guarded::proto::GuardBinding, Status>;
}
#[derive(Clone)]
pub struct NotificationClient {
    client: proto::concert_inbox_service_client::ConcertInboxServiceClient<Transport>,
    recovery: Option<Arc<dyn NotificationRecoveryBinding>>,
}
impl NotificationClient {
    pub fn connect(
        endpoint: &str,
        token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        recovery: Option<Arc<dyn NotificationRecoveryBinding>>,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Notification,
            platform::edge::EdgeBudget {
                edge: "concert->notification-inbox",
                expected_latency: Duration::from_millis(100),
                deadline: BUDGET,
                connect_deadline: Duration::from_millis(500),
                retry: platform::edge::RetryClass::Never,
                failure_policy: platform::edge::FailurePolicy::FailRequest,
            },
        )?;
        let identity = ServiceIdentityClientInterceptor::new(Workload::Concert, token)?;
        Ok(Self {
            client: proto::concert_inbox_service_client::ConcertInboxServiceClient::new(
                InterceptedService::new(channel, identity),
            )
            .max_decoding_message_size(1024 * 1024)
            .max_encoding_message_size(65536),
            recovery,
        })
    }
    pub async fn execute(
        &self,
        scope: Arc<OwnerGuardScope>,
        operation: proto::InboxOperation,
        input: proto::execute_request::Input,
    ) -> Result<proto::ExecuteResponse, Status> {
        scope.binding().check()?;
        let binding = scope.binding();
        let query = ChildQuery {
            actor: binding.actor(),
            event: binding.event(),
            subject: binding.actor(),
            family: binding.session_family(),
            request: binding.request_id(),
            expires_unix_micros: binding.deadline().expires_micros(),
            binding_digest: binding.digest(),
        };
        let id = Uuid::new_v4();
        let mut secret = Vec::with_capacity(32);
        secret.extend_from_slice(Uuid::new_v4().as_bytes());
        secret.extend_from_slice(Uuid::new_v4().as_bytes());
        let hex = secret.iter().map(|b| format!("{b:02x}")).collect();
        let reference = ChildRecoveryReference::NotificationInbox {
            query: query.clone(),
            child_id: id.to_string(),
            completion_secret: hex,
        };
        reference.validate_for(binding)?;
        scope.register_child_intent(&reference).await?;
        let parent = proto::ParentScope {
            binding: Some(scope.wire_binding()),
            auth_view: Some(scope.auth_view()),
            friend_view: Some(scope.friend_view()),
            root_view: Some(scope.root_read_view()),
        };
        let intent = proto::InboxIntent {
            child_id: id.to_string(),
            completion_secret: secret,
        };
        let child = Arc::new(NotificationChild {
            client: self.clone(),
            parent: parent.clone(),
            intent: intent.clone(),
            reference,
            deadline: binding.deadline(),
        });
        scope.retain_child(child).await?;
        let mut client = self.client.clone();
        let reply = bounded(
            binding.deadline(),
            client.execute(rpc(
                binding.deadline(),
                proto::ExecuteRequest {
                    scope: Some(parent),
                    intent: Some(intent),
                    operation: operation as i32,
                    input: Some(input),
                },
            )?),
        )
        .await?
        .into_inner();
        validate_reply(&reply, &query, id, operation)?;
        scope.limit_disclosure_until(
            chrono::DateTime::from_timestamp_micros(reply.expires_unix_micros)
                .ok_or_else(denied)?,
        )?;
        scope.binding().check()?;
        Ok(reply)
    }
    async fn complete(
        &self,
        parent: proto::ParentScope,
        intent: proto::InboxIntent,
    ) -> Result<(), Status> {
        // Settlement gets a bounded recovery budget, never a new disclosure
        // deadline and never an owner/root acquisition or completion capability.
        let deadline = recovery_deadline()?;
        let mut client = self.client.clone();
        let reply = bounded(
            deadline,
            client.complete(rpc(
                deadline,
                proto::GuardRequest {
                    scope: Some(parent),
                    intent: Some(intent),
                },
            )?),
        )
        .await?
        .into_inner();
        if reply.held || !reply.settled {
            return Err(Status::unavailable(
                "Notification child settlement unresolved",
            ));
        }
        Ok(())
    }
}
struct NotificationChild {
    client: NotificationClient,
    parent: proto::ParentScope,
    intent: proto::InboxIntent,
    reference: ChildRecoveryReference,
    deadline: AuthorityRpcDeadline,
}
#[async_trait]
impl HeldChildGuard for NotificationChild {
    async fn check(&self) -> Result<(), Status> {
        self.reference.check_content_time()?;
        let mut client = self.client.client.clone();
        let reply = bounded(
            self.deadline,
            client.check(rpc(
                self.deadline,
                proto::GuardRequest {
                    scope: Some(self.parent.clone()),
                    intent: Some(self.intent.clone()),
                },
            )?),
        )
        .await?
        .into_inner();
        if !reply.held || reply.settled {
            return Err(Status::permission_denied("Notification child not held"));
        }
        self.reference.check_content_time()
    }
    async fn complete_settled(&self) -> Result<(), Status> {
        self.client
            .complete(self.parent.clone(), self.intent.clone())
            .await
    }
    fn recovery_reference(&self) -> Result<ChildRecoveryReference, Status> {
        Ok(self.reference.clone())
    }
}
#[async_trait]
impl NotificationInboxIntentSettler for NotificationClient {
    async fn recover_notification_inbox(
        &self,
        reference: &ChildRecoveryReference,
    ) -> Result<(), Status> {
        reference.intent_id()?;
        let ChildRecoveryReference::NotificationInbox {
            query,
            child_id,
            completion_secret,
        } = reference
        else {
            return Err(denied());
        };
        let provider = self.recovery.as_ref().ok_or_else(|| {
            Status::failed_precondition("Notification durable root binding provider unavailable")
        })?;
        let binding = provider.load_binding(query).await?;
        if binding.actor_id != query.actor.to_string()
            || binding.event_id != query.event.to_string()
            || binding.session_family_id != query.family.to_string()
            || binding.request_id != query.request.to_string()
            || binding.expires_unix_micros != query.expires_unix_micros
            || guarded::wire_binding_digest(&binding)? != query.binding_digest
        {
            return Err(denied());
        }
        let secret = completion_secret
            .as_bytes()
            .chunks_exact(2)
            .map(|v| {
                let value = std::str::from_utf8(v).map_err(|_| denied())?;
                u8::from_str_radix(value, 16).map_err(|_| denied())
            })
            .collect::<Result<Vec<_>, Status>>()?;
        self.complete(
            proto::ParentScope {
                binding: Some(binding),
                auth_view: None,
                friend_view: None,
                root_view: None,
            },
            proto::InboxIntent {
                child_id: child_id.clone(),
                completion_secret: secret,
            },
        )
        .await
    }
}
fn validate_reply(
    reply: &proto::ExecuteResponse,
    query: &ChildQuery,
    id: Uuid,
    operation: proto::InboxOperation,
) -> Result<(), Status> {
    use proto::execute_response::Output;
    if reply.child_id != id.to_string()
        || reply.binding_digest != query.binding_digest
        || reply.expires_unix_micros > query.expires_unix_micros
        || reply.expires_unix_micros <= chrono::Utc::now().timestamp_micros()
    {
        return Err(denied());
    }
    match (operation, reply.output.as_ref()) {
        (proto::InboxOperation::List, Some(Output::List(v))) if v.items.len() <= 100 => {
            let mut ids = std::collections::BTreeSet::new();
            for n in &v.items {
                valid_notification(n, query)?;
                if !ids.insert(&n.notification_id) {
                    return Err(denied());
                }
            }
            if let Some(cursor) = &v.next_cursor {
                canonical(cursor)?;
            }
        }
        (proto::InboxOperation::UnreadCount, Some(Output::Count(v)))
            if v.actor_id == query.actor.to_string()
                && v.event_id == query.event.to_string()
                && (0..=concert_domain::MAX_REVISION).contains(&v.unread_total)
                && v.as_of_unix_micros > 0
                && v.as_of_unix_micros <= reply.expires_unix_micros => {}
        (proto::InboxOperation::MarkRead, Some(Output::MarkRead(v))) => {
            valid_notification(v, query)?
        }
        (proto::InboxOperation::MarkAllRead, Some(Output::MarkAll(v)))
            if v.read_count >= 0
                && v.read_at_unix_micros > 0
                && v.read_at_unix_micros <= reply.expires_unix_micros => {}
        _ => return Err(denied()),
    }
    Ok(())
}
fn valid_notification(n: &proto::Notification, query: &ChildQuery) -> Result<(), Status> {
    canonical(&n.notification_id)?;
    if n.event_id != query.event.to_string()
        || !(1..=6).contains(&n.kind)
        || n.created_at_unix_micros <= 0
        || n.created_at_unix_micros > query.expires_unix_micros
        || n.revision
            != if n.read_at_unix_micros.is_some() {
                2
            } else {
                1
            }
        || n.read_at_unix_micros
            .is_some_and(|t| t < n.created_at_unix_micros || t > query.expires_unix_micros)
    {
        return Err(denied());
    }
    if let Some(target) = &n.target {
        if !(1..=8).contains(&target.target_type) {
            return Err(denied());
        }
        canonical(&target.target_id)?;
    }
    Ok(())
}
fn canonical(v: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(v)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == v)
        .ok_or_else(denied)
}
fn recovery_deadline() -> Result<AuthorityRpcDeadline, Status> {
    let expiry = chrono::Utc::now().timestamp_micros() + 5_000_000;
    match platform::call_deadline::current() {
        Some(stop) => AuthorityRpcDeadline::within_original(expiry, stop, BUDGET),
        None => AuthorityRpcDeadline::new(expiry, BUDGET),
    }
}
fn rpc<T>(deadline: AuthorityRpcDeadline, value: T) -> Result<Request<T>, Status> {
    let mut request = Request::new(value);
    request.set_timeout(deadline.remaining(BUDGET, deadline.expires_micros())?);
    Ok(request)
}
async fn bounded<T>(
    deadline: AuthorityRpcDeadline,
    work: impl std::future::Future<Output = Result<T, Status>>,
) -> Result<T, Status> {
    deadline.remaining(BUDGET, deadline.expires_micros())?;
    let result = tokio::time::timeout_at(deadline.stop(), work)
        .await
        .map_err(|_| Status::deadline_exceeded("Notification child deadline"))??;
    deadline.remaining(BUDGET, deadline.expires_micros())?;
    Ok(result)
}
fn denied() -> Status {
    Status::permission_denied("Notification child binding denied")
}
