//! One running issuer instance for cross-service drills: the real auth state,
//! gRPC handler, and registered-caller interceptor over the test database, and
//! the Gateway's own client connected to it.
//!
//! The channel is the plaintext development path; mutual TLS on it is covered
//! by the transport tests.
use crate::support::TestApp;
use api_gateway::adapters::out::auth_client::GrpcAuthClient;
use auth_service::adapters::r#in::grpc::server::proto::auth_service_server::AuthServiceServer;
use auth_service::adapters::r#in::grpc::server::AuthGrpcServer;
use auth_service::config::{build_state, AppConfig, AppState};
use platform::grpc_auth::{RequireRegisteredCallerInterceptor, Workload};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context, Poll};
use std::time::Duration;
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;
use tonic::codegen::tokio_stream::Stream;
use tonic::transport::Server;

/// The credential the test harness registers for the Gateway.
pub const GATEWAY_CREDENTIAL: &str = "test-gateway-credential";

pub struct Issuer {
    pub state: Arc<AppState>,
    pub client: GrpcAuthClient,
    shutdown: oneshot::Sender<()>,
    task: tokio::task::JoinHandle<Result<(), tonic::transport::Error>>,
}

impl Issuer {
    pub async fn serve(app: &TestApp, config: AppConfig) -> anyhow::Result<Self> {
        let state = Arc::new(build_state(&config, app.pool.clone()).await?);
        let listener = TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("http://{}", listener.local_addr()?);
        let service = AuthServiceServer::with_interceptor(
            AuthGrpcServer::new(state.clone()),
            RequireRegisteredCallerInterceptor::new(config.caller_registry.clone())?,
        );
        let (shutdown, stopped) = oneshot::channel::<()>();
        let task = tokio::spawn(
            Server::builder()
                .add_service(service)
                .serve_with_incoming_shutdown(Incoming(listener), async move {
                    let _ = stopped.await;
                }),
        );
        let client =
            GrpcAuthClient::connect(endpoint, Workload::Gateway, GATEWAY_CREDENTIAL, None).await?;
        Ok(Self {
            state,
            client,
            shutdown,
            task,
        })
    }

    pub async fn stop(self) -> anyhow::Result<()> {
        drop(self.client);
        let _ = self.shutdown.send(());
        tokio::time::timeout(Duration::from_secs(5), self.task).await???;
        Ok(())
    }
}

struct Incoming(TcpListener);

impl Stream for Incoming {
    type Item = Result<TcpStream, std::io::Error>;

    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0
            .poll_accept(cx)
            .map(|result| Some(result.map(|(stream, _)| stream)))
    }
}
