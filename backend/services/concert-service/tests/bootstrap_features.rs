//! C03 component fixtures, not public Auth/World/IN03 acceptance. The release
//! fixture writer follows tests/bootstrap.rs at the delivery base; supplied
//! contract fixture bytes and production schemas remain unchanged.
use concert_application::bootstrap::{BootstrapAdmission, BootstrapRecord, BootstrapResult};
use concert_domain::{AuthUserId, EventId, EventPhase};
use concert_service::{
    bootstrap::{BootstrapFeatures, ProductReleaseBundle},
    config::bootstrap_features::ConfiguredBootstrapFeatures,
};
use serde_json::{json, Value};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

struct Fixture {
    root: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("mcr-bootstrap-features-{}", Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let doc: Value = serde_json::from_str(include_str!(
            "../../../../contracts/fixtures/core/bootstrap-open.json"
        ))
        .unwrap();
        for (name, value) in [
            ("bootstrap", doc.clone()),
            ("avatar", doc["avatar_manifest"].clone()),
            ("map", doc["map_manifest"].clone()),
        ] {
            std::fs::write(
                root.join(format!("{name}.json")),
                serde_json::to_vec(&value).unwrap(),
            )
            .unwrap();
        }
        for theme in doc["theme_manifests"].as_array().unwrap() {
            let name = theme["theme_id"].as_str().unwrap().to_lowercase();
            std::fs::write(
                root.join(format!("{name}.json")),
                serde_json::to_vec(theme).unwrap(),
            )
            .unwrap();
        }
        Self { root }
    }
    fn release(
        &self,
        features: BootstrapFeatures,
    ) -> Arc<concert_application::bootstrap::ValidatedProductRelease> {
        ProductReleaseBundle::from_synthetic_files(
            &self.root.join("bootstrap.json"),
            &self.root.join("avatar.json"),
            &self.root.join("map.json"),
            [
                &self.root.join("revenge.json"),
                &self.root.join("parade.json"),
                &self.root.join("danger.json"),
            ],
            features,
            true,
        )
        .unwrap()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}
fn document(release: &concert_application::bootstrap::ValidatedProductRelease) -> Value {
    serde_json::from_slice(release.release().bootstrap_json()).unwrap()
}

#[test]
fn absent_producers_fail_closed_without_replacing_validated_source() {
    let f = Fixture::new();
    let source = f.release(BootstrapFeatures::default());
    let settings =
        ConfiguredBootstrapFeatures::from_settings(None, None, None, None, None, None).unwrap();
    let result = settings.bind_release(source.clone()).unwrap();
    assert!(Arc::ptr_eq(&source, &result));
    for v in document(&result)["capabilities"]
        .as_object()
        .unwrap()
        .values()
    {
        assert_eq!(v, &json!({"available":false,"reason":"FEATURE_DISABLED"}));
    }
}

#[test]
fn configured_feature_dto_remains_distinct_from_missing_resources_and_user_policy() {
    let f = Fixture::new();
    let enabled = f.release(BootstrapFeatures {
        world: true,
        feed: true,
        social_sharing: true,
        mutations: true,
        ..Default::default()
    });
    let v = document(&enabled);
    for key in ["feed", "social_sharing", "mutations"] {
        assert_eq!(
            v["capabilities"][key],
            json!({"available":true,"reason":null})
        );
    }
    assert_eq!(
        v["capabilities"]["world"],
        json!({"available":false,"reason":"DEPENDENCY_UNAVAILABLE"})
    );
    assert_eq!(v["participant"]["age_status"], "UNKNOWN");
    for key in ["meet", "gifts", "wall"] {
        assert_eq!(v["capabilities"][key]["available"], false);
    }
    let disabled = ConfiguredBootstrapFeatures::from_settings(None, None, None, None, None, None)
        .unwrap()
        .bind_release(enabled.clone())
        .unwrap();
    assert_ne!(enabled.source_digest(), disabled.source_digest());
    let off = document(&disabled);
    for key in [
        "event",
        "participant",
        "avatar",
        "avatar_manifest",
        "map_manifest",
        "theme_manifests",
        "contract_version",
        "server_time",
    ] {
        assert_eq!(v[key], off[key]);
    }
    assert_eq!(off["capabilities"]["social_sharing"]["available"], false);
    let again = ConfiguredBootstrapFeatures::from_settings(None, None, None, None, None, None)
        .unwrap()
        .bind_release(disabled.clone())
        .unwrap();
    assert!(Arc::ptr_eq(&again, &disabled));
}

#[test]
fn persisted_readonly_and_archive_close_mutations_without_promoting_disabled_features() {
    let f = Fixture::new();
    let release = f.release(BootstrapFeatures {
        feed: true,
        social_sharing: true,
        mutations: true,
        ..Default::default()
    });
    let event = Uuid::parse_str(document(&release)["event"]["event_id"].as_str().unwrap()).unwrap();
    let actor = Uuid::new_v4();
    let admission = BootstrapAdmission::from_verified(
        AuthUserId::from_uuid(actor),
        EventId::from_uuid(event),
        Uuid::new_v4(),
        Uuid::new_v4(),
        "Current public name".into(),
        None,
        chrono::Utc::now(),
    )
    .unwrap();
    for phase in [
        EventPhase::Open,
        EventPhase::Live,
        EventPhase::ReadOnly,
        EventPhase::Archived,
    ] {
        let record = BootstrapRecord {
            event_id: EventId::from_uuid(event),
            event_revision: 2,
            event_slug: "current-event".into(),
            event_title: "Current event".into(),
            event_timezone: "Asia/Seoul".into(),
            phase,
            restriction_revision: 1,
            active: true,
            participant_revision: 1,
            preferences: concert_domain::Preferences {
                theme: concert_domain::Theme::Danger,
                low_power: false,
                reduced_motion: false,
                sound_enabled: false,
            },
            avatar_revision: 1,
            avatar: release.release().defaults().avatar.clone(),
            completed_at: chrono::Utc::now(),
        };
        let result = BootstrapResult {
            record,
            release: release.release().clone(),
        };
        let bytes = concert_service::bootstrap::release::bind_response(
            release.release().bootstrap_json(),
            &admission,
            &result,
            release.source_digest(),
        )
        .unwrap();
        let v: Value = serde_json::from_slice(&bytes).unwrap();
        let writable = matches!(phase, EventPhase::Open | EventPhase::Live);
        assert_eq!(v["capabilities"]["mutations"]["available"], writable);
        assert_eq!(
            v["capabilities"]["mutations"]["reason"],
            if writable {
                Value::Null
            } else {
                json!("EVENT_READ_ONLY")
            }
        );
        assert_eq!(v["capabilities"]["meet"]["available"], false);
        assert_eq!(v["participant"]["age_status"], "UNKNOWN");
        let wrong = BootstrapAdmission::from_verified(
            AuthUserId::from_uuid(actor),
            EventId::from_uuid(Uuid::new_v4()),
            Uuid::new_v4(),
            Uuid::new_v4(),
            "Current public name".into(),
            None,
            chrono::Utc::now(),
        )
        .unwrap();
        assert!(concert_service::bootstrap::release::bind_response(
            release.release().bootstrap_json(),
            &wrong,
            &result,
            release.source_digest()
        )
        .is_err());
    }
}

#[tokio::test]
async fn configured_feature_does_not_enable_an_unavailable_age_provider() {
    use concert_application::commands::{FingerprintKeys, SigningKey};
    use concert_service::adapters::out::{
        age_provider::runtime::{AgeRuntime, ProviderMode},
        SystemClock,
    };
    let f = Fixture::new();
    let release = f.release(BootstrapFeatures {
        social_sharing: true,
        ..Default::default()
    });
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgresql://localhost/unused")
        .unwrap();
    let keys = Arc::new(
        FingerprintKeys::new(SigningKey::new("c03_fixture", &[73; 32]).unwrap(), vec![]).unwrap(),
    );
    let age = AgeRuntime::new(
        pool.clone(),
        Arc::new(SystemClock),
        keys,
        ProviderMode::Unavailable,
    );
    assert!(!age.verification_available());
    assert_eq!(
        document(&release)["capabilities"]["social_sharing"]["available"],
        true
    );
    assert_eq!(document(&release)["participant"]["age_status"], "UNKNOWN");
    pool.close().await;
}
