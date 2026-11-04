//! Explicit startup opt-in; recovery is supervised with the service lifetime.
use super::{proto, OperationBudget, PgOwnerAuthority, RpcSettlementResolver};
use platform::{
    actor_delegation::relationship_read::RelationshipReadVerifier, grpc_tls::TlsMaterial,
    readiness::Probe,
};
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub async fn configured_store(
    pool: sqlx::PgPool,
    tls: Option<&TlsMaterial>,
    credential: &str,
    verifier: Option<&RelationshipReadVerifier>,
) -> anyhow::Result<Option<PgOwnerAuthority>> {
    match std::env::var("CONCERT_OWNER_AUTHORITY_ENABLED").as_deref() {
        Err(std::env::VarError::NotPresent) | Ok("false") => return Ok(None),
        Ok("true") => {}
        _ => anyhow::bail!("CONCERT_OWNER_AUTHORITY_ENABLED must be true or false"),
    }
    anyhow::ensure!(
        verifier.is_some(),
        "Owner authority requires dedicated signed relationship authority"
    );
    let tls = tls.ok_or_else(|| anyhow::anyhow!("Owner authority requires internal mTLS"))?;
    let mut endpoints = Vec::new();
    for (producer, key) in [
        (
            proto::Producer::Concert,
            "AUTHORITY_CONCERT_SETTLEMENT_ENDPOINT",
        ),
        (proto::Producer::Post, "AUTHORITY_POST_SETTLEMENT_ENDPOINT"),
        (
            proto::Producer::Profile,
            "AUTHORITY_PROFILE_SETTLEMENT_ENDPOINT",
        ),
    ] {
        match std::env::var(key) {
            Ok(url) => endpoints.push((producer, url)),
            Err(std::env::VarError::NotPresent) if producer != proto::Producer::Concert => {}
            _ => anyhow::bail!("Missing or invalid owner settlement endpoint: {key}"),
        }
    }
    let store = PgOwnerAuthority::new(
        pool,
        Arc::new(RpcSettlementResolver::new(tls, credential, &endpoints)?),
    )?;
    store
        .verify_ready(&OperationBudget::new(
            Instant::now() + Duration::from_secs(2),
        )?)
        .await?;
    // Unknown producer outcomes stay quarantined. Recovery never guesses abort.
    if let Err(status) = store
        .recover(&OperationBudget::new(
            Instant::now() + Duration::from_secs(2),
        )?)
        .await
    {
        tracing::warn!(code=?status.code(),"Owner startup recovery retained quarantine");
    }
    Ok(Some(store))
}
pub fn readiness_probe(store: Option<PgOwnerAuthority>) -> Probe {
    Probe::new(
        "concert-owner-authority",
        true,
        Duration::from_millis(250),
        move || {
            let store = store.clone();
            async move {
                match store {
                    None => true,
                    Some(store) => {
                        match OperationBudget::new(Instant::now() + Duration::from_millis(200)) {
                            Ok(clock) => store.verify_ready(&clock).await.is_ok(),
                            Err(_) => false,
                        }
                    }
                }
            }
        },
    )
}
pub async fn run_recovery(store: Option<PgOwnerAuthority>) -> anyhow::Result<()> {
    let Some(store) = store else {
        return platform::shutdown::idle().await;
    };
    platform::shutdown::periodic(
        "concert-owner-authority-recovery",
        Duration::from_millis(250),
        move || {
            let store = store.clone();
            async move {
                store
                    .recover(&OperationBudget::new(
                        Instant::now() + Duration::from_secs(2),
                    )?)
                    .await
                    .map_err(anyhow::Error::from)
            }
        },
    )
    .await
}

/// This new service accepts its own bounded caller set. Retained root-service
/// registries intentionally remain unchanged; Auth's excludes Post/Profile.
/// Authority opt-in always requires real credentials, including in TLS dev runs.
pub fn owner_caller_registry() -> anyhow::Result<platform::grpc_auth::CallerRegistry> {
    use platform::grpc_auth::Workload;
    platform::config::build_caller_registry(
        true,
        &[Workload::Concert, Workload::Post, Workload::Profile],
        &[Workload::Concert],
    )
}
