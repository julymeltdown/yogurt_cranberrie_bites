use chrono::{DateTime, Utc};
use std::fmt::{Debug, Formatter};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub struct Notification {
    pub id: Uuid,
    pub user_id: Uuid,
    pub notification_type: String,
    pub payload_json: String,
    pub created_at: DateTime<Utc>,
    pub read_at: Option<DateTime<Utc>>,
    pub event_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub occurred_at: DateTime<Utc>,
}

impl Debug for Notification {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("Notification")
            .field("id", &"[REDACTED]")
            .field("user_id", &"[REDACTED]")
            .field("notification_type", &"[REDACTED]")
            .field("payload_json", &"[REDACTED]")
            .field("created_at", &"[REDACTED]")
            .field("read_at", &"[REDACTED]")
            .field("event_id", &"[REDACTED]")
            .field("actor_id", &"[REDACTED]")
            .field("occurred_at", &"[REDACTED]")
            .finish()
    }
}

impl Notification {
    pub fn mark_read(mut self, read_at: DateTime<Utc>) -> (Self, bool) {
        if self.read_at.is_some() {
            return (self, false);
        }

        self.read_at = Some(read_at);
        (self, true)
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct NotificationAuditEvent {
    pub id: Uuid,
    pub actor_id: Option<Uuid>,
    pub notification_id: Option<Uuid>,
    pub user_id: Option<Uuid>,
    pub action: String,
    pub outcome: String,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub metadata_json: String,
    pub occurred_at: DateTime<Utc>,
}

impl Debug for NotificationAuditEvent {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NotificationAuditEvent")
            .field("id", &"[REDACTED]")
            .field("actor_id", &"[REDACTED]")
            .field("notification_id", &"[REDACTED]")
            .field("user_id", &"[REDACTED]")
            .field("action", &self.action)
            .field("outcome", &self.outcome)
            .field("request_id", &"[REDACTED]")
            .field("ip_address", &"[REDACTED]")
            .field("user_agent", &"[REDACTED]")
            .field("metadata_json", &"[REDACTED]")
            .field("occurred_at", &self.occurred_at)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn notification_debug_redacts_payload_and_personal_identifiers() {
        let notification = Notification {
            id: Uuid::new_v4(),
            user_id: Uuid::new_v4(),
            notification_type: "private-notification-type-marker".into(),
            payload_json: r#"{"content":"private-notification-payload-marker"}"#.into(),
            created_at: Utc::now(),
            read_at: Some(Utc::now()),
            event_id: Uuid::new_v4(),
            actor_id: Some(Uuid::new_v4()),
            occurred_at: Utc::now(),
        };

        let rendered = format!("{notification:?}");
        for private_value in [
            notification.id.to_string(),
            notification.user_id.to_string(),
            notification.event_id.to_string(),
            notification.actor_id.unwrap().to_string(),
            "private-notification-type-marker".to_owned(),
            "private-notification-payload-marker".to_owned(),
        ] {
            assert!(!rendered.contains(&private_value), "{rendered}");
        }
        assert!(rendered.contains("Notification"));
        assert!(rendered.contains("[REDACTED]"));
    }

    #[test]
    fn notification_audit_debug_redacts_identifiers_and_request_metadata() {
        let event = NotificationAuditEvent {
            id: Uuid::new_v4(),
            actor_id: Some(Uuid::new_v4()),
            notification_id: Some(Uuid::new_v4()),
            user_id: Some(Uuid::new_v4()),
            action: "DELETE_NOTIFICATION".into(),
            outcome: "SUCCESS".into(),
            request_id: Some("private-notification-request-marker".into()),
            ip_address: Some("198.51.100.53".into()),
            user_agent: Some("private-notification-agent-marker".into()),
            metadata_json: r#"{"private":"notification-audit-marker"}"#.into(),
            occurred_at: Utc::now(),
        };

        let rendered = format!("{event:?}");
        for private_value in [
            event.id.to_string(),
            event.actor_id.unwrap().to_string(),
            event.notification_id.unwrap().to_string(),
            event.user_id.unwrap().to_string(),
            "private-notification-request-marker".to_owned(),
            "198.51.100.53".to_owned(),
            "private-notification-agent-marker".to_owned(),
            "notification-audit-marker".to_owned(),
        ] {
            assert!(!rendered.contains(&private_value), "{rendered}");
        }
        assert!(rendered.contains("DELETE_NOTIFICATION"));
        assert!(rendered.contains("SUCCESS"));
        assert!(rendered.contains("[REDACTED]"));
    }
}
