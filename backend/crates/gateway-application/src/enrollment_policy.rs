//! MCR enrollment transport port. Canonical identity and consent remain Auth-owned.
use async_trait::async_trait;
use serde_json::Value;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EnrollmentOperation {
    Policy,
    Begin,
    Signup,
    Verify,
    Resend,
    Admission,
    Consent,
}
impl EnrollmentOperation {
    pub fn success_status(self) -> u16 {
        match self {
            Self::Begin => 201,
            Self::Signup | Self::Resend => 202,
            _ => 200,
        }
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EnrollmentFailure {
    pub status: u16,
    pub reason: &'static str,
}
impl EnrollmentFailure {
    pub const UNAVAILABLE: Self = Self {
        status: 503,
        reason: "DEPENDENCY_UNAVAILABLE",
    };
    pub const INVALID_RESPONSE: Self = Self {
        status: 502,
        reason: "DEPENDENCY_UNAVAILABLE",
    };
}
#[async_trait]
pub trait EnrollmentPolicyPort: Send + Sync {
    async fn execute(
        &self,
        operation: EnrollmentOperation,
        binding: Option<&str>,
        actor: Option<Uuid>,
        body: Value,
    ) -> Result<Value, EnrollmentFailure>;
}
