//! The database half of the composed visibility policy.
//!
//! [`ComposedPolicy`](super::visibility::ComposedPolicy) decides; this module
//! only *resolves facts*. It answers two questions from the concert service's own
//! database and never joins across services:
//!
//! * the stored audience of a post (`intro_posts.audience_*`);
//! * the block relationship between two profiles (`discovery_blocks`, which is
//!   the concert service's local mirror of the friend service's blocks).
//!
//! The profile-to-account mapping used for blocks comes from
//! `discovery_profiles.account_id`, which is the same local mirror discovery
//! already uses. A profile that has never been mirrored has no account here, and
//! an unknown account is treated as "no block" only when *both* sides are known;
//! if either side cannot be resolved the pair is reported as blocked, because a
//! block that cannot be evaluated must not be treated as absent.
//!
//! The claim read is deliberately **not** here: it belongs to
//! [`IdentityClaimsPort`](concert_application::identity_claims::IdentityClaimsPort)
//! and is performed by the adapter in [`super::claims_adapter`].
use async_trait::async_trait;
use concert_application::intro_visibility::BlockState;
use concert_domain::intro_audience::IntroAudience;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::visibility::IntroFacts;

/// The SQL-backed fact resolver.
#[derive(Clone)]
pub struct PgIntroFacts {
    pool: PgPool,
}

impl PgIntroFacts {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl IntroFacts for PgIntroFacts {
    async fn audience(&self, intro_id: Uuid) -> Option<IntroAudience> {
        let row = sqlx::query(
            "select audience_gender, audience_min_age, audience_max_age_exclusive \
             from intro_posts where intro_id = $1",
        )
        .bind(intro_id)
        .fetch_optional(&self.pool)
        .await
        .ok()??;
        let gender: String = row.try_get("audience_gender").ok()?;
        let minimum: Option<i16> = row.try_get("audience_min_age").ok()?;
        let maximum: Option<i16> = row.try_get("audience_max_age_exclusive").ok()?;
        // A stored audience that does not parse is not "unrestricted": an
        // unreadable audience must not widen visibility, so the caller is told
        // the post does not exist and the decision denies.
        IntroAudience::from_parts(&gender, minimum, maximum).ok()
    }

    async fn blocks(&self, viewer: Uuid, author: Uuid) -> BlockState {
        if viewer == author {
            return BlockState::None;
        }
        let row = sqlx::query(
            "select \
               (select account_id from discovery_profiles where profile_id = $1) as viewer_account, \
               (select account_id from discovery_profiles where profile_id = $2) as author_account",
        )
        .bind(viewer)
        .bind(author)
        .fetch_optional(&self.pool)
        .await;
        let Ok(Some(row)) = row else {
            // A store failure is not "no block".
            return BlockState::Mutual;
        };
        let viewer_account: Option<Uuid> = row.try_get("viewer_account").ok().flatten();
        let author_account: Option<Uuid> = row.try_get("author_account").ok().flatten();
        let (Some(viewer_account), Some(author_account)) = (viewer_account, author_account) else {
            // A profile that has never been mirrored has no account, so a block
            // cannot be ruled out. Treating that as "no block" would be the one
            // permissive default this module must not have.
            return BlockState::Mutual;
        };
        let blocked: Option<(bool, bool)> = sqlx::query_as(
            "select \
               exists(select 1 from discovery_blocks \
                      where blocker_account_id = $1 and blocked_account_id = $2 \
                        and deleted_at is null) as viewer_blocked, \
               exists(select 1 from discovery_blocks \
                      where blocker_account_id = $2 and blocked_account_id = $1 \
                        and deleted_at is null) as author_blocked",
        )
        .bind(viewer_account)
        .bind(author_account)
        .fetch_optional(&self.pool)
        .await
        .ok()
        .flatten();
        match blocked {
            Some((true, true)) => BlockState::Mutual,
            Some((true, false)) => BlockState::ViewerBlockedAuthor,
            Some((false, true)) => BlockState::AuthorBlockedViewer,
            Some((false, false)) => BlockState::None,
            None => BlockState::Mutual,
        }
    }
}
