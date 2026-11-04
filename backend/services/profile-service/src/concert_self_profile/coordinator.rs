//! Exact existing-operation recovery query to the configured owning Concert.
//! No request value selects a decision or starts a new effect.
use super::{
    guard::{binding_digest, edge_budget},
    identity::prepared_gid,
    model::Error,
    operation::Operation,
    wire::{decision, lease},
};
use platform::{
    actor_delegation::{
        authority_deadline::AuthorityRpcDeadline, participant_decision::ParticipantDecisionSigner,
        request_for, ActorDelegationClient,
    },
    grpc_auth::{ServiceIdentityClientInterceptor, Workload},
};
use prost::Message;
use std::time::Duration;
use tonic::{service::interceptor::InterceptedService, transport::Channel, Status};
type Transport =
    InterceptedService<ActorDelegationClient<Channel>, ServiceIdentityClientInterceptor>;
#[derive(Clone)]
pub(super) struct Coordinator {
    client:decision::concert_participant_decision_service_client::ConcertParticipantDecisionServiceClient<Transport>,
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Decision {
    Commit,
    Abort,
    Unresolved,
}
impl Coordinator {
    pub fn connect(
        endpoint: &str,
        token: &str,
        tls: Option<&platform::grpc_tls::TlsMaterial>,
        signer: ParticipantDecisionSigner,
    ) -> anyhow::Result<Self> {
        let channel = platform::grpc_tls::lazy_channel_for_edge_with_optional_tls(
            tls,
            endpoint,
            Workload::Concert,
            edge_budget("profile-self->concert-decision"),
        )?;
        let transport = InterceptedService::new(
            signer.client(channel),
            ServiceIdentityClientInterceptor::new(Workload::Profile, token)?,
        );
        Ok(Self{client:decision::concert_participant_decision_service_client::ConcertParticipantDecisionServiceClient::new(transport)})
    }
    pub async fn query(
        &self,
        operation: &Operation,
        deadline: AuthorityRpcDeadline,
    ) -> Result<Decision, Error> {
        let binding = lease::GuardBinding::decode(operation.binding_proto.as_slice())
            .map_err(|_| Error::Unavailable)?;
        if binding_digest(&binding).map_err(|_| Error::Unavailable)? != operation.binding_digest {
            return Err(Error::Unavailable);
        }
        let query = decision::ParticipantDecisionRequest {
            actor_id: operation.actor.to_string(),
            event_id: operation.event.to_string(),
            session_family_id: operation.family.to_string(),
            operation_id: operation.id.to_string(),
            parent_digest: binding.parent_digest,
            body_digest: binding.body_digest,
            guard_binding_digest: operation.binding_digest.to_vec(),
            profile_prepared_gid: prepared_gid(operation.id)?,
            profile_effect_digest: operation.identity.effect_digest.to_vec(),
            authority_expires_unix_micros: deadline.expires_micros(),
        };
        let mut request = request_for(query.clone(), &operation.actor.to_string())
            .map_err(|_| Error::Unavailable)?;
        request.set_timeout(
            deadline
                .remaining(Duration::from_secs(5), deadline.expires_micros())
                .map_err(|_| Error::Unavailable)?,
        );
        let mut client = self.client.clone();
        let response = tokio::time::timeout_at(deadline.stop(), client.resolve_decision(request))
            .await
            .map_err(|_| Error::Unavailable)?
            .map_err(|_: Status| Error::Unavailable)?;
        super::identity::remaining(deadline)?;
        if response
            .metadata()
            .get("cache-control")
            .and_then(|v| v.to_str().ok())
            != Some("no-store")
        {
            return Err(Error::Unavailable);
        }
        let response = response.into_inner();
        if response.query.as_ref() != Some(&query) {
            return Err(Error::Unavailable);
        }
        let decision = decision::ParticipantDecision::try_from(response.decision)
            .map_err(|_| Error::Unavailable)?;
        if decision == decision::ParticipantDecision::Unresolved && response.committed_revision == 0
        {
            return Ok(Decision::Unresolved);
        }
        let transaction = response.concert_transaction.ok_or(Error::Unavailable)?;
        let number = |v: &str| {
            v.parse::<u64>()
                .ok()
                .is_some_and(|n| n > 0 && n.to_string() == v)
        };
        if !number(&transaction.full_xid)
            || !number(&transaction.system_identifier)
            || transaction.database_oid == 0
            || transaction.backend_pid <= 0
            || transaction.backend_started_unix_micros <= 0
        {
            return Err(Error::Unavailable);
        }
        match decision {
            decision::ParticipantDecision::Commit
                if (1..=super::model::MAX_REVISION).contains(&response.committed_revision) =>
            {
                Ok(Decision::Commit)
            }
            decision::ParticipantDecision::Abort if response.committed_revision == 0 => {
                Ok(Decision::Abort)
            }
            _ => Err(Error::Unavailable),
        }
    }
}
