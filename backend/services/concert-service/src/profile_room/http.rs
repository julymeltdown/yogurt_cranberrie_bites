//! WP-07 T-605: the five personal-room routes.
//!
//! | # | operationId | Method | Path |
//! | --- | --- | --- | --- |
//! | 2.4.1 | `getProfileRoom` | GET | `/api/profiles/{profileId}/room` |
//! | 2.4.2 | `getOwnRoom` | GET | `/api/me/room` |
//! | 2.4.3 | `saveOwnRoom` | PUT | `/api/me/room` |
//! | 2.4.4 | `setOwnRoomVisibility` | PATCH | `/api/me/room/visibility` |
//! | 2.4.5 | `uploadRoomPoster` | POST | `/api/me/room/posters` |
//!
//! The route table is closed: any other `(method, path)` is not a room operation
//! at all, and is refused **before** the producer does any work. Two guards are
//! structural rather than conventional:
//!
//! * a target that names a concert event is refused before the table is consulted,
//!   including its percent-encoded spellings, so a concert-shaped request can never
//!   reach the room producer;
//! * a target that names an intro post is refused the same way. The room is
//!   subordinate to the profile and is never embedded in, or addressed through, an
//!   intro post.
//!
//! Every mutation carries a bounded body. A body over the bound is `413` before it
//! is parsed, so an oversized poster cannot make the service buffer it.
use super::repository::{
    conflict_body, http_reason, http_status, RoomActor, RoomErrorKind, RoomPersistence,
};
use concert_domain::profile_room::{ProfileId, MAX_REQUEST_BYTES};
use serde_json::{json, Value};
use std::sync::Arc;

/// The path of the owner's room.
pub const OWN_ROOM_PATH: &str = "/api/me/room";
/// The path of the owner's room visibility.
pub const VISIBILITY_PATH: &str = "/api/me/room/visibility";
/// The path of the owner's poster upload.
pub const POSTERS_PATH: &str = "/api/me/room/posters";
/// The prefix of a visitor's room read.
pub const PROFILE_ROOM_PREFIX: &str = "/api/profiles/";
/// The suffix of a visitor's room read.
pub const PROFILE_ROOM_SUFFIX: &str = "/room";

/// The closed room route table: `(method, path, operationId)`.
pub const ROUTES: &[(&str, &str, &str)] = &[
    ("GET", "/api/profiles/{profileId}/room", "getProfileRoom"),
    ("GET", OWN_ROOM_PATH, "getOwnRoom"),
    ("PUT", OWN_ROOM_PATH, "saveOwnRoom"),
    ("PATCH", VISIBILITY_PATH, "setOwnRoomVisibility"),
    ("POST", POSTERS_PATH, "uploadRoomPoster"),
];

/// One room operation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoomRoute {
    /// `getProfileRoom` — a visitor's read of a profile's room.
    ProfileRoom(ProfileId),
    /// `getOwnRoom`.
    Own,
    /// `saveOwnRoom`.
    Save,
    /// `setOwnRoomVisibility`.
    Visibility,
    /// `uploadRoomPoster`.
    Poster,
}

impl RoomRoute {
    /// The frozen operation id of this route.
    pub fn operation(self) -> &'static str {
        match self {
            Self::ProfileRoom(_) => "getProfileRoom",
            Self::Own => "getOwnRoom",
            Self::Save => "saveOwnRoom",
            Self::Visibility => "setOwnRoomVisibility",
            Self::Poster => "uploadRoomPoster",
        }
    }

    /// Whether this route writes.
    pub fn mutation(self) -> bool {
        matches!(self, Self::Save | Self::Visibility | Self::Poster)
    }
}

/// A refusal that must become a closed error envelope.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoomRefusal {
    pub status: u16,
    pub reason: &'static str,
}

impl RoomRefusal {
    fn invalid() -> Self {
        Self {
            status: 400,
            reason: "INVALID_INPUT",
        }
    }

    fn from_error(error: RoomErrorKind) -> Self {
        Self {
            status: http_status(error),
            reason: http_reason(error),
        }
    }
}

/// Whether a raw target names a concert event path.
///
/// Checked on the raw target before any parsing, so a percent-encoded or nested
/// segment cannot slip past a later, narrower parser.
pub fn names_event_path(target: &str) -> bool {
    let lowered = target.to_ascii_lowercase();
    lowered.contains("/events/")
        || lowered.contains("%2fevents%2f")
        || lowered.contains("%2fevents/")
        || lowered.contains("/events%2f")
}

/// Whether a raw target names an intro post.
///
/// A room is never addressed through an intro post. This is checked on the raw
/// target, including percent-encoded spellings, before the route table is
/// consulted.
pub fn names_intro_path(target: &str) -> bool {
    let lowered = target.to_ascii_lowercase();
    // Exact segment spellings only: a word that merely starts with "intro" is not
    // an intro post, and refusing it would be a false positive.
    lowered.contains("/intro/")
        || lowered.contains("/intro?")
        || lowered.contains("/intros/")
        || lowered.contains("/intros?")
        || lowered.contains("/intro-posts")
        || lowered.contains("?intro=")
        || lowered.ends_with("/intro")
        || lowered.ends_with("/intros")
        || lowered.contains("%2fintro%2f")
        || lowered.contains("%2fintro/")
        || lowered.contains("/intro%2f")
        || lowered.contains("%2fintro?")
}

