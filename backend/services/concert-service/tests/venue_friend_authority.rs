//! Real Concert, Auth and Friend owner servers over private PostgreSQL. The
//! Friend observation is the production signed ReadCurrent handler; the test
//! exercises the same authority the Concert product composition uses.

use chrono::{Duration, Utc};
use concert_application::venue_location::{VenueLocation, VenueLocationPolicyProvider};
use concert_application::venue_meeting::VenueMeetingAuthority;
use concert_domain::relationship::{Field, PolicyQuery};
use concert_domain::venue_location::{
    LocationSnapshot, PhoneFixInput, SharingCommand, VenueLocationError, VenueLocationPolicy,
    VisibleSource,
};
use concert_domain::venue_meeting::MeetingError;
use concert_domain::{AuthUserId, EventId, ParticipantKey};
use concert_service::adapters::out::relationship_authority::{
    grpc::GrpcRelationshipAuthority, BoundedRelationshipAuthority, RequestBinding,
};
use concert_service::rpc::{self, proto};
use concert_service::venue_authority::VenueFriendAuthority;
use concert_service::venue_location::{
    PgVenueLocation, PgVenueLocationAuthority, VenueLocationAuthority,
};
use http_body_util::{BodyExt, Full};
use platform::{
    actor_delegation::{
        relationship_read::RelationshipReadSigner, ActorDelegationSigner, ActorDelegationVerifier,
        HEADER,
    },
    call_deadline::CallDeadlineLayer,
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::{guard, MethodPolicy},
};
use prost::Message;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{
    str::FromStr,
    sync::{Arc, Mutex},
    time::Duration as StdDuration,
};
use tonic::{Request, Response, Status};
use tower::ServiceExt;
use uuid::Uuid;

const BOOTSTRAP: &str = "/concert.v1.ConcertService/Bootstrap";
const FRIEND_POLICY: MethodPolicy = MethodPolicy {
    service: "relationship.v1.FriendConcertRelationshipService",
    workload: Some(Workload::Friend),
    methods: &[("ReadCurrent", &[Workload::Concert])],
    actor_fields: &[("ReadCurrent", &[&[1]])],
};
const AUTH_POLICY: MethodPolicy = MethodPolicy {
    service: "relationship.v1.AuthConcertRelationshipService",
    workload: Some(Workload::Auth),
    methods: &[("ReadCurrent", &[Workload::Concert])],
    actor_fields: &[("ReadCurrent", &[&[1]])],
};

fn gateway_signer() -> ActorDelegationSigner {
    ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32]).unwrap()
}

fn relationship_signer() -> RelationshipReadSigner {
    RelationshipReadSigner::from_seed(
        &[37; 32],
        &ActorDelegationVerifier::new(Workload::Concert, &[gateway_signer().public_key()]).unwrap(),
    )
    .unwrap()
}

fn dedicated(audience: Workload) -> ActorDelegationVerifier {
    platform::actor_delegation::relationship_read::RelationshipReadVerifier::new(
        audience,
        &[
            ActorDelegationSigner::from_seed(Workload::Concert, &[37; 32])
                .unwrap()
                .public_key(),
        ],
        &ActorDelegationVerifier::new(audience, &[gateway_signer().public_key()]).unwrap(),
    )
    .unwrap()
    .into_inner()
}

#[derive(Clone)]
struct Probe {
    query: PolicyQuery,
    outcome: Arc<Mutex<Option<RequestBinding>>>,
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
        *self.outcome.lock().unwrap() = Some(binding);
        Ok(Response::new(proto::BootstrapResponse {
            bootstrap_json: b"{}".to_vec(),
        }))
    }
}

