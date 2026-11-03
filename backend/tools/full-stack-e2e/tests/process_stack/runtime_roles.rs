//! Service-owned grants, exercised by the actual external-mode processes.
//! Controller connections stay separate for fixture setup and observations.
use anyhow::{ensure, Result};
use sqlx::{Connection, Executor, PgConnection};
use std::path::Path;
use uuid::Uuid;

fn quoted_identifier(value: &str) -> String {
    format!("\"{}\"", value.replace('"', "\"\""))
}

pub async fn grant(owner: &mut PgConnection, service: &str, role: &str) -> Result<()> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../services")
        .join(service)
        .join("runtime-grants.sql");
    let sql = std::fs::read_to_string(path)?;
    ensure!(sql.contains(":\"runtime_role\""), "missing role parameter");
    let sql = sql.replace(":\"runtime_role\"", &quoted_identifier(role));
    let database: String = sqlx::query_scalar("select current_database()")
        .fetch_one(&mut *owner)
        .await?;
    let database = quoted_identifier(&database);
    // Only our disposable database: PUBLIC must not give another service
    // connect, temporary-schema or public-schema creation privileges.
    owner.execute(format!("revoke all on database {database} from public; grant connect on database {database} to {}; revoke create on schema public from public", quoted_identifier(role)).as_str()).await?;
    let mut tx = owner.begin().await?;
    sqlx::raw_sql(&sql).execute(&mut *tx).await?;
    tx.commit().await?;
    Ok(())
}

async fn denied(connection: &mut PgConnection, sql: &str) -> Result<()> {
    let error = sqlx::query(sql)
        .execute(connection)
        .await
        .expect_err("runtime accepted a forbidden database operation");
    ensure!(
        error.as_database_error().and_then(|e| e.code()).as_deref() == Some("42501"),
        "runtime rejection was not PostgreSQL insufficient_privilege"
    );
    Ok(())
}

pub async fn install(owner_url: &str, service: &str, prefix: &str) -> Result<String> {
    let role = format!("process_runtime_{}", prefix.to_ascii_lowercase());
    let password = Uuid::new_v4().simple().to_string();
    let mut owner = PgConnection::connect(owner_url).await?;
    // Fixture seeding follows the owner command's public-schema contract;
    // keep the adversarial shadow option on the URLs handed to real services.
    owner.execute("set search_path=public").await?;
    owner.execute(format!("create role {} login password '{password}' nosuperuser nocreatedb nocreaterole noinherit noreplication nobypassrls", quoted_identifier(&role)).as_str()).await?;
    grant(&mut owner, service, &role).await?;
    let mut url = reqwest::Url::parse(owner_url)?;
    url.set_username(&role)
        .map_err(|_| anyhow::anyhow!("invalid fixture username"))?;
    url.set_password(Some(&password))
        .map_err(|_| anyhow::anyhow!("invalid fixture password"))?;
    let mut runtime = PgConnection::connect(url.as_str()).await?;
    runtime.execute("set search_path=public").await?;
    let isolated: bool = sqlx::query_scalar("select current_user=$1 and not (rolsuper or rolcreatedb or rolcreaterole or rolinherit or rolreplication or rolbypassrls) and not exists(select 1 from pg_auth_members where member=r.oid) and not exists(select 1 from pg_class where relowner=r.oid) and not exists(select 1 from pg_namespace where nspowner=r.oid) and not exists(select 1 from pg_proc where proowner=r.oid) and not exists(select 1 from pg_database where datdba=r.oid) from pg_roles r where rolname=current_user")
        .bind(&role).fetch_one(&mut runtime).await?;
    ensure!(isolated, "runtime has owner attributes or membership");
    for statement in [
        "update public._sqlx_migrations set success=success where false",
        "delete from public._sqlx_migrations where false",
        "create table public.runtime_forbidden(id integer)",
        "create schema runtime_forbidden",
        "create temporary table runtime_forbidden(id integer)",
        "alter table public._sqlx_migrations disable trigger all",
        "set role postgres",
    ] {
        denied(&mut runtime, statement).await?;
    }
    if prefix == "POST" {
        for table in [
            "feed_affinity_capture_epoch",
            "feed_affinity_pair_baselines",
            "feed_affinity_pair_provenance",
            "feed_affinity_source_pairs",
            "feed_affinity_source_contributions",
            "feed_affinity_legacy_effect_observations",
            "feed_affinity_pair_heads",
            "feed_affinity_resolution_receipts",
            "feed_recommendation_retry_contexts",
        ] {
            denied(
                &mut runtime,
                &format!("delete from public.{table} where false"),
            )
            .await?;
        }
        for table in [
            "feed_affinity_test_provisioning",
            "feed_affinity_write_permits",
        ] {
            denied(
                &mut runtime,
                &format!("select * from public.{table} limit 0"),
            )
            .await?;
        }
        let can_issue: bool = sqlx::query_scalar("select has_function_privilege(current_user,'public.feed_affinity_issue_test_provisioning(uuid,text,bigint,text,uuid,uuid,uuid,jsonb)','EXECUTE')")
            .fetch_one(&mut runtime).await?;
        ensure!(
            !can_issue,
            "runtime can issue controlled affinity authority"
        );
    }
    if prefix == "POST" || prefix == "FRIEND" {
        cleanup_only(&mut owner, &mut runtime, prefix).await?;
    }
    runtime.close().await?;
    owner.close().await?;
    eprintln!("PROCESS_STACK_RESTRICTED_ROLE={prefix} role={role} owner=false ddl=false ledger_write=false");
    Ok(url.into())
}

