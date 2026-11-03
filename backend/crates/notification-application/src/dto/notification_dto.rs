use notification_domain::notification::Notification;
use notification_domain::notification_page::NotificationPage;
use serde::{Deserialize, Serialize};

macro_rules! redacted_debug {
    ($($type:ident { $($field:ident),+ $(,)? }),+ $(,)?) => {
        $(
            impl std::fmt::Debug for $type {
                fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                    let mut debug = formatter.debug_struct(stringify!($type));
                    $(debug.field(stringify!($field), &"[REDACTED]");)+
                    debug.finish()
                }
            }
        )+
    };
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationDto {
    pub id: String,
    pub user_id: String,
    pub r#type: String,
    pub payload_json: String,
    pub created_at: String,
    pub read_at: String,
    pub event_id: String,
    pub actor_id: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationCreateResultDto {
    pub notification: Option<NotificationDto>,
    pub suppressed: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationPageDto {
    pub items: Vec<NotificationDto>,
    pub next_cursor: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NotificationCreateDto {
    pub user_id: String,
    pub r#type: String,
    pub payload_json: String,
    pub event_id: String,
    pub actor_id: String,
    pub occurred_at: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MarkReadResultDto {
    pub notification: NotificationDto,
    pub updated: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteNotificationDto {
    pub notification_id: String,
    pub user_id: String,
    pub actor_id: String,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeleteNotificationResponseDto {
    pub notification_id: String,
    pub deleted: bool,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeExpiredNotificationsDto {
    pub actor_id: String,
    pub request_id: Option<String>,
    pub ip_address: Option<String>,
    pub user_agent: Option<String>,
    pub dry_run: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PurgeExpiredNotificationsResponseDto {
    pub purged_count: i64,
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyUserDeletionDto {
    pub saga_id: String,
    pub user_id: String,
    pub actor_id: String,
    pub retention_until: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ApplyUserDeletionResponseDto {
    pub applied: bool,
}

impl From<Notification> for NotificationDto {
    fn from(value: Notification) -> Self {
        Self {
            id: value.id.to_string(),
            user_id: value.user_id.to_string(),
            r#type: value.notification_type,
            payload_json: value.payload_json,
            created_at: value.created_at.to_rfc3339(),
            read_at: value
                .read_at
                .map(|value| value.to_rfc3339())
                .unwrap_or_default(),
            event_id: value.event_id.to_string(),
            actor_id: value
                .actor_id
                .map(|value| value.to_string())
                .unwrap_or_default(),
            occurred_at: value.occurred_at.to_rfc3339(),
        }
    }
}

impl From<NotificationPage> for NotificationPageDto {
    fn from(value: NotificationPage) -> Self {
        Self {
            items: value.items.into_iter().map(Into::into).collect(),
            next_cursor: value.next_cursor,
        }
    }
}

redacted_debug! {
    NotificationDto {
        id,
        user_id,
        r#type,
        payload_json,
        created_at,
        read_at,
        event_id,
        actor_id,
        occurred_at,
    },
    NotificationCreateDto { user_id, r#type, payload_json, event_id, actor_id, occurred_at },
    DeleteNotificationDto {
        notification_id,
        user_id,
        actor_id,
        request_id,
        ip_address,
        user_agent,
    },
    PurgeExpiredNotificationsDto { actor_id, request_id, ip_address, user_agent, dry_run },
    ApplyUserDeletionDto { saga_id, user_id, actor_id, retention_until },
}

impl std::fmt::Debug for NotificationPageDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("NotificationPageDto")
            .field("items_len", &self.items.len())
            .field("next_cursor", &"[REDACTED]")
            .finish()
    }
}

impl std::fmt::Debug for DeleteNotificationResponseDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("DeleteNotificationResponseDto")
            .field("notification_id", &"[REDACTED]")
            .field("deleted", &self.deleted)
            .finish()
    }
}

impl From<super::repository_dto::NotificationRecordDto> for NotificationDto {
    fn from(value: super::repository_dto::NotificationRecordDto) -> Self {
        Notification::from(value).into()
    }
}

impl From<super::repository_dto::NotificationPageRecordDto> for NotificationPageDto {
    fn from(value: super::repository_dto::NotificationPageRecordDto) -> Self {
        NotificationPage::from(value).into()
    }
}

#[cfg(test)]
mod secret_debug_tests {
    use super::*;

    #[test]
    fn notification_dto_debug_redacts_payloads_ids_and_cursors_without_changing_serde() {
        let notification = NotificationDto {
            id: "private-notification-id".into(),
            user_id: "private-notification-user".into(),
            r#type: "private-notification-type".into(),
            payload_json: r#"{"secret":"private-notification-payload"}"#.into(),
            created_at: "private-created-at".into(),
            read_at: "private-read-at".into(),
            event_id: "private-event-id".into(),
            actor_id: "private-actor-id".into(),
            occurred_at: "private-occurred-at".into(),
        };
        let create = NotificationCreateDto {
            user_id: "private-create-user".into(),
            r#type: "private-create-type".into(),
            payload_json: "private-create-payload".into(),
            event_id: "private-create-event".into(),
            actor_id: "private-create-actor".into(),
            occurred_at: "private-create-time".into(),
        };
        let page = NotificationPageDto {
            items: vec![notification.clone()],
            next_cursor: Some("private-notification-cursor".into()),
        };
        let create_result = NotificationCreateResultDto {
            notification: Some(notification.clone()),
            suppressed: false,
        };
        let mark_read = MarkReadResultDto {
            notification: notification.clone(),
            updated: true,
        };
        let delete = DeleteNotificationDto {
            notification_id: "private-delete-notification-id".into(),
            user_id: "private-delete-user".into(),
            actor_id: "private-delete-actor".into(),
            request_id: Some("private-delete-request".into()),
            ip_address: Some("private-delete-ip".into()),
            user_agent: Some("private-delete-agent".into()),
        };
        let delete_response = DeleteNotificationResponseDto {
            notification_id: "private-delete-response-id".into(),
            deleted: true,
        };
        let purge = PurgeExpiredNotificationsDto {
            actor_id: "private-purge-actor".into(),
            request_id: Some("private-purge-request".into()),
            ip_address: Some("private-purge-ip".into()),
            user_agent: Some("private-purge-agent".into()),
            dry_run: true,
        };
        let apply_deletion = ApplyUserDeletionDto {
            saga_id: "private-deletion-saga".into(),
            user_id: "private-deletion-user".into(),
            actor_id: "private-deletion-actor".into(),
            retention_until: "private-retention-date".into(),
        };

        let rendered = [
            format!("{notification:?}"),
            format!("{create:?}"),
            format!("{page:?}"),
            format!("{create_result:?}"),
            format!("{mark_read:?}"),
            format!("{delete:?}"),
            format!("{delete_response:?}"),
            format!("{purge:?}"),
            format!("{apply_deletion:?}"),
        ]
        .join("\n");
        for private_value in [
            "private-notification-id",
            "private-notification-user",
            "private-notification-type",
            "private-notification-payload",
            "private-created-at",
            "private-read-at",
            "private-event-id",
            "private-actor-id",
            "private-occurred-at",
            "private-create-user",
            "private-create-type",
            "private-create-payload",
            "private-create-event",
            "private-create-actor",
            "private-create-time",
            "private-notification-cursor",
            "private-delete-notification-id",
            "private-delete-user",
            "private-delete-actor",
            "private-delete-request",
            "private-delete-ip",
            "private-delete-agent",
            "private-delete-response-id",
            "private-purge-actor",
            "private-purge-request",
            "private-purge-ip",
            "private-purge-agent",
            "private-deletion-saga",
            "private-deletion-user",
            "private-deletion-actor",
            "private-retention-date",
        ] {
            assert!(!rendered.contains(private_value), "{rendered}");
        }
        assert!(rendered.contains("items_len: 1"), "{rendered}");
        assert!(rendered.contains("[REDACTED]"), "{rendered}");

        let serialized = serde_json::to_value(&notification).expect("serialize Notification DTO");
        assert_eq!(serialized["payloadJson"], notification.payload_json);
        assert_eq!(serialized["userId"], notification.user_id);
    }
}
