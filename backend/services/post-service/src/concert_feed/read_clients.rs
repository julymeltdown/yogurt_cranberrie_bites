//! Actual authenticated owner and Concert callback clients. No retries,
//! observation cache, bearer completion secret or inherited Post feed fallback.
use super::{
    authority_proto as wire, read_admission::ReadAdmission, read_tracking::RegistrationOwner,
    read_transaction::CapturedReadTransaction,
};
use chrono::{DateTime, Utc};
use platform::{
    actor_delegation::{
        concert_read::PostConcertRootReadSigner, request_for, ActorDelegationClient,
    },
    edge::{EdgeBudget, FailurePolicy, RetryClass},
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use post_domain::concert_feed::Error;
use std::time::Duration;
use tonic::{service::interceptor::InterceptedService, transport::Channel, Request, Response};
use uuid::Uuid;

type OwnerTransport = InterceptedService<Channel, ServiceIdentityClientInterceptor>;
type RootTransport =
    InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>;
const EDGE: EdgeBudget = EdgeBudget {
    edge: "post.concert_feed.canonical_read",
    expected_latency: Duration::from_millis(100),
    deadline: Duration::from_secs(5),
    connect_deadline: Duration::from_millis(500),
    retry: RetryClass::Never,
    failure_policy: FailurePolicy::FailRequest,
};
#[derive(Clone)]
pub(crate) struct ReadClients {
    auth:
        wire::auth_owner_authority_service_client::AuthOwnerAuthorityServiceClient<OwnerTransport>,
    friend: wire::friend_owner_authority_service_client::FriendOwnerAuthorityServiceClient<
        OwnerTransport,
    >,
    root: wire::concert_root_read_service_client::ConcertRootReadServiceClient<RootTransport>,
}
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct SubjectFence {
    pub subject: Uuid,
    pub event_revision: i64,
    pub pair_revision: i64,
    pub auth_epoch: Uuid,
    pub auth_revision: i64,
    pub friend_epoch: Uuid,
    pub friend_revision: i64,
    pub actor_security: i64,
    pub subject_security: i64,
    pub expires_at: DateTime<Utc>,
}
impl ReadClients {
    pub(crate) fn connect(
        auth_endpoint: &str,
        friend_endpoint: &str,
        concert_endpoint: &str,
        token: &str,
        tls: &platform::grpc_tls::TlsMaterial,
        signer: PostConcertRootReadSigner,
    ) -> anyhow::Result<Self> {
        let auth = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            Some(tls),
            auth_endpoint,
            Workload::Auth,
            EDGE.validate()?,
        )?;
        let friend = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            Some(tls),
            friend_endpoint,
            Workload::Friend,
            EDGE.validate()?,
        )?;
        let root = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            Some(tls),
            concert_endpoint,
            Workload::Concert,
            EDGE.validate()?,
        )?;
        let identity = ServiceIdentityClientInterceptor::new(Workload::Post, token)?;
        Ok(Self {
            auth: wire::auth_owner_authority_service_client::AuthOwnerAuthorityServiceClient::with_interceptor(
                auth, identity.clone(),
            ).max_decoding_message_size(65536).max_encoding_message_size(65536),
            friend: wire::friend_owner_authority_service_client::FriendOwnerAuthorityServiceClient::with_interceptor(
                friend, identity.clone(),
            ).max_decoding_message_size(65536).max_encoding_message_size(65536),
            root: wire::concert_root_read_service_client::ConcertRootReadServiceClient::with_interceptor(
                signer.client(root), identity,
            ).max_decoding_message_size(16384).max_encoding_message_size(65536),
        })
    }
    fn request<T>(&self, a: &ReadAdmission, value: T) -> Result<Request<T>, Error> {
        a.check()?;
        let mut request = Request::new(value);
        request.set_timeout(
            a.deadline
                .remaining(
                    Duration::from_secs(5),
                    a.context.expires_at.timestamp_micros(),
                )
                .map_err(|_| Error::Authority)?,
        );
        Ok(request)
    }
    /// Enroll through the actual root first, so unknown Post authors acquire
    /// the SAME held local pair/participant guard used by public projection.
    pub(crate) async fn subject(
        &self,
        a: &ReadAdmission,
        subject: Uuid,
    ) -> Result<SubjectFence, Error> {
        a.check()?;
        if subject.is_nil() {
            return Err(Error::Invalid);
        }
        let root = a.authority.root.as_ref().ok_or(Error::Authority)?;
        let message = wire::CheckSubjectRequest {
            root: Some(root.clone()),
            subject_id: subject.to_string(),
        };
        let mut request =
            request_for(message, &a.context.actor_id.to_string()).map_err(|_| Error::Authority)?;
        request.set_timeout(
            a.deadline
                .remaining(
                    Duration::from_secs(5),
                    a.context.expires_at.timestamp_micros(),
                )
                .map_err(|_| Error::Authority)?,
        );
        let local = reply(
            self.root
                .clone()
                .check_subject(request)
                .await
                .map_err(root_remote)?,
        )?;
        a.check()?;
        if local.scope.as_ref() != root.scope.as_ref()
            || local.subject_id != subject.to_string()
            || !revision(local.event_revision)
            || !revision(local.pair_revision)
            || local.expires_unix_micros > a.context.expires_at.timestamp_micros()
        {
            return Err(Error::Authority);
        }
        let expires_at = DateTime::from_timestamp_micros(local.expires_unix_micros)
            .filter(|expires| *expires > Utc::now())
            .ok_or(Error::Authority)?;
        let local_auth = owner_revision(local.auth_revision.as_ref())?;
        let local_friend = owner_revision(local.friend_revision.as_ref())?;
        let binding = a.authority.binding.clone().ok_or(Error::Authority)?;
        let auth_request = self.request(
            a,
            wire::CheckRequest {
                view: a.authority.auth.clone(),
                binding: Some(binding.clone()),
            },
        )?;
        let auth = reply(
            self.auth
                .clone()
                .check(auth_request)
                .await
                .map_err(remote)?,
        )?;
        a.check()?;
        if !auth.session_live || owner_revision(auth.revision.as_ref())? != local_auth {
            return Err(Error::Authority);
        }
        let actor = state(&auth.subjects, a.context.actor_id)?;
        let target = state(&auth.subjects, subject)?;
        if !actor.active
            || !target.active
            || !revision(actor.security_revision)
            || !revision(target.security_revision)
        {
            return Err(Error::Forbidden);
        }
        let friend_request = self.request(
            a,
            wire::CheckRequest {
                view: a.authority.friend.clone(),
                binding: Some(binding),
            },
        )?;
        let friend = reply(
            self.friend
                .clone()
                .check(friend_request)
                .await
                .map_err(remote)?,
        )?;
        a.check()?;
        if owner_revision(friend.revision.as_ref())? != local_friend {
            return Err(Error::Authority);
        }
        let pair = state(&friend.subjects, subject)?;
        if pair.actor_blocks_subject || pair.subject_blocks_actor {
            return Err(Error::Forbidden);
        }
        Ok(SubjectFence {
            subject,
            event_revision: local.event_revision,
            pair_revision: local.pair_revision,
            auth_epoch: local_auth.0,
            auth_revision: local_auth.1,
            friend_epoch: local_friend.0,
            friend_revision: local_friend.1,
            actor_security: actor.security_revision,
            subject_security: target.security_revision,
            expires_at,
        })
    }
    /// Called with actual captured identity while that transaction is open,
    /// before any protected row snapshot. Never register a caller's claimed PID.
    pub(crate) async fn track_owner(
        &self,
        a: &ReadAdmission,
        identity: &CapturedReadTransaction,
        owner: RegistrationOwner,
    ) -> Result<Uuid, Error> {
        let transaction = identity.wire();
        let view = match owner {
            RegistrationOwner::Auth => a.authority.auth.clone(),
            RegistrationOwner::Friend => a.authority.friend.clone(),
        };
        let request = self.request(
            a,
            wire::TrackRequest {
                view,
                binding: a.authority.binding.clone(),
                transaction: Some(transaction),
            },
        )?;
        let response = match owner {
            RegistrationOwner::Auth => self.auth.clone().track(request).await,
            RegistrationOwner::Friend => self.friend.clone().track(request).await,
        }
        .map_err(remote)?;
        // Return each real acknowledgement separately. The caller records it
        // BEFORE checking deadline again or starting the second owner's call.
        // An unknown/lost response stays an attempted registration locally.
        canonical_id(&reply(response)?.registration_id)
    }
}
fn reply<T>(response: Response<T>) -> Result<T, Error> {
    if response
        .metadata()
        .get("cache-control")
        .and_then(|value| value.to_str().ok())
        != Some("no-store")
    {
        return Err(Error::Unavailable);
    }
    Ok(response.into_inner())
}
fn remote(_: tonic::Status) -> Error {
    // An invalid/unavailable owner view is not a denied content author. Only
    // explicit checked owner states below can produce a filterable denial.
    Error::Unavailable
}
fn root_remote(status: tonic::Status) -> Error {
    match status.code() {
        // Reserved by the root callback for a currently inaccessible subject.
        // Invalid scope/view/caller is PermissionDenied and fails the operation.
        tonic::Code::NotFound => Error::Forbidden,
        _ => Error::Unavailable,
    }
}
fn canonical_id(value: &str) -> Result<Uuid, Error> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or(Error::Authority)
}
fn revision(value: i64) -> bool {
    (1..=9_007_199_254_740_991).contains(&value)
}
fn owner_revision(value: Option<&wire::OwnerRevision>) -> Result<(Uuid, i64), Error> {
    let value = value.ok_or(Error::Authority)?;
    if !revision(value.revision) {
        return Err(Error::Authority);
    }
    Ok((canonical_id(&value.epoch)?, value.revision))
}
fn state(states: &[wire::SubjectState], subject: Uuid) -> Result<&wire::SubjectState, Error> {
    if states.len() > 101 {
        return Err(Error::Authority);
    }
    let mut ids = std::collections::BTreeSet::new();
    let mut found = None;
    for state in states {
        let user = canonical_id(&state.user_id)?;
        if !ids.insert(user) {
            return Err(Error::Authority);
        }
        if user == subject {
            found = Some(state);
        }
    }
    found.ok_or(Error::Authority)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invalid_owner_views_and_root_scope_failures_cannot_be_filtered_into_empty_success() {
        for code in [
            tonic::Code::NotFound,
            tonic::Code::PermissionDenied,
            tonic::Code::Unavailable,
            tonic::Code::DeadlineExceeded,
        ] {
            assert_eq!(
                remote(tonic::Status::new(code, "private detail")),
                Error::Unavailable
            );
        }
        assert_eq!(
            root_remote(tonic::Status::permission_denied("scope invalid")),
            Error::Unavailable
        );
        assert_eq!(
            root_remote(tonic::Status::not_found("subject unavailable")),
            Error::Forbidden
        );
    }
    #[test]
    fn successful_dependency_response_requires_explicit_no_store_metadata() {
        assert!(matches!(reply(Response::new(())), Err(Error::Unavailable)));
        let mut r = Response::new(());
        r.metadata_mut()
            .insert("cache-control", "no-store".parse().unwrap());
        assert!(reply(r).is_ok());
    }
    #[test]
    fn ambiguous_subject_vectors_cannot_attest_an_author() {
        let user = Uuid::new_v4();
        let row = wire::SubjectState {
            user_id: user.to_string(),
            active: true,
            actor_blocks_subject: false,
            subject_blocks_actor: false,
            security_revision: 1,
        };
        assert!(state(&[row.clone()], user).is_ok());
        assert!(matches!(
            state(&[row.clone(), row.clone()], user),
            Err(Error::Authority)
        ));
        let mut malformed = row;
        malformed.user_id = Uuid::nil().to_string();
        assert!(matches!(state(&[malformed], user), Err(Error::Authority)));
    }
}
