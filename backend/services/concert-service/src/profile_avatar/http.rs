//! WP-08 T-703: the two profile avatar routes.
//!
//! `GET /api/me/avatar` and `PUT /api/me/avatar` are the frozen profile
//! operations `getOwnProfileAvatar` and `setOwnProfileAvatar`
//! (http-contract.md §2.5). There is no event path and no participant: the
//! owner is the authenticated profile the request was verified for.
//!
//! Two guards are structural rather than conventional:
//!
//! * the closed route table below has exactly two entries, so any other
//!   `(method, path)` is not a profile avatar operation at all;
//! * any target that contains `/events/` is refused *before* the route table is
//!   consulted, so a concert-shaped request can never reach the producer even if
//!   a later edit added a permissive route.
use super::repository::{
    http_reason, http_status, ProfileAvatarActor, ProfileAvatarError, ProfileAvatarPersistence,
};
use serde_json::{json, Value};
use std::sync::Arc;

/// The single path both profile avatar operations share.
pub const AVATAR_PATH: &str = "/api/me/avatar";

/// The closed profile avatar route table: `(method, path, operationId)`.
pub const ROUTES: &[(&str, &str, &str)] = &[
    ("GET", AVATAR_PATH, "getOwnProfileAvatar"),
    ("PUT", AVATAR_PATH, "setOwnProfileAvatar"),
];

/// One profile avatar operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProfileAvatarRoute {
    Get,
    Put,
}

impl ProfileAvatarRoute {
    /// The frozen operation id of this route.
    pub fn operation(self) -> &'static str {
        match self {
            Self::Get => "getOwnProfileAvatar",
            Self::Put => "setOwnProfileAvatar",
        }
    }
    pub fn mutation(self) -> bool {
        self == Self::Put
    }
}

/// A refusal that must become a closed error envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProfileAvatarRefusal {
    pub status: u16,
    pub reason: &'static str,
}

impl ProfileAvatarRefusal {
    fn invalid() -> Self {
        Self {
            status: 400,
            reason: "INVALID_INPUT",
        }
    }
    fn from_error(error: ProfileAvatarError) -> Self {
        Self {
            status: http_status(error),
            reason: http_reason(error),
        }
    }
}

/// Whether a raw target names a concert event path.
///
/// This is checked on the raw target, before any parsing, so a percent-encoded
/// or nested segment cannot slip past a later, narrower parser. `%2Fevents%2F`
/// decodes to the same path and is refused too.
pub fn names_event_path(target: &str) -> bool {
    let lowered = target.to_ascii_lowercase();
    lowered.contains("/events/")
        || lowered.contains("%2fevents%2f")
        || lowered.contains("%2fevents/")
        || lowered.contains("/events%2f")
}

/// Parse a profile avatar target into its operation.
///
/// `target` is the request path plus query exactly as received. A query is
/// refused: neither frozen avatar operation carries one, and accepting it would
/// let a later addition be silently ignored.
pub fn route(method: &str, target: &str) -> Result<ProfileAvatarRoute, ProfileAvatarRefusal> {
    if names_event_path(target) {
        return Err(ProfileAvatarRefusal::invalid());
    }
    if target.len() > 4096 || target.chars().any(char::is_control) {
        return Err(ProfileAvatarRefusal::invalid());
    }
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    if query.is_some() {
        return Err(ProfileAvatarRefusal::invalid());
    }
    ROUTES
        .iter()
        .find(|(verb, route_path, _)| *verb == method && *route_path == path)
        .map(|(_, _, operation)| match *operation {
            "getOwnProfileAvatar" => ProfileAvatarRoute::Get,
            _ => ProfileAvatarRoute::Put,
        })
        .ok_or_else(ProfileAvatarRefusal::invalid)
}

/// The result of one profile avatar request, before HTTP framing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ProfileAvatarResponse {
    pub status: u16,
    pub body: Value,
}

impl ProfileAvatarResponse {
    /// The frozen error envelope for a refusal.
    pub fn error(refusal: ProfileAvatarRefusal) -> Self {
        Self {
            status: refusal.status,
            body: json!({
                "code": format!("PROFILE_{}", refusal.status),
                "message": "Profile request unavailable",
                "details": {"reason": refusal.reason},
            }),
        }
    }
}

/// Serve one profile avatar request.
///
/// `actor` is the already verified authenticated profile; it is never derived
/// from `target` or `body`. `body` is the raw request body: empty for a read and
/// the `core.SetAvatarRequest` bytes for a write.
pub async fn handle(
    persistence: &Arc<ProfileAvatarPersistence>,
    actor: ProfileAvatarActor,
    method: &str,
    target: &str,
    body: &[u8],
) -> ProfileAvatarResponse {
    let route = match route(method, target) {
        Ok(route) => route,
        Err(refusal) => return ProfileAvatarResponse::error(refusal),
    };
    match route {
        ProfileAvatarRoute::Get => {
            // A read carries no body. One that does is a caller error, not a
            // silently ignored payload.
            if !body.is_empty() {
                return ProfileAvatarResponse::error(ProfileAvatarRefusal::invalid());
            }
            match persistence.read(actor).await {
                Ok(state) => ProfileAvatarResponse {
                    status: 200,
                    body: state.to_wire(),
                },
                Err(error) => {
                    ProfileAvatarResponse::error(ProfileAvatarRefusal::from_error(error))
                }
            }
        }
        ProfileAvatarRoute::Put => match persistence.set_avatar(actor, body).await {
            Ok(state) => ProfileAvatarResponse {
                status: 200,
                body: state.to_wire(),
            },
            Err(ProfileAvatarError::Conflict) => {
                // `409` carries the current config, so the client can reconcile
                // without a second round trip. A failed read is not disguised as
                // a conflict: it is reported as unavailable.
                match persistence.read(actor).await {
                    Ok(current) => ProfileAvatarResponse {
                        status: 409,
                        body: json!({
                            "code": "PROFILE_409",
                            "message": "Profile request unavailable",
                            "details": {"reason": "VERSION_CONFLICT", "current": current.to_wire()},
                        }),
                    },
                    Err(error) => ProfileAvatarResponse::error(
                        ProfileAvatarRefusal::from_error(error),
                    ),
                }
            }
            Err(error) => ProfileAvatarResponse::error(ProfileAvatarRefusal::from_error(error)),
        },
    }
}
