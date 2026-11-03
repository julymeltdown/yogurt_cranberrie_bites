//! Internal repository transfer records. These are not wire or domain types.
use chrono::{DateTime, Utc};
use notification_domain::notification::{Notification, NotificationAuditEvent};
use notification_domain::notification_page::NotificationPage;
use std::fmt::{Debug, Formatter};
use uuid::Uuid;

#[derive(Clone, PartialEq, Eq)]
pub struct NotificationRecordDto {
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

impl From<Notification> for NotificationRecordDto {
    fn from(value: Notification) -> Self {
        Self {
            id: value.id,
            user_id: value.user_id,
            notification_type: value.notification_type,
            payload_json: value.payload_json,
            created_at: value.created_at,
            read_at: value.read_at,
            event_id: value.event_id,
            actor_id: value.actor_id,
            occurred_at: value.occurred_at,
        }
    }
}

impl From<&Notification> for NotificationRecordDto {
    fn from(value: &Notification) -> Self {
        Self {
            id: value.id,
            user_id: value.user_id,
            notification_type: value.notification_type.clone(),
            payload_json: value.payload_json.clone(),
            created_at: value.created_at,
            read_at: value.read_at,
            event_id: value.event_id,
            actor_id: value.actor_id,
            occurred_at: value.occurred_at,
        }
    }
}

impl From<NotificationRecordDto> for Notification {
    fn from(value: NotificationRecordDto) -> Self {
        Self {
            id: value.id,
            user_id: value.user_id,
            notification_type: value.notification_type,
            payload_json: value.payload_json,
            created_at: value.created_at,
            read_at: value.read_at,
            event_id: value.event_id,
            actor_id: value.actor_id,
            occurred_at: value.occurred_at,
        }
    }
}

impl From<&NotificationRecordDto> for Notification {
    fn from(value: &NotificationRecordDto) -> Self {
        Self {
            id: value.id,
            user_id: value.user_id,
            notification_type: value.notification_type.clone(),
            payload_json: value.payload_json.clone(),
            created_at: value.created_at,
            read_at: value.read_at,
            event_id: value.event_id,
            actor_id: value.actor_id,
            occurred_at: value.occurred_at,
        }
    }
}

impl Debug for NotificationRecordDto {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NotificationRecordDto")
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

#[derive(Clone, PartialEq, Eq)]
pub struct NotificationAuditRecordDto {
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

impl From<NotificationAuditEvent> for NotificationAuditRecordDto {
    fn from(value: NotificationAuditEvent) -> Self {
        Self {
            id: value.id,
            actor_id: value.actor_id,
            notification_id: value.notification_id,
            user_id: value.user_id,
            action: value.action,
            outcome: value.outcome,
            request_id: value.request_id,
            ip_address: value.ip_address,
            user_agent: value.user_agent,
            metadata_json: value.metadata_json,
            occurred_at: value.occurred_at,
        }
    }
}

impl From<&NotificationAuditEvent> for NotificationAuditRecordDto {
    fn from(value: &NotificationAuditEvent) -> Self {
        Self {
            id: value.id,
            actor_id: value.actor_id,
            notification_id: value.notification_id,
            user_id: value.user_id,
            action: value.action.clone(),
            outcome: value.outcome.clone(),
            request_id: value.request_id.clone(),
            ip_address: value.ip_address.clone(),
            user_agent: value.user_agent.clone(),
            metadata_json: value.metadata_json.clone(),
            occurred_at: value.occurred_at,
        }
    }
}

impl From<NotificationAuditRecordDto> for NotificationAuditEvent {
    fn from(value: NotificationAuditRecordDto) -> Self {
        Self {
            id: value.id,
            actor_id: value.actor_id,
            notification_id: value.notification_id,
            user_id: value.user_id,
            action: value.action,
            outcome: value.outcome,
            request_id: value.request_id,
            ip_address: value.ip_address,
            user_agent: value.user_agent,
            metadata_json: value.metadata_json,
            occurred_at: value.occurred_at,
        }
    }
}

impl From<&NotificationAuditRecordDto> for NotificationAuditEvent {
    fn from(value: &NotificationAuditRecordDto) -> Self {
        Self {
            id: value.id,
            actor_id: value.actor_id,
            notification_id: value.notification_id,
            user_id: value.user_id,
            action: value.action.clone(),
            outcome: value.outcome.clone(),
            request_id: value.request_id.clone(),
            ip_address: value.ip_address.clone(),
            user_agent: value.user_agent.clone(),
            metadata_json: value.metadata_json.clone(),
            occurred_at: value.occurred_at,
        }
    }
}

impl Debug for NotificationAuditRecordDto {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NotificationAuditRecordDto")
            .field("id", &"[REDACTED]")
            .field("actor_id", &"[REDACTED]")
            .field("notification_id", &"[REDACTED]")
            .field("user_id", &"[REDACTED]")
            .field("action", &"[REDACTED]")
            .field("outcome", &"[REDACTED]")
            .field("request_id", &"[REDACTED]")
            .field("ip_address", &"[REDACTED]")
            .field("user_agent", &"[REDACTED]")
            .field("metadata_json", &"[REDACTED]")
            .field("occurred_at", &"[REDACTED]")
            .finish()
    }
}

#[derive(Clone, PartialEq, Eq)]
pub struct NotificationPageRecordDto {
    pub items: Vec<NotificationRecordDto>,
    pub next_cursor: Option<String>,
}

impl From<NotificationPage> for NotificationPageRecordDto {
    fn from(value: NotificationPage) -> Self {
        Self {
            items: value.items.into_iter().map(Into::into).collect(),
            next_cursor: value.next_cursor,
        }
    }
}

impl From<NotificationPageRecordDto> for NotificationPage {
    fn from(value: NotificationPageRecordDto) -> Self {
        Self {
            items: value.items.into_iter().map(Into::into).collect(),
            next_cursor: value.next_cursor,
        }
    }
}

impl Debug for NotificationPageRecordDto {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NotificationPageRecordDto")
            .field("items_len", &self.items.len())
            .field("next_cursor", &"[REDACTED]")
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::dto::notification_dto::{NotificationDto, NotificationPageDto};
    use crate::notification_contracts::{
        NotificationCreateOutcome, NotificationPosition, NotificationQueryPage,
    };
    use chrono::TimeZone;

