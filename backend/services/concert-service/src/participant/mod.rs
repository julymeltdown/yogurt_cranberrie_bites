//! Isolated Phase A producer. Not registered by product startup or Gateway.
pub mod context;
pub mod ports;
use concert_application::avatar::{AvatarAuthority, AvatarError, AvatarGrant, AvatarOperation};
use context::VerifiedParticipantRequest;
use platform::{grpc_auth::Workload, grpc_method_policy::MethodPolicy};
use ports::*;
use serde_json::{json, Value};
use std::sync::{Arc, OnceLock};
use tonic::{Request, Response, Status};
pub mod proto {
    tonic::include_proto!("concert.participant.v1");
}
pub const POLICY: MethodPolicy = MethodPolicy {
    service: "concert.participant.v1.ConcertParticipantService",
    workload: Some(Workload::Concert),
    methods: &[
        ("GetParticipant", &[Workload::Gateway]),
        ("SetAvatar", &[Workload::Gateway]),
        ("UpdatePreferences", &[Workload::Gateway]),
    ],
    actor_fields: &[
        ("GetParticipant", &[&[1]]),
        ("SetAvatar", &[&[1]]),
        ("UpdatePreferences", &[&[1]]),
    ],
};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Operation {
    GetParticipant,
    SetAvatar,
    UpdatePreferences,
}
impl Operation {
    pub fn avatar_operation(self) -> AvatarOperation {
        match self {
            Self::GetParticipant => AvatarOperation::Read,
            Self::SetAvatar => AvatarOperation::SetAvatar,
            Self::UpdatePreferences => AvatarOperation::SetPreferences,
        }
    }
}
#[derive(Clone)]
pub struct ParticipantService {
    store: Arc<dyn ParticipantStore>,
    auth: Arc<dyn CanonicalAuthPort>,
    profile: Arc<dyn CanonicalProfilePort>,
    mutations: Arc<dyn MutationAuthorityPort>,
}
impl Default for ParticipantService {
    fn default() -> Self {
        Self::new(
            Arc::new(Unavailable),
            Arc::new(Unavailable),
            Arc::new(Unavailable),
            Arc::new(Unavailable),
        )
    }
}
impl ParticipantService {
    pub fn new(
        store: Arc<dyn ParticipantStore>,
        auth: Arc<dyn CanonicalAuthPort>,
        profile: Arc<dyn CanonicalProfilePort>,
        mutations: Arc<dyn MutationAuthorityPort>,
    ) -> Self {
        Self {
            store,
            auth,
            profile,
            mutations,
        }
    }
    /// Product startup must register this ONLY through platform's workload and
    /// signed actor guard with POLICY. Direct handler calls lack sealed context.
    pub fn server(
        self,
    ) -> proto::concert_participant_service_server::ConcertParticipantServiceServer<Self> {
        proto::concert_participant_service_server::ConcertParticipantServiceServer::new(self)
            .max_decoding_message_size(16 * 1024)
            .max_encoding_message_size(16 * 1024)
    }
    async fn handle(
        &self,
        request: Request<proto::ParticipantRequest>,
        operation: Operation,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        let context = VerifiedParticipantRequest::verify(&request, operation)?;
        let stop = context.deadline().stop();
        let result = platform::call_deadline::scope(
            stop.into_std(),
            tokio::time::timeout_at(stop, self.produce(&context)),
        )
        .await;
        context.check()?;
        let document =
            result.map_err(|_| Status::deadline_exceeded("Participant deadline elapsed"))??;
        let bytes = serde_json::to_vec(&document).map_err(|_| status(AvatarError::InvalidState))?;
        context.check()?;
        let mut response = Response::new(proto::ParticipantResponse {
            response_json: bytes,
        });
        response
            .metadata_mut()
            .insert("cache-control", "no-store".parse().unwrap());
        Ok(response)
    }
    async fn produce(&self, context: &VerifiedParticipantRequest) -> Result<Value, Status> {
        context.check()?;
        if context.release_digest() != self.store.release_digest() {
            return Err(status(AvatarError::Unavailable));
        }
        let auth = self.auth.read(context).await.map_err(status)?;
        context.check()?;
        auth.check(context).map_err(status)?;
        // Auth is the negative-fact gate. No Profile call follows an Auth refusal.
        let profile = self.profile.read(context).await.map_err(status)?;
        context.check()?;
        auth.check(context).map_err(status)?;
        profile.check(context).map_err(status)?;
        let permit = if context.operation() != Operation::GetParticipant {
            let permit = self
                .mutations
                .authorize(context, &auth)
                .await
                .map_err(status)?;
            context.check()?;
            auth.check(context).map_err(status)?;
            profile.check(context).map_err(status)?;
            permit.check(context).map_err(status)?;
            Some(permit)
        } else {
            None
        };
        let authority = CheckedAuthority {
            context,
            auth: &auth,
            profile: &profile,
            permit: permit.as_deref(),
        };
        authority.check().map_err(status)?;
        let result = self
            .store
            .execute(
                context.key(),
                context.operation(),
                context.raw_body(),
                &authority,
            )
            .await
            .map_err(status)?;
        authority.check().map_err(status)?;
        let document = match (context.operation(), result) {
            (Operation::SetAvatar, StoreResult::Avatar(value)) => {
                let command = concert_domain::avatar::SetAvatar::parse(context.raw_body())
                    .map_err(|_| status(AvatarError::InvalidInput))?;
                if value["config"] != concert_domain::avatar::config_value(&command.selection)
                    || value["revision"].as_i64() != command.expected.next().ok().map(|r| r.get())
                {
                    return Err(status(AvatarError::InvalidState));
                }
                value
            }
            (
                Operation::GetParticipant | Operation::UpdatePreferences,
                StoreResult::Snapshot(value),
            ) => {
                if value.avatar.key != context.key() || value.participant.key != context.key() {
                    return Err(status(AvatarError::InvalidState));
                }
                if context.operation() == Operation::UpdatePreferences {
                    let (expected, preferences) =
                        crate::adapters::out::repository::avatar::parse_preferences(
                            context.raw_body(),
                        )
                        .map_err(status)?;
                    if Some(value.participant.revision) != expected.next().ok()
                        || value.participant.preferences != preferences
                    {
                        return Err(status(AvatarError::InvalidState));
                    }
                }
                let p = value.participant;
                let avatar = crate::adapters::out::repository::avatar::avatar_state(&value.avatar);
                json!({"event_id":p.key.event_id.uuid().to_string(),"user_id":p.key.user_id.uuid().to_string(),"revision":p.revision.get(),
                    "profile":{"user_id":profile.user_id.to_string(),"display_name":profile.display_name,"bio":profile.bio,"avatar":avatar},
                    "preferences":{"theme":p.preferences.theme.as_str(),"low_power":p.preferences.low_power,"reduced_motion":p.preferences.reduced_motion,"sound_enabled":p.preferences.sound_enabled},"age_status":"UNKNOWN"})
            }
            _ => return Err(status(AvatarError::InvalidState)),
        };
        validate_response(&document, context.operation())?;
        authority.check().map_err(status)?;
        Ok(document)
    }
}
struct CheckedAuthority<'a> {
    context: &'a VerifiedParticipantRequest,
    auth: &'a CurrentAuth,
    profile: &'a CurrentProfile,
    permit: Option<&'a dyn MutationPermit>,
}
impl AvatarAuthority for CheckedAuthority<'_> {
    fn check(&self) -> Result<AvatarGrant, AvatarError> {
        self.context
            .check()
            .map_err(|_| AvatarError::Unauthenticated)?;
        self.auth.check(self.context)?;
        self.profile.check(self.context)?;
        match (self.context.operation(), self.permit) {
            (Operation::GetParticipant, None) => {}
            (_, Some(permit)) => permit.check(self.context)?,
            _ => return Err(AvatarError::Unavailable),
        }
        Ok(AvatarGrant {
            key: self.context.key(),
            operation: self.context.operation().avatar_operation(),
            release_digest: self.context.release_digest().into(),
        })
    }
}
#[tonic::async_trait]
impl proto::concert_participant_service_server::ConcertParticipantService for ParticipantService {
    async fn get_participant(
        &self,
        r: Request<proto::ParticipantRequest>,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        self.handle(r, Operation::GetParticipant)
            .await
            .map_err(normalize_status)
    }
    async fn set_avatar(
        &self,
        r: Request<proto::ParticipantRequest>,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        self.handle(r, Operation::SetAvatar)
            .await
            .map_err(normalize_status)
    }
    async fn update_preferences(
        &self,
        r: Request<proto::ParticipantRequest>,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        self.handle(r, Operation::UpdatePreferences)
            .await
            .map_err(normalize_status)
    }
}
fn validate_response(value: &Value, operation: Operation) -> Result<(), Status> {
    static SCHEMAS: OnceLock<Option<(jsonschema::Validator, jsonschema::Validator)>> =
        OnceLock::new();
    let schemas=SCHEMAS.get_or_init(||{
        let source:Value=serde_json::from_str(include_str!(concat!(env!("CARGO_MANIFEST_DIR"),"/../../docs/api/mcr-core.openapi.json"))).ok()?;
        let compile=|name|jsonschema::options().should_validate_formats(true).build(&json!({"$schema":"https://json-schema.org/draft/2020-12/schema","$ref":format!("#/components/schemas/{name}"),"components":source["components"]})).ok();
        Some((compile("PrivateParticipant")?,compile("AvatarState")?))
    }).as_ref().ok_or_else(||status(AvatarError::Unavailable))?;
    let schema = if operation == Operation::SetAvatar {
        &schemas.1
    } else {
        &schemas.0
    };
    if !schema.is_valid(value) {
        return Err(status(AvatarError::InvalidState));
    }
    Ok(())
}
fn status(error: AvatarError) -> Status {
    let (mut value, reason) = match error {
        AvatarError::InvalidInput => (
            Status::invalid_argument("Invalid participant input"),
            "INVALID_INPUT",
        ),
        AvatarError::Unauthenticated => (
            Status::unauthenticated("Current authentication required"),
            "AUTHENTICATION_REQUIRED",
        ),
        AvatarError::Denied => (
            Status::permission_denied("Participant operation denied"),
            "PERMISSION_DENIED",
        ),
        AvatarError::Missing => (
            Status::not_found("Participant unavailable"),
            "RESOURCE_UNAVAILABLE",
        ),
        AvatarError::Conflict => (
            Status::aborted("Participant revision changed"),
            "VERSION_CONFLICT",
        ),
        AvatarError::ReadOnly => (
            Status::failed_precondition("Event is read only"),
            "EVENT_READ_ONLY",
        ),
        AvatarError::Unavailable => (
            Status::unavailable("Participant dependency unavailable"),
            "DEPENDENCY_UNAVAILABLE",
        ),
        AvatarError::InvalidState => (
            Status::internal("Participant response unavailable"),
            "INTERNAL_ERROR",
        ),
    };
    value
        .metadata_mut()
        .insert("x-mcr-core-reason", reason.parse().unwrap());
    value
}

fn normalize_status(mut value: Status) -> Status {
    if !value.metadata().contains_key("x-mcr-core-reason") {
        let reason = match value.code() {
            tonic::Code::Unauthenticated => "AUTHENTICATION_REQUIRED",
            tonic::Code::PermissionDenied => "PERMISSION_DENIED",
            tonic::Code::InvalidArgument => "INVALID_INPUT",
            tonic::Code::DeadlineExceeded => "DEADLINE_EXCEEDED",
            tonic::Code::Unavailable => "DEPENDENCY_UNAVAILABLE",
            _ => "INTERNAL_ERROR",
        };
        value
            .metadata_mut()
            .insert("x-mcr-core-reason", reason.parse().unwrap());
    }
    value
}
