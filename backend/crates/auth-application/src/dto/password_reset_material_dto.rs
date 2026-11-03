/// Private reset material crossing the outbound security boundary.
#[derive(Clone, PartialEq, Eq)]
pub struct PasswordResetMaterialDto {
    pub token: String,
    pub fingerprint: String,
}

impl std::fmt::Debug for PasswordResetMaterialDto {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("PasswordResetMaterialDto")
            .field("token", &"[REDACTED]")
            .field("fingerprint", &"[REDACTED]")
            .finish()
    }
}
