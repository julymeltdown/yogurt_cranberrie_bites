//! Actual Auth/Friend owner stores and mTLS, signed Gateway parent, real Root
//! ledger and native PG transactions. Business failure producer below is an
//! explicit receipt-only fixture: it grants no Party policy/resource authority
//! and does not qualify Root's separately owned factory or user journey.
#[path = "party_native_settlement/capture.rs"]
mod capture;
#[path = "party_native_settlement/environment.rs"]
mod environment;
#[path = "party_native_settlement/owners.rs"]
mod owners;
use anyhow::Context;
use async_trait::async_trait;
use concert_application::commands::{
    CanonicalRequest, CanonicalValue, CommandError, Completion, FingerprintKeys, Invalidation,
    RegisteredOperation, SafeFailure, Scope, SigningKey,
};
use concert_service::{
    adapters::out::{
        relationship_authority::guarded::{
            proto, CanonicalOwnerFence, ChildRecoveryReference, OwnerGuardScope,
            ProducerTransaction as GuardTransaction, ScopeRegistry,
        },
        repository::receipt::{CommandProducer, PgCommandRepository},
    },
    party_live::{
        settlement::{register_in, NativeRole},
        PartyLiveBinding, PartyNativeResolver, PARTY_OPERATIONS,
    },
    product_rpc::{
        scope_ledger::{
            ChildIntentSettler, ChildRecoveryKey, ChildRecoveryKeys, PgScopeLedger,
            ProducerTransaction, ScopeKey,
        },
        settlement::{NativeTransactionResolver, ProducerSettlementService, POLICY},
    },
};
use platform::{
    actor_delegation::owner_binding::OwnerBinding,
    grpc_auth::{CallerRegistry, Workload},
    grpc_method_policy::guard_without_delegation,
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;
struct NoChildren;
#[async_trait]
impl ChildIntentSettler for NoChildren {
    async fn recover_one(&self, _: &ChildRecoveryReference) -> Result<(), Status> {
        Err(Status::unavailable("Unexpected Party child intent"))
    }
}
fn incoming(
    l: tokio::net::TcpListener,
) -> impl futures::Stream<Item = Result<tokio::net::TcpStream, std::io::Error>> + Send + 'static {
    futures::stream::unfold(l, |l| async {
        let r = l.accept().await.map(|(s, _)| s);
        Some((r, l))
    })
}
struct Fixture {
    auth: PgPool,
    friend: PgPool,
    concert: PgPool,
    actor: Uuid,
    family: Uuid,
    event: Uuid,
    instance: Uuid,
    ledger: PgScopeLedger,
    fence: CanonicalOwnerFence,
    keys: FingerprintKeys,
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
impl Fixture {
    async fn new() -> anyhow::Result<Self> {
        let auth = environment::pool("auth").await?;
        let friend = environment::pool("friend").await?;
        let concert = environment::pool("concert").await?;
        let (actor, family, _) = environment::identities(&auth, 0).await?;
        let event = Uuid::new_v4();
        sqlx::query("insert into concert_events(event_id,slug,title,phase) values($1,$2,'Party settlement component','OPEN')").bind(event).bind(format!("party-native-{}",event.simple())).execute(&concert).await?;
        sqlx::query(
            "insert into concert_event_admissions(event_id,user_id,state) values($1,$2,'ACTIVE')",
        )
        .bind(event)
        .bind(actor)
        .execute(&concert)
        .await?;
        let instance = Uuid::new_v4();
        let keys = Arc::new(ChildRecoveryKeys::new(
            ChildRecoveryKey::new("party-native-v1", &[89; 32])?,
            vec![],
        )?);
        let ledger = PgScopeLedger::new(concert.clone(), instance, keys, Arc::new(NoChildren))?;
        let registry = ScopeRegistry::new(instance, Arc::new(ledger.clone()))?;
        let service = guard_without_delegation(
            &POLICY,
            ProducerSettlementService::new(
                ledger.clone(),
                Arc::new(PartyNativeResolver::new(concert.clone(), instance)?),
            )
            .server(),
            CallerRegistry::new()
                .register(Workload::Auth, "auth-fixture")?
                .register(Workload::Friend, "friend-fixture")?,
        )?;
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await?;
        let url = format!("https://{}", listener.local_addr()?);
        let server = tonic::transport::Server::builder()
            .tls_config(environment::tls(Workload::Concert)?.server_config()?)?
            .add_service(service);
        let task = tokio::spawn(async move {
            server
                .serve_with_incoming(incoming(listener))
                .await
                .expect("Actual Party settlement service");
        });
        let running_auth = owners::auth(auth.clone(), &url).await?;
        let running_friend = owners::friend(friend.clone(), &url).await?;
        let fence = CanonicalOwnerFence::connect(
            &running_auth.url,
            &running_friend.url,
            "concert-fixture",
            &environment::tls(Workload::Concert)?,
            owners::signer()?,
            registry.clone(),
        )?;
        Ok(Self {
            auth,
            friend,
            concert,
            actor,
            family,
            event,
            instance,
            ledger,
            fence,
            keys: FingerprintKeys::new(SigningKey::new("party-native-v1", &[91; 32])?, vec![])?,
            _registry: registry,
            _auth: running_auth,
            _friend: running_friend,
            task,
        })
    }
    async fn acquire(
        &self,
        operation: &'static str,
        key: Option<Uuid>,
    ) -> anyhow::Result<(Arc<OwnerGuardScope>, PartyLiveBinding)> {
        self.acquire_marked(operation, key, 1).await
    }
    async fn acquire_marked(
        &self,
        operation: &'static str,
        key: Option<Uuid>,
        marker: i64,
    ) -> anyhow::Result<(Arc<OwnerGuardScope>, PartyLiveBinding)> {
        let (method, suffix) = route(operation);
        let body = if method == "POST" {
            serde_json::to_vec(&json!({"fixture_marker":marker}))?
        } else {
            vec![]
        };
        let (binding, frame) = capture::binding(
            self.actor,
            self.family,
            self.event,
            operation,
            method,
            &suffix,
            body,
            key,
        )
        .await?;
        let scope = self.fence.acquire(binding).await?;
        let p = scope.producer_scope();
        self.ledger
            .bind_delivery(
                &self.scope_key(&scope)?,
                &Sha256::digest(p.scope_id.as_bytes()),
            )
            .await?;
        let binding =
            PartyLiveBinding::from_owner_guard(scope.binding(), operation, &frame.http_body, key)?;
        Ok((scope, binding))
    }
    fn scope_key(&self, scope: &OwnerGuardScope) -> anyhow::Result<ScopeKey> {
        let p = scope.producer_scope();
        Ok(ScopeKey {
            instance: self.instance,
            scope: Uuid::parse_str(&p.scope_id)?,
            binding_digest: p.binding_digest.as_slice().try_into()?,
        })
    }
    async fn finish(&self, scope: &OwnerGuardScope) -> anyhow::Result<()> {
        scope.mark_serialized().await?;
        scope
            .finish_response()
            .await
            .context("actual Party owner settlement")?;
        for p in [&self.auth, &self.friend, &self.concert] {
            let count: i64 = sqlx::query_scalar(
                "select count(*) from pg_prepared_xacts where database=current_database()",
            )
            .fetch_one(p)
            .await?;
            assert_eq!(count, 0, "actual owner exclusions remain prepared");
        }
        Ok(())
    }
    fn resolver(&self) -> PartyNativeResolver {
        PartyNativeResolver::new(self.concert.clone(), self.instance).unwrap()
    }
    async fn receipt_command(
        &self,
        scope: Arc<OwnerGuardScope>,
        binding: Arc<PartyLiveBinding>,
        marker: i64,
    ) -> anyhow::Result<concert_application::commands::SafeReceipt> {
        let event = self.event.to_string();
        let id = fixed().to_string();
        let user = fixed_user().to_string();
        let mut params = vec![("eventId", event.as_str())];
        match binding.operation() {
            "createConcertParty" => (),
            "decideConcertApplication" | "cancelConcertApplication" => {
                params.push(("applicationId", id.as_str()))
            }
            "removeConcertMember" => {
                params.push(("partyId", id.as_str()));
                params.push(("userId", user.as_str()));
            }
            _ => params.push(("partyId", id.as_str())),
        }
        let b = CanonicalValue::Object(vec![(
            "fixture_marker".into(),
            CanonicalValue::Integer(marker),
        )]);
        let request = CanonicalRequest::json(
            Scope {
                actor: self.actor,
                event: self.event,
                operation: RegisteredOperation::find(binding.operation())?,
                key: binding.idempotency_key().unwrap(),
            },
            &params,
            &binding
                .target()
                .split_once('?')
                .map(|(_, q)| {
                    q.split('&')
                        .filter_map(|s| s.split_once('='))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default(),
            if binding.operation().starts_with("cancelConcertApplication")
                || binding.operation() == "removeConcertMember"
            {
                None
            } else {
                Some(&b)
            },
        )?;
        let fixture = ReceiptOnlyFailure { scope, binding };
        Ok(PgCommandRepository::new(self.concert.clone())
            .execute(&request, fixed(), &self.keys, &fixture)
            .await?)
    }
}
fn fixed() -> Uuid {
    Uuid::from_u128(0x12340000123412341234123400000001)
}
fn fixed_user() -> Uuid {
    Uuid::from_u128(0x12340000123412341234123400000002)
}
fn route(operation: &str) -> (&'static str, String) {
    let id = fixed();
    match operation {
        "listConcertParties" => ("GET", "parties".into()),
        "getConcertParty" => ("GET", format!("parties/{id}")),
        "createConcertParty" => ("POST", "parties".into()),
        "applyConcertParty" => ("POST", format!("parties/{id}/applications")),
        "decideConcertApplication" => ("POST", format!("party-applications/{id}/decision")),
        "cancelConcertApplication" => (
            "DELETE",
            format!("party-applications/{id}?expected_revision=1"),
        ),
        "removeConcertMember" => (
            "DELETE",
            format!(
                "parties/{id}/members/{}?expected_revision=1&party_revision=1",
                fixed_user()
            ),
        ),
        "cancelConcertParty" => ("POST", format!("parties/{id}/cancel")),
        _ => panic!("closed fixture operation"),
    }
}
struct ReceiptOnlyFailure {
    scope: Arc<OwnerGuardScope>,
    binding: Arc<PartyLiveBinding>,
}
#[async_trait]
impl CommandProducer for ReceiptOnlyFailure {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if s.actor != self.binding.actor()
            || s.event != self.binding.event()
            || s.operation.id() != self.binding.operation()
            || Some(s.key) != self.binding.idempotency_key()
            || target != fixed()
        {
            return Err(CommandError::Denied);
        }
        register_in(&self.scope, c, &self.binding, NativeRole::Command)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        Ok(())
    }
    async fn apply(
        &self,
        _: &mut PgConnection,
        _: Scope,
        _: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        Ok((Completion::Failed(SafeFailure::InvalidTransition), None))
    }
}
fn stored(native: &GuardTransaction) -> ProducerTransaction {
    ProducerTransaction {
        producer: "CONCERT".into(),
        system_identifier: native.system_identifier.clone(),
        database_oid: native.database_oid,
        full_xid: native.full_xid.clone(),
        backend_pid: native.backend_pid,
        backend_started_unix_micros: native.backend_started.timestamp_micros(),
        effect_binding_digest: native.effect_binding_digest,
        producer_instance: native.producer_instance_id,
        settlement_reference: native.settlement_reference.clone(),
    }
}

#[test]
fn pure_party_roles_are_closed() {
    assert_eq!(PARTY_OPERATIONS.len(), 8);
    let roles = PARTY_OPERATIONS
        .iter()
        .map(|op| NativeRole::for_operation(op).unwrap())
        .collect::<Vec<_>>();
    assert_eq!(roles.iter().filter(|r| **r == NativeRole::Read).count(), 2);
    assert_eq!(
        roles.iter().filter(|r| **r == NativeRole::Command).count(),
        6
    );
    assert!(NativeRole::for_operation("updateConcertParty").is_err());
    assert!(serde_json::from_str::<NativeRole>("\"GENERIC\"").is_err());
}
#[tokio::test]
async fn pure_legacy_hash_and_wrong_producer_have_no_positive_default() -> anyhow::Result<()> {
    let pool = sqlx::postgres::PgPoolOptions::new()
        .connect_lazy("postgres://unused@127.0.0.1:1/unused")?;
    assert!(PartyNativeResolver::new(pool.clone(), Uuid::nil()).is_err());
    let instance = Uuid::new_v4();
    let binding = OwnerBinding {
        actor_id: Uuid::new_v4(),
        event_id: Uuid::new_v4(),
        session_family_id: Uuid::new_v4(),
        request_id: Uuid::new_v4(),
        operation: "listConcertParties".into(),
        expires_unix_micros: 1,
        parent_digest: [1; 32],
        parent_body_digest: [2; 32],
        parent_method: "/concert.product.v1.ProductService/Dispatch".into(),
        http_method: "GET".into(),
        http_target: String::new(),
        body_digest: [3; 32],
    };
    let binding = OwnerBinding {
        http_target: format!("/api/concert/events/{}/parties", binding.event_id),
        ..binding
    };
    let mut identity = ProducerTransaction {
        producer: "CONCERT".into(),
        system_identifier: "1".into(),
        database_oid: 1,
        full_xid: "10".into(),
        backend_pid: 1,
        backend_started_unix_micros: 1,
        effect_binding_digest: [4; 32],
        producer_instance: instance,
        settlement_reference: br#"{"kind":"PARTY_REQUEST_V1"}"#.to_vec(),
    };
    let resolver = PartyNativeResolver::new(pool, instance)?;
    for _ in 0..2 {
        let proof = tokio::time::timeout(
            std::time::Duration::from_millis(100),
            resolver.resolve(&binding, &identity),
        )
        .await??;
        assert_eq!(proof.status, proto::TransactionStatus::Unknown);
        assert!(proof.receipt_evidence.is_empty());
        identity.producer = "POST".into();
    }
    Ok(())
}

#[tokio::test]
#[ignore = "fresh owned PG16/mTLS three-database allocation only"]
async fn native_party_all_eight_operations_bind_real_commit_and_failed_receipt(
) -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    for op in PARTY_OPERATIONS {
        let role = NativeRole::for_operation(op)?;
        let key = (role == NativeRole::Command).then(Uuid::new_v4);
        let (scope, binding) = f.acquire(op, key).await?;
        if role == NativeRole::Command {
            let receipt = f
                .receipt_command(scope.clone(), Arc::new(binding), 1)
                .await?;
            assert_eq!(
                receipt.state,
                concert_application::commands::ReceiptState::Failed
            );
        } else {
            let mut tx = f.concert.begin().await?;
            let native = register_in(&scope, &mut tx, &binding, role).await?;
            assert_eq!(
                native.full_xid,
                sqlx::query_scalar::<_, String>("select pg_current_xact_id()::text")
                    .fetch_one(&mut *tx)
                    .await?
            );
            tx.commit().await?;
        }
        let p = scope.producer_scope();
        let identity:Value=sqlx::query_scalar("select identity from concert_party_native_witnesses where instance_id=$1 and scope_id=$2 limit 1").bind(f.instance).bind(Uuid::parse_str(&p.scope_id)?).fetch_one(&f.concert).await?;
        let identity: ProducerTransaction = serde_json::from_value(identity)?;
        let binding = scope.binding().owner_binding();
        assert_eq!(
            f.resolver().resolve(&binding, &identity).await?.status,
            proto::TransactionStatus::Unknown,
            "active scope is not settlement"
        );
        f.finish(&scope).await?;
        let proof = f.resolver().resolve(&binding, &identity).await?;
        assert_eq!(proof.status, proto::TransactionStatus::Committed);
        let v: Value = serde_json::from_slice(&proof.receipt_evidence)?;
        assert_eq!(v["role"], role_as_json(role));
        assert!(v.get("success").is_none());
        if role == NativeRole::Command {
            assert_eq!(v["receipt"]["state"], "FAILED");
        } else {
            assert!(v["receipt"].is_null());
        }
    }
    Ok(())
}
fn role_as_json(role: NativeRole) -> Value {
    serde_json::to_value(role).unwrap()
}

#[tokio::test]
#[ignore = "fresh owned PG16/mTLS three-database allocation only"]
async fn native_party_lookup_and_read_cannot_commit_command_effects() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    for op in ["decideConcertApplication", "cancelConcertApplication"] {
        let (scope, binding) = f.acquire(op, Some(Uuid::new_v4())).await?;
        let mut tx = f.concert.begin().await?;
        let native = register_in(&scope, &mut tx, &binding, NativeRole::Lookup).await?;
        tx.commit().await?;
        let owner = scope.binding().owner_binding();
        f.finish(&scope).await?;
        let proof = f.resolver().resolve(&owner, &stored(&native)).await?;
        assert_eq!(proof.status, proto::TransactionStatus::Committed);
        let proof: Value = serde_json::from_slice(&proof.receipt_evidence)?;
        assert_eq!(proof["role"], "LOOKUP");
        assert!(proof["receipt"].is_null());
    }
    // Actual receipt insert is explicitly forbidden in a lookup transaction, both
    // after registration and when someone tries to attach the witness afterward.
    for before in [false, true] {
        let (scope, binding) = f
            .acquire("decideConcertApplication", Some(Uuid::new_v4()))
            .await?;
        let mut tx = f.concert.begin().await?;
        if before {
            insert_pending_fixture(&mut tx, &binding).await?;
        }
        let native = register_in(&scope, &mut tx, &binding, NativeRole::Lookup).await?;
        if !before {
            insert_pending_fixture(&mut tx, &binding).await?;
        }
        assert!(
            tx.commit().await.is_err(),
            "LOOKUP committed an unreceipted command effect"
        );
        f.finish(&scope).await?;
        assert_eq!(
            f.resolver()
                .resolve(&scope.binding().owner_binding(), &stored(&native))
                .await?
                .status,
            proto::TransactionStatus::Aborted
        );
    }
    let (scope, binding) = f.acquire("listConcertParties", None).await?;
    let mut tx = f.concert.begin().await?;
    let native = register_in(&scope, &mut tx, &binding, NativeRole::Read).await?;
    // An otherwise valid receipt metadata row belongs to no allowed READ effect.
    let key = Uuid::new_v4();
    sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state,created_at,expires_at) select $1,$2,$3,'createConcertParty',$4,$5,$6,'PENDING',t,t+interval '1 day' from(select clock_timestamp() t) q")
  .bind(Uuid::new_v4()).bind(f.actor).bind(f.event).bind(key).bind(format!("hmac-sha256:v1:explicit-metadata-fixture:{}","00".repeat(32))).bind(fixed()).execute(&mut *tx).await?;
    assert!(tx.commit().await.is_err());
    f.finish(&scope).await?;
    assert_eq!(
        f.resolver()
            .resolve(&scope.binding().owner_binding(), &stored(&native))
            .await?
            .status,
        proto::TransactionStatus::Aborted
    );
    Ok(())
}
async fn insert_pending_fixture(c: &mut PgConnection, b: &PartyLiveBinding) -> anyhow::Result<()> {
    // Deliberately synthetic receipt metadata for negative SQL guards only. No
    // forged fingerprint is ever used to claim a successful Party business effect.
    sqlx::query("insert into concert_command_receipts(receipt_id,actor_id,event_id,operation_id,idempotency_key,request_hash,target_id,state,created_at,expires_at) select $1,$2,$3,$4,$5,$6,$7,'PENDING',t,t+interval '1 day' from(select clock_timestamp() t) q")
  .bind(Uuid::new_v4()).bind(b.actor()).bind(b.event()).bind(b.operation()).bind(b.idempotency_key().unwrap()).bind(format!("hmac-sha256:v1:explicit-metadata-fixture:{}","00".repeat(32))).bind(fixed()).execute(c).await?;
    Ok(())
}

