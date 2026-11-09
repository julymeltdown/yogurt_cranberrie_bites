//! T-403: attachment rules.
//!
//! At most four attachments, no duplicates, each owned by the author, each
//! `READY`, each purpose `INTRO_POST`.
//!
//! The rule that matters most is the one about **foreign ids**. A media id that
//! belongs to somebody else must be rejected with the *same* answer as an id
//! that does not exist at all. Otherwise the composer becomes an existence
//! oracle: an attacker could probe random ids and learn which ones are real
//! assets, and could learn which of their own guesses belong to a particular
//! victim by watching for a different error.
//!
//! [`AttachmentError::Unavailable`] is therefore the single answer for
//! *every* reason an asset cannot be attached: absent, foreign, not `READY`,
//! wrong purpose, or deleted. The caller turns it into one uniform
//! `RESOURCE_UNAVAILABLE`, exactly as the visibility rule does for a denied
//! post.
use uuid::Uuid;

/// The frozen `attachment_media_ids` bound.
pub const MAX_ATTACHMENTS: usize = 4;

/// The purpose an intro attachment must carry.
///
/// This is the same closed set as [`crate::media::Purpose`], and the same values
/// the `profile_media` CHECK constraint admits. It is a re-export rather than a
/// second enum so the two cannot drift: a purpose added to the media domain
/// appears here automatically, and a purpose this module does not expect is
/// still parseable by the caller that needs to reject it.
pub use crate::media::Purpose as MediaPurpose;

impl std::fmt::Display for AttachmentError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "invalid intro input: {self:?}")
    }
}

impl std::error::Error for AttachmentError {}

/// What the store knows about one candidate asset.
///
/// This is the *fact*, not the decision. The store reads the row and fills this
/// in; [`plan`] decides.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AttachmentCandidate {
    pub media_id: Uuid,
    pub owner_profile_id: Uuid,
    pub purpose: MediaPurpose,
    /// `PENDING`, `UPLOADED`, `READY` or `DELETED`, as stored.
    pub ready: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AttachmentError {
    /// More than four ids were supplied.
    TooMany,
    /// The same id appeared twice.
    Duplicate,
    /// The id is nil, which is never an asset.
    NilId,
    /// The asset cannot be attached, for any reason.
    ///
    /// Deliberately undifferentiated: absent, foreign, not `READY`, wrong
    /// purpose and deleted all produce this one value, so the response never
    /// reveals which.
    Unavailable,
}

/// The validated attachment set, in the author's order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AttachmentPlan {
    pub media_ids: Vec<Uuid>,
}

/// Validate a requested attachment set against what the store found.
///
/// `found` is keyed by id. A requested id that is missing from `found` is
/// [`AttachmentError::Unavailable`] — the same answer a foreign id gets, because
/// the caller cannot tell the two apart either.
pub fn plan(
    requested: &[Uuid],
    author: Uuid,
    found: &[AttachmentCandidate],
) -> Result<AttachmentPlan, AttachmentError> {
    if requested.len() > MAX_ATTACHMENTS {
        return Err(AttachmentError::TooMany);
    }
    if requested.iter().any(Uuid::is_nil) {
        return Err(AttachmentError::NilId);
    }
    // Duplicates are refused before the store is consulted, so a repeated id
    // cannot be used to pad the list past the cap.
    let mut seen = std::collections::BTreeSet::new();
    for id in requested {
        if !seen.insert(*id) {
            return Err(AttachmentError::Duplicate);
        }
    }
    let mut plan = Vec::with_capacity(requested.len());
    for id in requested {
        let candidate = found
            .iter()
            .find(|candidate| candidate.media_id == *id)
            .ok_or(AttachmentError::Unavailable)?;
        // Every rejection below is the same value on purpose.
        if candidate.owner_profile_id != author
            || !candidate.ready
            || candidate.purpose != MediaPurpose::IntroPost
        {
            return Err(AttachmentError::Unavailable);
        }
        plan.push(*id);
    }
    Ok(AttachmentPlan { media_ids: plan })
}
