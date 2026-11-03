use super::*;
use futures::Stream;
use std::{
    os::unix::fs::MetadataExt,
    pin::Pin,
    task::{Context, Poll},
};
use tonic::{
    body::Body,
    codegen::{http, Service},
    server::NamedService,
};

pub(super) fn root() -> anyhow::Result<PathBuf> {
    let root = PathBuf::from(std::env::var("MCR_INVALIDATION_CLIENT_ROOT")?);
    anyhow::ensure!(
        root.starts_with("/private/tmp") && root.canonicalize()? == root,
        "unsafe fixture root"
    );
    let m = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        m.is_dir()
            && m.mode() & 0o077 == 0
            && format!("{}:{}", m.dev(), m.ino())
                == std::env::var("MCR_INVALIDATION_CLIENT_IDENTITY")?,
        "foreign fixture root"
    );
    let marker: serde_json::Value =
        serde_json::from_slice(&std::fs::read(root.join("mcr-owner.json"))?)?;
    anyhow::ensure!(
        marker["uid"].as_u64() == Some(u64::from(m.uid()))
            && marker["token"].as_str()
                == Some(std::env::var("MCR_INVALIDATION_CLIENT_TOKEN")?.as_str()),
        "foreign fixture marker"
    );
    Ok(root)
}
pub(super) fn tls(role: Workload) -> anyhow::Result<TlsMaterial> {
    let p = root()?.join("pki");
    TlsMaterial::load(
        p.join(format!("{}.crt", role.as_str())),
        p.join(format!("{}.key", role.as_str())),
        p.join("ca.crt"),
    )
}
pub(super) struct Incoming(tokio::net::TcpListener);
impl Stream for Incoming {
    type Item = Result<tokio::net::TcpStream, std::io::Error>;
    fn poll_next(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        self.0.poll_accept(cx).map(|r| Some(r.map(|(s, _)| s)))
    }
}
pub(super) struct Running {
    pub endpoint: String,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: tokio::task::JoinHandle<anyhow::Result<()>>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.task.abort();
    }
}
impl Running {
    pub async fn actual(owner: InvalidationOwner, pool: sqlx::PgPool) -> anyhow::Result<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("https://{}", listener.local_addr()?);
        let (stop, rx) = tokio::sync::oneshot::channel();
        let role = match owner {
            InvalidationOwner::Auth => Workload::Auth,
            InvalidationOwner::Friend => Workload::Friend,
        };
        let mut builder = tonic::transport::Server::builder()
            .tls_config(tls(role)?.server_config()?)?
            .layer(platform::call_deadline::CallDeadlineLayer);
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
                        .serve_with_incoming_shutdown(Incoming(listener), async {
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
                        .serve_with_incoming_shutdown(Incoming(listener), async {
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
    pub async fn peer(case: Case) -> anyhow::Result<Self> {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let endpoint = format!("https://{}", listener.local_addr()?);
        let (stop, rx) = tokio::sync::oneshot::channel();
        let mut builder = tonic::transport::Server::builder()
            .tls_config(tls(Workload::Auth)?.server_config()?)?;
        let task = if matches!(
            case,
            Case::Malformed | Case::Truncated | Case::Oversized | Case::SlowBody
        ) {
            let server =
                builder.add_service(platform::grpc_method_policy::guard_without_delegation(
                    &auth_service::adapters::r#in::grpc::concert_invalidation::POLICY,
                    RawPeer(case),
                    registry(),
                )?);
            tokio::spawn(async move {
                server
                    .serve_with_incoming_shutdown(Incoming(listener), async {
                        let _ = rx.await;
                    })
                    .await?;
                Ok(())
            })
        } else {
            let generated=proto::auth_concert_invalidation_service_server::AuthConcertInvalidationServiceServer::new(Peer(case));
            let server =
                builder.add_service(platform::grpc_method_policy::guard_without_delegation(
                    &auth_service::adapters::r#in::grpc::concert_invalidation::POLICY,
                    generated,
                    registry(),
                )?);
            tokio::spawn(async move {
                server
                    .serve_with_incoming_shutdown(Incoming(listener), async {
                        let _ = rx.await;
                    })
                    .await?;
                Ok(())
            })
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
            "private-fixture-concert",
            &tls(Workload::Concert)?,
        )?)
    }
    pub async fn close(mut self) -> anyhow::Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        tokio::time::timeout(Duration::from_secs(2), &mut self.task).await???;
        anyhow::ensure!(
            tokio::net::TcpStream::connect(self.endpoint.trim_start_matches("https://"))
                .await
                .is_err(),
            "listener remains"
        );
        Ok(())
    }
}
#[derive(Clone, Copy, Debug)]
pub(super) enum Case {
    Query,
    Epoch,
    Gap,
    ForeignScope,
    Conflict,
    Malformed,
    Truncated,
    Oversized,
    SlowBody,
    SlowHandler,
}
struct Peer(Case);
fn reply<T>(value: T) -> Response<T> {
    let mut r = Response::new(value);
    r.metadata_mut()
        .insert("cache-control", "no-store".parse().unwrap());
    r
}
#[tonic::async_trait]
impl proto::auth_concert_invalidation_service_server::AuthConcertInvalidationService for Peer {
    async fn read_head(
        &self,
        r: Request<proto::ReadInvalidationHeadRequest>,
    ) -> Result<Response<proto::ReadInvalidationHeadResponse>, Status> {
        let mut query = r.into_inner();
        if matches!(self.0, Case::SlowHandler) {
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        if matches!(self.0, Case::Query) {
            query.request_id = Uuid::new_v4().to_string();
        }
        Ok(reply(proto::ReadInvalidationHeadResponse {
            query: Some(query),
            head: Some(proto::InvalidationCursor {
                owner_epoch: Uuid::new_v4().to_string(),
                sequence: 0,
            }),
        }))
    }
    async fn read_after(
        &self,
        r: Request<proto::ReadInvalidationsRequest>,
    ) -> Result<Response<proto::ReadInvalidationsResponse>, Status> {
        let q = r.into_inner();
        let after = q.after.as_ref().unwrap();
        let mut cursor = after.clone();
        cursor.sequence += 1;
        if matches!(self.0, Case::Gap) {
            cursor.sequence += 1;
        }
        let mut signal = proto::InvalidationSignal {
            cursor: Some(cursor.clone()),
            signal_id: Uuid::new_v4().to_string(),
            reason: proto::InvalidationReason::AuthUserRemoved as i32,
            scope: Some(proto::invalidation_signal::Scope::User(
                proto::InvalidationUserScope {
                    user_id: Uuid::new_v4().to_string(),
                },
            )),
        };
        if matches!(self.0, Case::ForeignScope) {
            signal.reason = proto::InvalidationReason::FriendUserRemoved as i32;
        }
        let mut signals = vec![signal];
        if matches!(self.0, Case::Conflict) {
            let mut conflict = signals[0].clone();
            cursor.sequence += 1;
            conflict.cursor = Some(cursor.clone());
            signals.push(conflict);
        }
        let mut head = cursor.clone();
        if matches!(self.0, Case::Epoch) {
            head.owner_epoch = Uuid::new_v4().to_string();
        }
        Ok(reply(proto::ReadInvalidationsResponse {
            query: Some(q),
            next: Some(cursor),
            head: Some(head),
            signals,
        }))
    }
}
#[derive(Clone)]
struct RawPeer(Case);
impl NamedService for RawPeer {
    const NAME: &'static str = "relationship.v1.AuthConcertInvalidationService";
}
impl Service<http::Request<Body>> for RawPeer {
    type Response = http::Response<Body>;
    type Error = Infallible;
    type Future =
        Pin<Box<dyn std::future::Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, _: http::Request<Body>) -> Self::Future {
        let body = match self.0 {
            Case::SlowBody => {
                Body::new(http_body_util::StreamBody::new(futures::stream::pending::<
                    Result<_, Infallible>,
                >()))
            }
            case => {
                let bytes = match case {
                    Case::Malformed => vec![0, 0, 0, 0, 2, 0x0a, 0xff],
                    Case::Truncated => vec![0, 0, 0, 0, 30, 0x0a],
                    Case::Oversized => {
                        let mut b = vec![0];
                        b.extend_from_slice(&(200_000u32).to_be_bytes());
                        b.resize(200_005, 0);
                        b
                    }
                    _ => unreachable!(),
                };
                Body::new(http_body_util::Full::new(tonic::codegen::Bytes::from(
                    bytes,
                )))
            }
        };
        Box::pin(async move {
            Ok(http::Response::builder()
                .header("content-type", "application/grpc")
                .body(body)
                .unwrap())
        })
    }
}

#[tokio::test]
#[ignore = "requires invalidation_clients_smoke.py owned TLS fixture"]
async fn authenticated_malicious_peers_cannot_mint_verified_observations() -> anyhow::Result<()> {
    let consumer = Uuid::new_v4();
    for (case, error) in [
        (Case::Query, ReadError::QueryMismatch),
        (Case::Malformed, ReadError::MalformedResponse),
        (Case::Truncated, ReadError::MalformedResponse),
        (Case::Oversized, ReadError::ResponseTooLarge),
    ] {
        let server = Running::peer(case).await?;
        let source = server.source(InvalidationOwner::Auth)?;
        assert_eq!(
            source.read_head(consumer, &budget()).await.unwrap_err(),
            error,
            "{case:?}"
        );
        drop(source);
        server.close().await?;
    }
    for (case, error) in [
        (Case::Epoch, ReadError::EpochChanged),
        (Case::Gap, ReadError::SequenceGap),
        (Case::ForeignScope, ReadError::OwnerMismatch),
        (Case::Conflict, ReadError::ConflictingSignal),
    ] {
        let server = Running::peer(case).await?;
        let source = server.source(InvalidationOwner::Auth)?;
        let after = OwnerCursor::new(InvalidationOwner::Auth, Uuid::new_v4(), 0)?;
        assert_eq!(
            source
                .read_after(consumer, after, 2, &budget())
                .await
                .unwrap_err(),
            error
        );
        drop(source);
        server.close().await?;
    }
    Ok(())
}

#[tokio::test]
#[ignore = "requires invalidation_clients_smoke.py owned TLS fixture"]
async fn original_budget_includes_readiness_decode_validation_and_cancellation(
) -> anyhow::Result<()> {
    let consumer = Uuid::new_v4();
    for case in [Case::SlowBody, Case::SlowHandler] {
        let server = Running::peer(case).await?;
        let source = server.source(InvalidationOwner::Auth)?;
        let b =
            ReplayBudget::for_duration(Duration::from_millis(70), ReplayCancellation::default())?;
        let start = Instant::now();
        assert_eq!(
            source.read_head(consumer, &b).await.unwrap_err(),
            ReadError::DeadlineExceeded
        );
        assert!(start.elapsed() < Duration::from_millis(300));
        assert_eq!(
            source.read_head(consumer, &b).await.unwrap_err(),
            ReadError::DeadlineExceeded
        );
        let cancel = ReplayCancellation::default();
        let b = ReplayBudget::for_duration(MAX_OPERATION, cancel.clone())?;
        let pending = source.read_head(consumer, &b);
        let trigger = async {
            tokio::time::sleep(Duration::from_millis(25)).await;
            cancel.cancel();
        };
        let start = Instant::now();
        let (result, _) = tokio::join!(pending, trigger);
        assert_eq!(result.unwrap_err(), ReadError::Cancelled);
        assert!(start.elapsed() < Duration::from_millis(300));
        let b = budget();
        let parent = Instant::now() + Duration::from_millis(40);
        let start = Instant::now();
        let result = platform::call_deadline::scope(parent, source.read_head(consumer, &b)).await;
        assert_eq!(result.unwrap_err(), ReadError::DeadlineExceeded);
        assert!(start.elapsed() < Duration::from_millis(300));
        // Dropping the caller future must stop the in-flight observation too.
        let cloned = source.clone();
        let pending = tokio::spawn(async move { cloned.read_head(consumer, &budget()).await });
        tokio::time::sleep(Duration::from_millis(25)).await;
        pending.abort();
        assert!(pending.await.unwrap_err().is_cancelled());
        drop(source);
        server.close().await?;
    }
    // A socket accepts TCP but never completes TLS: generated channel readiness
    // must consume the same original budget, before protobuf decoding exists.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("https://{}", listener.local_addr()?);
    let source = InvalidationSource::new(
        InvalidationOwner::Auth,
        &endpoint,
        "private-fixture-concert",
        &tls(Workload::Concert)?,
    )?;
    let held = tokio::spawn(async move {
        let (socket, _) = listener.accept().await.unwrap();
        tokio::time::sleep(Duration::from_secs(2)).await;
        drop(socket);
    });
    let short =
        ReplayBudget::for_duration(Duration::from_millis(60), ReplayCancellation::default())?;
    assert_eq!(
        source.read_head(consumer, &short).await.unwrap_err(),
        ReadError::DeadlineExceeded
    );
    held.abort();
    let _ = held.await;
    Ok(())
}