#[tokio::test]
#[ignore = "fresh owned PG16/mTLS three-database allocation only"]
async fn native_party_exact_scope_identity_and_immutable_witness_fail_closed() -> anyhow::Result<()>
{
    let f = Fixture::new().await?;
    let (scope, b) = f.acquire("listConcertParties", None).await?;
    let mut tx = f.concert.begin().await?;
    let native = register_in(&scope, &mut tx, &b, NativeRole::Read).await?;
    let original: Value = sqlx::query_scalar(
        "select to_jsonb(w) from concert_party_native_witnesses w where instance_id=$1",
    )
    .bind(f.instance)
    .fetch_one(&mut *tx)
    .await?;
    tx.commit().await?;
    let owner = scope.binding().owner_binding();
    f.finish(&scope).await?;
    let identity = stored(&native);
    assert_eq!(
        f.resolver().resolve(&owner, &identity).await?.status,
        proto::TransactionStatus::Committed
    );
    for sql in ["update concert_party_native_witnesses set created_at=clock_timestamp() where instance_id=$1","delete from concert_party_native_witnesses where instance_id=$1"] {
  assert_eq!(sqlx::query(sql).bind(f.instance).execute(&f.concert).await.unwrap_err().as_database_error().and_then(|e|e.code()).as_deref(),Some("23514"));
 }
    assert!(sqlx::query("truncate concert_party_native_witnesses")
        .execute(&f.concert)
        .await
        .is_err());
    let after: Value = sqlx::query_scalar(
        "select to_jsonb(w) from concert_party_native_witnesses w where instance_id=$1",
    )
    .bind(f.instance)
    .fetch_one(&f.concert)
    .await?;
    assert_eq!(original, after);
    for i in 0..9 {
        let mut changed = identity.clone();
        match i {
            0 => changed.system_identifier = "999999".into(),
            1 => changed.database_oid += 1,
            2 => changed.backend_pid += 1,
            3 => changed.backend_started_unix_micros += 1,
            4 => changed.full_xid = (native.full_xid.parse::<u64>()? + 1).to_string(),
            5 => changed.producer_instance = Uuid::new_v4(),
            6 => changed.effect_binding_digest[0] ^= 1,
            7 => changed.settlement_reference.push(b' '),
            _ => changed.producer = "POST".into(),
        }
        let p = f.resolver().resolve(&owner, &changed).await?;
        assert_eq!(p.status, proto::TransactionStatus::Unknown);
        assert!(p.receipt_evidence.is_empty());
    }
    let mut wrong = owner.clone();
    wrong.request_id = Uuid::new_v4();
    assert_eq!(
        f.resolver().resolve(&wrong, &identity).await?.status,
        proto::TransactionStatus::Unknown
    );
    // A real XID cannot carry another transaction's witness, even as a copied row.
    let error=sqlx::query("insert into concert_party_native_witnesses select * from concert_party_native_witnesses where instance_id=$1").bind(f.instance).execute(&f.concert).await.unwrap_err();
    assert_eq!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("23514")
    );
    // No receipt on the real command transaction: deferred commit must reject it.
    let (scope, b) = f
        .acquire("createConcertParty", Some(Uuid::new_v4()))
        .await?;
    let mut tx = f.concert.begin().await?;
    let native = register_in(&scope, &mut tx, &b, NativeRole::Command).await?;
    assert!(tx.commit().await.is_err());
    f.finish(&scope).await?;
    assert_eq!(
        f.resolver()
            .resolve(&scope.binding().owner_binding(), &stored(&native))
            .await?
            .status,
        proto::TransactionStatus::Aborted
    );
    // Original authority cannot be renewed by successful early registration.
    let (expired_scope, expired_binding) = f.acquire("listConcertParties", None).await?;
    let mut expired_tx = f.concert.begin().await?;
    let expired_native = register_in(
        &expired_scope,
        &mut expired_tx,
        &expired_binding,
        NativeRole::Read,
    )
    .await?;
    tokio::time::sleep_until(
        expired_binding.deadline().stop() + std::time::Duration::from_millis(20),
    )
    .await;
    assert!(
        expired_tx.commit().await.is_err(),
        "deferred commit renewed original deadline"
    );
    f.finish(&expired_scope).await?;
    assert_eq!(
        f.resolver()
            .resolve(
                &expired_scope.binding().owner_binding(),
                &stored(&expired_native)
            )
            .await?
            .status,
        proto::TransactionStatus::Aborted
    );
    // Foreign Party binding cannot be used with this actual held scope.
    let (scope, b) = f.acquire("getConcertParty", None).await?;
    let (other_scope, other) = f.acquire("getConcertParty", None).await?;
    let mut tx = f.concert.begin().await?;
    assert!(register_in(&scope, &mut tx, &other, NativeRole::Read)
        .await
        .is_err());
    tx.rollback().await?;
    scope.mark_serialized().await?;
    scope.finish_response().await?;
    // The separately acquired scope is intentionally finished too, never dropped
    // as if its owner exclusions had been proven terminal.
    f.finish(&other_scope).await?;
    let _ = b;
    Ok(())
}

