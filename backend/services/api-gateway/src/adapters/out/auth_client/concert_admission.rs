//! Uncached signed Auth lookup. These facts alone never grant Concert admission.
use super::{proto, GrpcAuthClient};
use crate::adapters::out::jwt::concert_access::ConcertAccessContext;
use chrono::{DateTime, Utc};
use platform::{actor_delegation::request_for, edge::Edge};
use std::fmt;
use tonic::{Code, Status};
use uuid::Uuid;

/// Parse/decode guard, not an observation lifetime or revocation SLO.
pub const MAX_ADMISSION_RESPONSE_BYTES: usize = 128;
pub const MAX_BINDING_EPOCH: i64 = 9_007_199_254_740_991;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingProvider {
    Email,
    Google,
    Apple,
    X,
}
impl BindingProvider {
    pub fn parse(value: &str) -> Result<Self, AdmissionReadError> {
        match value {
            "EMAIL" => Ok(Self::Email),
            "GOOGLE" => Ok(Self::Google),
            "APPLE" => Ok(Self::Apple),
            "X" => Ok(Self::X),
            _ => Err(AdmissionReadError::InvalidBinding),
        }
    }
    fn owner_name(self) -> &'static str {
        match self {
            Self::Email => "EMAIL",
            Self::Google => "GOOGLE",
            Self::Apple => "APPLE",
            Self::X => "X",
        }
    }
}

/// Shape only: Auth must establish actual ownership and current generation.
#[derive(Clone, Copy)]
pub struct BindingReference {
    identity_id: Uuid,
    provider: BindingProvider,
    epoch: i64,
}
impl BindingReference {
    pub fn new(
        identity_id: &str,
        provider: BindingProvider,
        epoch: i64,
    ) -> Result<Self, AdmissionReadError> {
        let id = Uuid::parse_str(identity_id).map_err(|_| AdmissionReadError::InvalidBinding)?;
        if id.is_nil() || id.to_string() != identity_id || !(1..=MAX_BINDING_EPOCH).contains(&epoch)
        {
            return Err(AdmissionReadError::InvalidBinding);
        }
        Ok(Self {
            identity_id: id,
            provider,
            epoch,
        })
    }
}
impl fmt::Debug for BindingReference {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("BindingReference([REDACTED])")
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BindingState {
    NotRequested,
    Bound,
    NotBound,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum AdmissionReadError {
    #[error("Admission access context expired or invalid")]
    AccessExpired,
    #[error("Invalid admission binding reference")]
    InvalidBinding,
    #[error("Admission read deadline exceeded")]
    DeadlineExceeded,
    #[error("Admission read denied")]
    Denied,
    #[error("Admission read capacity unavailable")]
    CapacityExhausted,
    #[error("Global service consent required")]
    ConsentRequired,
    #[error("Global enrollment policy unavailable")]
    PolicyUnavailable,
    #[error("Admission owner unavailable")]
    Unavailable,
    #[error("Invalid admission owner response")]
    InvalidResponse,
}

/// Request-bound observation; no constructor, Clone, serialization or rebind API.
/// Each accessor rechecks the borrowed context. Negative facts remain explicit.
/// A database observation timestamp is not a validity deadline.
/// ```compile_fail
/// use api_gateway::adapters::out::auth_client::concert_admission::AdmissionObservation;
/// let observation = AdmissionObservation {};
/// ```
/// ```compile_fail
/// use api_gateway::adapters::out::auth_client::concert_admission::AdmissionObservation;
/// fn export(observation: &AdmissionObservation<'_>) { let _ = serde_json::to_string(observation); }
/// ```
/// ```compile_fail
/// use api_gateway::adapters::out::auth_client::concert_admission::AdmissionObservation;
/// let _ = serde_json::from_str::<AdmissionObservation<'_>>("{}");
/// ```
pub struct AdmissionObservation<'a> {
    context: &'a ConcertAccessContext,
    binding: Option<BindingReference>,
    account_active: bool,
    session_live: bool,
    binding_state: BindingState,
    observed_at: DateTime<Utc>,
}
impl fmt::Debug for AdmissionObservation<'_> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AdmissionObservation([REDACTED])")
    }
}
impl AdmissionObservation<'_> {
    pub fn check(&self) -> Result<(), AdmissionReadError> {
        self.context
            .check()
            .map_err(|_| AdmissionReadError::AccessExpired)
    }
    pub fn account_active(&self) -> Result<bool, AdmissionReadError> {
        self.check()?;
        Ok(self.account_active)
    }
    pub fn session_live(&self) -> Result<bool, AdmissionReadError> {
        self.check()?;
        Ok(self.session_live)
    }
    pub fn binding_state(&self) -> Result<BindingState, AdmissionReadError> {
        self.check()?;
        Ok(self.binding_state)
    }
    pub fn observed_at(&self) -> Result<DateTime<Utc>, AdmissionReadError> {
        self.check()?;
        Ok(self.observed_at)
    }
    /// The exact requested shape, never an owner-granted capability.
    pub fn binding_reference(&self) -> Result<Option<&BindingReference>, AdmissionReadError> {
        self.check()?;
        Ok(self.binding.as_ref())
    }
}

