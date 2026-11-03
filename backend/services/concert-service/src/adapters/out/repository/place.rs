//! Native map-release persistence. No HTTP route, identity policy or public DTO.
use async_trait::async_trait;
use chrono::SecondsFormat;
use concert_application::place::{
    CatalogRead, PlaceError, PlaceRepository, PlaceSnapshot, SelectionOutcome,
};
use concert_domain::place::{
    CatalogRelease, ClosureState, PhotoPermission, Place, PlaceId, PlaceKind, Provenance,
    ReviewState, SourceKind, VerificationState,
};
use concert_domain::{EventId, Revision, MAX_REVISION};
use serde_json::{Map, Value};
use sqlx::{postgres::PgRow, PgPool, Postgres, Row, Transaction};

#[derive(Clone)]
pub struct PgPlaceRepository {
    pool: PgPool,
}
impl PgPlaceRepository {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    /// Check the current binding and place while retaining both SHARE locks in
    /// the caller's transaction. BE09 uses this seam and commits after its
    /// beacon insert, making physical selection and beacon creation atomic.
    pub async fn select_physical_in_transaction(
        transaction: &mut Transaction<'_, Postgres>,
        event_id: EventId,
        place_id: PlaceId,
        expected_catalog: Revision,
        expected_map: i64,
    ) -> Result<SelectionOutcome, PlaceError> {
        if !(1..=MAX_REVISION).contains(&expected_map) {
            return Err(PlaceError::InvalidState);
        }
        let Some(head) = sqlx::query("select event_id,catalog_revision,map_id,map_revision from public.concert_events_map_releases where event_id=$1 for share")
            .bind(event_id.uuid())
            .fetch_optional(&mut **transaction)
            .await
            .map_err(db)?
        else {
            return Ok(SelectionOutcome::NotFound);
        };
        if revision(head.try_get("catalog_revision").map_err(invalid)?)? != expected_catalog {
            return Ok(SelectionOutcome::StaleCatalog);
        }
        let map_id: String = head.try_get("map_id").map_err(invalid)?;
        let current_map: i64 = head.try_get("map_revision").map_err(invalid)?;
        if current_map != expected_map {
            return Ok(SelectionOutcome::MapMismatch);
        }
        let row = sqlx::query(&format!(
            "{PLACE_SELECT} where q.map_id=$1 and q.map_revision=$2 and q.place_id=$3 for share of q,s"
        ))
        .bind(&map_id)
        .bind(expected_map)
        .bind(place_id.uuid())
        .fetch_optional(&mut **transaction)
        .await
        .map_err(db)?;
        let Some(place_row) = row else {
            return Ok(SelectionOutcome::NotFound);
        };
        let value = place(&place_row)?;
        Ok(match (value.kind, value.verification, value.closure) {
            (_, _, ClosureState::Closed) => SelectionOutcome::Closed,
            (PlaceKind::VirtualConcept, _, _) => SelectionOutcome::ConceptOnly,
            (PlaceKind::Physical, VerificationState::Unverified, _) => SelectionOutcome::Unverified,
            (PlaceKind::Physical, VerificationState::Verified, ClosureState::Open) => {
                SelectionOutcome::Eligible(concert_application::place::PlaceDetail {
                    snapshot: snapshot(&head)?,
                    place: value,
                })
            }
            _ => return Err(PlaceError::InvalidState),
        })
    }
}

fn db(error: sqlx::Error) -> PlaceError {
    match error.as_database_error().and_then(|value| value.code()) {
        Some(code) if code.as_ref() == "23505" => PlaceError::Conflict,
        _ => PlaceError::Unavailable,
    }
}
fn invalid(_: sqlx::Error) -> PlaceError {
    PlaceError::InvalidState
}
fn revision(value: i64) -> Result<Revision, PlaceError> {
    Revision::new(value).map_err(|_| PlaceError::InvalidState)
}
fn snapshot(row: &PgRow) -> Result<PlaceSnapshot, PlaceError> {
    PlaceSnapshot::from_db(
        EventId::from_uuid(row.try_get("event_id").map_err(invalid)?),
        revision(row.try_get("catalog_revision").map_err(invalid)?)?,
        (
            row.try_get("map_id").map_err(invalid)?,
            row.try_get("map_revision").map_err(invalid)?,
        ),
    )
}

