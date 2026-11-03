//! Explicit time/identity and OAuth credential capabilities for Auth use cases.

use crate::dto::oauth_authorization_dto::OAuthAuthorizationMaterialDto;
use chrono::{DateTime, Utc};
use uuid::Uuid;

/// Non-deterministic values used by Auth lifecycle decisions and audit identity.
pub trait AuthRuntimePort: Send + Sync {
    fn now(&self) -> DateTime<Utc>;
    fn new_id(&self) -> Uuid;
}

/// Random protocol credentials and their PKCE encoding belong to security.
pub trait OAuthAuthorizationCryptoPort: Send + Sync {
    fn generate(&self) -> crate::errors::AuthResult<OAuthAuthorizationMaterialDto>;
}
