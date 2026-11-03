//! Current self-only Profile content. It is not identity or Concert admission.
use super::{proto, GrpcProfileClient};
use crate::adapters::out::jwt::concert_access::ConcertAccessContext;
use chrono::{DateTime, Datelike, Utc};
use platform::{actor_delegation::request_for, edge::Edge};
use std::fmt;
use tonic::{Code, Status};
use uuid::Uuid;

/// Parse/decode bound, not a content freshness or cache lifetime.
pub const MAX_CONCERT_PROFILE_RESPONSE_BYTES: usize = 1024;

/// Safe, stable failures; peer messages and details are never propagated.
#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum ConcertProfileReadError {
    #[error("Concert Profile access context expired or invalid")]
    AccessExpired,
    #[error("Concert Profile content unavailable")]
    NotFound,
    #[error("Concert Profile read denied")]
    Denied,
    #[error("Concert Profile read deadline exceeded")]
    DeadlineExceeded,
    #[error("Concert Profile capacity unavailable")]
    CapacityExhausted,
    #[error("Concert Profile owner unavailable")]
    Unavailable,
    #[error("Invalid Concert Profile owner response")]
    InvalidResponse,
}

/// Borrowed, request-bound current content. No constructor, Clone, conversion
/// to a broader Profile DTO, or serialization is exposed.
/// ```compile_fail
/// use api_gateway::adapters::out::profile_client::concert_identity::ConcertProfileObservation;
/// let _ = ConcertProfileObservation {};
/// ```
/// ```compile_fail
/// use api_gateway::adapters::out::profile_client::concert_identity::ConcertProfileObservation;
/// fn export(value: &ConcertProfileObservation<'_>) { let _ = serde_json::to_string(value); }
/// ```
/// ```compile_fail
/// use api_gateway::adapters::out::profile_client::concert_identity::ConcertProfileObservation;
/// let _ = serde_json::from_str::<ConcertProfileObservation<'_>>("{}");
/// ```
/// ```compile_fail
/// use api_gateway::adapters::out::profile_client::concert_identity::ConcertProfileObservation;
/// fn copy_owned<'a>(value: &ConcertProfileObservation<'a>) -> ConcertProfileObservation<'a> { (*value).clone() }
/// ```
pub struct ConcertProfileObservation<'a> {
    context: &'a ConcertAccessContext,
    user_id: Uuid,
    display_name: String,
    bio: Option<String>,
    observed_at: DateTime<Utc>,
}
impl fmt::Debug for ConcertProfileObservation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ConcertProfileObservation([REDACTED])")
    }
}
impl ConcertProfileObservation<'_> {
    fn check(&self) -> Result<(), ConcertProfileReadError> {
        self.context
            .check()
            .map_err(|_| ConcertProfileReadError::AccessExpired)
    }
    pub fn user_id(&self) -> Result<Uuid, ConcertProfileReadError> {
        self.check()?;
        Ok(self.user_id)
    }
    pub fn display_name(&self) -> Result<&str, ConcertProfileReadError> {
        self.check()?;
        Ok(&self.display_name)
    }
    pub fn bio(&self) -> Result<Option<&str>, ConcertProfileReadError> {
        self.check()?;
        Ok(self.bio.as_deref())
    }
    /// Statement observation only; it certifies neither a revision nor a TTL.
    pub fn observed_at(&self) -> Result<DateTime<Utc>, ConcertProfileReadError> {
        self.check()?;
        Ok(self.observed_at)
    }
}