fn object(value: &Value) -> Result<&Map<String, Value>, PlaceError> {
    value.as_object().ok_or(PlaceError::InvalidState)
}
fn nullable_string(object: &Map<String, Value>, key: &str) -> Result<Option<String>, PlaceError> {
    match object.get(key) {
        Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        _ => Err(PlaceError::InvalidState),
    }
}
fn text(object: &Map<String, Value>, key: &str) -> Result<String, PlaceError> {
    object
        .get(key)
        .and_then(Value::as_str)
        .map(str::to_owned)
        .ok_or(PlaceError::InvalidState)
}

fn provenance(row: &PgRow) -> Result<Provenance, PlaceError> {
    let raw: Value = row.try_get("provenance").map_err(invalid)?;
    let object = object(&raw)?;
    const FIELDS: [&str; 10] = [
        "source_id",
        "source_kind",
        "source_revision",
        "source_uri",
        "author",
        "license",
        "review_state",
        "reviewed_by",
        "reviewed_at",
        "photo_permission",
    ];
    if object.len() != FIELDS.len() || FIELDS.iter().any(|key| !object.contains_key(*key)) {
        return Err(PlaceError::InvalidState);
    }
    let source_kind: SourceKind = text(object, "source_kind")?
        .parse()
        .map_err(|_| PlaceError::InvalidState)?;
    let review_state: ReviewState = text(object, "review_state")?
        .parse()
        .map_err(|_| PlaceError::InvalidState)?;
    let source_uri = nullable_string(object, "source_uri")?;
    if source_kind.as_str() != row.try_get::<String, _>("source_kind").map_err(invalid)?
        || source_uri
            != row
                .try_get::<Option<String>, _>("source_uri")
                .map_err(invalid)?
        || review_state.as_str() != row.try_get::<String, _>("review_state").map_err(invalid)?
    {
        return Err(PlaceError::InvalidState);
    }
    let reviewed_at = nullable_string(object, "reviewed_at")?
        .map(|value| value.parse().map_err(|_| PlaceError::InvalidState))
        .transpose()?;
    let photo_permission: PhotoPermission = text(object, "photo_permission")?
        .parse()
        .map_err(|_| PlaceError::InvalidState)?;
    Provenance::new(
        text(object, "source_id")?,
        source_kind,
        text(object, "source_revision")?,
        source_uri,
        text(object, "author")?,
        text(object, "license")?,
        review_state,
        nullable_string(object, "reviewed_by")?,
        reviewed_at,
        photo_permission,
    )
    .map_err(|_| PlaceError::InvalidState)
}

fn place(row: &PgRow) -> Result<Place, PlaceError> {
    let kind: PlaceKind = row
        .try_get::<String, _>("place_kind")
        .map_err(invalid)?
        .parse()
        .map_err(|_| PlaceError::InvalidState)?;
    let verification: VerificationState = row
        .try_get::<String, _>("verification_state")
        .map_err(invalid)?
        .parse()
        .map_err(|_| PlaceError::InvalidState)?;
    let closure: ClosureState = row
        .try_get::<String, _>("closure_state")
        .map_err(invalid)?
        .parse()
        .map_err(|_| PlaceError::InvalidState)?;
    Place::new(
        PlaceId::from_uuid(row.try_get("place_id").map_err(invalid)?)
            .map_err(|_| PlaceError::InvalidState)?,
        row.try_get("label").map_err(invalid)?,
        kind,
        verification,
        closure,
        row.try_get("accessibility_notes").map_err(invalid)?,
        provenance(row)?,
    )
    .map_err(|_| PlaceError::InvalidState)
}

