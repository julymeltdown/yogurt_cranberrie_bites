use anyhow::Result as AnyhowResult;
use friend_service::adapters::out::grpc::profile_privacy_client::proto;
use platform::grpc_auth::{RequireRegisteredCallerInterceptor, Workload};
use std::collections::HashMap;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::net::TcpListener;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Code, Request, Response, Status};

#[derive(Clone)]
pub enum PrivacyReply {
    Visibility(&'static str),
    DelayedVisibility {
        account_visibility: &'static str,
        delay: Duration,
    },
    Status(Code),
    /// UNAVAILABLE for the first `failures` calls, then the visibility.
    /// `calls` counts every call that reached the server.
    Flaky {
        account_visibility: &'static str,
        failures: usize,
        calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    },
    /// Always this status, counting the calls that reached the server.
    CountedStatus {
        code: Code,
        calls: std::sync::Arc<std::sync::atomic::AtomicUsize>,
    },
}

#[derive(Clone)]
pub struct ProfileGrpcFixture {
    pub privacy_replies: HashMap<String, PrivacyReply>,
    pub profiles: Vec<proto::Profile>,
}

pub struct ProfileGrpcTestServer {
    address: SocketAddr,
    shutdown: oneshot::Sender<()>,
    task: JoinHandle<std::result::Result<(), tonic::transport::Error>>,
}

impl ProfileGrpcTestServer {
    pub fn endpoint(&self) -> String {
        format!("http://{}", self.address)
    }

    pub async fn shutdown_and_verify_port_released(self) -> AnyhowResult<()> {
        self.shutdown
            .send(())
            .map_err(|()| anyhow::anyhow!("profile gRPC server shutdown receiver dropped"))?;
        self.task.await??;
        std::net::TcpListener::bind(self.address)?;
        Ok(())
    }
}

pub async fn start_profile_grpc_server(
    fixture: ProfileGrpcFixture,
    expected_service_token: &str,
) -> AnyhowResult<ProfileGrpcTestServer> {
    let listener = TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    // The friend-service client identifies itself as `friend`, so accept that
    // workload with this fixture's credential rather than a bare shared token.
    let interceptor = RequireRegisteredCallerInterceptor::new(
        platform::grpc_auth::CallerRegistry::new()
            .register(Workload::Friend, expected_service_token)?,
    )?;
    let service = FixtureProfileService { fixture };
    let (shutdown, shutdown_signal) = oneshot::channel();
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .add_service(
                proto::profile_service_server::ProfileServiceServer::with_interceptor(
                    service,
                    interceptor,
                ),
            )
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async move {
                let _ = shutdown_signal.await;
            })
            .await
    });

    Ok(ProfileGrpcTestServer {
        address,
        shutdown,
        task,
    })
}

struct FixtureProfileService {
    fixture: ProfileGrpcFixture,
}

