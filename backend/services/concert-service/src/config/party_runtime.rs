//! Explicit Party startup over Root's existing canonical owners.
//! No listener, fallback key, operator grant or second owner lifecycle is created.
use crate::{
    config::{canonical_runtime::CanonicalOwnerComposition, product_runtime::ProductOwners},
    party_live::{PartyCursorCodec, ScopedPartyProductProducer},
    product_rpc::{proto, scoped::ScopedProductOwner, Operation, ProductOwner, VerifiedProduct},
};
use concert_application::commands::{FingerprintKeys, SigningKey};
use serde::Deserialize;
use std::{collections::BTreeSet, sync::Arc};

/// Deliberately neither Debug nor Serialize: these are managed private secrets.
pub struct PartyStartupSettings {
    receipts: Arc<FingerprintKeys>,
    cursor: [u8; 32],
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Key {
    id: String,
    secret_hex: String,
}

fn read_value(
    read: &mut impl FnMut(&str) -> anyhow::Result<Option<String>>,
    name: &str,
) -> anyhow::Result<Option<String>> {
    read(name).map_err(|_| anyhow::anyhow!("Cannot read {name}"))
}
fn required(
    read: &mut impl FnMut(&str) -> anyhow::Result<Option<String>>,
    name: &str,
) -> anyhow::Result<String> {
    read_value(read, name)?
        .filter(|value| !value.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{name} is required for Live parties"))
}
fn secret(raw: &str) -> anyhow::Result<[u8; 32]> {
    anyhow::ensure!(
        raw.len() == 64
            && raw
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "Party secrets require 32-byte lowercase hexadecimal values"
    );
    let mut value = [0; 32];
    for (out, pair) in value.iter_mut().zip(raw.as_bytes().chunks_exact(2)) {
        let digit = |byte| {
            if byte <= b'9' {
                byte - b'0'
            } else {
                byte - b'a' + 10
            }
        };
        *out = digit(pair[0]) * 16 + digit(pair[1]);
    }
    anyhow::ensure!(value != [0; 32], "Party secrets must be nonzero");
    Ok(value)
}

impl PartyStartupSettings {
    /// Pure, before database/listener construction. Default/off inspects only
    /// the party mode; unrelated malformed feature inputs cannot activate it.
    pub fn from_lookup(
        mut read: impl FnMut(&str) -> anyhow::Result<Option<String>>,
        grpc: bool,
        shared_canonical: bool,
    ) -> anyhow::Result<Option<Self>> {
        match read_value(&mut read, "CONCERT_PARTY_MODE")?.as_deref() {
            None | Some("off") => return Ok(None),
            Some("live") => (),
            _ => anyhow::bail!("CONCERT_PARTY_MODE must be off or live"),
        }
        anyhow::ensure!(
            grpc && shared_canonical,
            "Live parties require guarded gRPC and Root's shared canonical composition"
        );
        let cursor = secret(&required(&mut read, "CONCERT_PARTY_CURSOR_KEY_HEX")?)?;
        let raw = required(&mut read, "CONCERT_PARTY_RECEIPT_KEYS_JSON")?;
        anyhow::ensure!(
            raw.len() <= 16 * 1024,
            "Party key configuration is too large"
        );
        // Direct struct decoding rejects duplicate fields and unknown fields.
        let keys: Vec<Key> = serde_json::from_str(&raw)
            .map_err(|_| anyhow::anyhow!("Party keys require a closed JSON array"))?;
        anyhow::ensure!(
            (1..=32).contains(&keys.len()),
            "Party keys require one active and at most 31 retained versions"
        );
        let mut secrets = BTreeSet::from([cursor]);
        let mut versions = Vec::with_capacity(keys.len());
        for key in keys {
            let material = secret(&key.secret_hex)?;
            anyhow::ensure!(
                secrets.insert(material),
                "Party receipt versions and cursor secrets must be independent"
            );
            versions.push(
                SigningKey::new(&key.id, &material)
                    .map_err(|_| anyhow::anyhow!("Invalid Party receipt key version"))?,
            );
        }
        let active = versions.remove(0);
        let receipts = FingerprintKeys::new(active, versions)
            .map_err(|_| anyhow::anyhow!("Invalid Party receipt key versions"))?;
        Ok(Some(Self {
            receipts: Arc::new(receipts),
            cursor,
        }))
    }

    pub fn load(grpc: bool, shared_canonical: bool) -> anyhow::Result<Option<Self>> {
        Self::from_lookup(
            |name| match std::env::var(name) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => anyhow::bail!("Invalid Party configuration encoding"),
            },
            grpc,
            shared_canonical,
        )
    }

    /// Consumes the SAME pool, canonical owner fence, PublicProfile reader and
    /// delivery lifecycle. Root must mount PartyNativeResolver in its existing
    /// NativeResolverUnion before enabling this startup in the actual main.
    pub fn compose_shared(
        self,
        pool: sqlx::PgPool,
        owners: &CanonicalOwnerComposition,
        base: ProductOwners,
    ) -> anyhow::Result<ProductOwners> {
        let delivery = owners.core.delivery();
        let producer = Arc::new(ScopedPartyProductProducer {
            pool,
            keys: self.receipts,
            cursors: PartyCursorCodec::new(&self.cursor)?,
            profiles: owners.public_profile.clone(),
            resources: resources::PgPartyResources,
        });
        let party = Arc::new(ScopedProductOwner::new(
            owners.fence.clone(),
            delivery.clone(),
            producer,
        ));
        Ok(ProductOwners {
            product: Arc::new(PartyOverlay {
                base: base.product,
                party,
            }),
            delivery,
        })
    }
}

#[path = "party_resources.rs"]
mod resources;

struct PartyOverlay {
    base: Arc<dyn ProductOwner>,
    party: Arc<dyn ProductOwner>,
}
#[async_trait::async_trait]
impl ProductOwner for PartyOverlay {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        request: proto::ProductRequest,
    ) -> Result<tonic::Response<proto::ProductResponse>, tonic::Status> {
        match context.operation() {
            Operation::ListParties
            | Operation::GetParty(_)
            | Operation::CreateParty
            | Operation::ApplyParty(_)
            | Operation::DecidePartyApplication(_)
            | Operation::CancelPartyApplication(_)
            | Operation::RemovePartyMember(_, _)
            | Operation::CancelParty(_) => self.party.dispatch(context, request).await,
            _ => self.base.dispatch(context, request).await,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn configured(name: &str) -> anyhow::Result<Option<String>> {
        Ok(match name {
            "CONCERT_PARTY_MODE" => Some("live".into()),
            "CONCERT_PARTY_CURSOR_KEY_HEX" => Some("11".repeat(32)),
            "CONCERT_PARTY_RECEIPT_KEYS_JSON" => Some(format!(
                r#"[{{"id":"current","secret_hex":"{}"}},{{"id":"retained","secret_hex":"{}"}}]"#,
                "22".repeat(32),
                "33".repeat(32)
            )),
            _ => None,
        })
    }
    #[test]
    fn live_requires_both_guarded_grpc_and_shared_canonical_owners() {
        for (grpc, shared) in [(false, false), (false, true), (true, false)] {
            assert!(PartyStartupSettings::from_lookup(configured, grpc, shared).is_err());
        }
        assert!(PartyStartupSettings::from_lookup(configured, true, true)
            .unwrap()
            .is_some());
    }
    #[test]
    fn off_does_not_read_or_activate_other_inputs() {
        for mode in [None, Some("off")] {
            let value = PartyStartupSettings::from_lookup(
                |name| {
                    assert_eq!(name, "CONCERT_PARTY_MODE");
                    Ok(mode.map(str::to_owned))
                },
                false,
                false,
            )
            .unwrap();
            assert!(value.is_none());
        }
    }
    #[test]
    fn rejects_unknown_mode_and_secret_reuse_without_echoing_secret() {
        let error = PartyStartupSettings::from_lookup(|_| Ok(Some("enabled".into())), true, true)
            .err()
            .unwrap();
        assert_eq!(error.to_string(), "CONCERT_PARTY_MODE must be off or live");
        let repeated = "11".repeat(32);
        let error = PartyStartupSettings::from_lookup(
            |name| {
                if name == "CONCERT_PARTY_RECEIPT_KEYS_JSON" {
                    Ok(Some(format!(
                        r#"[{{"id":"current","secret_hex":"{}"}}]"#,
                        repeated
                    )))
                } else {
                    configured(name)
                }
            },
            true,
            true,
        )
        .err()
        .unwrap();
        assert!(!error.to_string().contains(&repeated));
        assert!(error.to_string().contains("must be independent"));
    }
    #[test]
    fn rejects_unknown_duplicate_and_empty_key_versions() {
        for raw in [
            "[]".to_owned(),
            format!(
                r#"[{{"id":"current","id":"other","secret_hex":"{}"}}]"#,
                "22".repeat(32)
            ),
            format!(
                r#"[{{"id":"current","secret_hex":"{}","fallback":true}}]"#,
                "22".repeat(32)
            ),
            format!(
                r#"[{{"id":"current","secret_hex":"{}"}},{{"id":"current","secret_hex":"{}"}}]"#,
                "22".repeat(32),
                "33".repeat(32)
            ),
        ] {
            assert!(PartyStartupSettings::from_lookup(
                |name| {
                    if name == "CONCERT_PARTY_RECEIPT_KEYS_JSON" {
                        Ok(Some(raw.clone()))
                    } else {
                        configured(name)
                    }
                },
                true,
                true
            )
            .is_err());
        }
    }
}