const PLACE_SELECT: &str = "select q.map_id,q.map_revision,q.place_id,q.label,q.place_kind,q.verification_state,q.accessibility_notes,q.source_kind,q.source_uri,q.review_state,q.provenance,s.closure_state from public.concert_map_release_places q join public.concert_place_current_state s on s.map_id=q.map_id and s.map_revision=q.map_revision and s.place_id=q.place_id";

fn provenance_json(value: &Provenance) -> Value {
    let reviewed_at = value
        .reviewed_at
        .map(|at| Value::String(at.to_rfc3339_opts(SecondsFormat::AutoSi, true)))
        .unwrap_or(Value::Null);
    serde_json::json!({
        "source_id": value.source_id,
        "source_kind": value.source_kind.as_str(),
        "source_revision": value.source_revision,
        "source_uri": value.source_uri,
        "author": value.author,
        "license": value.license,
        "review_state": value.review_state.as_str(),
        "reviewed_by": value.reviewed_by,
        "reviewed_at": reviewed_at,
        "photo_permission": value.photo_permission.as_str(),
    })
}

#[async_trait]
impl PlaceRepository for PgPlaceRepository {
    async fn catalog(&self, event_id: EventId) -> Result<Option<CatalogRead>, PlaceError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        sqlx::query("set local transaction isolation level repeatable read, read only")
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        let Some(head) = sqlx::query("select event_id,catalog_revision,map_id,map_revision from public.concert_events_map_releases where event_id=$1")
            .bind(event_id.uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(db)?
        else {
            return Ok(None);
        };
        let map_id: String = head.try_get("map_id").map_err(invalid)?;
        let map_revision: i64 = head.try_get("map_revision").map_err(invalid)?;
        let row = sqlx::query("select map_id,map_revision,content_hash,geometry_hash,canonical_bytes from public.concert_map_releases where map_id=$1 and map_revision=$2")
            .bind(&map_id)
            .bind(map_revision)
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
        let canonical: Vec<u8> = row.try_get("canonical_bytes").map_err(invalid)?;
        let manifest = concert_domain::place::parse_stored_release(&canonical)
            .map_err(|_| PlaceError::InvalidState)?;
        let release =
            CatalogRelease::new(manifest, canonical).map_err(|_| PlaceError::InvalidState)?;
        if row.try_get::<String, _>("map_id").map_err(invalid)? != release.map_id
            || row.try_get::<i64, _>("map_revision").map_err(invalid)?
                != release.manifest.map_revision
            || row.try_get::<String, _>("content_hash").map_err(invalid)?
                != release.manifest.content_hash
            || row.try_get::<String, _>("geometry_hash").map_err(invalid)?
                != release.manifest.geometry_hash
        {
            return Err(PlaceError::InvalidState);
        }
        let places = sqlx::query(&format!(
            "{PLACE_SELECT} where q.map_id=$1 and q.map_revision=$2 order by q.place_id limit 129"
        ))
        .bind(&map_id)
        .bind(map_revision)
        .fetch_all(&mut *tx)
        .await
        .map_err(db)?
        .iter()
        .map(|row| place(&row))
        .collect::<Result<Vec<_>, _>>()?;
        let snapshot = snapshot(&head)?;
        tx.commit().await.map_err(db)?;
        Ok(Some(CatalogRead {
            snapshot,
            release,
            places,
        }))
    }

