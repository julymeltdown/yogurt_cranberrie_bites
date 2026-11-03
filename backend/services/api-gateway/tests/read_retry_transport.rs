//! Loopback gRPC tests of Gateway read retries against a controlled Profile
//! double; no database. They show over a real TCP transport that a replay-safe
//! read absorbs one UNAVAILABLE, stops after the policy's attempts, and that a
//! mutation is attempted exactly once.
use api_gateway::adapters::out::profile_client::{proto, GrpcProfileClient};
use api_gateway::application::dto::profile_dto::ProfileUpdateRequest;
use api_gateway::application::ports::ProfileClientPort;
use api_gateway::config::GatewayMetrics;
use platform::actor_delegation::ActorDelegationSigner;
use platform::grpc_auth::{CallerRegistry, RequireRegisteredCallerInterceptor, Workload};
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Request, Response, Status};

const TOKEN: &str = "read-retry-transport-token";
const RECOVERS: &str = "a1111111-1111-4111-8111-111111111111";
/// An owner that reports a failed authority exchange marks its UNAVAILABLE.
const MARKED: &str = "c3333333-3333-4333-8333-333333333333";

#[derive(Default)]
struct Calls {
    get_profile: AtomicUsize,
    marked: AtomicUsize,
    get_profiles: AtomicUsize,
    update_profile: AtomicUsize,
}

struct FlakyProfile {
    calls: Arc<Calls>,
}

fn unused<T>() -> Result<Response<T>, Status> {
    Err(Status::unimplemented("not used by the retry fixture"))
}

#[tonic::async_trait]
impl proto::profile_service_server::ProfileService for FlakyProfile {
    async fn replay_mutation(
        &self,
        _request: Request<proto::ReplayMutationRequest>,
    ) -> Result<Response<proto::ReplayMutationResponse>, Status> {
        unused()
    }
    async fn get_mutation_receipt_status(
        &self,
        _request: Request<proto::GetMutationReceiptStatusRequest>,
    ) -> Result<Response<proto::GetMutationReceiptStatusResponse>, Status> {
        unused()
    }
    async fn get_profile(
        &self,
        request: Request<proto::GetProfileRequest>,
    ) -> Result<Response<proto::GetProfileResponse>, Status> {
        let user_id = request.into_inner().user_id;
        if user_id == MARKED {
            self.calls.marked.fetch_add(1, Ordering::SeqCst);
            let mut status = Status::unavailable("owner exchange failed");
            platform::grpc_retry::mark_not_retryable(&mut status);
            return Err(status);
        }
        // The first call fails for every user; RECOVERS answers afterwards.
        if self.calls.get_profile.fetch_add(1, Ordering::SeqCst) == 0 || user_id != RECOVERS {
            return Err(Status::unavailable("profile outage"));
        }
        Ok(Response::new(proto::GetProfileResponse {
            profile: Some(proto::Profile {
                user_id,
                username: "recovered".into(),
                ..Default::default()
            }),
        }))
    }
    async fn get_profile_by_username(
        &self,
        _request: Request<proto::GetProfileByUsernameRequest>,
    ) -> Result<Response<proto::GetProfileResponse>, Status> {
        unused()
    }
    async fn get_profiles(
        &self,
        _request: Request<proto::GetProfilesRequest>,
    ) -> Result<Response<proto::GetProfilesResponse>, Status> {
        self.calls.get_profiles.fetch_add(1, Ordering::SeqCst);
        Err(Status::internal("not retried"))
    }
    async fn update_profile(
        &self,
        _request: Request<proto::UpdateProfileRequest>,
    ) -> Result<Response<proto::UpdateProfileResponse>, Status> {
        self.calls.update_profile.fetch_add(1, Ordering::SeqCst);
        Err(Status::unavailable("outcome unknown to the caller"))
    }
    async fn update_profile_privacy(
        &self,
        _request: Request<proto::UpdateProfilePrivacyRequest>,
    ) -> Result<Response<proto::UpdateProfilePrivacyResponse>, Status> {
        unused()
    }
    async fn get_profile_privacy(
        &self,
        _request: Request<proto::GetProfilePrivacyRequest>,
    ) -> Result<Response<proto::GetProfilePrivacyResponse>, Status> {
        unused()
    }
    async fn delete_profile(
        &self,
        _request: Request<proto::DeleteProfileRequest>,
    ) -> Result<Response<proto::DeleteProfileResponse>, Status> {
        unused()
    }
    async fn search_profiles(
        &self,
        _request: Request<proto::SearchProfilesRequest>,
    ) -> Result<Response<proto::SearchProfilesResponse>, Status> {
        unused()
    }
    async fn upload_avatar(
        &self,
        _request: Request<proto::UploadAvatarRequest>,
    ) -> Result<Response<proto::UploadAvatarResponse>, Status> {
        unused()
    }
    async fn get_avatar(
        &self,
        _request: Request<proto::GetAvatarRequest>,
    ) -> Result<Response<proto::GetAvatarResponse>, Status> {
        unused()
    }
    async fn apply_user_deletion(
        &self,
        _request: Request<proto::ApplyUserDeletionRequest>,
    ) -> Result<Response<proto::ApplyUserDeletionResponse>, Status> {
        unused()
    }
}

