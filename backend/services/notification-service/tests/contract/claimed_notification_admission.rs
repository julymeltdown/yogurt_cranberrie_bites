//! Actual generated original Notification RPC over mTLS into real PostgreSQL.
//! Post owner/Friend observations and admission are explicit unit fixtures here;
//! this is destination acceptance, not the future four-service authority flow.
use notification_service::adapters::r#in::grpc::{
    method_policy::NOTIFICATION_METHOD_POLICY,
    server::{claimed_notification_service, proto, NotificationGrpcServer},
};
use platform::actor_delegation::notification::*;
use platform::actor_delegation::{ActorDelegationVerifier, Denial};
use platform::grpc_auth::{CallerRegistry, Workload};
use platform::grpc_tls::TlsMaterial;
use prost::Message;
use std::sync::atomic::{AtomicI64, AtomicU8, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use tokio_stream::wrappers::TcpListenerStream;
use tonic::{Code, Request};
use uuid::Uuid;

struct FixtureAdmission {
    mode: AtomicU8,
    calls: AtomicUsize,
    expires: AtomicI64,
}
impl NotificationAdmissionCheck for FixtureAdmission {
    fn check<'a>(&'a self, _: &'a VerifiedNotification) -> NotificationAdmissionFuture<'a> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            let explicit = self.expires.load(Ordering::SeqCst);
            let expires_micros = if explicit == 0 {
                chrono::Utc::now().timestamp_micros() + 5_000_000
            } else {
                explicit
            };
            match self.mode.load(Ordering::SeqCst) {
                0 => Ok(NotificationAdmission {
                    decision: NotificationDecision::Allowed { visibility: None },
                    expires_micros,
                }),
                1 => Ok(NotificationAdmission {
                    decision: NotificationDecision::Suppressed,
                    expires_micros,
                }),
                _ => Err(Denial::WrongBody),
            }
        })
    }
}
struct Protocol {
    owner: NotificationOwnerSigner,
    friend: NotificationFriendSigner,
    issuer: NotificationIssuer,
}
impl Protocol {
    fn new() -> anyhow::Result<Self> {
        let policy = LifetimePolicy::new(5_000_000).unwrap();
        let owner = NotificationOwnerSigner::new(&[71; 32], policy)?;
        // Construct issuer once to obtain the pinned Gateway public key; only
        // independently generated fixture keys are used by these tests.
        let provisional = NotificationIssuer::new(
            &[72; 32],
            &[owner.public_key()],
            &[owner.public_key()],
            policy,
        )?;
        let friend = NotificationFriendSigner::new(&[73; 32], &[provisional.public_key()], policy)?;
        let issuer = NotificationIssuer::new(
            &[72; 32],
            &[owner.public_key()],
            &[friend.public_key()],
            policy,
        )?;
        Ok(Self {
            owner,
            friend,
            issuer,
        })
    }
    fn delivery() -> NotificationDelivery {
        NotificationDelivery {
            claim: NotificationClaim {
                event_id: Uuid::new_v4(),
                aggregate_id: Uuid::new_v4(),
                event_digest: NotificationClaim::digest(b"stored direct event"),
                attempt_digest: NotificationClaim::digest(b"INTERNAL REPLAY SUCCEEDED count0"),
                lease_digest: NotificationClaim::digest(Uuid::new_v4().as_bytes()),
                attempt_id: Uuid::new_v4(),
                attempt: 0,
                lease_expires_micros: chrono::Utc::now().timestamp_micros() + 60_000_000,
            },
            recipient_id: Uuid::new_v4(),
            actor_id: Uuid::new_v4(),
            post_id: Uuid::new_v4(),
            kind: NotificationKind::LegacyComment,
            occurred_at: "2026-10-14T00:00:00+00:00".into(),
            membership: None,
        }
    }
    fn issue(&self, d: &NotificationDelivery) -> IssuedNotification {
        let now = chrono::Utc::now().timestamp_micros();
        let b = NotificationBinding::Delivery(d.clone());
        let p = self
            .issuer
            .prepare(Workload::Post, b.clone(), b.frame().unwrap(), now)
            .unwrap();
        let owner = self
            .owner
            .attest(p.binding(), p.body(), p.challenge(), None, now, now)
            .unwrap();
        let ready = self.issuer.prepare_policy(p, &owner, now).unwrap();
        let query = self
            .friend
            .verify_policy_query(ready.token(), Workload::Gateway, POLICY_METHOD, now)
            .unwrap();
        let policy = self
            .friend
            .policy(query, false, false, false, now, now)
            .unwrap();
        let final_policy = self
            .issuer
            .prepare_final_owner(ready, &policy, now)
            .unwrap();
        let final_request = final_policy.owner_request();
        let owner = self
            .owner
            .attest(
                final_request.binding(),
                final_request.body(),
                final_request.challenge(),
                None,
                now,
                now,
            )
            .unwrap();
        let NotificationIssue::Granted(g) = self
            .issuer
            .issue_delivery(final_policy, &owner, now)
            .unwrap()
        else {
            panic!("fixture suppression")
        };
        g
    }
}
fn message(d: &NotificationDelivery) -> proto::CreateNotificationRequest {
    let body = d.frame().unwrap();
    let request = proto::CreateNotificationRequest::decode(&body[5..]).unwrap();
    let b = request.encode_to_vec();
    assert_eq!(
        body,
        [vec![0], (b.len() as u32).to_be_bytes().to_vec(), b].concat()
    );
    request
}
fn presented(
    d: &NotificationDelivery,
    grant: Option<&str>,
    workload: Workload,
) -> Request<proto::CreateNotificationRequest> {
    let mut request = Request::new(message(d));
    request
        .metadata_mut()
        .insert("x-service-caller", workload.as_str().parse().unwrap());
    request.metadata_mut().insert(
        "x-service-token",
        if workload == Workload::Post {
            "post-fixture"
        } else {
            "gateway-fixture"
        }
        .parse()
        .unwrap(),
    );
    if let Some(g) = grant {
        request.metadata_mut().insert(HEADER, g.parse().unwrap());
    }
    request
}
fn tls(workload: Workload) -> anyhow::Result<TlsMaterial> {
    let leaf = test_support::tls::issue(workload.as_str())?;
    TlsMaterial::load(leaf.cert, leaf.key, leaf.ca)
}