/// Parse a room target into its operation.
///
/// `target` is the request path plus query exactly as received. A query is
/// refused: no frozen room operation carries one, and accepting it would let a
/// later addition be silently ignored.
pub fn route(method: &str, target: &str) -> Result<RoomRoute, RoomRefusal> {
    if names_event_path(target) || names_intro_path(target) {
        return Err(RoomRefusal::invalid());
    }
    if target.len() > 4096 || target.chars().any(char::is_control) {
        return Err(RoomRefusal::invalid());
    }
    let (path, query) = target
        .split_once('?')
        .map_or((target, None), |(path, query)| (path, Some(query)));
    if query.is_some() {
        return Err(RoomRefusal::invalid());
    }
    match (method, path) {
        ("GET", OWN_ROOM_PATH) => return Ok(RoomRoute::Own),
        ("PUT", OWN_ROOM_PATH) => return Ok(RoomRoute::Save),
        ("PATCH", VISIBILITY_PATH) => return Ok(RoomRoute::Visibility),
        ("POST", POSTERS_PATH) => return Ok(RoomRoute::Poster),
        _ => {}
    }
    // The visitor read is the only parameterised room route, and its segment is a
    // canonical profile id.
    if method == "GET" {
        if let Some(segment) = path
            .strip_prefix(PROFILE_ROOM_PREFIX)
            .and_then(|rest| rest.strip_suffix(PROFILE_ROOM_SUFFIX))
        {
            if !segment.is_empty() && !segment.contains('/') {
                let profile = segment.parse::<ProfileId>().map_err(|_| RoomRefusal::invalid())?;
                return Ok(RoomRoute::ProfileRoom(profile));
            }
        }
    }
    Err(RoomRefusal::invalid())
}

/// The result of one room request, before HTTP framing.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RoomResponse {
    pub status: u16,
    pub body: Value,
}

impl RoomResponse {
    /// The frozen error envelope for a refusal.
    pub fn error(refusal: RoomRefusal) -> Self {
        Self {
            status: refusal.status,
            body: json!({
                "code": format!("ROOM_{}", refusal.status),
                "message": "Room request unavailable",
                "details": {"reason": refusal.reason},
            }),
        }
    }

    fn from_error(error: RoomErrorKind) -> Self {
        Self::error(RoomRefusal::from_error(error))
    }
}

/// Serve one room request.
///
/// `actor` is the already verified authenticated profile; it is never derived
/// from `target` or `body`. `body` is the raw request body: empty for a read and
/// the frozen request bytes for a write.
pub async fn handle(
    persistence: &Arc<RoomPersistence>,
    actor: RoomActor,
    method: &str,
    target: &str,
    body: &[u8],
) -> RoomResponse {
    let route = match route(method, target) {
        Ok(route) => route,
        Err(refusal) => return RoomResponse::error(refusal),
    };
    // A read carries no body. One that does is a caller error, not a silently
    // ignored payload.
    if !route.mutation() && !body.is_empty() {
        return RoomResponse::error(RoomRefusal::invalid());
    }
    if route.mutation() && body.len() > MAX_REQUEST_BYTES {
        return RoomResponse::from_error(RoomErrorKind::TooLarge);
    }
    match route {
        RoomRoute::Own => match persistence.read_own(actor).await {
            Ok(body) => RoomResponse { status: 200, body },
            Err(error) => RoomResponse::from_error(error),
        },
        RoomRoute::ProfileRoom(target) => {
            match persistence.read_for_visitor(actor, target).await {
                Ok(body) => RoomResponse { status: 200, body },
                Err(error) => RoomResponse::from_error(error),
            }
        }
        RoomRoute::Save => match persistence.save(actor, body).await {
            Ok(body) => RoomResponse { status: 200, body },
            Err(RoomErrorKind::Conflict) => conflict(persistence, actor).await,
            Err(error) => RoomResponse::from_error(error),
        },
        RoomRoute::Visibility => match persistence.set_visibility(actor, body).await {
            Ok(body) => RoomResponse { status: 200, body },
            Err(RoomErrorKind::Conflict) => conflict(persistence, actor).await,
            Err(error) => RoomResponse::from_error(error),
        },
        RoomRoute::Poster => match persistence.upload_poster(actor, body).await {
            Ok(body) => RoomResponse { status: 200, body },
            Err(error) => RoomResponse::from_error(error),
        },
    }
}

/// `409` with the current room, so the client can offer an explicit resolution
/// instead of silently discarding its unsaved layout.
async fn conflict(persistence: &Arc<RoomPersistence>, actor: RoomActor) -> RoomResponse {
    match persistence.read_own(actor).await {
        Ok(current) => RoomResponse {
            status: 409,
            body: conflict_body(current),
        },
        // A failed read is not disguised as a conflict: it is reported as
        // unavailable.
        Err(error) => RoomResponse::from_error(error),
    }
}
