//! Restore harness derived from backend/services/concert-service/tests/canonical_owner_composition_support/notification.rs at 3afb8111d8ef206f3e73c63186699c98d9fd4997; original SHA256 32df277ffc99b4007bbe815b3c16ddde7af877553eb323aeb77fbd770ae0f7b9.
//! Negative-only Complete fault and private request observation. Every positive
//! result comes from the actual guarded NotificationInboxService with NO target
//! provider. No public inbox or target-policy acceptance is manufactured.
use concert_service::adapters::out::relationship_authority::guarded::{
    ChildQuery, ChildRecoveryReference,
};
use notification_service::concert::{authority::NotificationInboxService, transport::proto};
use proto::concert_inbox_service_server::ConcertInboxService;
use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc, Mutex,
};
use tonic::{Request, Response, Status};

#[derive(Default)]
pub(super) struct Observation {
    pub deny_complete: AtomicBool,
    pub complete_calls: AtomicUsize,
    pub reference: Mutex<Option<ChildRecoveryReference>>,
}
pub(super) struct Observed {
    pub inner: NotificationInboxService,
    pub state: Arc<Observation>,
}
#[tonic::async_trait]
impl ConcertInboxService for Observed {
    async fn execute(
        &self,
        request: Request<proto::ExecuteRequest>,
    ) -> Result<Response<proto::ExecuteResponse>, Status> {
        let message = request.get_ref();
        let binding = message
            .scope
            .as_ref()
            .and_then(|p| p.binding.as_ref())
            .ok_or_else(|| Status::invalid_argument("original parent required"))?;
        let intent = message
            .intent
            .as_ref()
            .ok_or_else(|| Status::invalid_argument("original intent required"))?;
        // Cross-crate generated wire types have identical frozen schemas. Decode
        // only the immutable original for private observation, never admission.
        use prost::Message;
        let wire = concert_service::adapters::out::relationship_authority::guarded::proto::GuardBinding::decode(binding.encode_to_vec().as_slice()).map_err(|_|Status::internal("binding observation"))?;
        let actor = binding
            .actor_id
            .parse()
            .map_err(|_| Status::internal("actor observation"))?;
        let q = ChildQuery {
            actor, subject: actor,
            event: binding.event_id.parse().map_err(|_|Status::internal("event observation"))?,
            family: binding.session_family_id.parse().map_err(|_|Status::internal("family observation"))?,
            request: binding.request_id.parse().map_err(|_|Status::internal("request observation"))?,
            binding_digest: concert_service::adapters::out::relationship_authority::guarded::wire_binding_digest(&wire)?,
            expires_unix_micros: binding.expires_unix_micros,
        };
        *self.state.reference.lock().unwrap() = Some(ChildRecoveryReference::NotificationInbox {
            query: q,
            child_id: intent.child_id.clone(),
            completion_secret: intent
                .completion_secret
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect(),
        });
        self.inner.execute(request).await
    }
    async fn check(
        &self,
        request: Request<proto::GuardRequest>,
    ) -> Result<Response<proto::GuardReply>, Status> {
        self.inner.check(request).await
    }
    async fn complete(
        &self,
        request: Request<proto::GuardRequest>,
    ) -> Result<Response<proto::GuardReply>, Status> {
        self.state.complete_calls.fetch_add(1, Ordering::SeqCst);
        if self.state.deny_complete.load(Ordering::SeqCst) {
            return Err(Status::unavailable("CF01 negative completion fault"));
        }
        self.inner.complete(request).await
    }
}
