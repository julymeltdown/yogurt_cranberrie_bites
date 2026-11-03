//! Guarded Gateway context -> real Concert owner clients. Controlled wire cases
//! are separate from the explicitly ignored two-owner PostgreSQL qualification.
use chrono::Utc;
use concert_domain::{
    relationship::{Field, PolicyQuery, RelationshipError},
    AuthUserId, EventId,
};
use concert_service::{
    adapters::out::relationship_authority::{
        grpc::{proto as wire, GrpcRelationshipAuthority},
        *,
    },
    rpc::{self, proto},
};
use http_body_util::{BodyExt, Full};
use platform::{
    actor_delegation::{ActorDelegationSigner, ActorDelegationVerifier, HEADER},
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::{guard, MethodPolicy},
};
use prost::Message;
use std::{
    sync::{Arc, Mutex},
    time::Duration,
};
use tonic::{Request, Response, Status};
use tower::ServiceExt;
use uuid::Uuid;
const BOOTSTRAP: &str = "/concert.v1.ConcertService/Bootstrap";
type Outcome = (
    RequestBinding,
    Result<BoundedObservation, RelationshipError>,
);
#[derive(Clone)]
struct Probe {
    client: Arc<GrpcRelationshipAuthority>,
    query: PolicyQuery,
    outcome: Arc<Mutex<Option<Outcome>>>,
}
#[tonic::async_trait]
impl proto::concert_service_server::ConcertService for Probe {
    async fn bootstrap(
        &self,
        request: Request<proto::BootstrapRequest>,
    ) -> Result<Response<proto::BootstrapResponse>, Status> {
        let authority = rpc::authorize_bootstrap(&request)?;
        let binding = RequestBinding::from_verified(&authority, self.query)
            .map_err(|_| Status::permission_denied("Binding refused"))?;
        let observed = self.client.observe(&binding).await;
        *self.outcome.lock().unwrap() = Some((binding, observed));
        Ok(Response::new(proto::BootstrapResponse {
            bootstrap_json: b"{}".to_vec(),
        }))
    }
}
async fn observe(client: &GrpcRelationshipAuthority, query: PolicyQuery, family: Uuid) -> Outcome {
    let at = Utc::now().timestamp_micros();
    let m = proto::BootstrapRequest {
        user_id: query.actor.uuid().to_string(),
        event_id: query.event.uuid().to_string(),
        session_family_id: family.to_string(),
        request_id: Uuid::new_v4().to_string(),
        authority_expires_unix_micros: at + 1_500_000,
        auth_observed_at_unix_micros: at,
        profile_observed_at_unix_micros: at,
        display_name: "Public name".into(),
        bio: None,
        http_method: "GET".into(),
        http_target: format!("/api/concert/events/{}/bootstrap", query.event.uuid()),
    };
    let outcome = Arc::new(Mutex::new(None));
    let signer = ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32]).unwrap();
    let service = guard(
        &rpc::POLICY,
        proto::concert_service_server::ConcertServiceServer::new(Probe {
            client: Arc::new(client.clone()),
            query,
            outcome: outcome.clone(),
        }),
        CallerRegistry::new()
            .register(Workload::Gateway, "gateway")
            .unwrap(),
        ActorDelegationVerifier::new(Workload::Concert, &[signer.public_key()]).unwrap(),
    )
    .unwrap();
    let bytes = m.encode_to_vec();
    let mut frame = vec![0];
    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    frame.extend(bytes);
    let proof = signer.issue(
        Workload::Concert,
        BOOTSTRAP,
        query.actor.uuid(),
        None,
        &frame,
        Utc::now().timestamp(),
    );
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(BOOTSTRAP)
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header("x-service-caller", "gateway")
        .header("x-service-token", "gateway")
        .header(HEADER, proof)
        .body(tonic::body::Body::new(Full::new(
            tonic::codegen::Bytes::from(frame),
        )))
        .unwrap();
    let resp = platform::call_deadline::scope(
        std::time::Instant::now() + Duration::from_millis(1400),
        service.oneshot(req),
    )
    .await
    .unwrap();
    let headers = resp.headers().clone();
    let body = resp.into_body().collect().await.unwrap();
    assert_eq!(
        headers
            .get("grpc-status")
            .or_else(|| body.trailers().and_then(|v| v.get("grpc-status")))
            .unwrap(),
        "0"
    );
    let result = outcome
        .lock()
        .unwrap()
        .take()
        .expect("probe invoked behind real guard");
    result
}
fn query(field: Field) -> PolicyQuery {
    PolicyQuery {
        event: EventId::from_uuid(Uuid::new_v4()),
        actor: AuthUserId::from_uuid(Uuid::new_v4()),
        subject: AuthUserId::from_uuid(Uuid::new_v4()),
        field,
    }
}
fn signer() -> ActorDelegationSigner {
    ActorDelegationSigner::from_seed(Workload::Concert, &[37; 32]).unwrap()
}
fn dedicated(audience: Workload) -> ActorDelegationVerifier {
    platform::actor_delegation::relationship_read::RelationshipReadVerifier::new(
        audience,
        &[signer().public_key()],
        &ActorDelegationVerifier::new(
            audience,
            &[
                ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32])
                    .unwrap()
                    .public_key(),
            ],
        )
        .unwrap(),
    )
    .unwrap()
    .into_inner()
}
fn client(auth: &str, friend: &str) -> GrpcRelationshipAuthority {
    GrpcRelationshipAuthority::connect(
        auth,
        friend,
        "concert",
        None,
        platform::actor_delegation::relationship_read::RelationshipReadSigner::from_seed(
            &[37; 32],
            &ActorDelegationVerifier::new(
                Workload::Concert,
                &[
                    ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32])
                        .unwrap()
                        .public_key(),
                ],
            )
            .unwrap(),
        )
        .unwrap(),
    )
    .unwrap()
}
const AUTH_POLICY: MethodPolicy = MethodPolicy {
    service: "relationship.v1.AuthConcertRelationshipService",
    workload: Some(Workload::Auth),
    methods: &[("ReadCurrent", &[Workload::Concert])],
    actor_fields: &[("ReadCurrent", &[&[1]])],
};
const FRIEND_POLICY: MethodPolicy = MethodPolicy {
    service: "relationship.v1.FriendConcertRelationshipService",
    workload: Some(Workload::Friend),
    methods: &[("ReadCurrent", &[Workload::Concert])],
    actor_fields: &[("ReadCurrent", &[&[1]])],
};
struct Running {
    endpoint: String,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Running {
    fn drop(&mut self) {
        self.task.abort();
    }
}
#[derive(Clone)]
struct Controlled(&'static str);
fn metadata<T>(value: T, kind: &str) -> Response<T> {
    let mut r = Response::new(value);
    if kind != "cache" {
        r.metadata_mut()
            .insert("cache-control", "no-store".parse().unwrap());
    }
    r
}
fn controlled(q: &mut wire::RelationshipRead, kind: &str) -> (i64, i64, String, i64) {
    let now = Utc::now().timestamp_micros();
    let mut at = now;
    let mut until = q.expires_unix_micros;
    let mut epoch = Uuid::from_u128(37).to_string();
    let mut rev = 7;
    if kind == "query" {
        q.subject_id = Uuid::new_v4().to_string();
    }
    if kind == "epoch" {
        epoch = Uuid::nil().to_string();
    }
    if kind == "revision" {
        rev = 0;
    }
    if kind == "expired" {
        until = now - 1;
    }
    if kind == "future" {
        at = now + 3_000_000;
    }
    if kind == "extended" {
        until += 3_000_000;
    }
    (at, until, epoch, rev)
}
#[tonic::async_trait]
impl wire::auth_concert_relationship_service_server::AuthConcertRelationshipService for Controlled {
    async fn read_current(
        &self,
        r: Request<wire::RelationshipRead>,
    ) -> Result<Response<wire::AuthRelationshipObservation>, Status> {
        if self.0 == "outage" {
            return Err(Status::unavailable("owner unavailable"));
        }
        if self.0 == "delay" {
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        let mut q = r.into_inner();
        let (at, until, epoch, rev) = controlled(&mut q, self.0);
        Ok(metadata(
            wire::AuthRelationshipObservation {
                query: Some(q),
                observed_at_unix_micros: at,
                valid_until_unix_micros: until,
                owner_epoch: epoch,
                revision: rev,
                actor_active: self.0 != "actor",
                session_live: self.0 != "session",
                subject_active: self.0 != "subject",
            },
            self.0,
        ))
    }
}
#[tonic::async_trait]
impl wire::friend_concert_relationship_service_server::FriendConcertRelationshipService
    for Controlled
{
    async fn read_current(
        &self,
        r: Request<wire::RelationshipRead>,
    ) -> Result<Response<wire::FriendRelationshipObservation>, Status> {
        if self.0 == "outage" {
            return Err(Status::unavailable("owner unavailable"));
        }
        let mut q = r.into_inner();
        let (at, until, epoch, rev) = controlled(&mut q, self.0);
        Ok(metadata(
            wire::FriendRelationshipObservation {
                query: Some(q),
                observed_at_unix_micros: at,
                valid_until_unix_micros: until,
                owner_epoch: epoch,
                revision: if rev == 0 { 0 } else { rev + 11 },
                actor_blocks_subject: self.0 == "forward",
                subject_blocks_actor: self.0 == "reverse",
                actor_deleted: self.0 == "actor-deleted",
                subject_deleted: self.0 == "subject-deleted",
            },
            self.0,
        ))
    }
}
async fn controlled_auth(kind: &'static str) -> Running {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let service = guard(
        &AUTH_POLICY,
        wire::auth_concert_relationship_service_server::AuthConcertRelationshipServiceServer::new(
            Controlled(kind),
        ),
        CallerRegistry::new()
            .register(Workload::Concert, "concert")
            .unwrap(),
        dedicated(Workload::Auth),
    )
    .unwrap();
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(CallDeadlineLayer)
            .add_service(service)
            .serve_with_incoming(
                tonic::codegen::tokio_stream::wrappers::TcpListenerStream::new(listener),
            )
            .await
            .unwrap();
    });
    Running { endpoint, task }
}
async fn controlled_friend(kind: &'static str) -> Running {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let service=guard(&FRIEND_POLICY,wire::friend_concert_relationship_service_server::FriendConcertRelationshipServiceServer::new(Controlled(kind)),CallerRegistry::new().register(Workload::Concert,"concert").unwrap(),dedicated(Workload::Friend)).unwrap();
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(CallDeadlineLayer)
            .add_service(service)
            .serve_with_incoming(
                tonic::codegen::tokio_stream::wrappers::TcpListenerStream::new(listener),
            )
            .await
            .unwrap();
    });
    Running { endpoint, task }
}
#[tokio::test]
async fn signed_two_owner_reads_preserve_vector_binding_and_projection_limits() {
    let auth = controlled_auth("valid").await;
    let friend = controlled_friend("valid").await;
    let client = client(&auth.endpoint, &friend.endpoint);
    let q = query(Field::Presence);
    let family = Uuid::new_v4();
    let (binding, result) = observe(&client, q, family).await;
    let observed = result.unwrap();
    let v = observed.revision();
    assert_eq!(v.auth.revision, 7);
    assert_eq!(v.friend.revision, 18);
    assert_eq!(observed.query(), q);
    assert!(observed
        .permits_public_projection(&binding, v, Utc::now())
        .is_ok());
    for changed in [
        RevisionVector {
            auth: OwnerRevision {
                revision: 8,
                ..v.auth
            },
            ..v
        },
        RevisionVector {
            friend: OwnerRevision {
                revision: 19,
                ..v.friend
            },
            ..v
        },
        RevisionVector {
            auth: OwnerRevision {
                epoch: Uuid::new_v4(),
                ..v.auth
            },
            ..v
        },
        RevisionVector {
            friend: OwnerRevision {
                epoch: Uuid::new_v4(),
                ..v.friend
            },
            ..v
        },
    ] {
        assert_eq!(
            observed.ensure_current(&binding, changed, Utc::now()),
            Err(RelationshipError::Stale)
        );
    }
    let (other, _) = observe(&client, q, family).await;
    assert_eq!(
        observed.ensure_current(&other, v, Utc::now()),
        Err(RelationshipError::Stale)
    );
    assert_eq!(
        observed.ensure_current(&binding, v, observed.valid_until()),
        Err(RelationshipError::Stale)
    );
    for field in [
        Field::PrivateHandle,
        Field::Interaction,
        Field::SafetyInteraction,
    ] {
        let (b, r) = observe(&client, PolicyQuery { field, ..q }, family).await;
        let o = r.unwrap();
        assert_eq!(
            o.permits_public_projection(&b, o.revision(), Utc::now()),
            Err(RelationshipError::Denied)
        );
    }
    let diagnostic = format!("{:?} {:?}", observed, binding);
    assert!(!diagnostic.contains(&q.actor.uuid().to_string()));
    assert!(!diagnostic.contains(&family.to_string()));
}
#[tokio::test]
async fn malformed_outage_and_late_owner_replies_fail_closed() {
    for side in ["auth", "friend"] {
        for kind in [
            "query", "epoch", "revision", "expired", "future", "extended", "cache", "outage",
        ] {
            let auth = controlled_auth(if side == "auth" { kind } else { "valid" }).await;
            let friend = controlled_friend(if side == "friend" { kind } else { "valid" }).await;
            let (_, r) = observe(
                &client(&auth.endpoint, &friend.endpoint),
                query(Field::Presence),
                Uuid::new_v4(),
            )
            .await;
            assert!(r.is_err(), "{side} {kind}");
        }
    }
    let auth = controlled_auth("delay").await;
    let friend = controlled_friend("valid").await;
    let start = std::time::Instant::now();
    let (_, r) = observe(
        &client(&auth.endpoint, &friend.endpoint),
        query(Field::Presence),
        Uuid::new_v4(),
    )
    .await;
    assert!(r.is_err());
    assert!(start.elapsed() < Duration::from_millis(1800));
}
#[tokio::test]
async fn every_owner_restriction_blocks_projection() {
    for (side, kind) in [
        ("auth", "actor"),
        ("auth", "session"),
        ("auth", "subject"),
        ("friend", "forward"),
        ("friend", "reverse"),
        ("friend", "actor-deleted"),
        ("friend", "subject-deleted"),
    ] {
        let auth = controlled_auth(if side == "auth" { kind } else { "valid" }).await;
        let friend = controlled_friend(if side == "friend" { kind } else { "valid" }).await;
        let (b, r) = observe(
            &client(&auth.endpoint, &friend.endpoint),
            query(Field::PublicProfile),
            Uuid::new_v4(),
        )
        .await;
        let o = r.unwrap();
        assert_eq!(
            o.permits_public_projection(&b, o.revision(), Utc::now()),
            Err(RelationshipError::Denied),
            "{kind}"
        );
    }
}

