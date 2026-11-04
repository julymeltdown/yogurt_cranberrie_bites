#[path = "support/avatar/mod.rs"]
mod support;
use concert_application::{avatar::*, bootstrap::BootstrapRepository};
use concert_domain::{avatar::config_value, *};
use concert_service::{
    adapters::out::repository::avatar::*, bootstrap::BootstrapPostgresRepository,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::Row;
use std::sync::Arc;
use uuid::Uuid;
const EVENT: &str = "00000000-0000-4000-8000-000000000701";
const USER: &str = "00000000-0000-4000-8000-000000000702";
fn key(event: Uuid, user: Uuid) -> ParticipantKey {
    ParticipantKey {
        event_id: EventId::from_uuid(event),
        user_id: AuthUserId::from_uuid(user),
    }
}
fn command(expected: i64, config: Value) -> Vec<u8> {
    serde_json::to_vec(&json!({"expected_revision":expected,"config":config})).unwrap()
}
fn prefs(expected: i64) -> Vec<u8> {
    serde_json::to_vec(&json!({"expected_revision":expected,"preferences":{"theme":"DANGER","low_power":true,"reduced_motion":true,"sound_enabled":false}})).unwrap()
}

/// Persistence-only lock regression. The selected character is combined with
/// fixture map/themes; this does not simulate canonical Auth or World admission.
#[tokio::test]
#[ignore = "requires avatar_smoke.py owned PostgreSQL cluster"]
async fn native_avatar_reads_share_presence_fences_and_writes_still_wait() {
    use std::time::Duration;
    let pool = support::pool().await;
    let release = support::selected_release();
    let event = Uuid::new_v4();
    let user = Uuid::new_v4();
    let own = key(event, user);
    support::seed(&pool, event).await;
    BootstrapPostgresRepository::new(pool.clone())
        .bootstrap(
            &support::admission(event, user),
            release.release().defaults(),
        )
        .await
        .unwrap();
    let service = Arc::new(
        AvatarPersistence::from_synthetic_test(pool.clone(), release.clone(), true).unwrap(),
    );
    let mut revision = 1;
    for table in [
        "concert_events",
        "concert_event_admissions",
        "concert_participants",
        "concert_avatar_settings",
    ] {
        let mut held = pool.begin().await.unwrap();
        let holder: i32 = sqlx::query_scalar("select pg_backend_pid()")
            .fetch_one(&mut *held)
            .await
            .unwrap();
        let query = format!("select event_id from {table} where event_id=$1 for share");
        sqlx::query(&query)
            .bind(event)
            .fetch_one(&mut *held)
            .await
            .unwrap();
        let authority =
            support::Authority::new(own, AvatarOperation::Read, service.release_digest());
        let read =
            tokio::time::timeout(Duration::from_secs(2), service.read(own, &authority)).await;
        if !matches!(&read, Ok(Ok(_))) {
            held.rollback().await.unwrap();
            pool.close().await;
            panic!("read could not finish while the compatible {table} SHARE fence remained held");
        }
        let snapshot = read.unwrap().unwrap();
        assert_eq!(snapshot.avatar.revision.get(), revision);
        let writer = service.clone();
        let bytes = command(revision, config_value(&release.release().defaults().avatar));
        let task = tokio::spawn(async move {
            let authority =
                support::Authority::new(own, AvatarOperation::SetAvatar, writer.release_digest());
            writer.set_avatar(own, &bytes, &authority).await
        });
        // Observe an actual PostgreSQL lock wait, rather than inferring one from
        // a task not having been scheduled. The held fence remains authoritative.
        let blocked = tokio::time::timeout(Duration::from_secs(2), async {
            loop {
                let count: i64 = sqlx::query_scalar(
                    "select count(*) from pg_stat_activity where datname=current_database() and $1=any(pg_blocking_pids(pid))",
                )
                .bind(holder)
                .fetch_one(&pool)
                .await
                .unwrap();
                if count > 0 {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        })
        .await;
        held.rollback().await.unwrap();
        let saved = task.await.unwrap();
        assert!(blocked.is_ok(), "mutation bypassed the {table} SHARE fence");
        assert_eq!(saved.unwrap()["revision"], revision + 1);
        revision += 1;
    }
    pool.close().await;
}
#[test]
fn witness_metadata_defaults_to_unavailable_and_binds_the_exact_release() {
    struct LegacyFence;
    #[async_trait::async_trait]
    impl AvatarEffectFence for LegacyFence {
        async fn register(&self, _: AvatarTransactionIdentity) -> Result<(), AvatarError> {
            Ok(())
        }
        async fn check(&self) -> Result<(), AvatarError> {
            Ok(())
        }
    }
    assert_eq!(
        LegacyFence.witness_binding().unwrap_err(),
        AvatarError::Unavailable
    );
    let valid = AvatarEffectWitnessBinding {
        producer_instance_id: Uuid::new_v4(),
        scope_id: Uuid::new_v4(),
        operation_id: Uuid::new_v4(),
        session_family_id: Uuid::new_v4(),
        parent_digest: [11; 32],
        guard_binding_digest: [22; 32],
        body_digest: [33; 32],
        release_digest: "a".repeat(64),
        effect_binding_digest: [44; 32],
    };
    valid.validate_for_release(&"a".repeat(64)).unwrap();
    assert_eq!(
        valid.validate_for_release(&"b".repeat(64)),
        Err(AvatarError::Unavailable)
    );
    for field in [
        "producer",
        "scope",
        "operation",
        "session",
        "parent",
        "guard",
        "body",
        "effect",
        "uppercase",
        "short",
    ] {
        let mut invalid = valid.clone();
        match field {
            "producer" => invalid.producer_instance_id = Uuid::nil(),
            "scope" => invalid.scope_id = Uuid::nil(),
            "operation" => invalid.operation_id = Uuid::nil(),
            "session" => invalid.session_family_id = Uuid::nil(),
            "parent" => invalid.parent_digest = [0; 32],
            "guard" => invalid.guard_binding_digest = [0; 32],
            "body" => invalid.body_digest = [0; 32],
            "effect" => invalid.effect_binding_digest = [0; 32],
            "uppercase" => invalid.release_digest = "A".repeat(64),
            "short" => invalid.release_digest.pop().map(|_| ()).unwrap(),
            _ => unreachable!(),
        }
        assert_eq!(
            invalid.validate_for_release(&invalid.release_digest),
            Err(AvatarError::Unavailable)
        );
    }
    let debug = format!("{valid:?}");
    assert_eq!(debug, "AvatarEffectWitnessBinding { .. }");
}
#[test]
fn preference_payload_is_exact_bounded_and_no_character_fields() {
    assert!(parse_preferences(&prefs(1)).is_ok());
    for raw in [
        b"{}".as_slice(),
        br#"{"expected_revision":1,"preferences":{"theme":"DANGER"}}"#,
        br#"{"expected_revision":1,"expected_revision":2,"preferences":{}}"#,
    ] {
        assert!(parse_preferences(raw).is_err());
    }
    let mut wire: Value = serde_json::from_slice(&prefs(1)).unwrap();
    wire["preferences"]["avatar"] = json!({});
    assert!(parse_preferences(&serde_json::to_vec(&wire).unwrap()).is_err());
    wire["preferences"]
        .as_object_mut()
        .unwrap()
        .remove("avatar");
    wire["preferences"]["theme"] = json!("OTHER");
    assert!(parse_preferences(&serde_json::to_vec(&wire).unwrap()).is_err());
}
#[tokio::test]
async fn release_projection_refuses_corruption_and_product_fixture_inventory() {
    use concert_application::bootstrap::{
        BootstrapDefaults, BootstrapRelease, ValidatedProductRelease,
    };
    let mut document: Value = serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/core/bootstrap-open.json"
    ))
    .unwrap();
    let command =
        concert_domain::avatar::SetAvatar::parse(&command(1, document["avatar"]["config"].clone()))
            .unwrap();
    let defaults = BootstrapDefaults {
        theme: Theme::Revenge,
        avatar: command.selection,
    };
    let make_release = |doc: &Value| {
        Arc::new(
            ValidatedProductRelease::from_validated(
                BootstrapRelease::new(defaults.clone(), serde_json::to_vec(doc).unwrap()).unwrap(),
                "a".repeat(64),
            )
            .unwrap(),
        )
    };
    // No connection is attempted by validation. This test address is not a
    // database fixture, and cannot authorize a live write.
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused@127.0.0.1:1/unused")
        .unwrap();
    assert!(AvatarPersistence::from_product(pool.clone(), make_release(&document)).is_err());
    assert!(
        AvatarPersistence::from_synthetic_test(pool.clone(), make_release(&document), false)
            .is_err()
    );
    assert!(
        AvatarPersistence::from_synthetic_test(pool.clone(), make_release(&document), true).is_ok()
    );
    document["avatar_manifest"]["atlas"]["items"][0]["item_id"] = json!("tampered");
    assert!(AvatarPersistence::from_synthetic_test(pool, make_release(&document), true).is_err());
}
#[tokio::test]
#[ignore = "requires avatar_smoke.py owned PostgreSQL cluster"]
async fn native_avatar_transactions() {
    let pool = support::pool().await;
    let release = support::release();
    let event = EVENT.parse::<Uuid>().unwrap();
    let user = USER.parse::<Uuid>().unwrap();
    let own = key(event, user);
    support::seed(&pool, event).await;
    let bootstrap = BootstrapPostgresRepository::new(pool.clone());
    for u in [user, Uuid::new_v4()] {
        bootstrap
            .bootstrap(&support::admission(event, u), release.release().defaults())
            .await
            .unwrap();
    }
    assert!(AvatarPersistence::from_product(pool.clone(), release.clone()).is_err());
    assert!(AvatarPersistence::from_synthetic_test(pool.clone(), release.clone(), false).is_err());
    let service = Arc::new(
        AvatarPersistence::from_synthetic_test(pool.clone(), release.clone(), true).unwrap(),
    );
    let mut desired = config_value(&release.release().defaults().avatar);
    desired["item_ids"]["accessory"] = Value::Null;
    let mut tasks = vec![];
    for _ in 0..16 {
        let service = service.clone();
        let bytes = command(1, desired.clone());
        tasks.push(tokio::spawn(async move {
            let a =
                support::Authority::new(own, AvatarOperation::SetAvatar, service.release_digest());
            service.set_avatar(own, &bytes, &a).await
        }));
    }
    let mut winners = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(value) => {
                assert_eq!(value, json!({"revision":2,"config":desired}));
                winners += 1;
            }
            Err(e) => assert_eq!(e, AvatarError::Conflict),
        }
    }
    assert_eq!(winners, 1);
    // A fresh authority failure AFTER SQL update must roll the mutation back.
    let mut fail =
        support::Authority::new(own, AvatarOperation::SetAvatar, service.release_digest());
    fail.fail_at = 8;
    assert_eq!(
        service
            .set_avatar(
                own,
                &command(2, config_value(&release.release().defaults().avatar)),
                &fail
            )
            .await
            .unwrap_err(),
        AvatarError::Unauthenticated
    );
    let read = support::Authority::new(own, AvatarOperation::Read, service.release_digest());
    let before = service.read(own, &read).await.unwrap();
    assert_eq!(before.avatar.revision.get(), 2);
    assert_eq!(config_value(&before.avatar.selection), desired);
    // Preference races use participant revision and retain the complete avatar.
    let mut tasks = vec![];
    for _ in 0..12 {
        let service = service.clone();
        tasks.push(tokio::spawn(async move {
            let a = support::Authority::new(
                own,
                AvatarOperation::SetPreferences,
                service.release_digest(),
            );
            service.set_preferences(own, &prefs(1), &a).await
        }));
    }
    let mut winners = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(v) => {
                assert_eq!(v.participant.revision.get(), 2);
                assert_eq!(v.avatar, before.avatar);
                winners += 1;
            }
            Err(e) => assert_eq!(e, AvatarError::Conflict),
        }
    }
    assert_eq!(winners, 1);
    let after = service.read(own, &read).await.unwrap();
    assert_eq!(after.avatar, before.avatar);
    assert_eq!(after.participant.preferences.theme, Theme::Danger);
    let mut fail = support::Authority::new(
        own,
        AvatarOperation::SetPreferences,
        service.release_digest(),
    );
    fail.fail_at = 8;
    assert_eq!(
        service
            .set_preferences(own, &prefs(2), &fail)
            .await
            .unwrap_err(),
        AvatarError::Unauthenticated
    );
    assert_eq!(service.read(own, &read).await.unwrap(), after);
    let write = support::Authority::new(own, AvatarOperation::SetAvatar, service.release_digest());
    let mut bad = desired.clone();
    bad["item_ids"]["body"] = "eyes_fixture".into();
    assert_eq!(
        service
            .set_avatar(own, &command(2, bad), &write)
            .await
            .unwrap_err(),
        AvatarError::InvalidInput
    );
    // Actor/event/operation/release are checked independently of request payload.
    for foreign in [key(event, Uuid::new_v4()), key(Uuid::new_v4(), user)] {
        assert_eq!(
            service
                .set_avatar(foreign, &command(2, desired.clone()), &write)
                .await
                .unwrap_err(),
            AvatarError::Denied
        );
    }
    let stale = support::Authority::new(own, AvatarOperation::SetAvatar, &"0".repeat(64));
    assert_eq!(
        service
            .set_avatar(own, &command(2, desired.clone()), &stale)
            .await
            .unwrap_err(),
        AvatarError::Unavailable
    );
    assert_eq!(
        service
            .set_avatar(own, &command(2, desired.clone()), &read)
            .await
            .unwrap_err(),
        AvatarError::Denied
    );
    // Missing admissions never create or revive participant defaults.
    let unknown = key(event, Uuid::new_v4());
    let a = support::Authority::new(unknown, AvatarOperation::Read, service.release_digest());
    assert_eq!(
        service.read(unknown, &a).await.unwrap_err(),
        AvatarError::Denied
    );
    // A failure observed only after commit is uncertain, not proof of rollback.
    let uncertain_event = Uuid::new_v4();
    support::seed(&pool, uncertain_event).await;
    bootstrap
        .bootstrap(
            &support::admission(uncertain_event, user),
            release.release().defaults(),
        )
        .await
        .unwrap();
    let uncertain_key = key(uncertain_event, user);
    let mut uncertain = support::Authority::new(
        uncertain_key,
        AvatarOperation::SetAvatar,
        service.release_digest(),
    );
    uncertain.fail_at = 10;
    assert_eq!(
        service
            .set_avatar(uncertain_key, &command(1, desired.clone()), &uncertain)
            .await
            .unwrap_err(),
        AvatarError::Unauthenticated
    );
    let read_uncertain = support::Authority::new(
        uncertain_key,
        AvatarOperation::Read,
        service.release_digest(),
    );
    let committed = service.read(uncertain_key, &read_uncertain).await.unwrap();
    assert_eq!(committed.avatar.revision.get(), 2);
    assert_eq!(config_value(&committed.avatar.selection), desired);
    // A write waiting behind event closure rechecks the new phase under lock.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("select event_id from concert_events where event_id=$1 for update")
        .bind(event)
        .execute(&mut *tx)
        .await
        .unwrap();
    let delayed = service.clone();
    let config = desired.clone();
    let probe = Arc::new(support::Authority::new(
        own,
        AvatarOperation::SetAvatar,
        service.release_digest(),
    ));
    let signal = probe.clone();
    let waiting =
        tokio::spawn(async move { delayed.set_avatar(own, &command(2, config), &*signal).await });
    while probe.calls.load(std::sync::atomic::Ordering::SeqCst) < 3 {
        tokio::task::yield_now().await;
    }
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(event)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(waiting.await.unwrap().unwrap_err(), AvatarError::ReadOnly);
    assert_eq!(service.read(own, &read).await.unwrap(), after);
    for phase in ["ARCHIVED", "DRAFT", "OPEN"] {
        sqlx::query("update concert_events set phase=$2,revision=revision+1 where event_id=$1")
            .bind(event)
            .bind(phase)
            .execute(&pool)
            .await
            .unwrap();
        if phase != "OPEN" {
            let error = service
                .set_avatar(own, &command(2, desired.clone()), &write)
                .await
                .unwrap_err();
            assert_eq!(
                error,
                if phase == "DRAFT" {
                    AvatarError::Missing
                } else {
                    AvatarError::ReadOnly
                }
            );
        }
    }
    // Suspension wins both reads and writes; no state is leaked or overwritten.
    sqlx::query("update concert_event_admissions set state='SUSPENDED',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2").bind(event).bind(user).execute(&pool).await.unwrap();
    assert_eq!(
        service.read(own, &read).await.unwrap_err(),
        AvatarError::Denied
    );
    assert_eq!(
        service
            .set_avatar(own, &command(2, desired.clone()), &write)
            .await
            .unwrap_err(),
        AvatarError::Denied
    );
    // Re-enable fixture for postmaster restart verification. This is test setup.
    sqlx::query("update concert_event_admissions set state='ACTIVE',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2").bind(event).bind(user).execute(&pool).await.unwrap();
    assert_eq!(service.read(own, &read).await.unwrap(), after);
    let untouched:i64=sqlx::query_scalar("select count(*) from concert_avatar_settings where event_id=$1 and user_id<>$2 and revision=1 and accessory_id is not null").bind(event).bind(user).fetch_one(&pool).await.unwrap();
    assert_eq!(untouched, 1);
    std::fs::write(
        std::path::PathBuf::from(std::env::var("MCR_BOOTSTRAP_ROOT").unwrap())
            .join("avatar-state.json"),
        serde_json::to_vec(&avatar_state(&after.avatar)).unwrap(),
    )
    .unwrap();
    pool.close().await;
}
#[tokio::test]
#[ignore = "requires avatar_smoke.py restarted owned PostgreSQL cluster"]
async fn native_avatar_restart_persistence() {
    let pool = support::pool().await;
    let release = support::release();
    let own = key(EVENT.parse().unwrap(), USER.parse().unwrap());
    let service = AvatarPersistence::from_synthetic_test(pool.clone(), release, true).unwrap();
    let read = support::Authority::new(own, AvatarOperation::Read, service.release_digest());
    let snapshot = service.read(own, &read).await.unwrap();
    assert_eq!(snapshot.avatar.revision.get(), 2);
    assert!(matches!(
        &snapshot.avatar.selection.appearance,
        AvatarAppearance::Layered {
            accessory: None,
            ..
        }
    ));
    assert_eq!(snapshot.participant.revision.get(), 2);
    assert_eq!(snapshot.participant.preferences.theme, Theme::Danger);
    let old: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::PathBuf::from(std::env::var("MCR_BOOTSTRAP_ROOT").unwrap())
                .join("avatar-state.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(old, avatar_state(&snapshot.avatar));
    pool.close().await;
}

const SELECTED_EVENT: &str = "00000000-0000-4000-8000-000000000711";
const SELECTED_USER: &str = "00000000-0000-4000-8000-000000000712";
fn selected_manifest() -> Value {
    serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json"
    ))
    .unwrap()
}
#[tokio::test]
#[ignore = "requires avatar_smoke.py owned PostgreSQL cluster"]
async fn native_selected_avatar_transactions() {
    let pool = support::pool().await;
    let release = support::selected_release();
    let event: Uuid = SELECTED_EVENT.parse().unwrap();
    let user: Uuid = SELECTED_USER.parse().unwrap();
    let own = key(event, user);
    support::seed(&pool, event).await;
    let bootstrap = BootstrapPostgresRepository::new(pool.clone());
    let first = bootstrap
        .bootstrap(
            &support::admission(event, user),
            release.release().defaults(),
        )
        .await
        .unwrap();
    assert_eq!(
        config_value(&first.avatar),
        selected_manifest()["presets"][0]["config"]
    );
    let service = Arc::new(
        AvatarPersistence::from_synthetic_test(pool.clone(), release.clone(), true).unwrap(),
    );
    let write = support::Authority::new(own, AvatarOperation::SetAvatar, service.release_digest());
    let read = support::Authority::new(own, AvatarOperation::Read, service.release_digest());
    let mut revision = 1;
    for preset in selected_manifest()["presets"].as_array().unwrap() {
        let desired = preset["config"].clone();
        let state = service
            .set_avatar(own, &command(revision, desired.clone()), &write)
            .await
            .unwrap();
        revision += 1;
        assert_eq!(state, json!({"revision":revision,"config":desired}));
        let snapshot = service.read(own, &read).await.unwrap();
        assert_eq!(avatar_state(&snapshot.avatar), state);
        // Canonical CoreRepository GET projection uses the same exclusive decoder.
        use concert_application::CoreRepository;
        let core =
            concert_service::adapters::out::repository::PgConcertRepository::new(pool.clone());
        assert_eq!(core.avatar(own).await.unwrap().unwrap(), snapshot.avatar);
        let stored:(String,String,String,bool) = sqlx::query_as("select contract_version,representation,preset_id,body_id is null and eyes_id is null and hair_id is null and outfit_id is null and accessory_id is null and skin_tone_id is null from concert_avatar_settings where event_id=$1 and user_id=$2")
            .bind(event).bind(user).fetch_one(&pool).await.unwrap();
        assert_eq!(
            stored,
            (
                "1.1.0".into(),
                "whole_character".into(),
                preset["preset_id"].as_str().unwrap().into(),
                true
            )
        );
        // Repeated bootstrap preserves a saved preset instead of default substitution.
        let again = bootstrap
            .bootstrap(
                &support::admission(event, user),
                release.release().defaults(),
            )
            .await
            .unwrap();
        assert_eq!(config_value(&again.avatar), desired);
        assert_eq!(again.avatar_revision, revision);
    }
    let desired = selected_manifest()["presets"][2]["config"].clone();
    let before = service.read(own, &read).await.unwrap();
    for (field, bad) in [
        ("preset_id", json!("photo01_front_07")),
        ("manifest_id", json!("other")),
        ("manifest_version", json!(2)),
        ("item_ids", json!({})),
        ("skin_tone_id", json!("tone_fixture")),
        ("url", json!("https://evil.invalid/a.png")),
        ("frame", json!(0)),
    ] {
        let mut invalid = desired.clone();
        invalid[field] = bad;
        assert_eq!(
            service
                .set_avatar(own, &command(revision, invalid), &write)
                .await
                .unwrap_err(),
            AvatarError::InvalidInput
        );
    }
    assert_eq!(
        service
            .set_avatar(own, &command(1, desired.clone()), &write)
            .await
            .unwrap_err(),
        AvatarError::Conflict
    );
    let wrong = support::Authority::new(
        key(event, Uuid::new_v4()),
        AvatarOperation::SetAvatar,
        service.release_digest(),
    );
    assert_eq!(
        service
            .set_avatar(own, &command(revision, desired.clone()), &wrong)
            .await
            .unwrap_err(),
        AvatarError::Denied
    );
    let wrong_event = support::Authority::new(
        key(Uuid::new_v4(), user),
        AvatarOperation::SetAvatar,
        service.release_digest(),
    );
    assert_eq!(
        service
            .set_avatar(own, &command(revision, desired.clone()), &wrong_event)
            .await
            .unwrap_err(),
        AvatarError::Denied
    );
    let wrong_release = support::Authority::new(own, AvatarOperation::SetAvatar, &"b".repeat(64));
    assert_eq!(
        service
            .set_avatar(own, &command(revision, desired.clone()), &wrong_release)
            .await
            .unwrap_err(),
        AvatarError::Unavailable
    );
    let mut fail =
        support::Authority::new(own, AvatarOperation::SetAvatar, service.release_digest());
    fail.fail_at = 8; // after SQL UPDATE, before commit
    assert_eq!(
        service
            .set_avatar(own, &command(revision, desired.clone()), &fail)
            .await
            .unwrap_err(),
        AvatarError::Unauthenticated
    );
    assert_eq!(service.read(own, &read).await.unwrap(), before);
    // Server storage constraints independently reject mixed/unknown variants.
    for expression in [
        "body_id='body_fixture'",
        "preset_id='photo01_front_07'",
        "representation='layered'",
        "contract_version='1.0.0'",
    ] {
        let sql = format!("update concert_avatar_settings set revision=revision+1,{expression} where event_id=$1 and user_id=$2");
        let err = sqlx::query(&sql)
            .bind(event)
            .bind(user)
            .execute(&pool)
            .await
            .unwrap_err();
        assert_eq!(
            err.as_database_error().unwrap().code().as_deref(),
            Some("23514")
        );
    }
    let mut tasks = vec![];
    for _ in 0..16 {
        let service = service.clone();
        let bytes = command(revision, desired.clone());
        tasks.push(tokio::spawn(async move {
            let a =
                support::Authority::new(own, AvatarOperation::SetAvatar, service.release_digest());
            service.set_avatar(own, &bytes, &a).await
        }));
    }
    let mut winners = 0;
    for task in tasks {
        match task.await.unwrap() {
            Ok(value) => {
                winners += 1;
                assert_eq!(value, json!({"revision":revision+1,"config":desired}));
            }
            Err(e) => assert_eq!(e, AvatarError::Conflict),
        }
    }
    assert_eq!(winners, 1);
    revision += 1;
    let preference = support::Authority::new(
        own,
        AvatarOperation::SetPreferences,
        service.release_digest(),
    );
    let changed = service
        .set_preferences(own, &prefs(1), &preference)
        .await
        .unwrap();
    assert_eq!(changed.avatar.revision.get(), revision);
    assert_eq!(config_value(&changed.avatar.selection), desired);
    // Closure while a write waits on the event lock must prevent its SQL mutation.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("select event_id from concert_events where event_id=$1 for update")
        .bind(event)
        .execute(&mut *tx)
        .await
        .unwrap();
    let signal = Arc::new(support::Authority::new(
        own,
        AvatarOperation::SetAvatar,
        service.release_digest(),
    ));
    let probe = signal.clone();
    let delayed = service.clone();
    let bytes = command(
        revision,
        selected_manifest()["presets"][0]["config"].clone(),
    );
    let waiting = tokio::spawn(async move { delayed.set_avatar(own, &bytes, &*signal).await });
    while probe.calls.load(std::sync::atomic::Ordering::SeqCst) < 3 {
        tokio::task::yield_now().await;
    }
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(event)
    .execute(&mut *tx)
    .await
    .unwrap();
    tx.commit().await.unwrap();
    assert_eq!(waiting.await.unwrap().unwrap_err(), AvatarError::ReadOnly);
    assert_eq!(service.read(own, &read).await.unwrap(), changed);
    let row = avatar_state(&changed.avatar);
    std::fs::write(
        std::path::PathBuf::from(std::env::var("MCR_BOOTSTRAP_ROOT").unwrap())
            .join("selected-avatar-state.json"),
        serde_json::to_vec(&row).unwrap(),
    )
    .unwrap();
    // A distinct event keeps the selected-avatar restart oracle unchanged.
    native_actual_effect_fence(&pool, release.clone()).await;
    pool.close().await;
}
#[tokio::test]
#[ignore = "requires avatar_smoke.py postmaster restart"]
async fn native_selected_avatar_restart_persistence() {
    let pool = support::pool().await;
    let release = support::selected_release();
    let service = AvatarPersistence::from_synthetic_test(pool.clone(), release, true).unwrap();
    let own = key(
        SELECTED_EVENT.parse().unwrap(),
        SELECTED_USER.parse().unwrap(),
    );
    let read = support::Authority::new(own, AvatarOperation::Read, service.release_digest());
    let snapshot = service.read(own, &read).await.unwrap();
    let expected: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::PathBuf::from(std::env::var("MCR_BOOTSTRAP_ROOT").unwrap())
                .join("selected-avatar-state.json"),
        )
        .unwrap(),
    )
    .unwrap();
    assert_eq!(avatar_state(&snapshot.avatar), expected);
    assert_eq!(snapshot.participant.preferences.theme, Theme::Danger);
    assert_eq!(
        service
            .set_avatar(
                own,
                &command(
                    snapshot.avatar.revision.get(),
                    selected_manifest()["presets"][0]["config"].clone()
                ),
                &support::Authority::new(own, AvatarOperation::SetAvatar, service.release_digest())
            )
            .await
            .unwrap_err(),
        AvatarError::ReadOnly
    );
    let witness: Value = serde_json::from_slice(
        &std::fs::read(
            std::path::PathBuf::from(std::env::var("MCR_BOOTSTRAP_ROOT").unwrap())
                .join("avatar-witness-state.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let rows: Value = sqlx::query_scalar("select jsonb_agg(to_jsonb(w) order by operation_id) from concert_participant_effect_witnesses w where event_id=$1 and actor_id=$2")
        .bind(witness["event"].as_str().unwrap().parse::<Uuid>().unwrap())
        .bind(witness["actor"].as_str().unwrap().parse::<Uuid>().unwrap())
        .fetch_one(&pool).await.unwrap();
    assert_eq!(rows, witness["rows"]);
    pool.close().await;
}

// Component-only fence. It observes a real native transaction but is not the
// external Auth/Friend scope adapter and never claims distributed settlement.
#[derive(Clone, Copy)]
enum WitnessFault {
    Healthy,
    MissingDescriptor,
    WrongRelease,
    WrongBody,
    WrongScope,
    Unpublished,
    Unconfirmed,
    AfterWitness,
    AfterCommit,
}
struct NativeEffectFence {
    pool: sqlx::PgPool,
    registered: std::sync::Mutex<Option<AvatarTransactionIdentity>>,
    reject_registration: bool,
    reject_after_effect: bool,
    witness: AvatarEffectWitnessBinding,
    owner_binding: Value,
    fault: WitnessFault,
}
#[async_trait::async_trait]
impl AvatarEffectFence for NativeEffectFence {
    async fn register(&self, identity: AvatarTransactionIdentity) -> Result<(), AvatarError> {
        let active: (i64, String, chrono::DateTime<chrono::Utc>, String) = sqlx::query_as(
            "select datid::bigint,backend_xid::text,backend_start,state from pg_stat_activity where pid=$1"
        ).bind(identity.backend_pid).fetch_one(&self.pool).await.unwrap();
        assert_eq!(active.0, i64::from(identity.database_oid));
        assert_eq!(
            active.1.parse::<u64>().unwrap(),
            identity.full_xid.parse::<u64>().unwrap() & 0xffff_ffff
        );
        assert_eq!(active.2, identity.backend_started);
        assert_eq!(active.3, "idle in transaction");
        let sysid: String =
            sqlx::query_scalar("select system_identifier::text from pg_control_system()")
                .fetch_one(&self.pool)
                .await
                .unwrap();
        assert_eq!(identity.system_identifier, sysid);
        *self.registered.lock().unwrap() = Some(identity);
        if self.reject_registration {
            return Err(AvatarError::Unavailable);
        }
        // Explicit component fixture: real SQL scope/identity constraints, not
        // real Auth/Friend registration or a production admission capability.
        let identity = self.registered.lock().unwrap().clone().unwrap();
        let transaction = concert_service::product_rpc::scope_ledger::ProducerTransaction {
            producer: "CONCERT".into(),
            system_identifier: identity.system_identifier,
            database_oid: identity.database_oid,
            full_xid: identity.full_xid,
            backend_pid: identity.backend_pid,
            backend_started_unix_micros: identity.backend_started.timestamp_micros(),
            effect_binding_digest: self.witness.effect_binding_digest,
            producer_instance: self.witness.producer_instance_id,
            settlement_reference: self.witness.operation_id.as_bytes().to_vec(),
        };
        let digest = transaction.digest().unwrap();
        let delivery = if matches!(self.fault, WitnessFault::Unpublished) {
            None
        } else {
            Some(vec![77u8; 32])
        };
        sqlx::query("insert into concert_product_owner_scopes(instance_id,scope_id,binding_digest,binding,delivery_secret_hash) values($1,$2,$3,$4,$5)")
            .bind(self.witness.producer_instance_id).bind(self.witness.scope_id)
            .bind(self.witness.guard_binding_digest.as_slice()).bind(&self.owner_binding).bind(delivery)
            .execute(&self.pool).await.unwrap();
        let confirmed = if matches!(self.fault, WitnessFault::Unconfirmed) {
            None
        } else {
            Some(chrono::Utc::now())
        };
        sqlx::query("insert into concert_product_owner_transactions(instance_id,scope_id,binding_digest,identity_digest,identity,auth_registration_id,friend_registration_id,confirmed_at) values($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind(self.witness.producer_instance_id).bind(self.witness.scope_id)
            .bind(self.witness.guard_binding_digest.as_slice()).bind(digest.as_slice())
            .bind(serde_json::to_value(transaction).unwrap()).bind(Uuid::new_v4()).bind(Uuid::new_v4())
            .bind(confirmed).execute(&self.pool).await.unwrap();
        Ok(())
    }
    fn witness_binding(&self) -> Result<AvatarEffectWitnessBinding, AvatarError> {
        let mut value = self.witness.clone();
        match self.fault {
            WitnessFault::MissingDescriptor => return Err(AvatarError::Unavailable),
            WitnessFault::WrongRelease => value.release_digest = "b".repeat(64),
            WitnessFault::WrongBody => value.body_digest = [99; 32],
            WitnessFault::WrongScope => value.scope_id = Uuid::from_u128(42),
            _ => (),
        }
        Ok(value)
    }
    async fn check(&self) -> Result<(), AvatarError> {
        let identity = self
            .registered
            .lock()
            .unwrap()
            .clone()
            .expect("registered before checks");
        if matches!(self.fault, WitnessFault::AfterCommit) {
            let committed: bool = sqlx::query_scalar("select exists(select 1 from concert_participant_effect_witnesses where operation_id=$1)")
                .bind(self.witness.operation_id).fetch_one(&self.pool).await.unwrap();
            if committed {
                return Err(AvatarError::Unavailable);
            }
        }
        if self.reject_after_effect || matches!(self.fault, WitnessFault::AfterWitness) {
            let query: String =
                sqlx::query_scalar("select query from pg_stat_activity where pid=$1")
                    .bind(identity.backend_pid)
                    .fetch_one(&self.pool)
                    .await
                    .unwrap();
            if self.reject_after_effect && query.starts_with("update public.concert_")
                || matches!(self.fault, WitnessFault::AfterWitness)
                    && query.starts_with("insert into public.concert_participant_effect_witnesses")
            {
                return Err(AvatarError::Denied);
            }
        }
        Ok(())
    }
}
struct FencedAvatarAuthority {
    base: support::Authority,
    fence: NativeEffectFence,
}
impl AvatarAuthority for FencedAvatarAuthority {
    fn check(&self) -> Result<AvatarGrant, AvatarError> {
        self.base.check()
    }
    fn effect_fence(&self) -> Option<&dyn AvatarEffectFence> {
        Some(&self.fence)
    }
}
async fn native_actual_effect_fence(
    pool: &sqlx::PgPool,
    release: Arc<concert_application::bootstrap::ValidatedProductRelease>,
) {
    let event = Uuid::new_v4();
    let user = Uuid::new_v4();
    let own = key(event, user);
    support::seed(pool, event).await;
    BootstrapPostgresRepository::new(pool.clone())
        .bootstrap(
            &support::admission(event, user),
            release.release().defaults(),
        )
        .await
        .unwrap();
    let document: Value = serde_json::from_slice(release.release().bootstrap_json()).unwrap();
    let catalog = concert_domain::avatar::AvatarCatalog::from_validated_manifest(
        &document["avatar_manifest"],
    )
    .unwrap();
    let repository = PgAvatarRepository::new(pool.clone());
    let selection = release.release().defaults().avatar.clone();
    let mutation = || {
        AvatarMutation::Avatar(concert_domain::avatar::SetAvatar {
            expected: Revision::new(1).unwrap(),
            selection: selection.clone(),
        })
    };
    let unfenced =
        support::Authority::new(own, AvatarOperation::SetAvatar, release.source_digest());
    assert_eq!(
        repository
            .execute(
                own,
                mutation(),
                &catalog,
                release.source_digest(),
                &unfenced
            )
            .await
            .unwrap_err(),
        AvatarError::Unavailable
    );
    let reader = support::Authority::new(own, AvatarOperation::Read, release.source_digest());
    let before = repository
        .execute(
            own,
            AvatarMutation::Read,
            &catalog,
            release.source_digest(),
            &reader,
        )
        .await
        .unwrap();
    // The actual update's trigger records its identity in the same transaction.
    sqlx::raw_sql("create table mcr_avatar_effect_probe (table_name text,system_identifier text,database_oid bigint,full_xid text,backend_pid int,backend_started timestamptz); create function mcr_avatar_effect_probe_fn() returns trigger language plpgsql as $$ begin insert into mcr_avatar_effect_probe select TG_TABLE_NAME,c.system_identifier::text,d.oid::bigint,pg_current_xact_id()::text,a.pid,a.backend_start from pg_control_system() c join pg_database d on d.datname=current_database() join pg_stat_activity a on a.pid=pg_backend_pid(); return NEW; end $$; create trigger mcr_avatar_effect_probe_avatar after update on concert_avatar_settings for each row execute function mcr_avatar_effect_probe_fn(); create trigger mcr_avatar_effect_probe_preferences after update on concert_participants for each row execute function mcr_avatar_effect_probe_fn();")
        .execute(pool).await.unwrap();
    let authority = |op, reject_registration, reject_after_effect| {
        let body = if op == AvatarOperation::SetAvatar {
            command(1, config_value(&selection))
        } else {
            prefs(1)
        };
        let owner_binding = platform::actor_delegation::owner_binding::OwnerBinding {
            actor_id: user,
            event_id: event,
            session_family_id: Uuid::new_v4(),
            request_id: Uuid::new_v4(),
            operation: "component_avatar_witness".into(),
            expires_unix_micros: (chrono::Utc::now() + chrono::Duration::minutes(5))
                .timestamp_micros(),
            parent_digest: [11; 32],
            parent_body_digest: [12; 32],
            parent_method: if op == AvatarOperation::SetAvatar {
                "/concert.participant.v1.ConcertParticipantService/SetAvatar".into()
            } else {
                "/concert.participant.v1.ConcertParticipantService/UpdatePreferences".into()
            },
            http_method: "PUT".into(),
            http_target: format!("/api/concert/events/{event}/component-witness"),
            body_digest: Sha256::digest(&body).into(),
        };
        let witness = AvatarEffectWitnessBinding {
            producer_instance_id: Uuid::new_v4(),
            scope_id: Uuid::new_v4(),
            operation_id: owner_binding.request_id,
            session_family_id: owner_binding.session_family_id,
            parent_digest: owner_binding.parent_digest,
            guard_binding_digest: owner_binding.digest().unwrap(),
            body_digest: owner_binding.body_digest,
            release_digest: release.source_digest().into(),
            effect_binding_digest: [44; 32],
        };
        FencedAvatarAuthority {
            base: support::Authority::new(own, op, release.source_digest()),
            fence: NativeEffectFence {
                pool: pool.clone(),
                registered: std::sync::Mutex::new(None),
                reject_registration,
                reject_after_effect,
                witness,
                owner_binding: serde_json::to_value(owner_binding).unwrap(),
                fault: WitnessFault::Healthy,
            },
        }
    };
    for (registration, after, error) in [
        (true, false, AvatarError::Unavailable),
        (false, true, AvatarError::Denied),
    ] {
        let fence = authority(AvatarOperation::SetAvatar, registration, after);
        assert_eq!(
            repository
                .execute(own, mutation(), &catalog, release.source_digest(), &fence)
                .await
                .unwrap_err(),
            error
        );
        assert!(fence.fence.registered.lock().unwrap().is_some());
        assert_eq!(
            repository
                .execute(
                    own,
                    AvatarMutation::Read,
                    &catalog,
                    release.source_digest(),
                    &reader
                )
                .await
                .unwrap(),
            before
        );
        let effects: i64 = sqlx::query_scalar("select count(*) from mcr_avatar_effect_probe")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(
            effects, 0,
            "registration/precommit rejection must roll back effects and trigger evidence"
        );
    }
    for fault in [
        WitnessFault::MissingDescriptor,
        WitnessFault::WrongRelease,
        WitnessFault::WrongBody,
        WitnessFault::WrongScope,
        WitnessFault::Unpublished,
        WitnessFault::Unconfirmed,
        WitnessFault::AfterWitness,
    ] {
        let mut fence = authority(AvatarOperation::SetAvatar, false, false);
        fence.fence.fault = fault;
        let error = repository
            .execute(own, mutation(), &catalog, release.source_digest(), &fence)
            .await
            .unwrap_err();
        assert_eq!(
            error,
            if matches!(fault, WitnessFault::AfterWitness) {
                AvatarError::Denied
            } else {
                AvatarError::Unavailable
            }
        );
        assert_eq!(
            repository
                .execute(
                    own,
                    AvatarMutation::Read,
                    &catalog,
                    release.source_digest(),
                    &reader
                )
                .await
                .unwrap(),
            before
        );
        let count: i64 = sqlx::query_scalar("select count(*) from concert_participant_effect_witnesses where event_id=$1 and actor_id=$2")
            .bind(event).bind(user).fetch_one(pool).await.unwrap();
        assert_eq!(count, 0, "failure must roll back both effect and witness");
        let effects: i64 = sqlx::query_scalar("select count(*) from mcr_avatar_effect_probe")
            .fetch_one(pool)
            .await
            .unwrap();
        assert_eq!(effects, 0);
    }
    let fence = authority(AvatarOperation::SetAvatar, false, false);
    let changed = repository
        .execute(own, mutation(), &catalog, release.source_digest(), &fence)
        .await
        .unwrap();
    assert_eq!(changed.avatar.revision.get(), 2);
    assert_native_witness(pool, &fence.fence, own, "AVATAR", 1, 2).await;
    let identity = fence.fence.registered.lock().unwrap().clone().unwrap();
    let actual:(String,String,i64,String,i32,chrono::DateTime<chrono::Utc>)=sqlx::query_as("select table_name,system_identifier,database_oid,full_xid,backend_pid,backend_started from mcr_avatar_effect_probe").fetch_one(pool).await.unwrap();
    assert_eq!(
        actual,
        (
            "concert_avatar_settings".into(),
            identity.system_identifier,
            i64::from(identity.database_oid),
            identity.full_xid,
            identity.backend_pid,
            identity.backend_started
        )
    );
    sqlx::query("truncate mcr_avatar_effect_probe")
        .execute(pool)
        .await
        .unwrap();
    let fence = authority(AvatarOperation::SetPreferences, false, false);
    let changed = repository
        .execute(
            own,
            AvatarMutation::Preferences {
                expected: Revision::new(1).unwrap(),
                value: Preferences {
                    theme: Theme::Danger,
                    low_power: true,
                    reduced_motion: true,
                    sound_enabled: false,
                },
            },
            &catalog,
            release.source_digest(),
            &fence,
        )
        .await
        .unwrap();
    assert_eq!(changed.participant.revision.get(), 2);
    assert_native_witness(pool, &fence.fence, own, "PREFERENCES", 1, 2).await;
    let identity = fence.fence.registered.lock().unwrap().clone().unwrap();
    let actual:(String,String,i64,String,i32,chrono::DateTime<chrono::Utc>)=sqlx::query_as("select table_name,system_identifier,database_oid,full_xid,backend_pid,backend_started from mcr_avatar_effect_probe").fetch_one(pool).await.unwrap();
    assert_eq!(
        actual,
        (
            "concert_participants".into(),
            identity.system_identifier,
            i64::from(identity.database_oid),
            identity.full_xid,
            identity.backend_pid,
            identity.backend_started
        )
    );
    let mut late = authority(AvatarOperation::SetAvatar, false, false);
    late.fence.fault = WitnessFault::AfterCommit;
    // Bind the actual second request body for this separate operation.
    let mut binding: platform::actor_delegation::owner_binding::OwnerBinding =
        serde_json::from_value(late.fence.owner_binding.clone()).unwrap();
    binding.body_digest = Sha256::digest(command(2, config_value(&selection))).into();
    late.fence.witness.body_digest = binding.body_digest;
    late.fence.witness.guard_binding_digest = binding.digest().unwrap();
    late.fence.owner_binding = serde_json::to_value(binding).unwrap();
    assert_eq!(
        repository
            .execute(
                own,
                AvatarMutation::Avatar(concert_domain::avatar::SetAvatar {
                    expected: Revision::new(2).unwrap(),
                    selection: selection.clone(),
                }),
                &catalog,
                release.source_digest(),
                &late
            )
            .await
            .unwrap_err(),
        AvatarError::Unavailable
    );
    assert_native_witness(pool, &late.fence, own, "AVATAR", 2, 3).await;
    let committed = repository
        .execute(
            own,
            AvatarMutation::Read,
            &catalog,
            release.source_digest(),
            &reader,
        )
        .await
        .unwrap();
    assert_eq!(committed.avatar.revision.get(), 3);
    for statement in [
        "update concert_participant_effect_witnesses set committed_revision=committed_revision where operation_id=$1",
        "delete from concert_participant_effect_witnesses where operation_id=$1",
        "insert into concert_participant_effect_witnesses select * from concert_participant_effect_witnesses where operation_id=$1",
    ] {
        let error = sqlx::query(statement).bind(late.fence.witness.operation_id).execute(pool).await.unwrap_err();
        assert_eq!(error.as_database_error().unwrap().code().as_deref(), Some("23514"));
    }
    let witnesses: Value = sqlx::query_scalar("select jsonb_agg(to_jsonb(w) order by operation_id) from concert_participant_effect_witnesses w where event_id=$1 and actor_id=$2")
        .bind(event).bind(user).fetch_one(pool).await.unwrap();
    assert_eq!(witnesses.as_array().unwrap().len(), 3);
    std::fs::write(
        std::path::PathBuf::from(std::env::var("MCR_BOOTSTRAP_ROOT").unwrap())
            .join("avatar-witness-state.json"),
        serde_json::to_vec(&json!({"event":event,"actor":user,"rows":witnesses})).unwrap(),
    )
    .unwrap();
    sqlx::raw_sql("drop trigger mcr_avatar_effect_probe_avatar on concert_avatar_settings; drop trigger mcr_avatar_effect_probe_preferences on concert_participants; drop function mcr_avatar_effect_probe_fn(); drop table mcr_avatar_effect_probe;").execute(pool).await.unwrap();
}

async fn assert_native_witness(
    pool: &sqlx::PgPool,
    fence: &NativeEffectFence,
    key: ParticipantKey,
    kind: &str,
    expected: i64,
    committed: i64,
) {
    let w = &fence.witness;
    let identity = fence.registered.lock().unwrap().clone().unwrap();
    let row =
        sqlx::query("select * from concert_participant_effect_witnesses where operation_id=$1")
            .bind(w.operation_id)
            .fetch_one(pool)
            .await
            .unwrap();
    for (name, expected) in [
        ("operation_id", w.operation_id),
        ("actor_id", key.user_id.uuid()),
        ("event_id", key.event_id.uuid()),
        ("session_family_id", w.session_family_id),
        ("producer_instance_id", w.producer_instance_id),
        ("scope_id", w.scope_id),
    ] {
        assert_eq!(row.get::<Uuid, _>(name), expected);
    }
    for (name, expected) in [
        ("parent_digest", w.parent_digest),
        ("guard_binding_digest", w.guard_binding_digest),
        ("body_digest", w.body_digest),
        ("effect_binding_digest", w.effect_binding_digest),
    ] {
        assert_eq!(row.get::<Vec<u8>, _>(name), expected);
    }
    assert_eq!(row.get::<String, _>("release_digest"), w.release_digest);
    assert_eq!(row.get::<String, _>("mutation_kind"), kind);
    assert_eq!(row.get::<i64, _>("expected_revision"), expected);
    assert_eq!(row.get::<i64, _>("committed_revision"), committed);
    assert_eq!(
        row.get::<String, _>("concert_system_identifier"),
        identity.system_identifier
    );
    assert_eq!(
        row.get::<i64, _>("concert_database_oid"),
        i64::from(identity.database_oid)
    );
    assert_eq!(row.get::<String, _>("concert_full_xid"), identity.full_xid);
    assert_eq!(
        row.get::<i32, _>("concert_backend_pid"),
        identity.backend_pid
    );
    assert_eq!(
        row.get::<chrono::DateTime<chrono::Utc>, _>("concert_backend_started"),
        identity.backend_started
    );
    let digest: Vec<u8> = sqlx::query_scalar("select identity_digest from concert_product_owner_transactions where instance_id=$1 and scope_id=$2")
        .bind(w.producer_instance_id).bind(w.scope_id).fetch_one(pool).await.unwrap();
    assert_eq!(row.get::<Vec<u8>, _>("identity_digest"), digest);
}
