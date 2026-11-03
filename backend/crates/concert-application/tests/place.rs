use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::place::*;
use concert_application::Clock;
use concert_domain::place::{
    CatalogRelease, ClosureState, Place, PlaceId, PlaceKind, VerificationState,
};
use concert_domain::{EventId, Revision};
use std::str::FromStr;
use uuid::Uuid;

const PLACE: &str = "00000000-0000-4000-8000-000000000001";
fn at() -> DateTime<Utc> {
    Utc::now()
}
struct Fixed;
impl Clock for Fixed {
    fn now(&self) -> DateTime<Utc> {
        at()
    }
}
#[derive(Clone)]
struct Policy {
    answer: Result<PlaceGrant, PlaceError>,
    operation: PlaceOperation,
    calls: usize,
}
#[async_trait]
impl PlaceAuthorization for Policy {
    async fn authorize(
        &self,
        access: PlaceAccess,
        operation: PlaceOperation,
        _: DateTime<Utc>,
    ) -> Result<PlaceGrant, PlaceError> {
        if access.event_id
            != self
                .answer
                .as_ref()
                .map(|grant| grant.event_id)
                .unwrap_or(access.event_id)
            || operation != self.operation
        {
            return Err(PlaceError::Denied);
        }
        self.answer.clone()
    }
}
fn snapshot(event: EventId, revision: i64, map: i64) -> PlaceSnapshot {
    PlaceSnapshot::from_db(
        event,
        Revision::new(revision).unwrap(),
        ("map_fixture".into(), map),
    )
    .unwrap()
}
struct Repo {
    event: EventId,
    published: Result<Option<CatalogRead>, PlaceError>,
    detail: Option<(PlaceSnapshot, Place)>,
    select: SelectionOutcome,
    calls: usize,
}
#[async_trait]
impl PlaceRepository for Repo {
    async fn catalog(&self, _: EventId) -> Result<Option<CatalogRead>, PlaceError> {
        self.published.clone()
    }
    async fn detail(
        &self,
        _: EventId,
        _: PlaceId,
    ) -> Result<Option<(PlaceSnapshot, Place)>, PlaceError> {
        Ok(self.detail.clone())
    }
    async fn publish_release(
        &self,
        event: EventId,
        _: CatalogRelease,
    ) -> Result<PlaceSnapshot, PlaceError> {
        Ok(snapshot(event, 1, 1))
    }
    async fn select_physical(
        &self,
        event: EventId,
        place: PlaceId,
        cat: Revision,
        map: i64,
    ) -> Result<SelectionOutcome, PlaceError> {
        let _ = (event, place, cat, map);
        Ok(self.select.clone())
    }
}
const RAW_FIXTURE: &[u8] =
    include_bytes!("../../../../contracts/fixtures/assets/map-manifest.json");
fn manifest() -> concert_domain::place::Manifest {
    concert_domain::place::parse_stored_release(&canonical_fixture()).unwrap()
}
fn catalog_read(event: EventId) -> CatalogRead {
    let manifest = manifest();
    CatalogRead {
        snapshot: snapshot(event, 1, 1),
        release: CatalogRelease {
            map_id: "map_fixture".into(),
            manifest: manifest.clone(),
            canonical_bytes: canonical_fixture(),
        },
        places: manifest.places,
    }
}
fn canonical_fixture() -> Vec<u8> {
    concert_domain::place::canonical_json_bytes(RAW_FIXTURE).unwrap()
}
fn policy(operation: PlaceOperation, event_id: EventId) -> Policy {
    Policy {
        answer: Ok(PlaceGrant {
            event_id,
            operation,
        }),
        operation,
        calls: 0,
    }
}
fn event() -> EventId {
    EventId::from_uuid(Uuid::from_u128(2))
}

