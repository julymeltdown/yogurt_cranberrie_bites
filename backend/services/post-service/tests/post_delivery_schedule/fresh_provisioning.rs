//! Controlled fixture creation evidence. This is not production activation or
//! clone/restore resistance: those require independently validated authority.
use super::Fixture;
use anyhow::{ensure, Context, Result};
use sqlx::{
    migrate::Migrator,
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool, Row,
};
use std::path::PathBuf;
use uuid::Uuid;

#[derive(Debug, PartialEq)]
struct DatabaseIdentityDto {
    system_identifier: String,
    database_oid: i64,
    database_name: String,
    server_version_num: i32,
}

pub(super) struct CreationObservationDto {
    identity: DatabaseIdentityDto,
    lineage: &'static str,
    controller_nonce: Option<Uuid>,
}

// Query through the CREATE DATABASE controller connection, independently of
// the later target pool, so a redirected target cannot supply its own identity.
async fn observe_creation(
    admin: &PgPool,
    name: &str,
    fresh_container: bool,
) -> Result<CreationObservationDto> {
    let row = sqlx::query(
        "select c.system_identifier::text as system_identifier,d.oid::bigint as database_oid, \
         d.datname::text as database_name,current_setting('server_version_num')::int as server_version_num \
         from pg_control_system() c join pg_database d on d.datname=$1",
    ).bind(name).fetch_one(admin).await?;
    let identity = DatabaseIdentityDto {
        system_identifier: row.get("system_identifier"),
        database_oid: row.get("database_oid"),
        database_name: row.get("database_name"),
        server_version_num: row.get("server_version_num"),
    };
    let (lineage, controller_nonce) = if fresh_container {
        ("TESTCONTROLLER_NEW_CONTAINER", None)
    } else {
        match (
            std::env::var("POST_DELIVERY_TEST_INITDB_SYSTEM_IDENTIFIER"),
            std::env::var("POST_DELIVERY_TEST_CONTROLLER_NONCE"),
        ) {
            (Ok(system), Ok(nonce)) => {
                ensure!(
                    system == identity.system_identifier,
                    "initdb controller cluster differs"
                );
                (
                    "TESTCONTROLLER_OBSERVED_INITDB",
                    Some(Uuid::parse_str(&nonce)?),
                )
            }
            (Err(std::env::VarError::NotPresent), Err(std::env::VarError::NotPresent)) => {
                ("UNQUALIFIED_EXTERNAL_CLUSTER", None)
            }
            _ => anyhow::bail!("incomplete native initdb controller identity"),
        }
    };
    Ok(CreationObservationDto {
        identity,
        lineage,
        controller_nonce,
    })
}

// No Clone/deserialization or constructor from a supplied database marker.
// The test controller retains this only for its newly created database lifetime.
pub(super) struct FreshFixtureProvisioningDto {
    nonce: Uuid,
    writer_generation: Uuid,
    identity: DatabaseIdentityDto,
    migrations: Vec<(i64, Vec<u8>)>,
    capture_epoch: Option<Uuid>,
    lineage: &'static str,
    controller_nonce: Option<Uuid>,
}

async fn identity(pool: &PgPool) -> Result<DatabaseIdentityDto> {
    let row = sqlx::query(
        "select c.system_identifier::text as system_identifier, d.oid::bigint as database_oid, \
         current_database() as database_name, current_setting('server_version_num')::int as server_version_num \
         from pg_control_system() c join pg_database d on d.datname=current_database()",
    )
    .fetch_one(pool)
    .await?;
    Ok(DatabaseIdentityDto {
        system_identifier: row.get("system_identifier"),
        database_oid: row.get("database_oid"),
        database_name: row.get("database_name"),
        server_version_num: row.get("server_version_num"),
    })
}