async fn binding_for(
    actor: Uuid,
    event: Uuid,
    subject: Uuid,
    field: Field,
    family: Uuid,
) -> RequestBinding {
    let outcome = Arc::new(Mutex::new(None));
    let service = guard(
        &rpc::POLICY,
        proto::concert_service_server::ConcertServiceServer::new(Probe {
            query: PolicyQuery {
                event: EventId::from_uuid(event),
                actor: AuthUserId::from_uuid(actor),
                subject: AuthUserId::from_uuid(subject),
                field,
            },
            outcome: outcome.clone(),
        }),
        CallerRegistry::new()
            .register(Workload::Gateway, "gateway")
            .unwrap(),
        ActorDelegationVerifier::new(Workload::Concert, &[gateway_signer().public_key()]).unwrap(),
    )
    .unwrap();
    let now = Utc::now().timestamp_micros();
    let request = proto::BootstrapRequest {
        user_id: actor.to_string(),
        event_id: event.to_string(),
        session_family_id: family.to_string(),
        request_id: Uuid::new_v4().to_string(),
        authority_expires_unix_micros: now + 1_500_000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
        display_name: "Venue friend".into(),
        bio: None,
        http_method: "GET".into(),
        http_target: format!("/api/concert/events/{event}/bootstrap"),
    };
    let bytes = request.encode_to_vec();
    let mut frame = vec![0];
    frame.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    frame.extend(bytes);
    let proof = gateway_signer().issue(
        Workload::Concert,
        BOOTSTRAP,
        actor,
        None,
        &frame,
        Utc::now().timestamp(),
    );
    let request = axum::http::Request::builder()
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
    let response = platform::call_deadline::scope(
        std::time::Instant::now() + StdDuration::from_millis(1400),
        service.oneshot(request),
    )
    .await
    .unwrap();
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap();
    let status = headers
        .get("grpc-status")
        .or_else(|| body.trailers().and_then(|value| value.get("grpc-status")))
        .unwrap()
        .to_str()
        .unwrap();
    assert_eq!(status, "0");
    let binding = outcome
        .lock()
        .unwrap()
        .take()
        .expect("guarded bootstrap returned a signed binding");
    binding
}

