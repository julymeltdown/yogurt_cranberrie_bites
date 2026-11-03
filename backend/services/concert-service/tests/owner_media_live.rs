//! Uploader runtime regressions. Test guards are deliberately NOT production Auth fences.
mod owner_media_live_support;
// Until the composition owner registers the root module, path inclusion validates
// these exact runtime files without acquiring shared lib.rs ownership.
mod adapters {
    pub use concert_service::adapters::*;
}
#[path = "../src/media_runtime/mod.rs"]
mod media_runtime;
use async_trait::async_trait;
use chrono::Utc;
use concert_application::{commands::*, media::*};
use concert_service::adapters::out::media::codec::BoundedImageSanitizer;
use image::{DynamicImage, ImageFormat, RgbaImage};
use media_runtime::*;
use platform::{actor_delegation::authority_deadline::AuthorityRpcDeadline, cpu::WorkerPool};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use std::{
    io::Cursor,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
    time::Duration,
};
use uuid::Uuid;
fn identity() -> OwnerMediaIdentity {
    OwnerMediaIdentity {
        actor: Uuid::new_v4(),
        event: Uuid::new_v4(),
        session_family: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        deadline: AuthorityRpcDeadline::new(
            Utc::now().timestamp_micros() + 10_000_000,
            Duration::from_secs(10),
        )
        .unwrap(),
    }
}
fn request(
    i: OwnerMediaIdentity,
    op: &str,
    key: Uuid,
    id: Option<Uuid>,
    body: Vec<u8>,
) -> Result<OwnerMediaRequest, CommandError> {
    let base = format!("/api/concert/events/{}", i.event);
    let (method, target, mime, key) = match op {
        "initConcertUpload" => (
            "POST",
            format!("{base}/media/uploads"),
            "application/json",
            key.to_string(),
        ),
        "putConcertUploadBytes" => (
            "PUT",
            format!("{base}/media/{}/bytes", id.unwrap()),
            "image/png",
            key.to_string(),
        ),
        "finalizeConcertUpload" => (
            "POST",
            format!("{base}/media/{}/finalize", id.unwrap()),
            "application/json",
            key.to_string(),
        ),
        "getConcertMedia" => (
            "GET",
            format!("{base}/media/{}", id.unwrap()),
            "",
            String::new(),
        ),
        "readConcertMedia" => (
            "GET",
            format!("{base}/media/{}/content", id.unwrap()),
            "",
            String::new(),
        ),
        _ => unreachable!(),
    };
    OwnerMediaRequest::from_verified(i, op, method, &target, mime, &key, body)
}
fn png() -> Vec<u8> {
    let mut c = Cursor::new(Vec::new());
    DynamicImage::ImageRgba8(RgbaImage::from_pixel(4, 3, image::Rgba([1, 2, 3, 255])))
        .write_to(&mut c, ImageFormat::Png)
        .unwrap();
    c.into_inner()
}
fn init(bytes: &[u8]) -> Vec<u8> {
    serde_json::to_vec(&serde_json::json!({"purpose":"POST","content_type":"image/png","byte_length":bytes.len(),"sha256":Sha256::digest(bytes).iter().map(|b|format!("{b:02x}")).collect::<String>()})).unwrap()
}
fn json(reply: OwnerMediaReply) -> serde_json::Value {
    if let OwnerMediaReply::Json { status, body } = reply {
        assert_eq!(status, 200);
        body
    } else {
        panic!("JSON required")
    }
}
#[tokio::test]
async fn exact_wire_binding_rejects_method_event_query_and_body_mismatches() {
    let i = identity();
    let key = Uuid::new_v4();
    let path = format!("/api/concert/events/{}/media/uploads", i.event);
    let b = init(&png());
    for (method,target,mime,key,body) in [
        ("PUT",path.clone(),"application/json",key.to_string(),b.clone()),
        ("POST",format!("{path}?actor=other"),"application/json",key.to_string(),b.clone()),
        ("POST",path.replace(&i.event.to_string(),&Uuid::new_v4().to_string()),"application/json",key.to_string(),b.clone()),
        ("POST",path.clone(),"text/plain",key.to_string(),b.clone()),
        ("POST",path.clone(),"application/json",Uuid::nil().to_string(),b.clone()),
        ("POST",path.clone(),"application/json",key.to_string(),br#"{"purpose":"POST","purpose":"REPORT","content_type":"image/png","byte_length":1,"sha256":"0000000000000000000000000000000000000000000000000000000000000000"}"#.to_vec()),
        ("POST",path.clone(),"application/json",key.to_string(),br#"{"purpose":"POST","content_type":"image/png","byte_length":1,"sha256":"0000000000000000000000000000000000000000000000000000000000000000","target_id":"ignored"}"#.to_vec()),
    ]{assert!(OwnerMediaRequest::from_verified(i,"initConcertUpload",method,&target,mime,&key,body).is_err());}
    assert!(request(i, "initConcertUpload", key, None, b).is_ok());
}
#[tokio::test]
async fn canonical_recovery_accepts_only_three_scoped_operations() {
    let i = identity();
    let key = Uuid::new_v4();
    let root = format!("/api/concert/events/{}/commands", i.event);
    for op in [
        "initConcertUpload",
        "putConcertUploadBytes",
        "finalizeConcertUpload",
    ] {
        let target = format!("{root}?operation_id={op}&idempotency_key={key}");
        let r = OwnerMediaRequest::from_verified(
            i,
            "getConcertCommandStatus",
            "GET",
            &target,
            "",
            "",
            vec![],
        )
        .unwrap();
        assert_eq!(r.binding().operation, "getConcertCommandStatus");
    }
    for query in [
        format!("operation_id=other&idempotency_key={key}"),
        format!("operation_id=initConcertUpload&idempotency_key={key}&media_id={key}"),
        format!(
            "operation_id=initConcertUpload&operation_id=initConcertUpload&idempotency_key={key}"
        ),
    ] {
        assert!(OwnerMediaRequest::from_verified(
            i,
            "getConcertCommandStatus",
            "GET",
            &format!("{root}?{query}"),
            "",
            "",
            vec![]
        )
        .is_err());
    }
}
#[tokio::test]
async fn binding_digest_covers_body_session_request_deadline_key_and_target() {
    let i = identity();
    let key = Uuid::new_v4();
    let b = init(&png());
    let a = request(i, "initConcertUpload", key, None, b.clone()).unwrap();
    for changed in [
        OwnerMediaIdentity {
            session_family: Uuid::new_v4(),
            ..i
        },
        OwnerMediaIdentity {
            request_id: Uuid::new_v4(),
            ..i
        },
        OwnerMediaIdentity {
            actor: Uuid::new_v4(),
            ..i
        },
        OwnerMediaIdentity {
            event: Uuid::new_v4(),
            ..i
        },
        OwnerMediaIdentity {
            deadline: AuthorityRpcDeadline::new(
                Utc::now().timestamp_micros() + 500_000,
                Duration::from_secs(1),
            )
            .unwrap(),
            ..i
        },
    ] {
        assert_ne!(
            a.binding().digest(),
            request(changed, "initConcertUpload", key, None, b.clone())
                .unwrap()
                .binding()
                .digest()
        );
    }
    let mut spaced = b.clone();
    spaced.push(b' ');
    assert_ne!(
        a.binding().digest(),
        request(i, "initConcertUpload", key, None, spaced)
            .unwrap()
            .binding()
            .digest()
    );
    assert_ne!(
        a.binding().digest(),
        request(i, "initConcertUpload", Uuid::new_v4(), None, b)
            .unwrap()
            .binding()
            .digest()
    );
}
#[tokio::test]
async fn expired_original_request_never_acquires_authority_or_database() {
    let hits = Arc::new(AtomicUsize::new(0));
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/never_connect")
        .unwrap();
    let s = runtime(pool, Arc::new(DeniedFence(hits.clone())));
    let i = OwnerMediaIdentity {
        deadline: AuthorityRpcDeadline::new(
            Utc::now().timestamp_micros() + 50_000,
            Duration::from_secs(1),
        )
        .unwrap(),
        ..identity()
    };
    let r = request(i, "initConcertUpload", Uuid::new_v4(), None, init(&png())).unwrap();
    tokio::time::sleep(Duration::from_millis(60)).await;
    assert!(matches!(
        s.dispatch(r).await,
        Err(CommandError::Unavailable)
    ));
    assert_eq!(hits.load(Ordering::SeqCst), 0);
}
struct DeniedFence(Arc<AtomicUsize>);
#[async_trait]
impl OwnerMediaFence for DeniedFence {
    async fn run(
        &self,
        _: Arc<OwnerMediaBinding>,
        _: OwnerMediaWork,
    ) -> Result<OwnerMediaReply, CommandError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(CommandError::Unavailable)
    }
}
fn runtime(pool: PgPool, fence: Arc<dyn OwnerMediaFence>) -> OwnerMediaRuntime {
    OwnerMediaRuntime::new(
        pool,
        Arc::new(BoundedImageSanitizer::new(
            WorkerPool::new(1).unwrap().class(0).unwrap(),
        )),
        Arc::new(
            FingerprintKeys::new(
                SigningKey::new("owner_media_test", &[9; 32]).unwrap(),
                vec![],
            )
            .unwrap(),
        ),
        fence,
    )
}
#[tokio::test]
async fn missing_owner_fence_denies_before_any_database_connection() {
    let hits = Arc::new(AtomicUsize::new(0));
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused:unused@127.0.0.1:1/never_connect")
        .unwrap();
    let s = runtime(pool, Arc::new(DeniedFence(hits.clone())));
    assert!(matches!(
        s.dispatch(
            request(
                identity(),
                "initConcertUpload",
                Uuid::new_v4(),
                None,
                init(&png())
            )
            .unwrap()
        )
        .await,
        Err(CommandError::Unavailable)
    ));
    assert_eq!(hits.load(Ordering::SeqCst), 1);
}

// This fixture uses SAME-DB row locks for deterministic regression coverage.
// It is not a canonical Auth owner, owner protocol, or production default.
struct FixtureFence;
struct FixtureGuard {
    digest: [u8; 32],
    actor: Uuid,
    family: Uuid,
}
#[async_trait]
impl OwnerMediaFence for FixtureFence {
    async fn run(
        &self,
        b: Arc<OwnerMediaBinding>,
        work: OwnerMediaWork,
    ) -> Result<OwnerMediaReply, CommandError> {
        work(Arc::new(FixtureGuard {
            digest: b.digest(),
            actor: b.identity.actor,
            family: b.identity.session_family,
        }))
        .await
    }
}
#[async_trait]
impl OwnerMediaGuard for FixtureGuard {
    fn binding_digest(&self) -> [u8; 32] {
        self.digest
    }
    fn authority_revision(&self) -> i64 {
        1
    }
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        b: &OwnerMediaBinding,
    ) -> Result<(), CommandError> {
        b.check_current()?;
        let live: Option<bool> = sqlx::query_scalar(
            "select live from owner_media_fixture_auth where actor=$1 and family=$2 for share",
        )
        .bind(self.actor)
        .bind(self.family)
        .fetch_optional(c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        if live == Some(true) {
            Ok(())
        } else {
            Err(CommandError::Denied)
        }
    }
}
async fn fixture() -> (PgPool, OwnerMediaIdentity, OwnerMediaRuntime) {
    let p = owner_media_live_support::owned_pool().await.unwrap();
    let i = identity();
    sqlx::query("create table if not exists owner_media_fixture_auth(actor uuid,family uuid,live bool,primary key(actor,family))").execute(&p).await.unwrap();
    sqlx::query("insert into owner_media_fixture_auth values($1,$2,true)")
        .bind(i.actor)
        .bind(i.session_family)
        .execute(&p)
        .await
        .unwrap();
    sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Owner media fixture','OPEN')").bind(i.event).bind(format!("owner-media-{}",i.event)).execute(&p).await.unwrap();
    sqlx::query(
        "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
    )
    .bind(i.event)
    .bind(i.actor)
    .execute(&p)
    .await
    .unwrap();
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(i.event).bind(i.actor).execute(&p).await.unwrap();
    let s = runtime(p.clone(), Arc::new(FixtureFence));
    (p, i, s)
}
async fn add_actor(p: &PgPool, i: OwnerMediaIdentity) {
    sqlx::query("insert into owner_media_fixture_auth values($1,$2,true)")
        .bind(i.actor)
        .bind(i.session_family)
        .execute(p)
        .await
        .unwrap();
    sqlx::query(
        "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
    )
    .bind(i.event)
    .bind(i.actor)
    .execute(p)
    .await
    .unwrap();
    sqlx::query("insert into concert_participants(event_id,user_id,theme,low_power,reduced_motion,sound_enabled) values($1,$2,'REVENGE',false,false,false)").bind(i.event).bind(i.actor).execute(p).await.unwrap();
}
async fn clean(p: PgPool, i: OwnerMediaIdentity) {
    // Durable BE15 receipts intentionally retain event references until this entire
    // owned cluster is removed by the harness. Do not delete the event here.
    sqlx::query("delete from concert_participants where event_id=$1")
        .bind(i.event)
        .execute(&p)
        .await
        .unwrap();
    p.close().await;
}
#[tokio::test]
async fn native_owner_upload_metadata_content_recovery_and_revocation() {
    let (p, i, s) = fixture().await;
    let bytes = png();
    let key = Uuid::new_v4();
    let intent = json(
        s.dispatch(request(i, "initConcertUpload", key, None, init(&bytes)).unwrap())
            .await
            .unwrap(),
    );
    let id = Uuid::parse_str(intent["media_id"].as_str().unwrap()).unwrap();
    assert_eq!(intent["upload_method"], "PUT");
    assert_eq!(intent["state"], "PENDING");
    let put_key = Uuid::new_v4();
    let put = json(
        s.dispatch(request(i, "putConcertUploadBytes", put_key, Some(id), bytes).unwrap())
            .await
            .unwrap(),
    );
    assert_eq!(put["revision"], 2);
    let final_key = Uuid::new_v4();
    let ready = json(
        s.dispatch(
            request(
                i,
                "finalizeConcertUpload",
                final_key,
                Some(id),
                br#"{"expected_revision":2}"#.to_vec(),
            )
            .unwrap(),
        )
        .await
        .unwrap(),
    );
    assert_eq!(ready["state"], "READY");
    assert_eq!(ready["width"], 4);
    let metadata = json(
        s.dispatch(request(i, "getConcertMedia", Uuid::nil(), Some(id), vec![]).unwrap())
            .await
            .unwrap(),
    );
    assert_eq!(metadata, ready);
    match s
        .dispatch(request(i, "readConcertMedia", Uuid::nil(), Some(id), vec![]).unwrap())
        .await
        .unwrap()
    {
        OwnerMediaReply::Image(image) => {
            assert_eq!(image.headers()[1], ("cache-control", "private, no-store"));
            assert_eq!(image::load_from_memory(&image.bytes).unwrap().width(), 4)
        }
        _ => panic!("private image required"),
    }
    for (op, k) in [
        ("initConcertUpload", key),
        ("putConcertUploadBytes", put_key),
        ("finalizeConcertUpload", final_key),
    ] {
        let target = format!(
            "/api/concert/events/{}/commands?operation_id={op}&idempotency_key={k}",
            i.event
        );
        let r = OwnerMediaRequest::from_verified(
            i,
            "getConcertCommandStatus",
            "GET",
            &target,
            "",
            "",
            vec![],
        )
        .unwrap();
        let r = json(s.dispatch(r).await.unwrap());
        assert_eq!(r["resource_id"], id.to_string());
        assert_eq!(r["operation_id"], op);
        assert_eq!(r["state"], "SUCCEEDED");
    }
    // A READ_ONLY transition forbids writes while preserving current owner reads
    // and recovery of the original immutable receipt under a fresh event revision.
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(i.event)
    .execute(&p)
    .await
    .unwrap();
    assert_eq!(
        json(
            s.dispatch(request(i, "getConcertMedia", Uuid::nil(), Some(id), vec![]).unwrap())
                .await
                .unwrap()
        )["state"],
        "READY"
    );
    let target=format!("/api/concert/events/{}/commands?operation_id=finalizeConcertUpload&idempotency_key={final_key}",i.event);
    assert_eq!(
        json(
            s.dispatch(
                OwnerMediaRequest::from_verified(
                    i,
                    "getConcertCommandStatus",
                    "GET",
                    &target,
                    "",
                    "",
                    vec![]
                )
                .unwrap()
            )
            .await
            .unwrap()
        )["resource_id"],
        id.to_string()
    );
    let foreign = OwnerMediaIdentity {
        actor: Uuid::new_v4(),
        session_family: Uuid::new_v4(),
        ..i
    };
    add_actor(&p, foreign).await;
    assert!(matches!(
        s.dispatch(request(foreign, "readConcertMedia", Uuid::nil(), Some(id), vec![]).unwrap())
            .await,
        Err(CommandError::Denied)
    ));
    sqlx::query("update owner_media_fixture_auth set live=false where actor=$1")
        .bind(i.actor)
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        s.dispatch(request(i, "getConcertMedia", Uuid::nil(), Some(id), vec![]).unwrap())
            .await,
        Err(CommandError::Denied)
    ));
    sqlx::query("update owner_media_fixture_auth set live=true where actor=$1")
        .bind(i.actor)
        .execute(&p)
        .await
        .unwrap();
    // Even the uploader loses this private runtime's access after target linkage.
    sqlx::query("update concert_media set target_id=$2,revision=revision+1 where media_id=$1")
        .bind(id)
        .bind(Uuid::new_v4())
        .execute(&p)
        .await
        .unwrap();
    assert!(matches!(
        s.dispatch(request(i, "getConcertMedia", Uuid::nil(), Some(id), vec![]).unwrap())
            .await,
        Err(CommandError::Denied)
    ));
    clean(p, i).await;
}
#[tokio::test]
async fn native_wrong_owner_read_only_suspension_and_failed_bytes_leave_no_effects() {
    let (p, i, s) = fixture().await;
    let bytes = png();
    let key = Uuid::new_v4();
    let intent = json(
        s.dispatch(request(i, "initConcertUpload", key, None, init(&bytes)).unwrap())
            .await
            .unwrap(),
    );
    let id = Uuid::parse_str(intent["media_id"].as_str().unwrap()).unwrap();
    let bad_key = Uuid::new_v4();
    assert!(s
        .dispatch(
            request(
                i,
                "putConcertUploadBytes",
                bad_key,
                Some(id),
                vec![0; bytes.len()]
            )
            .unwrap()
        )
        .await
        .is_err());
    let count: i64 = sqlx::query_scalar(
        "select count(*) from concert_command_receipts where idempotency_key=$1",
    )
    .bind(bad_key)
    .fetch_one(&p)
    .await
    .unwrap();
    assert_eq!(count, 0);
    let state: String = sqlx::query_scalar("select state from concert_media where media_id=$1")
        .bind(id)
        .fetch_one(&p)
        .await
        .unwrap();
    assert_eq!(state, "PENDING");
    let foreign = OwnerMediaIdentity {
        actor: Uuid::new_v4(),
        session_family: Uuid::new_v4(),
        ..i
    };
    add_actor(&p, foreign).await;
    assert!(s
        .dispatch(request(foreign, "getConcertMedia", Uuid::nil(), Some(id), vec![]).unwrap())
        .await
        .is_err());
    sqlx::query("update concert_event_admissions set state='SUSPENDED',restriction_revision=restriction_revision+1 where event_id=$1 and user_id=$2").bind(i.event).bind(i.actor).execute(&p).await.unwrap();
    assert!(matches!(
        s.dispatch(
            request(
                i,
                "putConcertUploadBytes",
                Uuid::new_v4(),
                Some(id),
                bytes.clone()
            )
            .unwrap()
        )
        .await,
        Err(CommandError::Denied)
    ));
    sqlx::query(
        "update concert_events set phase='READ_ONLY',revision=revision+1 where event_id=$1",
    )
    .bind(i.event)
    .execute(&p)
    .await
    .unwrap();
    assert!(matches!(
        s.dispatch(request(i, "initConcertUpload", Uuid::new_v4(), None, init(&bytes)).unwrap())
            .await,
        Err(CommandError::Denied)
    ));
    clean(p, i).await;
}

