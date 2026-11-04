//! Actual Auth/Friend owner enrollment, original frame, Root ledger and Concert SQL.
//! These component tests do not qualify HTTP or an absent production role owner.
#[path = "operations_live_support/capture.rs"]
mod capture;
#[path = "operations_live_support/environment.rs"]
mod environment;
#[path = "operations_live_support/owners.rs"]
mod owners;
use anyhow::Context;
use concert_application::commands::{CommandError, FingerprintKeys, SigningKey};
use concert_service::{
    adapters::out::relationship_authority::guarded::{
        CanonicalOwnerFence, ChildRecoveryReference, OwnerGuardScope, ScopeRegistry,
    },
    operations_live::{
        audience::{committed_audience, OperationsAudience},
        settlement::OperationsNativeResolver,
        OperationsError, OperationsOwners, OperationsReply, OperationsRuntime,
    },
    product_rpc::{
        scope_ledger::{
            ChildIntentSettler, ChildRecoveryKey, ChildRecoveryKeys, PgScopeLedger, ScopeKey,
        },
        settlement::{NativeTransactionResolver, ProducerSettlementService, POLICY},
    },
};
use platform::{
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard_without_delegation,
};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;
struct NoChildren;
#[async_trait::async_trait]
impl ChildIntentSettler for NoChildren {
    async fn recover_one(&self, _: &ChildRecoveryReference) -> Result<(), Status> {
        Err(Status::unavailable("Unexpected child intent"))
    }
}
struct Fixture {
    auth: PgPool,
    friend: PgPool,
    concert: PgPool,
    actor: Uuid,
    family: Uuid,
    subject: Uuid,
    event: Uuid,
    ledger: PgScopeLedger,
    fence: CanonicalOwnerFence,
    instance: Uuid,
    runtime: OperationsRuntime,
    _registry: Arc<ScopeRegistry>,
    _auth: owners::Running,
    _friend: owners::Running,
    task: tokio::task::JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
    }
}
fn incoming(
    listener: tokio::net::TcpListener,
) -> impl futures::Stream<Item = Result<tokio::net::TcpStream, std::io::Error>> + Send + 'static {
    futures::stream::unfold(listener, |listener| async {
        let result = listener.accept().await.map(|(s, _)| s);
        Some((result, listener))
    })
}
impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        let auth = environment::pool("auth").await?;
        let friend = environment::pool("friend").await?;
        let concert = environment::pool("concert").await?;
        let (actor, family, subjects) = environment::identities(&auth, 1).await?;
        let subject = subjects[0];
        let event = Uuid::new_v4();
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Operations native','OPEN')").bind(event).bind(format!("ops-{}",event.simple())).execute(&concert).await?;
        for user in [actor, subject] {
            sqlx::query("insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')").bind(event).bind(user).execute(&concert).await?;
        }
        let instance = Uuid::new_v4();
        let keys = Arc::new(ChildRecoveryKeys::new(
            ChildRecoveryKey::new("operations-native-v1", &[89; 32])?,
            vec![],
        )?);
        let ledger = PgScopeLedger::new(concert.clone(), instance, keys, Arc::new(NoChildren))?;
        let registry = ScopeRegistry::new(instance, Arc::new(ledger.clone()))?;
        let service = guard_without_delegation(
            &POLICY,
            ProducerSettlementService::new(
                ledger.clone(),
                Arc::new(OperationsNativeResolver::new(concert.clone(), instance)?),
            )
            .server(),
            CallerRegistry::new()
                .register(Workload::Auth, "auth-fixture")?
                .register(Workload::Friend, "friend-fixture")?,
        )?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let root_url = format!("https://{}", listener.local_addr()?);
        let server = tonic::transport::Server::builder()
            .tls_config(environment::tls(Workload::Concert)?.server_config()?)?
            .add_service(service);
        let task = tokio::spawn(async move {
            server
                .serve_with_incoming(incoming(listener))
                .await
                .expect("Actual Root settlement server");
        });
        let running_auth = owners::auth(auth.clone(), &root_url).await?;
        let running_friend = owners::friend(friend.clone(), &root_url).await?;
        let fence = CanonicalOwnerFence::connect(
            &running_auth.url,
            &running_friend.url,
            "concert-fixture",
            &environment::tls(Workload::Concert)?,
            owners::signer()?,
            registry.clone(),
        )?;
        let fingerprints = Arc::new(FingerprintKeys::new(
            SigningKey::new("operations-native-v1", &[91; 32])?,
            vec![],
        )?);
        let runtime = OperationsRuntime::new(
            concert.clone(),
            OperationsOwners::default(),
            fingerprints,
            &[92; 32],
        )?;
        Ok(Self {
            auth,
            friend,
            concert,
            actor,
            family,
            subject,
            event,
            ledger,
            fence,
            instance,
            runtime,
            _registry: registry,
            _auth: running_auth,
            _friend: running_friend,
            task,
        })
    }
    async fn acquire(
        &self,
        operation: &str,
        method: &str,
        suffix: &str,
        body: Vec<u8>,
        key: Option<Uuid>,
    ) -> anyhow::Result<(
        Arc<OwnerGuardScope>,
        concert_service::product_rpc::proto::ProductRequest,
    )> {
        let (binding, frame) = capture::binding(
            self.actor,
            self.family,
            self.event,
            operation,
            method,
            suffix,
            body,
            key,
        )
        .await?;
        let scope = self.fence.acquire(binding).await?;
        let value = scope.producer_scope();
        let delivery_secret: [u8; 32] = Sha256::digest(value.scope_id.as_bytes()).into();
        self.ledger
            .bind_delivery(
                &ScopeKey {
                    instance: self.instance,
                    scope: Uuid::parse_str(&value.scope_id)?,
                    binding_digest: value.binding_digest.as_slice().try_into()?,
                },
                &delivery_secret,
            )
            .await?;
        Ok((scope, frame))
    }
    async fn dispatch(
        &self,
        operation: &str,
        method: &str,
        suffix: &str,
        body: Vec<u8>,
        key: Option<Uuid>,
    ) -> anyhow::Result<(
        Result<OperationsReply, OperationsError>,
        Arc<OwnerGuardScope>,
    )> {
        let (scope, frame) = self.acquire(operation, method, suffix, body, key).await?;
        Ok((self.runtime.dispatch(scope.clone(), frame).await, scope))
    }
    async fn finish(&self, scope: &OwnerGuardScope) -> anyhow::Result<()> {
        scope.mark_serialized().await?;
        scope.finish_response().await.with_context(|| {
            format!(
                "Operations terminal settlement: operation={}, request={}",
                scope.binding().operation(),
                scope.binding().request_id()
            )
        })?;
        for pool in [&self.auth, &self.friend, &self.concert] {
            let count: i64 = sqlx::query_scalar(
                "select count(*) from pg_prepared_xacts where database=current_database()",
            )
            .fetch_one(pool)
            .await?;
            assert_eq!(count, 0, "Actual owner enrollment must settle");
        }
        Ok(())
    }
    fn report(&self, target_type: &str, media: Vec<Uuid>) -> Vec<u8> {
        serde_json::to_vec(&json!({"target_type":target_type,"target_id":self.subject,"reason":"SPAM","text":"private reporter canary","evidence_media_ids":media})).unwrap()
    }
}
#[tokio::test]
#[ignore = "frozen owned three-database PG16/mTLS controller only"]
async fn actual_member_report_tracks_transaction_and_private_outbox_then_replays(
) -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let key = Uuid::new_v4();
    let (reply, scope) = f
        .dispatch(
            "createConcertReport",
            "POST",
            "reports",
            f.report("USER", vec![]),
            Some(key),
        )
        .await?;
    let reply = reply?;
    assert_eq!(reply.status, 202);
    assert_eq!(reply.body["state"], "SUCCEEDED");
    let receipt = reply.body["receipt_id"].as_str().unwrap().to_string();
    let tracked:i64=sqlx::query_scalar("select count(*) from concert_product_owner_transactions where instance_id=$1 and confirmed_at is not null and auth_registration_id is not null and friend_registration_id is not null").bind(f.instance).fetch_one(&f.concert).await?;
    assert_eq!(tracked, 1);
    let outbox: Uuid =
        sqlx::query_scalar("select outbox_id from concert_outbox where receipt_id=$1")
            .bind(Uuid::parse_str(&receipt)?)
            .fetch_one(&f.concert)
            .await?;
    match committed_audience(&f.concert, outbox).await? {
        OperationsAudience::ReporterAndCurrentModerators {
            event, reporter, ..
        } => {
            assert_eq!(event, f.event);
            assert_eq!(reporter, f.actor);
        }
        _ => panic!("Report must not become public event audience"),
    };
    f.finish(&scope).await?;
    let (reply, scope) = f
        .dispatch(
            "createConcertReport",
            "POST",
            "reports",
            f.report("USER", vec![]),
            Some(key),
        )
        .await?;
    assert_eq!(reply?.body["receipt_id"], receipt);
    f.finish(&scope).await?;
    let count: i64 = sqlx::query_scalar("select count(*) from concert_outbox where receipt_id=$1")
        .bind(Uuid::parse_str(&receipt)?)
        .fetch_one(&f.concert)
        .await?;
    assert_eq!(count, 1);
    let (reply, scope) = f
        .dispatch("listOwnConcertReports", "GET", "me/reports", vec![], None)
        .await?;
    let reply = reply?;
    assert_eq!(reply.status, 200);
    assert_eq!(reply.body["items"].as_array().unwrap().len(), 1);
    for field in ["reporter_id", "reason", "text", "evidence_media_ids"] {
        assert!(reply.body["items"][0].get(field).is_none());
    }
    f.finish(&scope).await?;
    Ok(())
}
#[tokio::test]
#[ignore = "frozen owned three-database PG16/mTLS controller only"]
async fn absent_operator_or_evidence_provider_has_no_receipt_or_effects() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    for (operation, method, suffix, body, key) in [
        ("getConcertSettings", "GET", "admin/settings", vec![], None),
        (
            "createConcertReport",
            "POST",
            "reports",
            f.report("POST", vec![]),
            Some(Uuid::new_v4()),
        ),
        (
            "createConcertReport",
            "POST",
            "reports",
            f.report("USER", vec![Uuid::new_v4()]),
            Some(Uuid::new_v4()),
        ),
    ] {
        let (reply, scope) = f.dispatch(operation, method, suffix, body, key).await?;
        assert!(matches!(
            reply,
            Err(OperationsError::Command(CommandError::Unavailable))
        ));
        f.finish(&scope).await?;
    }
    for table in [
        "concert_reports",
        "concert_command_receipts",
        "concert_outbox",
    ] {
        let count: i64 =
            sqlx::query_scalar(&format!("select count(*) from {table} where event_id=$1"))
                .bind(f.event)
                .fetch_one(&f.concert)
                .await?;
        assert_eq!(count, 0);
    }
    let (reply, scope) = f
        .dispatch("listOwnConcertReports", "GET", "me/reports", vec![], None)
        .await?;
    assert_eq!(reply?.body["items"], json!([]));
    f.finish(&scope).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "frozen owned three-database PG16/mTLS controller only"]
