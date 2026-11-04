//! Live owning-service adapters. Construction is explicit; none mints a write
//! capability from a read observation or silently enables product composition.
pub mod auth;
pub mod coordinator;
pub mod decision_rpc;
pub mod guarded;
pub mod update;

#[cfg(test)]
mod auth_tests;

#[cfg(test)]
mod coordinator_tests;

use super::{context::VerifiedParticipantRequest, ports::CurrentAuth};
use concert_application::avatar::AvatarError;
use std::time::Duration;
use tonic::{Code, Response};

const OWNER_BUDGET: Duration = Duration::from_secs(2);

/// Retain the signed request's wall and monotonic bounds, including every
/// earlier owner/channel wait. An outbound call cannot restart either clock.
pub(crate) fn owner_window(
    context: &VerifiedParticipantRequest,
) -> Result<(tokio::time::Instant, i64, Duration), AvatarError> {
    context.check().map_err(|_| AvatarError::Unauthenticated)?;
    let original = context.deadline();
    let mut stop = original.stop();
    if let Some(outer) = platform::call_deadline::current() {
        stop = stop.min(outer.into());
    }
    let remaining = original
        .remaining(OWNER_BUDGET, original.expires_micros())
        .map_err(|_| AvatarError::Unauthenticated)?
        .min(stop.saturating_duration_since(tokio::time::Instant::now()));
    if remaining.is_zero() {
        return Err(AvatarError::Unauthenticated);
    }
    let expires = original.expires_micros().min(
        chrono::Utc::now()
            .timestamp_micros()
            .saturating_add(remaining.as_micros() as i64),
    );
    Ok((stop, expires, remaining))
}

pub(crate) fn owner_error(error: tonic::Status) -> AvatarError {
    match error.code() {
        Code::Unauthenticated => AvatarError::Unauthenticated,
        Code::PermissionDenied => AvatarError::Denied,
        _ => AvatarError::Unavailable,
    }
}

pub(crate) fn owner_reply<T>(response: Response<T>) -> Result<T, AvatarError> {
    if response
        .metadata()
        .get("cache-control")
        .and_then(|v| v.to_str().ok())
        != Some("no-store")
    {
        return Err(AvatarError::Unavailable);
    }
    Ok(response.into_inner())
}

pub(crate) fn current_auth(
    context: &VerifiedParticipantRequest,
    observed_at_micros: i64,
    expires_micros: i64,
) -> Result<CurrentAuth, AvatarError> {
    let value = CurrentAuth {
        user_id: context.key().user_id.uuid(),
        session_family_id: context.session_family_id(),
        observed_at_micros,
        expires_micros,
    };
    value.check(context)?;
    Ok(value)
}