/// Restore changes only the database, retaining the service's credentials and
/// schema options. pg_restore's fixture policy omits ACLs, so reapply the same
/// owner-managed grants; never silently restart the service as the controller.
pub async fn restored_gateway_url(serving_url: &str, restored_owner_url: &str) -> Result<String> {
    let mut serving = reqwest::Url::parse(serving_url)?;
    let restored = reqwest::Url::parse(restored_owner_url)?;
    ensure!(
        serving.host_str() == restored.host_str() && serving.port() == restored.port(),
        "restore endpoint differs from owned fixture"
    );
    let role = serving.username().to_owned();
    ensure!(
        role == "process_runtime_gateway",
        "unexpected restored service role"
    );
    let mut owner = PgConnection::connect(restored_owner_url).await?;
    grant(&mut owner, "api-gateway", &role).await?;
    owner.close().await?;
    serving.set_path(restored.path());
    Ok(serving.into())
}

/// Every service must be rejected by the five other current service databases.
pub async fn cross_database_denials(urls: [&str; 6]) -> Result<()> {
    for (source, original) in urls.iter().enumerate() {
        let mut url = reqwest::Url::parse(original)?;
        for (target, other) in urls.iter().enumerate() {
            if source == target {
                continue;
            }
            url.set_path(reqwest::Url::parse(other)?.path());
            let error = match PgConnection::connect(url.as_str()).await {
                Ok(connection) => {
                    connection.close().await?;
                    anyhow::bail!("runtime connected to another service database");
                }
                Err(error) => error,
            };
            ensure!(
                error.as_database_error().and_then(|e| e.code()).as_deref() == Some("42501"),
                "cross-database rejection was not insufficient_privilege"
            );
        }
    }
    eprintln!("PROCESS_STACK_CROSS_DATABASE_DENIALS=30");
    Ok(())
}

