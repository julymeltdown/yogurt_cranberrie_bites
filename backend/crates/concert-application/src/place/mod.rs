//! Event-scoped place reads and an atomic checked-current selection seam.
use crate::Clock;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_domain::place::{CatalogRelease, Place, PlaceId, MAX_PLACES};
use concert_domain::{EventId, Revision, MAX_REVISION};

#[derive(Clone, Copy, Debug, PartialEq, Eq, thiserror::Error)]
pub enum PlaceError {
    #[error("place is unavailable")]
    NotFound,
    #[error("map release is no longer current")]
    MapMismatch,
    #[error("place cannot be selected for a physical beacon")]
    Ineligible,
    #[error("place snapshot changed")]
    Conflict,
    #[error("place access is not authorized")]
    Denied,
    #[error("place authority is unavailable")]
    PermissionUnavailable,
    #[error("place storage is unavailable")]
    Unavailable,
    #[error("place storage returned invalid data")]
    InvalidState,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceAccess {
    pub event_id: EventId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlaceOperation {
    Read,
    SelectPhysical,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceGrant {
    pub event_id: EventId,
    pub operation: PlaceOperation,
}

#[async_trait]
pub trait PlaceAuthorization: Send + Sync {
    /// Current event-scoped authorization; UNAVAILABLE is never an allow.
    async fn authorize(
        &self,
        access: PlaceAccess,
        operation: PlaceOperation,
        at: DateTime<Utc>,
    ) -> Result<PlaceGrant, PlaceError>;
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceSnapshot {
    pub event_id: EventId,
    pub revision: Revision,
    pub map_id: String,
    pub map_revision: i64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceCatalog {
    pub snapshot: PlaceSnapshot,
    pub places: Vec<Place>,
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PlaceDetail {
    pub snapshot: PlaceSnapshot,
    pub place: Place,
}

/// Canonical release bytes plus closed, validated place rows from that release.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CatalogRead {
    pub snapshot: PlaceSnapshot,
    pub release: CatalogRelease,
    pub places: Vec<Place>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SelectionOutcome {
    Eligible(PlaceDetail),
    NotFound,
    Closed,
    ConceptOnly,
    Unverified,
    StaleCatalog,
    MapMismatch,
}

#[async_trait]
pub trait PlaceRepository: Send + Sync {
    async fn catalog(&self, event_id: EventId) -> Result<Option<CatalogRead>, PlaceError>;
    async fn detail(
        &self,
        event_id: EventId,
        place_id: PlaceId,
    ) -> Result<Option<(PlaceSnapshot, Place)>, PlaceError>;
    /// Administrator composition persists release members and initial current state.
    async fn publish_release(
        &self,
        event_id: EventId,
        release: CatalogRelease,
    ) -> Result<PlaceSnapshot, PlaceError>;
    /// The repository checks closure and catalog/map identity in the same lock.
    async fn select_physical(
        &self,
        event_id: EventId,
        place_id: PlaceId,
        expected_catalog: Revision,
        expected_map: i64,
    ) -> Result<SelectionOutcome, PlaceError>;
}

pub struct Places<R, C, A> {
    repository: R,
    clock: C,
    authorization: A,
}
impl<R: PlaceRepository, C: Clock, A: PlaceAuthorization> Places<R, C, A> {
    pub fn new(repository: R, clock: C, authorization: A) -> Self {
        Self {
            repository,
            clock,
            authorization,
        }
    }
    async fn grant(&self, event_id: EventId, operation: PlaceOperation) -> Result<(), PlaceError> {
        let access = PlaceAccess { event_id };
        let at = self.clock.now();
        let grant = self
            .authorization
            .authorize(access, operation, at)
            .await
            .map_err(|e| match e {
                PlaceError::Denied | PlaceError::PermissionUnavailable => e,
                _ => PlaceError::PermissionUnavailable,
            })?;
        if grant
            != (PlaceGrant {
                event_id,
                operation,
            })
        {
            return Err(PlaceError::Denied);
        }
        Ok(())
    }
    pub async fn catalog(&self, event_id: EventId) -> Result<PlaceCatalog, PlaceError> {
        self.grant(event_id, PlaceOperation::Read).await?;
        let read = self
            .repository
            .catalog(event_id)
            .await?
            .ok_or(PlaceError::NotFound)?;
        if !catalog_valid(&read, event_id) {
            return Err(PlaceError::InvalidState);
        }
        Ok(PlaceCatalog {
            snapshot: read.snapshot,
            places: read.places,
        })
    }
    pub async fn detail(
        &self,
        event_id: EventId,
        place_id: PlaceId,
    ) -> Result<PlaceDetail, PlaceError> {
        self.grant(event_id, PlaceOperation::Read).await?;
        let (snapshot, place) = self
            .repository
            .detail(event_id, place_id)
            .await?
            .ok_or(PlaceError::NotFound)?;
        if snapshot.event_id != event_id || place.id != place_id {
            return Err(PlaceError::InvalidState);
        }
        Ok(PlaceDetail { snapshot, place })
    }
    /// Returns a checked-current, event-bound destination or a typed refusal.
    pub async fn select_physical(
        &self,
        event_id: EventId,
        place_id: PlaceId,
        expected_catalog: Revision,
        expected_map: i64,
    ) -> Result<PlaceDetail, PlaceError> {
        self.grant(event_id, PlaceOperation::SelectPhysical).await?;
        match self
            .repository
            .select_physical(event_id, place_id, expected_catalog, expected_map)
            .await?
        {
            SelectionOutcome::Eligible(detail) => {
                if detail.snapshot.event_id != event_id
                    || detail.snapshot.revision != expected_catalog
                    || detail.snapshot.map_revision != expected_map
                    || detail.place.id != place_id
                    || !detail.place.permits_new_physical_selection()
                {
                    Err(PlaceError::InvalidState)
                } else {
                    Ok(detail)
                }
            }
            SelectionOutcome::NotFound => Err(PlaceError::NotFound),
            SelectionOutcome::Closed
            | SelectionOutcome::ConceptOnly
            | SelectionOutcome::Unverified => Err(PlaceError::Ineligible),
            SelectionOutcome::StaleCatalog => Err(PlaceError::Conflict),
            SelectionOutcome::MapMismatch => Err(PlaceError::MapMismatch),
        }
    }
}

impl PlaceSnapshot {
    pub fn from_db(
        event_id: EventId,
        revision: Revision,
        map: (String, i64),
    ) -> Result<Self, PlaceError> {
        if !(1..=MAX_REVISION).contains(&map.1) {
            return Err(PlaceError::InvalidState);
        }
        Ok(Self {
            event_id,
            revision,
            map_id: map.0,
            map_revision: map.1,
        })
    }
}

fn catalog_valid(read: &CatalogRead, event_id: EventId) -> bool {
    if read.snapshot.event_id != event_id
        || read.snapshot.map_id != read.release.map_id
        || read.snapshot.map_revision != read.release.manifest.map_revision
        || read.release.manifest.places.len() > MAX_PLACES
        || read.places.len() != read.release.manifest.places.len()
    {
        return false;
    }
    let mut manifest = read.release.manifest.places.iter().collect::<Vec<_>>();
    let mut current = read.places.iter().collect::<Vec<_>>();
    manifest.sort_by_key(|place| place.id);
    current.sort_by_key(|place| place.id);
    manifest.len() == current.len()
        && manifest.into_iter().zip(current).all(|(release, place)| {
            release.id == place.id
                && release.label == place.label
                && release.kind == place.kind
                && release.verification == place.verification
                && release.accessibility_notes == place.accessibility_notes
                && release.provenance == place.provenance
        })
}