#[tonic::async_trait]
impl proto::profile_service_server::ProfileService for FixtureProfileService {
    async fn replay_mutation(
        &self,
        _request: tonic::Request<proto::ReplayMutationRequest>,
    ) -> Result<tonic::Response<proto::ReplayMutationResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Profile mutation replay is not used by the Friend fixture",
        ))
    }

    async fn get_mutation_receipt_status(
        &self,
        _request: tonic::Request<proto::GetMutationReceiptStatusRequest>,
    ) -> Result<tonic::Response<proto::GetMutationReceiptStatusResponse>, tonic::Status> {
        Err(tonic::Status::unimplemented(
            "Profile mutation status is not used by the Friend fixture",
        ))
    }

    async fn get_profile(
        &self,
        _request: Request<proto::GetProfileRequest>,
    ) -> std::result::Result<Response<proto::GetProfileResponse>, Status> {
        Err(Status::unimplemented(
            "fixture only supports privacy and discovery",
        ))
    }

    async fn get_profile_by_username(
        &self,
        _request: Request<proto::GetProfileByUsernameRequest>,
    ) -> std::result::Result<Response<proto::GetProfileResponse>, Status> {
        Err(Status::unimplemented(
            "fixture only supports privacy and discovery",
        ))
    }

    async fn get_profiles(
        &self,
        _request: Request<proto::GetProfilesRequest>,
    ) -> std::result::Result<Response<proto::GetProfilesResponse>, Status> {
        Ok(Response::new(proto::GetProfilesResponse {
            profiles: self.fixture.profiles.clone(),
        }))
    }

    async fn update_profile(
        &self,
        _request: Request<proto::UpdateProfileRequest>,
    ) -> std::result::Result<Response<proto::UpdateProfileResponse>, Status> {
        Err(Status::unimplemented(
            "fixture only supports privacy and discovery",
        ))
    }

    async fn update_profile_privacy(
        &self,
        _request: Request<proto::UpdateProfilePrivacyRequest>,
    ) -> std::result::Result<Response<proto::UpdateProfilePrivacyResponse>, Status> {
        Err(Status::unimplemented(
            "fixture only supports privacy and discovery",
        ))
    }

    async fn get_profile_privacy(
        &self,
        request: Request<proto::GetProfilePrivacyRequest>,
    ) -> std::result::Result<Response<proto::GetProfilePrivacyResponse>, Status> {
        let request = request.into_inner();
        match self.fixture.privacy_replies.get(&request.user_id) {
            Some(PrivacyReply::Visibility(account_visibility)) => {
                Ok(Response::new(proto::GetProfilePrivacyResponse {
                    user_id: request.user_id,
                    account_visibility: (*account_visibility).into(),
                    lifecycle_status: "ACTIVE".into(),
                }))
            }
            Some(PrivacyReply::DelayedVisibility {
                account_visibility,
                delay,
            }) => {
                tokio::time::sleep(*delay).await;
                Ok(Response::new(proto::GetProfilePrivacyResponse {
                    user_id: request.user_id,
                    account_visibility: (*account_visibility).into(),
                    lifecycle_status: "ACTIVE".into(),
                }))
            }
            Some(PrivacyReply::Status(code)) => Err(Status::new(*code, "fixture status")),
            Some(PrivacyReply::Flaky {
                account_visibility,
                failures,
                calls,
            }) => {
                let seen = calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                if seen < *failures {
                    return Err(Status::unavailable("fixture outage"));
                }
                Ok(Response::new(proto::GetProfilePrivacyResponse {
                    user_id: request.user_id,
                    account_visibility: (*account_visibility).into(),
                    lifecycle_status: "ACTIVE".into(),
                }))
            }
            Some(PrivacyReply::CountedStatus { code, calls }) => {
                calls.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Err(Status::new(*code, "fixture status"))
            }
            None => Err(Status::not_found("privacy fixture missing user")),
        }
    }

    async fn delete_profile(
        &self,
        _request: Request<proto::DeleteProfileRequest>,
    ) -> std::result::Result<Response<proto::DeleteProfileResponse>, Status> {
        Err(Status::unimplemented(
            "fixture only supports privacy and discovery",
        ))
    }

    async fn search_profiles(
        &self,
        _request: Request<proto::SearchProfilesRequest>,
    ) -> std::result::Result<Response<proto::SearchProfilesResponse>, Status> {
        Err(Status::unimplemented(
            "fixture only supports privacy and discovery",
        ))
    }

    async fn upload_avatar(
        &self,
        _request: Request<proto::UploadAvatarRequest>,
    ) -> std::result::Result<Response<proto::UploadAvatarResponse>, Status> {
        Err(Status::unimplemented(
            "fixture only supports privacy and discovery",
        ))
    }

    async fn get_avatar(
        &self,
        _request: Request<proto::GetAvatarRequest>,
    ) -> std::result::Result<Response<proto::GetAvatarResponse>, Status> {
        Err(Status::unimplemented(
            "fixture only supports privacy and discovery",
        ))
    }

    async fn apply_user_deletion(
        &self,
        _request: Request<proto::ApplyUserDeletionRequest>,
    ) -> std::result::Result<Response<proto::ApplyUserDeletionResponse>, Status> {
        Err(Status::unimplemented(
            "fixture only supports privacy and discovery",
        ))
    }
}
