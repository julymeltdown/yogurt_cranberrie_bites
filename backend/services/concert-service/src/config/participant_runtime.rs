//! Explicit C03 composition. Root mounts guarded services and supervises recovery.
//! No optional public product, synthetic release, parity key or successful fallback.
use super::{
    canonical_runtime::{
        CanonicalOwnerComposition, CanonicalOwnerConfig, CanonicalOwnerEndpoints,
        CanonicalRuntimeCore,
    },
    native_resolver::NativeResolverUnion,
};
use crate::{
    adapters::out::{
        relationship_authority::guarded::{CanonicalOwnerFence, ChildRecoveryReference},
        repository::avatar::AvatarPersistence,
    },
    participant::live::{
        auth::GrpcCanonicalAuth, coordinator::ParticipantProfileJournal,
        decision_rpc::ParticipantDecisionService, profile::GrpcCanonicalSelfProfile,
        recovery::ParticipantRecovery, service::GuardedParticipantService,
        settlement::ParticipantNativeResolver,
    },
    product_rpc::{
        delivery::DeliveryPublisher,
        scope_ledger::{ChildIntentSettler, ChildRecoveryKey, ChildRecoveryKeys},
        settlement::ProducerSettlementService,
    },
};
use concert_application::{bootstrap::ValidatedProductRelease, community::giveaway::PickupKeys};
use platform::{
    actor_delegation::{
        concert_self_profile::ConcertSelfProfileSigner,
        participant_decision::ParticipantDecisionVerifier,
        relationship_read::RelationshipReadSigner, ActorDelegationSigner, ActorDelegationVerifier,
    },
    grpc_auth::{CallerRegistry, ServiceIdentityClientInterceptor, Workload},
    grpc_tls::TlsMaterial,
};
use serde::Deserialize;
use std::{collections::BTreeSet, sync::Arc};
use tonic::Status;
use uuid::Uuid;

const ENDPOINTS: [&str; 3] = [
    "CONCERT_PARTICIPANT_AUTH_GRPC_ENDPOINT",
    "CONCERT_PARTICIPANT_FRIEND_GRPC_ENDPOINT",
    "CONCERT_PARTICIPANT_PROFILE_GRPC_ENDPOINT",
];
const INSTANCE: &str = "CONCERT_PRODUCER_INSTANCE_ID";
const RELEASE: &str = "CONCERT_PARTICIPANT_RELEASE_DIGEST";
const KEYS: &str = "CONCERT_PARTICIPANT_RECOVERY_KEYS_JSON";
const CALLERS: [Workload; 4] = [
    Workload::Gateway,
    Workload::Auth,
    Workload::Friend,
    Workload::Profile,
];

// Deliberately no Debug: settings retain credentials and at-rest recovery keys.
pub struct ParticipantSettings {
    endpoints: [String; 3],
    instance: Uuid,
    release: String,
    keys: Arc<ChildRecoveryKeys>,
    callers: CallerRegistry,
    credential: String,
}
impl ParticipantSettings {
    pub fn instance_id(&self) -> Uuid {
        self.instance
    }
    pub fn release_digest(&self) -> &str {
        &self.release
    }
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct RecoveryKey {
    id: String,
    secret_hex: String,
}
fn required(
    read: &mut impl FnMut(&str) -> anyhow::Result<Option<String>>,
    name: &str,
) -> anyhow::Result<String> {
    read(name)?
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| anyhow::anyhow!("{name} is required for Participant serving"))
}
fn recovery_keys(value: &str) -> anyhow::Result<ChildRecoveryKeys> {
    anyhow::ensure!(
        value.len() <= 2048,
        "Participant recovery key configuration is too large"
    );
    let raw: Vec<RecoveryKey> = serde_json::from_str(value)
        .map_err(|_| anyhow::anyhow!("Participant recovery keys must be a closed JSON array"))?;
    anyhow::ensure!(
        (1..=4).contains(&raw.len()),
        "Participant recovery keys require one active and at most three retained keys"
    );
    let mut secrets = BTreeSet::new();
    let mut keys = Vec::new();
    for key in raw {
        anyhow::ensure!(
            key.secret_hex.len() == 64
                && key
                    .secret_hex
                    .bytes()
                    .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
            "Participant recovery secrets must be 32-byte lowercase hex"
        );
        anyhow::ensure!(
            secrets.insert(key.secret_hex.clone()),
            "Participant recovery key secrets must differ"
        );
        let mut secret = [0u8; 32];
        for (i, pair) in key.secret_hex.as_bytes().chunks_exact(2).enumerate() {
            let digit = |b: u8| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
            secret[i] = digit(pair[0]) * 16 + digit(pair[1]);
        }
        keys.push(
            ChildRecoveryKey::new(&key.id, &secret)
                .map_err(|_| anyhow::anyhow!("Participant recovery key is invalid"))?,
        );
    }
    let active = keys.remove(0);
    ChildRecoveryKeys::new(active, keys)
        .map_err(|_| anyhow::anyhow!("Participant recovery key identities must be unique"))
}

