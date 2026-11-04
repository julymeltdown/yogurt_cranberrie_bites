//! Negative-only authenticated response fixtures. They never qualify a business
//! owner, successful deletion, source proof or production mount.
use auth_application::concert_deletion::*;
use auth_service::adapters::out::grpc::concert_deletion_client::{proto, ConcertDeletionClient};
use platform::{
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::{guard_without_delegation, MethodPolicy},
    grpc_tls::TlsMaterial,
};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::time::Instant;
const POLICY: MethodPolicy = MethodPolicy {
    service: "concert.account_deletion.v1.AccountDeletionService",
    workload: Some(Workload::Concert),
    methods: &[("Apply", &[Workload::Auth])],
    actor_fields: &[],
};
#[test]
fn plaintext_has_no_transport_fallback() {
    assert!(ConcertDeletionClient::connect(
        "http://127.0.0.1:1",
        &TlsMaterial::from_pem(Vec::new(), Vec::new(), Vec::new()),
        "fixture"
    )
    .is_err());
}
fn command() -> ConcertDeletionCommand {
    let user = uuid::Uuid::new_v4();
    ConcertDeletionCommand {
        key: ConcertDeletionKey {
            saga_id: uuid::Uuid::new_v4(),
            user_id: user,
            target: ConcertDeletionTarget::Tickets,
        },
        actor_id: user,
        retention_until: chrono::DateTime::from_timestamp(1_800_000_000, 123_456_789).unwrap(),
    }
}
// Ownership checks adapted read-only from canonical_owner_composition_support/
// environment.rs at fa6f3f72, not a new cluster or ambient PKI helper.
fn tls(role: Workload) -> anyhow::Result<TlsMaterial> {
    use std::os::unix::fs::{MetadataExt, PermissionsExt};
    let root = std::path::PathBuf::from(std::env::var("MCR_CANONICAL_ROOT")?);
    let m = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && m.is_dir()
            && !m.file_type().is_symlink()
            && m.permissions().mode() & 0o077 == 0,
        "owned root"
    );
    anyhow::ensure!(
        format!("{}:{}", m.dev(), m.ino()) == std::env::var("MCR_CANONICAL_ROOT_IDENTITY")?,
        "root identity"
    );
    let marker = root.join("mcr-owner.json");
    let mm = std::fs::symlink_metadata(&marker)?;
    anyhow::ensure!(
        mm.is_file() && !mm.file_type().is_symlink() && mm.permissions().mode() & 0o077 == 0,
        "owned marker"
    );
    let v: serde_json::Value = serde_json::from_slice(&std::fs::read(marker)?)?;
    anyhow::ensure!(
        v["uid"].as_u64() == Some(m.uid() as u64)
            && v["token"].as_str() == Some(std::env::var("MCR_CANONICAL_ROOT_TOKEN")?.as_str()),
        "marker identity"
    );
    let p = root.join("pki");
    TlsMaterial::load(
        p.join(format!("{}.crt", role.as_str())),
        p.join(format!("{}.key", role.as_str())),
        p.join("ca.crt"),
    )
}
struct Negative {
    mode: Arc<AtomicUsize>,
    calls: Arc<AtomicUsize>,
}
#[tonic::async_trait]
impl proto::account_deletion_service_server::AccountDeletionService for Negative {
    async fn apply(
        &self,
        q: tonic::Request<proto::ApplyRequest>,
    ) -> Result<tonic::Response<proto::ApplyResponse>, tonic::Status> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let q = q.into_inner();
        let mode = self.mode.load(Ordering::SeqCst);
        let mut r = proto::ApplyResponse {
            command: q.command,
            fence_revision: 1,
            source_binding: vec![9; 32],
            request_nonce: q.request_nonce,
            expires_unix_micros: q.expires_unix_micros,
        };
        match mode {
            0 => r.command = None,
            1 => r.command.as_mut().unwrap().saga_id = uuid::Uuid::new_v4().to_string(),
            2 => r.command.as_mut().unwrap().user_id = uuid::Uuid::new_v4().to_string(),
            3 => r.command.as_mut().unwrap().actor_id = uuid::Uuid::new_v4().to_string(),
            4 => r.command.as_mut().unwrap().target = 2,
            5 => r.command.as_mut().unwrap().retention_seconds += 1,
            6 => r.command.as_mut().unwrap().retention_nanos += 1,
            7 => r.fence_revision = 0,
            8 => r.source_binding = vec![0; 32],
            9 => r.source_binding.truncate(31),
            10 => r.request_nonce = uuid::Uuid::new_v4().as_bytes().to_vec(),
            11 => r.expires_unix_micros += 1,
            12 => (), // no no-store metadata below
            13 => r.source_binding.clear(),
            14 => return Err(tonic::Status::unavailable("negative fixture no reply")),
            15 => {
                tokio::time::sleep(Duration::from_secs(1)).await;
                return Err(tonic::Status::unavailable("negative delayed fixture"));
            }
            _ => return Err(tonic::Status::unavailable("negative fixture only")),
        }
        let mut response = tonic::Response::new(r);
        if mode != 12 {
            response
                .metadata_mut()
                .insert("cache-control", "no-store".parse().unwrap());
        }
        Ok(response)
    }
}
#[tokio::test]
#[ignore = "requires owned existing PG16 fixture PKI; negative-only authenticated responses"]
async fn authenticated_malformed_receipts_and_original_clock_are_rejected() -> anyhow::Result<()> {
    let mode = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicUsize::new(0));
    let service = guard_without_delegation(
        &POLICY,
        proto::account_deletion_service_server::AccountDeletionServiceServer::new(Negative {
            mode: mode.clone(),
            calls: calls.clone(),
        }),
        CallerRegistry::new().register(Workload::Auth, "auth-fixture")?,
    )?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let url = format!("https://{}", listener.local_addr()?);
    let incoming = tokio_stream::wrappers::TcpListenerStream::new(listener);
    let config = tls(Workload::Concert)?.server_config()?;
    let server = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .tls_config(config)
            .unwrap()
            .layer(platform::call_deadline::CallDeadlineLayer)
            .add_service(service)
            .serve_with_incoming(incoming)
            .await
            .unwrap();
    });
    struct Stop(tokio::task::JoinHandle<()>);
    impl Drop for Stop {
        fn drop(&mut self) {
            self.0.abort();
        }
    }
    let _server = Stop(server);
    let client = ConcertDeletionClient::connect(&url, &tls(Workload::Auth)?, "auth-fixture")?;
    for value in 0..15 {
        mode.store(value, Ordering::SeqCst);
        assert!(
            client
                .apply_with_deadline(command(), Instant::now() + Duration::from_secs(1))
                .await
                .is_err(),
            "negative-only mode {value}"
        );
    }
    let owners = client.owners();
    let count = calls.load(Ordering::SeqCst);
    assert_eq!(
        owners.tickets.as_ref().unwrap().apply(command()).await,
        Err(ConcertDeletionFailure::Unavailable)
    );
    assert_eq!(
        owners.sessions.as_ref().unwrap().apply(command()).await,
        Err(ConcertDeletionFailure::InvalidCommand)
    );
    assert_eq!(
        owners.sharing.as_ref().unwrap().apply(command()).await,
        Err(ConcertDeletionFailure::InvalidCommand)
    );
    assert_eq!(
        owners.private_data.as_ref().unwrap().apply(command()).await,
        Err(ConcertDeletionFailure::InvalidCommand)
    );
    assert_eq!(
        calls.load(Ordering::SeqCst),
        count,
        "missing clock/wrong typed target must not dispatch"
    );
    mode.store(15, Ordering::SeqCst);
    let original = Instant::now() + Duration::from_millis(30);
    let began = Instant::now();
    platform::call_deadline::scope(original.into_std(), async {
        assert!(client
            .apply_with_deadline(command(), Instant::now() + Duration::from_secs(1))
            .await
            .is_err());
        assert!(client
            .apply_with_deadline(command(), original)
            .await
            .is_err());
    })
    .await;
    assert!(began.elapsed() < Duration::from_millis(500));
    println!("BE42 APPLY CLIENT NEGATIVE PASS: authenticated malformed/partial/rebound receipts and absent/expired/shorter original clock refused; no business owner qualified");
    Ok(())
}