#[tokio::test]
#[ignore = "fresh owned PG16/mTLS three-database allocation only"]
async fn native_party_failed_receipt_replay_and_changed_body_remain_truthful() -> anyhow::Result<()>
{
    let f = Fixture::new().await?;
    let key = Uuid::new_v4();
    let mut receipt_id = None;
    for _ in 0..2 {
        let (scope, b) = f.acquire("createConcertParty", Some(key)).await?;
        let receipt = f.receipt_command(scope.clone(), Arc::new(b), 1).await?;
        assert_eq!(
            receipt.state,
            concert_application::commands::ReceiptState::Failed
        );
        if let Some(id) = receipt_id {
            assert_eq!(receipt.id, id);
        } else {
            receipt_id = Some(receipt.id);
        }
        f.finish(&scope).await?;
    }
    let count:i64=sqlx::query_scalar("select count(*) from concert_command_receipts where actor_id=$1 and event_id=$2 and idempotency_key=$3").bind(f.actor).bind(f.event).bind(key).fetch_one(&f.concert).await?;
    assert_eq!(count, 1);
    let (scope, b) = f.acquire_marked("createConcertParty", Some(key), 2).await?;
    let error = f
        .receipt_command(scope.clone(), Arc::new(b), 2)
        .await
        .unwrap_err();
    assert!(matches!(
        error.downcast_ref::<CommandError>(),
        Some(CommandError::Conflict)
    ));
    f.finish(&scope).await?;
    let p = scope.producer_scope();
    let rows:Vec<Value>=sqlx::query_scalar("select identity from concert_product_owner_transactions where instance_id=$1 and scope_id=$2").bind(f.instance).bind(Uuid::parse_str(&p.scope_id)?).fetch_all(&f.concert).await?;
    assert!(!rows.is_empty());
    for row in rows {
        let native: ProducerTransaction = serde_json::from_value(row)?;
        assert_eq!(
            f.resolver()
                .resolve(&scope.binding().owner_binding(), &native)
                .await?
                .status,
            proto::TransactionStatus::Aborted
        );
    }
    // Retained receipt retirement clears business outcome fields, not the actual
    // earlier native commit. Exercise this metadata lifecycle directly; this is
    // not a claim that a 24-hour expiry worker elapsed in the fixture.
    sqlx::query("update concert_command_receipts set state='RETIRED',failure_reason=null where receipt_id=$1")
        .bind(receipt_id.unwrap()).execute(&f.concert).await?;
    let committed = sqlx::query("select binding,identity from concert_party_native_witnesses where instance_id=$1 and native_role='COMMAND'")
        .bind(f.instance).fetch_all(&f.concert).await?;
    assert_eq!(committed.len(), 2);
    for row in committed {
        let owner: OwnerBinding = serde_json::from_value(row.try_get("binding")?)?;
        let identity: ProducerTransaction = serde_json::from_value(row.try_get("identity")?)?;
        let proof = f.resolver().resolve(&owner, &identity).await?;
        assert_eq!(proof.status, proto::TransactionStatus::Committed);
        let value: Value = serde_json::from_slice(&proof.receipt_evidence)?;
        assert_eq!(value["receipt"]["state"], "RETIRED");
        assert!(value.get("success").is_none());
    }

    Ok(())
}

