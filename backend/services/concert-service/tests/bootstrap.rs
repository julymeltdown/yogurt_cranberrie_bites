#[path = "support/bootstrap/mod.rs"]
mod support;
use concert_application::bootstrap::*;
use concert_service::bootstrap::{BootstrapPostgresRepository, BootstrapService};
use std::sync::Arc;
use uuid::Uuid;

#[tokio::test]
#[ignore = "requires bootstrap_smoke.py owned native cluster"]
async fn native_bootstrap_transactions_and_guarded_response() {
    let pool = support::pool().await;
    let release = support::release();
    let repo = Arc::new(BootstrapPostgresRepository::new(pool.clone()));
    let event = Uuid::parse_str("00000000-0000-4000-8000-000000000201").unwrap();
    support::seed(&pool, event).await;
    let user = Uuid::new_v4();
    let mut tasks = vec![];
    for _ in 0..16 {
        let repo = repo.clone();
        let release = release.clone();
        tasks.push(tokio::spawn(async move {
            repo.bootstrap(
                &support::admission(event, user),
                release.release().defaults(),
            )
            .await
            .unwrap()
        }));
    }
    for task in tasks {
        let record = task.await.unwrap();
        assert_eq!(record.participant_revision, 1);
        assert_eq!(record.avatar_revision, 1);
    }
    for table in [
        "concert_participants",
        "concert_avatar_settings",
        "concert_event_admissions",
    ] {
        assert_eq!(support::count(&pool, table, event).await, 1);
    }
    // Preference/avatar mutations survive repeated reads and readonly phases.
    sqlx::query("update concert_participants set theme='DANGER',low_power=true,reduced_motion=true,revision=revision+1 where event_id=$1 and user_id=$2").bind(event).bind(user).execute(&pool).await.unwrap();
    sqlx::query("update concert_avatar_settings set accessory_id=null,revision=revision+1 where event_id=$1 and user_id=$2").bind(event).bind(user).execute(&pool).await.unwrap();
    let record = repo
        .bootstrap(
            &support::admission(event, user),
            release.release().defaults(),
        )
        .await
        .unwrap();
    assert_eq!(record.preferences.theme, concert_domain::Theme::Danger);
    assert!(record.preferences.low_power);
    assert!(matches!(
        &record.avatar.appearance,
        concert_domain::AvatarAppearance::Layered {
            accessory: None,
            ..
        }
    ));
    let event2 = Uuid::new_v4();
    support::seed(&pool, event2).await;
    repo.bootstrap(
        &support::admission(event2, user),
        release.release().defaults(),
    )
    .await
    .unwrap();
    assert_eq!(
        support::count(&pool, "concert_participants", event2).await,
        1
    );
    // Roll back all inserts when authority or response binding fails at commit.
    for error in [
        BootstrapError::Unauthenticated,
        BootstrapError::ReleaseUnavailable,
    ] {
        let other = Uuid::new_v4();
        assert_eq!(
            repo.bootstrap_checked(
                &support::admission(event, other),
                release.release().defaults(),
                &|_| Err(error)
            )
            .await
            .unwrap_err(),
            error
        );
    }
    assert_eq!(
        support::count(&pool, "concert_participants", event).await,
        1
    );
    let actor2 = Uuid::new_v4();
    repo.bootstrap(
        &support::admission(event, actor2),
        release.release().defaults(),
    )
    .await
    .unwrap();
    assert_eq!(
        support::count(&pool, "concert_participants", event).await,
        2
    );
    // A suspended actor with no participant must never be revived.
    let suspended = Uuid::new_v4();
    sqlx::query(
        "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
    )
    .bind(event)
    .bind(suspended)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("update concert_event_admissions set state='SUSPENDED',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2").bind(event).bind(suspended).execute(&pool).await.unwrap();
    assert_eq!(
        repo.bootstrap(
            &support::admission(event, suspended),
            release.release().defaults()
        )
        .await
        .unwrap_err(),
        BootstrapError::Denied
    );
    assert_eq!(
        support::count(&pool, "concert_participants", event).await,
        2
    );
    // A moderation transaction that already holds the admission lock wins.
    let mut moderation = pool.begin().await.unwrap();
    sqlx::query("update concert_event_admissions set state='SUSPENDED',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2").bind(event).bind(actor2).execute(&mut *moderation).await.unwrap();
    let repo2 = repo.clone();
    let rel2 = release.clone();
    let waiting = tokio::spawn(async move {
        repo2
            .bootstrap(
                &support::admission(event, actor2),
                rel2.release().defaults(),
            )
            .await
    });
    let mut blocked = false;
    for _ in 0..100 {
        blocked = sqlx::query_scalar::<_, bool>("select exists(select 1 from pg_stat_activity where datname=current_database() and wait_event_type='Lock' and query like '%concert_event_admissions%')").fetch_one(&pool).await.unwrap();
        if blocked {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert!(
        blocked,
        "bootstrap must actually wait on the moderation lock"
    );
    assert!(!waiting.is_finished());
    moderation.commit().await.unwrap();
    assert_eq!(waiting.await.unwrap().unwrap_err(), BootstrapError::Denied);
    // Real generated service, signed guard, database and protobuf response.
    let document = invoke(
        BootstrapService::new(pool.clone(), release.clone()),
        event,
        user,
        900,
    )
    .await;
    assert_eq!(document.0, 0);
    let value = document.1.unwrap();
    assert_eq!(value["participant"]["user_id"], user.to_string());
    assert_eq!(value["avatar"], value["participant"]["profile"]["avatar"]);
    assert_eq!(value["participant"]["preferences"]["theme"], "DANGER");
    assert_eq!(
        value["avatar"]["config"]["item_ids"]["accessory"],
        serde_json::Value::Null
    );
    assert!(value["capabilities"]
        .as_object()
        .unwrap()
        .values()
        .all(|v| v["available"] == false));
    let output = std::path::PathBuf::from(std::env::var("MCR_BOOTSTRAP_ROOT").unwrap())
        .join("response.json");
    std::fs::write(output, serde_json::to_vec(&value).unwrap()).unwrap();
    // A valid but differently configured event cannot partially create membership.
    let wrong_event = Uuid::new_v4();
    support::seed(&pool, wrong_event).await;
    assert_ne!(
        invoke(
            BootstrapService::new(pool.clone(), release.clone()),
            wrong_event,
            Uuid::new_v4(),
            900
        )
        .await
        .0,
        0
    );
    assert_eq!(
        support::count(&pool, "concert_event_admissions", wrong_event).await,
        0
    );
    assert_eq!(
        support::count(&pool, "concert_participants", wrong_event).await,
        0
    );
    assert_eq!(
        support::count(&pool, "concert_avatar_settings", wrong_event).await,
        0
    );
    assert_ne!(
        invoke(
            BootstrapService::new(pool.clone(), release.clone()),
            event,
            suspended,
            900
        )
        .await
        .0,
        0
    );
    // Expiry while awaiting the event lock must cause rollback, not merely hide success.
    let delayed = Uuid::new_v4();
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("select event_id from concert_events where event_id=$1 for update")
        .bind(event)
        .execute(&mut *lock)
        .await
        .unwrap();
    let task = tokio::spawn(invoke(
        BootstrapService::new(pool.clone(), release.clone()),
        event,
        delayed,
        80,
    ));
    tokio::time::sleep(std::time::Duration::from_millis(140)).await;
    lock.rollback().await.unwrap();
    assert_ne!(task.await.unwrap().0, 0);
    let exists: bool = sqlx::query_scalar(
        "select exists(select 1 from concert_event_admissions where event_id=$1 and user_id=$2)",
    )
    .bind(event)
    .bind(delayed)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!exists);
    for phase in ["READ_ONLY", "ARCHIVED"] {
        sqlx::query("update concert_events set phase=$2,revision=revision+1 where event_id=$1")
            .bind(event)
            .bind(phase)
            .execute(&pool)
            .await
            .unwrap();
        let record = repo
            .bootstrap(
                &support::admission(event, user),
                release.release().defaults(),
            )
            .await
            .unwrap();
        assert_eq!(record.participant_revision, 2);
        assert_eq!(record.avatar_revision, 2);
        assert!(repo
            .bootstrap(
                &support::admission(event, Uuid::new_v4()),
                release.release().defaults()
            )
            .await
            .is_err());
    }
    sqlx::query("update concert_events set phase='DRAFT',revision=revision+1 where event_id=$1")
        .bind(event)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        repo.bootstrap(
            &support::admission(event, user),
            release.release().defaults()
        )
        .await
        .unwrap_err(),
        BootstrapError::EventNotOpen
    );
    pool.close().await;
}

