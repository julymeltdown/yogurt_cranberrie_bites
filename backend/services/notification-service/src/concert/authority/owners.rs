//! Canonical Auth/Friend inspection under the caller's EXISTING root. This
//! adapter exposes no Acquire or root Complete operation and uses Notification
//! workload identity for its own real native transaction registration.
use super::{binding_digest, canonical, AuthorityStore, Intent, NativeIdentity};
use crate::concert::transport::{lease, proto};
use platform::{
    actor_delegation::authority_deadline::AuthorityRpcDeadline,
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};
use tonic::{service::interceptor::InterceptedService, transport::Channel, Request, Status};
use uuid::Uuid;
const BUDGET: Duration = Duration::from_secs(5);
type Transport = InterceptedService<Channel, ServiceIdentityClientInterceptor>;
#[derive(Clone)]
pub(crate) struct Owners {
    auth: lease::auth_owner_authority_service_client::AuthOwnerAuthorityServiceClient<Transport>,
    friend:
        lease::friend_owner_authority_service_client::FriendOwnerAuthorityServiceClient<Transport>,
}
pub(crate) struct CheckedScope {
    wire: proto::ParentScope,
    digest: [u8; 32],
    actor: Uuid,
    deadline: AuthorityRpcDeadline,
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
                platform::edge::EdgeBudget {
                    edge,
                    expected_latency: Duration::from_millis(100),
                    deadline: BUDGET,
                    connect_deadline: Duration::from_millis(500),
                    retry: platform::edge::RetryClass::Never,
                    failure_policy: platform::edge::FailurePolicy::FailRequest,
                },
            )
        };
        let identity = || ServiceIdentityClientInterceptor::new(Workload::Notification, token);
        Ok(Self {
            auth: lease::auth_owner_authority_service_client::AuthOwnerAuthorityServiceClient::new(
                InterceptedService::new(
                    channel(auth, Workload::Auth, "notification-inbox->auth")?,
                    identity()?,
                ),
            ),
            friend:
                lease::friend_owner_authority_service_client::FriendOwnerAuthorityServiceClient::new(
                    InterceptedService::new(
                        channel(friend, Workload::Friend, "notification-inbox->friend")?,
                        identity()?,
                    ),
                ),
        })
    }
    pub async fn inspect(
        &self,
        wire: proto::ParentScope,
        verified: AuthorityRpcDeadline,
    ) -> Result<CheckedScope, Status> {
        let binding = wire.binding.as_ref().ok_or_else(denied)?;
        let actor = canonical(&binding.actor_id)?;
        let digest = binding_digest(binding)?;
        let original = platform::call_deadline::current().ok_or_else(|| {
            Status::invalid_argument("Original Notification owner deadline required")
        })?;
        let mut expires = binding.expires_unix_micros.min(verified.expires_micros());
        for view in [&wire.auth_view, &wire.friend_view] {
            let view = view.as_ref().ok_or_else(denied)?;
            canonical(&view.guard_id)?;
            if view.binding_digest != digest
                || view.view_secret.len() != 32
                || view.view_secret.iter().all(|b| *b == 0)
                || view.expires_unix_micros > binding.expires_unix_micros
            {
                return Err(denied());
            }
            expires = expires.min(view.expires_unix_micros);
        }
        let root = wire.root_view.as_ref().ok_or_else(denied)?;
        let root_scope = root.scope.as_ref().ok_or_else(denied)?;
        canonical(&root_scope.instance_id)?;
        canonical(&root_scope.scope_id)?;
        if root.binding.as_ref() != Some(binding)
            || root_scope.binding_digest != digest
            || root.view_secret.len() != 32
            || root.view_secret.iter().all(|b| *b == 0)
        {
            return Err(denied());
        }
        let scope = CheckedScope {
            wire,
            digest,
            actor,
            deadline: AuthorityRpcDeadline::within_original(
                expires,
                original.min(verified.stop().into_std()),
                BUDGET,
            )?,
        };
        self.check(&scope, &[]).await?;
        Ok(scope)
    }
    /// Enroll/check actual source actors. False source decisions mean the inbox
    /// item must be hidden; they never authorize a target or event membership.
    pub async fn check(
        &self,
        scope: &CheckedScope,
        subjects: &[Uuid],
    ) -> Result<BTreeMap<Uuid, bool>, Status> {
        scope.check()?;
        let subjects: BTreeSet<_> = subjects
            .iter()
            .copied()
            .filter(|v| *v != scope.actor)
            .collect();
        if subjects.len() > 100 || subjects.iter().any(Uuid::is_nil) {
            return Err(denied());
        }
        let mut expected = subjects.clone();
        expected.insert(scope.actor);
        let binding = scope.binding().clone();
        let ids: Vec<_> = subjects.iter().map(Uuid::to_string).collect();
        let mut auth = self.auth.clone();
        let a = if ids.is_empty() {
            bounded(
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
            .into_inner()
        } else {
            bounded(
                scope.deadline,
                auth.enroll(rpc(
                    scope.deadline,
                    lease::EnrollRequest {
                        view: scope.wire.auth_view.clone(),
                        binding: Some(binding.clone()),
                        subject_ids: ids.clone(),
                    },
                )?),
            )
            .await?
            .into_inner()
        };
        let mut friend = self.friend.clone();
        let f = if ids.is_empty() {
            bounded(
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
            .into_inner()
        } else {
            bounded(
                scope.deadline,
                friend.enroll(rpc(
                    scope.deadline,
                    lease::EnrollRequest {
                        view: scope.wire.friend_view.clone(),
                        binding: Some(binding),
                        subject_ids: ids,
                    },
                )?),
            )
            .await?
            .into_inner()
        };
        valid_revision(a.revision.as_ref())?;
        valid_revision(f.revision.as_ref())?;
        let a_subjects = states(&a.subjects)?;
        let f_subjects = states(&f.subjects)?;
        let actor_a = a_subjects.get(&scope.actor).ok_or_else(denied)?;
        let actor_f = f_subjects.get(&scope.actor).ok_or_else(denied)?;
        if !a.session_live
            || !actor_a.active
            || actor_a.security_revision <= 0
            || !actor_f.active
            || actor_f.security_revision != 0
            || actor_f.actor_blocks_subject
            || actor_f.subject_blocks_actor
        {
            return Err(denied());
        }
        let mut result = BTreeMap::new();
        for id in expected {
            let a = a_subjects.get(&id).ok_or_else(denied)?;
            let f = f_subjects.get(&id).ok_or_else(denied)?;
            if a.security_revision <= 0 || f.security_revision != 0 {
                return Err(denied());
            }
            result.insert(
                id,
                a.active && f.active && !f.actor_blocks_subject && !f.subject_blocks_actor,
            );
        }
        scope.check()?;
        Ok(result)
    }
    pub async fn track(
        &self,
        scope: &CheckedScope,
        store: &AuthorityStore,
        intent: &Intent,
        identity: &NativeIdentity,
    ) -> Result<(), Status> {
        self.check(scope, &[]).await?;
        if intent.binding != *scope.binding() || intent.binding_digest != scope.digest {
            return Err(denied());
        }
        let transaction = identity.wire(intent.id);
        let mut auth = self.auth.clone();
        let a = bounded(
            scope.deadline,
            auth.track(rpc(
                scope.deadline,
                lease::TrackRequest {
                    view: scope.wire.auth_view.clone(),
                    binding: Some(scope.binding().clone()),
                    transaction: Some(transaction.clone()),
                },
            )?),
        )
        .await?
        .into_inner();
        let mut friend = self.friend.clone();
        let f = bounded(
            scope.deadline,
            friend.track(rpc(
                scope.deadline,
                lease::TrackRequest {
                    view: scope.wire.friend_view.clone(),
                    binding: Some(scope.binding().clone()),
                    transaction: Some(transaction),
                },
            )?),
        )
        .await?
        .into_inner();
        store
            .acknowledge(
                intent,
                identity,
                canonical(&a.registration_id)?,
                canonical(&f.registration_id)?,
                scope.deadline,
            )
            .await?;
        scope.check()
    }
}
impl CheckedScope {
    pub fn binding(&self) -> &lease::GuardBinding {
        self.wire.binding.as_ref().expect("checked binding")
    }
    pub fn wire(&self) -> &proto::ParentScope {
        &self.wire
    }
    pub fn deadline(&self) -> AuthorityRpcDeadline {
        self.deadline
    }
    pub fn digest(&self) -> [u8; 32] {
        self.digest
    }
    pub fn actor(&self) -> Uuid {
        self.actor
    }
    pub fn check(&self) -> Result<(), Status> {
        self.deadline
            .remaining(BUDGET, self.deadline.expires_micros())
            .map(|_| ())
    }
}
fn states(values: &[lease::SubjectState]) -> Result<BTreeMap<Uuid, &lease::SubjectState>, Status> {
    let mut result = BTreeMap::new();
    for value in values {
        if result.insert(canonical(&value.user_id)?, value).is_some() {
            return Err(denied());
        }
    }
    Ok(result)
}
fn valid_revision(value: Option<&lease::OwnerRevision>) -> Result<(), Status> {
    let value = value.ok_or_else(denied)?;
    canonical(&value.epoch)?;
    if !(1..=concert_domain::MAX_REVISION).contains(&value.revision) {
        return Err(denied());
    }
    Ok(())
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
    let value = tokio::time::timeout_at(deadline.stop(), work)
        .await
        .map_err(|_| Status::deadline_exceeded("Notification owner deadline"))??;
    deadline.remaining(BUDGET, deadline.expires_micros())?;
    Ok(value)
}
fn denied() -> Status {
    Status::permission_denied("Canonical Notification owner scope denied")
}