/// Capture a real guard-produced seal for the direct-handler mutation test.
#[derive(Clone)]
struct Capture<S> {
    inner: S,
    seal: Arc<Mutex<Option<AdmittedNotification>>>,
}
impl<S: tonic::server::NamedService> tonic::server::NamedService for Capture<S> {
    const NAME: &'static str = S::NAME;
}
impl<S> tonic::codegen::Service<tonic::codegen::http::Request<tonic::body::Body>> for Capture<S>
where
    S: tonic::codegen::Service<tonic::codegen::http::Request<tonic::body::Body>>,
{
    type Response = S::Response;
    type Error = S::Error;
    type Future = S::Future;
    fn poll_ready(
        &mut self,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, r: tonic::codegen::http::Request<tonic::body::Body>) -> Self::Future {
        *self.seal.lock().unwrap() = r.extensions().get::<AdmittedNotification>().cloned();
        self.inner.call(r)
    }
}
struct Running {
    stop: Option<tokio::sync::oneshot::Sender<()>>,
    task: Option<tokio::task::JoinHandle<Result<(), tonic::transport::Error>>>,
}
impl Running {
    async fn shutdown(mut self) -> anyhow::Result<()> {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        tokio::time::timeout(std::time::Duration::from_secs(5), self.task.take().unwrap())
            .await???;
        Ok(())
    }
}
impl Drop for Running {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
        if let Some(task) = self.task.take() {
            task.abort();
        }
    }
}

