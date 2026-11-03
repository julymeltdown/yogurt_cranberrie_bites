//! Configured Gateway owner chain and signed Concert dispatch over loopback RPC.
use super::*;
use api_gateway::adapters::out::concert_client::{proto, BootstrapError, GrpcConcertClient};
use uuid::Uuid;
#[derive(Clone, Copy)]
enum Reply {
    Valid,
    Foreign,
    Cacheable,
    Hold,
}
#[derive(Clone)]
struct Concert {
    seen: Arc<Mutex<Vec<proto::BootstrapRequest>>>,
    reply: Arc<Mutex<Reply>>,
}
static POLICY: MethodPolicy = MethodPolicy {
    service: "concert.v1.ConcertService",
    workload: Some(Workload::Concert),
    methods: &[("Bootstrap", &[Workload::Gateway])],
    actor_fields: &[("Bootstrap", &[&[1]])],
};
#[tonic::async_trait]
impl proto::concert_service_server::ConcertService for Concert {
    async fn bootstrap(
        &self,
        request: Request<proto::BootstrapRequest>,
    ) -> Result<Response<proto::BootstrapResponse>, Status> {
        assert!(matches!(
            request.extensions().get::<VerifiedCaller>(),
            Some(VerifiedCaller(Workload::Gateway))
        ));
        let actor = request.extensions().get::<VerifiedActor>().unwrap().0;
        let proof = *request.extensions().get::<VerifiedReadAuthority>().unwrap();
        let deadline = proof.deadline(Duration::from_secs(2))?;
        let m = request.into_inner();
        assert_eq!(actor.to_string(), m.user_id);
        assert_eq!(m.session_family_id, FAMILY);
        assert_eq!(m.http_method, "GET");
        assert_eq!(
            m.http_target,
            format!("/api/concert/events/{}/bootstrap", m.event_id)
        );
        assert_eq!(m.display_name, "Mira");
        assert_eq!(m.bio, None);
        assert!(m.authority_expires_unix_micros > Utc::now().timestamp_micros());
        assert!(m.authority_expires_unix_micros - m.auth_observed_at_unix_micros <= 2_000_000);
        self.seen.lock().unwrap().push(m.clone());
        let mode = *self.reply.lock().unwrap();
        if matches!(mode, Reply::Hold) {
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
        deadline.remaining(Duration::from_secs(2), m.authority_expires_unix_micros)?;
        let mut json: Value = serde_json::from_str(include_str!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../../contracts/fixtures/core/bootstrap-open.json"
        )))
        .unwrap();
        json["event"]["event_id"] = m.event_id.clone().into();
        json["participant"]["event_id"] = m.event_id.into();
        json["participant"]["user_id"] = if matches!(mode, Reply::Foreign) {
            OTHER
        } else {
            USER
        }
        .into();
        json["participant"]["profile"]["user_id"] = USER.into();
        let mut response = Response::new(proto::BootstrapResponse {
            bootstrap_json: serde_json::to_vec(&json).unwrap(),
        });
        if !matches!(mode, Reply::Cacheable) {
            response
                .metadata_mut()
                .insert("cache-control", "no-store".parse().unwrap());
        }
        Ok(response)
    }
}
struct RunningConcert {
    service: Concert,
    client: GrpcConcertClient,
    stop: oneshot::Sender<()>,
    task: JoinHandle<()>,
}
impl RunningConcert {
    async fn start() -> Self {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let endpoint = format!("http://{}", listener.local_addr().unwrap());
        let service = Concert {
            seen: Arc::new(Mutex::new(vec![])),
            reply: Arc::new(Mutex::new(Reply::Valid)),
        };
        let guarded = guard(
            &POLICY,
            proto::concert_service_server::ConcertServiceServer::new(service.clone()),
            public_registry(),
            ActorDelegationVerifier::local_parity(Workload::Concert),
        )
        .unwrap();
        let (stop, stopped) = oneshot::channel();
        let task = tokio::spawn(async move {
            Server::builder()
                .layer(CallDeadlineLayer)
                .add_service(guarded)
                .serve_with_incoming_shutdown(
                    tokio_stream::wrappers::TcpListenerStream::new(listener),
                    async {
                        let _ = stopped.await;
                    },
                )
                .await
                .unwrap();
        });
        let client = GrpcConcertClient::connect(
            &endpoint,
            "composition-token",
            None,
            ActorDelegationSigner::local_parity(Workload::Gateway),
        )
        .unwrap();
        Self {
            service,
            client,
            stop,
            task,
        }
    }
    async fn close(self) {
        let _ = self.stop.send(());
        self.task.await.unwrap();
    }
}
#[tokio::test]
async fn bootstrap_fresh_owner_reads_produce_one_signed_bound_concert_request() {
    let _ = key(); // Key generation is fixture setup, not owner-observation latency.
    let owners = Running::start().await;
    let concert = RunningConcert::start().await;
    let ctx = context(Duration::from_secs(4));
    let event = Uuid::new_v4();
    let request = Uuid::new_v4();
    let result = concert
        .client
        .bootstrap(&owners.reader, &ctx, event, request)
        .await
        .unwrap();
    assert_eq!(result["participant"]["user_id"], USER);
    assert_eq!(result["event"]["event_id"], event.to_string());
    let seen = concert.service.seen.lock().unwrap().clone();
    assert_eq!(seen.len(), 1);
    assert_eq!(seen[0].request_id, request.to_string());
    assert_eq!(owners.auth.calls.load(Ordering::SeqCst), 1);
    assert_eq!(owners.profile.calls.load(Ordering::SeqCst), 1);
    assert_eq!(owners.auth.seen.lock().unwrap()[0].binding, None);
    assert!(!format!("{seen:?}").contains(PRIVATE));
    concert.close().await;
    owners.close().await;
}
#[tokio::test]
async fn bootstrap_auth_refusal_suppresses_both_profile_and_concert() {
    let _ = key();
    let owners = Running::start().await;
    let concert = RunningConcert::start().await;
    let mut refused = admission();
    refused.account_active = false;
    *owners.auth.mode.lock().unwrap() = Mode::Reply(refused);
    let error = concert
        .client
        .bootstrap(
            &owners.reader,
            &context(Duration::from_secs(4)),
            Uuid::new_v4(),
            Uuid::new_v4(),
        )
        .await
        .unwrap_err();
    assert_eq!(error, BootstrapError::Denied);
    assert_eq!(owners.profile.calls.load(Ordering::SeqCst), 0);
    assert_eq!(concert.service.seen.lock().unwrap().len(), 0);
    concert.close().await;
    owners.close().await;
}
#[tokio::test]
async fn bootstrap_rejects_foreign_or_cacheable_results_and_preserves_original_deadline() {
    let _ = key();
    let owners = Running::start().await;
    let concert = RunningConcert::start().await;
    for mode in [Reply::Foreign, Reply::Cacheable] {
        *concert.service.reply.lock().unwrap() = mode;
        assert_eq!(
            concert
                .client
                .bootstrap(
                    &owners.reader,
                    &context(Duration::from_secs(4)),
                    Uuid::new_v4(),
                    Uuid::new_v4()
                )
                .await
                .unwrap_err(),
            BootstrapError::InvalidResponse
        );
    }
    *concert.service.reply.lock().unwrap() = Reply::Hold;
    let started = std::time::Instant::now();
    assert_eq!(
        concert
            .client
            .bootstrap(
                &owners.reader,
                &context(Duration::from_millis(100)),
                Uuid::new_v4(),
                Uuid::new_v4()
            )
            .await
            .unwrap_err(),
        BootstrapError::Expired
    );
    assert!(started.elapsed() < Duration::from_millis(500));
    assert_eq!(concert.service.seen.lock().unwrap().len(), 3); // No automatic replay.
    concert.close().await;
    owners.close().await;
}
