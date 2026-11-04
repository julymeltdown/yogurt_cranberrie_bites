//! Exact C04 feature admission. Canonical owners still authorize every request.
use super::*;
use std::collections::BTreeMap;

pub(super) fn router() -> Router<Arc<AppState>> {
    Router::new()
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
            Self::SetIntro
                | Self::PublishIntro
                | Self::CloseIntro
                | Self::MarkRead
                | Self::MarkAllRead
        )
    }
    pub(super) fn body_limit(self) -> usize {
        match self {
            Self::SetIntro | Self::PublishIntro | Self::CloseIntro => 16 * 1024,
            Self::MarkRead | Self::MarkAllRead => 8192,
            _ => 0,
        }
    }
    #[cfg(test)]
    fn operation(self) -> &'static str {
        match self {
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
        tail == "intros"
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
/// Body schemas, receipts and visibility remain the owning Root's decisions.
pub(super) fn valid_target(route: FeatureRoute, target: &str) -> bool {
    if target.len() > 4096 || target.contains('#') || target.chars().any(char::is_control) {
        return false;
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
                .replace("{notificationId}", &id);
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
        assert_eq!(count, 10);
    }
}
