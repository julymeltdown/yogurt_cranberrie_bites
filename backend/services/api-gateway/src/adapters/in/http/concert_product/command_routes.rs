//! Exact C04 command templates. Authority and behavior stay with the Concert producer.
use super::*;

pub(super) fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route(
            "/api/concert/events/:event_id/parties/:party_id/owner",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/parties/:party_id/rally/me",
            put(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/beacons",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/beacons/:beacon_id",
            axum::routing::patch(dispatch).delete(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/giveaways",
            get(dispatch).post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/giveaways/:giveaway_id",
            get(dispatch).patch(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/giveaways/:giveaway_id/management",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/giveaways/:giveaway_id/pickup-meeting",
            put(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/giveaways/:giveaway_id/reservations",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/reservations/:reservation_id",
            get(dispatch).delete(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/reservations/:reservation_id/pickup",
            get(dispatch),
        )
        .route("/api/concert/events/:event_id/me/giveaways", get(dispatch))
        .route(
            "/api/concert/events/:event_id/reservations/:reservation_id/pickup-code",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/reservations/:reservation_id/claim",
            post(dispatch),
        )
        .route("/api/concert/events/:event_id/posts", post(dispatch))
        .route(
            "/api/concert/events/:event_id/comments/:comment_id",
            axum::routing::patch(dispatch).delete(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/posts/:post_id/reaction",
            put(dispatch).delete(dispatch),
        )
        .route("/api/concert/events/:event_id/reports", post(dispatch))
        .route(
            "/api/concert/events/:event_id/admin/settings",
            put(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/admin/phase",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/admin/service-policy",
            put(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/admin/places/:place_id",
            put(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/admin/notices",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/admin/notices/:notice_id",
            put(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/admin/reports/:report_id/decision",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/admin/users/:user_id/moderation",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/admin/content/moderation",
            post(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/admin/closure",
            post(dispatch),
        )
}

fn tail(path: &str) -> Option<(&str, &str)> {
    path.strip_prefix("/api/concert/events/")?.split_once('/')
}

pub(super) fn protects(path: &str) -> bool {
    tail(path).is_some_and(|(_, tail)| {
        tail == "beacons"
            || tail.starts_with("beacons/")
            || tail == "giveaways"
            || tail.starts_with("giveaways/")
            || tail == "me/giveaways"
            || tail.starts_with("reservations/")
            || tail == "reports"
            || tail == "admin/settings"
            || tail == "admin/phase"
            || tail == "admin/service-policy"
            || tail.starts_with("admin/places/")
            || tail == "admin/notices"
            || tail.starts_with("admin/notices/")
            || tail.starts_with("admin/reports/")
            || tail.starts_with("admin/users/")
            || tail == "admin/content/moderation"
            || tail == "admin/closure"
            || tail.starts_with("comments/")
            || tail == "posts"
            || tail.starts_with("posts/")
            || tail.starts_with("parties/")
    })
}

pub(super) fn owns(method: &Method, path: &str) -> bool {
    let Some((event, tail)) = tail(path) else {
        return false;
    };
    if uuid(event).is_none() {
        return false;
    }
    let pieces: Vec<_> = tail.split('/').collect();
    let id = |value: &str| uuid(value).is_some();
    match (method.as_str(), pieces.as_slice()) {
        ("PATCH", ["parties", party]) if id(party) => true,
        ("POST", ["parties", party, "owner"]) if id(party) => true,
        ("PUT", ["parties", party, "rally", "me"]) if id(party) => true,
        ("POST", ["beacons"]) => true,
        ("PATCH" | "DELETE", ["beacons", beacon]) if id(beacon) => true,
        ("POST", ["giveaways"]) => true,
        ("PATCH", ["giveaways", giveaway]) if id(giveaway) => true,
        ("PUT", ["giveaways", giveaway, "pickup-meeting"]) if id(giveaway) => true,
        ("POST", ["giveaways", giveaway, "reservations"]) if id(giveaway) => true,
        ("DELETE", ["reservations", reservation]) if id(reservation) => true,
        ("POST", ["reservations", reservation, "pickup-code" | "claim"]) if id(reservation) => {
            true
        }
        ("POST", ["posts"]) => true,
        ("PATCH" | "DELETE", ["posts", post]) if id(post) => true,
        ("POST", ["posts", post, "comments"]) if id(post) => true,
        ("PATCH" | "DELETE", ["comments", comment]) if id(comment) => true,
        ("PUT" | "DELETE", ["posts", post, "reaction"]) if id(post) => true,
        ("POST", ["reports"]) => true,
        ("PUT", ["admin", "settings"]) => true,
        ("POST", ["admin", "phase"]) => true,
        ("PUT", ["admin", "service-policy"]) => true,
        ("PUT", ["admin", "places", place]) if id(place) => true,
        ("POST", ["admin", "notices"]) => true,
        ("PUT", ["admin", "notices", notice]) if id(notice) => true,
        ("POST", ["admin", "reports", report, "decision"]) if id(report) => true,
        ("POST", ["admin", "users", user, "moderation"]) if id(user) => true,
        ("POST", ["admin", "content", "moderation"]) => true,
        ("POST", ["admin", "closure"]) => true,
        _ => false,
    }
}
