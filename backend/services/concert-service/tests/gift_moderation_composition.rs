//! Component routing only. Lazy pools, synthetic identities and rejecting
//! counters are not Auth authority, committed native proof or DB acceptance.
// Include the exact new factory without making Root's module export a gate.
pub use concert_service::{config, gift_live, moderation_live, product_rpc};
#[path = "../src/config/gift_moderation.rs"]
mod gift_moderation;

use config::canonical_runtime::CanonicalRuntimeCore;
use gift_moderation::GiftModerationComposition;
use platform::actor_delegation::owner_binding::OwnerBinding;
use product_rpc::{
    scope_ledger::{ChildIntentSettler, ChildRecoveryKey, ChildRecoveryKeys, ProducerTransaction},
    settlement::{NativeTransactionResolution, NativeTransactionResolver},
};
use concert_service::adapters::out::relationship_authority::guarded::ChildRecoveryReference;
use moderation_live::ports::ModerationOwners;
use serde_json::{json, Value};
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::sync::{Arc, Mutex};
use tonic::{Code, Status};
use uuid::Uuid;

struct RejectChildren;
#[async_trait::async_trait]
impl ChildIntentSettler for RejectChildren {
    async fn recover_one(&self, _: &ChildRecoveryReference) -> Result<(), Status> {
        Err(Status::unavailable("No child effect in composition test"))
    }
}

#[derive(Default)]
struct Fallback(Mutex<Vec<(String, Vec<u8>)>>);
#[async_trait::async_trait]
impl NativeTransactionResolver for Fallback {
    async fn resolve(&self, b: &OwnerBinding, n: &ProducerTransaction)
        -> Result<NativeTransactionResolution, Status> {
        self.0.lock().unwrap().push((b.operation.clone(), n.settlement_reference.clone()));
        Err(Status::unavailable("existing fallback sentinel; no positive proof"))
    }
}

fn fixture() -> anyhow::Result<(PgPool, CanonicalRuntimeCore, GiftModerationComposition)> {
    // No network or owned PostgreSQL is used by these routing-only cases.
    let pool = PgPoolOptions::new().connect_lazy("postgres://composition:unused@127.0.0.1:1/composition")?;
    let keys = Arc::new(ChildRecoveryKeys::new(
        ChildRecoveryKey::new("composition-test-only", &[41; 32])?, vec![],
    )?);
    let core = CanonicalRuntimeCore::new(pool.clone(), Uuid::from_u128(501), keys, Arc::new(RejectChildren))?;
    let composition = GiftModerationComposition::new(&core, pool.clone())?;
    Ok((pool, core, composition))
}

fn binding(effect: bool) -> OwnerBinding {
    let event = Uuid::from_u128(2);
    OwnerBinding {
        actor_id: Uuid::from_u128(1), event_id: event,
        session_family_id: Uuid::from_u128(3), request_id: Uuid::from_u128(4),
        operation: if effect { "moderateConcertContent" } else { "getConcertGiveaway" }.into(),
        expires_unix_micros: 1_900_000_000_000_000,
        parent_digest: [1; 32], parent_body_digest: [2; 32],
        parent_method: "/concert.product.v1.ProductService/Dispatch".into(),
        http_method: if effect { "POST" } else { "GET" }.into(),
        http_target: if effect { format!("/api/concert/events/{event}/admin/content") }
            else { format!("/api/concert/events/{event}/giveaways/{}", Uuid::from_u128(5)) },
        body_digest: [3; 32],
    }
}