#[tokio::test]
#[ignore = "fresh owned PG16/mTLS three-database allocation only"]
async fn native_party_active_worker_drop_waits_for_actual_rollback_drain() -> anyhow::Result<()> {
    let f = Fixture::new().await?;
    let (scope, b) = f.acquire("listConcertParties", None).await?;
    let mut seed = f.concert.begin().await?;
    let seeded = register_in(&scope, &mut seed, &b, NativeRole::Read).await?;
    seed.rollback().await?;
    // Simulate cancellation after durable Track but before witness installation.
    // The worker holds an actual transaction, not a fabricated status fixture.
    let pool = f.concert.clone();
    let worker_scope = scope.clone();
    let reference = seeded.settlement_reference.clone();
    let effect = seeded.effect_binding_digest;
    let instance = f.instance;
    let (send, receive) = tokio::sync::oneshot::channel();
    let hold = Arc::new(tokio::sync::Notify::new());
    let worker_hold = hold.clone();
    let worker = tokio::spawn(async move {
        let mut tx = pool.begin().await?;
        let native =
            GuardTransaction::capture_concert_in(&mut tx, effect, instance, reference).await?;
        worker_scope.track_transaction(native.clone()).await?;
        send.send(native)
            .map_err(|_| anyhow::anyhow!("worker receiver disappeared"))?;
        worker_hold.notified().await;
        tx.rollback().await?;
        Ok::<(), anyhow::Error>(())
    });
    let native = receive.await?;
    scope.mark_serialized().await?;
    let key = f.scope_key(&scope)?;
    f.ledger.seal(&key).await?;
    f.ledger.finish_root_work(&key).await?;
    let identity = stored(&native);
    let owner = scope.binding().owner_binding();
    assert_eq!(
        f.resolver().resolve(&owner, &identity).await?.status,
        proto::TransactionStatus::InProgress,
        "active native worker was mistaken for rollback"
    );
    assert!(
        scope.finish_response().await.is_err(),
        "owner exclusion released with unresolved active worker"
    );
    worker.abort();
    assert!(worker.await.unwrap_err().is_cancelled());
    tokio::time::timeout(std::time::Duration::from_secs(2), async {
        loop {
            if f.resolver().resolve(&owner, &identity).await?.status
                == proto::TransactionStatus::Aborted
            {
                return Ok::<(), Status>(());
            }
            tokio::time::sleep(std::time::Duration::from_millis(5)).await;
        }
    })
    .await??;
    // Only actual pg_xact_status rollback proof, not Drop or join completion,
    // permits both production owner services to settle their prepared exclusions.
    scope.finish_response().await?;
    for pool in [&f.auth, &f.friend] {
        assert_eq!(
            sqlx::query_scalar::<_, i64>(
                "select count(*) from pg_prepared_xacts where database=current_database()"
            )
            .fetch_one(pool)
            .await?,
            0
        );
    }
    Ok(())
}