async fn committed_native_witness_is_immutable_and_requires_exact_finished_scope(
) -> anyhow::Result<()> {
    use concert_service::{
        adapters::out::relationship_authority::guarded::ProducerTransaction as GuardTransaction,
        product_rpc::scope_ledger::ProducerTransaction,
    };
    use platform::actor_delegation::owner_binding::OwnerBinding;
    let f = Fixture::new().await?;
    let (reply, scope) = f
        .dispatch("listOwnConcertReports", "GET", "me/reports", vec![], None)
        .await?;
    assert_eq!(reply?.body["items"], json!([]));
    let (bound, native): (serde_json::Value, serde_json::Value) = sqlx::query_as(
        "select binding,identity from concert_operations_native_witnesses where instance_id=$1",
    )
    .bind(f.instance)
    .fetch_one(&f.concert)
    .await?;
    let binding: OwnerBinding = serde_json::from_value(bound)?;
    let identity: ProducerTransaction = serde_json::from_value(native)?;
    let resolver = OperationsNativeResolver::new(f.concert.clone(), f.instance)?;
    let resolve = resolver.resolve(&binding, &identity).await?;
    assert_eq!(
        resolve.status,
        concert_service::adapters::out::relationship_authority::guarded::proto::TransactionStatus::Unknown,
        "Native COMMIT alone cannot release an active Root scope"
    );
    assert!(resolve.receipt_evidence.is_empty());
    f.finish(&scope).await?;
    let resolved = resolver.resolve(&binding, &identity).await?;
    assert_eq!(
        resolved.status,
        concert_service::adapters::out::relationship_authority::guarded::proto::TransactionStatus::Committed
    );
    let proof: serde_json::Value = serde_json::from_slice(&resolved.receipt_evidence)?;
    assert_eq!(proof["kind"], "OPERATIONS_NATIVE_TRANSACTION_V1");
    assert!(
        proof.get("state").is_none(),
        "Native proof is not command success"
    );
    for statement in [
        "update concert_operations_native_witnesses set created_at=clock_timestamp() where instance_id=$1",
        "delete from concert_operations_native_witnesses where instance_id=$1",
        "insert into concert_operations_native_witnesses select * from concert_operations_native_witnesses where instance_id=$1",
    ] {
        let error = sqlx::query(statement).bind(f.instance).execute(&f.concert).await.unwrap_err();
        assert_eq!(error.as_database_error().and_then(|e| e.code()).as_deref(), Some("23514"));
    }
    let mut changed = binding.clone();
    changed.actor_id = f.subject;
    assert!(resolver
        .resolve(&changed, &identity)
        .await?
        .receipt_evidence
        .is_empty());
    let mut changed = identity.clone();
    changed.effect_binding_digest[0] ^= 1;
    assert!(resolver
        .resolve(&binding, &changed)
        .await?
        .receipt_evidence
        .is_empty());
    // This is an actual separate PostgreSQL COMMIT with no producer witness,
    // not an invented positive native status or registration.
    let mut tx = f.concert.begin().await?;
    let unwitnessed = GuardTransaction::capture_concert_in(
        &mut tx,
        identity.effect_binding_digest,
        f.instance,
        identity.settlement_reference.clone(),
    )
    .await?;
    tx.commit().await?;
    let mut unproven = identity.clone();
    unproven.system_identifier = unwitnessed.system_identifier;
    unproven.database_oid = unwitnessed.database_oid;
    unproven.full_xid = unwitnessed.full_xid;
    unproven.backend_pid = unwitnessed.backend_pid;
    unproven.backend_started_unix_micros = unwitnessed.backend_started.timestamp_micros();
    let resolved = resolver.resolve(&binding, &unproven).await?;
    assert_eq!(resolved.status, concert_service::adapters::out::relationship_authority::guarded::proto::TransactionStatus::Unknown);
    assert!(resolved.receipt_evidence.is_empty());
    Ok(())
}

