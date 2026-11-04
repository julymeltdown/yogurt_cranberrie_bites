//! Reviewed v2 production constructors + real PG persistence. Auth/Friend scope
//! registration is explicitly a component fixture, never real public admission.
#[path = "avatar_motion_v2_runtime_support/fence.rs"]
mod fence;
#[path = "support/avatar/mod.rs"]
mod support;
use concert_application::{avatar::*, bootstrap::BootstrapRepository};
use concert_domain::{
    avatar::{config_value, parse_config},
    avatar_motion_v2::reviewed,
    *,
};
use concert_service::{
    adapters::out::repository::avatar::*,
    bootstrap::{BootstrapFeatures, BootstrapPostgresRepository, ProductReleaseBundle},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    path::{Path, PathBuf},
    sync::Arc,
};
use uuid::Uuid;
const EVENT: &str = "00000000-0000-4000-8000-000000009401";
const USER: &str = "00000000-0000-4000-8000-000000009402";
fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}
fn fixture() -> PathBuf {
    root().join("backend/releases/selected-character-motion-v2/reviewed/runtime-fixture")
}
fn key(event: Uuid, user: Uuid) -> ParticipantKey {
    ParticipantKey {
        event_id: EventId::from_uuid(event),
        user_id: AuthUserId::from_uuid(user),
    }
}
fn command(expected: i64, config: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"expected_revision":expected,"config":config})).unwrap()
}
fn paths_release(
    directory: &Path,
    avatar: &Path,
) -> Result<
    Arc<concert_application::bootstrap::ValidatedProductRelease>,
    concert_service::bootstrap::BootstrapServiceError,
> {
    ProductReleaseBundle::from_source_bound_files(
        &directory.join("bootstrap.json"),
        avatar,
        &root().join("backend/releases/reference-scene-v2/quality-v6/map-manifest.json"),
        [
            &directory.join("theme-danger.json"),
            &directory.join("theme-parade.json"),
            &directory.join("theme-revenge.json"),
        ],
        BootstrapFeatures::default(),
        &root().join("frontend/public"),
        &root().join("backend/releases/reference-scene-v2/quality-v6/source-binding.json"),
    )
}
fn release() -> Arc<concert_application::bootstrap::ValidatedProductRelease> {
    paths_release(
        &fixture(),
        &fixture().parent().unwrap().join("manifest.json"),
    )
    .expect("actual source-bound reviewed release")
}
fn preset(n: usize) -> Value {
    serde_json::from_str::<Value>(reviewed::MANIFEST_JSON).unwrap()["presets"][n]["config"].clone()
}
fn layered() -> Value {
    json!({"contract_version":"2.0.0","kind":"avatar_config","manifest_id":reviewed::MANIFEST_ID,"manifest_version":2,"representation":"motion_layered","item_ids":{"body":"motion_v2_body_7","eyes":"motion_v2_eyes_3","hair":"motion_v2_hair_5","outfit":null,"accessory":null}})
}

#[tokio::test]
async fn reviewed_product_constructor_binds_actual_review_resources_and_source_bundle() {
    let release = release();
    assert_eq!(
        config_value(&release.release().defaults().avatar),
        preset(0)
    );
    let document: Value = serde_json::from_slice(release.release().bootstrap_json()).unwrap();
    assert_eq!(
        document["avatar_manifest"],
        serde_json::from_str::<Value>(reviewed::MANIFEST_JSON).unwrap()
    );
    assert_eq!(document["capabilities"]["mutations"]["available"], false);
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused@127.0.0.1:1/unused")
        .unwrap();
    AvatarPersistence::from_product(pool, release)
        .expect("real product constructor; no synthetic bypass");
}
#[test]
fn reviewed_product_constructor_rejects_candidate_and_wrong_resource_location() {
    let temp = std::env::temp_dir().join(format!("mcr-motion-v2-{}", Uuid::new_v4()));
    std::fs::create_dir(&temp).unwrap();
    let copied = temp.join("manifest.json");
    std::fs::write(&copied, reviewed::MANIFEST_JSON).unwrap();
    assert!(
        paths_release(&fixture(), &copied).is_err(),
        "unbound avatar root must fail"
    );
    assert!(paths_release(
        &fixture(),
        &root().join("backend/releases/selected-character-motion-v2/manifest.json")
    )
    .is_err());
    std::fs::remove_dir_all(temp).unwrap();
}

