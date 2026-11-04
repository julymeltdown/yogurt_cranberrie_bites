//! No forged JWT, HttpConcertAccess, client address or positive owner scope.
//! A transport gate delays polling the *unchanged* production response body.
use super::{environment, fixture::Fixture};
use api_gateway::{
    adapters::{
        out::concert_product::GrpcConcertProductClient,
        r#in::http::{
            self as gateway_http, client_identity::TrustedProxies, server::with_transport_layers,
        },
    },
    config::{build_state, connect_pool, AppConfig},
};
use axum::{
    body::Body,
    extract::{Request, State},
    middleware::{from_fn_with_state, Next},
    response::Response,
};
use concert_service::{
    adapters::out::relationship_authority::guarded::OwnerGuardScope,
    product_rpc::{
        self, delivery, proto,
        scoped::{ProductRepresentation, ScopedProductOwner, ScopedProductProducer},
        VerifiedProduct,
    },
};
use futures::StreamExt;
use platform::{
    actor_delegation::{response_scope::RetainedScopeLayer, ActorDelegationVerifier},
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::{guard, guard_without_delegation},
};
use sqlx::PgPool;
use std::{io::Read, process::Stdio, sync::Arc, time::Duration};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    sync::{mpsc, oneshot, Semaphore},
};
use tonic::Status;
use uuid::Uuid;

struct Refuse {
    pool: PgPool,
    actor: Uuid,
    scopes: mpsc::UnboundedSender<Arc<OwnerGuardScope>>,
}
#[async_trait::async_trait]
impl ScopedProductProducer for Refuse {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        _: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        if context.actor() != self.actor {
            return Err(Status::permission_denied("wrong actual login principal"));
        }
        let p = scope.producer_scope();
        let published: bool = sqlx::query_scalar("select exists(select 1 from concert_product_owner_scopes where instance_id=$1 and scope_id=$2 and delivery_secret_hash is not null)")
            .bind(Uuid::parse_str(&p.instance_id).map_err(|_| Status::internal("fixture instance"))?)
            .bind(Uuid::parse_str(&p.scope_id).map_err(|_| Status::internal("fixture scope"))?)
            .fetch_one(&self.pool).await.map_err(|_| Status::unavailable("fixture ledger"))?;
        if !published {
            return Err(Status::internal("missing durable delivery"));
        }
        self.scopes
            .send(scope)
            .map_err(|_| Status::unavailable("fixture observer closed"))?;
        Err(Status::permission_denied("PRIVATE_PRODUCER_DIAGNOSTIC"))
    }
}

#[derive(Clone)]
struct Gates(mpsc::UnboundedSender<Gate>);
struct Gate {
    allow: Arc<Semaphore>,
    dropped: oneshot::Receiver<()>,
    status: u16,
}
struct ObserveDrop(Option<oneshot::Sender<()>>);
impl Drop for ObserveDrop {
    fn drop(&mut self) {
        if let Some(sender) = self.0.take() {
            let _ = sender.send(());
        }
    }
}
async fn gate_body(State(gates): State<Gates>, request: Request, next: Next) -> Response {
    let response = next.run(request).await;
    let (parts, inner) = response.into_parts();
    let gate = Arc::new(Semaphore::new(0));
    let (signal, dropped) = oneshot::channel();
    let observe = ObserveDrop(Some(signal));
    let _ = gates.0.send(Gate {
        allow: gate.clone(),
        dropped,
        status: parts.status.as_u16(),
    });
    // Preserve every frame, including errors/trailers. In particular, do not
    // collect the inner body or synthesize its EOF/completion callback.
    let frames = futures::stream::once(async move {
        let _observe = observe;
        gate.acquire_owned()
            .await
            .expect("fixture gate open")
            .forget();
        inner
    })
    .flat_map(http_body_util::BodyStream::new);
    Response::from_parts(parts, Body::new(http_body_util::StreamBody::new(frames)))
}
struct Tasks(Vec<tokio::task::JoinHandle<()>>);
impl Drop for Tasks {
    fn drop(&mut self) {
        for task in &self.0 {
            task.abort();
        }
    }
}

async fn pending(pool: &PgPool) -> anyhow::Result<i64> {
    Ok(sqlx::query_scalar(
        "select count(*) from pg_prepared_xacts where database=current_database()",
    )
    .fetch_one(pool)
    .await?)
}
async fn writer_probe(
    pool: &PgPool,
    actor: Uuid,
    friend: bool,
    blocked: bool,
) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("set local lock_timeout='80ms'")
        .execute(&mut *tx)
        .await?;
    let result = if friend {
        sqlx::query("insert into user_blocks(blocker_id,blocked_id,created_at) values($1,$2,clock_timestamp())")
            .bind(actor).bind(Uuid::new_v4()).execute(&mut *tx).await
    } else {
        sqlx::query("update users set password_hash=password_hash where id=$1")
            .bind(actor)
            .execute(&mut *tx)
            .await
    };
    if blocked {
        anyhow::ensure!(
            result
                .err()
                .and_then(|e| e
                    .as_database_error()
                    .and_then(|d| d.code())
                    .map(|c| c.into_owned()))
                .as_deref()
                == Some("55P03"),
            "actual writer was not excluded"
        );
    } else {
        result?;
    }
    tx.rollback().await?;
    Ok(())
}
async fn request(
    address: std::net::SocketAddr,
    event: Uuid,
    token: &str,
) -> anyhow::Result<tokio::net::TcpStream> {
    let mut stream = tokio::net::TcpStream::connect(address).await?;
    stream.write_all(format!("GET /api/concert/events/{event}/feed HTTP/1.1\r\nHost: {address}\r\nAuthorization: Bearer {token}\r\nConnection: close\r\n\r\n").as_bytes()).await?;
    Ok(stream)
}

