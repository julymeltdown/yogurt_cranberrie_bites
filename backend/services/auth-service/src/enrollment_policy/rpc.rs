//! Same-listener, Gateway-only enrollment adapter; no public publication RPC.
use super::*;
use crate::adapters::out::persistence::user_repository::PgUserRepository;
use crate::application::use_cases::signup::SignupUseCase;
use platform::actor_delegation::{ActorDelegationVerifier, VerifiedActor};
use platform::grpc_auth::{CallerRegistry, VerifiedCaller, Workload};
use platform::grpc_method_policy::{GuardedService, MethodPolicy};
use serde::{de::DeserializeOwned, Serialize};
use std::{future::Future, sync::Arc};
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("auth.enrollment.v1");
}
use proto::enrollment_policy_service_server::{
    EnrollmentPolicyService, EnrollmentPolicyServiceServer,
};
const GATEWAY: &[Workload] = &[Workload::Gateway];
pub static POLICY: MethodPolicy = MethodPolicy {
    service: "auth.enrollment.v1.EnrollmentPolicyService",
    workload: Some(Workload::Auth),
    methods: &[
        ("ReadPolicy", GATEWAY),
        ("BeginAttempt", GATEWAY),
        ("Signup", GATEWAY),
        ("Verify", GATEWAY),
        ("Resend", GATEWAY),
        ("ReadAdmission", GATEWAY),
        ("AcceptConsent", GATEWAY),
    ],
    actor_fields: &[("ReadAdmission", &[&[1]]), ("AcceptConsent", &[&[1]])],
};
#[derive(Clone)]
pub struct EnrollmentServer {
    store: EnrollmentStore,
    signup: SignupUseCase<Arc<PgUserRepository>>,
}
impl EnrollmentServer {
    pub fn new(store: EnrollmentStore, signup: SignupUseCase<Arc<PgUserRepository>>) -> Self {
        Self { store, signup }
    }
}
pub fn guarded_server(
    store: EnrollmentStore,
    signup: SignupUseCase<Arc<PgUserRepository>>,
    registry: CallerRegistry,
    verifier: ActorDelegationVerifier,
) -> anyhow::Result<GuardedService<EnrollmentPolicyServiceServer<EnrollmentServer>>> {
    platform::grpc_method_policy::guard(
        &POLICY,
        EnrollmentPolicyServiceServer::new(EnrollmentServer::new(store, signup))
            .max_decoding_message_size(16384)
            .max_encoding_message_size(32768),
        registry,
        verifier,
    )
}
fn caller<T>(request: &Request<T>) -> Result<(), Status> {
    match request.extensions().get::<VerifiedCaller>() {
        Some(VerifiedCaller(Workload::Gateway)) => Ok(()),
        Some(_) => Err(Status::permission_denied("PERMISSION_DENIED")),
        None => Err(Status::unauthenticated("AUTHENTICATION_REQUIRED")),
    }
}
fn actor(request: &Request<proto::ActorCommand>) -> Result<Uuid, Status> {
    caller(request)?;
    let id = Uuid::parse_str(&request.get_ref().user_id)
        .map_err(|_| Status::invalid_argument("INVALID_INPUT"))?;
    if id.is_nil() || id.to_string() != request.get_ref().user_id {
        return Err(Status::invalid_argument("INVALID_INPUT"));
    }
    match request.extensions().get::<VerifiedActor>() {
        Some(VerifiedActor(actual)) if *actual == id => Ok(id),
        _ => Err(Status::permission_denied("PERMISSION_DENIED")),
    }
}
fn decode<T: DeserializeOwned>(value: &str) -> Result<T, Status> {
    if value.len() > 12288 {
        return Err(Status::invalid_argument("INVALID_INPUT"));
    }
    let parsed: serde_json::Value =
        serde_json::from_str(value).map_err(|_| Status::invalid_argument("INVALID_INPUT"))?;
    for field in ["attemptId", "policyRevision", "idempotencyKey"] {
        if let Some(value) = parsed.get(field) {
            let text = value
                .as_str()
                .ok_or_else(|| Status::invalid_argument("INVALID_INPUT"))?;
            let id =
                Uuid::parse_str(text).map_err(|_| Status::invalid_argument("INVALID_INPUT"))?;
            if id.is_nil() || id.to_string() != text {
                return Err(Status::invalid_argument("INVALID_INPUT"));
            }
        }
    }
    serde_json::from_value(parsed).map_err(|_| Status::invalid_argument("INVALID_INPUT"))
}
fn status(error: EnrollmentError) -> Status {
    let reason = error.to_string();
    match error {
        EnrollmentError::PolicyUnavailable | EnrollmentError::Unavailable => {
            Status::unavailable(reason)
        }
        EnrollmentError::PolicyStale | EnrollmentError::IdempotencyConflict => {
            Status::already_exists(reason)
        }
        EnrollmentError::AttemptExpired => Status::out_of_range(reason),
        EnrollmentError::AttemptInvalid => Status::not_found(reason),
        EnrollmentError::AccountInactive => Status::permission_denied(reason),
        _ => Status::invalid_argument(reason),
    }
}
async fn reply<T: Serialize>(
    work: impl Future<Output = Result<T, EnrollmentError>>,
) -> Result<Response<proto::EnrollmentReply>, Status> {
    let ceiling = std::time::Instant::now() + platform::edge::Edge::GatewayAuth.budget().deadline;
    let stop = platform::call_deadline::current().map_or(ceiling, |d| d.min(ceiling));
    let value = tokio::time::timeout_at(stop.into(), work)
        .await
        .map_err(|_| Status::deadline_exceeded("DEADLINE_EXCEEDED"))?
        .map_err(status)?;
    if std::time::Instant::now() >= stop {
        return Err(Status::deadline_exceeded("DEADLINE_EXCEEDED"));
    }
    let mut response = Response::new(proto::EnrollmentReply {
        body_json: serde_json::to_string(&value).map_err(|_| Status::internal("INTERNAL_ERROR"))?,
    });
    response
        .metadata_mut()
        .insert("cache-control", "no-store".parse().expect("static"));
    Ok(response)
}
#[tonic::async_trait]
impl EnrollmentPolicyService for EnrollmentServer {
    async fn read_policy(
        &self,
        request: Request<proto::ReadPolicyRequest>,
    ) -> Result<Response<proto::EnrollmentReply>, Status> {
        caller(&request)?;
        reply(self.store.current_policy()).await
    }
    async fn begin_attempt(
        &self,
        request: Request<proto::AnonymousCommand>,
    ) -> Result<Response<proto::EnrollmentReply>, Status> {
        caller(&request)?;
        let input = decode(&request.get_ref().payload_json)?;
        reply(self.store.begin(&request.get_ref().client_binding, input)).await
    }
    async fn signup(
        &self,
        request: Request<proto::AnonymousCommand>,
    ) -> Result<Response<proto::EnrollmentReply>, Status> {
        caller(&request)?;
        let input = decode(&request.get_ref().payload_json)?;
        reply(
            self.store
                .signup(&self.signup, &request.get_ref().client_binding, input),
        )
        .await
    }
    async fn verify(
        &self,
        request: Request<proto::AnonymousCommand>,
    ) -> Result<Response<proto::EnrollmentReply>, Status> {
        caller(&request)?;
        let input = decode(&request.get_ref().payload_json)?;
        reply(self.store.verify(&request.get_ref().client_binding, input)).await
    }
    async fn resend(
        &self,
        request: Request<proto::AnonymousCommand>,
    ) -> Result<Response<proto::EnrollmentReply>, Status> {
        caller(&request)?;
        let input = decode(&request.get_ref().payload_json)?;
        reply(self.store.resend(&request.get_ref().client_binding, input)).await
    }
    async fn read_admission(
        &self,
        request: Request<proto::ActorCommand>,
    ) -> Result<Response<proto::EnrollmentReply>, Status> {
        let user = actor(&request)?;
        if !request.get_ref().payload_json.is_empty() {
            return Err(Status::invalid_argument("INVALID_INPUT"));
        }
        reply(self.store.admission(user)).await
    }
    async fn accept_consent(
        &self,
        request: Request<proto::ActorCommand>,
    ) -> Result<Response<proto::EnrollmentReply>, Status> {
        let user = actor(&request)?;
        let input = decode(&request.get_ref().payload_json)?;
        reply(self.store.consent(user, input)).await
    }
}
