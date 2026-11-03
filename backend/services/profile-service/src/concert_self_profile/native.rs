//! Owned-store SQL qualification only; not a substitute for real guard/RPC tests.
use super::{fingerprint::effect_digest, identity::*, model::*, operation::Operation, store::*};
use chrono::Utc;
use platform::actor_delegation::authority_deadline::AuthorityRpcDeadline;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool, Row,
};
use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr, time::Duration};
use uuid::Uuid;

async fn pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(std::env::var("MCR_SELF_PROFILE_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && root.canonicalize()? == root
            && info.is_dir()
            && info.mode() & 0o077 == 0,
        "private fixture root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino()) == std::env::var("MCR_SELF_PROFILE_IDENTITY")?,
        "fixture inode mismatch"
    );
    let marker = root.join("mcr-owner.json");
    let mi = std::fs::symlink_metadata(&marker)?;
    anyhow::ensure!(
        mi.is_file()
            && !mi.file_type().is_symlink()
            && mi.mode() & 0o077 == 0
            && mi.uid() == info.uid(),
        "private marker required"
    );
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(marker)?)?;
    anyhow::ensure!(
        value["uid"].as_u64() == Some(info.uid() as u64)
            && value["token"].as_str() == Some(std::env::var("MCR_SELF_PROFILE_TOKEN")?.as_str()),
        "fixture marker mismatch"
    );
    for name in ["data", "socket"] {
        let p = root.join(name);
        let m = std::fs::symlink_metadata(&p)?;
        anyhow::ensure!(
            m.is_dir() && !m.file_type().is_symlink() && p.canonicalize()? == p,
            "foreign fixture child refused"
        );
    }
    let options = PgConnectOptions::from_str(&std::env::var("MCR_SELF_PROFILE_URL")?)?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_profile_self")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_port() == 5432
            && options.get_options().is_none(),
        "foreign database refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await?;
    let actual:(String,String,String,i32,String)=sqlx::query_as("select current_database(),current_user,current_setting('listen_addresses'),current_setting('server_version_num')::int,current_setting('data_directory')").fetch_one(&pool).await?;
    anyhow::ensure!(
        actual
            == (
                "mcr_profile_self".into(),
                "mcr_owner".into(),
                "".into(),
                160014,
                root.join("data").to_string_lossy().into()
            ),
        "foreign server refused"
    );
    Ok(pool)
}
fn deadline() -> AuthorityRpcDeadline {
    AuthorityRpcDeadline::new(
        Utc::now().timestamp_micros() + 4_000_000,
        Duration::from_secs(4),
    )
    .unwrap()
}
async fn fixture(pool: &PgPool) -> anyhow::Result<(Uuid, Version)> {
    let user = Uuid::new_v4();
    sqlx::query("insert into profiles(user_id,username,nickname,avatar_url,bio) values($1,$2,'Current','retained-avatar','Existing bio')").bind(user).bind(format!("self-{}",user.simple())).execute(pool).await?;
    let row = sqlx::query(
        "select concert_self_epoch,concert_self_revision from profiles where user_id=$1",
    )
    .bind(user)
    .fetch_one(pool)
    .await?;
    Ok((
        user,
        Version::new(
            row.get("concert_self_epoch"),
            row.get("concert_self_revision"),
        )?,
    ))
}
#[tokio::test]
#[ignore = "requires marker/inode-owned isolated Profile PostgreSQL"]
async fn native_versions_cover_retained_updates_and_tombstone_fences() -> anyhow::Result<()> {
    let pool = pool().await?;
    let (user, version) = fixture(&pool).await?;
    sqlx::query("update profiles set bio='Legacy changed',concert_self_epoch=$2,concert_self_revision=1 where user_id=$1").bind(user).bind(Uuid::new_v4()).execute(&pool).await?;
    let row=sqlx::query("select concert_self_epoch,concert_self_revision,nickname,avatar_url from profiles where user_id=$1").bind(user).fetch_one(&pool).await?;
    assert_eq!(row.get::<Uuid, _>("concert_self_epoch"), version.epoch());
    assert_eq!(row.get::<i64, _>("concert_self_revision"), 2);
    assert_eq!(row.get::<String, _>("nickname"), "Current");
    assert_eq!(row.get::<String, _>("avatar_url"), "retained-avatar");
    sqlx::query("insert into profile_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) values($1,$2,$1,clock_timestamp()+interval '1 day')").bind(user).bind(Uuid::new_v4()).execute(&pool).await?;
    let result = sqlx::query("update profiles set nickname='Late write' where user_id=$1")
        .bind(user)
        .execute(&pool)
        .await;
    assert!(result.is_err());
    pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires marker/inode-owned isolated Profile PostgreSQL"]
async fn native_prepare_preserves_visibility_and_blocks_legacy_writes() -> anyhow::Result<()> {
    let pool = pool().await?;
    for should_commit in [true, false] {
        let (actor, version) = fixture(&pool).await?;
        let id = Uuid::new_v4();
        let event = Uuid::new_v4();
        let raw = if should_commit {
            br#"{"expected_revision":1,"display_name":"Prepared commit","bio":null}"#.as_slice()
        } else {
            br#"{"expected_revision":1,"display_name":"Prepared abort","bio":null}"#.as_slice()
        };
        let command = super::patch::ParticipantPatch::parse(raw)?;
        let command_hash: [u8; 32] = {
            use sha2::{Digest, Sha256};
            Sha256::digest(raw).into()
        };
        let digest = effect_digest([7; 32], id, version, raw)?;
        let mut effect = EffectConnection::open(&pool, Uuid::new_v4(), digest, deadline()).await?;
        let operation = Operation {
            id,
            actor,
            event,
            family: Uuid::new_v4(),
            binding_digest: [7; 32],
            binding_proto: vec![1],
            command_digest: command_hash,
            expected: version,
            identity: effect.identity().clone(),
        };
        // Synthetic local SQL binding only. This test never claims canonical
        // owner admission; actual Check/Track RPC qualification is separate.
        operation.register(&pool, deadline()).await?;
        let content = update_in(&mut effect, actor, version, command.profile().unwrap()).await?;
        receipt_in(&mut effect, id, actor, event, command_hash, &content).await?;
        effect.prepare(id).await?;
        let old: String = sqlx::query_scalar("select nickname from profiles where user_id=$1")
            .bind(actor)
            .fetch_one(&pool)
            .await?;
        assert_eq!(old, "Current");
        assert_eq!(
            operation.status(&pool, deadline()).await?,
            NativeStatus::Prepared
        );
        let mut blocked = pool.begin().await?;
        sqlx::query("set local statement_timeout='60ms'")
            .execute(&mut *blocked)
            .await?;
        assert!(
            sqlx::query("update profiles set nickname='Racing legacy' where user_id=$1")
                .bind(actor)
                .execute(&mut *blocked)
                .await
                .is_err()
        );
        blocked.rollback().await?;
    }
    pool.close().await;
    Ok(())
}
#[tokio::test]
#[ignore = "requires same owned Profile PostgreSQL after controlled restart"]
async fn native_after_restart_resolves_actual_commit_and_abort_receipts() -> anyhow::Result<()> {
    let pool = pool().await?;
    let ids: Vec<Uuid> = sqlx::query_scalar(
        "select operation_id from concert_self_profile_operations order by created_at,operation_id",
    )
    .fetch_all(&pool)
    .await?;
    assert_eq!(ids.len(), 2);
    for (index, id) in ids.into_iter().enumerate() {
        let op = Operation::load(&pool, id, deadline())
            .await?
            .expect("persisted identity");
        assert_eq!(op.status(&pool, deadline()).await?, NativeStatus::Prepared);
        let command = if index == 0 { "commit" } else { "rollback" };
        // Fixture-only explicit resolution. Production Reconcile must obtain an
        // actual bound coordinator decision and never trusts this test choice.
        sqlx::query(&format!("{command} prepared '{}'", prepared_gid(id)?))
            .execute(&pool)
            .await?;
        let expected = if index == 0 {
            NativeStatus::Committed
        } else {
            NativeStatus::Aborted
        };
        assert_eq!(op.status(&pool, deadline()).await?, expected);
        let (nickname, bio, avatar): (String, String, String) =
            sqlx::query_as("select nickname,bio,avatar_url from profiles where user_id=$1")
                .bind(op.actor)
                .fetch_one(&pool)
                .await?;
        assert_eq!(avatar, "retained-avatar");
        if index == 0 {
            assert_eq!(nickname, "Prepared commit");
            assert_eq!(bio, "");
        } else {
            assert_eq!(nickname, "Current");
            assert_eq!(bio, "Existing bio");
        }
    }
    assert_eq!(
        sqlx::query_scalar::<_, i64>("select count(*) from concert_self_profile_receipts")
            .fetch_one(&pool)
            .await?,
        1
    );
    pool.close().await;
    Ok(())
}
