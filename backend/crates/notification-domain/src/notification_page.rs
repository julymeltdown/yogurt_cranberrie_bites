use crate::notification::Notification;
use std::fmt::{Debug, Formatter};

#[derive(Clone, PartialEq, Eq)]
pub struct NotificationPage {
    pub items: Vec<Notification>,
    pub next_cursor: Option<String>,
}

impl Debug for NotificationPage {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NotificationPage")
            .field("items_len", &self.items.len())
            .field("next_cursor", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;
    use uuid::Uuid;

    #[test]
    fn notification_page_debug_redacts_cursor_and_nested_items() {
        let page = NotificationPage {
            items: vec![Notification {
                id: Uuid::new_v4(),
                user_id: Uuid::new_v4(),
                notification_type: "private-page-notification-marker".into(),
                payload_json: "private-page-payload-marker".into(),
                created_at: Utc::now(),
                read_at: None,
                event_id: Uuid::new_v4(),
                actor_id: None,
                occurred_at: Utc::now(),
            }],
            next_cursor: Some("private-page-cursor-marker".into()),
        };

        let rendered = format!("{page:?}");
        assert!(rendered.contains("items_len: 1"), "{rendered}");
        assert!(rendered.contains("[REDACTED]"), "{rendered}");
        assert!(!rendered.contains("private-page-notification-marker"));
        assert!(!rendered.contains("private-page-payload-marker"));
        assert!(!rendered.contains("private-page-cursor-marker"));
    }
}
