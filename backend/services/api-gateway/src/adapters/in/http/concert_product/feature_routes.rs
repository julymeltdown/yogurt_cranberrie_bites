//! Exact C04 feature admission. Canonical owners still authorize every request.
use super::*;
use serde::Deserialize;
use std::collections::BTreeMap;

pub(super) fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/concert/events/:event_id/mural/snapshot", get(dispatch))
        .route("/api/concert/events/:event_id/mural/operations", get(dispatch))
        .route("/api/concert/events/:event_id/mural/pixels", post(dispatch))
        .route("/api/concert/events/:event_id/mural/commands/:key/result", get(dispatch))
        .route("/api/concert/events/:event_id/mural/tools", get(dispatch))
        .route("/api/concert/events/:event_id/mural/actions/:key", get(dispatch))
        .route("/api/concert/events/:event_id/mural/erase", post(dispatch))
        .route("/api/concert/events/:event_id/mural/fill", post(dispatch))
        .route("/api/concert/events/:event_id/mural/undo", post(dispatch))
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
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum FeatureRoute {
    MuralSnapshot, MuralDelta, MuralPixel, MuralPixelResult, MuralTools, MuralActionResult, MuralErase, MuralFill, MuralUndo,
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
}
impl FeatureRoute {
    pub(super) fn mutation(self) -> bool {
        matches!(
            self,
            Self::MuralPixel | Self::MuralErase | Self::MuralFill | Self::MuralUndo | Self::StartAgeVerification
                | Self::CompleteAgeVerification
                | Self::SetIntro
                | Self::PublishIntro
                | Self::CloseIntro
                | Self::MarkRead
                | Self::MarkAllRead
        )
    }
    pub(super) fn body_limit(self) -> usize {
        match self {
            Self::MuralPixel | Self::MuralErase | Self::MuralFill | Self::MuralUndo => 1024,
            Self::SetIntro | Self::PublishIntro | Self::CloseIntro => 16 * 1024,
            Self::MarkRead
            | Self::MarkAllRead
            | Self::StartAgeVerification
            | Self::CompleteAgeVerification => 8192,
            _ => 0,
        }
    }
    #[cfg(test)]
    fn operation(self) -> &'static str {
        match self {
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
        }
    }
}

fn tail(path: &str) -> Option<(&str, &str)> {
    path.strip_prefix("/api/concert/events/")?.split_once('/')
}
pub(super) fn protects(path: &str) -> bool {
    tail(path).is_some_and(|(_, tail)| {
        tail == "mural" || tail.starts_with("mural/") || tail == "me/age-status"
            || tail.starts_with("me/age-status/")
            || tail == "me/age-verifications"
            || tail.starts_with("me/age-verifications/")
            || tail == "intros"
            || tail.starts_with("intros/")
            || tail == "me/intro"
            || tail.starts_with("me/intro/")
            || tail == "me/notifications"
            || tail.starts_with("me/notifications/")
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
        ("GET", ["mural", "snapshot"]) => Some(MuralSnapshot),
        ("GET", ["mural", "operations"]) => Some(MuralDelta),
        ("POST", ["mural", "pixels"]) => Some(MuralPixel),
        ("GET", ["mural", "commands", key, "result"]) if uuid(key).is_some() => Some(MuralPixelResult),
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
        _ => None,
    }
}

/// Validate the frozen query without normalizing the signed target or body.
/// Receipts, proof trust and visibility remain the owning Root's decisions.
pub(super) fn valid_target(route: FeatureRoute, target: &str) -> bool {
    if target.len() > 4096 || target.contains('#') || target.chars().any(char::is_control) {
        return false;
    }
    if route == FeatureRoute::MuralDelta {
        return target.split_once('?').and_then(|(_,q)| q.strip_prefix("after_revision=")).is_some_and(|v| !v.is_empty() && (v.len()==1 || !v.starts_with('0')) && v.bytes().all(|b|b.is_ascii_digit()) && v.parse::<i64>().is_ok_and(|n|(1..=9_007_199_254_740_991).contains(&n)));
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
    if matches!(route, FeatureRoute::MuralPixel | FeatureRoute::MuralErase | FeatureRoute::MuralFill | FeatureRoute::MuralUndo) { return mural_body(route,bytes); }
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
        assert_eq!(count, 22);
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MuralPixelBody { expected_revision:i64, x:i32, y:i32, color_index:i32 }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MuralEraseBody { expected_revision:i64, x:i32, y:i32 }
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MuralUndoBody { expected_revision:i64, target_action_id:String }
fn mural_body(route:FeatureRoute,bytes:&[u8])->bool {
    if bytes.len()>1024 {return false;}
    let revision=|r| (1..=9_007_199_254_740_991i64).contains(&r);
    let position=|x,y| (0..128).contains(&x)&&(0..128).contains(&y);
    match route {
        FeatureRoute::MuralPixel|FeatureRoute::MuralFill=>serde_json::from_slice::<MuralPixelBody>(bytes).is_ok_and(|b|revision(b.expected_revision)&&position(b.x,b.y)&&(0..16).contains(&b.color_index)),
        FeatureRoute::MuralErase=>serde_json::from_slice::<MuralEraseBody>(bytes).is_ok_and(|b|revision(b.expected_revision)&&position(b.x,b.y)),
        FeatureRoute::MuralUndo=>serde_json::from_slice::<MuralUndoBody>(bytes).is_ok_and(|b|revision(b.expected_revision)&&uuid(&b.target_action_id).is_some()),
        _=>false,
    }
}

#[cfg(test)]
mod wall_tests {
    use super::*;
    #[test]
    fn closed_wall_body_and_delta_query_preserve_original_authority_bytes(){
        use FeatureRoute::*;
        assert!(valid_body(MuralFill,br#"{"expected_revision":1,"x":127,"y":127,"color_index":15}"#));
        for body in [br#"{"expected_revision":1,"x":0,"y":0,"color_index":2,"x":1}"#.as_slice(),br#"{"expected_revision":1,"x":0,"y":0,"color_index":2,"actor_id":"other"}"#.as_slice(),br#"{"expected_revision":1,"x":128,"y":0,"color_index":2}"#.as_slice()] {assert!(!valid_body(MuralFill,body));}
        assert!(!valid_body(MuralErase,br#"{"expected_revision":1,"x":0,"y":0,"color_index":0}"#));
        for query in ["", "?after_revision=0", "?after_revision=1&after_revision=2", "?after_revision=+1", "?after_revision=1&actor_id=other", "?after_revision=9007199254740992"]{assert!(!valid_target(MuralDelta,&format!("/wall{query}")));}
        assert!(valid_target(MuralDelta,"/wall?after_revision=9007199254740991"));
        assert!(!valid_target(MuralTools,"/wall?actor_id=other"));
    }
}