#[tokio::test]
async fn gateway_profile_reads_retry_once_and_mutations_run_once() -> anyhow::Result<()> {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("http://{}", listener.local_addr()?);
    let calls = Arc::new(Calls::default());
    let interceptor = RequireRegisteredCallerInterceptor::new(
        CallerRegistry::new().register(Workload::Gateway, TOKEN)?,
    )?;
    let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
    let server = tokio::spawn(
        tonic::transport::Server::builder()
            .add_service(
                proto::profile_service_server::ProfileServiceServer::with_interceptor(
                    FlakyProfile {
                        calls: calls.clone(),
                    },
                    interceptor,
                ),
            )
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            }),
    );
    let metrics = GatewayMetrics::default();
    let client = GrpcProfileClient::connect_with_metrics(
        endpoint,
        Workload::Gateway,
        TOKEN,
        None,
        ActorDelegationSigner::local_parity(Workload::Gateway),
        metrics.clone(),
    )
    .await?;

    // One outage is absorbed by a second attempt over the real transport.
    let profile = client
        .get_profile(RECOVERS.into())
        .await
        .expect("recovered");
    assert_eq!(profile.username, "recovered");
    assert_eq!(calls.get_profile.load(Ordering::SeqCst), 2);

    // A persistent outage stops at the policy's attempts.
    let other = "b2222222-2222-4222-8222-222222222222";
    assert!(client.get_profile(other.into()).await.is_err());
    assert_eq!(
        calls.get_profile.load(Ordering::SeqCst),
        2 + platform::grpc_retry::READ_RETRY.max_attempts as usize
    );

    // A non-UNAVAILABLE failure of a read is returned after one call.
    assert!(client.get_profiles(vec![RECOVERS.into()]).await.is_err());
    assert_eq!(calls.get_profiles.load(Ordering::SeqCst), 1);

    // The owner's no-retry marker crosses the wire and ends the loop.
    assert!(client.get_profile(MARKED.into()).await.is_err());
    assert_eq!(calls.marked.load(Ordering::SeqCst), 1);

    // A mutation whose outcome is unknown is never re-sent by the client.
    let update = client
        .update_profile(
            RECOVERS.into(),
            ProfileUpdateRequest {
                username: "renamed".into(),
                nickname: "Renamed".into(),
                avatar_url: String::new(),
                bio: String::new(),
            },
        )
        .await;
    assert!(update.is_err());
    assert_eq!(calls.update_profile.load(Ordering::SeqCst), 1);

    let rendered = metrics.render_prometheus();
    assert!(
        rendered.contains("grpc_edge_read_retries_total{edge=\"gateway->profile\"} 2"),
        "{rendered}"
    );
    let _ = stop.send(());
    server.await??;
    Ok(())
}
