//! Native Concert coordinator half only. Profile prepared receipts and owner
//! eligibility are explicit fixtures; this does not accept real cross-store flow.
use super::{coordinator::*, decision_rpc::*, *};
use crate::participant::{proto, Operation, POLICY};
use concert_domain::Revision;
use http_body_util::{BodyExt, Full};
use platform::{
    actor_delegation::{
        owner_binding::OwnerBinding, ActorDelegationSigner, ActorDelegationVerifier, HEADER,
    },
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard,
};
use prost::Message;
use sqlx::{
    postgres::{PgConnectOptions, PgPoolOptions},
    PgPool,
};
use std::{
    os::unix::fs::MetadataExt,
    path::PathBuf,
    str::FromStr,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};
use tonic::{Request, Response, Status};
use tower::ServiceExt;
use uuid::Uuid;

struct FixtureAuthority {
    pool: PgPool,
    revoke_after_effect: bool,
    tracked: AtomicUsize,
}
#[async_trait::async_trait]
impl CoordinatorAuthority for FixtureAuthority {
    async fn check(&self, request: &VerifiedParticipantRequest) -> Result<(), AvatarError> {
        request.check().map_err(|_| AvatarError::Unauthenticated)?;
        if self.revoke_after_effect {
            let called: bool =
                sqlx::query_scalar("select is_called from public.participant_live_effect_probe")
                    .fetch_one(&self.pool)
                    .await
                    .map_err(|_| AvatarError::Unavailable)?;
            if called {
                return Err(AvatarError::Denied);
            }
        }
        Ok(())
    }
    async fn track_concert_transaction(
        &self,
        _: &VerifiedParticipantRequest,
        native: &NativeTransaction,
        _: [u8; 32],
    ) -> Result<(), AvatarError> {
        let actual: (i64, chrono::DateTime<chrono::Utc>) = sqlx::query_as(
            "select count(*) over(),backend_start from pg_stat_activity where pid=$1",
        )
        .bind(native.backend_pid())
        .fetch_one(&self.pool)
        .await
        .map_err(|_| AvatarError::Unavailable)?;
        if actual != (1, native.backend_started()) {
            return Err(AvatarError::Denied);
        }
        let outcome: Option<String> = sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
            .bind(native.full_xid())
            .fetch_one(&self.pool)
            .await
            .map_err(|_| AvatarError::Unavailable)?;
        if outcome.as_deref() != Some("in progress") {
            return Err(AvatarError::Denied);
        }
        self.tracked.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}
#[derive(Clone)]
struct Captured {
    query: proto::ParticipantDecisionRequest,
    native: NativeTransaction,
}
struct Entry {
    pool: PgPool,
    authority: Arc<FixtureAuthority>,
    captured: Arc<Mutex<Option<Captured>>>,
    prepared_calls: Arc<AtomicUsize>,
}
#[tonic::async_trait]
impl proto::concert_participant_service_server::ConcertParticipantService for Entry {
    async fn get_participant(
        &self,
        _: Request<proto::ParticipantRequest>,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        Err(Status::unimplemented("Coordinator fixture only"))
    }
    async fn set_avatar(
        &self,
        _: Request<proto::ParticipantRequest>,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        Err(Status::unimplemented("Coordinator fixture only"))
    }
    async fn update_preferences(
        &self,
        request: Request<proto::ParticipantRequest>,
    ) -> Result<Response<proto::ParticipantResponse>, Status> {
        let context = VerifiedParticipantRequest::verify(&request, Operation::UpdatePreferences)?;
        let parent = context.parent_authority()?;
        let binding = OwnerBinding {
            actor_id: context.key().user_id.uuid(),
            event_id: context.key().event_id.uuid(),
            session_family_id: context.session_family_id(),
            request_id: context.request_id(),
            operation: "updateConcertParticipant".into(),
            expires_unix_micros: context.deadline().expires_micros(),
            parent_digest: parent.parent_digest(),
            parent_body_digest: parent.request_digest(),
            parent_method: parent.request().method.into(),
            http_method: context.http_method().into(),
            http_target: context.http_target().into(),
            body_digest: context.body_sha256(),
        };
        binding.matches_parent(parent)?;
        let digest = binding.digest()?;
        let journal = ParticipantProfileJournal::for_fixture(self.pool.clone());
        let mut tx = journal
            .begin(
                &context,
                digest,
                CanonicalProfileVersion {
                    epoch: Uuid::from_u128(17),
                    revision: Revision::initial(),
                },
                self.authority.as_ref(),
            )
            .await
            .map_err(crate::participant::status)?;
        let effect = tx.expected_profile().clone();
        *self.captured.lock().unwrap() = Some(Captured {
            native: tx.native_transaction().clone(),
            query: proto::ParticipantDecisionRequest {
                actor_id: context.key().user_id.uuid().to_string(),
                event_id: context.key().event_id.uuid().to_string(),
                session_family_id: context.session_family_id().to_string(),
                operation_id: context.request_id().to_string(),
                parent_digest: parent.parent_digest().to_vec(),
                body_digest: context.body_sha256().to_vec(),
                guard_binding_digest: digest.to_vec(),
                profile_prepared_gid: effect.gid().into(),
                profile_effect_digest: effect.digest().to_vec(),
                authority_expires_unix_micros: 0,
            },
        });
        tx.lock_participant(&context, self.authority.as_ref())
            .await
            .map_err(crate::participant::status)?;
        // Explicitly synthetic Profile receipt. Only the Concert CAS/journal
        // path is qualified here; no actual Profile producer was invoked.
        tx.mark_prepare_attempt(&context, self.authority.as_ref())
            .await
            .map_err(crate::participant::status)?;
        self.prepared_calls.fetch_add(1, Ordering::SeqCst);
        let next = tx
            .commit_after_prepared(
                &context,
                self.authority.as_ref(),
                effect.gid(),
                effect.digest(),
            )
            .await
            .map_err(crate::participant::status)?;
        assert_eq!(
            journal.ensure_visible(&context, Some(next)).await,
            Err(AvatarError::Unavailable),
            "Concert commit alone must not expose a partially committed Profile"
        );
        Ok(Response::new(proto::ParticipantResponse {
            response_json: next.get().to_string().into_bytes(),
        }))
    }
}
fn framed<M: Message>(message: &M) -> Vec<u8> {
    let bytes = message.encode_to_vec();
    let mut body = vec![0];
    body.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    body.extend(bytes);
    body
}
fn signed<M: Message>(
    message: &M,
    signer: &ActorDelegationSigner,
    method: &str,
    actor: Uuid,
    caller: &str,
    token: &str,
) -> axum::http::Request<tonic::body::Body> {
    let body = framed(message);
    let signed = signer.issue(
        Workload::Concert,
        method,
        actor,
        None,
        &body,
        chrono::Utc::now().timestamp(),
    );
    axum::http::Request::builder()
        .method("POST")
        .uri(method)
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header("x-service-caller", caller)
        .header("x-service-token", token)
        .header(HEADER, signed)
        .body(tonic::body::Body::new(Full::new(
            tonic::codegen::Bytes::from(body),
        )))
        .unwrap()
}
async fn collected(response: axum::http::Response<tonic::body::Body>) -> (tonic::Code, Vec<u8>) {
    let headers = response.headers().clone();
    let body = response.into_body().collect().await.unwrap();
    let code = tonic::Code::from_i32(
        headers
            .get("grpc-status")
            .or_else(|| {
                body.trailers()
                    .and_then(|trailers| trailers.get("grpc-status"))
            })
            .unwrap()
            .to_str()
            .unwrap()
            .parse()
            .unwrap(),
    );
    (code, body.to_bytes().to_vec())
}
async fn owned_pool() -> anyhow::Result<PgPool> {
    let root = PathBuf::from(std::env::var("MCR_PARTICIPANT_COORDINATOR_ROOT")?);
    let info = std::fs::symlink_metadata(&root)?;
    anyhow::ensure!(
        root.is_absolute()
            && root.canonicalize()? == root
            && info.is_dir()
            && info.mode() & 0o077 == 0,
        "owned private root required"
    );
    anyhow::ensure!(
        format!("{}:{}", info.dev(), info.ino())
            == std::env::var("MCR_PARTICIPANT_COORDINATOR_IDENTITY")?,
        "root inode mismatch"
    );
    let marker = root.join("mcr-owner.json");
    let mi = std::fs::symlink_metadata(&marker)?;
    anyhow::ensure!(
        mi.is_file()
            && !mi.file_type().is_symlink()
            && mi.mode() & 0o077 == 0
            && mi.uid() == info.uid(),
        "private marker required"
    );
    let value: serde_json::Value = serde_json::from_slice(&std::fs::read(marker)?)?;
    anyhow::ensure!(
        value["uid"].as_u64() == Some(info.uid() as u64)
            && value["token"].as_str()
                == Some(std::env::var("MCR_PARTICIPANT_COORDINATOR_TOKEN")?.as_str()),
        "fixture owner mismatch"
    );
    for name in ["data", "socket"] {
        let child = root.join(name);
        let ci = std::fs::symlink_metadata(&child)?;
        anyhow::ensure!(
            ci.is_dir() && !ci.file_type().is_symlink() && child.canonicalize()? == child,
            "owned child required"
        );
    }
    let options = PgConnectOptions::from_str(&std::env::var("MCR_PARTICIPANT_COORDINATOR_URL")?)?;
    anyhow::ensure!(
        options.get_database() == Some("mcr_participant_coordinator_sql")
            && options.get_username() == "mcr_owner"
            && options.get_socket() == Some(&root.join("socket"))
            && options.get_options().is_none()
            && options.get_port() == 5432,
        "foreign DB refused"
    );
    let pool = PgPoolOptions::new()
        .max_connections(6)
        .connect_with(options)
        .await?;
    let actual:(String,String,String,String,i32)=sqlx::query_as("select current_database(),current_user,current_setting('data_directory'),current_setting('listen_addresses'),current_setting('server_version_num')::int").fetch_one(&pool).await?;
    anyhow::ensure!(
        actual
            == (
                "mcr_participant_coordinator_sql".into(),
                "mcr_owner".into(),
                root.join("data").to_string_lossy().into(),
                "".into(),
                160014
            ),
        "foreign server refused"
    );
    Ok(pool)
}

#[tokio::test]
#[ignore = "requires marker/inode-owned private PG16.14 fixture with migration 0027"]
async fn native_guarded_coordinator_cas_rollback_and_decision_recovery() -> anyhow::Result<()> {
    let pool = owned_pool().await?;
    sqlx::raw_sql("create sequence public.participant_live_effect_probe; create function public.participant_live_effect_probe_fn() returns trigger language plpgsql as $$begin perform nextval('public.participant_live_effect_probe');return NEW;end$$; create trigger participant_live_effect_probe after update on public.concert_participants for each row execute function public.participant_live_effect_probe_fn();").execute(&pool).await?;
    for mode in ["commit", "stale", "rollback"] {
        let event = Uuid::new_v4();
        let user = Uuid::new_v4();
        sqlx::query("insert into public.concert_events(event_id,slug,title,phase)values($1,$2,'Coordinator native fixture','OPEN')").bind(event).bind(format!("coordinator-{}",event.simple())).execute(&pool).await?;
        sqlx::query(
        "insert into public.concert_event_admissions(event_id,user_id,state)values($1,$2,'ACTIVE')",
    )
    .bind(event)
    .bind(user)
    .execute(&pool)
    .await?;
        sqlx::query(
        "insert into public.concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled)values($1,$2,'REVENGE',false,false,true)",
    )
    .bind(event)
    .bind(user)
    .execute(&pool)
    .await?;
        sqlx::query("select setval('public.participant_live_effect_probe',1,false)")
            .execute(&pool)
            .await?;
        let expected = if mode == "stale" {
            99
        } else {
            sqlx::query_scalar::<_, i64>(
                "select revision from public.concert_participants where event_id=$1 and user_id=$2",
            )
            .bind(event)
            .bind(user)
            .fetch_one(&pool)
            .await?
        };
        let old_preferences:(String,bool)=sqlx::query_as("select theme,low_power from public.concert_participants where event_id=$1 and user_id=$2").bind(event).bind(user).fetch_one(&pool).await?;
        let authority = Arc::new(FixtureAuthority {
            pool: pool.clone(),
            revoke_after_effect: mode == "rollback",
            tracked: AtomicUsize::new(0),
        });
        let captured = Arc::new(Mutex::new(None));
        let prepared_calls = Arc::new(AtomicUsize::new(0));
        let gateway = ActorDelegationSigner::from_seed(Workload::Gateway, &[61; 32])?;
        let service = guard(
            &POLICY,
            proto::concert_participant_service_server::ConcertParticipantServiceServer::new(
                Entry {
                    pool: pool.clone(),
                    authority: authority.clone(),
                    captured: captured.clone(),
                    prepared_calls: prepared_calls.clone(),
                },
            ),
            CallerRegistry::new().register(Workload::Gateway, "native-gateway")?,
            ActorDelegationVerifier::new(Workload::Concert, &[gateway.public_key()])?,
        )?;
        let now = chrono::Utc::now().timestamp_micros();
        let message = proto::ParticipantRequest {
            user_id: user.to_string(),
            event_id: event.to_string(),
            session_family_id: Uuid::new_v4().to_string(),
            request_id: Uuid::new_v4().to_string(),
            http_method: "PATCH".into(),
            http_target: format!("/api/concert/events/{event}/me"),
            request_json: serde_json::to_vec(
                &serde_json::json!({"expected_revision":expected,"display_name":"Native proposed profile"}),
            )?,
            release_digest: "a".repeat(64),
            authority_expires_unix_micros: now + 1_800_000,
            auth_observed_at_unix_micros: now,
            profile_observed_at_unix_micros: now,
        };
        let response = platform::call_deadline::scope(
            std::time::Instant::now() + Duration::from_millis(1800),
            service.oneshot(signed(
                &message,
                &gateway,
                "/concert.participant.v1.ConcertParticipantService/UpdatePreferences",
                user,
                "gateway",
                "native-gateway",
            )),
        )
        .await?;
        let (code, _) = collected(response.map(tonic::body::Body::new)).await;
        assert_eq!(
            code,
            match mode {
                "commit" => tonic::Code::Ok,
                "stale" => tonic::Code::Aborted,
                _ => tonic::Code::PermissionDenied,
            },
            "{mode}"
        );
        assert_eq!(authority.tracked.load(Ordering::SeqCst), 1);
        assert_eq!(
            prepared_calls.load(Ordering::SeqCst),
            usize::from(mode != "stale")
        );
        let captured = captured
            .lock()
            .unwrap()
            .clone()
            .expect("registered actual transaction");
        let actual_status: Option<String> =
            sqlx::query_scalar("select pg_xact_status($1::text::xid8)")
                .bind(captured.native.full_xid())
                .fetch_one(&pool)
                .await?;
        assert_eq!(
            actual_status.as_deref(),
            Some(if mode == "commit" {
                "committed"
            } else {
                "aborted"
            })
        );
        let preferences:(String,bool)=sqlx::query_as("select theme,low_power from public.concert_participants where event_id=$1 and user_id=$2").bind(event).bind(user).fetch_one(&pool).await?;
        assert_eq!(
            preferences, old_preferences,
            "profile-only preserves preferences"
        );
        let revision: i64 = sqlx::query_scalar(
            "select revision from public.concert_participants where event_id=$1 and user_id=$2",
        )
        .bind(event)
        .bind(user)
        .fetch_one(&pool)
        .await?;
        if mode == "commit" {
            assert_eq!(revision, expected + 1);
        } else if mode == "rollback" {
            assert_eq!(revision, expected);
            let called: bool =
                sqlx::query_scalar("select is_called from public.participant_live_effect_probe")
                    .fetch_one(&pool)
                    .await?;
            assert!(called, "real UPDATE happened before rollback");
        }
        for foreign in [false, true] {
            let mut query = captured.query.clone();
            if foreign {
                query.body_digest = vec![99; 32];
            }
            query.authority_expires_unix_micros = chrono::Utc::now().timestamp_micros() + 1_800_000;
            let profile = ActorDelegationSigner::from_seed(Workload::Profile, &[62; 32])?;
            let service = guard(
                &DECISION_POLICY,
                ParticipantDecisionService::new(pool.clone()).server(),
                CallerRegistry::new().register(Workload::Profile, "native-profile")?,
                ActorDelegationVerifier::new(Workload::Concert, &[profile.public_key()])?,
            )?;
            let response = platform::call_deadline::scope(
                std::time::Instant::now() + Duration::from_millis(1800),
                service.oneshot(signed(
                    &query,
                    &profile,
                    "/concert.participant.v1.ConcertParticipantDecisionService/ResolveDecision",
                    user,
                    "profile",
                    "native-profile",
                )),
            )
            .await?;
            let (code, bytes) = collected(response.map(tonic::body::Body::new)).await;
            assert_eq!(code, tonic::Code::Ok);
            let reply = proto::ParticipantDecisionResponse::decode(&bytes[5..])?;
            assert_eq!(reply.query, Some(query));
            assert_eq!(
                reply.decision,
                if foreign {
                    proto::ParticipantDecision::Unresolved as i32
                } else if mode == "commit" {
                    proto::ParticipantDecision::Commit as i32
                } else {
                    proto::ParticipantDecision::Abort as i32
                }
            );
            assert_eq!(
                reply.committed_revision,
                if !foreign && mode == "commit" {
                    expected + 1
                } else {
                    0
                }
            );
        }
    }
    pool.close().await;
    Ok(())
}