async fn capture_epoch(pool: &PgPool) -> Result<Option<Uuid>> {
    let installed: bool =
        sqlx::query_scalar("select to_regclass('public.feed_affinity_capture_epoch') is not null")
            .fetch_one(pool)
            .await?;
    if !installed {
        return Ok(None);
    }
    Ok(Some(
        sqlx::query_scalar("select epoch_id from feed_affinity_capture_epoch where singleton")
            .fetch_one(pool)
            .await?,
    ))
}

pub(super) async fn create(
    admin: &PgPool,
    options: PgConnectOptions,
    fresh_container: bool,
    nonce: Uuid,
    writer_generation: Uuid,
    through: i64,
) -> Result<(PgPool, FreshFixtureProvisioningDto)> {
    let name = format!("port_rust_delivery_fixture_{}", nonce.simple());
    // A failed CREATE (including a name collision) owns no existing database.
    sqlx::query(&format!("create database {name} template template0"))
        .execute(admin)
        .await?;
    let created = observe_creation(admin, &name, fresh_container).await;
    let pool = PgPoolOptions::new()
        .max_connections(8)
        .connect_with(options.database(&name))
        .await;
    let outcome = match pool {
        Ok(pool) => match prepare(&pool, created, nonce, writer_generation, through).await {
            Ok(proof) => return Ok((pool, proof)),
            Err(error) => {
                pool.close().await;
                error
            }
        },
        Err(error) => error.into(),
    };
    sqlx::query(&format!("drop database {name} with (force)"))
        .execute(admin)
        .await
        .with_context(|| format!("failed to clean owned fixture {name} after: {outcome}"))?;
    let absent: bool =
        sqlx::query_scalar("select not exists(select 1 from pg_database where datname=$1)")
            .bind(&name)
            .fetch_one(admin)
            .await?;
    ensure!(absent, "failed fixture database survived cleanup: {name}");
    println!(
        "FAILED_FIXTURE_CLEANUP {}",
        serde_json::json!({"database":name,"removed":true})
    );
    Err(outcome)
}

