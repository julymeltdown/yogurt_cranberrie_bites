//! Explicit actor-scoped physical self-reports, not GPS-verified or virtual presence.
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum RallyState {
    OnMyWay,
    Here,
    Met,
    NeedHelp,
    Left,
}
impl RallyState {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::OnMyWay => "ON_MY_WAY",
            Self::Here => "HERE",
            Self::Met => "MET",
            Self::NeedHelp => "NEED_HELP",
            Self::Left => "LEFT",
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SetRally {
    pub expected_revision: i64,
    pub state: RallyState,
    pub meeting_revision: i64,
    pub physical_presence_acknowledged: bool,
}
impl SetRally {
    pub fn valid(&self) -> bool {
        self.physical_presence_acknowledged
            && (0..crate::MAX_REVISION).contains(&self.expected_revision)
            && (1..=crate::MAX_REVISION).contains(&self.meeting_revision)
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RallyEntry {
    pub user_id: Uuid,
    pub state: RallyState,
    pub meeting_revision: i64,
    pub revision: i64,
    pub self_reported_at: DateTime<Utc>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Rally {
    pub party_id: Uuid,
    pub event_id: Uuid,
    pub meeting_revision: i64,
    /// No implicit latest-beacon selection. A future explicit, authorized binding
    /// is needed to return a beacon ID; the current party meeting has none.
    pub beacon_id: Option<Uuid>,
    pub entries: Vec<RallyEntry>,
}