#[tokio::test]
#[ignore = "requires bootstrap_smoke.py restart phase"]
async fn native_bootstrap_restart_persistence() {
    let pool = support::pool().await;
    let (participants,avatars,suspended):(i64,i64,i64)=sqlx::query_as("select (select count(*) from concert_participants),(select count(*) from concert_avatar_settings),(select count(*) from concert_event_admissions where state='SUSPENDED')").fetch_one(&pool).await.unwrap();
    assert_eq!((participants, avatars, suspended), (3, 3, 2));
    let row:(String,bool,i64,Option<String>)=sqlx::query_as("select p.theme,p.low_power,a.revision,a.accessory_id from concert_participants p join concert_avatar_settings a using(event_id,user_id) where p.theme='DANGER'").fetch_one(&pool).await.unwrap();
    assert_eq!(row, ("DANGER".into(), true, 2, None));
    pool.close().await;
}

async fn invoke(
    service: BootstrapService,
    event: Uuid,
    user: Uuid,
    budget: u64,
) -> (i32, Option<serde_json::Value>) {
    use concert_service::rpc::{proto, POLICY};
    use http_body_util::{BodyExt, Full};
    use platform::{
        actor_delegation::{ActorDelegationSigner, ActorDelegationVerifier, HEADER},
        grpc_auth::{CallerRegistry, Workload},
        grpc_method_policy::guard,
    };
    use prost::Message;
    use tower::ServiceExt;
    let signer = ActorDelegationSigner::from_seed(Workload::Gateway, &[42; 32]).unwrap();
    let guard = guard(
        &POLICY,
        proto::concert_service_server::ConcertServiceServer::new(service),
        CallerRegistry::new()
            .register(Workload::Gateway, "test-gateway")
            .unwrap(),
        ActorDelegationVerifier::new(Workload::Concert, &[signer.public_key()]).unwrap(),
    )
    .unwrap();
    let now = chrono::Utc::now().timestamp_micros();
    let m = proto::BootstrapRequest {
        user_id: user.to_string(),
        event_id: event.to_string(),
        session_family_id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
        authority_expires_unix_micros: now + budget as i64 * 1000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
        display_name: "Current public name".into(),
        bio: Some("Current public bio".into()),
        http_method: "GET".into(),
        http_target: format!("/api/concert/events/{event}/bootstrap"),
    };
    let bytes = m.encode_to_vec();
    let mut body = vec![0];
    body.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    body.extend(bytes);
    let method = "/concert.v1.ConcertService/Bootstrap";
    let token = signer.issue(
        Workload::Concert,
        method,
        user,
        None,
        &body,
        chrono::Utc::now().timestamp(),
    );
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(method)
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header("x-service-caller", "gateway")
        .header("x-service-token", "test-gateway")
        .header(HEADER, token)
        .body(tonic::body::Body::new(Full::new(
            tonic::codegen::Bytes::from(body),
        )))
        .unwrap();
    let response = platform::call_deadline::scope(
        std::time::Instant::now() + std::time::Duration::from_millis(budget + 200),
        guard.oneshot(req),
    )
    .await
    .unwrap();
    let headers = response.headers().clone();
    let collected = response.into_body().collect().await.unwrap();
    let code: i32 = headers
        .get("grpc-status")
        .or_else(|| collected.trailers().and_then(|t| t.get("grpc-status")))
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    if code != 0 {
        return (code, None);
    }
    assert_eq!(headers.get("cache-control").unwrap(), "no-store");
    let bytes = collected.to_bytes();
    assert_eq!(bytes[0], 0);
    let len = u32::from_be_bytes(bytes[1..5].try_into().unwrap()) as usize;
    assert_eq!(bytes.len(), len + 5);
    let message = proto::BootstrapResponse::decode(&bytes[5..]).unwrap();
    (
        code,
        Some(serde_json::from_slice(&message.bootstrap_json).unwrap()),
    )
}

