//! The one decoder from a `profile_media` row to attachment facts.
//!
//! Three callers used to read the same four columns and decode them three
//! slightly different ways:
//!
//! * the intro store defaulted an unparseable purpose to `ProfilePost`;
//! * the intro command defaulted it the same way but defaulted a missing state
//!   to `DELETED`;
//! * the profile-post command decoded the row inline with a third set of
//!   defaults.
//!
//! A defaulted purpose is the dangerous one: the purpose is what decides whether
//! an asset may cross from the intro surface to the profile surface (V-06-d). A
//! decoder that guesses `ProfilePost` for an unreadable row would let a
//! mis-associated asset be attached where it does not belong.
//!
//! This reader defaults nothing that matters. A row whose identity or purpose
//! cannot be read is **not a candidate**, which is exactly how an absent asset
//! behaves: the pure rule answers `Unavailable` for both, so a caller cannot
//! tell them apart.
use concert_application::intro_posts::attachments::{AttachmentCandidate, MediaPurpose};
use sqlx::PgConnection;
use uuid::Uuid;

/// Read the `profile_media` rows for a requested attachment set.
///
/// `requested` is the ids the caller wants to attach. The query is by id only:
/// it never filters by owner, because ownership is the pure rule's decision, and
/// it must answer the same way for a foreign id as for an absent one.
pub async fn candidates_in(
    connection: &mut PgConnection,
    requested: &[Uuid],
) -> Result<Vec<AttachmentCandidate>, sqlx::Error> {
    if requested.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(
        "select media_id,owner_profile_id,purpose,state from profile_media where media_id = any($1)",
    )
    .bind(requested)
    .fetch_all(connection)
    .await?;
    Ok(rows
        .iter()
        .filter_map(|row| {
            use sqlx::Row;
            // A row whose identity or purpose cannot be read is not a candidate:
            // it must behave exactly like an absent asset.
            let media_id = row.try_get::<Uuid, _>("media_id").ok()?;
            let owner_profile_id = row.try_get::<Uuid, _>("owner_profile_id").ok()?;
            let purpose = row
                .try_get::<String, _>("purpose")
                .ok()
                .and_then(|value| MediaPurpose::parse(&value).ok())?;
            let ready = row
                .try_get::<String, _>("state")
                .is_ok_and(|state| state == "READY");
            Some(AttachmentCandidate {
                media_id,
                owner_profile_id,
                purpose,
                ready,
            })
        })
        .collect())
}