#[tokio::test]
#[ignore = "requires owned motion-v2 native runner"]
async fn native_motion_v2_transactions_cas_constraints_and_rollback() {
    let pool = support::pool().await;
    let release = release();
    let service = Arc::new(AvatarPersistence::from_product(pool.clone(), release.clone()).unwrap());
    let event = EVENT.parse().unwrap();
    let user = USER.parse().unwrap();
    let own = key(event, user);
    support::seed(&pool, event).await;
    let bootstrap = BootstrapPostgresRepository::new(pool.clone());
    bootstrap
        .bootstrap(
            &support::admission(event, user),
            release.release().defaults(),
        )
        .await
        .unwrap();
    // Every shared positive config traverses real Bootstrap storage and the same
    // production read decoder, including both required nullable fields.
    for entry in
        std::fs::read_dir(root().join("contracts/fixtures/assets/motion-v2/reviewed/valid"))
            .unwrap()
    {
        let wire = std::fs::read(entry.unwrap().path()).unwrap();
        let selection = parse_config(&wire).unwrap();
        let fixture_user = Uuid::new_v4();
        let defaults = concert_application::bootstrap::BootstrapDefaults {
            theme: Theme::Revenge,
            avatar: selection.clone(),
        };
        bootstrap
            .bootstrap(&support::admission(event, fixture_user), &defaults)
            .await
            .unwrap();
        let fixture_key = key(event, fixture_user);
        let authority =
            support::Authority::new(fixture_key, AvatarOperation::Read, release.source_digest());
        assert_eq!(
            service
                .read(fixture_key, &authority)
                .await
                .unwrap()
                .avatar
                .selection,
            selection
        );
    }
    let reader = || support::Authority::new(own, AvatarOperation::Read, release.source_digest());
    assert_eq!(
        config_value(&service.read(own, &reader()).await.unwrap().avatar.selection),
        preset(0)
    );
    let bytes = command(1, layered());
    let unfenced =
        support::Authority::new(own, AvatarOperation::SetAvatar, release.source_digest());
    assert_eq!(
        service
            .set_avatar(own, &bytes, &unfenced)
            .await
            .unwrap_err(),
        AvatarError::Unavailable
    );
    let reject = fence::authority(own, release.source_digest(), &bytes, &pool, true);
    assert_eq!(
        service.set_avatar(own, &bytes, &reject).await.unwrap_err(),
        AvatarError::Denied
    );
    assert_eq!(
        service
            .read(own, &reader())
            .await
            .unwrap()
            .avatar
            .revision
            .get(),
        1
    );
    let mut tasks = vec![];
    for _ in 0..12 {
        let service = service.clone();
        let pool = pool.clone();
        let bytes = bytes.clone();
        let digest = release.source_digest().to_string();
        tasks.push(tokio::spawn(async move {
            let authority = fence::authority(own, &digest, &bytes, &pool, false);
            service.set_avatar(own, &bytes, &authority).await
        }));
    }
    let mut success = 0;
    let mut conflicts = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(state) => {
                success += 1;
                assert_eq!(state, json!({"revision":2,"config":layered()}));
            }
            Err(AvatarError::Conflict) => conflicts += 1,
            other => panic!("unexpected CAS result {other:?}"),
        }
    }
    assert_eq!((success, conflicts), (1, 11));
    let reread = service.read(own, &reader()).await.unwrap();
    assert_eq!(config_value(&reread.avatar.selection), layered());
    let columns: (String,String,Option<String>,Option<String>,Option<String>,Option<String>)=sqlx::query_as("select contract_version,representation,preset_id,outfit_id,accessory_id,skin_tone_id from concert_avatar_settings where event_id=$1 and user_id=$2").bind(event).bind(user).fetch_one(&pool).await.unwrap();
    assert_eq!(
        columns,
        (
            "2.0.0".into(),
            "motion_layered".into(),
            None,
            None,
            None,
            None
        )
    );
    // Each invalid SQL operation runs in its own transaction and cannot alter the state.
    for assignment in [
        "manifest_id='mcr_selected_motion_20261103'",
        "manifest_version=1",
        "body_id='motion_v2_body_8'",
        "eyes_id=NULL",
        "hair_id='legacy'",
        "outfit_id='motion_v2_outfit_6'",
        "accessory_id='legacy'",
        "skin_tone_id='tone_0'",
        "preset_id='motion_v2_photo01_front_01'",
    ] {
        let query = format!(
            "update concert_avatar_settings set revision=revision+1,{assignment} where event_id=$1 and user_id=$2"
        );
        let error = sqlx::query(&query)
            .bind(event)
            .bind(user)
            .execute(&pool)
            .await
            .unwrap_err();
        assert_eq!(
            error.as_database_error().and_then(|e| e.code()).as_deref(),
            Some("23514"),
            "{assignment}"
        );
        assert_eq!(
            error.as_database_error().and_then(|e| e.constraint()),
            Some("concert_avatar_representation"),
            "wrong rejection source: {assignment}"
        );
    }
    assert_eq!(service.read(own, &reader()).await.unwrap(), reread);
    let bytes = command(2, preset(5));
    let authority = fence::authority(own, release.source_digest(), &bytes, &pool, false);
    assert_eq!(
        service.set_avatar(own, &bytes, &authority).await.unwrap(),
        json!({"revision":3,"config":preset(5)})
    );
    let snapshot = service.read(own, &reader()).await.unwrap();
    // The real constructor/read path is reopened; the runner additionally restarts PostgreSQL.
    let reopened = AvatarPersistence::from_product(pool.clone(), release.clone()).unwrap();
    assert_eq!(reopened.read(own, &reader()).await.unwrap(), snapshot);
    let witnesses:i64=sqlx::query_scalar("select count(*) from concert_participant_effect_witnesses where event_id=$1 and actor_id=$2").bind(event).bind(user).fetch_one(&pool).await.unwrap();
    assert_eq!(
        witnesses, 2,
        "only committed avatar mutations have witnesses"
    );
    let wrong = support::Authority::new(
        key(event, Uuid::new_v4()),
        AvatarOperation::Read,
        release.source_digest(),
    );
    assert_eq!(
        service.read(own, &wrong).await.unwrap_err(),
        AvatarError::Denied
    );
    let output = std::path::PathBuf::from(std::env::var("MCR_BOOTSTRAP_ROOT").unwrap())
        .join("motion-v2-state.json");
    std::fs::write(
        output,
        serde_json::to_vec(&avatar_state(&snapshot.avatar)).unwrap(),
    )
    .unwrap();
    pool.close().await;
}
#[tokio::test]
#[ignore = "runner executes after real postmaster restart"]
async fn native_motion_v2_restart_persistence() {
    let pool = support::pool().await;
    let release = release();
    let own = key(EVENT.parse().unwrap(), USER.parse().unwrap());
    let service = AvatarPersistence::from_product(pool.clone(), release.clone()).unwrap();
    let authority = support::Authority::new(own, AvatarOperation::Read, release.source_digest());
    let before: Value = serde_json::from_slice(
        &std::fs::read(
            PathBuf::from(std::env::var("MCR_BOOTSTRAP_ROOT").unwrap())
                .join("motion-v2-state.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let state = service.read(own, &authority).await.unwrap();
    assert_eq!(avatar_state(&state.avatar), before);
    assert_eq!(config_value(&state.avatar.selection), preset(5));
    assert_eq!(state.avatar.revision.get(), 3);
    let record = BootstrapPostgresRepository::new(pool.clone())
        .bootstrap(
            &support::admission(EVENT.parse().unwrap(), USER.parse().unwrap()),
            release.release().defaults(),
        )
        .await
        .unwrap();
    assert_eq!(config_value(&record.avatar), preset(5));
    assert_eq!(record.avatar_revision, 3);
    pool.close().await;
}