#[tokio::test]
#[ignore = "frozen owned three-database PG16/mTLS controller only"]
async fn failed_command_commits_native_witness_without_inventing_success() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let (reply, scope) = f
        .dispatch(
            "createConcertReport",
            "POST",
            "reports",
            f.report("USER", vec![]),
            Some(Uuid::new_v4()),
        )
        .await?;
    assert_eq!(reply?.body["state"], "SUCCEEDED");
    f.finish(&scope).await?;
    let key = Uuid::new_v4();
    let (reply, scope) = f
        .dispatch(
            "createConcertReport",
            "POST",
            "reports",
            f.report("USER", vec![]),
            Some(key),
        )
        .await?;
    assert_eq!(reply?.body["state"], "FAILED");
    f.finish(&scope).await?;
    let state: String = sqlx::query_scalar("select state from concert_command_receipts where actor_id=$1 and event_id=$2 and idempotency_key=$3")
        .bind(f.actor).bind(f.event).bind(key).fetch_one(&f.concert).await?;
    assert_eq!(state, "FAILED");
    let witness_count: i64 = sqlx::query_scalar(
        "select count(*) from concert_operations_native_witnesses where instance_id=$1",
    )
    .bind(f.instance)
    .fetch_one(&f.concert)
    .await?;
    assert_eq!(witness_count, 2);
    let outbox_count: i64 =
        sqlx::query_scalar("select count(*) from concert_outbox where event_id=$1")
            .bind(f.event)
            .fetch_one(&f.concert)
            .await?;
    assert_eq!(
        outbox_count, 1,
        "FAILED receipt creates no additional effect invalidation"
    );
    Ok(())
}