fn reference(b: &OwnerBinding, instance: Uuid, effect: bool) -> Value {
    let digest = b.digest().unwrap().iter().map(|v| format!("{v:02x}")).collect::<String>();
    let mut value = json!({"kind":"GIFT_VISIBILITY_READ_V1", "instance_id":instance,
        "scope_id":Uuid::from_u128(12), "event_id":b.event_id, "binding_digest":digest});
    if effect {
        value["kind"] = json!("GIFT_MODERATION_OWNER_V1");
        for (key, item) in [
            ("owner_receipt_id", json!(Uuid::from_u128(13))),
            ("command_receipt_id", json!(Uuid::from_u128(14))),
            ("actor_id", json!(b.actor_id)), ("giveaway_id", json!(Uuid::from_u128(5))),
            ("operation_id", json!(b.operation)), ("owner_step", json!("GIVEAWAY_CONTENT")),
            ("expected_revision", json!(1)), ("action", json!("HIDE")),
            ("change_digest", json!("ab".repeat(32))),
        ] { value[key] = item; }
    }
    value
}

fn native(instance: Uuid, reference: Vec<u8>) -> ProducerTransaction {
    ProducerTransaction {
        producer: "CONCERT".into(), system_identifier: "123456".into(), database_oid: 7,
        full_xid: "42".into(), backend_pid: 10, backend_started_unix_micros: 1_800_000_000_000_000,
        effect_binding_digest: [11; 32], producer_instance: instance, settlement_reference: reference,
    }
}

