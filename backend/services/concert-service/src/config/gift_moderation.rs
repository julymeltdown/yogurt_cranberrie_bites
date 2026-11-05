//! Gift-only composition over an existing canonical core; never route activation.
use crate::{
    config::canonical_runtime::CanonicalRuntimeCore,
    gift_live::moderation::{GiftModerationNativeResolver, GiftModerationOwner},
    moderation_live::ports::{ModerationDirectory, ModerationOwner, ModerationOwners},
    product_rpc::{
        scope_ledger::ProducerTransaction,
        settlement::{NativeTransactionResolution, NativeTransactionResolver},
    },
};
use platform::actor_delegation::owner_binding::OwnerBinding;
use serde::Deserialize;
use sqlx::PgPool;
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

/// Root supplies its existing Concert pool paired with this core. Construction
/// derives the instance from the core, but cannot attest to the pool's custody.
/// No core, role provider, worker, listener or feature switch is created here.
pub struct GiftModerationComposition {
    owner: Arc<GiftModerationOwner>,
    native: Arc<GiftModerationNativeResolver>,
    instance: Uuid,
}

impl GiftModerationComposition {
    pub fn new(core: &CanonicalRuntimeCore, pool: PgPool) -> Result<Self, Status> {
        let instance = core.ledger().instance();
        let owner = GiftModerationOwner::new(pool.clone(), instance)
            .map_err(|_| Status::unavailable("Gift moderation composition unavailable"))?;
        let native = GiftModerationNativeResolver::new(pool, instance)?;
        Ok(Self { owner: Arc::new(owner), native: Arc::new(native), instance })
    }

    pub fn instance(&self) -> Uuid { self.instance }

    pub fn giveaway_owner(&self) -> Arc<dyn ModerationOwner> { self.owner.clone() }

    /// The exact same allocation implements effects and Gift directory reads.
    /// Root's multi-content directory composer selects this port explicitly.
    pub fn directory(&self) -> Arc<dyn ModerationDirectory> { self.owner.clone() }

    /// Never replace an installed owner, including a repeated self-install.
    /// In particular, the global directory and every non-Gift port survive.
    pub fn install_giveaway(&self, ports: &mut ModerationOwners) -> Result<(), Status> {
        if ports.giveaway_content.is_some() {
            return Err(Status::failed_precondition("Giveaway moderation owner already installed"));
        }
        ports.giveaway_content = Some(self.giveaway_owner());
        Ok(())
    }

    /// Gift's two closed reference kinds precede the supplied existing union.
    /// Ordinary Gift13 and other owners keep their original resolver and bytes.
    pub fn native_resolver(
        &self,
        fallback: Arc<dyn NativeTransactionResolver>,
    ) -> Arc<dyn NativeTransactionResolver> {
        Arc::new(GiftFirstResolver {
            gift: self.native.clone(), fallback, instance: self.instance,
        })
    }
}

fn invalid() -> Status { Status::invalid_argument("Invalid Gift moderation native reference") }

#[derive(Deserialize)]
struct Kind { kind: String }

// These are the exact fields authored by Gift's frozen native owner. Derive
// rejects duplicate fields, unknown fields, missing fields and incorrect types.
#[derive(Deserialize)]
#[serde(tag = "kind", deny_unknown_fields)]
enum Reference {
    #[serde(rename = "GIFT_VISIBILITY_READ_V1")]
    Read {
        instance_id: Uuid, scope_id: Uuid, event_id: Uuid, binding_digest: String,
    },
    #[serde(rename = "GIFT_MODERATION_OWNER_V1")]
    Effect {
        owner_receipt_id: Uuid, command_receipt_id: Uuid, actor_id: Uuid,
        event_id: Uuid, giveaway_id: Uuid, operation_id: String, owner_step: String,
        expected_revision: i64, action: String, change_digest: String,
        instance_id: Uuid, scope_id: Uuid, binding_digest: String,
    },
}

fn digest_text(raw: &[u8; 32]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(64);
    for byte in raw {
        out.push(DIGITS[(byte >> 4) as usize] as char);
        out.push(DIGITS[(byte & 15) as usize] as char);
    }
    out
}

impl Reference {
    fn matches(&self, instance: Uuid, binding: &OwnerBinding) -> Result<bool, Status> {
        let expected_digest = digest_text(&binding.digest()?);
        let common = |i: Uuid, s: Uuid, e: Uuid, d: &str| {
            i == instance && !s.is_nil() && e == binding.event_id && d == expected_digest
        };
        Ok(match self {
            Self::Read { instance_id, scope_id, event_id, binding_digest } => {
                common(*instance_id, *scope_id, *event_id, binding_digest)
                    && crate::gift_live::settlement::handles_binding(binding)
            }
            Self::Effect { owner_receipt_id, command_receipt_id, actor_id, event_id,
                giveaway_id, operation_id, owner_step, expected_revision, action,
                change_digest, instance_id, scope_id, binding_digest } => {
                common(*instance_id, *scope_id, *event_id, binding_digest)
                    && !owner_receipt_id.is_nil() && !command_receipt_id.is_nil()
                    && !giveaway_id.is_nil() && *actor_id == binding.actor_id
                    && operation_id == "moderateConcertContent"
                    && operation_id == &binding.operation && binding.http_method == "POST"
                    && owner_step == "GIVEAWAY_CONTENT"
                    && *expected_revision >= 1 && *expected_revision < concert_domain::MAX_REVISION
                    && matches!(action.as_str(), "HIDE" | "RESTORE")
                    && change_digest.len() == 64
                    && change_digest.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
            }
        })
    }
}

struct GiftFirstResolver {
    gift: Arc<GiftModerationNativeResolver>,
    fallback: Arc<dyn NativeTransactionResolver>,
    instance: Uuid,
}

#[async_trait::async_trait]
impl NativeTransactionResolver for GiftFirstResolver {
    async fn resolve(
        &self, binding: &OwnerBinding, native: &ProducerTransaction,
    ) -> Result<NativeTransactionResolution, Status> {
        // The SDK permits opaque references for other owners. Preserve those
        // unchanged; JSON-object references must have one unambiguous kind.
        // A malformed object must never escape into a permissive fallback.
        let raw = &native.settlement_reference;
        if raw.len() > 32768 { return Err(invalid()); }
        if raw.iter().copied().find(|b| !b.is_ascii_whitespace()) != Some(b'{') {
            return self.fallback.resolve(binding, native).await;
        }
        let kind: Kind = serde_json::from_slice(raw).map_err(|_| invalid())?;
        if !matches!(kind.kind.as_str(), "GIFT_MODERATION_OWNER_V1" | "GIFT_VISIBILITY_READ_V1") {
            return self.fallback.resolve(binding, native).await;
        }
        let reference: Reference = serde_json::from_slice(raw).map_err(|_| invalid())?;
        if native.producer != "CONCERT" || native.producer_instance != self.instance
            || !reference.matches(self.instance, binding)? {
            return Err(invalid());
        }
        native.digest()?;
        // Native transaction status and journal/Root witnesses remain entirely
        // with the actual owner resolver. This router manufactures no evidence.
        self.gift.resolve(binding, native).await
    }
}
