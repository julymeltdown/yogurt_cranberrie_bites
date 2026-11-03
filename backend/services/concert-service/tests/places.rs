//! Run only through places_smoke.py with a marker/inode-owned PostgreSQL cluster.
use concert_application::place::{PlaceRepository, SelectionOutcome};
use concert_domain::place::*;
use concert_domain::{EventId, Revision};
use concert_service::adapters::out::repository::place::PgPlaceRepository;
use sha2::Digest;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{
    os::unix::fs::{MetadataExt, PermissionsExt},
    path::PathBuf,
    str::FromStr,
    time::Duration,
};
use uuid::Uuid;

async fn owned_pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(std::env::var("PLACES_TEST_OWNER_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        info.is_dir() && !info.file_type().is_symlink() && info.permissions().mode() & 0o077 == 0,
        "private owned root"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("PLACES_TEST_OWNER_IDENTITY")?,
        "fixture identity"
    );
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("mcr-owner.json"))?)?;
    anyhow::ensure!(
        marker["token"].as_str() == Some(std::env::var("PLACES_TEST_OWNER_TOKEN")?.as_str()),
        "fixture marker"
    );
    let options = PgConnectOptions::from_str(&std::env::var("PLACES_TEST_DATABASE_URL")?)?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_places")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign db"
    );
    let pool = PgPoolOptions::new()
        .max_connections(3)
        .acquire_timeout(Duration::from_secs(3))
        .connect_with(options)
        .await?;
    let actual: String = sqlx::query_scalar("show data_directory")
        .fetch_one(&pool)
        .await?;
    anyhow::ensure!(
        actual == root.join("data").to_string_lossy(),
        "foreign data"
    );
    Ok(pool)
}
fn manifest() -> Manifest {
    parse_stored_release(&canonical_fixture()).unwrap()
}
fn canonical_fixture() -> Vec<u8> {
    concert_domain::place::canonical_json_bytes(include_bytes!(
        "../../../../contracts/fixtures/assets/map-manifest.json"
    ))
    .unwrap()
}
fn canonical_release(revision: i64, place: &Place) -> anyhow::Result<CatalogRelease> {
    let bytes = release_bytes(revision, place)?;
    Ok(CatalogRelease::new(parse_stored_release(&bytes)?, bytes)?)
}
fn canonical_fixture_for(place: &Place) -> anyhow::Result<Vec<u8>> {
    release_bytes(2, place)
}
fn release_bytes(revision: i64, place: &Place) -> anyhow::Result<Vec<u8>> {
    let value: serde_json::Value = serde_json::from_slice(include_bytes!(
        "../../../../contracts/fixtures/assets/map-manifest.json"
    ))?;
    let mut object = value.as_object().unwrap().clone();
    object.remove("content_hash");
    object.insert("map_revision".into(), revision.into());
    object.insert(
        "places".into(),
        serde_json::Value::Array(vec![serde_json::json!({
            "place_id": place.id.uuid().to_string(),
            "label": place.label,
            "place_kind": place.kind.as_str(),
            "verification_state": place.verification.as_str(),
            "closure_state": place.closure.as_str(),
            "accessibility_notes": place.accessibility_notes,
            "provenance": serde_json::json!({
                "source_id": place.provenance.source_id,
                "source_kind": place.provenance.source_kind.as_str(),
                "source_revision": place.provenance.source_revision,
                "source_uri": place.provenance.source_uri,
                "author": place.provenance.author,
                "license": place.provenance.license,
                "review_state": place.provenance.review_state.as_str(),
                "reviewed_by": place.provenance.reviewed_by,
                "reviewed_at": place.provenance.reviewed_at.map(|at| at.to_rfc3339()),
                "photo_permission": place.provenance.photo_permission.as_str(),
            })
        })]),
    );
    object["geometry"]["interactions"][0]["place_id"] = place.id.uuid().to_string().into();
    let digest = format!(
        "sha256:{}",
        sha2::Sha256::digest(serde_json::to_vec(&serde_json::Value::Object(
            object.clone()
        ))?)
        .iter()
        .map(|v| format!("{v:02x}"))
        .collect::<String>()
    );
    object.insert("content_hash".into(), digest.into());
    let bytes = serde_json::to_vec(&serde_json::Value::Object(object))?;
    Ok(bytes)
}
async fn event(pool: &PgPool) -> EventId {
    let id = Uuid::new_v4();
    sqlx::query(
        "insert into concert_events(event_id,slug,title,phase) values($1,$2,'Places event','OPEN')",
    )
    .bind(id)
    .bind(format!("places-{}", id.simple()))
    .execute(pool)
    .await
    .unwrap();
    EventId::from_uuid(id)
}