struct Bundle {
    root: std::path::PathBuf,
    document: serde_json::Value,
}
impl Bundle {
    fn new() -> Self {
        let root = std::env::temp_dir().join(format!("mcr-bootstrap-release-{}", Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        Self {
            root,
            document: serde_json::from_str(include_str!(
                "../../../../contracts/fixtures/core/bootstrap-open.json"
            ))
            .unwrap(),
        }
    }
    fn write(&self) {
        let mut files = vec![
            ("bootstrap", self.document.clone()),
            ("avatar", self.document["avatar_manifest"].clone()),
            ("map", self.document["map_manifest"].clone()),
        ];
        for theme in self.document["theme_manifests"].as_array().unwrap() {
            files.push((
                match theme["theme_id"].as_str().unwrap() {
                    "DANGER" => "danger",
                    "PARADE" => "parade",
                    _ => "revenge",
                },
                theme.clone(),
            ));
        }
        for (name, value) in files {
            std::fs::write(
                self.root.join(format!("{name}.json")),
                serde_json::to_vec(&value).unwrap(),
            )
            .unwrap();
        }
    }
    fn load(
        &self,
        product: bool,
        synthetic: bool,
    ) -> Result<Arc<ValidatedProductRelease>, concert_service::bootstrap::BootstrapServiceError>
    {
        use concert_service::bootstrap::{BootstrapFeatures, ProductReleaseBundle};
        let b = self.root.join("bootstrap.json");
        let a = self.root.join("avatar.json");
        let m = self.root.join("map.json");
        let d = self.root.join("danger.json");
        let p = self.root.join("parade.json");
        let r = self.root.join("revenge.json");
        if product {
            ProductReleaseBundle::from_product_files(
                &b,
                &a,
                &m,
                [&d, &p, &r],
                BootstrapFeatures::default(),
            )
        } else {
            ProductReleaseBundle::from_synthetic_files(
                &b,
                &a,
                &m,
                [&d, &p, &r],
                BootstrapFeatures::default(),
                synthetic,
            )
        }
    }
}
impl Drop for Bundle {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.root).unwrap();
    }
}