/// Seed actual owner-managed rows, reject runtime reassignment, and retain the
/// row locks plus DELETE needed for retention. Owner updates remain supported.
async fn cleanup_only(
    owner: &mut PgConnection,
    runtime: &mut PgConnection,
    service: &str,
) -> Result<()> {
    let (prefix, model, constraint) = if service == "POST" {
        ("feed", "feed-linear-v1", "post_recommendation_owner_update")
    } else {
        (
            "people",
            "people-linear-v1",
            "friend_recommendation_owner_update",
        )
    };
    let subject = Uuid::new_v4();
    sqlx::query(&format!("insert into public.{prefix}_experiment_assignments values($1,'runtime-grant-check','A','DETERMINISTIC',now()-interval '2 days',now()-interval '1 day')"))
        .bind(subject).execute(&mut *owner).await?;
    sqlx::query(&format!("insert into public.{prefix}_embeddings values('USER',$1,$2,array_fill(0.1::real,array[16]),now()-interval '2 days')"))
        .bind(subject).bind(model).execute(&mut *owner).await?;
    sqlx::query(&format!(
        "update public.{prefix}_experiment_assignments set variant='B' where viewer_id=$1"
    ))
    .bind(subject)
    .execute(&mut *owner)
    .await?;
    sqlx::query(&format!("update public.{prefix}_embeddings set embedding=array_fill(0.2::real,array[16]) where subject_id=$1"))
        .bind(subject).execute(&mut *owner).await?;
    for (table, key) in [
        (format!("{prefix}_experiment_assignments"), "viewer_id"),
        (format!("{prefix}_embeddings"), "subject_id"),
    ] {
        let error = sqlx::query(&format!(
            "update public.{table} set {key}=$1 where {key}=$2"
        ))
        .bind(Uuid::new_v4())
        .bind(subject)
        .execute(&mut *runtime)
        .await
        .expect_err("runtime reassigned owner-managed data");
        ensure!(
            error.as_database_error().and_then(|e| e.code()).as_deref() == Some("42501")
                && error.as_database_error().and_then(|e| e.constraint()) == Some(constraint),
            "reassignment was not rejected by the owner boundary"
        );
        let mut tx = runtime.begin().await?;
        let row: Uuid = sqlx::query_scalar(&format!(
            "select {key} from public.{table} where {key}=$1 for update skip locked"
        ))
        .bind(subject)
        .fetch_one(&mut *tx)
        .await?;
        ensure!(
            row == subject,
            "runtime retention could not lock its seeded row"
        );
        ensure!(
            sqlx::query(&format!("delete from public.{table} where {key}=$1"))
                .bind(subject)
                .execute(&mut *tx)
                .await?
                .rows_affected()
                == 1,
            "runtime retention could not delete its locked row"
        );
        tx.commit().await?;
    }
    eprintln!("PROCESS_STACK_CLEANUP_ONLY={service} owner_update=true runtime_reassignment=false runtime_lock_delete=true");
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Direct PostgreSQL acceptance of the grant policies. This deliberately
    /// does not stand in for service startup or a full business journey.
    #[tokio::test]
    async fn six_service_policies_enforce_runtime_privilege_boundaries() -> Result<()> {
        let (pg, admin_url) = test_support::postgres::start_postgres_with_url().await?;
        let mut admin = test_support::postgres::with_startup_diagnostics(
            Some(&pg),
            "runtime-grant policy admin connection",
            async { Ok(PgConnection::connect(&admin_url).await?) },
        )
        .await?;
        let suffix = Uuid::new_v4().simple().to_string();
        let mut serving_urls = Vec::new();
        for (service, prefix) in [
            ("auth-service", "AUTH"),
            ("profile-service", "PROFILE"),
            ("friend-service", "FRIEND"),
            ("post-service", "POST"),
            ("notification-service", "NOTIFICATION"),
            ("api-gateway", "GATEWAY"),
        ] {
            let database = format!("grants_{}_{suffix}", prefix.to_ascii_lowercase());
            admin
                .execute(format!("create database {}", quoted_identifier(&database)).as_str())
                .await?;
            let mut url = reqwest::Url::parse(&admin_url)?;
            url.set_path(&database);
            let mut owner = PgConnection::connect(url.as_str()).await?;
            let migrations = Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../services")
                .join(service)
                .join("migrations");
            sqlx::migrate::Migrator::new(migrations.as_path())
                .await?
                .run(&mut owner)
                .await?;
            owner.execute("create schema shadow").await?;
            owner.close().await?;
            url.query_pairs_mut()
                .append_pair("options", "-csearch_path=shadow");
            let runtime_url = install(url.as_str(), service, prefix).await?;
            let mut runtime = PgConnection::connect(&runtime_url).await?;
            let path: String = sqlx::query_scalar("show search_path")
                .fetch_one(&mut runtime)
                .await?;
            ensure!(
                path == "shadow",
                "fixture changed the serving URL schema option"
            );
            runtime.close().await?;
            serving_urls.push(runtime_url);
        }
        cross_database_denials([
            &serving_urls[0],
            &serving_urls[1],
            &serving_urls[2],
            &serving_urls[3],
            &serving_urls[4],
            &serving_urls[5],
        ])
        .await?;
        admin.close().await?;
        pg.rm().await?;
        Ok(())
    }
}
