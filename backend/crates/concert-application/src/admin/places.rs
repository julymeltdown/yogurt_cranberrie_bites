//! Place mutation stays with the map/place owner, in the caller's transaction.
use crate::commands::CommandError;
use concert_domain::{place::Place, Revision};
#[derive(Clone, Debug)]
pub struct PlaceChange {
    pub place: Place,
    pub expected: Revision,
    pub reason: String,
}
impl PlaceChange {
    pub fn validate(&self) -> Result<(), CommandError> {
        super::event::reason(&self.reason)?;
        let p = &self.place;
        let v = &p.provenance;
        let provenance = concert_domain::place::Provenance::new(
            v.source_id.clone(),
            v.source_kind,
            v.source_revision.clone(),
            v.source_uri.clone(),
            v.author.clone(),
            v.license.clone(),
            v.review_state,
            v.reviewed_by.clone(),
            v.reviewed_at,
            v.photo_permission,
        )
        .map_err(|_| CommandError::InvalidInput)?;
        concert_domain::place::Place::new(
            p.id,
            p.label.clone(),
            p.kind,
            p.verification,
            p.closure,
            p.accessibility_notes.clone(),
            provenance,
        )
        .map_err(|_| CommandError::InvalidInput)?;
        self.expected
            .next()
            .map_err(|_| CommandError::InvalidInput)?;
        Ok(())
    }
}
