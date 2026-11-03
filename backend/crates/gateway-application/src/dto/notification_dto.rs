use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NotificationResponse {
    pub id: String,
    pub user_id: String,
    #[serde(rename = "type")]
    pub notification_type: String,
    pub payload_json: String,
    pub created_at: String,
    pub read_at: String,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NotificationFeedResponse {
    pub items: Vec<NotificationResponse>,
    pub next_cursor: Option<String>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct NotificationMarkAllReadResponse {
    pub updated_count: i64,
}
