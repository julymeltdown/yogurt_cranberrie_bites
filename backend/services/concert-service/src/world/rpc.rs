//! Internal world wire plus default-deny Gateway workload/actor policy.
use platform::actor_delegation::{
    authority_deadline::AuthorityRpcDeadline, read_authority::VerifiedReadAuthority, VerifiedActor,
};
use platform::grpc_auth::{VerifiedCaller, Workload};
use platform::grpc_method_policy::MethodPolicy;
use std::time::Duration;
use tonic::{Request, Status};
use uuid::Uuid;

pub mod proto {
    tonic::include_proto!("concert.world.v1");
}

pub const POLICY: MethodPolicy = MethodPolicy {
    service: "concert.world.v1.WorldService",
    workload: Some(Workload::Concert),
    methods: &[
        ("ListRooms", &[Workload::Gateway]),
        ("IssueTicket", &[Workload::Gateway]),
        ("CurrentMembership", &[Workload::Gateway]),
    ],
    actor_fields: &[
        ("ListRooms", &[&[1]]),
        ("IssueTicket", &[&[1]]),
        ("CurrentMembership", &[&[1]]),
    ],
};
pub const WORLD_BUDGET: Duration = Duration::from_secs(2);

#[derive(Clone)]
pub struct VerifiedWorld {
    actor: Uuid,
    event: Uuid,
    session_family: Uuid,
    request_id: Uuid,
    http_method: String,
    http_target: String,
    http_body: Vec<u8>,
    access_expires_unix_micros: i64,
    deadline: AuthorityRpcDeadline,
}
impl std::fmt::Debug for VerifiedWorld {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("VerifiedWorld([REDACTED])")
    }
}
impl VerifiedWorld {
    pub fn actor(&self) -> Uuid {
        self.actor
    }
    pub fn event_id(&self) -> Uuid {
        self.event
    }
    pub fn session_family_id(&self) -> Uuid {
        self.session_family
    }
    pub fn request_id(&self) -> Uuid {
        self.request_id
    }
    pub fn http_method(&self) -> &str {
        &self.http_method
    }
    pub fn http_target(&self) -> &str {
        &self.http_target
    }
    pub fn http_body(&self) -> &[u8] {
        &self.http_body
    }
    pub fn access_expires_unix_micros(&self) -> i64 {
        self.access_expires_unix_micros
    }
    pub fn deadline(&self) -> AuthorityRpcDeadline {
        self.deadline
    }
    pub fn check(&self) -> Result<(), Status> {
        self.deadline
            .remaining(WORLD_BUDGET, self.deadline.expires_micros())
            .map(|_| ())
    }
}

fn uuid(value: &str, label: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(|| Status::invalid_argument(format!("Invalid {label}")))
}

fn deadline(
    proof: VerifiedReadAuthority,
    expires: i64,
    observed: [i64; 2],
) -> Result<AuthorityRpcDeadline, Status> {
    let now = chrono::Utc::now().timestamp_micros();
    if expires <= 0
        || observed.iter().any(|at| {
            *at <= 0
                || *at > now
                || expires <= *at
                || expires.saturating_sub(*at) > WORLD_BUDGET.as_micros() as i64
        })
    {
        return Err(Status::permission_denied(
            "Invalid world observation bounds",
        ));
    }
    let original = proof.deadline(WORLD_BUDGET)?;
    if expires > original.expires_micros() {
        return Err(Status::permission_denied("World authority exceeds request"));
    }
    AuthorityRpcDeadline::within_original(expires, original.stop().into_std(), WORLD_BUDGET)
}

fn common<T>(request: &Request<T>) -> Result<(VerifiedReadAuthority, Uuid), Status> {
    if !matches!(
        request.extensions().get::<VerifiedCaller>(),
        Some(VerifiedCaller(Workload::Gateway))
    ) {
        return Err(Status::unauthenticated("Verified Gateway required"));
    }
    Ok((
        *request
            .extensions()
            .get::<VerifiedReadAuthority>()
            .ok_or_else(|| Status::unauthenticated("Bounded world authority required"))?,
        request
            .extensions()
            .get::<VerifiedActor>()
            .ok_or_else(|| Status::unauthenticated("Verified world actor required"))?
            .0,
    ))
}