#[tokio::test]
async fn selected_original_rpc_enforces_admission_and_preserves_pg_creation_dedup_and_suppression(
) -> anyhow::Result<()> {
    use proto::notification_service_server::{NotificationService, NotificationServiceServer};
    let app = crate::support::start_postgres_app().await?;
    let p = Protocol::new()?;
    let decision = Arc::new(FixtureAdmission {
        mode: AtomicU8::new(0),
        calls: AtomicUsize::new(0),
        expires: AtomicI64::new(0),
    });
    let seal = Arc::new(Mutex::new(None));
    // Same required-handler/guard composition as the public selected factory,
    // with only an observation wrapper inserted to retain a real sealed value.
    let handler = Capture {
        inner: NotificationServiceServer::new(NotificationGrpcServer::requiring_claimed_admission(
            app.state.clone(),
        )),
        seal: seal.clone(),
    };
    let guarded = NotificationGuard::new(
        handler,
        NotificationVerifier::new(&[p.issuer.public_key()], 64)?,
        decision.clone(),
    )?;
    // Compile the public factory as well; it constructs the same required mode.
    let _selected = claimed_notification_service(
        app.state.clone(),
        NotificationVerifier::new(&[p.issuer.public_key()], 64)?,
        decision.clone(),
    )?;
    let registry = CallerRegistry::new()
        .register(Workload::Post, "post-fixture")?
        .register(Workload::Gateway, "gateway-fixture")?;
    let service = platform::grpc_method_policy::guard(
        &NOTIFICATION_METHOD_POLICY,
        guarded,
        registry,
        ActorDelegationVerifier::local_parity(Workload::Notification),
    )?;
    let notification_tls = tls(Workload::Notification)?;
    let post_tls = tls(Workload::Post)?;
    let gateway_tls = tls(Workload::Gateway)?;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
    let endpoint = format!("https://{}", listener.local_addr()?);
    let (stop, stopped) = tokio::sync::oneshot::channel();
    let task = tokio::spawn(
        tonic::transport::Server::builder()
            .tls_config(notification_tls.server_config()?)?
            .add_service(service)
            .serve_with_incoming_shutdown(TcpListenerStream::new(listener), async {
                let _ = stopped.await;
            }),
    );
    let running = Running {
        stop: Some(stop),
        task: Some(task),
    };
    let mut post = proto::notification_service_client::NotificationServiceClient::new(
        post_tls.connect(&endpoint, Workload::Notification).await?,
    );
    let mut gateway = proto::notification_service_client::NotificationServiceClient::new(
        gateway_tls
            .connect(&endpoint, Workload::Notification)
            .await?,
    );
    let mut d = Protocol::delivery();
    let g = p.issue(&d);
    assert_eq!(
        post.create_notification(presented(&d, None, Workload::Post))
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    // A valid Gateway certificate + credential cannot use the original Post write.
    assert_eq!(
        gateway
            .create_notification(presented(&d, Some(&g.token), Workload::Gateway))
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    // A valid Gateway certificate cannot impersonate Post even with Post metadata.
    assert_eq!(
        gateway
            .create_notification(presented(&d, Some(&g.token), Workload::Post))
            .await
            .unwrap_err()
            .code(),
        Code::Unauthenticated
    );
    assert_eq!(decision.calls.load(Ordering::SeqCst), 0);
    let first = post
        .create_notification(presented(&d, Some(&g.token), Workload::Post))
        .await?
        .into_inner();
    assert!(!first.suppressed);
    let original = first.notification.unwrap();
    assert_eq!(original.event_id, d.claim.event_id.to_string());
    assert_eq!(original.actor_id, d.actor_id.to_string());
    assert_eq!(original.r#type, "COMMENT");
    assert_eq!(original.payload_json, message(&d).payload_json);
    let admitted = seal.lock().unwrap().take().unwrap();
    // A copied genuine seal cannot authorize a modified decoded request.
    let mut tampered = Request::new(message(&d));
    tampered.get_mut().actor_id = Uuid::new_v4().to_string();
    tampered.extensions_mut().insert(admitted);
    let required = NotificationGrpcServer::requiring_claimed_admission(app.state.clone());
    assert_eq!(
        required
            .create_notification(tampered)
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    assert_eq!(
        post.create_notification(presented(&d, Some(&g.token), Workload::Post))
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    // A freshly admitted retry reaches owning dedup and returns the immutable row.
    d.post_id = Uuid::new_v4();
    d.occurred_at = "2026-10-14T01:00:00+00:00".into();
    let retry = p.issue(&d);
    let existing = post
        .create_notification(presented(&d, Some(&retry.token), Workload::Post))
        .await?
        .into_inner()
        .notification
        .unwrap();
    assert_eq!(existing, original);
    let count: i64 = sqlx::query_scalar("select count(*) from notifications")
        .fetch_one(&app.pool)
        .await?;
    assert_eq!(count, 1);
    // Fresh policy suppression returns a successful generated response/trailers
    // without invoking even the destination transaction (zero subject rows).
    decision.mode.store(1, Ordering::SeqCst);
    let suppressed = Protocol::delivery();
    let g = p.issue(&suppressed);
    let result = post
        .create_notification(presented(&suppressed, Some(&g.token), Workload::Post))
        .await?
        .into_inner();
    assert!(result.suppressed);
    assert!(result.notification.is_none());
    let touched: i64 = sqlx::query_scalar(
        "select count(*) from notification_subject_states where subject_id=any($1)",
    )
    .bind(vec![suppressed.actor_id, suppressed.recipient_id])
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(touched, 0);
    // Destination deletion suppression still precedes dedup for an existing row.
    decision.mode.store(0, Ordering::SeqCst);
    sqlx::query("insert into notification_recipient_suppressions(user_id,reason,suppressed_at) values($1,'account_deletion',now())")
        .bind(d.recipient_id).execute(&app.pool).await?;
    let g = p.issue(&d);
    let deleted = post
        .create_notification(presented(&d, Some(&g.token), Workload::Post))
        .await?
        .into_inner();
    assert!(deleted.suppressed);
    assert!(deleted.notification.is_none());
    // Current admission dependency failure is an error, never a no-write ACK.
    decision.mode.store(2, Ordering::SeqCst);
    let unavailable = Protocol::delivery();
    let g = p.issue(&unavailable);
    assert_eq!(
        post.create_notification(presented(&unavailable, Some(&g.token), Workload::Post))
            .await
            .unwrap_err()
            .code(),
        Code::PermissionDenied
    );
    let untouched: i64 = sqlx::query_scalar(
        "select count(*) from notification_subject_states where subject_id=any($1)",
    )
    .bind(vec![unavailable.actor_id, unavailable.recipient_id])
    .fetch_one(&app.pool)
    .await?;
    assert_eq!(untouched, 0);
    // Non-write methods retain the original Gateway actor-delegation requirement.
    let mut read = Request::new(proto::ListNotificationsRequest {
        user_id: d.recipient_id.to_string(),
        cursor: String::new(),
        limit: 10,
    });
    read.metadata_mut()
        .insert("x-service-caller", "gateway".parse()?);
    read.metadata_mut()
        .insert("x-service-token", "gateway-fixture".parse()?);
    assert_eq!(
        gateway.list_notifications(read).await.unwrap_err().code(),
        Code::Unauthenticated
    );
    // Hold an actual owning subject lock until the refreshed authority expires.
    // The original signed grant stays live; handler-entry admission alone would
    // permit a late insert. Observe the blocked database request before release.
    decision.mode.store(0, Ordering::SeqCst);
    for block_actor in [false, true] {
        let blocked = Protocol::delivery();
        let grant = p.issue(&blocked);
        for id in [blocked.recipient_id, blocked.actor_id] {
            sqlx::query(
                "insert into notification_subject_states(subject_id,created_at) values($1,now())",
            )
            .bind(id)
            .execute(&app.pool)
            .await?;
        }
        let subject = if block_actor {
            blocked.actor_id
        } else {
            blocked.recipient_id
        };
        let mut holder = app.pool.begin().await?;
        sqlx::query(
            "select subject_id from notification_subject_states where subject_id=$1 for update",
        )
        .bind(subject)
        .fetch_one(&mut *holder)
        .await?;
        let expires = chrono::Utc::now().timestamp_micros() + 500_000;
        decision.expires.store(expires, Ordering::SeqCst);
        let req = presented(&blocked, Some(&grant.token), Workload::Post);
        let mut client = post.clone();
        let waiting = tokio::spawn(async move { client.create_notification(req).await });
        tokio::time::timeout(std::time::Duration::from_millis(400),async{
            loop {
                let locked:bool=sqlx::query_scalar("select exists(select 1 from pg_stat_activity where datname=current_database() and pid<>pg_backend_pid() and wait_event_type='Lock' and query like '%notification_subject_states%')").fetch_one(&app.pool).await.unwrap();
                if locked {break;}tokio::time::sleep(std::time::Duration::from_millis(5)).await;
            }
        }).await?;
        let remaining = expires
            .saturating_sub(chrono::Utc::now().timestamp_micros())
            .max(0) as u64;
        tokio::time::sleep(std::time::Duration::from_micros(remaining + 20_000)).await;
        assert!(chrono::Utc::now().timestamp_micros() < grant.expires_micros);
        holder.rollback().await?;
        let failure = tokio::time::timeout(std::time::Duration::from_secs(2), waiting)
            .await??
            .unwrap_err();
        assert_eq!(failure.code(), Code::PermissionDenied);
        assert_eq!(failure.message(), "Notification admission expired");
        let committed: i64 =
            sqlx::query_scalar("select count(*) from notifications where event_id=$1")
                .bind(blocked.claim.event_id)
                .fetch_one(&app.pool)
                .await?;
        assert_eq!(committed, 0);
        println!("NOTIFICATION_PERSISTENCE_EXPIRY blocked_role={} original_grant_live=true observed_pg_lock=true committed=0",if block_actor{"actor"}else{"recipient"});
    }
    decision.expires.store(0, Ordering::SeqCst);
    let final_count: i64 = sqlx::query_scalar("select count(*) from notifications")
        .fetch_one(&app.pool)
        .await?;
    assert_eq!(final_count, 1);
    println!("NOTIFICATION_SELECTED_DESTINATION mtls=true real_pg=true original_rpc=true immutable_dedup=true policy_suppression=true deletion_suppression_before_dedup=true copied_seal_denied=true upstream_authority=FIXTURE");
    drop((post, gateway, required, _selected));
    running.shutdown().await?;
    app.shutdown().await?;
    Ok(())
}
