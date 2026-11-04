//! Actual guarded replay servers, adapted from client checkpoint 998cf728.
use super::*;
use platform::{
    grpc_auth::{CallerRegistry, Workload},
    grpc_tls::TlsMaterial,
};
use std::{os::unix::fs::MetadataExt, path::PathBuf};
pub fn material(role: Workload) -> anyhow::Result<TlsMaterial> {
    let root = PathBuf::from(std::env::var("MCR_WORLD_REMOTE_ROOT")?);
    let metadata = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.canonicalize()? == root
            && metadata.is_dir()
            && metadata.mode() & 0o077 == 0
            && format!("{}:{}", metadata.dev(), metadata.ino())
                == std::env::var("MCR_WORLD_REMOTE_IDENTITY")?,
        "foreign TLS root"
    );
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("mcr-owner.json"))?)?;
    anyhow::ensure!(
        marker["uid"].as_u64() == Some(u64::from(metadata.uid()))
            && marker["token"].as_str() == Some(std::env::var("MCR_WORLD_REMOTE_TOKEN")?.as_str()),
        "foreign TLS marker"
    );
    let p = root.join("pki");
    anyhow::ensure!(p.canonicalize()? == p, "foreign PKI");
    TlsMaterial::load(
        p.join(format!("{}.crt", role.as_str())),
        p.join(format!("{}.key", role.as_str())),
        p.join("ca.crt"),
    )
}
fn registry() -> CallerRegistry {
    CallerRegistry::new()
        .register(Workload::Concert, "remote-fixture-concert")
        .unwrap()
}
pub struct Server {
    pub endpoint: String,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
}
impl Server {
    /// Negative source fixture: actual guarded journal handler and TLS identity,
    /// with explicitly corrupted responses after its real consistent SQL read.
    pub async fn corruptible_auth(pool: PgPool, mode: Arc<AtomicUsize>) -> anyhow::Result<Self> {
        use auth_service::adapters::r#in::grpc::concert_invalidation::{
            InvalidationServer, POLICY,
        };
        use auth_service::adapters::r#in::grpc::concert_relationship::proto;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("https://{}", listener.local_addr()?);
        let generated=proto::auth_concert_invalidation_service_server::AuthConcertInvalidationServiceServer::new(Corruptible {inner:InvalidationServer::from_pool(pool),mode});
        let guarded =
            platform::grpc_method_policy::guard_without_delegation(&POLICY, generated, registry())?;
        let (stop, rx) = tokio::sync::oneshot::channel();
        let incoming = futures::stream::unfold(listener, |listener| async move {
            Some((listener.accept().await.map(|(s, _)| s), listener))
        });
        let task = tokio::spawn(async move {
            tonic::transport::Server::builder()
                .tls_config(material(Workload::Auth)?.server_config()?)?
                .layer(platform::call_deadline::CallDeadlineLayer)
                .add_service(guarded)
                .serve_with_incoming_shutdown(incoming, async {
                    let _ = rx.await;
                })
                .await?;
            Ok(())
        });
        Ok(Self {
            endpoint,
            stop: Some(stop),
            task,
        })
    }
    pub async fn new(owner: InvalidationOwner, pool: PgPool) -> anyhow::Result<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("https://{}", listener.local_addr()?);
        let role = match owner {
            InvalidationOwner::Auth => Workload::Auth,
            InvalidationOwner::Friend => Workload::Friend,
        };
        let mut builder = tonic::transport::Server::builder()
            .tls_config(material(role)?.server_config()?)?
            .layer(platform::call_deadline::CallDeadlineLayer);
        let (stop, rx) = tokio::sync::oneshot::channel();
        let incoming = futures::stream::unfold(listener, |listener| async move {
            Some((listener.accept().await.map(|(s, _)| s), listener))
        });
        let task = match owner {
            InvalidationOwner::Auth => {
                let server = builder.add_service(
                    auth_service::adapters::r#in::grpc::concert_invalidation::guarded_server(
                        pool,
                        registry(),
                    )?,
                );
                tokio::spawn(async move {
                    server
                        .serve_with_incoming_shutdown(incoming, async {
                            let _ = rx.await;
                        })
                        .await?;
                    Ok(())
                })
            }
            InvalidationOwner::Friend => {
                let server = builder.add_service(
                    friend_service::adapters::r#in::grpc::concert_invalidation::guarded_server(
                        pool,
                        registry(),
                    )?,
                );
                tokio::spawn(async move {
                    server
                        .serve_with_incoming_shutdown(incoming, async {
                            let _ = rx.await;
                        })
                        .await?;
                    Ok(())
                })
            }
        };
        Ok(Self {
            endpoint,
            stop: Some(stop),
            task,
        })
    }
    pub fn source(&self, owner: InvalidationOwner) -> anyhow::Result<InvalidationSource> {
        Ok(InvalidationSource::new(
            owner,
            &self.endpoint,
            "remote-fixture-concert",
            &material(Workload::Concert)?,
        )?)
    }
    pub async fn finish(mut self) -> anyhow::Result<()> {
        let _ = self.stop.take().unwrap().send(());
        tokio::time::timeout(Duration::from_secs(2), &mut self.task).await???;
        Ok(())
    }
}
struct Corruptible {
    inner: auth_service::adapters::r#in::grpc::concert_invalidation::InvalidationServer,
    mode: Arc<AtomicUsize>,
}
use auth_service::adapters::r#in::grpc::concert_relationship::proto as auth_proto;
#[tonic::async_trait]
impl auth_proto::auth_concert_invalidation_service_server::AuthConcertInvalidationService
    for Corruptible
{
    async fn read_head(
        &self,
        r: tonic::Request<auth_proto::ReadInvalidationHeadRequest>,
    ) -> Result<tonic::Response<auth_proto::ReadInvalidationHeadResponse>, tonic::Status> {
        use auth_proto::auth_concert_invalidation_service_server::AuthConcertInvalidationService;
        self.inner.read_head(r).await
    }
    async fn read_after(
        &self,
        r: tonic::Request<auth_proto::ReadInvalidationsRequest>,
    ) -> Result<tonic::Response<auth_proto::ReadInvalidationsResponse>, tonic::Status> {
        use auth_proto::auth_concert_invalidation_service_server::AuthConcertInvalidationService;
        let mut response = self.inner.read_after(r).await?;
        let page = response.get_mut();
        match self.mode.load(Ordering::Acquire) {
            0 => {}
            1 => page.query.as_mut().unwrap().request_id = Uuid::new_v4().to_string(),
            2 => page.head.as_mut().unwrap().owner_epoch = Uuid::new_v4().to_string(),
            3 => {
                if let Some(s) = page.signals.first_mut() {
                    s.cursor.as_mut().unwrap().sequence += 1;
                }
            }
            4 => page.signals.reverse(),
            5 => {
                if let Some(s) = page.signals.first_mut() {
                    s.reason = auth_proto::InvalidationReason::FriendUserRemoved as i32;
                }
            }
            6 => {
                if page.signals.len() > 1 {
                    page.signals[1].signal_id = page.signals[0].signal_id.clone();
                }
            }
            7 => tokio::time::sleep(Duration::from_millis(1200)).await,
            8 => {
                self.mode.store(9, Ordering::Release);
                tokio::time::sleep(Duration::from_millis(1200)).await;
            }
            _ => return Err(tonic::Status::unavailable("source fixture unavailable")),
        }
        Ok(response)
    }
}
impl Drop for Server {
    fn drop(&mut self) {
        self.task.abort();
    }
}
pub struct Servers {
    pub auth: Server,
    pub friend: Server,
}
impl Servers {
    pub async fn new(s: &fixture::Stack) -> anyhow::Result<Self> {
        Ok(Self {
            auth: Server::new(InvalidationOwner::Auth, s.auth.clone()).await?,
            friend: Server::new(InvalidationOwner::Friend, s.friend.clone()).await?,
        })
    }
    pub fn runtime(&self, s: &fixture::Stack) -> anyhow::Result<RemoteInvalidationRuntime> {
        Ok(RemoteInvalidationRuntime::new(
            s.concert.clone(),
            s.bridge.invalidation_context().1,
            self.auth.source(InvalidationOwner::Auth)?,
            self.friend.source(InvalidationOwner::Friend)?,
            vec![s.bridge.clone()],
        )?)
    }
    pub async fn finish(self) -> anyhow::Result<()> {
        self.auth.finish().await?;
        self.friend.finish().await
    }
}
