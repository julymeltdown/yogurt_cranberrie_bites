//! Closed C04 request decoding. Parsed input carries no owner authority.
use super::proto;
use crate::product_rpc::proto::ProductRequest;
use serde::Deserialize;
use std::collections::BTreeMap;
use tonic::Status;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NotificationOperation {
    List,
    UnreadCount,
    MarkRead(Uuid),
    MarkAllRead,
}
impl NotificationOperation {
    pub fn id(self) -> &'static str {
        match self {
            Self::List => "listConcertNotifications",
            Self::UnreadCount => "getConcertNotificationUnreadCount",
            Self::MarkRead(_) => "markConcertNotification",
            Self::MarkAllRead => "markAllConcertNotifications",
        }
    }
    pub(crate) fn wire(self) -> proto::InboxOperation {
        match self {
            Self::List => proto::InboxOperation::List,
            Self::UnreadCount => proto::InboxOperation::UnreadCount,
            Self::MarkRead(_) => proto::InboxOperation::MarkRead,
            Self::MarkAllRead => proto::InboxOperation::MarkAllRead,
        }
    }
    pub(crate) fn mutation(self) -> bool {
        matches!(self, Self::MarkRead(_) | Self::MarkAllRead)
    }
    pub fn route(event: Uuid, method: &str, target: &str) -> Result<Self, Status> {
        if event.is_nil() || target.len() > 4096 || target.chars().any(char::is_control) {
            return Err(invalid());
        }
        let path = target.split_once('?').map_or(target, |v| v.0);
        let base = format!("/api/concert/events/{event}/me/notifications");
        Ok(match (method, path) {
            ("GET", path) if path == base => Self::List,
            ("GET", path) if path == format!("{base}/unread-count") => Self::UnreadCount,
            ("PATCH", path) if path == format!("{base}/read-all") => Self::MarkAllRead,
            ("PATCH", path) => Self::MarkRead(canonical(
                path.strip_prefix(&format!("{base}/"))
                    .and_then(|v| v.strip_suffix("/read"))
                    .ok_or_else(invalid)?,
            )?),
            _ => return Err(invalid()),
        })
    }
}

pub struct NotificationRequest {
    pub(crate) operation: NotificationOperation,
    pub(crate) input: proto::execute_request::Input,
}
impl NotificationRequest {
    pub fn operation(&self) -> NotificationOperation {
        self.operation
    }

    pub fn parse(request: &ProductRequest) -> Result<Self, Status> {
        canonical(&request.user_id)?;
        canonical(&request.session_family_id)?;
        canonical(&request.request_id)?;
        let event = canonical(&request.event_id)?;
        if request.http_body.len() > 8192 {
            return Err(invalid());
        }
        let operation =
            NotificationOperation::route(event, &request.http_method, &request.http_target)?;
        let mut query = query(&request.http_target)?;
        let key = if operation.mutation() {
            if request.content_type != "application/json" || request.http_body.is_empty() {
                return Err(invalid());
            }
            Some(canonical(&request.idempotency_key)?)
        } else {
            if !request.http_body.is_empty()
                || !request.content_type.is_empty()
                || !request.idempotency_key.is_empty()
            {
                return Err(invalid());
            }
            None
        };
        use proto::execute_request::Input;
        let input = match operation {
            NotificationOperation::List => {
                let limit = query
                    .remove("limit")
                    .map(|v| {
                        if v.is_empty() || !v.bytes().all(|b| b.is_ascii_digit()) {
                            return Err(invalid());
                        }
                        v.parse::<u32>().map_err(|_| invalid())
                    })
                    .transpose()?
                    .unwrap_or(20);
                if !(1..=100).contains(&limit) {
                    return Err(invalid());
                }
                let unread_only = query
                    .remove("unread_only")
                    .map(|v| match v.as_str() {
                        "true" => Ok(true),
                        "false" => Ok(false),
                        _ => Err(invalid()),
                    })
                    .transpose()?
                    .unwrap_or(false);
                let cursor = query.remove("cursor");
                if cursor.as_ref().is_some_and(|v| !valid_cursor(v)) {
                    return Err(invalid());
                }
                Input::List(proto::ListInput {
                    limit,
                    unread_only,
                    cursor,
                })
            }
            NotificationOperation::UnreadCount => Input::Count(proto::CountInput {}),
            NotificationOperation::MarkRead(id) => {
                let body: Revision =
                    serde_json::from_slice(&request.http_body).map_err(|_| invalid())?;
                if !(1..=concert_domain::MAX_REVISION).contains(&body.expected_revision) {
                    return Err(invalid());
                }
                Input::MarkRead(proto::MarkReadInput {
                    notification_id: id.to_string(),
                    expected_revision: body.expected_revision,
                    idempotency_key: key.ok_or_else(invalid)?.to_string(),
                })
            }
            NotificationOperation::MarkAllRead => {
                let _: Empty = serde_json::from_slice(&request.http_body).map_err(|_| invalid())?;
                Input::MarkAll(proto::MarkAllInput {
                    idempotency_key: key.ok_or_else(invalid)?.to_string(),
                })
            }
        };
        if !query.is_empty() {
            return Err(invalid());
        }
        Ok(Self { operation, input })
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Revision {
    expected_revision: i64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Empty {}
pub(crate) fn canonical(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .ok()
        .filter(|id| !id.is_nil() && id.to_string() == value)
        .ok_or_else(invalid)
}
pub(crate) fn valid_cursor(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 1024
        && value
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-'))
}
fn query(target: &str) -> Result<BTreeMap<String, String>, Status> {
    let mut fields = BTreeMap::new();
    let value = target.split_once('?').map_or("", |v| v.1);
    if value.is_empty() {
        return Ok(fields);
    }
    for pair in value.split('&') {
        let (name, value) = pair.split_once('=').ok_or_else(invalid)?;
        if fields.insert(decode(name)?, decode(value)?).is_some() {
            return Err(invalid());
        }
    }
    Ok(fields)
}
fn decode(value: &str) -> Result<String, Status> {
    fn hex(b: u8) -> Option<u8> {
        match b {
            b'0'..=b'9' => Some(b - b'0'),
            b'a'..=b'f' => Some(b - b'a' + 10),
            b'A'..=b'F' => Some(b - b'A' + 10),
            _ => None,
        }
    }
    let mut bytes = Vec::with_capacity(value.len());
    let mut input = value.bytes();
    while let Some(byte) = input.next() {
        bytes.push(match byte {
            b'+' => b' ',
            b'%' => {
                input.next().and_then(hex).ok_or_else(invalid)? * 16
                    + input.next().and_then(hex).ok_or_else(invalid)?
            }
            b => b,
        });
    }
    let result = String::from_utf8(bytes).map_err(|_| invalid())?;
    if result.chars().any(char::is_control) {
        return Err(invalid());
    }
    Ok(result)
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid Notification product request")
}
