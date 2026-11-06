//! Explicit Party factory over the existing canonical owner composition.
//! It creates no second scope registry, fence or listener.
use super::{canonical_runtime::CanonicalOwnerComposition, product_runtime::ProductOwners};
use crate::{
    party_live::resources::PgPartyResources,
    party_live::{PartyCursorCodec, ScopedPartyProductProducer},
    product_rpc::{proto, scoped::ScopedProductOwner, Operation, ProductOwner, VerifiedProduct},
};
use async_trait::async_trait;
use concert_application::commands::{FingerprintKeys, SigningKey};
use serde::Deserialize;
use std::{collections::BTreeSet, sync::Arc};

pub struct PartyRuntimeSettings {
    receipts: Arc<FingerprintKeys>,
    cursor: [u8; 32],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Key {
    id: String,
    secret_hex: String,
}

fn secret(value: &str) -> anyhow::Result<[u8; 32]> {
    anyhow::ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "Party secrets require 32-byte lowercase hexadecimal values"
    );
    let mut out = [0; 32];
    for (target, pair) in out.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        let digit = |b| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
        *target = digit(pair[0]) * 16 + digit(pair[1]);
    }
    anyhow::ensure!(out != [0; 32], "Party secrets must be nonzero");
    Ok(out)
}

fn required(
    read: &mut impl FnMut(&str) -> anyhow::Result<Option<String>>,
    key: &str,
) -> anyhow::Result<String> {
    read(key)?
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{key} is required for live Parties"))
}

impl PartyRuntimeSettings {
    pub fn from_values(
        mut read: impl FnMut(&str) -> anyhow::Result<Option<String>>,
        guarded_grpc: bool,
        shared_canonical: bool,
    ) -> anyhow::Result<Option<Self>> {
        match read("CONCERT_PARTY_MODE")?.as_deref() {
            None | Some("off") => return Ok(None),
            Some("live") => (),
            _ => anyhow::bail!("CONCERT_PARTY_MODE must be off or live"),
        }
        anyhow::ensure!(
            guarded_grpc && shared_canonical,
            "Live Parties require guarded gRPC and the shared canonical owner composition"
        );
        let cursor = secret(&required(&mut read, "CONCERT_PARTY_CURSOR_KEY_HEX")?)?;
        let raw = required(&mut read, "CONCERT_PARTY_RECEIPT_KEYS_JSON")?;
        anyhow::ensure!(
            raw.len() <= 16 * 1024,
            "Party receipt configuration is too large"
        );
        let keys: Vec<Key> = serde_json::from_str(&raw)
            .map_err(|_| anyhow::anyhow!("Party keys require a closed JSON array"))?;
        anyhow::ensure!(
            (1..=32).contains(&keys.len()),
            "Party keys require one active and at most 31 retained keys"
        );
        let mut ids = BTreeSet::new();
        let mut values = BTreeSet::from([cursor]);
        let mut keys = keys
            .into_iter()
            .map(|key| {
                let value = secret(&key.secret_hex)?;
                anyhow::ensure!(
                    ids.insert(key.id.clone()) && values.insert(value),
                    "Party receipt IDs and cursor/receipt secrets must be independent"
                );
                Ok(SigningKey::new(&key.id, &value)?)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let active = keys.remove(0);
        Ok(Some(Self {
            receipts: Arc::new(FingerprintKeys::new(active, keys)?),
            cursor,
        }))
    }

    pub fn load(guarded_grpc: bool, shared_canonical: bool) -> anyhow::Result<Option<Self>> {
        Self::from_values(
            |key| match std::env::var(key) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => anyhow::bail!("Invalid Party configuration encoding"),
            },
            guarded_grpc,
            shared_canonical,
        )
    }

    pub fn compose(
        self,
        pool: sqlx::PgPool,
        owners: &CanonicalOwnerComposition,
        base: ProductOwners,
    ) -> anyhow::Result<ProductOwners> {
        let producer = ScopedPartyProductProducer {
            pool,
            keys: self.receipts,
            cursors: PartyCursorCodec::new(&self.cursor)
                .map_err(|_| anyhow::anyhow!("Invalid Party cursor key"))?,
            profiles: owners.public_profile.clone(),
            resources: PgPartyResources,
        };
        let delivery = owners.core.delivery();
        Ok(ProductOwners {
            product: Arc::new(PartyProductOwner {
                party: ScopedProductOwner::new(
                    owners.fence.clone(),
                    delivery.clone(),
                    Arc::new(producer),
                ),
                base: base.product,
            }),
            delivery,
        })
    }
}

struct PartyProductOwner {
    party: ScopedProductOwner,
    base: Arc<dyn ProductOwner>,
}

#[async_trait]
impl ProductOwner for PartyProductOwner {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        request: proto::ProductRequest,
    ) -> Result<tonic::Response<proto::ProductResponse>, tonic::Status> {
        if !matches!(
            context.operation(),
            Operation::ListParties
                | Operation::GetParty(_)
                | Operation::CreateParty
                | Operation::ApplyParty(_)
                | Operation::DecidePartyApplication(_)
                | Operation::CancelPartyApplication(_)
                | Operation::RemovePartyMember(_, _)
                | Operation::CancelParty(_)
        ) {
            if matches!(
                context.operation(),
                Operation::UpdateParty(_)
                    | Operation::TransferParty(_)
                    | Operation::ListPartyApplications(_)
                    | Operation::GetPartyApplication(_)
                    | Operation::ListPartyMembers(_)
                    | Operation::GetPartyRally(_)
                    | Operation::SetPartyRally(_)
                    | Operation::PartyActivity
            ) {
                return Err(tonic::Status::unavailable(
                    "Party operation producer unavailable",
                ));
            }
            return self.base.dispatch(context, request).await;
        }
        self.party.dispatch(context, request).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(
        mode: &str,
        cursor: &str,
        keys: &str,
        grpc: bool,
        canonical: bool,
    ) -> anyhow::Result<Option<PartyRuntimeSettings>> {
        PartyRuntimeSettings::from_values(
            |name| {
                Ok(match name {
                    "CONCERT_PARTY_MODE" => Some(mode.into()),
                    "CONCERT_PARTY_CURSOR_KEY_HEX" => Some(cursor.into()),
                    "CONCERT_PARTY_RECEIPT_KEYS_JSON" => Some(keys.into()),
                    _ => None,
                })
            },
            grpc,
            canonical,
        )
    }

    #[test]
    fn off_does_not_require_any_dependencies_or_read_secrets() {
        assert!(PartyRuntimeSettings::from_values(
            |name| {
                assert_eq!(name, "CONCERT_PARTY_MODE");
                Ok(None)
            },
            false,
            false
        )
        .unwrap()
        .is_none());
    }

    #[test]
    fn live_requires_shared_guard_and_independent_retained_keys() {
        let cursor = "11".repeat(32);
        let keys = format!(r#"[{{"id":"active","secret_hex":"{}"}}]"#, "22".repeat(32));
        assert!(parse("live", &cursor, &keys, true, true).unwrap().is_some());
        assert!(parse("live", &cursor, &keys, false, true).is_err());
        assert!(parse("live", &cursor, &keys, true, false).is_err());
        assert!(parse("live", &"00".repeat(32), &keys, true, true).is_err());
        assert!(parse("live", &"22".repeat(32), &keys, true, true).is_err());
        assert!(parse("mock", &cursor, &keys, true, true).is_err());
    }
}