#[tokio::test]
async fn one_allocation_supplies_both_ports_and_the_actual_core_instance() -> anyhow::Result<()> {
    let (pool, core, c) = fixture()?;
    assert_eq!(c.instance(), core.ledger().instance());
    assert_eq!(Arc::as_ptr(&c.giveaway_owner()) as *const (), Arc::as_ptr(&c.directory()) as *const ());
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn closed_pool_cannot_produce_a_ready_composition() -> anyhow::Result<()> {
    let (pool, core, _) = fixture()?;
    pool.close().await;
    assert!(GiftModerationComposition::new(&core, pool).is_err());
    Ok(())
}

#[tokio::test]
async fn installing_giveaway_preserves_all_other_ports_and_directory() -> anyhow::Result<()> {
    let (pool, _, c) = fixture()?;
    let sentinel = c.giveaway_owner();
    let directory = c.directory();
    let mut ports = ModerationOwners {
        event_restriction: Some(sentinel.clone()), event_world_termination: Some(sentinel.clone()),
        event_private_authority_revocation: Some(sentinel.clone()), post_content: Some(sentinel.clone()),
        party_content: Some(sentinel.clone()), intro_content: Some(sentinel.clone()),
        directory: Some(directory.clone()), ..ModerationOwners::default()
    };
    c.install_giveaway(&mut ports)?;
    for actual in [&ports.event_restriction, &ports.event_world_termination,
        &ports.event_private_authority_revocation, &ports.post_content, &ports.party_content, &ports.intro_content] {
        assert!(Arc::ptr_eq(actual.as_ref().unwrap(), &sentinel));
    }
    assert!(Arc::ptr_eq(ports.directory.as_ref().unwrap(), &directory));
    let installed = ports.giveaway_content.clone().unwrap();
    assert!(c.install_giveaway(&mut ports).is_err());
    assert!(Arc::ptr_eq(ports.giveaway_content.as_ref().unwrap(), &installed));
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn both_closed_gift_kinds_precede_fallback_without_fabricating_commit() -> anyhow::Result<()> {
    let (pool, _, c) = fixture()?;
    let fallback = Arc::new(Fallback::default());
    let resolver = c.native_resolver(fallback.clone());
    pool.close().await; // Actual Gift resolver must fail at its real closed pool.
    for effect in [false, true] {
        let b = binding(effect);
        let n = native(c.instance(), serde_json::to_vec(&reference(&b, c.instance(), effect))?);
        let error = resolver.resolve(&b, &n).await.err().expect("no native proof exists");
        assert_eq!(error.code(), Code::Unavailable);
        assert!(fallback.0.lock().unwrap().is_empty());
    }
    Ok(())
}

#[tokio::test]
async fn ordinary_gift_other_kinds_and_opaque_references_keep_exact_fallback_bytes() -> anyhow::Result<()> {
    let (pool, _, c) = fixture()?;
    let fallback = Arc::new(Fallback::default());
    let resolver = c.native_resolver(fallback.clone());
    let b = binding(false);
    let inputs = vec![br#"{"kind":"GIFT_REQUEST_V1","opaque":"ordinary"}"#.to_vec(),
        br#"{"kind":"GIFT_STATUS_V1"}"#.to_vec(), br#"{"kind":"OTHER_OWNER_V2","opaque":true}"#.to_vec(),
        b"opaque-other-owner-reference".to_vec()];
    for raw in &inputs {
        let n = native(c.instance(), raw.clone());
        assert_eq!(resolver.resolve(&b, &n).await.err().unwrap().message(), "existing fallback sentinel; no positive proof");
    }
    assert_eq!(*fallback.0.lock().unwrap(), inputs.into_iter().map(|raw| (b.operation.clone(), raw)).collect::<Vec<_>>());
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn malformed_duplicate_and_unclosed_gift_objects_never_reach_fallback() -> anyhow::Result<()> {
    let (pool, _, c) = fixture()?;
    let fallback = Arc::new(Fallback::default());
    let resolver = c.native_resolver(fallback.clone());
    let b = binding(false);
    let valid = reference(&b, c.instance(), false);
    let mut extra = valid.clone(); extra["allow"] = json!(true);
    let mut missing = valid.clone(); missing.as_object_mut().unwrap().remove("scope_id");
    let mut wrong_type = valid.clone(); wrong_type["event_id"] = json!(7);
    let mut duplicate = serde_json::to_string(&valid)?;
    duplicate.pop(); duplicate.push_str(",\"kind\":\"OTHER_OWNER_V2\"}");
    let raws = vec![b"{broken".to_vec(), serde_json::to_vec(&extra)?, serde_json::to_vec(&missing)?,
        serde_json::to_vec(&wrong_type)?, duplicate.into_bytes(), b"{}".to_vec(), vec![b' '; 32769]];
    for raw in raws {
        assert_eq!(resolver.resolve(&b, &native(c.instance(), raw)).await.err().unwrap().code(), Code::InvalidArgument);
    }
    assert!(fallback.0.lock().unwrap().is_empty());
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn foreign_instance_event_digest_actor_and_effect_vocabulary_fail_before_io() -> anyhow::Result<()> {
    let (pool, _, c) = fixture()?;
    let fallback = Arc::new(Fallback::default());
    let resolver = c.native_resolver(fallback.clone());
    for effect in [false, true] {
        let b = binding(effect);
        let original = reference(&b, c.instance(), effect);
        let mut changes = vec![("instance_id", json!(Uuid::from_u128(999))),
            ("event_id", json!(Uuid::from_u128(999))), ("scope_id", json!(Uuid::nil())),
            ("binding_digest", json!("00".repeat(32)))];
        if effect { changes.extend([("actor_id", json!(Uuid::from_u128(999))),
            ("action", json!("DELETE")), ("operation_id", json!("moderateConcertUser")),
            ("owner_step", json!("POST_CONTENT")), ("expected_revision", json!(0))]); }
        for (field, value) in changes {
            let mut changed = original.clone(); changed[field] = value;
            let n = native(c.instance(), serde_json::to_vec(&changed)?);
            assert_eq!(resolver.resolve(&b, &n).await.err().unwrap().code(), Code::InvalidArgument, "{field}");
        }
        let mut n = native(Uuid::from_u128(999), serde_json::to_vec(&original)?);
        assert_eq!(resolver.resolve(&b, &n).await.err().unwrap().code(), Code::InvalidArgument);
        n.producer_instance = c.instance(); n.producer = "POST".into();
        assert_eq!(resolver.resolve(&b, &n).await.err().unwrap().code(), Code::InvalidArgument);
    }
    assert!(fallback.0.lock().unwrap().is_empty());
    pool.close().await;
    Ok(())
}