#[tokio::test]
async fn owned_native_places_are_scoped_immutable_and_preserve_current_closure(
) -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let repo = PgPlaceRepository::new(pool.clone());
    let a = event(&pool).await;
    let b = event(&pool).await;
    let at = chrono::Utc::now();
    let r1 = CatalogRelease::new(manifest(), canonical_fixture())?;
    let s1 = repo.publish_release(a, r1.clone()).await?;
    assert_eq!(
        repo.publish_release(b, r1).await?.revision,
        Revision::initial(),
        "independent events start their own catalog revision"
    );
    assert_eq!(s1.map_revision, 1);
    assert!(repo.catalog(b).await?.is_some());
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from concert_map_release_places where map_id='map_fixture' and map_revision=1"
        )
        .fetch_one(&pool)
        .await?,
        1
    );
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from concert_place_current_state where map_id='map_fixture' and map_revision=1"
        )
        .fetch_one(&pool)
        .await?,
        1
    );
    let read = repo.catalog(a).await?.expect("published");
    let id = read.places[0].id;
    assert_eq!(read.release.canonical_bytes, canonical_fixture());
    assert_eq!(
        repo.select_physical(a, id, Revision::initial(), 1).await?,
        SelectionOutcome::ConceptOnly
    );
    let physical = Place::new(
        id,
        "Fixture station".into(),
        PlaceKind::Physical,
        VerificationState::Verified,
        ClosureState::Open,
        "Step-free fixture notes.".into(),
        Provenance::new(
            "venue_reference".into(),
            SourceKind::PlaceReference,
            "reviewed-1".into(),
            Some("https://fixture.invalid/place".into()),
            "Venue fixture author".into(),
            "Synthetic reviewed metadata".into(),
            ReviewState::Reviewed,
            Some("parent-reviewer".into()),
            Some(at),
            PhotoPermission::NotApplicable,
        )?,
    )?;
    let r2 = canonical_release(2, &physical)?;
    let s2 = repo.publish_release(a, r2).await?;
    assert_eq!(s2.revision.get(), 2);
    assert_eq!(s2.map_revision, 2);
    assert_eq!(
        repo.catalog(b)
            .await?
            .expect("independent binding")
            .snapshot
            .map_revision,
        1
    );
    let other = event(&pool).await;
    assert!(repo.catalog(other).await?.is_none());
    let rebound = canonical_release(2, &physical)?;
    let mut collision =
        serde_json::from_slice::<serde_json::Value>(&canonical_fixture_for(&physical)?)?;
    // Keep the interaction walkable but isolate it from the spawn.
    collision["geometry"]["collision_grid"][3][2] = serde_json::json!(1);
    collision["geometry"]["collision_grid"][2][3] = serde_json::json!(1);
    collision
        .as_object_mut()
        .expect("fixture object")
        .remove("content_hash");
    let geometry_hash = format!(
        "sha256:{}",
        sha2::Sha256::digest(serde_json::to_vec(&collision["geometry"])?)
            .iter()
            .map(|value| format!("{value:02x}"))
            .collect::<String>()
    );
    collision["geometry_hash"] = geometry_hash.into();
    let collision_hash = format!(
        "sha256:{}",
        sha2::Sha256::digest(serde_json::to_vec(&collision)?)
            .iter()
            .map(|value| format!("{value:02x}"))
            .collect::<String>()
    );
    collision["content_hash"] = collision_hash.into();
    assert!(parse_stored_release(&serde_json::to_vec(&collision)?).is_err());
    sqlx::query("update concert_map_release_places set label='Tampered' where map_id='map_fixture' and map_revision=2")
        .execute(&pool)
        .await
        .expect_err("release membership is immutable");
    assert_eq!(
        repo.publish_release(a, rebound).await?.revision.get(),
        3,
        "exact immutable bytes may be reused without rewriting metadata"
    );
    assert!(matches!(
        repo.select_physical(a, id, Revision::new(3)?, 2).await?,
        SelectionOutcome::Eligible(detail)
            if detail.place.permits_new_physical_selection()
    ));
    assert_eq!(read.snapshot.revision, Revision::new(1)?);
    let reopen = sqlx::query(
        "update concert_place_current_state set closure_state='OPEN',revision=revision+1 where map_id='map_fixture' and map_revision=2 and place_id=$1",
    )
    .bind(id.uuid())
    .execute(&pool)
    .await?;
    assert_eq!(reopen.rows_affected(), 1);
    let mut held = pool.begin().await?;
    assert!(matches!(
        PgPlaceRepository::select_physical_in_transaction(&mut held, a, id, Revision::new(3)?, 2,)
            .await?,
        SelectionOutcome::Eligible(_)
    ));
    sqlx::query("insert into concert_place_selection_race(event_id,place_id) values($1,$2)")
        .bind(a.uuid())
        .bind(id.uuid())
        .execute(&mut *held)
        .await?;
    let contender = pool.clone();
    let update = tokio::spawn(async move {
        sqlx::query(
            "update concert_place_current_state set closure_state='CLOSED',revision=revision+1 where map_id='map_fixture' and map_revision=2 and place_id=$1",
        )
        .bind(id.uuid())
        .execute(&contender)
        .await
        .map(|result| result.rows_affected())
    });
    tokio::time::sleep(Duration::from_millis(80)).await;
    assert!(
        !update.is_finished(),
        "closure update must wait for held share lock"
    );
    held.commit().await?;
    assert_eq!(
        sqlx::query_scalar::<_, i64>(
            "select count(*) from concert_place_selection_race where event_id=$1 and place_id=$2",
        )
        .bind(a.uuid())
        .bind(id.uuid())
        .fetch_one(&pool)
        .await?,
        1
    );
    assert_eq!(update.await??, 1);
    assert_eq!(
        repo.select_physical(a, id, Revision::new(3)?, 2).await?,
        SelectionOutcome::Closed
    );
    let historical = repo.catalog(a).await?.unwrap();
    assert_eq!(historical.snapshot.revision, Revision::new(3)?);
    assert_eq!(historical.release.manifest.map_revision, 2);
    assert_eq!(
        historical.release.canonical_bytes,
        canonical_fixture_for(&physical)?
    );
    assert_eq!(
        repo.select_physical(a, id, Revision::new(2)?, 2).await?,
        SelectionOutcome::StaleCatalog
    );
    assert_eq!(
        repo.select_physical(a, id, Revision::new(3)?, 1).await?,
        SelectionOutcome::MapMismatch
    );
    assert_eq!(
        repo.detail(a, id)
            .await?
            .expect("current detail")
            .0
            .revision,
        Revision::new(3)?
    );
    assert_eq!(
        repo.detail(b, id)
            .await?
            .expect("foreign event")
            .0
            .map_revision,
        1
    );
    // closed current state does not alter immutable historical bytes
    let old_bytes: Vec<u8> = sqlx::query_scalar(
        "select canonical_bytes from concert_map_releases where map_id='map_fixture' and map_revision=1",
    )
    .fetch_one(&pool)
    .await?;
    assert_eq!(old_bytes, canonical_fixture());
    let bad = CatalogRelease {
        map_id: "map_fixture".into(),
        manifest: manifest(),
        canonical_bytes: vec![b'{', b'}'],
    };
    assert!(repo.publish_release(a, bad).await.is_err());
    pool.close().await;
    Ok(())
}
