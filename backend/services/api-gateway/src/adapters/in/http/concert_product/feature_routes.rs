//! Exact C04 feature admission. Canonical owners still authorize every request.
use super::*;
use serde::Deserialize;
use std::collections::BTreeMap;

pub(super) fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/concert/events/:event_id/me/operations-access",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/mural/snapshot",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/mural/operations",
            get(dispatch),
        )
        .route("/api/concert/events/:event_id/mural/pixels", post(dispatch))
        .route(
            "/api/concert/events/:event_id/mural/commands/:key/result",
            get(dispatch),
        )
        .route("/api/concert/events/:event_id/mural/tools", get(dispatch))
        .route(
            "/api/concert/events/:event_id/mural/actions/:key",
            get(dispatch),
        )
        .route("/api/concert/events/:event_id/mural/erase", post(dispatch))
        .route("/api/concert/events/:event_id/mural/fill", post(dispatch))
        .route("/api/concert/events/:event_id/mural/undo", post(dispatch))
        .route("/api/concert/events/:event_id/admin/mural", get(dispatch))
        .route(
            "/api/concert/events/:event_id/admin/mural/control",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/admin/mural/locks",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/admin/mural/restore",
            post(dispatch),
        )
        .route("/api/concert/events/:event_id/me/age-status", get(dispatch))
        .route(
            "/api/concert/events/:event_id/me/age-verifications",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/me/age-verifications/callback",
            post(dispatch),
        )
        .route("/api/concert/events/:event_id/intros", get(dispatch))
        .route(
            "/api/concert/events/:event_id/intros/:intro_id",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/me/intro",
            get(dispatch).put(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/me/intro/publish",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/me/intro/close",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/me/notifications",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/me/notifications/unread-count",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/me/notifications/:notification_id/read",
            axum::routing::patch(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/me/notifications/read-all",
            axum::routing::patch(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/venue-meeting/context",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/venue-meeting/invitations",
            get(dispatch).post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/venue-meeting/invitations/:invitation_id",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/venue-meeting/invitations/:invitation_id/decision",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/venue-meeting/messages",
            get(dispatch).post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/venue-meeting/commands/:command_key",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/venue/location",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/venue/location/sharing",
            put(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/venue/location/fix",
            put(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/venue/location/commands/:key",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/venue/landmarks/:landmark_id/notes",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/venue/landmarks/:landmark_id/notes/:note_id/replies",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/venue/landmarks/:landmark_id/notes/:note_id",
            delete(dispatch),
        )
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FeatureRoute {
    OwnOperationsAccess,
    MuralSnapshot,
    MuralDelta,
    MuralPixel,
    MuralPixelResult,
    MuralTools,
    MuralActionResult,
    MuralErase,
    MuralFill,
    MuralUndo,
    MuralAdmin,
    MuralAdminControl,
    MuralAdminLocks,
    MuralAdminRestore,
    AgeStatus,
    StartAgeVerification,
    CompleteAgeVerification,
    ListIntros,
    GetIntro,
    OwnIntro,
    SetIntro,
    PublishIntro,
    CloseIntro,
    ListNotifications,
    UnreadCount,
    MarkRead,
    MarkAllRead,
    MeetingContext,
    ListInvitations,
    CreateInvitation,
    GetInvitation(Uuid),
    DecideInvitation(Uuid),
    ListMessages,
    SendMessage,
    MeetingCommand(Uuid),
    VenueLocationSnapshot,
    SetVenueLocationSharing,
    PublishVenueLocationFix,
    VenueLocationCommand(Uuid),
    CreateVenuePlaceNote,
    CreateVenuePlaceReply,
    WithdrawVenuePlaceNote,
}
impl FeatureRoute {
    pub(super) fn mutation(self) -> bool {
        matches!(
            self,
            Self::MuralPixel
                | Self::MuralErase
                | Self::MuralFill
                | Self::MuralUndo
                | Self::MuralAdminControl
                | Self::MuralAdminLocks
                | Self::MuralAdminRestore
                | Self::StartAgeVerification
                | Self::CompleteAgeVerification
                | Self::SetIntro
                | Self::PublishIntro
                | Self::CloseIntro
                | Self::MarkRead
                | Self::MarkAllRead
                | Self::CreateInvitation
                | Self::DecideInvitation(_)
                | Self::SendMessage
                | Self::SetVenueLocationSharing
                | Self::PublishVenueLocationFix
                | Self::CreateVenuePlaceNote
                | Self::CreateVenuePlaceReply
                | Self::WithdrawVenuePlaceNote
        )
    }
    pub(super) fn body_limit(self) -> usize {
        match self {
            Self::MuralAdminControl | Self::MuralAdminLocks | Self::MuralAdminRestore => 8192,
            Self::MuralPixel | Self::MuralErase | Self::MuralFill | Self::MuralUndo => 1024,
            Self::SetIntro | Self::PublishIntro | Self::CloseIntro => 16 * 1024,
            Self::MarkRead
            | Self::MarkAllRead
            | Self::StartAgeVerification
            | Self::CompleteAgeVerification => 8192,
            Self::CreateInvitation | Self::DecideInvitation(_) | Self::SendMessage => 12_000,
            Self::SetVenueLocationSharing | Self::PublishVenueLocationFix => 8192,
            Self::CreateVenuePlaceNote | Self::CreateVenuePlaceReply => 8192,
            _ => 0,
        }
    }
    #[cfg(test)]
    fn operation(self) -> &'static str {
        match self {
            Self::OwnOperationsAccess => "getOwnConcertOperationsAccess",
            Self::MuralAdmin => "getConcertAdminMural",
            Self::MuralAdminControl => "controlConcertMural",
            Self::MuralAdminLocks => "lockConcertMuralRegion",
            Self::MuralAdminRestore => "restoreConcertMural",
            Self::MuralSnapshot => "getConcertMuralSnapshot",
            Self::MuralDelta => "getConcertMuralDelta",
            Self::MuralPixel => "placeConcertPixel",
            Self::MuralPixelResult => "getConcertMuralOwnResult",
            Self::MuralTools => "getConcertMuralTools",
            Self::MuralActionResult => "getOwnConcertMuralAction",
            Self::MuralErase => "eraseConcertMuralPixel",
            Self::MuralFill => "fillConcertMuralRegion",
            Self::MuralUndo => "undoConcertMuralAction",
            Self::AgeStatus => "getConcertAgeStatus",
            Self::StartAgeVerification => "startConcertAgeVerification",
            Self::CompleteAgeVerification => "completeConcertAgeVerification",
            Self::ListIntros => "listConcertIntros",
            Self::GetIntro => "getConcertIntro",
            Self::OwnIntro => "getOwnConcertIntro",
            Self::SetIntro => "setConcertIntro",
            Self::PublishIntro => "publishConcertIntro",
            Self::CloseIntro => "closeConcertIntro",
            Self::ListNotifications => "listConcertNotifications",
            Self::UnreadCount => "getConcertNotificationUnreadCount",
            Self::MarkRead => "markConcertNotification",
            Self::MarkAllRead => "markAllConcertNotifications",
            Self::MeetingContext => "resolveVenueMeetingContext",
            Self::ListInvitations => "listVenueMeetingInvitations",
            Self::CreateInvitation => "createVenueMeetingInvitation",
            Self::GetInvitation(_) => "getVenueMeetingInvitation",
            Self::DecideInvitation(_) => "decideVenueMeetingInvitation",
            Self::ListMessages => "listVenueMeetingMessages",
            Self::SendMessage => "sendVenueMeetingMessage",
            Self::MeetingCommand(_) => "getVenueMeetingCommand",
            Self::VenueLocationSnapshot => "getVenueLocationSnapshot",
            Self::SetVenueLocationSharing => "setVenueLocationSharing",
            Self::PublishVenueLocationFix => "publishVenueLocationFix",
            Self::VenueLocationCommand(_) => "getVenueLocationCommand",
            Self::CreateVenuePlaceNote => "createVenuePlaceNote",
            Self::CreateVenuePlaceReply => "createVenuePlaceReply",
            Self::WithdrawVenuePlaceNote => "withdrawVenuePlaceNote",
        }
    }
}

fn tail(path: &str) -> Option<(&str, &str)> {
    path.strip_prefix("/api/concert/events/")?.split_once('/')
}
pub(super) fn protects(path: &str) -> bool {
    tail(path).is_some_and(|(_, tail)| {
        tail == "mural"
            || tail.starts_with("mural/")
            || tail == "admin/mural"
            || tail.starts_with("admin/mural/")
            || tail == "me/operations-access"
            || tail.starts_with("me/operations-access/")
            || tail == "me/age-status"
            || tail.starts_with("me/age-status/")
            || tail == "me/age-verifications"
            || tail.starts_with("me/age-verifications/")
            || tail == "intros"
            || tail.starts_with("intros/")
            || tail == "me/intro"
            || tail.starts_with("me/intro/")
            || tail == "me/notifications"
            || tail.starts_with("me/notifications/")
            || tail == "venue-meeting"
            || tail.starts_with("venue-meeting/")
            || tail == "venue"
            || tail.starts_with("venue/")
    })
}
pub(super) fn route(method: &Method, path: &str) -> Option<FeatureRoute> {
    let (event, tail) = tail(path)?;
    uuid(event)?;
    use FeatureRoute::*;
    match (
        method.as_str(),
        tail.split('/').collect::<Vec<_>>().as_slice(),
    ) {
        ("GET", ["me", "operations-access"]) => Some(OwnOperationsAccess),
        ("GET", ["admin", "mural"]) => Some(MuralAdmin),
        ("POST", ["admin", "mural", "control"]) => Some(MuralAdminControl),
        ("POST", ["admin", "mural", "locks"]) => Some(MuralAdminLocks),
        ("POST", ["admin", "mural", "restore"]) => Some(MuralAdminRestore),
        ("GET", ["mural", "snapshot"]) => Some(MuralSnapshot),
        ("GET", ["mural", "operations"]) => Some(MuralDelta),
        ("POST", ["mural", "pixels"]) => Some(MuralPixel),
        ("GET", ["mural", "commands", key, "result"]) if uuid(key).is_some() => {
            Some(MuralPixelResult)
        }
        ("GET", ["mural", "tools"]) => Some(MuralTools),
        ("GET", ["mural", "actions", key]) if uuid(key).is_some() => Some(MuralActionResult),
        ("POST", ["mural", "erase"]) => Some(MuralErase),
        ("POST", ["mural", "fill"]) => Some(MuralFill),
        ("POST", ["mural", "undo"]) => Some(MuralUndo),
        ("GET", ["me", "age-status"]) => Some(AgeStatus),
        ("POST", ["me", "age-verifications"]) => Some(StartAgeVerification),
        ("POST", ["me", "age-verifications", "callback"]) => Some(CompleteAgeVerification),
        ("GET", ["intros"]) => Some(ListIntros),
        ("GET", ["intros", id]) if uuid(id).is_some() => Some(GetIntro),
        ("GET", ["me", "intro"]) => Some(OwnIntro),
        ("PUT", ["me", "intro"]) => Some(SetIntro),
        ("POST", ["me", "intro", "publish"]) => Some(PublishIntro),
        ("POST", ["me", "intro", "close"]) => Some(CloseIntro),
        ("GET", ["me", "notifications"]) => Some(ListNotifications),
        ("GET", ["me", "notifications", "unread-count"]) => Some(UnreadCount),
        ("PATCH", ["me", "notifications", id, "read"]) if uuid(id).is_some() => Some(MarkRead),
        ("PATCH", ["me", "notifications", "read-all"]) => Some(MarkAllRead),
        ("GET", ["venue-meeting", "context"]) => Some(MeetingContext),
        ("GET", ["venue-meeting", "invitations"]) => Some(ListInvitations),
        ("POST", ["venue-meeting", "invitations"]) => Some(CreateInvitation),
        ("GET", ["venue-meeting", "invitations", id]) if uuid(id).is_some() => {
            Some(GetInvitation(Uuid::parse_str(id).ok()?))
        }
        ("POST", ["venue-meeting", "invitations", id, "decision"]) if uuid(id).is_some() => {
            Some(DecideInvitation(Uuid::parse_str(id).ok()?))
        }
        ("GET", ["venue-meeting", "messages"]) => Some(ListMessages),
        ("POST", ["venue-meeting", "messages"]) => Some(SendMessage),
        ("GET", ["venue-meeting", "commands", key]) if uuid(key).is_some() => {
            Some(MeetingCommand(Uuid::parse_str(key).ok()?))
        }
        ("GET", ["venue", "location"]) => Some(VenueLocationSnapshot),
        ("PUT", ["venue", "location", "sharing"]) => Some(SetVenueLocationSharing),
        ("PUT", ["venue", "location", "fix"]) => Some(PublishVenueLocationFix),
        ("GET", ["venue", "location", "commands", key]) if uuid(key).is_some() => {
            Some(VenueLocationCommand(Uuid::parse_str(key).ok()?))
        }
        ("POST", ["venue", "landmarks", landmark, "notes"]) if landmark_id(landmark) => {
            Some(CreateVenuePlaceNote)
        }
        ("POST", ["venue", "landmarks", landmark, "notes", note, "replies"])
            if landmark_id(landmark) && uuid(note).is_some() =>
        {
            Some(CreateVenuePlaceReply)
        }
        ("DELETE", ["venue", "landmarks", landmark, "notes", note])
            if landmark_id(landmark) && uuid(note).is_some() =>
        {
            Some(WithdrawVenuePlaceNote)
        }
        _ => None,
    }
}

fn landmark_id(value: &str) -> bool {
    (1..=128).contains(&value.len())
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'))
        && value != "."
        && value != ".."
}

fn meeting_read(route: FeatureRoute) -> Option<u8> {
    match route {
        FeatureRoute::MeetingContext => Some(0),
        FeatureRoute::ListInvitations => Some(1),
        FeatureRoute::GetInvitation(_) => Some(2),
        FeatureRoute::ListMessages => Some(3),
        FeatureRoute::MeetingCommand(_) => Some(4),
        _ => None,
    }
}

fn canonical_uuid(value: &str) -> bool {
    Uuid::parse_str(value)
        .ok()
        .is_some_and(|id| !id.is_nil() && id.to_string() == value)
}

fn meeting_decimal(value: &str, minimum: i64) -> bool {
    !value.is_empty()
        && (value.len() == 1 || !value.starts_with('0'))
        && value.bytes().all(|b| b.is_ascii_digit())
        && value
            .parse::<i64>()
            .is_ok_and(|number| (minimum..=9_007_199_254_740_991).contains(&number))
}

fn meeting_target(kind: u8, target: &str) -> bool {
    if kind == 1 {
        let Some((_, query)) = target.split_once('?') else {
            return false;
        };
        let mut fields = BTreeMap::new();
        for pair in query.split('&') {
            let Some((key, value)) = pair.split_once('=') else {
                return false;
            };
            let (Some(key), Some(value)) = (decode(key), decode(value)) else {
                return false;
            };
            if fields.insert(key, value).is_some() {
                return false;
            }
        }
        return fields
            .remove("direction")
            .is_some_and(|value| matches!(value.as_str(), "INCOMING" | "OUTGOING"))
            && fields
                .remove("cursor")
                .is_none_or(|value| meeting_decimal(&value, 0))
            && fields.is_empty();
    }
    let Some((_, query)) = target.split_once('?') else {
        return false;
    };
    let mut fields = BTreeMap::new();
    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            return false;
        };
        let (Some(key), Some(value)) = (decode(key), decode(value)) else {
            return false;
        };
        if fields.insert(key, value).is_some() {
            return false;
        }
    }
    if !fields
        .remove("selectedFriendId")
        .is_some_and(|value| canonical_uuid(&value))
    {
        return false;
    }
    if kind == 3
        && fields
            .remove("cursor")
            .is_some_and(|value| !meeting_decimal(&value, 0))
    {
        return false;
    }
    let landmark_id = fields.remove("landmarkId");
    let catalog = fields.remove("catalogRevision");
    let revision = fields.remove("landmarkRevision");
    let landmark = match (landmark_id, catalog, revision) {
        (None, None, None) => kind != 2,
        (Some(id), Some(catalog), Some(revision)) => {
            !id.is_empty()
                && id.len() <= 128
                && id
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'-' | b'.' | b'_' | b'~'))
                && id != "."
                && id != ".."
                && meeting_decimal(&catalog, 1)
                && meeting_decimal(&revision, 1)
        }
        _ => false,
    };
    landmark && fields.is_empty()
}