#[tokio::test]
async fn native_transaction_settlement_proves_terminal_xid_not_backend_absence() {
    use media_runtime::settlement::*;
    let p = owner_media_live_support::owned_pool().await.unwrap();
    let mut tx = p.begin().await.unwrap();
    let committed = MediaTransactionIdentity::capture_in(&mut tx).await.unwrap();
    assert_eq!(
        transaction_state(&p, &committed).await,
        MediaTransactionState::InProgress
    );
    tx.commit().await.unwrap();
    assert_eq!(
        transaction_state(&p, &committed).await,
        MediaTransactionState::Committed
    );
    let mut tx = p.begin().await.unwrap();
    let aborted = MediaTransactionIdentity::capture_in(&mut tx).await.unwrap();
    tx.rollback().await.unwrap();
    assert_eq!(
        transaction_state(&p, &aborted).await,
        MediaTransactionState::Aborted
    );
    assert_ne!(committed.xid(), aborted.xid());
    p.close().await;
}

#[tokio::test]
async fn native_internal_resolver_binds_actual_instance_cluster_request_and_receipt_scope() {
    use media_runtime::settlement::*;
    let p = owner_media_live_support::owned_pool().await.unwrap();
    let i = identity();
    let r = request(i, "initConcertUpload", Uuid::new_v4(), None, init(&png())).unwrap();
    let guard_digest = [7; 32];
    let instance = Uuid::new_v4();
    let mut tx = p.begin().await.unwrap();
    let actual = MediaTransactionIdentity::capture_in(&mut tx).await.unwrap();
    tx.commit().await.unwrap();
    let mut binding = MediaResolveBinding {
        actor: i.actor,
        event: i.event,
        session_family: i.session_family,
        request_id: i.request_id,
        operation: r.binding().operation.into(),
        guard_digest,
        body_digest: r.binding().body_sha256,
        registration_id: Uuid::new_v4(),
    };
    let mut transaction = MediaResolveTransaction {
        system_identifier: actual.system_identifier().into(),
        database_oid: actual.database_oid(),
        full_xid: actual.xid().into(),
        backend_pid: actual.backend_pid(),
        backend_started_unix_micros: actual.backend_started_at().timestamp_micros(),
        producer_instance_id: instance,
        settlement_reference: settlement_reference(r.binding(), guard_digest).unwrap(),
        effect_binding_digest: guard_digest,
    };
    let settled = resolve_transaction(&p, instance, &binding, &transaction)
        .await
        .unwrap();
    assert_eq!(settled.state, MediaTransactionState::Committed);
    // This committed read-only fixture has no command receipt. Absence never
    // rewrites its transaction outcome into an asserted rollback.
    assert!(settled.receipt_evidence.is_empty());
    binding.request_id = Uuid::new_v4();
    assert!(matches!(
        resolve_transaction(&p, instance, &binding, &transaction).await,
        Err(CommandError::Denied)
    ));
    binding.request_id = i.request_id;
    assert!(matches!(
        resolve_transaction(&p, Uuid::new_v4(), &binding, &transaction).await,
        Err(CommandError::InvalidInput)
    ));
    transaction.system_identifier = "1".into();
    assert_eq!(
        resolve_transaction(&p, instance, &binding, &transaction)
            .await
            .unwrap()
            .state,
        MediaTransactionState::Unknown
    );
    p.close().await;
}
