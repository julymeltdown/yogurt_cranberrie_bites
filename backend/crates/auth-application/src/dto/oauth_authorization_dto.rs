//! Secret authorization material crossing the outbound security boundary.

#[derive(Clone, PartialEq, Eq)]
pub struct OAuthAuthorizationMaterialDto {
    pub state: String,
    pub nonce: String,
    pub code_verifier: String,
    pub code_challenge: String,
}

impl std::fmt::Debug for OAuthAuthorizationMaterialDto {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("OAuthAuthorizationMaterialDto")
            .field("state", &"[REDACTED]")
            .field("nonce", &"[REDACTED]")
            .field("code_verifier", &"[REDACTED]")
            .field("code_challenge", &"[REDACTED]")
            .finish()
    }
}