impl GrpcAuthClient {
    pub async fn read_concert_admission<'a>(
        &self,
        context: &'a ConcertAccessContext,
        binding: Option<BindingReference>,
    ) -> Result<AdmissionObservation<'a>, AdmissionReadError> {
        let access = || {
            context
                .check()
                .map_err(|_| AdmissionReadError::AccessExpired)
        };
        access()?;
        let deadline = context
            .rpc_deadline()
            .map_err(|_| AdmissionReadError::AccessExpired)?;
        let ceiling = platform::grpc_deadline::local_deadline(Edge::GatewayAuth);
        let remaining = context
            .remaining(ceiling)
            .map_err(|_| AdmissionReadError::AccessExpired)?
            .min(
                deadline
                    .remaining(ceiling, context.access_expires_micros())
                    .map_err(|_| AdmissionReadError::DeadlineExceeded)?,
            );
        let mut stop = deadline.stop().min(tokio::time::Instant::now() + remaining);
        if let Some(outer) = platform::call_deadline::current() {
            stop = stop.min(outer.into());
        }
        let actor = context
            .user_id()
            .map_err(|_| AdmissionReadError::AccessExpired)?;
        let message = proto::ReadConcertAdmissionRequest {
            user_id: actor.to_string(),
            session_family_id: context
                .session_family()
                .map_err(|_| AdmissionReadError::AccessExpired)?
                .to_string(),
            binding: binding.map(|b| proto::ConcertBindingReference {
                identity_id: b.identity_id.to_string(),
                provider: b.provider.owner_name().into(),
                binding_epoch: b.epoch,
            }),
        };
        let mut request =
            request_for(message, &actor.to_string()).map_err(|_| AdmissionReadError::Denied)?;
        let timeout = remaining.min(stop.saturating_duration_since(tokio::time::Instant::now()));
        if timeout.is_zero() {
            return Err(AdmissionReadError::DeadlineExceeded);
        }
        request.set_timeout(timeout);
        let mut client = self
            .client
            .clone()
            .max_decoding_message_size(MAX_ADMISSION_RESPONSE_BYTES);
        access()?;
        // Includes readiness, protobuf/body collection, configured signing,
        // server and response decoding. No replay_safe_read, cache or retry.
        let result = platform::call_deadline::scope(stop.into_std(), async {
            // The shared non-retrying helper checks the process-wide edge
            // limiter before polling generated readiness/signing/dispatch. Its
            // permit and standard metrics live until completion, error, timeout
            // or cancellation. The absolute timeout stays inside its accounting
            // so a local expiry increments the standard counter exactly once.
            let remaining = stop.saturating_duration_since(tokio::time::Instant::now());
            if remaining.is_zero() {
                self.metrics
                    .edge_deadline_metrics()
                    .record_deadline_exhausted(Edge::GatewayAuth);
                return Err(Status::deadline_exceeded("Request timed out"));
            }
            platform::grpc_deadline::with_deadline(
                remaining,
                Edge::GatewayAuth,
                self.metrics.edge_deadline_metrics(),
                async {
                    match tokio::time::timeout_at(stop, client.read_concert_admission(request))
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
            return Err(AdmissionReadError::DeadlineExceeded);
        }
        let response = result
            .map_err(|status| match status.code() {
                Code::FailedPrecondition if status.message() == "CONSENT_REQUIRED" => AdmissionReadError::ConsentRequired,
                Code::Unavailable if status.message() == "POLICY_UNAVAILABLE" => AdmissionReadError::PolicyUnavailable,
                Code::DeadlineExceeded => AdmissionReadError::DeadlineExceeded,
                Code::Unauthenticated | Code::PermissionDenied => AdmissionReadError::Denied,
                Code::ResourceExhausted => AdmissionReadError::CapacityExhausted,
                Code::DataLoss | Code::OutOfRange => AdmissionReadError::InvalidResponse,
                _ => AdmissionReadError::Unavailable,
            })?
            .into_inner();
        let state = match response.binding_state {
            1 if binding.is_none() => BindingState::NotRequested,
            2 if binding.is_some() => BindingState::Bound,
            3 if binding.is_some() => BindingState::NotBound,
            _ => return Err(AdmissionReadError::InvalidResponse),
        };
        let observed_at = DateTime::from_timestamp_micros(response.observed_at_unix_micros)
            .ok_or(AdmissionReadError::InvalidResponse)?;
        access()?;
        let observation = AdmissionObservation {
            context,
            binding,
            account_active: response.account_active,
            session_live: response.session_live,
            binding_state: state,
            observed_at,
        };
        observation.check()?;
        if tokio::time::Instant::now() >= stop {
            return Err(AdmissionReadError::DeadlineExceeded);
        }
        Ok(observation)
    }
}
