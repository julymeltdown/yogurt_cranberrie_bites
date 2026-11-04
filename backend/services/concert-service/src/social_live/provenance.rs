//! Closed immutable accepted-provenance configuration. Verification-start
//! availability and arbitrary mutable trust providers cannot escape EOF fences.
use crate::relationship::eligibility::{
    ConfiguredSharingVerifier, SharingVerifierStatus, UnavailableSharingVerifier, VerifierStatus,
};
use concert_application::age_policy::Context;
use concert_domain::relationship::RelationshipError;
use sqlx::PgConnection;

pub enum SocialClaimProvenance {
    Unavailable,
    Configured(ConfiguredSharingVerifier),
}
impl SocialClaimProvenance {
    pub fn configured(
        verifier: String,
        source_revision: String,
    ) -> Result<Self, RelationshipError> {
        ConfiguredSharingVerifier::new(verifier, source_revision).map(Self::Configured)
    }
}
#[async_trait::async_trait]
impl SharingVerifierStatus for SocialClaimProvenance {
    async fn current_in(
        &self,
        c: &mut PgConnection,
        request: Context,
    ) -> Result<VerifierStatus, RelationshipError> {
        match self {
            Self::Unavailable => UnavailableSharingVerifier.current_in(c, request).await,
            Self::Configured(verifier) => verifier.current_in(c, request).await,
        }
    }
}