#[test]
fn explicit_synthetic_release_never_qualifies_as_reviewed_product_art() {
    let bundle = Bundle::new();
    bundle.write();
    assert!(bundle.load(false, true).is_ok());
    assert!(bundle.load(false, false).is_err());
    assert!(bundle.load(true, true).is_err());
    std::fs::remove_file(bundle.root.join("map.json")).unwrap();
    assert!(bundle.load(false, true).is_err());
}

#[test]
fn schema_hash_defaults_and_private_unknown_fields_fail_closed() {
    let mutations: [fn(&mut serde_json::Value); 9] = [
        |v| v["unexpected"] = true.into(),
        |v| v["participant"]["profile"]["private_canary"] = "private-canary".into(),
        |v| v["participant"]["profile"]["email"] = "private-canary".into(),
        |v| v["event"]["event_id"] = "invalid".into(),
        |v| v["avatar_manifest"]["content_hash"] = "sha256:invalid".into(),
        |v| v["avatar"]["config"]["item_ids"]["accessory"] = "unknown_item".into(),
        |v| v["avatar_manifest"]["fallback_preset_id"] = "unknown_preset".into(),
        |v| v["map_manifest"]["geometry"]["collision_grid"][0] = serde_json::json!([0]),
        |v| v["server_time"] = "bad-time".into(),
    ];
    for mutation in mutations {
        let mut bundle = Bundle::new();
        mutation(&mut bundle.document);
        bundle.write();
        assert!(bundle.load(false, true).is_err());
    }
    let bundle = Bundle::new();
    bundle.write();
    let path = bundle.root.join("bootstrap.json");
    let bytes = std::fs::read_to_string(&path).unwrap();
    std::fs::write(
        &path,
        bytes.replacen("{", "{\"contract_version\":\"0.1.0\",", 1),
    )
    .unwrap();
    assert!(bundle.load(false, true).is_err());
}