async fn owned_pool(owner: &str) -> sqlx::PgPool {
    use std::{os::unix::fs::MetadataExt, path::PathBuf, str::FromStr};
    let root = PathBuf::from(
        std::env::var("MCR_RELATIONSHIP_ROOT").expect("private harness root required"),
    );
    assert_eq!(root.canonicalize().unwrap(), root);
    assert!(root.starts_with("/private/tmp") || root.starts_with("/tmp"));
    let m = std::fs::symlink_metadata(&root).unwrap();
    assert!(m.is_dir());
    assert_eq!(m.mode() & 0o077, 0);
    assert_eq!(
        format!("{}:{}", m.dev(), m.ino()),
        std::env::var("MCR_RELATIONSHIP_IDENTITY").unwrap()
    );
    let path = root.join("mcr-owner.json");
    let mm = std::fs::symlink_metadata(&path).unwrap();
    assert!(mm.is_file());
    assert_eq!(mm.uid(), m.uid());
    assert_eq!(mm.mode() & 0o077, 0);
    let marker: serde_json::Value = serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap();
    assert_eq!(marker["uid"].as_u64(), Some(u64::from(m.uid())));
    assert_eq!(
        marker["token"].as_str(),
        Some(std::env::var("MCR_RELATIONSHIP_TOKEN").unwrap().as_str())
    );
    for name in ["data", "socket"] {
        let p = root.join(name);
        assert!(std::fs::symlink_metadata(&p).unwrap().is_dir());
        assert_eq!(p.canonicalize().unwrap(), p);
    }
    let options = sqlx::postgres::PgConnectOptions::from_str(
        &std::env::var(format!("MCR_RELATIONSHIP_{}_URL", owner.to_uppercase())).unwrap(),
    )
    .unwrap();
    assert_eq!(options.get_username(), "mcr_owner");
    assert_eq!(
        options.get_database(),
        Some(format!("mcr_{owner}_relationship").as_str())
    );
    assert_eq!(options.get_socket(), Some(&root.join("socket")));
    assert_eq!(options.get_port(), 5432);
    assert!(options.get_options().is_none());
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await
        .unwrap();
    let row:(String,String,String,String,String,i32)=sqlx::query_as("select current_database(),current_user,current_setting('data_directory'),current_setting('unix_socket_directories'),current_setting('listen_addresses'),current_setting('server_version_num')::int").fetch_one(&pool).await.unwrap();
    assert_eq!(
        row,
        (
            format!("mcr_{owner}_relationship"),
            "mcr_owner".into(),
            root.join("data").to_str().unwrap().into(),
            root.join("socket").to_str().unwrap().into(),
            "".into(),
            160014
        )
    );
    pool
}
async fn native_auth(pool: sqlx::PgPool) -> Running {
    use auth_service::adapters::r#in::grpc::concert_relationship as owner;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let service=guard(&owner::POLICY,owner::proto::auth_concert_relationship_service_server::AuthConcertRelationshipServiceServer::new(owner::RelationshipServer::from_pool(pool)),CallerRegistry::new().register(Workload::Concert,"concert").unwrap(),dedicated(Workload::Auth)).unwrap();
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(CallDeadlineLayer)
            .add_service(service)
            .serve_with_incoming(
                tonic::codegen::tokio_stream::wrappers::TcpListenerStream::new(listener),
            )
            .await
            .unwrap();
    });
    Running { endpoint, task }
}
async fn native_friend(pool: sqlx::PgPool) -> Running {
    use friend_service::adapters::r#in::grpc::concert_relationship as owner;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let service=guard(&owner::POLICY,owner::proto::friend_concert_relationship_service_server::FriendConcertRelationshipServiceServer::new(owner::RelationshipServer::from_pool(pool)),CallerRegistry::new().register(Workload::Concert,"concert").unwrap(),dedicated(Workload::Friend)).unwrap();
    let task = tokio::spawn(async move {
        tonic::transport::Server::builder()
            .layer(CallDeadlineLayer)
            .add_service(service)
            .serve_with_incoming(
                tonic::codegen::tokio_stream::wrappers::TcpListenerStream::new(listener),
            )
            .await
            .unwrap();
    });
    Running { endpoint, task }
}
#[tokio::test]
#[ignore = "owned owner_relationship_smoke.py two-database cluster required"]
async fn native_two_owner_client_rechecks_revocation_and_block_generations() {
    let auth_pool = owned_pool("auth").await;
    let friend_pool = owned_pool("friend").await;
    let auth = native_auth(auth_pool.clone()).await;
    let friend = native_friend(friend_pool.clone()).await;
    let client = client(&auth.endpoint, &friend.endpoint);
    let q = query(Field::Presence);
    let actor = q.actor.uuid();
    let subject = q.subject.uuid();
    let family = Uuid::new_v4();
    for id in [actor, subject] {
        sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'private-canary','ACTIVE')").bind(id).bind(format!("{id}@native.invalid")).execute(&auth_pool).await.unwrap();
    }
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,'private-canary',$2,$3,clock_timestamp()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(actor).bind(family).execute(&auth_pool).await.unwrap();
    let (binding, r) = observe(&client, q, family).await;
    let first = r.unwrap();
    assert!(first
        .permits_public_projection(&binding, first.revision(), Utc::now())
        .is_ok());
    sqlx::query(
        "insert into user_blocks(blocker_id,blocked_id,created_at) values($1,$2,clock_timestamp())",
    )
    .bind(subject)
    .bind(actor)
    .execute(&friend_pool)
    .await
    .unwrap();
    let (b, r) = observe(&client, q, family).await;
    let blocked = r.unwrap();
    assert!(blocked.inherited_block());
    assert_eq!(blocked.revision().auth, first.revision().auth);
    assert!(blocked.revision().friend.revision > first.revision().friend.revision);
    assert_eq!(
        first.ensure_current(&binding, blocked.revision(), Utc::now()),
        Err(RelationshipError::Stale)
    );
    assert_eq!(
        blocked.permits_public_projection(&b, blocked.revision(), Utc::now()),
        Err(RelationshipError::Denied)
    );
    sqlx::query(
        "update user_blocks set deleted_at=clock_timestamp() where blocker_id=$1 and blocked_id=$2",
    )
    .bind(subject)
    .bind(actor)
    .execute(&friend_pool)
    .await
    .unwrap();
    let (b, r) = observe(&client, q, family).await;
    let unblocked = r.unwrap();
    assert!(unblocked
        .permits_public_projection(&b, unblocked.revision(), Utc::now())
        .is_ok());
    assert!(unblocked.revision().friend.revision > blocked.revision().friend.revision);
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(subject)
        .execute(&auth_pool)
        .await
        .unwrap();
    let (b, r) = observe(&client, q, family).await;
    let restricted = r.unwrap();
    assert!(!restricted.subject_allowed());
    assert!(restricted.revision().auth.revision > unblocked.revision().auth.revision);
    assert_eq!(
        restricted.permits_public_projection(&b, restricted.revision(), Utc::now()),
        Err(RelationshipError::Denied)
    );
    sqlx::query("update users set status='ACTIVE' where id=$1")
        .bind(subject)
        .execute(&auth_pool)
        .await
        .unwrap();
    sqlx::query(
        "update refresh_tokens set revoked_at=clock_timestamp() where user_id=$1 and family_id=$2",
    )
    .bind(actor)
    .bind(family)
    .execute(&auth_pool)
    .await
    .unwrap();
    let (b, r) = observe(&client, q, family).await;
    let revoked = r.unwrap();
    assert!(!revoked.actor_allowed());
    assert_eq!(
        revoked.permits_public_projection(&b, revoked.revision(), Utc::now()),
        Err(RelationshipError::Denied)
    );
    sqlx::query("insert into friend_user_deletion_tombstones(user_id,saga_id,actor_id,retention_until) values($1,$2,$1,clock_timestamp()+interval '30 days')").bind(subject).bind(Uuid::new_v4()).execute(&friend_pool).await.unwrap();
    let (_, r) = observe(&client, q, family).await;
    assert!(!r.unwrap().subject_allowed());
    drop(auth);
    drop(friend);
    auth_pool.close().await;
    friend_pool.close().await;
}