#[tokio::test]
#[ignore = "only after actual owned PostgreSQL stop/start in the controller"]
async fn native_party_physical_restart_reconstructs_exact_resolver_proof() -> anyhow::Result<()> {
    let pool = environment::pool("concert").await?;
    let rows=sqlx::query("select w.instance_id,w.binding,w.identity from concert_party_native_witnesses w join concert_product_owner_scopes s using(instance_id,scope_id,binding_digest) where s.phase='ROOT_FINISHED' order by w.created_at").fetch_all(&pool).await?;
    anyhow::ensure!(
        rows.len() >= 10,
        "missing earlier completed native component cohorts"
    );
    for row in rows {
        let instance: Uuid = row.try_get("instance_id")?;
        let binding: OwnerBinding = serde_json::from_value(row.try_get("binding")?)?;
        let identity: ProducerTransaction = serde_json::from_value(row.try_get("identity")?)?;
        let resolver = PartyNativeResolver::new(pool.clone(), instance)?;
        let r = resolver.resolve(&binding, &identity).await?;
        assert_eq!(r.status, proto::TransactionStatus::Committed);
        assert!(!r.receipt_evidence.is_empty());
        let before:Value=sqlx::query_scalar("select to_jsonb(w) from concert_party_native_witnesses w where instance_id=$1 and identity_digest=$2").bind(instance).bind(identity.digest()?.as_slice()).fetch_one(&pool).await?;
        let repeated = resolver.resolve(&binding, &identity).await?;
        assert_eq!(r.receipt_evidence, repeated.receipt_evidence);
        let after:Value=sqlx::query_scalar("select to_jsonb(w) from concert_party_native_witnesses w where instance_id=$1 and identity_digest=$2").bind(instance).bind(identity.digest()?.as_slice()).fetch_one(&pool).await?;
        assert_eq!(before, after);
    }
    pool.close().await;
    Ok(())
}