#[test]
fn request_binding_uses_current_profile_event_avatar_and_preferences() {
    let bundle = Bundle::new();
    bundle.write();
    let release = bundle.load(false, true).unwrap();
    let event = Uuid::parse_str(bundle.document["event"]["event_id"].as_str().unwrap()).unwrap();
    let actor = Uuid::new_v4();
    let admission = support::admission(event, actor);
    let record = BootstrapRecord {
        event_id: concert_domain::EventId::from_uuid(event),
        event_revision: 5,
        event_slug: "current-event".into(),
        event_title: "Current title".into(),
        event_timezone: "Asia/Seoul".into(),
        phase: concert_domain::EventPhase::ReadOnly,
        restriction_revision: 3,
        active: true,
        participant_revision: 4,
        preferences: concert_domain::Preferences {
            theme: concert_domain::Theme::Danger,
            low_power: true,
            reduced_motion: true,
            sound_enabled: false,
        },
        avatar_revision: 2,
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
    let v: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(v["event"]["title"], "Current title");
    assert_eq!(v["event"]["revision"], 5);
    assert_eq!(v["event"]["phase"], "READ_ONLY");
    assert_eq!(v["participant"]["user_id"], actor.to_string());
    assert_eq!(
        v["participant"]["profile"]["display_name"],
        "Current public name"
    );
    assert_eq!(v["participant"]["age_status"], "UNKNOWN");
    assert_eq!(v["avatar"], v["participant"]["profile"]["avatar"]);
    let other = support::admission(Uuid::new_v4(), actor);
    assert!(concert_service::bootstrap::release::bind_response(
        release.release().bootstrap_json(),
        &other,
        &result,
        release.source_digest()
    )
    .is_err());
}

#[test]
fn missing_world_resources_override_enabled_configuration() {
    use concert_service::bootstrap::{BootstrapFeatures, ProductReleaseBundle};
    let bundle = Bundle::new();
    bundle.write();
    let features = BootstrapFeatures {
        world: true,
        ..BootstrapFeatures::default()
    };
    let release = ProductReleaseBundle::from_synthetic_files(
        &bundle.root.join("bootstrap.json"),
        &bundle.root.join("avatar.json"),
        &bundle.root.join("map.json"),
        [
            &bundle.root.join("danger.json"),
            &bundle.root.join("parade.json"),
            &bundle.root.join("revenge.json"),
        ],
        features,
        true,
    )
    .unwrap();
    let document: serde_json::Value =
        serde_json::from_slice(release.release().bootstrap_json()).unwrap();
    assert_eq!(
        document["capabilities"]["world"],
        serde_json::json!({"available":false,"reason":"DEPENDENCY_UNAVAILABLE"})
    );
    assert_eq!(document["capabilities"]["mutations"]["available"], false);
    assert_ne!(
        release.source_digest(),
        bundle.load(false, true).unwrap().source_digest()
    );
}

#[test]
fn selected_release_binds_all_six_presets_without_layered_defaults() {
    use concert_domain::{
        avatar::{config_value, parse_config},
        AssetId, AvatarAppearance,
    };
    use serde_json::{json, Value};
    use sha2::{Digest, Sha256};
    let mut bundle = Bundle::new();
    let manifest: Value = serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/assets/selected-static/avatar-manifest.json"
    ))
    .unwrap();
    bundle.document["avatar_manifest"] = manifest.clone();
    bundle.document["avatar"]["config"] = manifest["presets"][0]["config"].clone();
    bundle.document["participant"]["profile"]["avatar"] = bundle.document["avatar"].clone();
    for theme in bundle.document["theme_manifests"].as_array_mut().unwrap() {
        theme["avatar_manifest_id"] = manifest["manifest_id"].clone();
        theme["avatar_manifest_version"] = manifest["manifest_version"].clone();
        theme["avatar_content_hash"] = manifest["content_hash"].clone();
        theme.as_object_mut().unwrap().remove("content_hash");
        let canonical =
            concert_domain::place::canonical_json_bytes(&serde_json::to_vec(theme).unwrap())
                .unwrap();
        theme["content_hash"] = format!("sha256:{:x}", Sha256::digest(canonical)).into();
    }
    bundle.write();
    let release = bundle.load(false, true).unwrap();
    assert!(bundle.load(true, true).is_err()); // fixture map/themes are still unapproved product art
    let event = bundle.document["event"]["event_id"]
        .as_str()
        .unwrap()
        .parse()
        .unwrap();
    let actor = Uuid::new_v4();
    let admission = support::admission(event, actor);
    for preset in manifest["presets"].as_array().unwrap() {
        let selection = parse_config(&serde_json::to_vec(&preset["config"]).unwrap()).unwrap();
        let record = BootstrapRecord {
            event_id: concert_domain::EventId::from_uuid(event),
            event_revision: 1,
            event_slug: "selected-event".into(),
            event_title: "Selected event".into(),
            event_timezone: "Asia/Seoul".into(),
            phase: concert_domain::EventPhase::Open,
            restriction_revision: 1,
            active: true,
            participant_revision: 1,
            preferences: concert_domain::Preferences {
                theme: concert_domain::Theme::Revenge,
                low_power: false,
                reduced_motion: false,
                sound_enabled: false,
            },
            avatar_revision: 7,
            avatar: selection,
            completed_at: chrono::Utc::now(),
        };
        let mut result = BootstrapResult {
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
        let wire: Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(
            wire["avatar"],
            json!({"revision":7,"config":preset["config"]})
        );
        assert_eq!(wire["participant"]["profile"]["avatar"], wire["avatar"]);
        assert_eq!(config_value(&result.record.avatar), preset["config"]);
        result.record.avatar.appearance = AvatarAppearance::WholeCharacter {
            preset_id: AssetId::new("photo01_front_07".into()).unwrap(),
        };
        assert!(concert_service::bootstrap::release::bind_response(
            release.release().bootstrap_json(),
            &admission,
            &result,
            release.source_digest()
        )
        .is_err());
    }
    bundle.document["avatar"]["config"]["item_ids"] = json!({});
    bundle.document["participant"]["profile"]["avatar"] = bundle.document["avatar"].clone();
    bundle.write();
    assert!(bundle.load(false, true).is_err());
}

