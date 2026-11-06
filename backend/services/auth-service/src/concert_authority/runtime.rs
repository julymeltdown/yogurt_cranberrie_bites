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
        (
            proto::Producer::Notification,
            "AUTHORITY_NOTIFICATION_SETTLEMENT_ENDPOINT",
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
    // Verification is supervised after the listener binds so settlement dependents
    // can reach this service; readiness stays false until it succeeds.
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

const READY_WINDOW: Duration = Duration::from_secs(10);
const READY_INTERVAL: Duration = Duration::from_millis(250);
const READY_ATTEMPT: Duration = Duration::from_secs(2);

fn retryable(status: &tonic::Status) -> bool {
    matches!(
        status.code(),
        tonic::Code::Unavailable
            | tonic::Code::DeadlineExceeded
            | tonic::Code::Aborted
            | tonic::Code::ResourceExhausted
    )
}

async fn retry_ready<'a, F>(window: Duration, interval: Duration, mut attempt: F) -> Result<(), tonic::Status>
where
    F: FnMut(Duration) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), tonic::Status>> + Send + 'a>>,
{
    let deadline = Instant::now() + window;
    let mut last = None;
    loop {
        let now = Instant::now();
        let remaining = deadline.saturating_duration_since(now);
        if remaining.is_zero() {
            return Err(last.unwrap_or_else(|| {
                tonic::Status::unavailable("Owner authority unavailable")
            }));
        }
        match attempt(remaining.min(READY_ATTEMPT)).await {
            Ok(()) => return Ok(()),
            Err(status) if !retryable(&status) => return Err(status),
            Err(status) => last = Some(status),
        }
        let now = Instant::now();
        if now >= deadline {
            return Err(last.expect("one attempt completed"));
        }
        tokio::time::sleep(interval.min(deadline.saturating_duration_since(now))).await;
    }
}

pub async fn verify_ready_bounded(store: &PgOwnerAuthority) -> Result<(), tonic::Status> {
    retry_ready(READY_WINDOW, READY_INTERVAL, |remaining| {
        Box::pin(async move {
            let budget = OperationBudget::new(Instant::now() + remaining)
                .map_err(|_| tonic::Status::unavailable("Owner authority unavailable"))?;
            store.verify_ready(&budget).await
        })
    })
    .await
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
        &[
            Workload::Concert,
            Workload::Post,
            Workload::Profile,
            Workload::Notification,
        ],
        &[Workload::Concert],
    )
}

#[cfg(test)]
mod startup_retry_tests {
    use super::*;

    #[tokio::test]
    async fn late_settlement_owner_is_reached_within_the_window() {
        let mut calls = 0;
        let result = retry_ready(Duration::from_millis(200), Duration::from_millis(5), |_| {
            calls += 1;
            let pending = calls < 3;
            Box::pin(async move {
                if pending {
                    Err(tonic::Status::unavailable("Owner authority unavailable"))
                } else {
                    Ok(())
                }
            })
        })
        .await;
        assert!(result.is_ok());
        assert_eq!(calls, 3);
    }

    #[tokio::test]
    async fn exhausted_window_returns_the_original_error_without_ready_state() {
        let mut calls = 0;
        let result = retry_ready(Duration::from_millis(20), Duration::from_millis(4), |_| {
            calls += 1;
            Box::pin(async { Err(tonic::Status::unavailable("Owner authority unavailable")) })
        })
        .await;
        let status = result.expect_err("exhaustion must fail");
        assert_eq!(status.code(), tonic::Code::Unavailable);
        assert_eq!(status.message(), "Owner authority unavailable");
        assert!(calls > 1);
    }

    #[tokio::test]
    async fn definitive_refusal_fails_without_retrying() {
        let mut calls = 0;
        let result = retry_ready(Duration::from_millis(200), Duration::from_millis(5), |_| {
            calls += 1;
            Box::pin(async { Err(tonic::Status::permission_denied("Owner authority denied")) })
        })
        .await;
        assert_eq!(result.expect_err("refusal").code(), tonic::Code::PermissionDenied);
        assert_eq!(calls, 1);
    }
}
