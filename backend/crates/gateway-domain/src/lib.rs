pub mod access_token;
pub mod admission;
pub mod aggregation;
pub mod client;
pub mod contract;
pub mod high_risk;
pub mod moderation;
pub mod operation;
pub mod policy;
pub mod profile;
pub mod telemetry;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "SCREAMING_SNAKE_CASE")]
pub enum IdentityProvider {
    Email,
    Google,
    Apple,
    X,
}