fn source_spatial_candidate() -> concert_service::bootstrap::spatial::SourceBoundSpatialCandidate {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../..");
    concert_service::bootstrap::spatial::load_candidate(
        &root.join("backend/releases/reference-scene-v2/map-manifest.json"),
        &root.join("backend/releases/reference-scene-v2/source-binding.json"),
        &root.join("frontend/public"),
    )
    .unwrap()
}
const SPATIAL_EVENT: &str = "00000000-0000-4000-8000-000000000831";
#[tokio::test]
#[ignore = "requires test_spatial_release.py private PostgreSQL harness"]
async fn native_spatial_candidate_transactions() {
    use concert_application::place::{PlaceRepository, SelectionOutcome};
    use concert_service::adapters::out::repository::place::PgPlaceRepository;
    let pool = support::pool().await;
    let event: Uuid = SPATIAL_EVENT.parse().unwrap();
    support::seed(&pool, event).await;
    let candidate = source_spatial_candidate();
    let repo = PgPlaceRepository::new(pool.clone());
    let event = concert_domain::EventId::from_uuid(event);
    let first = repo
        .publish_release(event, candidate.catalog.clone())
        .await
        .unwrap();
    assert_eq!(first.map_revision, 3);
    assert_eq!(first.map_id, "mcr_reference_scene_r3");
    let read = repo.catalog(event).await.unwrap().unwrap();
    assert_eq!(read.release, candidate.catalog);
    assert_eq!(read.places.len(), 4);
    let again = repo
        .publish_release(event, candidate.catalog.clone())
        .await
        .unwrap();
    // The existing administrative API advances event catalog state on every
    // publication; immutable map identity and canonical bytes stay unchanged.
    assert_eq!(again.revision.get(), first.revision.get() + 1);
    assert_eq!(
        (again.map_id.as_str(), again.map_revision),
        (first.map_id.as_str(), first.map_revision)
    );
    assert_eq!(
        repo.catalog(event).await.unwrap().unwrap().release,
        candidate.catalog
    );
    for place in &read.places {
        assert_eq!(
            repo.select_physical(event, place.id, again.revision, again.map_revision)
                .await
                .unwrap(),
            SelectionOutcome::ConceptOnly
        );
        assert_eq!(
            repo.select_physical(event, place.id, again.revision, 4)
                .await
                .unwrap(),
            SelectionOutcome::MapMismatch
        );
    }
    let row:(Vec<u8>,String)=sqlx::query_as("select canonical_bytes,geometry_hash from concert_map_releases where map_id=$1 and map_revision=$2")
        .bind(&first.map_id).bind(first.map_revision).fetch_one(&pool).await.unwrap();
    assert_eq!(row.0, candidate.catalog.canonical_bytes);
    assert_eq!(row.1, candidate.catalog.manifest.geometry_hash);
    pool.close().await;
}
#[tokio::test]
#[ignore = "requires test_spatial_release.py postmaster restart"]
async fn native_spatial_candidate_restart() {
    use concert_application::place::PlaceRepository;
    let pool = support::pool().await;
    let repo =
        concert_service::adapters::out::repository::place::PgPlaceRepository::new(pool.clone());
    let read = repo
        .catalog(concert_domain::EventId::from_uuid(
            SPATIAL_EVENT.parse().unwrap(),
        ))
        .await
        .unwrap()
        .unwrap();
    let candidate = source_spatial_candidate();
    assert_eq!(read.release, candidate.catalog);
    assert_eq!(read.places.len(), 4);
    assert!(candidate.scenery_within_budget);
    pool.close().await;
}
