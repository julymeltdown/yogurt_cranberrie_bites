use auth_application::concert_relationship::*;
use auth_service::adapters::{
    out::persistence::concert_relationship::PgCurrentRelationship,
    r#in::grpc::concert_relationship::{proto, RelationshipServer, POLICY},
};
use chrono::Utc;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};
use uuid::Uuid;
struct Owner(Arc<AtomicUsize>, bool);
#[async_trait::async_trait]
impl CurrentRelationshipPort for Owner {
    async fn read_current(
        &self,
        q: RelationshipRead,
        _: &ReadBudget,
    ) -> Result<CurrentRelationship, ReadError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        let mut query = q;
        if self.1 {
            query = RelationshipRead::new(
                q.event_id(),
                Uuid::new_v4(),
                q.subject_id(),
                q.session_family_id(),
                q.request_id(),
                q.purpose(),
                q.expires_at(),
            )
            .unwrap();
        }
        Ok(CurrentRelationship {
            query,
            observed_at: Utc::now(),
            valid_until: q.expires_at(),
            owner_epoch: Uuid::new_v4(),
            revision: 1,
            actor_active: true,
            session_live: true,
            subject_active: true,
        })
    }
}
fn message() -> proto::RelationshipRead {
    proto::RelationshipRead {
        actor_id: Uuid::new_v4().to_string(),
        event_id: Uuid::new_v4().to_string(),
        subject_id: Uuid::new_v4().to_string(),
        session_family_id: Uuid::new_v4().to_string(),
        request_id: Uuid::new_v4().to_string(),
        purpose: "PRESENCE".into(),
        expires_unix_micros: (Utc::now() + chrono::Duration::milliseconds(800)).timestamp_micros(),
    }
}
fn framed(m: &proto::RelationshipRead) -> Vec<u8> {
    use prost::Message;
    let bytes = m.encode_to_vec();
    let mut v = vec![0];
    v.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    v.extend(bytes);
    v
}
async fn invoke(m: proto::RelationshipRead, kind: &str) -> (i32, usize) {
    use http_body_util::{BodyExt, Full};
    use platform::{
        actor_delegation::{ActorDelegationSigner, ActorDelegationVerifier, HEADER},
        grpc_auth::{CallerRegistry, Workload},
        grpc_method_policy::guard,
    };
    use tower::ServiceExt;
    let calls = Arc::new(AtomicUsize::new(0));
    let signer = ActorDelegationSigner::from_seed(Workload::Concert, &[37; 32]).unwrap();
    let svc = guard(
        &POLICY,
        proto::auth_concert_relationship_service_server::AuthConcertRelationshipServiceServer::new(
            RelationshipServer::new(Arc::new(Owner(calls.clone(), kind == "foreign-reply"))),
        ),
        CallerRegistry::new()
            .register(Workload::Concert, "concert")
            .unwrap()
            .register(Workload::Gateway, "gateway")
            .unwrap(),
        platform::actor_delegation::relationship_read::RelationshipReadVerifier::new(
            Workload::Auth,
            &[signer.public_key()],
            &ActorDelegationVerifier::new(
                Workload::Auth,
                &[
                    ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32])
                        .unwrap()
                        .public_key(),
                ],
            )
            .unwrap(),
        )
        .unwrap()
        .into_inner(),
    )
    .unwrap();
    let body = framed(&m);
    let signed = if kind == "body" {
        framed(&message())
    } else {
        body.clone()
    };
    let actor = if kind == "actor" {
        Uuid::new_v4()
    } else {
        Uuid::parse_str(&m.actor_id).unwrap()
    };
    let aud = if kind == "audience" {
        Workload::Concert
    } else {
        Workload::Auth
    };
    let method = "/relationship.v1.AuthConcertRelationshipService/ReadCurrent";
    let token = signer.issue(aud, method, actor, None, &signed, Utc::now().timestamp());
    let mut req = axum::http::Request::builder()
        .method("POST")
        .uri(if kind == "method" {
            "/relationship.v1.AuthConcertRelationshipService/Unknown"
        } else {
            method
        })
        .header("content-type", "application/grpc")
        .header("te", "trailers")
        .header(
            "x-service-caller",
            if kind == "sibling" {
                "gateway"
            } else {
                "concert"
            },
        )
        .header(
            "x-service-token",
            if kind == "credential" {
                "bad"
            } else if kind == "sibling" {
                "gateway"
            } else {
                "concert"
            },
        );
    if kind != "unsigned" {
        req = req.header(HEADER, token);
    }
    let req = req
        .body(tonic::body::Body::new(Full::new(
            tonic::codegen::Bytes::from(body),
        )))
        .unwrap();
    let resp = if kind == "no-deadline" {
        svc.oneshot(req).await.unwrap()
    } else {
        platform::call_deadline::scope(
            std::time::Instant::now() + std::time::Duration::from_secs(1),
            svc.oneshot(req),
        )
        .await
        .unwrap()
    };
    let headers = resp.headers().clone();
    let collected = resp.into_body().collect().await.unwrap();
    let status: i32 = headers
        .get("grpc-status")
        .or_else(|| collected.trailers().and_then(|t| t.get("grpc-status")))
        .unwrap()
        .to_str()
        .unwrap()
        .parse()
        .unwrap();
    if status == 0 {
        assert_eq!(headers.get("cache-control").unwrap(), "no-store");
        use prost::Message;
        let bytes = collected.to_bytes();
        let decoded = proto::AuthRelationshipObservation::decode(&bytes[5..]).unwrap();
        assert_eq!(decoded.query, Some(m));
        assert!(decoded.revision > 0);
    }
    (status, calls.load(Ordering::SeqCst))
}
#[tokio::test]
async fn generated_guard_allows_only_exact_concert_signed_bound_read() {
    assert_eq!(invoke(message(), "valid").await, (0, 1));
    for kind in [
        "unsigned",
        "credential",
        "sibling",
        "actor",
        "body",
        "audience",
        "method",
        "no-deadline",
    ] {
        let (code, calls) = invoke(message(), kind).await;
        assert_ne!(code, 0, "{kind}");
        assert_eq!(calls, 0, "{kind}");
    }
}
#[tokio::test]
async fn invalid_query_expiry_or_owner_reply_never_becomes_success() {
    for kind in ["purpose", "event", "family", "request", "expired", "long"] {
        let mut m = message();
        match kind {
            "purpose" => m.purpose = "UNKNOWN".into(),
            "event" => m.event_id = Uuid::nil().to_string(),
            "family" => m.session_family_id = "private-canary".into(),
            "request" => m.request_id = Uuid::nil().to_string(),
            "expired" => m.expires_unix_micros = Utc::now().timestamp_micros() - 1,
            "long" => m.expires_unix_micros += 3_000_000,
            _ => unreachable!(),
        }
        let (code, calls) = invoke(m, "valid").await;
        assert_ne!(code, 0, "{kind}");
        assert_eq!(calls, 0, "{kind}");
    }
    assert_ne!(invoke(message(), "foreign-reply").await.0, 0);
}