impl GrpcProfileClient {
    pub async fn read_concert_self_profile<'a>(
        &self,
        context: &'a ConcertAccessContext,
    ) -> Result<ConcertProfileObservation<'a>, ConcertProfileReadError> {
        let access = || {
            context
                .check()
                .map_err(|_| ConcertProfileReadError::AccessExpired)
        };
        access()?;
        let deadline = context
            .rpc_deadline()
            .map_err(|_| ConcertProfileReadError::AccessExpired)?;
        let ceiling = platform::grpc_deadline::local_deadline(Edge::GatewayProfile);
        let remaining = context
            .remaining(ceiling)
            .map_err(|_| ConcertProfileReadError::AccessExpired)?
            .min(
                deadline
                    .remaining(ceiling, context.access_expires_micros())
                    .map_err(|_| ConcertProfileReadError::DeadlineExceeded)?,
            );
        let mut stop = deadline.stop().min(tokio::time::Instant::now() + remaining);
        if let Some(outer) = platform::call_deadline::current() {
            stop = stop.min(outer.into());
        }
        let actor = context
            .user_id()
            .map_err(|_| ConcertProfileReadError::AccessExpired)?;
        let message = proto::ReadConcertSelfProfileRequest {
            user_id: actor.to_string(),
        };
        let mut request = request_for(message, &actor.to_string())
            .map_err(|_| ConcertProfileReadError::Denied)?;
        let timeout = stop.saturating_duration_since(tokio::time::Instant::now());
        if timeout.is_zero() {
            return Err(ConcertProfileReadError::DeadlineExceeded);
        }
        request.set_timeout(timeout);
        let mut client = self
            .client
            .clone()
            .max_decoding_message_size(MAX_CONCERT_PROFILE_RESPONSE_BYTES);
        access()?;
        let result = platform::call_deadline::scope(stop.into_std(), async {
            let remaining = stop.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                self.metrics
                    .edge_deadline_metrics()
                    .record_deadline_exhausted(Edge::GatewayProfile);
                return Err(Status::deadline_exceeded("Request timed out"));
            }
            platform::grpc_deadline::with_deadline(
                remaining,
                Edge::GatewayProfile,
                self.metrics.edge_deadline_metrics(),
                async {
                    match tokio::time::timeout_at(stop, client.read_concert_self_profile(request))
                        .await
                    {
                        Ok(result) if tokio::time::Instant::now() < stop => result,
                        _ => Err(Status::deadline_exceeded("Request timed out")),
                    }
                },
            )
            .await
        })
        .await;
        access()?;
        if tokio::time::Instant::now() >= stop {
            return Err(ConcertProfileReadError::DeadlineExceeded);
        }
        let response = result
            .map_err(|status| match status.code() {
                Code::NotFound => ConcertProfileReadError::NotFound,
                Code::DeadlineExceeded => ConcertProfileReadError::DeadlineExceeded,
                Code::Unauthenticated | Code::PermissionDenied => ConcertProfileReadError::Denied,
                Code::ResourceExhausted => ConcertProfileReadError::CapacityExhausted,
                Code::DataLoss | Code::OutOfRange => ConcertProfileReadError::InvalidResponse,
                _ => ConcertProfileReadError::Unavailable,
            })?
            .into_inner();
        let profile = response
            .profile
            .ok_or(ConcertProfileReadError::InvalidResponse)?;
        let user_id =
            canonical_uuid(&profile.user_id).ok_or(ConcertProfileReadError::InvalidResponse)?;
        if user_id != actor {
            return Err(ConcertProfileReadError::InvalidResponse);
        }
        if !valid_display_name(&profile.display_name) {
            return Err(ConcertProfileReadError::InvalidResponse);
        }
        let bio = match profile.bio {
            None => None,
            Some(value) if value.is_empty() => None,
            Some(value) if valid_bio(&value) => Some(value),
            Some(_) => return Err(ConcertProfileReadError::InvalidResponse),
        };
        let observed_at = DateTime::from_timestamp_micros(profile.observed_at_unix_micros)
            .filter(|value| (1..=9999).contains(&value.year()))
            .ok_or(ConcertProfileReadError::InvalidResponse)?;
        access()?;
        let observation = ConcertProfileObservation {
            context,
            user_id,
            display_name: profile.display_name,
            bio,
            observed_at,
        };
        observation.check()?;
        if tokio::time::Instant::now() >= stop {
            return Err(ConcertProfileReadError::DeadlineExceeded);
        }
        Ok(observation)
    }
}

fn canonical_uuid(value: &str) -> Option<Uuid> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
}
fn valid_display_name(value: &str) -> bool {
    let count = value.chars().count();
    !value.trim().is_empty() && count <= 40 && !value.chars().any(char::is_control)
}
fn valid_bio(value: &str) -> bool {
    value.chars().count() <= 160 && !value.chars().any(char::is_control)
}
