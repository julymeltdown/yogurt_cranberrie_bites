//! Generated RPC behind actual workload/signature/body/actor guard. Synthetic
//! authority/store fixtures are explicit; an ignored probe uses native storage.
use async_trait::async_trait;
use concert_application::avatar::*;
use concert_domain::{
    avatar::{config_value, SetAvatar},
    *,
};
use concert_service::participant::{
    context::VerifiedParticipantRequest, ports::*, proto, Operation, ParticipantService, POLICY,
};
use http_body_util::{BodyExt, Full};
use platform::{
    actor_delegation::{ActorDelegationSigner, ActorDelegationVerifier, HEADER},
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard,
};
use prost::Message;
use serde_json::{json, Value};
use std::{
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};
use tonic::Code;
use tower::ServiceExt;
use uuid::Uuid;
const DIGEST: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const EVENT: &str = "00000000-0000-4000-8000-000000000801";
const USER: &str = "00000000-0000-4000-8000-000000000802";
#[derive(Default)]
struct Counters {
    auth: AtomicUsize,
    profile: AtomicUsize,
    permit: AtomicUsize,
    store: AtomicUsize,
}
struct Owners {
    calls: Arc<Counters>,
    fault: &'static str,
}
#[async_trait]
impl CanonicalAuthPort for Owners {
    async fn read(&self, r: &VerifiedParticipantRequest) -> Result<CurrentAuth, AvatarError> {
        self.calls.auth.fetch_add(1, Ordering::SeqCst);
        if self.fault == "auth-denied" {
            return Err(AvatarError::Denied);
        }
        if self.fault == "auth-unavailable" {
            return Err(AvatarError::Unavailable);
        }
        if self.fault == "slow-auth" {
            tokio::time::sleep(Duration::from_millis(120)).await;
        }
        Ok(CurrentAuth {
            user_id: if self.fault == "foreign-auth" {
                Uuid::new_v4()
            } else {
                r.key().user_id.uuid()
            },
            session_family_id: if self.fault == "foreign-session" {
                Uuid::new_v4()
            } else {
                r.session_family_id()
            },
            observed_at_micros: chrono::Utc::now().timestamp_micros(),
            expires_micros: r.deadline().expires_micros(),
        })
    }
}
#[async_trait]
impl CanonicalProfilePort for Owners {
    async fn read(&self, r: &VerifiedParticipantRequest) -> Result<CurrentProfile, AvatarError> {
        self.calls.profile.fetch_add(1, Ordering::SeqCst);
        if self.fault == "profile-unavailable" {
            return Err(AvatarError::Unavailable);
        }
        if self.fault == "blocking-profile" {
            std::thread::sleep(Duration::from_millis(120));
        }
        if self.fault == "slow-profile" {
            tokio::time::sleep(Duration::from_millis(120)).await;
        }
        Ok(CurrentProfile {
            user_id: if self.fault == "foreign-profile" {
                Uuid::new_v4()
            } else {
                r.key().user_id.uuid()
            },
            display_name: if self.fault == "invalid-profile" {
                "bad\nname".into()
            } else {
                "Canonical current name".into()
            },
            bio: Some("Canonical current bio".into()),
            observed_at_micros: if self.fault == "stale-profile" {
                chrono::Utc::now().timestamp_micros() - 3_000_000
            } else {
                chrono::Utc::now().timestamp_micros()
            },
            expires_micros: r.deadline().expires_micros(),
        })
    }
}
struct Permit {
    body: [u8; 32],
    request: Uuid,
    key: ParticipantKey,
    session: Uuid,
    op: Operation,
    digest: String,
    calls: AtomicUsize,
    expire_after: usize,
}
impl MutationPermit for Permit {
    fn check(&self, r: &VerifiedParticipantRequest) -> Result<(), AvatarError> {
        if self.calls.fetch_add(1, Ordering::SeqCst) >= self.expire_after {
            return Err(AvatarError::Denied);
        }
        if self.body != r.body_sha256()
            || self.request != r.request_id()
            || self.key != r.key()
            || self.session != r.session_family_id()
            || self.op != r.operation()
            || self.digest != r.release_digest()
        {
            return Err(AvatarError::Denied);
        }
        r.check().map_err(|_| AvatarError::Unauthenticated)
    }
}
#[async_trait]
impl MutationAuthorityPort for Owners {
    async fn authorize(
        &self,
        r: &VerifiedParticipantRequest,
        _: &CurrentAuth,
    ) -> Result<Box<dyn MutationPermit>, AvatarError> {
        self.calls.permit.fetch_add(1, Ordering::SeqCst);
        if self.fault == "capability-denied" {
            return Err(AvatarError::Denied);
        }
        if self.fault == "capability-unavailable" {
            return Err(AvatarError::Unavailable);
        }
        if self.fault == "slow-capability" {
            tokio::time::sleep(Duration::from_millis(120)).await;
        }
        Ok(Box::new(Permit {
            body: if self.fault == "foreign-body" {
                [0; 32]
            } else {
                r.body_sha256()
            },
            request: r.request_id(),
            key: r.key(),
            session: r.session_family_id(),
            op: r.operation(),
            digest: r.release_digest().into(),
            calls: AtomicUsize::new(0),
            expire_after: if self.fault == "withdraw-permit" {
                2
            } else if self.fault == "withdraw-after-update" {
                9
            } else {
                usize::MAX
            },
        }))
    }
}
struct Store {
    calls: Arc<Counters>,
    value: Mutex<AvatarSnapshot>,
    fault: &'static str,
}
fn snapshot(key: ParticipantKey) -> AvatarSnapshot {
    let config: Value = serde_json::from_str(include_str!(
        "../../../../contracts/fixtures/assets/avatar-config.json"
    ))
    .unwrap();
    let selection = SetAvatar::parse(
        &serde_json::to_vec(&json!({"expected_revision":1,"config":config})).unwrap(),
    )
    .unwrap()
    .selection;
    let now = chrono::Utc::now();
    AvatarSnapshot {
        avatar: AvatarSettings {
            key,
            revision: Revision::initial(),
            selection,
            created_at: now,
            updated_at: now,
        },
        participant: Participant {
            key,
            revision: Revision::initial(),
            preferences: Preferences {
                theme: Theme::Revenge,
                low_power: false,
                reduced_motion: false,
                sound_enabled: false,
            },
            created_at: now,
            updated_at: now,
        },
    }
}
#[async_trait]
impl ParticipantStore for Store {
    fn release_digest(&self) -> &str {
        DIGEST
    }
    async fn execute(
        &self,
        key: ParticipantKey,
        op: Operation,
        bytes: &[u8],
        authority: &dyn AvatarAuthority,
    ) -> Result<StoreResult, AvatarError> {
        self.calls.store.fetch_add(1, Ordering::SeqCst);
        let grant = authority.check()?;
        assert_eq!(grant.key, key);
        assert_eq!(grant.operation, op.avatar_operation());
        assert_eq!(grant.release_digest, DIGEST);
        let mut value = self.value.lock().unwrap();
        match op {
            Operation::GetParticipant => {
                let mut v = value.clone();
                if self.fault == "foreign-store" {
                    v.avatar.key.user_id = AuthUserId::from_uuid(Uuid::new_v4());
                }
                Ok(StoreResult::Snapshot(v))
            }
            Operation::SetAvatar => {
                let command = SetAvatar::parse(bytes).map_err(|_| AvatarError::InvalidInput)?;
                if command.expected != value.avatar.revision {
                    return Err(AvatarError::Conflict);
                }
                authority.check()?;
                value.avatar.revision = command.expected.next().unwrap();
                value.avatar.selection = command.selection;
                let mut wire =
                    concert_service::adapters::out::repository::avatar::avatar_state(&value.avatar);
                if self.fault == "invalid-success" {
                    wire["private_handle"] = "secret".into();
                }
                Ok(StoreResult::Avatar(wire))
            }
            Operation::UpdatePreferences => {
                let (expected, prefs) =
                    concert_service::adapters::out::repository::avatar::parse_preferences(bytes)?;
                if expected != value.participant.revision {
                    return Err(AvatarError::Conflict);
                }
                authority.check()?;
                value.participant.revision = expected.next().unwrap();
                value.participant.preferences = prefs;
                Ok(StoreResult::Snapshot(value.clone()))
            }
        }
    }
}
fn key() -> ParticipantKey {
    ParticipantKey {
        event_id: EVENT.parse().unwrap(),
        user_id: USER.parse().unwrap(),
    }
}
fn message(op: Operation) -> proto::ParticipantRequest {
    let now = chrono::Utc::now().timestamp_micros();
    let (method,target,body)=match op {
  Operation::GetParticipant=>("GET",format!("/api/concert/events/{EVENT}/me"),vec![]),
  Operation::SetAvatar=>("PUT",format!("/api/concert/events/{EVENT}/me/avatar"),serde_json::to_vec(&json!({"expected_revision":1,"config":config_value(&snapshot(key()).avatar.selection)})).unwrap()),
  Operation::UpdatePreferences=>("PATCH",format!("/api/concert/events/{EVENT}/me"),serde_json::to_vec(&json!({"expected_revision":1,"preferences":{"theme":"DANGER","low_power":true,"reduced_motion":false,"sound_enabled":false}})).unwrap()),
 };
    proto::ParticipantRequest {
        user_id: USER.into(),
        event_id: EVENT.into(),
        session_family_id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
        http_method: method.into(),
        http_target: target,
        request_json: body,
        release_digest: DIGEST.into(),
        authority_expires_unix_micros: now + 1_800_000,
        auth_observed_at_unix_micros: now,
        profile_observed_at_unix_micros: now,
    }
}
fn method(op: Operation) -> &'static str {
    match op {
        Operation::GetParticipant => {
            "/concert.participant.v1.ConcertParticipantService/GetParticipant"
        }
        Operation::SetAvatar => "/concert.participant.v1.ConcertParticipantService/SetAvatar",
        Operation::UpdatePreferences => {
            "/concert.participant.v1.ConcertParticipantService/UpdatePreferences"
        }
    }
}
fn framed(m: &proto::ParticipantRequest) -> Vec<u8> {
    let bytes = m.encode_to_vec();
    let mut result = vec![0];
    result.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    result.extend(bytes);
    result
}
async fn invoke(
    service: ParticipantService,
    m: proto::ParticipantRequest,
    op: Operation,
    kind: &str,
    outer: Duration,
) -> (Code, Option<Value>, Option<String>) {
    let signer = ActorDelegationSigner::from_seed(Workload::Gateway, &[71; 32]).unwrap();
    let verifier = ActorDelegationVerifier::new(Workload::Concert, &[signer.public_key()]).unwrap();
    let guarded = guard(
        &POLICY,
        service.server(),
        CallerRegistry::new()
            .register(Workload::Gateway, "test-gateway")
            .unwrap()
            .register(Workload::Post, "test-post")
            .unwrap(),
        verifier,
    )
    .unwrap();
    let body = framed(&m);
    let signed_body = if kind == "tampered" {
        framed(&message(op))
    } else {
        body.clone()
    };
    let actor = if kind == "actor" {
        Uuid::new_v4()
    } else {
        Uuid::parse_str(&m.user_id).unwrap_or(Uuid::new_v4())
    };
    let token = signer.issue(
        if kind == "audience" {
            Workload::Profile
        } else {
            Workload::Concert
        },
        method(op),
        actor,
        None,
        &signed_body,
        chrono::Utc::now().timestamp(),
    );
    let mut request = axum::http::Request::builder()
        .method("POST")
        .uri(method(op))
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header(
            "x-service-caller",
            if kind == "sibling" { "post" } else { "gateway" },
        )
        .header(
            "x-service-token",
            if kind == "credential" {
                "wrong"
            } else if kind == "sibling" {
                "test-post"
            } else {
                "test-gateway"
            },
        );
    if kind != "unsigned" {
        request = request.header(HEADER, token);
    }
    let request = request
        .body(tonic::body::Body::new(Full::new(
            tonic::codegen::Bytes::from(body),
        )))
        .unwrap();
    let response = if kind == "no-deadline" {
        guarded.oneshot(request).await.unwrap()
    } else {
        platform::call_deadline::scope(std::time::Instant::now() + outer, guarded.oneshot(request))
            .await
            .unwrap()
    };
    let headers = response.headers().clone();
    let collected = response.into_body().collect().await.unwrap();
    let code = Code::from_i32(
        headers
            .get("grpc-status")
            .or_else(|| collected.trailers().and_then(|v| v.get("grpc-status")))
            .unwrap()
            .to_str()
            .unwrap()
            .parse()
            .unwrap(),
    );
    let reason = headers
        .get("x-mcr-core-reason")
        .or_else(|| {
            collected
                .trailers()
                .and_then(|v| v.get("x-mcr-core-reason"))
        })
        .map(|v| v.to_str().unwrap().to_string());
    let bytes = collected.to_bytes();
    let value = if code == Code::Ok {
        let decoded = proto::ParticipantResponse::decode(&bytes[5..]).unwrap();
        assert_eq!(headers.get("cache-control").unwrap(), "no-store");
        Some(serde_json::from_slice(&decoded.response_json).unwrap())
    } else {
        None
    };
    (code, value, reason)
}
fn service(fault: &'static str) -> (ParticipantService, Arc<Counters>) {
    let calls = Arc::new(Counters::default());
    let owners = Arc::new(Owners {
        calls: calls.clone(),
        fault,
    });
    (
        ParticipantService::new(
            Arc::new(Store {
                calls: calls.clone(),
                value: Mutex::new(snapshot(key())),
                fault,
            }),
            owners.clone(),
            owners.clone(),
            owners,
        ),
        calls,
    )
}
async fn call(
    s: ParticipantService,
    m: proto::ParticipantRequest,
    op: Operation,
) -> (Code, Option<Value>, Option<String>) {
    invoke(s, m, op, "valid", Duration::from_secs(2)).await
}
#[tokio::test]
async fn guarded_get_avatar_preferences_and_conflict_reread_use_frozen_json() {
    let (s, calls) = service("");
    let (code, initial, _) = call(
        s.clone(),
        message(Operation::GetParticipant),
        Operation::GetParticipant,
    )
    .await;
    assert_eq!(code, Code::Ok);
    let initial = initial.unwrap();
    assert_eq!(initial["profile"]["display_name"], "Canonical current name");
    assert_eq!(initial["age_status"], "UNKNOWN");
    assert_eq!(calls.permit.load(Ordering::SeqCst), 0);
    let mut avatar = message(Operation::SetAvatar);
    let mut json: Value = serde_json::from_slice(&avatar.request_json).unwrap();
    json["config"]["item_ids"]["accessory"] = Value::Null;
    avatar.request_json = serde_json::to_vec(&json).unwrap();
    let (code, saved, _) = call(s.clone(), avatar.clone(), Operation::SetAvatar).await;
    assert_eq!(code, Code::Ok);
    assert_eq!(saved.as_ref().unwrap()["revision"], 2);
    assert_eq!(saved.as_ref().unwrap()["config"], json["config"]);
    let (code, _, reason) = call(s.clone(), avatar, Operation::SetAvatar).await;
    assert_eq!(code, Code::Aborted);
    assert_eq!(reason.as_deref(), Some("VERSION_CONFLICT"));
    let (code, current, _) = call(
        s.clone(),
        message(Operation::GetParticipant),
        Operation::GetParticipant,
    )
    .await;
    assert_eq!(code, Code::Ok);
    assert_eq!(
        current.unwrap()["profile"]["avatar"],
        saved.clone().unwrap()
    );
    let (code, preferences, _) = call(
        s,
        message(Operation::UpdatePreferences),
        Operation::UpdatePreferences,
    )
    .await;
    assert_eq!(code, Code::Ok);
    let preferences = preferences.unwrap();
    assert_eq!(preferences["revision"], 2);
    assert_eq!(preferences["preferences"]["theme"], "DANGER");
    assert_eq!(preferences["profile"]["avatar"], saved.unwrap());
}
#[tokio::test]
async fn invalid_transport_never_calls_owner_or_store_ports() {
    for op in [
        Operation::GetParticipant,
        Operation::SetAvatar,
        Operation::UpdatePreferences,
    ] {
        for kind in [
            "unsigned",
            "credential",
            "sibling",
            "audience",
            "actor",
            "tampered",
            "no-deadline",
        ] {
            let (s, c) = service("");
            let (code, _, _) = invoke(s, message(op), op, kind, Duration::from_secs(2)).await;
            assert_ne!(code, Code::Ok, "{op:?}/{kind}");
            assert_eq!(c.auth.load(Ordering::SeqCst), 0);
            assert_eq!(c.store.load(Ordering::SeqCst), 0);
        }
    }
}
#[tokio::test]
async fn signed_bad_fields_body_path_release_and_time_never_dispatch_store() {
    for kind in [
        "target",
        "method",
        "get-body",
        "release",
        "expired",
        "future",
        "stale",
        "nil-session",
        "nil-request",
        "profile-fields",
        "duplicate",
        "url",
        "too-large",
    ] {
        let op = if [
            "get-body",
            "expired",
            "future",
            "stale",
            "nil-session",
            "nil-request",
        ]
        .contains(&kind)
        {
            Operation::GetParticipant
        } else if kind == "profile-fields" {
            Operation::UpdatePreferences
        } else {
            Operation::SetAvatar
        };
        let mut m = message(op);
        match kind {
            "target" => m.http_target.push_str("?user=foreign"),
            "method" => m.http_method = "POST".into(),
            "get-body" => m.request_json = b"{}".to_vec(),
            "release" => m.release_digest = "b".repeat(64),
            "expired" => {
                m.auth_observed_at_unix_micros -= 3_000_000;
                m.profile_observed_at_unix_micros -= 3_000_000;
                m.authority_expires_unix_micros = chrono::Utc::now().timestamp_micros() - 1;
            }
            "future" => m.auth_observed_at_unix_micros += 3_000_000,
            "stale" => m.auth_observed_at_unix_micros -= 3_000_000,
            "nil-session" => m.session_family_id = Uuid::nil().to_string(),
            "nil-request" => m.request_id = Uuid::nil().to_string(),
            "profile-fields" => {
                let mut v: Value = serde_json::from_slice(&m.request_json).unwrap();
                v["bio"] = "must-not-ignore".into();
                m.request_json = serde_json::to_vec(&v).unwrap();
            }
            "duplicate" => {
                let text = String::from_utf8(m.request_json).unwrap().replace(
                    "\"expected_revision\":1",
                    "\"expected_revision\":1,\"expected_revision\":2",
                );
                m.request_json = text.into_bytes();
            }
            "url" => {
                let mut v: Value = serde_json::from_slice(&m.request_json).unwrap();
                v["config"]["url"] = "https://evil.invalid".into();
                m.request_json = serde_json::to_vec(&v).unwrap();
            }
            "too-large" => m.request_json = vec![b' '; 4097],
            _ => unreachable!(),
        }
        let (s, c) = service("");
        let (code, _, _) = call(s, m, op).await;
        assert_ne!(code, Code::Ok, "{kind}");
        assert_eq!(c.auth.load(Ordering::SeqCst), 0, "{kind}");
        assert_eq!(c.store.load(Ordering::SeqCst), 0, "{kind}");
    }
}
#[tokio::test]
async fn owner_failures_and_missing_or_withdrawn_capability_fail_closed() {
    for fault in [
        "auth-denied",
        "auth-unavailable",
        "foreign-auth",
        "foreign-session",
        "profile-unavailable",
        "foreign-profile",
        "invalid-profile",
        "stale-profile",
        "capability-denied",
        "capability-unavailable",
        "foreign-body",
        "withdraw-permit",
    ] {
        let (s, c) = service(fault);
        let (code, _, _) = call(s, message(Operation::SetAvatar), Operation::SetAvatar).await;
        assert_ne!(code, Code::Ok, "{fault}");
        if fault.starts_with("auth-")
            || fault.starts_with("foreign-auth")
            || fault == "foreign-session"
        {
            assert_eq!(c.profile.load(Ordering::SeqCst), 0, "{fault}");
        }
        assert!(
            c.store.load(Ordering::SeqCst) <= usize::from(fault == "withdraw-permit"),
            "{fault}"
        );
    }
    let (s, c) = service("invalid-success");
    assert_eq!(
        call(s, message(Operation::SetAvatar), Operation::SetAvatar)
            .await
            .0,
        Code::Internal
    );
    assert_eq!(c.store.load(Ordering::SeqCst), 1);
    let (s, _) = service("foreign-store");
    assert_eq!(
        call(
            s,
            message(Operation::GetParticipant),
            Operation::GetParticipant
        )
        .await
        .0,
        Code::Internal
    );
    let (code, _, _) = call(
        ParticipantService::default(),
        message(Operation::GetParticipant),
        Operation::GetParticipant,
    )
    .await;
    assert_eq!(code, Code::Unavailable);
}
#[tokio::test]
async fn original_deadline_cannot_restart_after_owner_waits() {
    for fault in [
        "slow-auth",
        "slow-profile",
        "blocking-profile",
        "slow-capability",
    ] {
        let (s, c) = service(fault);
        let (code, _, reason) = invoke(
            s,
            message(Operation::SetAvatar),
            Operation::SetAvatar,
            "valid",
            Duration::from_millis(50),
        )
        .await;
        assert_eq!(code, Code::DeadlineExceeded, "{fault}");
        assert_eq!(reason.as_deref(), Some("DEADLINE_EXCEEDED"), "{fault}");
        assert_eq!(c.store.load(Ordering::SeqCst), 0, "{fault}");
    }
}
#[test]
fn participant_policy_is_complete_and_gateway_only() {
    assert_eq!(
        platform::grpc_method_policy::proto_rpc_names(include_str!(
            "../../../proto/concert/v1/participant.proto"
        )),
        ["GetParticipant", "SetAvatar", "UpdatePreferences"]
    );
    for op in [
        Operation::GetParticipant,
        Operation::SetAvatar,
        Operation::UpdatePreferences,
    ] {
        for caller in Workload::ALL {
            assert_eq!(
                POLICY.allows(method(op), caller),
                caller == Workload::Gateway
            );
        }
        assert_eq!(POLICY.actor_fields_for(method(op)), Some(&[&[1][..]][..]));
    }
}

