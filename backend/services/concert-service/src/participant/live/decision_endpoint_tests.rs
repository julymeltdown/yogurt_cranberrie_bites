//! External native-join host for the actual read-only decision handler. This
//! fixture supplies no decision, Profile receipt, or mutation grant of its own.
use super::{coordinator_tests::owned_pool, decision_rpc::*};
use crate::participant::proto;
use platform::{
    actor_delegation::{
        participant_decision::{ParticipantDecisionSigner, ParticipantDecisionVerifier},
        request_for, ActorDelegationVerifier,
    },
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor, Workload},
    grpc_method_policy::guard,
};
use std::{
    io::Write,
    os::unix::fs::{MetadataExt, OpenOptionsExt},
    path::{Path, PathBuf},
    time::Duration,
};
use uuid::Uuid;

pub(super) fn fixture_tls(root: &Path, role: &str) -> anyhow::Result<Option<platform::grpc_tls::TlsMaterial>> {
    match std::env::var("MCR_PARTICIPANT_DECISION_TLS_MODE")?.as_str() {
        "plaintext" => Ok(None),
        "mtls" => {
            let owner = std::fs::metadata(root)?.uid();
            let mut paths = Vec::new();
            for kind in ["CERT", "KEY", "CA"] {
                let path = PathBuf::from(std::env::var(format!("{role}_GRPC_TLS_{kind}_PATH"))?);
                let info = std::fs::symlink_metadata(&path)?;
                anyhow::ensure!(
                    path.is_absolute()
                        && path.canonicalize()? == path
                        && path.starts_with(root)
                        && info.is_file()
                        && !info.file_type().is_symlink()
                        && info.uid() == owner
                        && (kind != "KEY" || info.mode() & 0o077 == 0),
                    "owned fixture TLS path required"
                );
                paths.push(path);
            }
            Ok(Some(platform::grpc_tls::TlsMaterial::load(
                &paths[0], &paths[1], &paths[2],
            )?))
        }
        _ => anyhow::bail!("explicit mtls or plaintext fixture mode required"),
    }
}