async fn disconnect_request(
    address: std::net::SocketAddr,
    event: Uuid,
    token: &str,
) -> anyhow::Result<tokio::process::Child> {
    let mut child = tokio::process::Command::new("/usr/bin/python3")
        .args(["-I", "-c", include_str!("http_disconnect.py")])
        .env_clear()
        .env("PATH", "/usr/bin:/bin")
        .stdin(Stdio::piped())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()?;
    let input = serde_json::json!({"address":address.to_string(), "target":format!("/api/concert/events/{event}/feed"), "bearer":token});
    let pipe = child
        .stdin
        .as_mut()
        .ok_or_else(|| anyhow::anyhow!("fixture pipe missing"))?;
    pipe.write_all(serde_json::to_string(&input)?.as_bytes())
        .await?;
    pipe.write_all(b"\n").await?;
    pipe.flush().await?;
    Ok(child)
}
#[derive(Clone, Copy, PartialEq, Eq)]
enum Terminal {
    Complete,
    Disconnect,
    Expire,
}

#[tokio::test]
#[ignore = "requires frozen owned real-login/PG16 HTTP controller"]
async fn actual_public_router_retains_real_owners_and_checks_before_bytes() -> anyhow::Result<()> {
    // The controller closes this pipe after one real login. Never log or export it.
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let input: serde_json::Value = serde_json::from_str(&input)?;
    let actor = Uuid::parse_str(
        input["user_id"]
            .as_str()
            .ok_or_else(|| anyhow::anyhow!("login actor absent"))?,
    )?;
    let token = input["access_token"]
        .as_str()
        .ok_or_else(|| anyhow::anyhow!("login token absent"))?;
    let mut config = AppConfig::load()?;
    anyhow::ensure!(
        config.concert_grpc_endpoint.is_none(),
        "controller must not attach an existing Concert"
    );
    let f = Fixture::build_for_gateway(false, config.actor_delegation_signer.public_key()).await?;
    let (scopes_tx, mut scopes) = mpsc::unbounded_channel();
    let owner = ScopedProductOwner::new(
        Arc::new(f.fence.clone()),
        f.delivery_publisher(),
        Arc::new(Refuse {
            pool: f.concert.clone(),
            actor,
            scopes: scopes_tx,
        }),
    );
    let callers = CallerRegistry::new().register(Workload::Gateway, &config.outbound_identity)?;
    let product = guard(
        &product_rpc::POLICY,
        proto::product_service_server::ProductServiceServer::new(
            product_rpc::ProductRpcService::new(Arc::new(owner)),
        ),
        callers.clone(),
        ActorDelegationVerifier::new(
            Workload::Concert,
            &[config.actor_delegation_signer.public_key()],
        )?,
    )?;
    let delivery = guard_without_delegation(
        &delivery::POLICY,
        proto::product_delivery_service_server::ProductDeliveryServiceServer::new(
            delivery::DeliveryService::new(f.delivery_publisher()),
        ),
        callers,
    )?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("https://{}", listener.local_addr()?);
    let incoming = futures::stream::unfold(listener, |listener| async {
        Some((listener.accept().await.map(|(s, _)| s), listener))
    });
    let server = tonic::transport::Server::builder()
        .tls_config(environment::tls(Workload::Concert)?.server_config()?)?
        .layer(platform::call_deadline::CallDeadlineLayer)
        .layer(RetainedScopeLayer)
        .add_service(product)
        .add_service(delivery);
    let mut tasks = Tasks(vec![tokio::spawn(async move {
        server
            .serve_with_incoming(incoming)
            .await
            .expect("owned Concert RPC listener");
    })]);
    let pool = connect_pool(&config).await?;
    let mut state = build_state(&config, pool).await?;
    anyhow::ensure!(
        state.concert_identity.is_some(),
        "actual Auth/Profile composition required"
    );
    // Native six-service parity uses unencrypted owner identity RPCs; this
    // explicit real Concert client uses the fixture CA and Gateway certificate.
    // No injected client ports or identity observations are supplied.
    state.concert_product_client = Some(GrpcConcertProductClient::connect(
        &endpoint,
        &config.outbound_identity,
        Some(&environment::tls(Workload::Gateway)?),
        config.actor_delegation_signer.clone(),
    )?);
    if let Some(keys) = &state.signing_keys {
        keys.refresh().await;
    }
    config.concert_grpc_endpoint = Some(endpoint);
    let (gates_tx, mut gates) = mpsc::unbounded_channel();
    let router = with_transport_layers(
        gateway_http::router(Arc::new(state)),
        &config,
        TrustedProxies::default(),
    )
    .layer(from_fn_with_state(Gates(gates_tx), gate_body));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let address = listener.local_addr()?;
    tasks.0.push(tokio::spawn(async move {
        axum::serve(
            listener,
            router.into_make_service_with_connect_info::<std::net::SocketAddr>(),
        )
        .await
        .expect("owned Gateway HTTP listener");
    }));

    let event = Uuid::new_v4();
    let mut quarantined = 0;
    for terminal in [Terminal::Complete, Terminal::Disconnect, Terminal::Expire] {
        let mut reset = if terminal == Terminal::Disconnect {
            Some(disconnect_request(address, event, token).await?)
        } else {
            None
        };
        let mut stream = if reset.is_none() {
            Some(request(address, event, token).await?)
        } else {
            None
        };
        let gate = tokio::time::timeout(Duration::from_secs(4), gates.recv())
            .await?
            .ok_or_else(|| anyhow::anyhow!("actual Gateway response absent"))?;
        // The producer publishes its observation before the response exists.
        // A preceding real auth/router/RPC rejection must report its safe
        // numeric status rather than timing out on an observation that never
        // happened. Do not log the bearer, headers or private response content.
        let scope = scopes.try_recv().map_err(|_| {
            anyhow::anyhow!(
                "real Gateway response status={} arrived before actual producer dispatch",
                gate.status
            )
        })?;
        assert_eq!(
            (pending(&f.auth).await?, pending(&f.friend).await?),
            (quarantined + 1, quarantined + 1)
        );
        writer_probe(&f.auth, actor, false, true).await?;
        writer_probe(&f.friend, actor, true, true).await?;
        if let Some(reset) = reset.as_mut() {
            let pipe = reset
                .stdin
                .as_mut()
                .ok_or_else(|| anyhow::anyhow!("fixture reset pipe missing"))?;
            pipe.write_all(b"DROP\n").await?;
            pipe.flush().await?;
            anyhow::ensure!(
                tokio::time::timeout(Duration::from_secs(2), reset.wait())
                    .await??
                    .success(),
                "actual reset helper failed"
            );
            // The gate was never opened. Its destruction proves the real HTTP
            // server dropped this unpolled response after the actual TCP reset.
            tokio::time::timeout(Duration::from_secs(2), gate.dropped).await??;
            quarantined += 1;
            assert_eq!(
                (pending(&f.auth).await?, pending(&f.friend).await?),
                (quarantined, quarantined)
            );
            writer_probe(&f.auth, actor, false, true).await?;
            writer_probe(&f.friend, actor, true, true).await?;
            continue;
        }
        if terminal == Terminal::Expire {
            let expiry = scope.disclosure_ceiling()?.expires_micros();
            let wait = expiry
                .saturating_sub(chrono::Utc::now().timestamp_micros())
                .max(0) as u64;
            tokio::time::sleep(Duration::from_micros(wait) + Duration::from_millis(10)).await;
        }
        gate.allow.add_permits(1);
        let mut bytes = Vec::new();
        let read = tokio::time::timeout(
            Duration::from_secs(4),
            stream
                .as_mut()
                .expect("fixture HTTP stream")
                .read_to_end(&mut bytes),
        )
        .await?;
        let text = String::from_utf8_lossy(&bytes);
        assert!(!text.contains("PRIVATE_PRODUCER_DIAGNOSTIC"));
        if terminal == Terminal::Expire {
            assert!(
                !text.contains("PERMISSION_DENIED"),
                "expired Product representation escaped the actual HTTP body guard"
            );
            // Both an aborted HTTP body and a clean header-only EOF are bounded
            // negative results. Neither may certify terminal owner settlement.
            let _ = read;
            quarantined += 1;
            assert_eq!(
                (pending(&f.auth).await?, pending(&f.friend).await?),
                (quarantined, quarantined)
            );
            writer_probe(&f.auth, actor, false, true).await?;
            writer_probe(&f.friend, actor, true, true).await?;
        } else {
            read?;
            assert!(text.starts_with("HTTP/1.1 403"));
            assert!(text.contains("PERMISSION_DENIED"));
            assert_eq!((pending(&f.auth).await?, pending(&f.friend).await?), (0, 0));
            writer_probe(&f.auth, actor, false, false).await?;
            writer_probe(&f.friend, actor, true, false).await?;
        }
    }
    f.assert_no_recovery_calls();
    println!("actual login/JWT/current Auth+Profile/full public router/socket normalization/production retained HTTP EOF settles both real owners; actual TCP reset drops the unopened response body and retains both guards; delayed poll past original ceiling discloses no representation and retains both guards. Two scopes remain quarantined for owned-cluster teardown; no positive product, Gift, full TLS identity-main or restart claim");
    Ok(())
}