#[path = "support/bootstrap/mod.rs"]
mod native_support;
#[tokio::test]
#[ignore = "requires private marker-owned PostgreSQL16.14 bootstrap fixture"]
async fn native_guarded_participant_persistence_and_reread() {
    use concert_application::bootstrap::BootstrapRepository;
    use concert_service::{
        adapters::out::repository::avatar::AvatarPersistence,
        bootstrap::BootstrapPostgresRepository,
    };
    let pool = native_support::pool().await;
    let release = native_support::release();
    let event = EVENT.parse().unwrap();
    let user = USER.parse().unwrap();
    native_support::seed(&pool, event).await;
    BootstrapPostgresRepository::new(pool.clone())
        .bootstrap(
            &native_support::admission(event, user),
            release.release().defaults(),
        )
        .await
        .unwrap();
    let persistence =
        Arc::new(AvatarPersistence::from_synthetic_test(pool.clone(), release, true).unwrap());
    let digest = persistence.release_digest().to_owned();
    let owners = Arc::new(Owners {
        calls: Arc::new(Counters::default()),
        fault: "",
    });
    let service = ParticipantService::new(
        persistence.clone(),
        owners.clone(),
        owners.clone(),
        owners.clone(),
    );
    let make = |op| {
        let mut m = message(op);
        m.release_digest = digest.clone();
        m
    };
    let denied = ParticipantService::new(
        persistence.clone(),
        owners.clone(),
        owners,
        Arc::new(Unavailable),
    );
    assert_eq!(
        call(denied, make(Operation::SetAvatar), Operation::SetAvatar)
            .await
            .0,
        Code::Unavailable
    );
    // A sequence is deliberately nontransactional: it proves the UPDATE
    // executed before permit withdrawal, while the row itself must roll back.
    sqlx::raw_sql("create sequence public.participant_native_update_probe; create function public.participant_native_probe() returns trigger language plpgsql as $$ begin perform nextval('public.participant_native_update_probe'); return new; end; $$; create trigger participant_native_probe after update on concert_avatar_settings for each row execute function public.participant_native_probe();").execute(&pool).await.unwrap();
    let withdrawing = Arc::new(Owners {
        calls: Arc::new(Counters::default()),
        fault: "withdraw-after-update",
    });
    let withdrawing_service = ParticipantService::new(
        persistence,
        withdrawing.clone(),
        withdrawing.clone(),
        withdrawing,
    );
    assert_eq!(
        call(
            withdrawing_service,
            make(Operation::SetAvatar),
            Operation::SetAvatar
        )
        .await
        .0,
        Code::PermissionDenied
    );
    let executed: bool =
        sqlx::query_scalar("select is_called from public.participant_native_update_probe")
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(
        executed,
        "withdrawal must happen after a real SQL UPDATE, not before dispatch"
    );
    let (code, initial, _) = call(
        service.clone(),
        make(Operation::GetParticipant),
        Operation::GetParticipant,
    )
    .await;
    assert_eq!(code, Code::Ok);
    let initial = initial.unwrap();
    assert_eq!(initial["profile"]["avatar"]["revision"], 1);
    let mut avatar = make(Operation::SetAvatar);
    let mut wire: Value = serde_json::from_slice(&avatar.request_json).unwrap();
    wire["config"]["item_ids"]["accessory"] = Value::Null;
    avatar.request_json = serde_json::to_vec(&wire).unwrap();
    let (code, saved, _) = call(service.clone(), avatar.clone(), Operation::SetAvatar).await;
    assert_eq!(code, Code::Ok);
    assert_eq!(saved.as_ref().unwrap()["revision"], 2);
    let (code, _, reason) = call(service.clone(), avatar, Operation::SetAvatar).await;
    assert_eq!(code, Code::Aborted);
    assert_eq!(reason.as_deref(), Some("VERSION_CONFLICT"));
    let (code, current, _) = call(
        service.clone(),
        make(Operation::GetParticipant),
        Operation::GetParticipant,
    )
    .await;
    assert_eq!(code, Code::Ok);
    assert_eq!(
        current.unwrap()["profile"]["avatar"],
        saved.clone().unwrap()
    );
    let (code, updated, _) = call(
        service.clone(),
        make(Operation::UpdatePreferences),
        Operation::UpdatePreferences,
    )
    .await;
    assert_eq!(code, Code::Ok);
    let updated = updated.unwrap();
    assert_eq!(updated["revision"], 2);
    assert_eq!(updated["profile"]["avatar"], saved.unwrap());
    assert_eq!(
        updated["profile"]["display_name"],
        initial["profile"]["display_name"]
    );
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(event)
    .execute(&pool)
    .await
    .unwrap();
    let (code, _, reason) = call(
        service.clone(),
        make(Operation::UpdatePreferences),
        Operation::UpdatePreferences,
    )
    .await;
    assert_eq!(code, Code::FailedPrecondition);
    assert_eq!(reason.as_deref(), Some("EVENT_READ_ONLY"));
    assert_eq!(
        call(
            service.clone(),
            make(Operation::GetParticipant),
            Operation::GetParticipant
        )
        .await
        .0,
        Code::Ok
    );
    sqlx::query("update concert_event_admissions set state='SUSPENDED',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2").bind(event).bind(user).execute(&pool).await.unwrap();
    assert_eq!(
        call(
            service,
            make(Operation::GetParticipant),
            Operation::GetParticipant
        )
        .await
        .0,
        Code::PermissionDenied
    );
    pool.close().await;
}

#[tokio::test]
async fn direct_handler_without_verified_guard_context_is_denied() {
    use concert_service::participant::proto::concert_participant_service_server::ConcertParticipantService;
    let (service, calls) = service("");
    let response = service
        .get_participant(tonic::Request::new(message(Operation::GetParticipant)))
        .await;
    assert_eq!(response.unwrap_err().code(), Code::Unauthenticated);
    assert_eq!(calls.auth.load(Ordering::SeqCst), 0);
}