#[tokio::test]
#[ignore = "external join only: requires owned private Concert PG16.14 and explicit production-format keys"]
async fn native_decision_service_endpoint() -> anyhow::Result<()> {
    // This performs the same marker/inode/socket/server checks as the native
    // journal probe, before reading any external endpoint configuration.
    let pool = owned_pool().await?;
    let root = PathBuf::from(std::env::var("MCR_PARTICIPANT_COORDINATOR_ROOT")?);
    let tls = fixture_tls(&root, "CONCERT")?;
    let ready = root.join("participant-decision-ready.json");
    let ready_temporary = root.join(".participant-decision-ready.tmp");
    let stop = root.join("participant-decision-stop");
    anyhow::ensure!(
        !ready.try_exists()? && !stop.try_exists()?,
        "fresh readiness and stop paths required"
    );
    let gateway = ActorDelegationVerifier::from_env(true, Workload::Concert)?;
    let verifier = ParticipantDecisionVerifier::configured_from_env(&gateway)?
        .ok_or_else(|| anyhow::anyhow!("explicit decision verification keys required"))?;
    let token = std::env::var("MCR_PARTICIPANT_DECISION_PROFILE_TOKEN")?;
    let service = guard(
        &DECISION_POLICY,
        ParticipantDecisionService::new(pool.clone()).server(),
        CallerRegistry::new().register(Workload::Profile, &token)?,
        verifier.into_inner(),
    )?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!(
        "{}://{}",
        if tls.is_some() { "https" } else { "http" },
        listener.local_addr()?
    );
    let incoming = futures::stream::unfold(listener, |listener| async move {
        Some((listener.accept().await.map(|(stream, _)| stream), listener))
    });
    let stop_task = stop.clone();
    let (shutdown, shutdown_receiver) = tokio::sync::oneshot::channel();
    let mut builder = tonic::transport::Server::builder();
    if let Some(material) = tls {
        builder = builder.tls_config(material.server_config()?)?;
    }
    let task = tokio::spawn(async move {
        builder
            .layer(CallDeadlineLayer)
            .add_service(service)
            .serve_with_incoming_shutdown(incoming, async {
                let _ = shutdown_receiver.await;
            })
            .await
    });
    let mut file = std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .mode(0o600)
        .open(&ready_temporary)?;
    serde_json::to_writer(
        &mut file,
        &serde_json::json!({
            "endpoint": endpoint,
            "pid": std::process::id(),
            "service": DECISION_POLICY.service,
            "scope": "actual guarded decision reader; external fixture owns journal operations"
        }),
    )?;
    file.write_all(b"\n")?;
    file.sync_all()?;
    drop(file);
    std::fs::rename(&ready_temporary, &ready)?;
    // Bounded lifetime, including a crashed orchestrator. No timeout can decide
    // a mutation or settle a Profile prepared transaction.
    let stopped = tokio::time::timeout(Duration::from_secs(180), async move {
        loop {
            if let Ok(info) = std::fs::symlink_metadata(&stop_task) {
                use std::os::unix::fs::MetadataExt;
                let owner = std::fs::metadata(stop_task.parent().unwrap())?;
                anyhow::ensure!(
                    info.is_file()
                        && !info.file_type().is_symlink()
                        && info.uid() == owner.uid()
                        && info.mode() & 0o077 == 0,
                    "invalid fixture stop marker"
                );
                return Ok::<_, anyhow::Error>(());
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await;
    let _ = shutdown.send(());
    let served = tokio::time::timeout(Duration::from_secs(3), task).await;
    std::fs::remove_file(&ready)?;
    pool.close().await;
    stopped.map_err(|_| anyhow::anyhow!("external join exceeded fixture lifetime"))??;
    served.map_err(|_| anyhow::anyhow!("decision server shutdown exceeded bound"))???;
    Ok(())
}

#[tokio::test]
#[ignore = "external join only: requires owned private fixture and running decision endpoint"]
async fn native_decision_service_client() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    let root = PathBuf::from(std::env::var("MCR_PARTICIPANT_COORDINATOR_ROOT")?);
    let tls = fixture_tls(&root, "PROFILE")?;
    let ready: serde_json::Value = serde_json::from_slice(&std::fs::read(
        root.join("participant-decision-ready.json"),
    )?)?;
    let endpoint = ready["endpoint"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("missing endpoint"))?;
    let prefix = if tls.is_some() {
        "https://127.0.0.1:"
    } else {
        "http://127.0.0.1:"
    };
    let port: u16 = endpoint
        .strip_prefix(prefix)
        .ok_or_else(|| anyhow::anyhow!("private loopback required"))?
        .parse()?;
    anyhow::ensure!(
        port != 0 && ready["service"] == DECISION_POLICY.service,
        "wrong endpoint"
    );
    let gateway = ActorDelegationVerifier::from_env(true, Workload::Concert)?;
    let signer = ParticipantDecisionSigner::from_env(&gateway)?;
    let endpoint = match tls {
        Some(material) => material.endpoint(endpoint, Workload::Concert)?,
        None => tonic::transport::Endpoint::from_shared(endpoint.to_string())?,
    };
    let channel = endpoint.connect().await?;
    let token = std::env::var("MCR_PARTICIPANT_DECISION_PROFILE_TOKEN")?;
    let user = Uuid::new_v4();
    let operation = Uuid::new_v4();
    let mut query = proto::ParticipantDecisionRequest {
        actor_id: user.to_string(),
        event_id: Uuid::new_v4().to_string(),
        session_family_id: Uuid::new_v4().to_string(),
        operation_id: operation.to_string(),
        parent_digest: vec![11; 32],
        body_digest: vec![12; 32],
        guard_binding_digest: vec![13; 32],
        profile_prepared_gid: format!("mcr_sp_{}", operation.simple()),
        profile_effect_digest: vec![14; 32],
        authority_expires_unix_micros: chrono::Utc::now().timestamp_micros() + 1_500_000,
    };
    for fault in ["valid", "wrong-credential", "foreign-actor"] {
        query.authority_expires_unix_micros = chrono::Utc::now().timestamp_micros() + 1_500_000;
        let mut client = proto::concert_participant_decision_service_client::ConcertParticipantDecisionServiceClient::with_interceptor(
            signer.client(channel.clone()), ServiceIdentityClientInterceptor::new(Workload::Profile, if fault == "wrong-credential" { "incorrect-fixture-credential" } else { &token })?);
        let actor = if fault == "foreign-actor" {
            Uuid::new_v4().to_string()
        } else {
            user.to_string()
        };
        let mut request = request_for(query.clone(), &actor)?;
        request.set_timeout(Duration::from_millis(1500));
        let result = platform::call_deadline::scope(
            std::time::Instant::now() + Duration::from_millis(1500),
            client.resolve_decision(request),
        )
        .await;
        if fault != "valid" {
            assert_eq!(
                result.unwrap_err().code(),
                if fault == "wrong-credential" {
                    tonic::Code::Unauthenticated
                } else {
                    tonic::Code::PermissionDenied
                },
                "{fault}"
            );
            continue;
        }
        let reply = result?;
        assert_eq!(reply.metadata().get("cache-control").unwrap(), "no-store");
        let reply = reply.into_inner();
        assert_eq!(reply.query, Some(query.clone()));
        assert_eq!(
            reply.decision,
            proto::ParticipantDecision::Unresolved as i32
        );
        assert_eq!(reply.committed_revision, 0);
        assert!(
            reply.concert_transaction.is_none(),
            "no operation must not invent a transaction"
        );
    }
    pool.close().await;
    Ok(())
}
