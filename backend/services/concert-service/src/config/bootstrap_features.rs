//! C03 server availability from validated startup composition. These flags are
//! necessary capabilities, never eligibility, age, consent or policy baselines.
use super::{
    bootstrap_runtime::BootstrapRuntime, intro_runtime::IntroRuntimeSettings,
    participant_runtime::ParticipantRuntimeSettings, product_runtime::feed::FeedRuntimeSettings,
    social_startup::SocialStartupSettings, world_runtime::WorldRuntimeSettings,
};
use crate::bootstrap::BootstrapFeatures;
use concert_application::bootstrap::ValidatedProductRelease;
use std::sync::Arc;

/// No bool/environment constructor: actual validated component settings are
/// required. Public DTO flags remain closed and fail closed by default.
pub struct ConfiguredBootstrapFeatures {
    features: BootstrapFeatures,
}
impl ConfiguredBootstrapFeatures {
    pub fn from_settings(
        bootstrap: Option<&BootstrapRuntime>,
        world: Option<&WorldRuntimeSettings>,
        participant: Option<&ParticipantRuntimeSettings>,
        intro: Option<&IntroRuntimeSettings>,
        feed: Option<&FeedRuntimeSettings>,
        social: Option<&SocialStartupSettings>,
    ) -> anyhow::Result<Self> {
        let guarded = bootstrap.is_some();
        let canonical = guarded && participant.is_some();
        let shared = canonical && intro.is_some();
        anyhow::ensure!(
            world.is_none() || guarded,
            "World availability requires guarded bootstrap"
        );
        anyhow::ensure!(
            participant.is_none() || guarded,
            "Mutation availability requires guarded bootstrap"
        );
        anyhow::ensure!(
            intro.is_none() || canonical,
            "Intro availability requires canonical Participant"
        );
        anyhow::ensure!(
            feed.is_none() || shared,
            "Feed availability requires shared canonical Intro"
        );
        // SocialStartupSettings itself requires World and real Notification
        // inbox/target TLS composition before its value can exist.
        anyhow::ensure!(
            social.is_none() || (shared && world.is_some()),
            "Social availability requires shared canonical Intro and World"
        );
        Ok(Self {
            features: BootstrapFeatures {
                world: world.is_some(),
                feed: feed.is_some(),
                social_sharing: social.is_some(),
                mutations: canonical,
                ..BootstrapFeatures::default()
            },
        })
    }
    pub(crate) fn world_enabled(&self) -> bool {
        self.features.world
    }
    pub fn bind_release(
        &self,
        source: Arc<ValidatedProductRelease>,
    ) -> anyhow::Result<Arc<ValidatedProductRelease>> {
        crate::bootstrap::release::with_features(source, &self.features)
            .map_err(|_| anyhow::anyhow!("Configured bootstrap release is unavailable"))
    }
}
