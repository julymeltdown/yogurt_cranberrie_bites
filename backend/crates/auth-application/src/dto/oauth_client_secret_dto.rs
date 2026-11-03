//! JWT wire claims for the outbound Apple client-secret signer.

use auth_domain::oauth_client_secret::AppleClientSecretClaims;

#[derive(serde::Serialize)]
pub struct AppleClientSecretClaimsDto {
    iss: String,
    sub: String,
    aud: String,
    iat: i64,
    exp: i64,
}

impl From<AppleClientSecretClaims> for AppleClientSecretClaimsDto {
    fn from(claims: AppleClientSecretClaims) -> Self {
        Self {
            iss: claims.issuer,
            sub: claims.subject,
            aud: claims.audience,
            iat: claims.issued_at,
            exp: claims.expires_at,
        }
    }
}