struct Running {
    endpoint: String,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for Running {
    fn drop(&mut self) {
        self.task.abort();
    }
}

async fn start_auth(pool: PgPool) -> Running {
    use auth_service::adapters::r#in::grpc::concert_relationship as owner;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let service = guard(
        &AUTH_POLICY,
        owner::proto::auth_concert_relationship_service_server::AuthConcertRelationshipServiceServer::new(
            owner::RelationshipServer::from_pool(pool),
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

async fn start_friend(pool: PgPool) -> Running {
    use friend_service::adapters::r#in::grpc::concert_relationship as owner;
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let endpoint = format!("http://{}", listener.local_addr().unwrap());
    let service = guard(
        &FRIEND_POLICY,
        owner::proto::friend_concert_relationship_service_server::FriendConcertRelationshipServiceServer::new(
            owner::RelationshipServer::from_pool(pool),
        ),
        CallerRegistry::new()
            .register(Workload::Concert, "concert")
            .unwrap(),
        dedicated(Workload::Friend),
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

async fn owned_pool(variable: &str, database: &str) -> PgPool {
    let value = std::env::var(variable).unwrap_or_else(|_| panic!("{variable} is required"));
    let options = PgConnectOptions::from_str(&value).expect("valid private database URL");
    assert_eq!(options.get_username(), "mcr_owner");
    assert_eq!(options.get_database(), Some(database));
    PgPoolOptions::new()
        .max_connections(8)
        .connect_with(options)
        .await
        .expect("private database")
}

struct Fixture {
    concert: PgPool,
    friend: PgPool,
    auth: PgPool,
    event: Uuid,
    owner: Uuid,
    viewer: Uuid,
    stranger: Uuid,
    owner_family: Uuid,
    viewer_family: Uuid,
    stranger_family: Uuid,
    auth_endpoint: String,
    friend_endpoint: String,
    client: GrpcRelationshipAuthority,
    _auth_server: Running,
    _friend_server: Running,
}

impl Fixture {
    fn key(&self, user: Uuid) -> ParticipantKey {
        ParticipantKey {
            event_id: EventId::from_uuid(self.event),
            user_id: AuthUserId::from_uuid(user),
        }
    }
}

async fn fixture() -> Fixture {
    let concert = owned_pool(
        "MCR_VENUE_FRIEND_CONCERT_DATABASE_URL",
        "mcr_concert_venue_friend",
    )
    .await;
    let friend = owned_pool(
        "MCR_VENUE_FRIEND_FRIEND_DATABASE_URL",
        "mcr_friend_venue_friend",
    )
    .await;
    let auth = owned_pool(
        "MCR_VENUE_FRIEND_AUTH_DATABASE_URL",
        "mcr_auth_venue_friend",
    )
    .await;
    let event = Uuid::new_v4();
    let owner = Uuid::new_v4();
    let viewer = Uuid::new_v4();
    let stranger = Uuid::new_v4();
    let owner_family = Uuid::new_v4();
    let viewer_family = Uuid::new_v4();
    let stranger_family = Uuid::new_v4();
    sqlx::query(
        "insert into public.concert_events(event_id,slug,title,phase)
         values($1,$2,'Venue friend authority','OPEN')",
    )
    .bind(event)
    .bind(format!("venue-friend-{}", event.simple()))
    .execute(&concert)
    .await
    .unwrap();
    for user in [owner, viewer, stranger] {
        sqlx::query(
            "insert into public.concert_participants
             (event_id,user_id,theme,low_power,reduced_motion,sound_enabled)
             values($1,$2,'REVENGE',false,false,false)",
        )
        .bind(event)
        .bind(user)
        .execute(&concert)
        .await
        .unwrap();
    }
    for (user, family) in [
        (owner, owner_family),
        (viewer, viewer_family),
        (stranger, stranger_family),
    ] {
        sqlx::query(
            "insert into public.users(id,primary_email,password_hash,status)
             values($1,$2,'venue-friend-test','ACTIVE')",
        )
        .bind(user)
        .bind(format!("{user}@venue-friend.invalid"))
        .execute(&auth)
        .await
        .unwrap();
        sqlx::query(
            "insert into public.refresh_tokens(jti,token_hash,user_id,family_id,expires_at)
             values($1,'venue-friend-test',$2,$3,clock_timestamp()+interval '1 hour')",
        )
        .bind(format!("{user}-{}", Uuid::new_v4()))
        .bind(user)
        .bind(family)
        .execute(&auth)
        .await
        .unwrap();
    }
    for (follower, followee) in [(owner, viewer), (viewer, owner)] {
        sqlx::query(
            "insert into public.follows(follower_id,followee_id,created_at)
             values($1,$2,clock_timestamp())",
        )
        .bind(follower)
        .bind(followee)
        .execute(&friend)
        .await
        .unwrap();
    }
    let auth_server = start_auth(auth.clone()).await;
    let friend_server = start_friend(friend.clone()).await;
    let client = GrpcRelationshipAuthority::connect(
        &auth_server.endpoint,
        &friend_server.endpoint,
        "concert",
        None,
        relationship_signer(),
    )
    .unwrap();
    Fixture {
        concert,
        friend,
        auth,
        event,
        owner,
        viewer,
        stranger,
        owner_family,
        viewer_family,
        stranger_family,
        auth_endpoint: auth_server.endpoint.clone(),
        friend_endpoint: friend_server.endpoint.clone(),
        client,
        _auth_server: auth_server,
        _friend_server: friend_server,
    }
}

struct FixturePolicy {
    event: EventId,
}

#[async_trait::async_trait]
impl VenueLocationPolicyProvider for FixturePolicy {
    async fn policy(&self, event: EventId) -> Result<VenueLocationPolicy, VenueLocationError> {
        if event != self.event {
            return Err(VenueLocationError::PolicyUnavailable);
        }
        Ok(VenueLocationPolicy {
            event_id: event,
            venue_key: "venue-friend-authority".into(),
            revision: 1,
            timezone: "Asia/Seoul".into(),
            valid_until: Utc::now() + Duration::hours(1),
            fresh_ms: 30_000,
            last_known_ms: 120_000,
            fix_max_age_ms: 120_000,
            future_skew_ms: 5_000,
            sharing_lease_ms: 60_000,
            request_ttl_ms: 60_000,
            check_in_ttl_ms: 60_000,
            coordinate_retention_ms: 300_000,
            command_retention_ms: 60_000,
            max_accuracy_m: 50.0,
            max_viewers: 10,
        })
    }
}

async fn location_for(
    fixture: &Fixture,
    actor: Uuid,
    subject: Uuid,
    family: Uuid,
) -> PgVenueLocation<FixturePolicy, PgVenueLocationAuthority> {
    let binding = binding_for(actor, fixture.event, subject, Field::Interaction, family).await;
    let relationships: Arc<dyn BoundedRelationshipAuthority> = Arc::new(fixture.client.clone());
    let authority =
        PgVenueLocationAuthority::from_binding(fixture.concert.clone(), relationships, binding);
    PgVenueLocation::new(
        fixture.concert.clone(),
        FixturePolicy {
            event: EventId::from_uuid(fixture.event),
        },
        authority,
    )
}

fn removed_authority_location(
    fixture: &Fixture,
) -> PgVenueLocation<FixturePolicy, PgVenueLocationAuthority> {
    PgVenueLocation::new(
        fixture.concert.clone(),
        FixturePolicy {
            event: EventId::from_uuid(fixture.event),
        },
        PgVenueLocationAuthority::default(),
    )
}

async fn seed_sharing(fixture: &Fixture, owner: Uuid, viewer: Uuid) {
    sqlx::query(
        "insert into public.concert_venue_location_sharing
         (event_id,owner_id,state,revision,expires_at,updated_at)
         values($1,$2,'ACTIVE',1,clock_timestamp()+interval '5 minutes',clock_timestamp())
         on conflict(event_id,owner_id) do update
           set state=excluded.state,revision=excluded.revision,
               expires_at=excluded.expires_at,updated_at=clock_timestamp()",
    )
    .bind(fixture.event)
    .bind(owner)
    .execute(&fixture.concert)
    .await
    .unwrap();
    sqlx::query(
        "insert into public.concert_venue_location_viewers(event_id,owner_id,viewer_id)
         values($1,$2,$3) on conflict do nothing",
    )
    .bind(fixture.event)
    .bind(owner)
    .bind(viewer)
    .execute(&fixture.concert)
    .await
    .unwrap();
    sqlx::query(
        "insert into public.concert_venue_location_fixes
         (event_id,owner_id,latitude,longitude,accuracy_m,captured_at,received_at,sharing_revision)
         values($1,$2,37.5665,126.9780,8.0,clock_timestamp()-interval '1 second',clock_timestamp(),1)
         on conflict(event_id,owner_id) do update
           set latitude=excluded.latitude,longitude=excluded.longitude,
               accuracy_m=excluded.accuracy_m,captured_at=excluded.captured_at,
               received_at=excluded.received_at,sharing_revision=excluded.sharing_revision",
    )
    .bind(fixture.event)
    .bind(owner)
    .execute(&fixture.concert)
    .await
    .unwrap();
}

async fn snapshot(
    fixture: &Fixture,
    actor: Uuid,
    subject: Uuid,
    family: Uuid,
) -> Result<LocationSnapshot, VenueLocationError> {
    let location = location_for(fixture, actor, subject, family).await;
    location.snapshot(fixture.key(actor)).await
}

#[tokio::test]
async fn mutual_friends_allow_location_only_after_explicit_consent() {
    let fixture = fixture().await;
    let before = snapshot(
        &fixture,
        fixture.viewer,
        fixture.owner,
        fixture.viewer_family,
    )
    .await
    .unwrap();
    assert!(before.friends.is_empty());
    let viewer_location = location_for(
        &fixture,
        fixture.viewer,
        fixture.owner,
        fixture.viewer_family,
    )
    .await;
    assert_eq!(
        viewer_location
            .visible_for(
                fixture.key(fixture.viewer),
                AuthUserId::from_uuid(fixture.owner)
            )
            .await,
        Err(VenueLocationError::NotFound)
    );

    let owner_location = location_for(
        &fixture,
        fixture.owner,
        fixture.viewer,
        fixture.owner_family,
    )
    .await;
    let receipt = owner_location
        .set_sharing(
            fixture.key(fixture.owner),
            Uuid::new_v4(),
            SharingCommand::Active {
                expected_revision: 0,
                viewer_ids: vec![fixture.viewer],
            },
        )
        .await
        .unwrap();
    assert_eq!(receipt.revision, 1);
    owner_location
        .publish_fix(
            fixture.key(fixture.owner),
            Uuid::new_v4(),
            PhoneFixInput {
                expected_sharing_revision: 1,
                latitude: 37.5665,
                longitude: 126.9780,
                accuracy_m: 8.0,
                captured_at: Utc::now() - Duration::seconds(1),
            },
        )
        .await
        .unwrap();

    let after = snapshot(
        &fixture,
        fixture.viewer,
        fixture.owner,
        fixture.viewer_family,
    )
    .await
    .unwrap();
    assert_eq!(after.friends.len(), 1);
    assert_eq!(after.friends[0].user_id, fixture.owner);
    assert_eq!(after.friends[0].source, VisibleSource::Phone);
    assert!(!after.friends[0].physical_arrival_verified);
    assert_eq!(after.friends[0].position.latitude, 37.5665);

    let viewer_location = location_for(
        &fixture,
        fixture.viewer,
        fixture.owner,
        fixture.viewer_family,
    )
    .await;
    assert_eq!(
        viewer_location
            .visible_for(
                fixture.key(fixture.viewer),
                AuthUserId::from_uuid(fixture.owner)
            )
            .await
            .unwrap()
            .user_id,
        fixture.owner
    );
    assert_eq!(
        snapshot(
            &fixture,
            fixture.stranger,
            fixture.owner,
            fixture.stranger_family
        )
        .await
        .unwrap()
        .friends
        .len(),
        0
    );
    let stranger_location = location_for(
        &fixture,
        fixture.stranger,
        fixture.owner,
        fixture.stranger_family,
    )
    .await;
    assert_eq!(
        stranger_location
            .visible_for(
                fixture.key(fixture.stranger),
                AuthUserId::from_uuid(fixture.owner)
            )
            .await,
        Err(VenueLocationError::Denied)
    );
    let owner_location = location_for(
        &fixture,
        fixture.owner,
        fixture.stranger,
        fixture.owner_family,
    )
    .await;
    assert_eq!(
        owner_location
            .set_sharing(
                fixture.key(fixture.owner),
                Uuid::new_v4(),
                SharingCommand::Active {
                    expected_revision: 1,
                    viewer_ids: vec![fixture.stranger],
                },
            )
            .await,
        Err(VenueLocationError::Denied)
    );
}

#[tokio::test]
async fn one_way_follow_is_not_friendship() {
    let fixture = fixture().await;
    sqlx::query(
        "update public.follows set deleted_at=clock_timestamp()
         where follower_id=$1 and followee_id=$2",
    )
    .bind(fixture.viewer)
    .bind(fixture.owner)
    .execute(&fixture.friend)
    .await
    .unwrap();
    seed_sharing(&fixture, fixture.owner, fixture.viewer).await;
    assert_eq!(
        snapshot(
            &fixture,
            fixture.viewer,
            fixture.owner,
            fixture.viewer_family
        )
        .await
        .unwrap()
        .friends
        .len(),
        0
    );
    let viewer_location = location_for(
        &fixture,
        fixture.viewer,
        fixture.owner,
        fixture.viewer_family,
    )
    .await;
    assert_eq!(
        viewer_location
            .visible_for(
                fixture.key(fixture.viewer),
                AuthUserId::from_uuid(fixture.owner)
            )
            .await,
        Err(VenueLocationError::Denied)
    );
    let owner_location = location_for(
        &fixture,
        fixture.owner,
        fixture.viewer,
        fixture.owner_family,
    )
    .await;
    assert_eq!(
        owner_location
            .set_sharing(
                fixture.key(fixture.owner),
                Uuid::new_v4(),
                SharingCommand::Active {
                    expected_revision: 1,
                    viewer_ids: vec![fixture.viewer],
                },
            )
            .await,
        Err(VenueLocationError::Denied)
    );
}

#[tokio::test]
async fn blocked_pair_is_denied() {
    let fixture = fixture().await;
    sqlx::query(
        "insert into public.user_blocks(blocker_id,blocked_id,created_at)
         values($1,$2,clock_timestamp())",
    )
    .bind(fixture.owner)
    .bind(fixture.viewer)
    .execute(&fixture.friend)
    .await
    .unwrap();
    seed_sharing(&fixture, fixture.owner, fixture.viewer).await;
    assert_eq!(
        snapshot(
            &fixture,
            fixture.viewer,
            fixture.owner,
            fixture.viewer_family
        )
        .await
        .unwrap()
        .friends
        .len(),
        0
    );
    let viewer_location = location_for(
        &fixture,
        fixture.viewer,
        fixture.owner,
        fixture.viewer_family,
    )
    .await;
    assert_eq!(
        viewer_location
            .visible_for(
                fixture.key(fixture.viewer),
                AuthUserId::from_uuid(fixture.owner)
            )
            .await,
        Err(VenueLocationError::Denied)
    );
    let owner_location = location_for(
        &fixture,
        fixture.owner,
        fixture.viewer,
        fixture.owner_family,
    )
    .await;
    assert_eq!(
        owner_location
            .set_sharing(
                fixture.key(fixture.owner),
                Uuid::new_v4(),
                SharingCommand::Active {
                    expected_revision: 1,
                    viewer_ids: vec![fixture.viewer],
                },
            )
            .await,
        Err(VenueLocationError::Denied)
    );
}

#[tokio::test]
async fn suspended_account_is_denied() {
    let fixture = fixture().await;
    sqlx::query("update public.users set status='SUSPENDED' where id=$1")
        .bind(fixture.viewer)
        .execute(&fixture.auth)
        .await
        .unwrap();
    assert_eq!(
        snapshot(
            &fixture,
            fixture.viewer,
            fixture.owner,
            fixture.viewer_family
        )
        .await,
        Err(VenueLocationError::Denied)
    );
    sqlx::query("update public.users set status='ACTIVE' where id=$1")
        .bind(fixture.viewer)
        .execute(&fixture.auth)
        .await
        .unwrap();
    sqlx::query("update public.users set status='SUSPENDED' where id=$1")
        .bind(fixture.owner)
        .execute(&fixture.auth)
        .await
        .unwrap();
    let owner_location = location_for(
        &fixture,
        fixture.owner,
        fixture.viewer,
        fixture.owner_family,
    )
    .await;
    assert_eq!(
        owner_location
            .set_sharing(
                fixture.key(fixture.owner),
                Uuid::new_v4(),
                SharingCommand::Active {
                    expected_revision: 0,
                    viewer_ids: vec![fixture.viewer],
                },
            )
            .await,
        Err(VenueLocationError::Denied)
    );
}

#[tokio::test]
async fn archived_event_is_denied() {
    let fixture = fixture().await;
    sqlx::query(
        "update public.concert_events set phase='ARCHIVED', revision=revision+1 where event_id=$1",
    )
    .bind(fixture.event)
    .execute(&fixture.concert)
    .await
    .unwrap();
    assert_eq!(
        snapshot(
            &fixture,
            fixture.viewer,
            fixture.owner,
            fixture.viewer_family
        )
        .await,
        Err(VenueLocationError::Denied)
    );
    let owner_location = location_for(
        &fixture,
        fixture.owner,
        fixture.viewer,
        fixture.owner_family,
    )
    .await;
    assert_eq!(
        owner_location
            .set_sharing(
                fixture.key(fixture.owner),
                Uuid::new_v4(),
                SharingCommand::Active {
                    expected_revision: 0,
                    viewer_ids: vec![fixture.viewer],
                },
            )
            .await,
        Err(VenueLocationError::Denied)
    );
}

#[tokio::test]
async fn meeting_friend_gate_uses_the_same_bilateral_authority() {
    let fixture = fixture().await;
    let binding = binding_for(
        fixture.viewer,
        fixture.event,
        fixture.owner,
        Field::Interaction,
        fixture.viewer_family,
    )
    .await;
    let relationships: Arc<dyn BoundedRelationshipAuthority> = Arc::new(fixture.client.clone());
    let authority = VenueFriendAuthority::new(fixture.concert.clone(), relationships, binding);
    let facts = authority
        .friend(fixture.event, fixture.viewer, fixture.owner)
        .await
        .unwrap();
    assert!(facts.relationship_revision > 0);

    sqlx::query(
        "update public.follows set deleted_at=clock_timestamp()
         where follower_id=$1 and followee_id=$2",
    )
    .bind(fixture.owner)
    .bind(fixture.viewer)
    .execute(&fixture.friend)
    .await
    .unwrap();
    let binding = binding_for(
        fixture.viewer,
        fixture.event,
        fixture.owner,
        Field::Interaction,
        fixture.viewer_family,
    )
    .await;
    let relationships: Arc<dyn BoundedRelationshipAuthority> = Arc::new(fixture.client.clone());
    let authority = VenueFriendAuthority::new(fixture.concert.clone(), relationships, binding);
    assert_eq!(
        authority
            .friend(fixture.event, fixture.viewer, fixture.owner)
            .await,
        Err(MeetingError::ResourceUnavailable)
    );
}

#[tokio::test]
async fn friend_owner_outage_fails_closed() {
    let fixture = fixture().await;
    let dead_endpoint = {
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        drop(listener);
        format!("http://{address}")
    };
    assert_ne!(dead_endpoint, fixture.friend_endpoint);
    let dead = GrpcRelationshipAuthority::connect(
        &fixture.auth_endpoint,
        &dead_endpoint,
        "concert",
        None,
        relationship_signer(),
    )
    .unwrap();
    let binding = binding_for(
        fixture.owner,
        fixture.event,
        fixture.viewer,
        Field::Interaction,
        fixture.owner_family,
    )
    .await;
    let relationships: Arc<dyn BoundedRelationshipAuthority> = Arc::new(dead);
    let authority =
        PgVenueLocationAuthority::from_binding(fixture.concert.clone(), relationships, binding);
    let location = PgVenueLocation::new(
        fixture.concert.clone(),
        FixturePolicy {
            event: EventId::from_uuid(fixture.event),
        },
        authority,
    );
    assert_eq!(
        location.snapshot(fixture.key(fixture.owner)).await,
        Err(VenueLocationError::PolicyUnavailable)
    );
    assert_eq!(
        location
            .set_sharing(
                fixture.key(fixture.owner),
                Uuid::new_v4(),
                SharingCommand::Active {
                    expected_revision: 0,
                    viewer_ids: vec![fixture.viewer],
                },
            )
            .await,
        Err(VenueLocationError::PolicyUnavailable)
    );
}

#[tokio::test]
async fn removing_the_friend_authority_flips_the_mutual_friend_outcome() {
    let fixture = fixture().await;
    seed_sharing(&fixture, fixture.owner, fixture.viewer).await;
    let binding = binding_for(
        fixture.viewer,
        fixture.event,
        fixture.owner,
        Field::Interaction,
        fixture.viewer_family,
    )
    .await;
    let relationships: Arc<dyn BoundedRelationshipAuthority> = Arc::new(fixture.client.clone());
    let authority =
        PgVenueLocationAuthority::from_binding(fixture.concert.clone(), relationships, binding);
    let mut connection = fixture.concert.acquire().await.unwrap();
    assert_eq!(
        authority
            .friend_in(
                &mut connection,
                EventId::from_uuid(fixture.event),
                AuthUserId::from_uuid(fixture.viewer),
                AuthUserId::from_uuid(fixture.owner),
                Utc::now(),
            )
            .await,
        Ok(true)
    );
    let removed = PgVenueLocationAuthority::default();
    assert_eq!(
        removed
            .friend_in(
                &mut connection,
                EventId::from_uuid(fixture.event),
                AuthUserId::from_uuid(fixture.viewer),
                AuthUserId::from_uuid(fixture.owner),
                Utc::now(),
            )
            .await,
        Err(VenueLocationError::PolicyUnavailable)
    );
    drop(connection);
    assert_eq!(
        snapshot(
            &fixture,
            fixture.viewer,
            fixture.owner,
            fixture.viewer_family
        )
        .await
        .unwrap()
        .friends
        .len(),
        1
    );
    assert_eq!(
        removed_authority_location(&fixture)
            .snapshot(fixture.key(fixture.viewer))
            .await,
        Err(VenueLocationError::PolicyUnavailable)
    );
}
