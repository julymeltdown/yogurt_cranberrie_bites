//! Actual owner capability inspection. A transport actor/read observation cannot
//! construct CheckedScope and a child never receives a completion secret.
use super::{
    identity::TransactionIdentity,
    wire::{lease, proto},
};
use platform::{
    actor_delegation::authority_deadline::AuthorityRpcDeadline,
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use sha2::{Digest, Sha256};
use std::time::Duration;
use tonic::{service::interceptor::InterceptedService, transport::Channel, Request, Status};
use uuid::Uuid;

const BUDGET: Duration = Duration::from_secs(5);
type Transport = InterceptedService<Channel, ServiceIdentityClientInterceptor>;
#[derive(Clone)]
pub(super) struct Owners {
    auth: lease::auth_owner_authority_service_client::AuthOwnerAuthorityServiceClient<Transport>,
    friend:
        lease::friend_owner_authority_service_client::FriendOwnerAuthorityServiceClient<Transport>,
}
pub(super) struct CheckedScope {
    wire: proto::SelfScope,
    digest: [u8; 32],
    actor: Uuid,
    event: Uuid,
    family: Uuid,
    request: Uuid,
    deadline: AuthorityRpcDeadline,
}
impl std::fmt::Debug for CheckedScope {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("CheckedScope([REDACTED])")
    }
}
impl Owners {
    pub fn connect(
        auth: &str,
        friend: &str,
        token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
    ) -> anyhow::Result<Self> {
        let channel = |endpoint, peer, edge| {
            platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
                tls,
                endpoint,
                peer,
                edge_budget(edge),
            )
        };
        let identity = || ServiceIdentityClientInterceptor::new(Workload::Profile, token);
        Ok(Self {
            auth: lease::auth_owner_authority_service_client::AuthOwnerAuthorityServiceClient::new(
                InterceptedService::new(
                    channel(auth, Workload::Auth, "profile-self->auth")?,
                    identity()?,
                ),
            ),
            friend:
                lease::friend_owner_authority_service_client::FriendOwnerAuthorityServiceClient::new(
                    InterceptedService::new(
                        channel(friend, Workload::Friend, "profile-self->friend")?,
                        identity()?,
                    ),
                ),
        })
    }
    pub async fn inspect(
        &self,
        wire: proto::SelfScope,
        subject: &str,
        verified: AuthorityRpcDeadline,
    ) -> Result<CheckedScope, Status> {
        let binding = wire.binding.as_ref().ok_or_else(denied)?;
        let actor = canonical(&binding.actor_id)?;
        let event = canonical(&binding.event_id)?;
        let family = canonical(&binding.session_family_id)?;
        let request = canonical(&binding.request_id)?;
        if canonical(subject)? != actor {
            return Err(denied());
        }
        let original = platform::call_deadline::current()
            .ok_or_else(|| Status::invalid_argument("Original self Profile deadline required"))?;
        let mut expires = binding.expires_unix_micros.min(verified.expires_micros());
        let digest = binding_digest(binding)?;
        for view in [&wire.auth_view, &wire.friend_view] {
            let view = view.as_ref().ok_or_else(denied)?;
            canonical(&view.guard_id)?;
            if view.binding_digest != digest
                || view.view_secret.len() != 32
                || view.expires_unix_micros > binding.expires_unix_micros
                || view.expires_unix_micros <= chrono::Utc::now().timestamp_micros()
            {
                return Err(denied());
            }
            expires = expires.min(view.expires_unix_micros);
        }
        let deadline = AuthorityRpcDeadline::within_original(
            expires,
            original.min(verified.stop().into_std()),
            BUDGET,
        )?;
        let checked = CheckedScope {
            wire,
            digest,
            actor,
            event,
            family,
            request,
            deadline,
        };
        self.recheck(&checked).await?;
        Ok(checked)
    }
    pub async fn recheck(&self, scope: &CheckedScope) -> Result<(), Status> {
        scope.check()?;
        let binding = scope.binding().clone();
        let mut auth = self.auth.clone();
        let a = bounded(
            scope.deadline,
            auth.check(rpc(
                scope.deadline,
                lease::CheckRequest {
                    view: scope.wire.auth_view.clone(),
                    binding: Some(binding.clone()),
                },
            )?),
        )
        .await?
        .into_inner();
        valid_revision(a.revision.as_ref())?;
        let auth_subject = self_subject(&a.subjects, scope.actor)?;
        if !a.session_live || !auth_subject.active || auth_subject.security_revision <= 0 {
            return Err(denied());
        }
        let mut friend = self.friend.clone();
        let f = bounded(
            scope.deadline,
            friend.check(rpc(
                scope.deadline,
                lease::CheckRequest {
                    view: scope.wire.friend_view.clone(),
                    binding: Some(binding),
                },
            )?),
        )
        .await?
        .into_inner();
        valid_revision(f.revision.as_ref())?;
        let friend_subject = self_subject(&f.subjects, scope.actor)?;
        if !friend_subject.active
            || friend_subject.security_revision != 0
            || friend_subject.actor_blocks_subject
            || friend_subject.subject_blocks_actor
        {
            return Err(denied());
        }
        scope.check()
    }
    pub async fn track(
        &self,
        scope: &CheckedScope,
        identity: &TransactionIdentity,
        operation: Uuid,
    ) -> Result<lease::TransactionIdentity, Status> {
        self.recheck(scope).await?;
        identity.validate().map_err(|_| denied())?;
        if operation.is_nil() {
            return Err(denied());
        }
        let native = wire_identity(identity, operation);
        let request = |view| lease::TrackRequest {
            view,
            binding: Some(scope.binding().clone()),
            transaction: Some(native.clone()),
        };
        let mut auth = self.auth.clone();
        let a = bounded(
            scope.deadline,
            auth.track(rpc(scope.deadline, request(scope.wire.auth_view.clone()))?),
        )
        .await?
        .into_inner();
        canonical(&a.registration_id)?;
        let mut friend = self.friend.clone();
        let f = bounded(
            scope.deadline,
            friend.track(rpc(
                scope.deadline,
                request(scope.wire.friend_view.clone()),
            )?),
        )
        .await?
        .into_inner();
        canonical(&f.registration_id)?;
        scope.check()?;
        Ok(native)
    }
}
pub(super) fn wire_identity(
    identity: &TransactionIdentity,
    operation: Uuid,
) -> lease::TransactionIdentity {
    lease::TransactionIdentity {
        producer: lease::Producer::Profile as i32,
        full_xid: identity.full_xid.clone(),
        backend_pid: identity.backend_pid,
        backend_started_unix_micros: identity.backend_started.timestamp_micros(),
        effect_binding_digest: identity.effect_digest.to_vec(),
        system_identifier: identity.system_identifier.clone(),
        database_oid: identity.database_oid,
        settlement_reference: operation.as_bytes().to_vec(),
        producer_instance_id: identity.producer_instance.to_string(),
    }
}
impl CheckedScope {
    pub fn binding(&self) -> &lease::GuardBinding {
        self.wire.binding.as_ref().expect("checked binding")
    }
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn event(&self) -> Uuid {
        self.event
    }
    pub fn family(&self) -> Uuid {
        self.family
    }
    pub fn request(&self) -> Uuid {
        self.request
    }
    pub fn deadline(&self) -> AuthorityRpcDeadline {
        self.deadline
    }
    pub fn check(&self) -> Result<(), Status> {
        self.deadline
            .remaining(BUDGET, self.deadline.expires_micros())
            .map(|_| ())
    }
    pub fn assert_patch(&self, raw: &[u8]) -> Result<(), Status> {
        let binding = self.binding();
        if binding.operation != "updateConcertParticipant"
            || binding.http_method != "PATCH"
            || binding.http_target != format!("/api/concert/events/{}/me", self.event)
            || binding.body_digest != Sha256::digest(raw).as_slice()
        {
            return Err(denied());
        }
        self.check()
    }
}
pub(super) fn binding_digest(binding: &lease::GuardBinding) -> Result<[u8; 32], Status> {
    use platform::actor_delegation::owner_binding::{binding_digest, OwnerBinding};
    let ids = [
        canonical(&binding.actor_id)?,
        canonical(&binding.event_id)?,
        canonical(&binding.session_family_id)?,
        canonical(&binding.request_id)?,
    ];
    if binding.operation.is_empty()
        || binding.operation.len() > 100
        || binding.operation.chars().any(char::is_control)
        || binding.parent_method.is_empty()
        || !binding.parent_method.starts_with('/')
        || binding.parent_digest.len() != 32
        || binding.parent_body_digest.len() != 32
        || binding.body_digest.len() != 32
        || binding.expires_unix_micros <= 0
        || binding.subject_ids.len() > 101
        || !matches!(
            binding.http_method.as_str(),
            "GET" | "PATCH" | "POST" | "PUT" | "DELETE"
        )
        || !binding
            .http_target
            .starts_with(&format!("/api/concert/events/{}/", ids[1]))
        || binding.http_target.len() > 4096
        || binding.http_target.chars().any(char::is_control)
    {
        return Err(denied());
    }
    let mut subjects = std::collections::BTreeSet::new();
    for subject in &binding.subject_ids {
        let id = canonical(subject)?;
        if id != ids[0] && !subjects.insert(id) {
            return Err(denied());
        }
    }
    if subjects.len() > 100 {
        return Err(denied());
    }
    binding_digest(&OwnerBinding {
        actor_id: ids[0],
        event_id: ids[1],
        session_family_id: ids[2],
        request_id: ids[3],
        operation: binding.operation.clone(),
        expires_unix_micros: binding.expires_unix_micros,
        parent_digest: binding
            .parent_digest
            .clone()
            .try_into()
            .map_err(|_| denied())?,
        parent_body_digest: binding
            .parent_body_digest
            .clone()
            .try_into()
            .map_err(|_| denied())?,
        parent_method: binding.parent_method.clone(),
        http_method: binding.http_method.clone(),
        http_target: binding.http_target.clone(),
        body_digest: binding
            .body_digest
            .clone()
            .try_into()
            .map_err(|_| denied())?,
    })
}
pub(super) fn edge_budget(edge: &'static str) -> platform::edge::EdgeBudget {
    platform::edge::EdgeBudget {
        edge,
        expected_latency: Duration::from_millis(100),
        deadline: BUDGET,
        connect_deadline: Duration::from_millis(500),
        retry: platform::edge::RetryClass::Never,
        failure_policy: platform::edge::FailurePolicy::FailRequest,
    }
}
pub(super) fn canonical(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .ok()
        .filter(|v| !v.is_nil() && v.to_string() == value)
        .ok_or_else(denied)
}
fn valid_revision(revision: Option<&lease::OwnerRevision>) -> Result<(), Status> {
    let r = revision.ok_or_else(denied)?;
    canonical(&r.epoch)?;
    if !(1..=super::model::MAX_REVISION).contains(&r.revision) {
        return Err(denied());
    }
    Ok(())
}
fn self_subject(
    subjects: &[lease::SubjectState],
    actor: Uuid,
) -> Result<&lease::SubjectState, Status> {
    let actor = actor.to_string();
    let mut matches = subjects.iter().filter(|s| s.user_id == actor);
    let subject = matches.next().ok_or_else(denied)?;
    if matches.next().is_some() {
        return Err(denied());
    }
    Ok(subject)
}
fn rpc<T>(deadline: AuthorityRpcDeadline, value: T) -> Result<Request<T>, Status> {
    let mut request = Request::new(value);
    request.set_timeout(deadline.remaining(BUDGET, deadline.expires_micros())?);
    Ok(request)
}
async fn bounded<T>(
    deadline: AuthorityRpcDeadline,
    future: impl std::future::Future<Output = Result<T, Status>>,
) -> Result<T, Status> {
    deadline.remaining(BUDGET, deadline.expires_micros())?;
    let result = tokio::time::timeout_at(deadline.stop(), future)
        .await
        .map_err(|_| Status::deadline_exceeded("Self Profile owner deadline exceeded"))?;
    deadline.remaining(BUDGET, deadline.expires_micros())?;
    result
}
fn denied() -> Status {
    Status::permission_denied("Canonical self Profile scope denied")
}