/// Pure settings validation, with no environment mutation, DB connection or listener.
/// Off/default is independent of optional product modes and returns no authority.
pub fn settings(
    mut read: impl FnMut(&str) -> anyhow::Result<Option<String>>,
    grpc_enabled: bool,
) -> anyhow::Result<Option<ParticipantSettings>> {
    match read("CONCERT_PARTICIPANT_MODE")?.as_deref() {
        None | Some("off") => return Ok(None),
        Some("live") => {}
        _ => anyhow::bail!("CONCERT_PARTICIPANT_MODE must be off or live"),
    }
    anyhow::ensure!(
        grpc_enabled,
        "Participant requires the actual guarded Concert gRPC listener"
    );
    let mut endpoints = Vec::new();
    for name in ENDPOINTS {
        let value = required(&mut read, name)?;
        let uri: axum::http::Uri = value
            .parse()
            .map_err(|_| anyhow::anyhow!("{name} is invalid"))?;
        anyhow::ensure!(uri.scheme_str() == Some("https") && uri.host().is_some_and(|h| !h.is_empty()) && uri.port_u16().is_some_and(|p| p > 0) && uri.authority().is_some_and(|a| !a.as_str().contains('@')) && uri.path_and_query().is_none_or(|p| p.as_str() == "/"), "Participant endpoints require explicit HTTPS host/port without credentials, query or path");
        endpoints.push(value);
    }
    let raw = required(&mut read, INSTANCE)?;
    let instance: Uuid = raw
        .parse()
        .map_err(|_| anyhow::anyhow!("Participant instance must be a canonical UUID"))?;
    anyhow::ensure!(
        !instance.is_nil() && instance.to_string() == raw,
        "Participant instance must be a nonzero canonical UUID"
    );
    let release = required(&mut read, RELEASE)?;
    anyhow::ensure!(
        release.len() == 64
            && release
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "Participant release must be a configured lowercase SHA256"
    );
    let keys = Arc::new(recovery_keys(&required(&mut read, KEYS)?)?);
    let mut callers = CallerRegistry::new();
    let mut seen = BTreeSet::new();
    let mut credential = None;
    for workload in CALLERS.into_iter().chain([Workload::Concert]) {
        let name = platform::config::service_credential_env_name(workload);
        let token =
            platform::config::resolve_service_credential(Some(required(&mut read, &name)?), &name)?
                .expect("explicit credential");
        ServiceIdentityClientInterceptor::new(workload, &token)?;
        anyhow::ensure!(
            seen.insert(token.clone()),
            "Participant workload credentials must be distinct"
        );
        if workload == Workload::Concert {
            credential = Some(token);
            continue;
        }
        let previous_name = platform::config::previous_service_credential_env_name(workload);
        let previous =
            platform::config::resolve_service_credential(read(&previous_name)?, &previous_name)?;
        if let Some(previous) = previous.as_ref() {
            ServiceIdentityClientInterceptor::new(workload, previous)?;
            anyhow::ensure!(
                seen.insert(previous.clone()),
                "Participant rotation credentials must be distinct"
            );
        }
        callers = callers.register_with_previous(workload, &token, previous.as_deref())?;
    }
    Ok(Some(ParticipantSettings {
        endpoints: endpoints.try_into().expect("three endpoints"),
        instance,
        release,
        keys,
        callers,
        credential: credential.expect("Concert credential"),
    }))
}

