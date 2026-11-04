//! Dedicated Profile RPC and retained visibility child; no World/self projection.
use super::super::relationship_authority::guarded::{
    ChildQuery, ChildRecoveryReference, GuardedSubject, HeldChildGuard,
};
use platform::{
    actor_delegation::{
        authority_deadline::AuthorityRpcDeadline, relationship_read::RelationshipReadSigner,
        request_for, ActorDelegationClient,
    },
    edge::{EdgeBudget, FailurePolicy, RetryClass},
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use std::{sync::Arc, time::Duration};
use tonic::{service::interceptor::InterceptedService, transport::Channel, Status};
pub(super) mod proto {
    tonic::include_proto!("profile.concert_public.v1");
}
type Transport =
    InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>;
type Client =
    proto::concert_public_profile_service_client::ConcertPublicProfileServiceClient<Transport>;
const BUDGET: Duration = Duration::from_secs(2);
const EDGE: EdgeBudget = EdgeBudget {
    edge: "concert.public_profile.owner_read",
    expected_latency: Duration::from_millis(100),
    deadline: BUDGET,
    connect_deadline: Duration::from_millis(500),
    retry: RetryClass::Never,
    failure_policy: FailurePolicy::FailRequest,
};
#[derive(Clone)]
pub(super) struct ProfileReader {
    client: Client,
}
pub(super) struct Content {
    pub display_name: String,
    pub bio: Option<String>,
}
impl ProfileReader {
    pub fn connect(
        endpoint: &str,
        token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        signer: RelationshipReadSigner,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Profile,
            EDGE.validate()?,
        )?;
        let client = proto::concert_public_profile_service_client::ConcertPublicProfileServiceClient::with_interceptor(
            signer.client(channel, Workload::Profile)?,
            ServiceIdentityClientInterceptor::new(Workload::Concert, token)?,
        )
        .max_decoding_message_size(4096)
        .max_encoding_message_size(4096);
        Ok(Self { client })
    }
    pub async fn read(&self, subject: &GuardedSubject) -> Result<Content, Status> {
        subject.check().await?;
        let q = subject.query();
        let b = subject.binding();
        if q.actor.uuid() != b.actor()
            || q.event.uuid() != b.event()
            || q.field != concert_domain::relationship::Field::PublicProfile
        {
            return Err(unavailable());
        }
        let original = b.deadline();
        let deadline = AuthorityRpcDeadline::within_original(
            original.expires_micros(),
            original.stop().into_std(),
            BUDGET,
        )?;
        let query = ChildQuery::from_subject(subject);
        let child_id = query.profile_child_id(uuid::Uuid::new_v4())?;
        let completion_secret = uuid::Uuid::new_v4().simple().to_string();
        let reference = ChildRecoveryReference::ProfilePublic {
            query,
            child_id: child_id.clone(),
            completion_secret: completion_secret.clone(),
        };
        // Persist the exact private completion material before any remote I/O.
        subject.register_child_intent(&reference).await?;
        let m = proto::PublicProfileReadRequest {
            actor_id: b.actor().to_string(),
            event_id: b.event().to_string(),
            subject_id: q.subject.uuid().to_string(),
            session_family_id: b.session_family().to_string(),
            request_id: b.request_id().to_string(),
            purpose: 1,
            expires_unix_micros: deadline.expires_micros(),
            child_id,
            completion_secret,
        };
        let child = Arc::new(ProfileChild {
            client: self.client.clone(),
            request: proto::PublicProfileGuardRequest {
                actor_id: m.actor_id.clone(),
                event_id: m.event_id.clone(),
                subject_id: m.subject_id.clone(),
                session_family_id: m.session_family_id.clone(),
                request_id: m.request_id.clone(),
                purpose: 1,
                expires_unix_micros: deadline.expires_micros(),
                child_id: m.child_id.clone(),
                completion_secret: m.completion_secret.clone(),
            },
            deadline,
            reference,
        });
        // Attach before dispatch so ordinary terminal error handling can cancel
        // even an intent whose RPC never reached the owner.
        subject.retain_child(child).await?;
        let mut request = request_for(m.clone(), &m.actor_id)?;
        request.set_timeout(deadline.remaining(BUDGET, deadline.expires_micros())?);
        let response = self.client.clone().read_current(request).await?;
        deadline.remaining(BUDGET, deadline.expires_micros())?;
        if response
            .metadata()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            != Some("no-store")
        {
            return Err(unavailable());
        }
        let body = response.into_inner();
        if body.actor_id != m.actor_id
            || body.event_id != m.event_id
            || body.subject_id != m.subject_id
            || body.session_family_id != m.session_family_id
            || body.request_id != m.request_id
            || body.child_id != m.child_id
            || body.completion_secret != m.completion_secret
        {
            return Err(unavailable());
        }
        subject.check().await?;
        let now = chrono::Utc::now().timestamp_micros();
        if body.observed_at_unix_micros <= 0
            || body.observed_at_unix_micros > now
            || now.saturating_sub(body.observed_at_unix_micros) > 2_000_000
            || body.display_name.trim().is_empty()
            || body.display_name.chars().count() > 40
            || body.display_name.chars().any(char::is_control)
            || body
                .bio
                .as_ref()
                .is_some_and(|v| v.chars().count() > 160 || v.chars().any(char::is_control))
        {
            return Err(unavailable());
        }
        Ok(Content {
            display_name: body.display_name,
            bio: body.bio,
        })
    }
}
impl ProfileReader {
    pub async fn recover_one(&self, reference: &ChildRecoveryReference) -> Result<(), Status> {
        let ChildRecoveryReference::ProfilePublic {
            query,
            child_id,
            completion_secret,
        } = reference
        else {
            return Err(unavailable());
        };
        reference.intent_id()?;
        let id = uuid::Uuid::parse_str(completion_secret).map_err(|_| unavailable())?;
        if id.is_nil() || id.simple().to_string() != *completion_secret {
            return Err(unavailable());
        }
        let deadline =
            AuthorityRpcDeadline::new(chrono::Utc::now().timestamp_micros() + 2_000_000, BUDGET)?;
        ProfileChild {
            client: self.client.clone(),
            deadline,
            reference: reference.clone(),
            request: proto::PublicProfileGuardRequest {
                actor_id: query.actor.to_string(),
                event_id: query.event.to_string(),
                subject_id: query.subject.to_string(),
                session_family_id: query.family.to_string(),
                request_id: query.request.to_string(),
                purpose: 1,
                expires_unix_micros: query.expires_unix_micros,
                child_id: child_id.clone(),
                completion_secret: completion_secret.clone(),
            },
        }
        .complete_settled()
        .await
    }
}
struct ProfileChild {
    client: Client,
    request: proto::PublicProfileGuardRequest,
    deadline: AuthorityRpcDeadline,
    reference: ChildRecoveryReference,
}
impl ProfileChild {
    async fn control(&self, complete: bool) -> Result<(), Status> {
        let deadline = if complete {
            AuthorityRpcDeadline::new(chrono::Utc::now().timestamp_micros() + 2_000_000, BUDGET)?
        } else {
            self.deadline
        };
        deadline.remaining(BUDGET, deadline.expires_micros())?;
        let mut m = self.request.clone();
        if !complete {
            m.completion_secret.clear();
        }
        let mut request = request_for(m.clone(), &m.actor_id)?;
        request.set_timeout(deadline.remaining(BUDGET, deadline.expires_micros())?);
        // Settlement is a separate bounded control call, never another data read.
        let response = platform::call_deadline::scope(deadline.stop().into_std(), async {
            if complete {
                self.client.clone().complete(request).await
            } else {
                self.client.clone().check(request).await
            }
        })
        .await?;
        deadline.remaining(BUDGET, deadline.expires_micros())?;
        if response
            .metadata()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            != Some("no-store")
            || response.into_inner().held == complete
        {
            return Err(unavailable());
        }
        Ok(())
    }
}
#[async_trait::async_trait]
impl HeldChildGuard for ProfileChild {
    fn recovery_reference(&self) -> Result<ChildRecoveryReference, Status> {
        Ok(self.reference.clone())
    }
    async fn check(&self) -> Result<(), Status> {
        self.control(false).await
    }
    async fn complete_settled(&self) -> Result<(), Status> {
        self.control(true).await
    }
}
fn unavailable() -> Status {
    Status::unavailable("Public Profile owner unavailable")
}
