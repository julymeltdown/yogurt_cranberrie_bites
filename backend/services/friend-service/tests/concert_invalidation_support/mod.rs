pub async fn owned_pool() -> sqlx::PgPool {
    use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr};
    let root = PathBuf::from(
        std::env::var("MCR_FRIEND_INVALIDATION_ROOT").expect("private harness root required"),
    );
    assert_eq!(root.canonicalize().unwrap(), root);
    assert!(root.starts_with("/private/tmp") || root.starts_with("/tmp"));
    let m = std::fs::symlink_metadata(&root).unwrap();
    assert!(m.is_dir());
    assert_eq!(m.mode() & 0o077, 0);
    assert_eq!(
        format!("{}:{}", m.dev(), m.ino()),
        std::env::var("MCR_FRIEND_INVALIDATION_IDENTITY").unwrap()
    );
    let path = root.join("mcr-owner.json");
    let mm = std::fs::symlink_metadata(&path).unwrap();
    assert!(mm.is_file());
    assert_eq!(mm.uid(), m.uid());
    assert_eq!(mm.mode() & 0o077, 0);
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(marker["uid"].as_u64(), Some(u64::from(m.uid())));
    assert_eq!(
        marker["token"].as_str(),
        Some(
            std::env::var("MCR_FRIEND_INVALIDATION_TOKEN")
                .unwrap()
                .as_str()
        )
    );
    for name in ["data", "socket"] {
        let p = root.join(name);
        assert!(std::fs::symlink_metadata(&p).unwrap().is_dir());
        assert_eq!(p.canonicalize().unwrap(), p);
    }
    let options = sqlx::postgres::PgConnectOptions::from_str(
        &std::env::var("MCR_FRIEND_INVALIDATION_URL").unwrap(),
    )
    .unwrap();
    assert_eq!(options.get_username(), "mcr_owner");
    assert_eq!(options.get_database(), Some("mcr_friend_invalidation"));
    assert_eq!(options.get_socket(), Some(&root.join("socket")));
    assert_eq!(options.get_port(), 5432);
    assert!(options.get_options().is_none());
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await
        .unwrap();
    let row:(String,String,String,String,String,i32)=sqlx::query_as("select current_database(),current_user,current_setting('data_directory'),current_setting('unix_socket_directories'),current_setting('listen_addresses'),current_setting('server_version_num')::int").fetch_one(&pool).await.unwrap();
    assert_eq!(
        row,
        (
            "mcr_friend_invalidation".into(),
            "mcr_owner".into(),
            root.join("data").to_str().unwrap().into(),
            root.join("socket").to_str().unwrap().into(),
            "".into(),
            160014
        )
    );
    pool
}