pub struct ParticipantRuntimeSettings {
    settings: ParticipantSettings,
    tls: TlsMaterial,
    gateway: ActorDelegationVerifier,
    relationship: RelationshipReadSigner,
    profile: ConcertSelfProfileSigner,
    decision: ActorDelegationVerifier,
}
fn validate_keys(
    gateway: &ActorDelegationVerifier,
    relationship: &RelationshipReadSigner,
    profile: &ConcertSelfProfileSigner,
    decision: &ActorDelegationVerifier,
) -> anyhow::Result<()> {
    anyhow::ensure!(
        gateway.audience() == Workload::Concert && decision.audience() == Workload::Concert,
        "Participant verifiers require the Concert audience"
    );
    anyhow::ensure!(
        !gateway.trusts(&relationship.public_key()) && !gateway.trusts(&profile.public_key()),
        "Participant outbound keys must be distinct from Gateway keys"
    );
    anyhow::ensure!(
        relationship.public_key() != profile.public_key(),
        "Participant relationship and SELF keys must be distinct"
    );
    anyhow::ensure!(
        !decision.trusts(&relationship.public_key()) && !decision.trusts(&profile.public_key()),
        "Participant decision keys must be distinct from outbound producer keys"
    );
    for workload in Workload::ALL {
        let parity = ActorDelegationSigner::local_parity(workload).public_key();
        anyhow::ensure!(
            !gateway.trusts(&parity)
                && !decision.trusts(&parity)
                && relationship.public_key() != parity
                && profile.public_key() != parity,
            "Participant cannot use parity identities"
        );
    }
    Ok(())
}
impl ParticipantRuntimeSettings {
    /// All credentials, explicit dedicated keys and mTLS are required even locally.
    /// Validate before opening the database; no channel connects during this load.
    pub fn load(grpc_enabled: bool) -> anyhow::Result<Option<Self>> {
        let Some(settings) = settings(
            |name| match std::env::var(name) {
                Ok(v) => Ok(Some(v)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => anyhow::bail!("{name} must contain valid Unicode"),
            },
            grpc_enabled,
        )?
        else {
            return Ok(None);
        };
        let gateway = ActorDelegationVerifier::from_env(true, Workload::Concert)?;
        let relationship = RelationshipReadSigner::from_env(&gateway)?;
        let profile = ConcertSelfProfileSigner::from_env(&gateway)?;
        let decision =
            ParticipantDecisionVerifier::configured_from_env(&gateway)?.ok_or_else(|| {
                anyhow::anyhow!(
                    "PROFILE_PARTICIPANT_DECISION_VERIFYING_KEYS is required for Participant"
                )
            })?;
        let tls = platform::config::resolve_tls_material(true, "concert")?
            .ok_or_else(|| anyhow::anyhow!("Participant requires actual Concert mTLS"))?;
        Self::validated_parts(settings, tls, gateway, relationship, profile, decision).map(Some)
    }

    /// Compose explicit typed configuration without mutating process environment.
    /// ParticipantSettings can only originate from the closed settings parser.
    /// This applies the same key-role, TLS and endpoint checks as load().
    pub fn from_validated(
        settings: ParticipantSettings,
        tls: TlsMaterial,
        gateway: ActorDelegationVerifier,
        relationship: RelationshipReadSigner,
        profile: ConcertSelfProfileSigner,
        decision_keys: &[[u8; 32]],
    ) -> anyhow::Result<Self> {
        let decision = ParticipantDecisionVerifier::new(decision_keys, &gateway)?;
        Self::validated_parts(settings, tls, gateway, relationship, profile, decision)
    }

    fn validated_parts(
        settings: ParticipantSettings,
        tls: TlsMaterial,
        gateway: ActorDelegationVerifier,
        relationship: RelationshipReadSigner,
        profile: ConcertSelfProfileSigner,
        decision: ParticipantDecisionVerifier,
    ) -> anyhow::Result<Self> {
        let decision = decision.into_inner();
        validate_keys(&gateway, &relationship, &profile, &decision)?;
        tls.server_config()?;
        // Lazy channels still validate exact owner identity/TLS endpoint configuration.
        tls.endpoint(&settings.endpoints[0], Workload::Auth)?;
        tls.endpoint(&settings.endpoints[1], Workload::Friend)?;
        tls.endpoint(&settings.endpoints[2], Workload::Profile)?;
        Ok(Self {
            settings,
            tls,
            gateway,
            relationship,
            profile,
            decision,
        })
    }
    pub fn compose(
        self,
        pool: sqlx::PgPool,
        release: Arc<ValidatedProductRelease>,
    ) -> anyhow::Result<ParticipantRuntime> {
        anyhow::ensure!(
            release.source_digest() == self.settings.release,
            "Participant and active product release differ"
        );
        let canonical = CanonicalRuntimeCore::new(
            pool.clone(),
            self.settings.instance,
            self.settings.keys.clone(),
            Arc::new(ParticipantOnlyChildren),
        )?;
        let fence = Arc::new(canonical.owner_fence(
            &self.settings.endpoints[0],
            &self.settings.endpoints[1],
            &self.settings.credential,
            &self.tls,
            self.relationship.clone(),
        )?);
        let settlement = canonical.settlement_service(Arc::new(ParticipantNativeResolver::new(
            pool.clone(),
            self.settings.instance,
        )?));
        self.assemble(pool, release, canonical, fence, settlement)
    }

    /// One origin-bound assembly for actual Participant and Product owners.
    /// No externally supplied core, fence, keyring or native resolver can be
    /// paired with unrelated settings. Construction opens no listener/worker.
    pub fn compose_shared(
        self,
        pool: sqlx::PgPool,
        release: Arc<ValidatedProductRelease>,
        additional: ParticipantProductOwners,
    ) -> anyhow::Result<SharedParticipantRuntime> {
        anyhow::ensure!(
            release.source_digest() == self.settings.release,
            "Participant and active product release differ"
        );
        anyhow::ensure!(
            additional.profile_endpoint == self.settings.endpoints[2],
            "Shared public Profile and Participant SELF require the same configured owner"
        );
        validate_shared_endpoint(&additional.profile_endpoint)?;
        validate_shared_endpoint(&additional.notification_endpoint)?;
        // Build the complete closed resolver with the exact same pool/instance,
        // before constructing the live owners. Never accept a foreign adapter.
        let native = NativeResolverUnion::new(pool.clone(), self.settings.instance)?;
        let owners = CanonicalOwnerComposition::compose(
            pool.clone(),
            release.clone(),
            CanonicalOwnerConfig {
                instance: self.settings.instance,
                child_keys: self.settings.keys.clone(),
                pickup_keys: additional.pickup_keys,
                endpoints: CanonicalOwnerEndpoints {
                    auth: self.settings.endpoints[0].clone(),
                    friend: self.settings.endpoints[1].clone(),
                    profile: additional.profile_endpoint,
                    notification: additional.notification_endpoint,
                },
                credential: self.settings.credential.clone(),
                tls: self.tls.clone(),
                relationship_signer: self.relationship.clone(),
            },
        )?;
        let settlement = owners.core.settlement_service(Arc::new(native));
        let participant = self.assemble(
            pool,
            release,
            owners.core.clone(),
            owners.fence.clone(),
            settlement,
        )?;
        Ok(SharedParticipantRuntime {
            participant,
            owners,
        })
    }

    fn assemble(
        self,
        pool: sqlx::PgPool,
        release: Arc<ValidatedProductRelease>,
        canonical: CanonicalRuntimeCore,
        fence: Arc<CanonicalOwnerFence>,
        settlement: ProducerSettlementService,
    ) -> anyhow::Result<ParticipantRuntime> {
        let delivery = canonical.delivery();
        let auth = GrpcCanonicalAuth::connect(
            &self.settings.endpoints[0],
            &self.settings.credential,
            Some(&self.tls),
            self.relationship,
        )?;
        let profile = GrpcCanonicalSelfProfile::connect(
            &self.settings.endpoints[2],
            &self.settings.credential,
            Some(&self.tls),
            self.profile,
        )?;
        let journal = Arc::new(
            ParticipantProfileJournal::from_product(pool.clone(), &release).map_err(|_| {
                anyhow::anyhow!("Participant journal requires actual product release")
            })?,
        );
        let store = Arc::new(
            AvatarPersistence::from_product(pool.clone(), release).map_err(|_| {
                anyhow::anyhow!("Participant persistence requires actual product release")
            })?,
        );
        Ok(ParticipantRuntime {
            participant: GuardedParticipantService::new(
                store,
                auth,
                profile.clone(),
                fence.clone(),
                journal.clone(),
                Arc::new(delivery.clone()),
            ),
            decision: ParticipantDecisionService::new(pool.clone()),
            decision_verifier: self.decision,
            settlement,
            canonical,
            fence,
            delivery,
            recovery: ParticipantRecovery::new(journal, profile),
            callers: self.settings.callers,
            gateway_verifier: self.gateway,
        })
    }
}
/// Additional actual owner configuration only. Original Auth/Friend endpoints,
/// producer identity, retained child keys, TLS and credentials are inherited
/// from the validated Participant settings, never supplied a second time.
pub struct ParticipantProductOwners {
    pub profile_endpoint: String,
    pub notification_endpoint: String,
    pub pickup_keys: Arc<PickupKeys>,
}

/// Root mounts participant.settlement and participant.delivery exactly once.
/// Both consumers already share the same registry, ledger, fence and publisher.
/// Root still owns mounting, worker supervision and full production acceptance.
pub struct SharedParticipantRuntime {
    pub participant: ParticipantRuntime,
    pub owners: CanonicalOwnerComposition,
}

fn validate_shared_endpoint(value: &str) -> anyhow::Result<()> {
    let uri: axum::http::Uri = value
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid shared owner endpoint"))?;
    anyhow::ensure!(uri.scheme_str() == Some("https") && uri.host().is_some_and(|h| !h.is_empty()) && uri.port_u16().is_some_and(|p| p > 0) && uri.authority().is_some_and(|a| !a.as_str().contains('@')) && uri.path_and_query().is_none_or(|p| p.as_str() == "/"),
        "Shared owner endpoints require explicit HTTPS host/port without credentials, query or path");
    Ok(())
}

/// Root guards each service with its exact policy; the Decision service must use
/// decision_verifier, never gateway_verifier. Mount one shared Delivery service,
/// retain original response EOF middleware, and supervise recovery.run(receiver).
pub struct ParticipantRuntime {
    /// Share these exact instances with other feature factories. Their real
    /// typed settlers and closed native resolver union must be installed first.
    pub canonical: CanonicalRuntimeCore,
    pub fence: Arc<CanonicalOwnerFence>,
    pub participant: GuardedParticipantService,
    pub decision: ParticipantDecisionService,
    pub decision_verifier: ActorDelegationVerifier,
    pub settlement: ProducerSettlementService,
    pub delivery: DeliveryPublisher,
    pub recovery: ParticipantRecovery,
    pub callers: CallerRegistry,
    pub gateway_verifier: ActorDelegationVerifier,
}
/// C03 never creates public/Gift/Social child intents. An unexpected existing
/// intent must stay unresolved; another product cannot gain C03's ledger authority.
struct ParticipantOnlyChildren;
#[async_trait::async_trait]
impl ChildIntentSettler for ParticipantOnlyChildren {
    async fn recover_one(&self, _: &ChildRecoveryReference) -> Result<(), Status> {
        Err(Status::failed_precondition(
            "Child recovery is outside Participant composition",
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn production_key_roles_remain_distinct_and_reject_all_parity_identities() {
        let gateway_key = ActorDelegationSigner::from_seed(Workload::Gateway, &[41; 32])
            .unwrap()
            .public_key();
        let gateway = ActorDelegationVerifier::new(Workload::Concert, &[gateway_key]).unwrap();
        let relationship = RelationshipReadSigner::from_seed(&[51; 32], &gateway).unwrap();
        let profile = ConcertSelfProfileSigner::from_seed(&[61; 32], &gateway).unwrap();
        let decision_key = ActorDelegationSigner::from_seed(Workload::Profile, &[71; 32])
            .unwrap()
            .public_key();
        let decision = ParticipantDecisionVerifier::new(&[decision_key], &gateway)
            .unwrap()
            .into_inner();
        assert!(validate_keys(&gateway, &relationship, &profile, &decision).is_ok());
        let same = ConcertSelfProfileSigner::from_seed(&[51; 32], &gateway).unwrap();
        assert!(validate_keys(&gateway, &relationship, &same, &decision).is_err());
        for key in [relationship.public_key(), profile.public_key()] {
            let reused = ParticipantDecisionVerifier::new(&[key], &gateway)
                .unwrap()
                .into_inner();
            assert!(validate_keys(&gateway, &relationship, &profile, &reused).is_err());
        }
        for workload in Workload::ALL {
            let parity = ActorDelegationSigner::local_parity(workload).public_key();
            let bad =
                ActorDelegationVerifier::new(Workload::Concert, &[gateway_key, parity]).unwrap();
            assert!(validate_keys(&bad, &relationship, &profile, &decision).is_err());
        }
    }
}
