//! Explicit Intro startup over the shared Participant/Product canonical owners.
//! Configuration creates no identity, owner grant, listener or background task.
use super::{
    canonical_runtime::CanonicalOwnerComposition, participant_runtime::ParticipantProductOwners,
    product_runtime::ProductOwners,
};
use crate::{
    adapters::out::{
        age_provider::runtime::{AgeRuntime, ProviderMode},
        SystemClock,
    },
    intro_live::{IntroCursor, IntroRuntime},
    product_rpc::{
        child_recovery::ChildRecoverySettings, proto, scoped::ScopedProductOwner, ProductOwner,
        VerifiedProduct,
    },
};
use concert_application::{
    commands::{FingerprintKeys, SigningKey},
    community::giveaway::{PickupKey, PickupKeys},
};
use serde::Deserialize;
use std::{collections::BTreeSet, sync::Arc};

/// No Debug/Serialize: these are actual operator-supplied retained secrets.
pub struct IntroRuntimeSettings {
    profile_endpoint: String,
    notification_endpoint: String,
    pickup: Arc<PickupKeys>,
    receipts: Arc<FingerprintKeys>,
    cursor: IntroCursor,
    recovery: ChildRecoverySettings,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Key {
    id: String,
    secret_hex: String,
}

fn required(
    read: &mut impl FnMut(&str) -> anyhow::Result<Option<String>>,
    name: &str,
) -> anyhow::Result<String> {
    read(name)?
        .filter(|v| !v.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{name} is required for live Intro"))
}
fn secret(raw: &str) -> anyhow::Result<[u8; 32]> {
    anyhow::ensure!(
        raw.len() == 64
            && raw
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "Intro keys require 32-byte lowercase hexadecimal secrets"
    );
    let mut key = [0; 32];
    for (i, pair) in raw.as_bytes().chunks_exact(2).enumerate() {
        let digit = |b| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
        key[i] = digit(pair[0]) * 16 + digit(pair[1]);
    }
    anyhow::ensure!(key != [0; 32], "Intro keys must be nonzero");
    Ok(key)
}
fn keys(raw: &str, seen: &mut BTreeSet<[u8; 32]>) -> anyhow::Result<Vec<(String, [u8; 32])>> {
    anyhow::ensure!(
        raw.len() <= 16 * 1024,
        "Intro key configuration is too large"
    );
    let keys: Vec<Key> = serde_json::from_str(raw)
        .map_err(|_| anyhow::anyhow!("Intro keys require a closed JSON array"))?;
    anyhow::ensure!(
        (1..=32).contains(&keys.len()),
        "Intro keys require one active and at most 31 retained keys"
    );
    keys.into_iter()
        .map(|key| {
            let value = secret(&key.secret_hex)?;
            anyhow::ensure!(
                seen.insert(value),
                "Intro cursor, receipt and pickup secrets must be independent"
            );
            Ok((key.id, value))
        })
        .collect()
}
fn endpoint(value: String) -> anyhow::Result<String> {
    let uri: axum::http::Uri = value
        .parse()
        .map_err(|_| anyhow::anyhow!("Invalid Intro owner endpoint"))?;
    anyhow::ensure!(
        uri.scheme_str() == Some("https")
            && uri.host().is_some_and(|h| !h.is_empty())
            && uri.port_u16().is_some_and(|p| p > 0)
            && uri.authority().is_some_and(|a| !a.as_str().contains('@'))
            && uri.path_and_query().is_none_or(|p| p.as_str() == "/"),
        "Intro owner endpoints require explicit HTTPS host/port without credentials, query or path"
    );
    Ok(value)
}
impl IntroRuntimeSettings {
    /// Pure parser; off/default does not inspect unrelated keys or owner inputs.
    pub fn from_values(
        mut read: impl FnMut(&str) -> anyhow::Result<Option<String>>,
        grpc: bool,
        participant: bool,
        recovery: ChildRecoverySettings,
    ) -> anyhow::Result<Option<Self>> {
        match read("CONCERT_INTRO_MODE")?.as_deref() {
            None | Some("off") => return Ok(None),
            Some("live") => (),
            _ => anyhow::bail!("CONCERT_INTRO_MODE must be off or live"),
        }
        anyhow::ensure!(
            grpc && participant,
            "Live Intro requires guarded gRPC and shared live Participant composition"
        );
        let profile_endpoint = endpoint(required(
            &mut read,
            "CONCERT_PARTICIPANT_PROFILE_GRPC_ENDPOINT",
        )?)?;
        let notification_endpoint =
            endpoint(required(&mut read, "CONCERT_NOTIFICATION_GRPC_ENDPOINT")?)?;
        let cursor_key = secret(&required(&mut read, "CONCERT_INTRO_CURSOR_KEY_HEX")?)?;
        let mut seen = BTreeSet::from([cursor_key]);
        let receipt_keys = keys(
            &required(&mut read, "CONCERT_INTRO_RECEIPT_KEYS_JSON")?,
            &mut seen,
        )?;
        let pickup_keys = keys(&required(&mut read, "CONCERT_PICKUP_KEYS_JSON")?, &mut seen)?;
        let mut receipt_keys = receipt_keys
            .into_iter()
            .map(|(id, secret)| SigningKey::new(&id, &secret))
            .collect::<Result<Vec<_>, _>>()?;
        let active = receipt_keys.remove(0);
        let receipts = Arc::new(FingerprintKeys::new(active, receipt_keys)?);
        let mut pickup_keys = pickup_keys
            .into_iter()
            .map(|(id, secret)| PickupKey::new(&id, &secret))
            .collect::<Result<Vec<_>, _>>()?;
        let active = pickup_keys.remove(0);
        let pickup = Arc::new(PickupKeys::new(active, pickup_keys)?);
        let cursor = IntroCursor::new(cursor_key)?;
        Ok(Some(Self {
            profile_endpoint,
            notification_endpoint,
            pickup,
            receipts,
            cursor,
            recovery,
        }))
    }
    pub fn load(grpc: bool, participant: bool) -> anyhow::Result<Option<Self>> {
        // Do not reject optional recovery configuration in the independent off mode.
        let mode = std::env::var("CONCERT_INTRO_MODE");
        if matches!(mode, Err(std::env::VarError::NotPresent)) || mode.as_deref() == Ok("off") {
            return Ok(None);
        }
        Self::from_values(
            |name| match std::env::var(name) {
                Ok(v) => Ok(Some(v)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => anyhow::bail!("Invalid Intro configuration encoding"),
            },
            grpc,
            participant,
            ChildRecoverySettings::from_env()?,
        )
    }
    pub fn participant_owners(&self) -> ParticipantProductOwners {
        ParticipantProductOwners {
            profile_endpoint: self.profile_endpoint.clone(),
            notification_endpoint: self.notification_endpoint.clone(),
            pickup_keys: self.pickup.clone(),
        }
    }
    pub fn recovery_settings(&self) -> ChildRecoverySettings {
        self.recovery
    }
    pub fn compose(self, pool: sqlx::PgPool, owners: &CanonicalOwnerComposition) -> ProductOwners {
        // Missing external verification is a real unavailable provider mode. It
        // cannot mint an age claim or admit age-restricted discovery.
        let age = Arc::new(AgeRuntime::new(
            pool.clone(),
            Arc::new(SystemClock),
            self.receipts.clone(),
            ProviderMode::Unavailable,
        ));
        let producer = Arc::new(IntroRuntime::new(
            pool,
            owners.public_profile.clone(),
            self.cursor,
            self.receipts,
            age,
        ));
        let delivery = owners.core.delivery();
        ProductOwners {
            product: Arc::new(IntroProductOwner(ScopedProductOwner::new(
                owners.fence.clone(),
                delivery.clone(),
                producer,
            ))),
            delivery,
        }
    }
}

/// Only this configured feature may acquire a canonical scope. Other admitted
/// product families stay unavailable until their actual producers are composed.
struct IntroProductOwner(ScopedProductOwner);
#[async_trait::async_trait]
impl ProductOwner for IntroProductOwner {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        request: proto::ProductRequest,
    ) -> Result<tonic::Response<proto::ProductResponse>, tonic::Status> {
        if !crate::intro_live::protocol::handles_operation(context.operation().contract_id()) {
            return Err(tonic::Status::unavailable("Product producer unavailable"));
        }
        self.0.dispatch(context, request).await
    }
}
