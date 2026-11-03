//! Strict Concert access capture at the HTTP boundary.
//!
//! The extractor reuses the original request deadline installed by the HTTP
//! timeout layer and the configured issuer key set. It produces only a private
//! request-bound context, not an owner result, event grant or participation
//! decision. An unknown-key refresh may await; strict verification runs again
//! afterward against the same original stop.
use super::{parse_bearer_token, GatewayError, RequestDeadline, AUTHORIZATION};
use crate::adapters::out::jwt::concert_access::{ConcertAccessContext, ConcertAccessRejection};
use crate::config::AppState;
use crate::domain::access_token::TokenRejection;
use async_trait::async_trait;
use axum::extract::{FromRef, FromRequestParts};
use axum::http::request::Parts;
use std::sync::Arc;

/// Strict, nonserialized identity/time input for a future Concert route.
/// It has no public constructor and is never logged or returned as a grant.
#[derive(Debug)]
pub struct HttpConcertAccess(ConcertAccessContext);

impl HttpConcertAccess {
    /// Recheck immediately before dispatch. Callers still owe current owner
    /// account/session/binding/Profile checks; this only proves signed access.
    pub fn check(&self) -> Result<(), GatewayError> {
        self.0
            .check()
            .map_err(|_| GatewayError::unauthorized("Invalid bearer token"))
    }

    pub fn user_id(&self) -> Result<uuid::Uuid, GatewayError> {
        self.0
            .user_id()
            .map_err(|_| GatewayError::unauthorized("Invalid bearer token"))
    }

    pub fn session_family(&self) -> Result<uuid::Uuid, GatewayError> {
        self.0
            .session_family()
            .map_err(|_| GatewayError::unauthorized("Invalid bearer token"))
    }

    pub fn remaining(
        &self,
        ceiling: std::time::Duration,
    ) -> Result<std::time::Duration, GatewayError> {
        self.0
            .remaining(ceiling)
            .map_err(|_| GatewayError::unauthorized("Invalid bearer token"))
    }

    #[allow(dead_code)]
    pub(crate) fn borrowed(&self) -> &ConcertAccessContext {
        &self.0
    }
}

#[async_trait]
impl<S> FromRequestParts<S> for HttpConcertAccess
where
    Arc<AppState>: FromRef<S>,
    S: Send + Sync,
{
    type Rejection = GatewayError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let state = Arc::<AppState>::from_ref(state);
        let original_stop = parts
            .extensions
            .get::<RequestDeadline>()
            .map(|deadline| deadline.0)
            .ok_or_else(|| GatewayError::gateway_timeout("Request timed out"))?;
        let authorization = parts
            .headers
            .get(AUTHORIZATION)
            .and_then(|value| value.to_str().ok())
            .ok_or_else(|| GatewayError::unauthorized("Invalid bearer token"))?;
        let token = parse_bearer_token(authorization)
            .ok_or_else(|| GatewayError::unauthorized("Invalid bearer token"))?;
        let first = state
            .jwt_verifier
            .verify_concert_access(token, Some(original_stop));
        let verified = match first {
            Err(ConcertAccessRejection::Token(TokenRejection::UnknownKid)) => {
                match &state.signing_keys {
                    Some(refresher) if refresher.refresh_for_unknown_kid().await => state
                        .jwt_verifier
                        .verify_concert_access(token, Some(original_stop)),
                    _ => return Err(GatewayError::unauthorized("Invalid bearer token")),
                }
            }
            result => result,
        };
        let context = verified.map_err(|_| GatewayError::unauthorized("Invalid bearer token"))?;
        // Verification and an unknown-key refresh have both ended. Check the
        // same captured monotonic and signed wall stops before returning.
        context
            .check()
            .map_err(|_| GatewayError::unauthorized("Invalid bearer token"))?;
        Ok(Self(context))
    }
}