#[tokio::test]
#[ignore = "frozen owned three-database PG16/mTLS controller only"]
async fn exact_parent_frame_rejects_rebinding_before_transaction_registration() -> anyhow::Result<()>
{
    let f = Fixture::new().await?;
    let (scope, mut frame) = f
        .acquire("listOwnConcertReports", "GET", "me/reports", vec![], None)
        .await?;
    frame.http_target.push_str("?limit=1");
    assert!(matches!(
        f.runtime.dispatch(scope.clone(), frame).await,
        Err(OperationsError::Command(CommandError::Denied))
    ));
    let count: i64 = sqlx::query_scalar(
        "select count(*) from concert_product_owner_transactions where instance_id=$1",
    )
    .bind(f.instance)
    .fetch_one(&f.concert)
    .await?;
    assert_eq!(count, 0);
    f.finish(&scope).await?;
    Ok(())
}
#[tokio::test]
#[ignore = "frozen owned three-database PG16/mTLS controller only"]
async fn actual_auth_revocation_waits_for_terminal_and_replay_rechecks_authority(
) -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let key = Uuid::new_v4();
    let (reply, scope) = f
        .dispatch(
            "createConcertReport",
            "POST",
            "reports",
            f.report("USER", vec![]),
            Some(key),
        )
        .await?;
    assert_eq!(reply?.body["state"], "SUCCEEDED");
    let mut connection = f.auth.acquire().await?;
    let pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *connection)
        .await?;
    let actor = f.actor;
    let writer = tokio::spawn(async move {
        let mut tx = sqlx::Acquire::begin(&mut *connection).await?;
        sqlx::query("set local lock_timeout='2s'")
            .execute(&mut *tx)
            .await?;
        sqlx::query("update users set status='SUSPENDED' where id=$1")
            .bind(actor)
            .execute(&mut *tx)
            .await?;
        tx.commit().await
    });
    let stop = std::time::Instant::now() + std::time::Duration::from_millis(750);
    loop {
        let blocked: bool = sqlx::query_scalar("select cardinality(pg_blocking_pids($1))>0")
            .bind(pid)
            .fetch_one(&f.auth)
            .await?;
        if blocked {
            break;
        }
        anyhow::ensure!(
            std::time::Instant::now() < stop,
            "Actual Auth revocation did not wait on retained owner exclusion"
        );
        tokio::time::sleep(std::time::Duration::from_millis(2)).await;
    }
    assert!(!writer.is_finished());
    f.finish(&scope).await?;
    tokio::time::timeout(std::time::Duration::from_secs(2), writer).await???;
    assert!(
        f.dispatch(
            "createConcertReport",
            "POST",
            "reports",
            f.report("USER", vec![]),
            Some(key)
        )
        .await
        .is_err(),
        "Completed receipt cannot bypass fresh canonical authorization"
    );
    let count: i64 = sqlx::query_scalar("select count(*) from concert_reports where event_id=$1")
        .bind(f.event)
        .fetch_one(&f.concert)
        .await?;
    assert_eq!(count, 1);
    Ok(())
}
