/// Private challenge material crossing the outbound security boundary.
#[derive(Clone, PartialEq, Eq)]
pub struct VerificationCodeDto {
    pub code: String,
}

impl std::fmt::Debug for VerificationCodeDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("VerificationCodeDto")
            .field("code", &"[REDACTED]")
            .finish()
    }
}