/// Validate the frozen query without normalizing the signed target or body.
/// Receipts, proof trust and visibility remain the owning Root's decisions.
pub(super) fn valid_target(route: FeatureRoute, target: &str) -> bool {
    if target.len() > 4096 || target.contains('#') || target.chars().any(char::is_control) {
        return false;
    }
    if let Some(route) = meeting_read(route) {
        return meeting_target(route, target);
    }
    if route == FeatureRoute::MuralDelta {
        return target
            .split_once('?')
            .filter(|(_, q)| !q.contains('&'))
            .and_then(|(_, q)| q.split_once('='))
            .and_then(|(k, v)| Some((decode(k)?, decode(v)?)))
            .is_some_and(|(k, v)| {
                k == "after_revision"
                    && !v.is_empty()
                    && !v.starts_with('0')
                    && v.bytes().all(|b| b.is_ascii_digit())
                    && v.parse::<i64>()
                        .is_ok_and(|n| (1..=9_007_199_254_740_991).contains(&n))
            });
    }
    let Some((_, query)) = target.split_once('?') else {
        return true;
    };
    if query.is_empty()
        || !matches!(
            route,
            FeatureRoute::ListIntros | FeatureRoute::ListNotifications
        )
    {
        return false;
    }
    let mut fields = BTreeMap::new();
    for pair in query.split('&') {
        let Some((key, value)) = pair.split_once('=') else {
            return false;
        };
        let (Some(key), Some(value)) = (decode(key), decode(value)) else {
            return false;
        };
        if fields.insert(key, value).is_some() {
            return false;
        }
    }
    fields.into_iter().all(|(key, value)| match key.as_str() {
        "limit" => {
            !value.is_empty()
                && value.bytes().all(|b| b.is_ascii_digit())
                && value.parse::<u32>().is_ok_and(|n| (1..=100).contains(&n))
        }
        "cursor" => {
            !value.is_empty()
                && value.len() <= 1024
                && value
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
        }
        "q" if route == FeatureRoute::ListIntros => (1..=100).contains(&value.chars().count()),
        "unread_only" if route == FeatureRoute::ListNotifications => {
            matches!(value.as_str(), "true" | "false")
        }
        _ => false,
    })
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgeStart {
    return_target: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct AgeCallback {
    verification_id: String,
    state_token: String,
    provider_response_token: String,
}

/// Mirror frozen C04/B91 admission without changing the original signed bytes.
/// Typed parsing of the original object rejects duplicate and escaped-alias keys.
pub(super) fn valid_body(route: FeatureRoute, bytes: &[u8]) -> bool {
    if matches!(
        route,
        FeatureRoute::MuralPixel
            | FeatureRoute::MuralErase
            | FeatureRoute::MuralFill
            | FeatureRoute::MuralUndo
    ) {
        return mural_body(route, bytes);
    }
    if matches!(
        route,
        FeatureRoute::MuralAdminControl
            | FeatureRoute::MuralAdminLocks
            | FeatureRoute::MuralAdminRestore
    ) {
        return mural_admin_body(route, bytes);
    }
    if !matches!(
        route,
        FeatureRoute::StartAgeVerification | FeatureRoute::CompleteAgeVerification
    ) {
        return true;
    }
    if bytes.len() > 8192
        || !serde_json::from_slice::<serde_json::Value>(bytes).is_ok_and(|v| v.is_object())
    {
        return false;
    }
    match route {
        FeatureRoute::StartAgeVerification => serde_json::from_slice::<AgeStart>(bytes)
            .is_ok_and(|body| body.return_target == "AGE_PRIVACY"),
        FeatureRoute::CompleteAgeVerification => serde_json::from_slice::<AgeCallback>(bytes)
            .is_ok_and(|body| {
                uuid(&body.verification_id).is_some()
                    && age_token(&body.state_token, 512)
                    && age_token(&body.provider_response_token, 4096)
            }),
        _ => false,
    }
}
fn age_token(value: &str, maximum: usize) -> bool {
    !value.is_empty() && value.chars().count() <= maximum && !value.chars().any(char::is_control)
}

fn decode(value: &str) -> Option<String> {
    let mut bytes = Vec::with_capacity(value.len());
    let mut input = value.bytes();
    while let Some(byte) = input.next() {
        bytes.push(match byte {
            b'+' => b' ',
            b'%' => {
                let hi = (input.next()? as char).to_digit(16)?;
                let lo = (input.next()? as char).to_digit(16)?;
                (hi * 16 + lo) as u8
            }
            other => other,
        });
    }
    let result = String::from_utf8(bytes).ok()?;
    (!result.chars().any(char::is_control)).then_some(result)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn frozen_feature_methods_paths_receipts_and_error_protection_are_exact() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .ancestors()
            .nth(3)
            .unwrap();
        let rows: serde_json::Value = serde_json::from_slice(
            &std::fs::read(root.join("contracts/domain/v1/operations.json")).unwrap(),
        )
        .unwrap();
        let event = Uuid::new_v4().to_string();
        let id = Uuid::new_v4().to_string();
        let mut count = 0;
        for row in rows.as_array().unwrap() {
            let method = Method::from_bytes(
                row["method"]
                    .as_str()
                    .unwrap()
                    .to_ascii_uppercase()
                    .as_bytes(),
            )
            .unwrap();
            let path = row["path"]
                .as_str()
                .unwrap()
                .replace("{eventId}", &event)
                .replace("{introId}", &id)
                .replace("{notificationId}", &id)
                .replace("{idempotencyKey}", &id)
                .replace("{key}", &id);
            if let Some(r) = route(&method, &path) {
                count += 1;
                assert_eq!(r.operation(), row["operationId"].as_str().unwrap());
                assert_eq!(
                    owns_receipt(&method, &path),
                    row["idempotency_key"].as_bool().unwrap()
                );
                assert!(is_product(&path));
                assert!(route(&method, &format!("{path}/extra")).is_none());
                assert!(route(&method, &path.replace(&event, "not-uuid")).is_none());
                assert!(
                    route(
                        &method,
                        &path.replace(&id, "00000000-0000-0000-0000-000000000000")
                    )
                    .is_none()
                        || !path.contains(&id)
                );
                for other in [
                    Method::GET,
                    Method::POST,
                    Method::PUT,
                    Method::PATCH,
                    Method::DELETE,
                ] {
                    if route(&other, &path).is_none() {
                        assert!(!owns_receipt(&other, &path));
                    }
                }
            }
        }
        assert_eq!(count, 27);
    }

    #[test]
    fn venue_location_and_place_notes_families_are_exact_and_closed() {
        use FeatureRoute::*;
        let event = Uuid::new_v4();
        let id = Uuid::new_v4();
        let base = format!("/api/concert/events/{event}/venue");
        let note = format!("{base}/landmarks/lm-main/notes/{id}");
        for (method, path, expected, mutation) in [
            (
                Method::GET,
                format!("{base}/location"),
                VenueLocationSnapshot,
                false,
            ),
            (
                Method::PUT,
                format!("{base}/location/sharing"),
                SetVenueLocationSharing,
                true,
            ),
            (
                Method::PUT,
                format!("{base}/location/fix"),
                PublishVenueLocationFix,
                true,
            ),
            (
                Method::GET,
                format!("{base}/location/commands/{id}"),
                VenueLocationCommand(id),
                false,
            ),
            (
                Method::POST,
                format!("{base}/landmarks/lm-main/notes"),
                CreateVenuePlaceNote,
                true,
            ),
            (
                Method::POST,
                format!("{note}/replies"),
                CreateVenuePlaceReply,
                true,
            ),
            (Method::DELETE, note.clone(), WithdrawVenuePlaceNote, true),
        ] {
            let matched = route(&method, &path).expect("venue family route");
            assert_eq!(matched, expected);
            assert!(protects(&path));
            assert_eq!(super::super::owns_receipt(&method, &path), mutation);
            assert!(valid_target(matched, &path));
            assert!(route(&method, &format!("{path}/extra")).is_none());
            assert!(!valid_target(matched, &format!("{path}?role=OPERATOR")));
        }
        assert!(route(
            &Method::DELETE,
            &format!("{base}/landmarks/lm-main/notes/not-a-uuid")
        )
        .is_none());
        assert!(route(&Method::POST, &format!("{base}/landmarks/bad%2Fid/notes")).is_none());
        assert!(route(&Method::GET, &format!("{base}/location/requests")).is_none());
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MuralAdminControlBody {
    expected_revision: i64,
    action: String,
    reason: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MuralAdminLockBody {
    expected_revision: i64,
    x_min: i32,
    y_min: i32,
    x_max_exclusive: i32,
    y_max_exclusive: i32,
    locked: bool,
    reason: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MuralAdminRestoreBody {
    expected_revision: i64,
    snapshot_id: String,
    reason: String,
}
fn mural_admin_body(route: FeatureRoute, bytes: &[u8]) -> bool {
    if bytes.len() > 8192 {
        return false;
    }
    let revision = |n: i64| (1..=9_007_199_254_740_991).contains(&n);
    let reason = |s: &str| (1..=500).contains(&s.chars().count());
    match route {
        FeatureRoute::MuralAdminControl => serde_json::from_slice::<MuralAdminControlBody>(bytes)
            .is_ok_and(|b| {
                revision(b.expected_revision)
                    && reason(&b.reason)
                    && matches!(b.action.as_str(), "PAUSE" | "RESUME" | "FREEZE")
            }),
        FeatureRoute::MuralAdminLocks => serde_json::from_slice::<MuralAdminLockBody>(bytes)
            .is_ok_and(|b| {
                let _ = b.locked;
                revision(b.expected_revision)
                    && reason(&b.reason)
                    && (0..128).contains(&b.x_min)
                    && (0..128).contains(&b.y_min)
                    && (1..=128).contains(&b.x_max_exclusive)
                    && (1..=128).contains(&b.y_max_exclusive)
                    && b.x_min < b.x_max_exclusive
                    && b.y_min < b.y_max_exclusive
            }),
        FeatureRoute::MuralAdminRestore => serde_json::from_slice::<MuralAdminRestoreBody>(bytes)
            .is_ok_and(|b| {
                revision(b.expected_revision) && reason(&b.reason) && uuid(&b.snapshot_id).is_some()
            }),
        _ => false,
    }
}
#[cfg(test)]
mod mural_admin_admission_tests {
    use super::*;
    #[test]
    fn only_four_frozen_admin_routes_and_three_owned_receipts() {
        let event = uuid::Uuid::new_v4();
        let base = format!("/api/concert/events/{event}/admin/mural");
        for (method, suffix, op) in [
            (Method::GET, "", FeatureRoute::MuralAdmin),
            (Method::POST, "/control", FeatureRoute::MuralAdminControl),
            (Method::POST, "/locks", FeatureRoute::MuralAdminLocks),
            (Method::POST, "/restore", FeatureRoute::MuralAdminRestore),
        ] {
            let path = format!("{base}{suffix}");
            assert_eq!(route(&method, &path), Some(op));
            assert!(protects(&path));
            assert_eq!(super::super::owns_receipt(&method, &path), op.mutation());
            assert!(valid_target(op, &path));
            assert!(!valid_target(op, &format!("{path}?role=OPERATOR")));
            assert!(route(&method, &format!("{path}/alias")).is_none());
        }
    }
    #[test]
    fn closed_admin_bodies_reject_duplicates_authority_and_invalid_half_open_bounds() {
        use FeatureRoute::*;
        assert!(valid_body(
            MuralAdminControl,
            br#"{"expected_revision":1,"action":"PAUSE","reason":"Protect Wall"}"#
        ));
        for b in [br#"{"expected_revision":1,"action":"PAUSE","reason":"x","role":"OPERATOR"}"#.as_slice(),br#"{"expected_revision":1,"action":"PAUSE","reason":"x","expected_settings_revision":1}"#.as_slice(),br#"{"expected_revision":1,"action":"PAUSE","reason":"x","action":"FREEZE"}"#.as_slice()] {assert!(!valid_body(MuralAdminControl,b));}
        assert!(valid_body(MuralAdminLocks,br#"{"expected_revision":1,"x_min":127,"y_min":127,"x_max_exclusive":128,"y_max_exclusive":128,"locked":true,"reason":"Protect last cell"}"#));
        assert!(!valid_body(MuralAdminLocks,br#"{"expected_revision":1,"x_min":128,"y_min":0,"x_max_exclusive":128,"y_max_exclusive":1,"locked":true,"reason":"x"}"#));
        assert!(!valid_body(MuralAdminRestore,br#"{"expected_revision":1,"snapshot_id":"00000000-0000-0000-0000-000000000000","reason":"x"}"#));
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MuralPixelBody {
    expected_revision: i64,
    x: i32,
    y: i32,
    color_index: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MuralEraseBody {
    expected_revision: i64,
    x: i32,
    y: i32,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MuralUndoBody {
    expected_revision: i64,
    target_action_id: String,
}
fn mural_body(route: FeatureRoute, bytes: &[u8]) -> bool {
    if bytes.len() > 1024 {
        return false;
    }
    let revision = |r| (1..=9_007_199_254_740_991i64).contains(&r);
    let position = |x, y| (0..128).contains(&x) && (0..128).contains(&y);
    match route {
        FeatureRoute::MuralPixel | FeatureRoute::MuralFill => serde_json::from_slice::<
            MuralPixelBody,
        >(bytes)
        .is_ok_and(|b| {
            revision(b.expected_revision) && position(b.x, b.y) && (0..16).contains(&b.color_index)
        }),
        FeatureRoute::MuralErase => serde_json::from_slice::<MuralEraseBody>(bytes)
            .is_ok_and(|b| revision(b.expected_revision) && position(b.x, b.y)),
        FeatureRoute::MuralUndo => serde_json::from_slice::<MuralUndoBody>(bytes)
            .is_ok_and(|b| revision(b.expected_revision) && uuid(&b.target_action_id).is_some()),
        _ => false,
    }
}

#[cfg(test)]
mod wall_tests {
    use super::*;
    #[test]
    fn closed_wall_body_and_delta_query_preserve_original_authority_bytes() {
        use FeatureRoute::*;
        assert!(valid_body(
            MuralFill,
            br#"{"expected_revision":1,"x":127,"y":127,"color_index":15}"#
        ));
        for body in [
            br#"{"expected_revision":1,"x":0,"y":0,"color_index":2,"x":1}"#.as_slice(),
            br#"{"expected_revision":1,"x":0,"y":0,"color_index":2,"actor_id":"other"}"#.as_slice(),
            br#"{"expected_revision":1,"x":128,"y":0,"color_index":2}"#.as_slice(),
        ] {
            assert!(!valid_body(MuralFill, body));
        }
        assert!(!valid_body(
            MuralErase,
            br#"{"expected_revision":1,"x":0,"y":0,"color_index":0}"#
        ));
        for query in [
            "",
            "?after_revision=0",
            "?after_revision=1&after_revision=2",
            "?after_revision=+1",
            "?after_revision=1&actor_id=other",
            "?after_revision=9007199254740992",
        ] {
            assert!(!valid_target(MuralDelta, &format!("/wall{query}")));
        }
        assert!(valid_target(
            MuralDelta,
            "/wall?after_revision=9007199254740991"
        ));
        assert!(valid_target(MuralDelta, "/wall?%61fter_revision=%31"));
        assert!(!valid_target(MuralDelta, "/wall?after_revision=01"));
        assert!(!valid_target(
            MuralDelta,
            "/wall?after_revision=%31&%61fter_revision=2"
        ));
        assert!(!valid_target(MuralTools, "/wall?actor_id=other"));
    }
}
