//! Real SQLx ledger qualification of fresh install and forward upgrade from published 0008.
#[path = "support/meeting/mod.rs"]
mod support;
use sha2::{Digest, Sha256};
use sqlx::{postgres::PgPoolOptions, Row};
use std::path::{Path, PathBuf};
use support::*;
use uuid::Uuid;

#[tokio::test]
async fn published_0008_checksum_fresh_install_and_forward_0016_upgrade() -> anyhow::Result<()> {
    let main = owned_pool().await?;
    let source = Path::new(env!("CARGO_MANIFEST_DIR")).join("migrations");
    let published = std::fs::read(source.join("0008_meeting_beacons.sql"))?;
    assert_eq!(
        format!("{:x}", Sha256::digest(&published)),
        "b8259eb000c15059ddcba60db64518562d2cede7d2d68c42de3df8a85b44c655",
        "published migration must remain byte-for-byte stable"
    );
    let full = sqlx::migrate::Migrator::new(source.clone()).await?;
    for database in ["mcr_meeting_fresh", "mcr_meeting_upgrade"] {
        let exists: bool =
            sqlx::query_scalar("select exists(select 1 from pg_database where datname=$1)")
                .bind(database)
                .fetch_one(&main)
                .await?;
        if !exists {
            // Static closed namespace; same verified marker-owned server, no external URL.
            sqlx::query(&format!("create database {database} template template0"))
                .execute(&main)
                .await?;
        }
        let options = main.connect_options().as_ref().clone().database(database);
        let pool = PgPoolOptions::new()
            .max_connections(2)
            .connect_with(options)
            .await?;
        let root = PathBuf::from(std::env::var("MEETING_TEST_OWNER_ROOT")?);
        let actual: String = sqlx::query_scalar("show data_directory")
            .fetch_one(&pool)
            .await?;
        assert_eq!(actual, root.join("data").to_string_lossy());
        if !exists && database == "mcr_meeting_upgrade" {
            let baseline_dir = root.join(format!("meeting-baseline-{}", Uuid::new_v4().simple()));
            std::fs::create_dir(&baseline_dir)?;
            for entry in std::fs::read_dir(&source)? {
                let path = entry?.path();
                let name = path.file_name().unwrap().to_string_lossy();
                if path.extension().and_then(|v| v.to_str()) == Some("sql")
                    && name
                        .split('_')
                        .next()
                        .and_then(|n| n.parse::<i64>().ok())
                        .is_some_and(|n| n <= 8)
                {
                    std::fs::copy(&path, baseline_dir.join(path.file_name().unwrap()))?;
                }
            }
            let baseline = sqlx::migrate::Migrator::new(baseline_dir.clone()).await?;
            baseline.run(&pool).await?;
            let max: i64 =
                sqlx::query_scalar("select max(version) from _sqlx_migrations where success")
                    .fetch_one(&pool)
                    .await?;
            assert_eq!(max, 8);
            let event = Uuid::new_v4();
            let party = Uuid::new_v4();
            let user = Uuid::new_v4();
            sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,'legacy-meeting-upgrade','Synthetic legacy report','OPEN')").bind(event).execute(&pool).await?;
            sqlx::query("insert into concert_meeting_settings(event_id,event_cutoff) values($1,clock_timestamp()+interval '1 day')").bind(event).execute(&pool).await?;
            sqlx::query("insert into concert_meeting_rally_states(event_id,party_id,user_id,state,meeting_revision,membership_revision) values($1,$2,$3,'HERE',1,1)").bind(event).bind(party).bind(user).execute(&pool).await?;
            let before: serde_json::Value =
                sqlx::query_scalar("select to_jsonb(r) from concert_meeting_rally_states r")
                    .fetch_one(&pool)
                    .await?;
            full.run(&pool).await?;
            let row=sqlx::query("select to_jsonb(r)-'meeting_deadline' as original,meeting_deadline from concert_meeting_rally_states r").fetch_one(&pool).await?;
            assert_eq!(
                row.get::<serde_json::Value, _>("original"),
                before,
                "forward ALTER preserves every prior field and revision"
            );
            assert_eq!(
                row.get::<chrono::DateTime<chrono::Utc>, _>("meeting_deadline")
                    .timestamp(),
                0,
                "legacy arrival has no new physical authority"
            );
            std::fs::remove_dir_all(baseline_dir)?;
        } else {
            full.run(&pool).await?;
        }
        // Real ledger replay/checksum matching, rather than direct SQL reapplication.
        full.run(&pool).await?;
        let installed: Vec<i64> = sqlx::query_scalar(
            "select version from _sqlx_migrations where success order by version",
        )
        .fetch_all(&pool)
        .await?;
        let expected = full.iter().map(|m| m.version).collect::<Vec<_>>();
        assert_eq!(installed, expected);
        assert!(installed.contains(&8) && installed.contains(&16));
        let checksum: Vec<u8> =
            sqlx::query_scalar("select checksum from _sqlx_migrations where version=8")
                .fetch_one(&pool)
                .await?;
        assert_eq!(
            checksum.as_slice(),
            full.iter()
                .find(|m| m.version == 8)
                .unwrap()
                .checksum
                .as_ref()
        );
        pool.close().await;
    }
    main.close().await;
    Ok(())
}