    fn time(seconds: i64, nanos: u32) -> DateTime<Utc> {
        Utc.timestamp_opt(seconds, nanos).single().unwrap()
    }

    fn notification(read: bool, actor: bool) -> Notification {
        Notification {
            id: Uuid::from_u128(101),
            user_id: Uuid::from_u128(202),
            notification_type: "SYSTEM-private-type".into(),
            payload_json: " {\"z\":\"private-payload-한글\",\"a\":1} ".into(),
            created_at: time(1_700_000_100, 123_456_789),
            read_at: read.then(|| time(1_700_000_200, 234_567_891)),
            event_id: Uuid::from_u128(303),
            actor_id: actor.then(|| Uuid::from_u128(404)),
            occurred_at: time(1_699_999_900, 908_076_543),
        }
    }

    // This is deliberately specified separately from the domain fixture.
    fn record(read: bool, actor: bool) -> NotificationRecordDto {
        NotificationRecordDto {
            id: Uuid::from_u128(101),
            user_id: Uuid::from_u128(202),
            notification_type: "SYSTEM-private-type".into(),
            payload_json: " {\"z\":\"private-payload-한글\",\"a\":1} ".into(),
            created_at: time(1_700_000_100, 123_456_789),
            read_at: read.then(|| time(1_700_000_200, 234_567_891)),
            event_id: Uuid::from_u128(303),
            actor_id: actor.then(|| Uuid::from_u128(404)),
            occurred_at: time(1_699_999_900, 908_076_543),
        }
    }

    #[test]
    fn notification_records_preserve_distinct_identity_optional_fields_and_nanoseconds() {
        for read in [false, true] {
            for actor in [false, true] {
                let domain = notification(read, actor);
                let transfer = record(read, actor);
                assert_eq!(NotificationRecordDto::from(domain.clone()), transfer);
                assert_eq!(NotificationRecordDto::from(&domain), transfer);
                assert_eq!(Notification::from(transfer.clone()), domain);
                assert_eq!(Notification::from(&transfer), domain);
                let (marked, changed) =
                    Notification::from(transfer).mark_read(time(1_800_000_000, 567_890_123));
                assert_eq!(changed, !read);
                assert_eq!(
                    marked.read_at,
                    Some(if read {
                        time(1_700_000_200, 234_567_891)
                    } else {
                        time(1_800_000_000, 567_890_123)
                    })
                );
            }
        }
    }

