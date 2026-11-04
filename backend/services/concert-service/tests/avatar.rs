#[path = "support/avatar/mod.rs"]
mod support;
use concert_application::{avatar::*, bootstrap::BootstrapRepository};
use concert_domain::{avatar::config_value, *};
use concert_service::{
    adapters::out::repository::avatar::*, bootstrap::BootstrapPostgresRepository,
};
use serde_json::{json, Value};
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
    pool.close().await;
}

// Component-only fence. It observes a real native transaction but is not the
// external Auth/Friend scope adapter and never claims distributed settlement.
struct NativeEffectFence {
    pool: sqlx::PgPool,
    registered: std::sync::Mutex<Option<AvatarTransactionIdentity>>,
    reject_registration: bool,
    reject_after_effect: bool,
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
            Err(AvatarError::Unavailable)
        } else {
            Ok(())
        }
    }
    async fn check(&self) -> Result<(), AvatarError> {
        let identity = self
            .registered
            .lock()
            .unwrap()
            .clone()
            .expect("registered before checks");
        if self.reject_after_effect {
            let query: String =
                sqlx::query_scalar("select query from pg_stat_activity where pid=$1")
                    .bind(identity.backend_pid)
                    .fetch_one(&self.pool)
                    .await
                    .unwrap();
            if query.starts_with("update public.concert_") {
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
    let authority = |op, reject_registration, reject_after_effect| FencedAvatarAuthority {
        base: support::Authority::new(own, op, release.source_digest()),
        fence: NativeEffectFence {
            pool: pool.clone(),
            registered: std::sync::Mutex::new(None),
            reject_registration,
            reject_after_effect,
        },
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
    let fence = authority(AvatarOperation::SetAvatar, false, false);
    let changed = repository
        .execute(own, mutation(), &catalog, release.source_digest(), &fence)
        .await
        .unwrap();
    assert_eq!(changed.avatar.revision.get(), 2);
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
    sqlx::raw_sql("drop trigger mcr_avatar_effect_probe_avatar on concert_avatar_settings; drop trigger mcr_avatar_effect_probe_preferences on concert_participants; drop function mcr_avatar_effect_probe_fn(); drop table mcr_avatar_effect_probe;").execute(pool).await.unwrap();
}
