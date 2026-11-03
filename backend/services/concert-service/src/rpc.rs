//! Shared internal Concert wire and default-deny workload/actor policy.
use platform::actor_delegation::{
    authority_deadline::AuthorityRpcDeadline, read_authority::VerifiedReadAuthority, VerifiedActor,
};
use platform::grpc_auth::VerifiedCaller;
use platform::grpc_auth::Workload;
use platform::grpc_method_policy::MethodPolicy;
use std::time::Duration;
use tonic::{Request, Status};
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("concert.v1");
}

pub const POLICY: MethodPolicy = MethodPolicy {
    service: "concert.v1.ConcertService",
    workload: Some(Workload::Concert),
    methods: &[("Bootstrap", &[Workload::Gateway])],
    actor_fields: &[("Bootstrap", &[&[1]])],
};

/// Processing/observation-age ceiling, not a claim of an instantaneous
/// revocation protocol. Every request also retains its earlier absolute stops.
pub const BOOTSTRAP_BUDGET: Duration = Duration::from_secs(2);

/// Constructible only after the shared workload/signature/actor guard. This
/// carries the Gateway's bounded current-owner assertion, not an event grant.
pub struct VerifiedBootstrap {
    user_id: Uuid,
    event_id: Uuid,
    session_family_id: Uuid,
    request_id: Uuid,
    display_name: String,
    bio: Option<String>,
    deadline: AuthorityRpcDeadline,
}
impl std::fmt::Debug for VerifiedBootstrap {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VerifiedBootstrap([REDACTED])")
    }
}
impl VerifiedBootstrap {
    pub fn user_id(&self) -> Uuid {
        self.user_id
    }
    pub fn event_id(&self) -> Uuid {
        self.event_id
    }
    pub fn session_family_id(&self) -> Uuid {
        self.session_family_id
    }
    pub fn request_id(&self) -> Uuid {
        self.request_id
    }
    pub fn display_name(&self) -> &str {
        &self.display_name
    }
    pub fn bio(&self) -> Option<&str> {
        self.bio.as_deref()
    }
    pub fn deadline(&self) -> AuthorityRpcDeadline {
        self.deadline
    }
    /// Call after every await and immediately before commit/response. SQL must
    /// independently check current event phase/restriction under transaction locks.
    pub fn check(&self) -> Result<(), Status> {
        self.deadline
            .remaining(BOOTSTRAP_BUDGET, self.deadline.expires_micros())
            .map(|_| ())
    }
}

pub fn authorize_bootstrap(
    request: &Request<proto::BootstrapRequest>,
) -> Result<VerifiedBootstrap, Status> {
    if !matches!(
        request.extensions().get::<VerifiedCaller>(),
        Some(VerifiedCaller(Workload::Gateway))
    ) {
        return Err(Status::unauthenticated("Verified Gateway required"));
    }
    let actor = request
        .extensions()
        .get::<VerifiedActor>()
        .ok_or_else(|| Status::unauthenticated("Verified bootstrap actor required"))?
        .0;
    let proof = request
        .extensions()
        .get::<VerifiedReadAuthority>()
        .ok_or_else(|| Status::unauthenticated("Bounded bootstrap authority required"))?;
    let message = request.get_ref();
    let uuid = |value: &str| {
        Uuid::parse_str(value)
            .ok()
            .filter(|id| !id.is_nil() && id.to_string() == value)
            .ok_or_else(|| Status::invalid_argument("Invalid bootstrap identity"))
    };
    let user_id = uuid(&message.user_id)?;
    let event_id = uuid(&message.event_id)?;
    let session_family_id = uuid(&message.session_family_id)?;
    let request_id = uuid(&message.request_id)?;
    if actor != user_id || proof.actor() != user_id {
        return Err(Status::permission_denied("Bootstrap actor mismatch"));
    }
    if message.http_method != "GET"
        || message.http_target != format!("/api/concert/events/{event_id}/bootstrap")
    {
        return Err(Status::permission_denied("Bootstrap target mismatch"));
    }
    if message.display_name.trim().is_empty()
        || message.display_name.chars().count() > 40
        || message.display_name.chars().any(char::is_control)
        || message
            .bio
            .as_ref()
            .is_some_and(|v| v.chars().count() > 160 || v.chars().any(char::is_control))
    {
        return Err(Status::invalid_argument(
            "Invalid public profile projection",
        ));
    }
    let now = chrono::Utc::now().timestamp_micros();
    let expires = message.authority_expires_unix_micros;
    for observed in [
        message.auth_observed_at_unix_micros,
        message.profile_observed_at_unix_micros,
    ] {
        if observed <= 0
            || observed > now
            || expires <= observed
            || expires.saturating_sub(observed) > BOOTSTRAP_BUDGET.as_micros() as i64
        {
            return Err(Status::permission_denied(
                "Invalid bootstrap observation bounds",
            ));
        }
    }
    let original = proof.deadline(BOOTSTRAP_BUDGET)?;
    let deadline = AuthorityRpcDeadline::within_original(
        expires.min(original.expires_micros()),
        original.stop().into_std(),
        BOOTSTRAP_BUDGET,
    )?;
    let value = VerifiedBootstrap {
        user_id,
        event_id,
        session_family_id,
        request_id,
        display_name: message.display_name.clone(),
        bio: message.bio.clone(),
        deadline,
    };
    value.check()?;
    Ok(value)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bootstrap_requires_gateway_and_the_signed_actor() {
        let path = "/concert.v1.ConcertService/Bootstrap";
        for caller in Workload::ALL {
            assert_eq!(POLICY.allows(path, caller), caller == Workload::Gateway);
        }
        assert_eq!(POLICY.actor_fields_for(path), Some(&[&[1][..]][..]));
        assert!(!POLICY.allows("/concert.v1.ConcertService/Unknown", Workload::Gateway));
        let methods = platform::grpc_method_policy::proto_rpc_names(include_str!(
            "../../../proto/concert/v1/concert.proto"
        ));
        assert_eq!(methods, ["Bootstrap"]);
    }
}