    async fn detail(
        &self,
        event_id: EventId,
        place_id: PlaceId,
    ) -> Result<Option<(PlaceSnapshot, Place)>, PlaceError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        sqlx::query("set local transaction isolation level repeatable read, read only")
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        let row = sqlx::query(&format!(
            "{PLACE_SELECT} join public.concert_events_map_releases b on b.map_id=q.map_id and b.map_revision=q.map_revision where b.event_id=$1 and q.place_id=$2"
        ))
        .bind(event_id.uuid())
        .bind(place_id.uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?;
        let Some(place_row) = row else {
            return Ok(None);
        };
        let head = sqlx::query("select event_id,catalog_revision,map_id,map_revision from public.concert_events_map_releases where event_id=$1")
            .bind(event_id.uuid())
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
        let value = place(&place_row)?;
        let snapshot = snapshot(&head)?;
        tx.commit().await.map_err(db)?;
        Ok(Some((snapshot, value)))
    }

    async fn publish_release(
        &self,
        event_id: EventId,
        release: CatalogRelease,
    ) -> Result<PlaceSnapshot, PlaceError> {
        release
            .verify_immutable(&release.canonical_bytes)
            .map_err(|_| PlaceError::InvalidState)?;
        let mut tx = self.pool.begin().await.map_err(db)?;
        let exists: bool = sqlx::query_scalar(
            "select exists(select 1 from public.concert_events where event_id=$1)",
        )
        .bind(event_id.uuid())
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        if !exists {
            return Err(PlaceError::NotFound);
        }
        // Map identity is global. Serialize publication so no two writers can
        // append different member sets to the same release.
        sqlx::query("select pg_advisory_xact_lock(hashtextextended($1::text,0))")
            .bind(format!(
                "{}:{}",
                release.map_id, release.manifest.map_revision
            ))
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        sqlx::query("insert into public.concert_map_releases(map_id,map_revision,content_hash,geometry_hash,canonical_bytes) values($1,$2,$3,$4,$5) on conflict(map_id,map_revision) do nothing")
            .bind(&release.map_id)
            .bind(release.manifest.map_revision)
            .bind(&release.manifest.content_hash)
            .bind(&release.manifest.geometry_hash)
            .bind(&release.canonical_bytes)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        let stored: Vec<u8> = sqlx::query_scalar("select canonical_bytes from public.concert_map_releases where map_id=$1 and map_revision=$2")
            .bind(&release.map_id)
            .bind(release.manifest.map_revision)
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
        release
            .verify_immutable(&stored)
            .map_err(|_| PlaceError::Conflict)?;
        let existing = sqlx::query("select q.map_id,q.map_revision,q.place_id,q.label,q.place_kind,q.verification_state,q.accessibility_notes,q.source_kind,q.source_uri,q.review_state,q.provenance,'OPEN'::text as closure_state from public.concert_map_release_places q where q.map_id=$1 and q.map_revision=$2 order by q.place_id")
            .bind(&release.map_id)
            .bind(release.manifest.map_revision)
            .fetch_all(&mut *tx)
            .await
            .map_err(db)?
            .iter()
            .map(|row| place(row))
            .collect::<Result<Vec<_>, _>>()?;
        if !existing.is_empty() {
            let mut expected = release
                .manifest
                .places
                .iter()
                .map(|value| value.id)
                .collect::<Vec<_>>();
            expected.sort();
            let mut actual = existing.iter().map(|value| value.id).collect::<Vec<_>>();
            actual.sort();
            let mismatched = release.manifest.places.iter().any(|value| {
                !existing.iter().any(|row| {
                    row.id == value.id
                        && row.label == value.label
                        && row.kind == value.kind
                        && row.verification == value.verification
                        && row.accessibility_notes == value.accessibility_notes
                        && row.provenance == value.provenance
                })
            });
            if expected != actual || mismatched {
                return Err(PlaceError::Conflict);
            }
            let stored_closures = sqlx::query(
                "select place_id,closure_state from public.concert_place_current_state where map_id=$1 and map_revision=$2",
            )
            .bind(&release.map_id)
            .bind(release.manifest.map_revision)
            .fetch_all(&mut *tx)
            .await
            .map_err(db)?;
            if stored_closures.len() != existing.len() {
                return Err(PlaceError::Conflict);
            }
        }

        for value in &release.manifest.places {
            sqlx::query("insert into public.concert_map_release_places(map_id,map_revision,place_id,label,place_kind,verification_state,accessibility_notes,source_kind,source_uri,review_state,provenance) select $1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11 where not exists(select 1 from public.concert_map_release_places where map_id=$1 and map_revision=$2 and place_id=$3)")
                .bind(&release.map_id)
                .bind(release.manifest.map_revision)
                .bind(value.id.uuid())
                .bind(&value.label)
                .bind(value.kind.as_str())
                .bind(value.verification.as_str())
                .bind(&value.accessibility_notes)
                .bind(value.provenance.source_kind.as_str())
                .bind(&value.provenance.source_uri)
                .bind(value.provenance.review_state.as_str())
                .bind(provenance_json(&value.provenance))
                .execute(&mut *tx)
                .await
                .map_err(db)?;
            sqlx::query("insert into public.concert_place_current_state(map_id,map_revision,place_id,closure_state) values($1,$2,$3,$4) on conflict(map_id,map_revision,place_id) do nothing")
                .bind(&release.map_id)
                .bind(release.manifest.map_revision)
                .bind(value.id.uuid())
                .bind(value.closure.as_str())
                .execute(&mut *tx)
                .await
                .map_err(db)?;
        }
        let member_count: i64 = sqlx::query_scalar("select count(*) from public.concert_map_release_places where map_id=$1 and map_revision=$2")
            .bind(&release.map_id)
            .bind(release.manifest.map_revision)
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
        let current_count: i64 = sqlx::query_scalar("select count(*) from public.concert_place_current_state where map_id=$1 and map_revision=$2")
            .bind(&release.map_id)
            .bind(release.manifest.map_revision)
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
        if member_count != release.manifest.places.len() as i64
            || current_count != release.manifest.places.len() as i64
        {
            return Err(PlaceError::Conflict);
        }
        let prior: Option<i64> = sqlx::query_scalar(
            "select catalog_revision from public.concert_events_map_releases where event_id=$1 for update",
        )
        .bind(event_id.uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(db)?;
        let catalog_revision = prior
            .unwrap_or(0)
            .checked_add(1)
            .filter(|value| *value <= 60_000)
            .ok_or(PlaceError::Conflict)?;
        if prior.is_some() {
            sqlx::query(
                "update public.concert_events_map_releases set catalog_revision=$2,map_id=$3,map_revision=$4 where event_id=$1",
            )
            .bind(event_id.uuid())
            .bind(catalog_revision)
            .bind(&release.map_id)
            .bind(release.manifest.map_revision)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        } else {
            sqlx::query(
                "insert into public.concert_events_map_releases(event_id,catalog_revision,map_id,map_revision) values($1,$2,$3,$4)",
            )
            .bind(event_id.uuid())
            .bind(catalog_revision)
            .bind(&release.map_id)
            .bind(release.manifest.map_revision)
            .execute(&mut *tx)
            .await
            .map_err(db)?;
        }
        let result = PlaceSnapshot::from_db(
            event_id,
            revision(catalog_revision)?,
            (release.map_id, release.manifest.map_revision),
        )?;
        tx.commit().await.map_err(db)?;
        Ok(result)
    }

    async fn select_physical(
        &self,
        event_id: EventId,
        place_id: PlaceId,
        expected_catalog: Revision,
        expected_map: i64,
    ) -> Result<SelectionOutcome, PlaceError> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        let outcome = Self::select_physical_in_transaction(
            &mut tx,
            event_id,
            place_id,
            expected_catalog,
            expected_map,
        )
        .await?;
        tx.commit().await.map_err(db)?;
        Ok(outcome)
    }
}

// BE38 delegated append-only administrative owner port registration.
pub mod operations;
pub use operations::PgPlaceOperations;