async fn prepare(
    pool: &PgPool,
    created: Result<CreationObservationDto>,
    nonce: Uuid,
    writer_generation: Uuid,
    through: i64,
) -> Result<FreshFixtureProvisioningDto> {
    let created = created?;
    let observed = identity(pool).await?;
    ensure!(
        observed == created.identity,
        "created database identity differs"
    );
    ensure!(
        observed.server_version_num / 10000 == 16,
        "PostgreSQL16 fixture required"
    );
    let empty: bool = sqlx::query_scalar(
        "select not exists(select 1 from pg_class c join pg_namespace n on n.oid=c.relnamespace \
         where n.nspname='public' and c.relkind in ('r','p','m','v','f'))",
    )
    .fetch_one(pool)
    .await?;
    ensure!(empty, "fresh fixture contains relations before migrations");
    let mut migrator =
        Migrator::new(PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")).await?;
    ensure!(
        migrator.iter().any(|m| m.version == through),
        "unknown fixture migration boundary"
    );
    migrator.migrations = std::borrow::Cow::Owned(
        migrator
            .iter()
            .filter(|m| m.version <= through)
            .cloned()
            .collect(),
    );
    let migrations = migrator
        .iter()
        .map(|m| (m.version, m.checksum.to_vec()))
        .collect();
    migrator.run(pool).await?;
    let proof = FreshFixtureProvisioningDto {
        nonce,
        writer_generation,
        identity: observed,
        migrations,
        capture_epoch: capture_epoch(pool).await?,
        lineage: created.lineage,
        controller_nonce: created.controller_nonce,
    };
    proof.verify_before_seed(pool).await?;
    // Runtime output names the actual instance and controller-owned lifetime.
    // It deliberately contains no activation or production-readiness verdict.
    println!(
        "FRESH_FIXTURE_PROVISIONING {}",
        serde_json::json!({
            "scope":"controlled CREATE DATABASE TEMPLATE template0; before seed",
            "database":proof.identity.database_name,"databaseOid":proof.identity.database_oid,
            "systemIdentifier":proof.identity.system_identifier,
            "serverVersionNum":proof.identity.server_version_num,
            "provisioningNonce":proof.nonce,"writerGeneration":proof.writer_generation,
            "captureEpoch":proof.capture_epoch,"migrationCount":proof.migrations.len(),
            "clusterLineage":proof.lineage,"controllerNonce":proof.controller_nonce,
            "migrationVersions":proof.migrations.iter().map(|m|m.0).collect::<Vec<_>>(),
            "exactSuccessfulChecksumsVerified":true,
            "productionCloneRestoreAuthority":false,
            "orderedActivationEligible":false,
        })
    );
    Ok(proof)
}

pub(super) async fn verify_migrations(pool: &PgPool, migrator: &Migrator) -> Result<()> {
    let expected: Vec<_> = migrator
        .iter()
        .map(|m| (m.version, m.checksum.to_vec()))
        .collect();
    verify_manifest(pool, &expected).await
}

async fn verify_manifest(pool: &PgPool, expected: &[(i64, Vec<u8>)]) -> Result<()> {
    let rows =
        sqlx::query("select version,checksum,success from _sqlx_migrations order by version")
            .fetch_all(pool)
            .await?;
    ensure!(
        rows.len() == expected.len(),
        "fixture migration set differs"
    );
    for (row, (version, checksum)) in rows.iter().zip(expected) {
        ensure!(
            row.get::<bool, _>("success"),
            "fixture migration is unsuccessful"
        );
        ensure!(
            row.get::<i64, _>("version") == *version
                && row.get::<Vec<u8>, _>("checksum") == *checksum,
            "fixture migration version/checksum differs"
        );
    }
    Ok(())
}

impl FreshFixtureProvisioningDto {
    pub(super) async fn issue_test_baseline_authority(&self, pool: &PgPool) -> Result<()> {
        self.verify_before_seed(pool).await?;
        ensure!(
            self.lineage != "UNQUALIFIED_EXTERNAL_CLUSTER",
            "trusted controller lineage required"
        );
        ensure!(
            self.migrations.last().is_some_and(|m| m.0 >= 25),
            "baseline migration required"
        );
        let manifest = serde_json::Value::Array(self.migrations.iter().map(|(version,checksum)|
            serde_json::json!({"version":version,"checksum":checksum.iter().map(|b|format!("{b:02x}")).collect::<String>()})
        ).collect());
        sqlx::query("select feed_affinity_issue_test_provisioning($1,$2,$3,$4,$5,$6,$7,$8)")
            .bind(self.nonce)
            .bind(&self.identity.system_identifier)
            .bind(self.identity.database_oid)
            .bind(&self.identity.database_name)
            .bind(self.capture_epoch)
            .bind(Uuid::new_v4())
            .bind(self.writer_generation)
            .bind(manifest)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn verify_before_seed(&self, pool: &PgPool) -> Result<()> {
        ensure!(
            identity(pool).await? == self.identity,
            "fixture database identity changed"
        );
        verify_manifest(pool, &self.migrations).await?;
        ensure!(
            capture_epoch(pool).await? == self.capture_epoch,
            "fixture capture epoch changed"
        );
        // Migration-owned model/bootstrap rows are allowed. Every application
        // fact/effect table is empty at initial qualification. This scan is a
        // fixture assertion, never an application absence-based adoption rule.
        let tables: Vec<String> = sqlx::query_scalar(
            "select tablename::text from pg_tables where schemaname='public' \
             and tablename not in ('_sqlx_migrations','feed_affinity_capture_epoch', \
             'recommendation_model_registry','recommendation_rollouts', \
             'feed_recommendation_models','feed_recommendation_rollouts') order by tablename",
        )
        .fetch_all(pool)
        .await?;
        for table in tables {
            let quoted = table.replace('"', "\"\"");
            let has_rows: bool = sqlx::query_scalar(&format!(
                "select exists(select 1 from public.\"{quoted}\" limit 1)"
            ))
            .fetch_one(pool)
            .await?;
            ensure!(!has_rows, "fixture seeded before qualification: {table}");
        }
        Ok(())
    }
}

#[tokio::test]
async fn fresh_database_receipt_binds_identity_epoch_and_exact_migration_checksums() -> Result<()> {
    let f = Fixture::new(24).await?;
    assert_eq!(f.provisioning.migrations.len(), 24);
    assert!(f.provisioning.capture_epoch.is_some());
    assert_ne!(f.provisioning.nonce, f.provisioning.writer_generation);
    f.provisioning.verify_before_seed(&f.pool).await?;
    let other = Fixture::new(24).await?;
    assert!(f
        .provisioning
        .verify_before_seed(&other.pool)
        .await
        .is_err());
    assert_ne!(f.provisioning.nonce, other.provisioning.nonce);
    other.shutdown().await?;
    f.shutdown().await
}

#[tokio::test]
async fn migration_ledger_corruption_and_preseed_cannot_qualify_fresh() -> Result<()> {
    let f = Fixture::new(24).await?;
    for alteration in [
        "update _sqlx_migrations set checksum=decode('00','hex') where version=24",
        "update _sqlx_migrations set success=false where version=24",
        "delete from _sqlx_migrations where version=24",
        "insert into _sqlx_migrations(version,description,success,checksum,execution_time) values(25,'forged',true,decode('00','hex'),0)",
    ] {
        let mut tx = f.pool.begin().await?;
        sqlx::query(alteration).execute(&mut *tx).await?;
        // Commit the test corruption so verification observes the changed ledger.
        tx.commit().await?;
        assert!(f.provisioning.verify_before_seed(&f.pool).await.is_err());
        sqlx::query("delete from _sqlx_migrations where version in (24,25)")
            .execute(&f.pool).await?;
        let checksum = &f.provisioning.migrations.last().unwrap().1;
        sqlx::query("insert into _sqlx_migrations(version,description,success,checksum,execution_time) values(24,'restored test ledger',true,$1,0)")
            .bind(checksum).execute(&f.pool).await?;
        f.provisioning.verify_before_seed(&f.pool).await?;
    }
    sqlx::query("insert into posts(id,author_id,content,visibility,created_at) values($1,$2,'preseed','PUBLIC',now())")
        .bind(Uuid::new_v4()).bind(Uuid::new_v4()).execute(&f.pool).await?;
    assert!(f.provisioning.verify_before_seed(&f.pool).await.is_err());
    f.shutdown().await
}

#[tokio::test]
async fn historical_upgrade_does_not_refresh_the_creation_receipt() -> Result<()> {
    let f = Fixture::new(22).await?;
    assert!(f.provisioning.capture_epoch.is_none());
    let nonce = f.provisioning.nonce;
    f.upgrade().await?;
    assert_eq!(f.provisioning.nonce, nonce);
    assert!(f.provisioning.verify_before_seed(&f.pool).await.is_err());
    f.shutdown().await
}

#[tokio::test]
async fn failed_construction_removes_owned_database_and_collision_preserves_existing() -> Result<()>
{
    let f = Fixture::new(24).await?;
    let admin = &f.local.as_ref().unwrap().0;
    let options = admin.connect_options().as_ref().clone();
    let nonce = Uuid::new_v4();
    let failed_name = format!("port_rust_delivery_fixture_{}", nonce.simple());
    assert!(create(
        admin,
        options.clone(),
        false,
        nonce,
        Uuid::new_v4(),
        i64::MAX
    )
    .await
    .is_err());
    assert!(
        !sqlx::query_scalar::<_, bool>("select exists(select 1 from pg_database where datname=$1)")
            .bind(failed_name)
            .fetch_one(admin)
            .await?
    );
    // Deliberately reuse the existing fixture's name on its own controller.
    // CREATE must fail without adopting, mutating or deleting that database.
    assert!(create(
        admin,
        options,
        false,
        f.provisioning.nonce,
        Uuid::new_v4(),
        24
    )
    .await
    .is_err());
    f.provisioning.verify_before_seed(&f.pool).await?;
    f.shutdown().await
}
