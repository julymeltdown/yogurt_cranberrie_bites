//! Real SQLx ledger upgrade. Published media/beacon migrations stay immutable.
mod meeting_photo_support;
use concert_application::media::Purpose;
use meeting_photo_support::*;
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, Row};
use std::path::{Path, PathBuf};
use uuid::Uuid;
#[tokio::test]
async fn sqlx_fresh_upgrade_existing_media_checksum_and_replay() -> anyhow::Result<()> {
    let main = owned_pool().await?;
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let all = sqlx::migrate::Migrator::new(source.clone()).await?;
    for database in ["mcr_photo_fresh", "mcr_photo_upgrade"] {
        let exists: bool =
            sqlx::query_scalar("select exists(select 1 from pg_database where datname=$1)")
                .bind(database)
                .fetch_one(&main)
                .await?;
        if !exists {
            sqlx::query(&format!("create database {database} template template0"))
                .execute(&main)
                .await?;
        }
        let options = main.connect_options().as_ref().clone().database(database);
        let p = PgPoolOptions::new()
            .max_connections(6)
            .connect_with(options)
            .await?;
        let root = PathBuf::from(std::env::var("MEETING_TEST_OWNER_ROOT")?);
        let dir: String = sqlx::query_scalar("show data_directory")
            .fetch_one(&p)
            .await?;
        assert_eq!(dir, root.join("data").to_string_lossy());
        if !exists && database == "mcr_photo_upgrade" {
            let before_dir = root.join(format!("photo-before-{}", Uuid::new_v4()));
            std::fs::create_dir(&before_dir)?;
            for e in std::fs::read_dir(&source)? {
                let path = e?.path();
                if path.extension().and_then(|x| x.to_str()) == Some("sql")
                    && path
                        .file_name()
                        .unwrap()
                        .to_string_lossy()
                        .split('_')
                        .next()
                        .and_then(|x| x.parse::<i64>().ok())
                        .is_some_and(|n| n < 29)
                {
                    std::fs::copy(&path, before_dir.join(path.file_name().unwrap()))?;
                }
            }
            sqlx::migrate::Migrator::new(before_dir.clone())
                .await?
                .run(&p)
                .await?;
            let f = setup(&p).await?;
            for u in [f.owner, f.peer] {
                sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(f.event).bind(u).execute(&p).await?;
            }
            let id = upload(&p, &f, f.owner, Purpose::Post).await?;
            let before: serde_json::Value =
                sqlx::query_scalar("select to_jsonb(m) from concert_media m where media_id=$1")
                    .bind(id)
                    .fetch_one(&p)
                    .await?;
            all.run(&p).await?;
            let after: serde_json::Value =
                sqlx::query_scalar("select to_jsonb(m) from concert_media m where media_id=$1")
                    .bind(id)
                    .fetch_one(&p)
                    .await?;
            assert_eq!(
                before, after,
                "forward purpose change does not rewrite existing media"
            );
            let image = upload(&p, &f, f.owner, Purpose::Beacon).await?;
            let b = Uuid::new_v4();
            let m = create(&f, Some(image));
            let s = service(&p);
            s.execute(frame(&f, b, Uuid::new_v4(), &m), m).await?;
            assert_eq!(
                s.read(read(&f, b, f.peer)).await?.meeting.media_id,
                Some(image)
            );
            std::fs::remove_dir_all(before_dir)?;
        } else {
            all.run(&p).await?;
        }
        all.run(&p).await?;
        let versions: Vec<i64> = sqlx::query_scalar(
            "select version from _sqlx_migrations where success order by version",
        )
        .fetch_all(&p)
        .await?;
        assert_eq!(versions, all.iter().map(|m| m.version).collect::<Vec<_>>());
        for version in [8, 12, 29] {
            let actual: Vec<u8> =
                sqlx::query_scalar("select checksum from _sqlx_migrations where version=$1")
                    .bind(version)
                    .fetch_one(&p)
                    .await?;
            assert_eq!(
                actual.as_slice(),
                all.iter()
                    .find(|m| m.version == version)
                    .unwrap()
                    .checksum
                    .as_ref()
            );
        }
        // Pin checksums of published prerequisites independently of the SQLx SHA384 ledger.
        for (file, expected) in [
            (
                "0008_meeting_beacons.sql",
                "b8259eb000c15059ddcba60db64518562d2cede7d2d68c42de3df8a85b44c655",
            ),
            (
                "0012_media.sql",
                "aae18779fda409d5a9de1844336c3e58d3705197346ce94b9b5e0a926e119224",
            ),
        ] {
            assert_eq!(
                format!("{:x}", Sha256::digest(std::fs::read(source.join(file))?)),
                expected
            );
        }
        let count: i64 = sqlx::query_scalar(
            "select count(*) from _sqlx_migrations where version=29 and success",
        )
        .fetch_one(&p)
        .await?;
        assert_eq!(count, 1);
        p.close().await;
    }
    main.close().await;
    Ok(())
}