    #[test]
    fn audit_records_preserve_context_metadata_and_optional_identity_in_both_directions() {
        for present in [false, true] {
            let domain = NotificationAuditEvent {
                id: Uuid::from_u128(501),
                actor_id: present.then(|| Uuid::from_u128(602)),
                notification_id: present.then(|| Uuid::from_u128(703)),
                user_id: present.then(|| Uuid::from_u128(804)),
                action: "private-action".into(),
                outcome: "private-outcome".into(),
                request_id: present.then(|| "private-request".into()),
                ip_address: present.then(|| "198.51.100.27".into()),
                user_agent: present.then(|| "private-agent".into()),
                metadata_json: " {\"retentionUntil\":\"private-metadata\",\"dryRun\":false} "
                    .into(),
                occurred_at: time(-123, 765_432_109),
            };
            let transfer = NotificationAuditRecordDto {
                id: Uuid::from_u128(501),
                actor_id: present.then(|| Uuid::from_u128(602)),
                notification_id: present.then(|| Uuid::from_u128(703)),
                user_id: present.then(|| Uuid::from_u128(804)),
                action: "private-action".into(),
                outcome: "private-outcome".into(),
                request_id: present.then(|| "private-request".into()),
                ip_address: present.then(|| "198.51.100.27".into()),
                user_agent: present.then(|| "private-agent".into()),
                metadata_json: " {\"retentionUntil\":\"private-metadata\",\"dryRun\":false} "
                    .into(),
                occurred_at: time(-123, 765_432_109),
            };
            assert_eq!(NotificationAuditRecordDto::from(domain.clone()), transfer);
            assert_eq!(NotificationAuditRecordDto::from(&domain), transfer);
            assert_eq!(NotificationAuditEvent::from(transfer.clone()), domain);
            assert_eq!(NotificationAuditEvent::from(&transfer), domain);
            let rendered = format!("{transfer:?}");
            for private in [
                "private-action",
                "private-outcome",
                "private-request",
                "198.51.100.27",
                "private-agent",
                "private-metadata",
                &Uuid::from_u128(501).to_string(),
                &time(-123, 765_432_109).to_rfc3339(),
            ] {
                assert!(!rendered.contains(private), "{rendered}");
            }
        }
    }

    #[test]
    fn pages_preserve_order_cursor_and_existing_wire_null_and_empty_string_rules() {
        for cursor in [None, Some("private-cursor:101".to_owned())] {
            let first = notification(true, false);
            let mut second = notification(false, true);
            second.id = Uuid::from_u128(17);
            let domain = NotificationPage {
                items: vec![first.clone(), second.clone()],
                next_cursor: cursor.clone(),
            };
            let transfer = NotificationPageRecordDto::from(domain.clone());
            assert_eq!(
                transfer
                    .items
                    .iter()
                    .map(|item| item.id)
                    .collect::<Vec<_>>(),
                vec![Uuid::from_u128(101), Uuid::from_u128(17)]
            );
            assert_eq!(transfer.next_cursor, cursor);
            assert_eq!(NotificationPage::from(transfer.clone()), domain);
            let public = NotificationPageDto::from(transfer);
            assert_eq!(
                public.items,
                vec![NotificationDto::from(first), NotificationDto::from(second)]
            );
            assert_eq!(public.next_cursor, cursor);
            let wire = serde_json::to_value(public).unwrap();
            assert_eq!(wire["items"][0]["actorId"], "");
            assert_eq!(wire["items"][1]["readAt"], "");
            assert_eq!(
                wire["nextCursor"],
                cursor.map_or(serde_json::Value::Null, serde_json::Value::String)
            );
        }
        let wire = serde_json::to_value(NotificationDto::from(record(false, false))).unwrap();
        assert_eq!(
            wire["payloadJson"],
            " {\"z\":\"private-payload-한글\",\"a\":1} "
        );
        assert_eq!(wire["createdAt"], "2023-11-14T22:15:00.123456789+00:00");
        assert_eq!(wire["occurredAt"], "2023-11-14T22:11:40.908076543+00:00");
    }

    #[test]
    fn record_and_page_debug_redact_nested_content_and_cursor_values() {
        let transfer = record(true, true);
        let page = NotificationPageRecordDto {
            items: vec![transfer.clone()],
            next_cursor: Some("private-cursor".into()),
        };
        let outcome = NotificationCreateOutcome::Created(transfer.clone());
        let query = NotificationQueryPage {
            items: vec![transfer.clone()],
            next_position: Some(NotificationPosition {
                created_at: time(1_700_000_100, 123_456_789),
                id: Uuid::from_u128(101),
            }),
            legacy_next_cursor: Some("private-cursor".into()),
        };
        for rendered in [
            format!("{transfer:?}"),
            format!("{page:?}"),
            format!("{outcome:?}"),
            format!("{query:?}"),
        ] {
            assert!(rendered.contains("[REDACTED]"));
            for private in [
                "SYSTEM-private-type",
                "private-payload",
                "private-cursor",
                &Uuid::from_u128(101).to_string(),
                &Uuid::from_u128(202).to_string(),
                &Uuid::from_u128(303).to_string(),
                &Uuid::from_u128(404).to_string(),
                &time(1_700_000_100, 123_456_789).to_rfc3339(),
            ] {
                assert!(!rendered.contains(private), "{rendered}");
            }
        }
    }
}
