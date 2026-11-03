use concert_application::bootstrap::*;
use concert_domain::{AuthUserId, EventId};
use concert_service::bootstrap::{BootstrapFeatures, ProductReleaseBundle};
use sqlx::{postgres::PgConnectOptions, PgPool};
use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr, sync::Arc};
use uuid::Uuid;

pub async fn pool() -> PgPool {
    let root =
        PathBuf::from(std::env::var("MCR_BOOTSTRAP_ROOT").expect("private harness root required"));
    assert_eq!(root.canonicalize().unwrap(), root);
    assert!(root.starts_with("/private/tmp") || root.starts_with("/tmp"));
    assert!(root
        .file_name()
        .unwrap()
        .to_str()
        .unwrap()
        .starts_with("mcr-pg-"));
    let meta = std::fs::symlink_metadata(&root).unwrap();
    assert!(meta.is_dir());
    assert_eq!(meta.mode() & 0o077, 0);
    let marker_path = root.join("mcr-owner.json");
    let marker_meta = std::fs::symlink_metadata(&marker_path).unwrap();
    assert!(marker_meta.is_file());
    assert_eq!(meta.uid(), marker_meta.uid());
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(marker_path).unwrap()).unwrap();
    assert_eq!(marker["uid"].as_u64().unwrap(), u64::from(meta.uid()));
    assert_eq!(
        marker["token"].as_str().unwrap(),
        std::env::var("MCR_BOOTSTRAP_TOKEN").unwrap()
    );
    assert_eq!(
        meta.ino().to_string(),
        std::env::var("MCR_BOOTSTRAP_INODE").unwrap()
    );
    for name in ["socket", "data"] {
        let path = root.join(name);
        assert!(std::fs::symlink_metadata(&path).unwrap().is_dir());
        assert_eq!(path.canonicalize().unwrap(), path);
    }
    let url = std::env::var("MCR_BOOTSTRAP_DATABASE_URL").expect("private URL required");
    let options = PgConnectOptions::from_str(&url).expect("private database options");
    assert_eq!(options.get_username(), "mcr_owner");
    assert_eq!(options.get_database(), Some("mcr_concert_bootstrap"));
    assert_eq!(options.get_socket().unwrap(), &root.join("socket"));
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(20)
        .connect_with(options)
        .await
        .unwrap();
    let (db, role, data, sockets, tcp, version): (String,String,String,String,String,i32) = sqlx::query_as(
        "select current_database(),current_user,current_setting('data_directory'),current_setting('unix_socket_directories'),current_setting('listen_addresses'),current_setting('server_version_num')::int"
    ).fetch_one(&pool).await.unwrap();
    assert_eq!(db, "mcr_concert_bootstrap");
    assert_eq!(role, "mcr_owner");
    assert_eq!(PathBuf::from(data), root.join("data"));
    assert_eq!(PathBuf::from(sockets), root.join("socket"));
    assert!(tcp.is_empty());
    assert_eq!(version, 160014);
    pool
}

pub fn release() -> Arc<ValidatedProductRelease> {
    let root = PathBuf::from(std::env::var("MCR_BOOTSTRAP_ROOT").unwrap()).join("release");
    release_from_directory(root)
}
/// Metadata/native fixture only: uses the selected canonical config with fixture map/themes.
pub fn selected_release() -> Arc<ValidatedProductRelease> {
    let root = PathBuf::from(std::env::var("MCR_BOOTSTRAP_ROOT").unwrap()).join("release-selected");
    release_from_directory(root)
}
fn release_from_directory(root: PathBuf) -> Arc<ValidatedProductRelease> {
    ProductReleaseBundle::from_synthetic_files(
        &root.join("bootstrap.json"),
        &root.join("avatar.json"),
        &root.join("map.json"),
        [
            &root.join("danger.json"),
            &root.join("parade.json"),
            &root.join("revenge.json"),
        ],
        BootstrapFeatures::default(),
        true,
    )
    .expect("explicit synthetic release must validate")
}

pub fn admission(event: Uuid, user: Uuid) -> BootstrapAdmission {
    BootstrapAdmission::from_verified(
        AuthUserId::from_uuid(user),
        EventId::from_uuid(event),
        Uuid::new_v4(),
        Uuid::new_v4(),
        "Current public name".into(),
        Some("Current public bio".into()),
        chrono::Utc::now(),
    )
    .unwrap()
}

pub async fn seed(pool: &PgPool, event: Uuid) {
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Native bootstrap test','OPEN')")
        .bind(event).bind(format!("native-{event}")).execute(pool).await.unwrap();
}

pub async fn count(pool: &PgPool, table: &str, event: Uuid) -> i64 {
    assert!([
        "concert_participants",
        "concert_avatar_settings",
        "concert_event_admissions"
    ]
    .contains(&table));
    sqlx::query_scalar(&format!("select count(*) from {table} where event_id=$1"))
        .bind(event)
        .fetch_one(pool)
        .await
        .unwrap()
}
