//! Exact C04 operations read routes. A thin authenticated proxy: the product
//! producer still owns role, review and visibility decisions.
use super::*;

pub(super) fn router() -> Router<Arc<AppState>> {
    Router::new()
        .route("/api/concert/events/:event_id/notices", get(dispatch))
        .route(
            "/api/concert/events/:event_id/notices/:notice_id",
            get(dispatch),
        )
        .route(
            "/api/concert/events/:event_id/admin/settings",
            get(dispatch),
        )
        .route("/api/concert/events/:event_id/admin/places", get(dispatch))
        .route("/api/concert/events/:event_id/admin/notices", get(dispatch))
        .route("/api/concert/events/:event_id/admin/reports", get(dispatch))
        .route(
            "/api/concert/events/:event_id/admin/reports/:report_id",
            get(dispatch),
        )
}

fn tail(path: &str) -> Option<(&str, &str)> {
    path.strip_prefix("/api/concert/events/")?.split_once('/')
}

pub(super) fn protects(path: &str) -> bool {
    tail(path).is_some_and(|(_, tail)| {
        tail == "notices"
            || tail.starts_with("notices/")
            || tail == "admin/settings"
            || tail.starts_with("admin/settings/")
            || tail == "admin/places"
            || tail.starts_with("admin/places/")
            || tail == "admin/notices"
            || tail.starts_with("admin/notices/")
            || tail == "admin/reports"
            || tail.starts_with("admin/reports/")
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_operations_read_template_is_exact_and_nonmutating() {
        let event = Uuid::new_v4();
        let id = Uuid::new_v4();
        let root = format!("/api/concert/events/{event}");
        for suffix in [
            "notices".to_owned(),
            format!("notices/{id}"),
            "admin/settings".to_owned(),
            "admin/places".to_owned(),
            "admin/notices".to_owned(),
            "admin/reports".to_owned(),
            format!("admin/reports/{id}"),
        ] {
            let path = format!("{root}/{suffix}");
            assert!(protects(&path), "{suffix}");
            assert!(!owns_receipt(&Method::GET, &path), "{suffix}");
        }
        for suffix in ["notices/not-a-uuid", "admin/reports/not-a-uuid"] {
            let path = format!("{root}/{suffix}");
            assert!(protects(&path), "{suffix}");
            assert!(!owns_receipt(&Method::GET, &path));
        }
    }
}
