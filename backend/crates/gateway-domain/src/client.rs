use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct ClientProfile {
    pub id: String,
    pub client_type: String,
    pub allowed_contracts: Vec<String>,
    pub policy_set_id: Option<String>,
}