async fn owned_pool() -> sqlx::PgPool {
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
        &std::env::var("MCR_RELATIONSHIP_AUTH_URL").unwrap(),
    )
    .unwrap();
    assert_eq!(options.get_username(), "mcr_owner");
    assert_eq!(options.get_database(), Some("mcr_auth_relationship"));
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
            "mcr_auth_relationship".into(),
            "mcr_owner".into(),
            root.join("data").to_str().unwrap().into(),
            root.join("socket").to_str().unwrap().into(),
            "".into(),
            160014
        )
    );
    pool
}
fn budget() -> ReadBudget {
    ReadBudget::new(
        std::time::Instant::now() + std::time::Duration::from_secs(1),
        ReadCancellation::default(),
    )
    .unwrap()
}
fn input(actor: Uuid, subject: Uuid, family: Uuid) -> RelationshipRead {
    RelationshipRead::new(
        Uuid::new_v4(),
        actor,
        subject,
        family,
        Uuid::new_v4(),
        Purpose::Presence,
        Utc::now() + chrono::Duration::milliseconds(900),
    )
    .unwrap()
}

#[tokio::test]
#[ignore = "owned owner_relationship_smoke.py cluster required"]
async fn native_relationship_owner_snapshot_and_revisions() {
    let pool = owned_pool().await;
    let owner = PgCurrentRelationship::new(pool.clone());
    let actor = Uuid::new_v4();
    let subject = Uuid::new_v4();
    let family = Uuid::new_v4();
    for id in [actor, subject] {
        sqlx::query("insert into users(id,primary_email,password_hash,status) values($1,$2,'private-canary','ACTIVE')").bind(id).bind(format!("{id}@native.invalid")).execute(&pool).await.unwrap();
    }
    sqlx::query("insert into refresh_tokens(jti,token_hash,user_id,family_id,expires_at) values($1,'private-canary',$2,$3,clock_timestamp()+interval '1 hour')").bind(Uuid::new_v4().to_string()).bind(actor).bind(family).execute(&pool).await.unwrap();
    let q = input(actor, subject, family);
    let first = owner.read_current(q, &budget()).await.unwrap();
    assert!(first.eligible());
    let wrong = owner
        .read_current(input(actor, subject, Uuid::new_v4()), &budget())
        .await
        .unwrap();
    assert!(!wrong.eligible());
    let missing = owner
        .read_current(input(actor, Uuid::new_v4(), family), &budget())
        .await
        .unwrap();
    assert!(!missing.subject_active);
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(subject)
        .execute(&mut *tx)
        .await
        .unwrap();
    let old = owner
        .read_current(input(actor, subject, family), &budget())
        .await
        .unwrap();
    assert_eq!(old.revision, first.revision);
    assert!(old.eligible());
    tx.rollback().await.unwrap();
    assert_eq!(
        owner
            .read_current(input(actor, subject, family), &budget())
            .await
            .unwrap()
            .revision,
        first.revision
    );
    sqlx::query("update users set status='SUSPENDED' where id=$1")
        .bind(subject)
        .execute(&pool)
        .await
        .unwrap();
    let suspended = owner
        .read_current(input(actor, subject, family), &budget())
        .await
        .unwrap();
    assert!(!suspended.subject_active);
    assert!(suspended.revision > first.revision);
    assert_eq!(suspended.owner_epoch, first.owner_epoch);
    sqlx::query("update users set status='ACTIVE' where id=$1")
        .bind(subject)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("update refresh_tokens set revoked_at=clock_timestamp() where user_id=$1")
        .bind(actor)
        .execute(&pool)
        .await
        .unwrap();
    let revoked = owner
        .read_current(input(actor, subject, family), &budget())
        .await
        .unwrap();
    assert!(!revoked.session_live);
    assert!(revoked.revision > suspended.revision);
    let cancelled = ReadCancellation::default();
    cancelled.cancel();
    assert_eq!(
        owner
            .read_current(
                input(actor, subject, family),
                &ReadBudget::new(
                    std::time::Instant::now() + std::time::Duration::from_secs(1),
                    cancelled
                )
                .unwrap()
            )
            .await
            .unwrap_err(),
        ReadError::Cancelled
    );
    let mut lock = pool.begin().await.unwrap();
    sqlx::query("lock table users in access exclusive mode")
        .execute(&mut *lock)
        .await
        .unwrap();
    let short = ReadBudget::new(
        std::time::Instant::now() + std::time::Duration::from_millis(25),
        ReadCancellation::default(),
    )
    .unwrap();
    assert_eq!(
        owner
            .read_current(input(actor, subject, family), &short)
            .await
            .unwrap_err(),
        ReadError::DeadlineExceeded
    );
    lock.rollback().await.unwrap();
    assert!(owner
        .read_current(input(actor, subject, family), &budget())
        .await
        .is_ok());
    for sql in [
        "delete from auth_concert_relationship_revision",
        "truncate auth_concert_relationship_revision",
        "update auth_concert_relationship_revision set revision=revision",
    ] {
        assert!(sqlx::query(sql).execute(&pool).await.is_err());
    }
    pool.close().await;
}
