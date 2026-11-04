use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool, Row,
};
use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr};
pub async fn pool() -> PgPool {
    let root = PathBuf::from(std::env::var("MCR_SOCIAL_ROOT").expect("private root required"));
    assert_eq!(root.canonicalize().unwrap(), root);
    assert!(root.starts_with("/private/tmp"));
    let m = std::fs::symlink_metadata(&root).unwrap();
    assert!(m.is_dir());
    assert_eq!(m.mode() & 0o077, 0);
    assert_eq!(
        format!("{}:{}", m.dev(), m.ino()),
        std::env::var("MCR_SOCIAL_IDENTITY").unwrap()
    );
    let marker = root.join("mcr-owner.json");
    let mm = std::fs::symlink_metadata(&marker).unwrap();
    assert!(mm.is_file());
    assert_eq!(mm.uid(), m.uid());
    assert_eq!(mm.mode() & 0o077, 0);
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(marker).unwrap()).unwrap();
    assert_eq!(
        v["token"].as_str(),
        Some(std::env::var("MCR_SOCIAL_TOKEN").unwrap().as_str())
    );
    assert_eq!(v["uid"].as_u64(), Some(u64::from(m.uid())));
    let options =
        PgConnectOptions::from_str(&std::env::var("MCR_SOCIAL_FRIEND_URL").unwrap()).unwrap();
    assert_eq!(options.get_username(), "mcr_owner");
    assert_eq!(options.get_database(), Some("mcr_social_friend"));
    assert_eq!(options.get_socket(), Some(&root.join("socket")));
    assert_eq!(options.get_port(), 5432);
    assert!(options.get_options().is_none());
    let p = PgPoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .unwrap();
    let row=sqlx::query("select current_setting('listen_addresses') l,current_setting('data_directory') d,(select system_identifier::text from pg_control_system()) s").fetch_one(&p).await.unwrap();
    assert_eq!(row.get::<String, _>("l"), "");
    assert_eq!(
        row.get::<String, _>("d"),
        root.join("data").to_str().unwrap()
    );
    assert_eq!(
        row.get::<String, _>("s"),
        std::env::var("MCR_SOCIAL_SYSTEM").unwrap()
    );
    sqlx::migrate::Migrator::new(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("migrations")
            .as_path(),
    )
    .await
    .unwrap()
    .run(&p)
    .await
    .unwrap();
    p
}