#[tokio::test]
async fn authorization_denial_or_unavailable_never_reads_store() {
    for error in [PlaceError::Denied, PlaceError::PermissionUnavailable] {
        let repo = Repo {
            event: event(),
            published: Err(PlaceError::InvalidState),
            detail: None,
            select: SelectionOutcome::NotFound,
            calls: 0,
        };
        let places = Places::new(
            repo,
            Fixed,
            Policy {
                answer: Err(error),
                operation: PlaceOperation::Read,
                calls: 0,
            },
        );
        assert_eq!(places.catalog(event()).await.unwrap_err(), error);
    }
}
#[tokio::test]
async fn published_catalog_is_event_bound_and_has_no_fabricated_lease() {
    let repo = Repo {
        event: event(),
        published: Ok(Some(catalog_read(event()))),
        detail: None,
        select: SelectionOutcome::NotFound,
        calls: 0,
    };
    let places = Places::new(repo, Fixed, policy(PlaceOperation::Read, event()));
    let c = places.catalog(event()).await.unwrap();
    assert_eq!(c.snapshot.event_id, event());
    assert_eq!(c.snapshot.map_revision, 1);
    let mut foreign = catalog_read(event());
    foreign.snapshot.event_id = EventId::from_uuid(Uuid::from_u128(3));
    let repo = Repo {
        event: event(),
        published: Ok(Some(foreign)),
        detail: None,
        select: SelectionOutcome::NotFound,
        calls: 0,
    };
    assert_eq!(
        Places::new(repo, Fixed, policy(PlaceOperation::Read, event()))
            .catalog(event())
            .await
            .unwrap_err(),
        PlaceError::InvalidState
    );
}
#[tokio::test]
async fn catalog_revision_and_map_revision_stay_distinct() {
    let mut read = catalog_read(event());
    read.snapshot = snapshot(event(), 7, 1);
    let repo = Repo {
        event: event(),
        published: Ok(Some(read)),
        detail: None,
        select: SelectionOutcome::NotFound,
        calls: 0,
    };
    let places = Places::new(repo, Fixed, policy(PlaceOperation::Read, event()));
    let catalog = places.catalog(event()).await.unwrap();
    assert_eq!(catalog.snapshot.revision, Revision::new(7).unwrap());
    assert_eq!(catalog.snapshot.map_revision, 1);
}
#[tokio::test]
async fn selection_requires_exact_current_map_and_refuses_concept_unverified_closed_stale() {
    let event = event();
    let place_id = PlaceId::from_str(PLACE).unwrap();
    for (outcome, error) in [
        (SelectionOutcome::NotFound, PlaceError::NotFound),
        (SelectionOutcome::Closed, PlaceError::Ineligible),
        (SelectionOutcome::ConceptOnly, PlaceError::Ineligible),
        (SelectionOutcome::Unverified, PlaceError::Ineligible),
        (SelectionOutcome::StaleCatalog, PlaceError::Conflict),
        (SelectionOutcome::MapMismatch, PlaceError::MapMismatch),
    ] {
        let repo = Repo {
            event,
            published: Ok(None),
            detail: None,
            select: outcome,
            calls: 0,
        };
        let places = Places::new(repo, Fixed, policy(PlaceOperation::SelectPhysical, event));
        assert_eq!(
            places
                .select_physical(event, place_id, Revision::initial(), 1)
                .await
                .unwrap_err(),
            error
        );
    }
}
#[tokio::test]
async fn detail_rejects_cross_event_and_wrong_place_responses() {
    let manifest = concert_domain::place::parse_stored_release(&canonical_fixture()).unwrap();
    let wanted = manifest.places[0].id;
    for (snapshot_event, place) in [
        (
            EventId::from_uuid(Uuid::from_u128(3)),
            manifest.places[0].clone(),
        ),
        (
            event(),
            Place::new(
                PlaceId::from_str("00000000-0000-4000-8000-000000000002").unwrap(),
                "Wrong fixture place".into(),
                PlaceKind::VirtualConcept,
                VerificationState::ConceptOnly,
                ClosureState::Open,
                "Synthetic concept only.".into(),
                manifest.places[0].provenance.clone(),
            )
            .unwrap(),
        ),
    ] {
        let repo = Repo {
            event: event(),
            published: Ok(None),
            detail: Some((snapshot(snapshot_event, 1, 1), place)),
            select: SelectionOutcome::NotFound,
            calls: 0,
        };
        assert_eq!(
            Places::new(repo, Fixed, policy(PlaceOperation::Read, event()))
                .detail(event(), wanted)
                .await
                .unwrap_err(),
            PlaceError::InvalidState
        );
    }
}