pub fn authorize_world(
    request: &Request<proto::WorldRequest>,
    method: &str,
) -> Result<VerifiedWorld, Status> {
    let (proof, actor) = common(request)?;
    let message = request.get_ref();
    let user = uuid(&message.user_id, "world actor")?;
    let event = uuid(&message.event_id, "world event")?;
    let family = uuid(&message.session_family_id, "world session")?;
    let request_id = uuid(&message.request_id, "world request")?;
    if proof.actor() != actor || actor != user {
        return Err(Status::permission_denied("World actor mismatch"));
    }
    let target = match method {
        "ListRooms" if message.http_method == "GET" && message.http_body.is_empty() => {
            format!("/api/concert/events/{event}/rooms")
        }
        "IssueTicket" if message.http_method == "POST" => {
            format!("/api/concert/events/{event}/world-tickets")
        }
        _ => return Err(Status::permission_denied("World target mismatch")),
    };
    if message.http_target != target {
        return Err(Status::permission_denied("World target mismatch"));
    }
    let deadline = deadline(
        proof,
        message.authority_expires_unix_micros,
        [
            message.auth_observed_at_unix_micros,
            message.profile_observed_at_unix_micros,
        ],
    )?;
    if message.access_expires_unix_micros <= chrono::Utc::now().timestamp_micros()
        || message.authority_expires_unix_micros > message.access_expires_unix_micros
    {
        return Err(Status::permission_denied("Invalid world access expiry"));
    }
    let verified = VerifiedWorld {
        actor,
        event,
        session_family: family,
        request_id,
        http_method: message.http_method.clone(),
        http_target: message.http_target.clone(),
        http_body: message.http_body.clone(),
        access_expires_unix_micros: message.access_expires_unix_micros,
        deadline,
    };
    verified.check()?;
    Ok(verified)
}

pub fn authorize_current_membership(
    request: &Request<proto::CurrentMembershipRequest>,
) -> Result<VerifiedWorld, Status> {
    let (proof, actor) = common(request)?;
    let message = request.get_ref();
    let user = uuid(&message.user_id, "world actor")?;
    let event = uuid(&message.event_id, "world event")?;
    let family = uuid(&message.session_family_id, "world session")?;
    let request_id = uuid(&message.request_id, "world request")?;
    if proof.actor() != actor || actor != user {
        return Err(Status::permission_denied("World actor mismatch"));
    }
    let deadline = deadline(
        proof,
        message.authority_expires_unix_micros,
        [
            message.auth_observed_at_unix_micros,
            message.profile_observed_at_unix_micros,
        ],
    )?;
    if message.access_expires_unix_micros <= chrono::Utc::now().timestamp_micros()
        || message.authority_expires_unix_micros > message.access_expires_unix_micros
    {
        return Err(Status::permission_denied("Invalid world access expiry"));
    }
    let verified = VerifiedWorld {
        actor,
        event,
        session_family: family,
        request_id,
        http_method: String::new(),
        http_target: String::new(),
        http_body: Vec::new(),
        access_expires_unix_micros: message.access_expires_unix_micros,
        deadline,
    };
    verified.check()?;
    Ok(verified)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn policy_is_gateway_only_and_actor_bound() {
        for method in ["ListRooms", "IssueTicket", "CurrentMembership"] {
            let path = format!("/concert.world.v1.WorldService/{method}");
            for caller in Workload::ALL {
                assert_eq!(POLICY.allows(&path, caller), caller == Workload::Gateway);
            }
            assert_eq!(POLICY.actor_fields_for(&path), Some(&[&[1][..]][..]));
        }
        assert!(!POLICY.allows("/concert.world.v1.WorldService/Unknown", Workload::Gateway));
    }
}
