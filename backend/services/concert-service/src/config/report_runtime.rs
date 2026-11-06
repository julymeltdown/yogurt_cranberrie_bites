//! Explicit USER/no-media report and event-administration startup over Root's
//! existing canonical owners. The operator role and reviewed-notice owners lock
//! their durable rows on each command transaction; neither has a positive default.
//! No listener, fallback key, operator grant or second owner lifecycle is created.
use crate::{
    config::{canonical_runtime::CanonicalOwnerComposition, product_runtime::ProductOwners},
    operations_live::{
        report_product::ReportProduct, OperationsOwners, OperationsRuntime, PgOperationsReview,
        PgOperationsRoles,
    },
    product_rpc::{proto, scoped::ScopedProductOwner, Operation, ProductOwner, VerifiedProduct},
};
use concert_application::commands::{FingerprintKeys, SigningKey};
use serde::Deserialize;
use std::{collections::BTreeSet, sync::Arc};

/// Deliberately neither Debug nor Serialize: these are managed private secrets.
pub struct ReportStartupSettings {
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
        .ok_or_else(|| anyhow::anyhow!("{name} is required for USER reports"))
}
fn secret(raw: &str) -> anyhow::Result<[u8; 32]> {
    anyhow::ensure!(
        raw.len() == 64
            && raw
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "Report secrets require 32-byte lowercase hexadecimal values"
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
    anyhow::ensure!(value != [0; 32], "Report secrets must be nonzero");
    Ok(value)
}

impl ReportStartupSettings {
    /// Pure, before database/listener construction. Default/off inspects only
    /// the report mode; unrelated malformed feature inputs cannot activate it.
    pub fn from_lookup(
        mut read: impl FnMut(&str) -> anyhow::Result<Option<String>>,
        grpc: bool,
        shared_canonical: bool,
    ) -> anyhow::Result<Option<Self>> {
        match read_value(&mut read, "CONCERT_REPORT_MODE")?.as_deref() {
            None | Some("off") => return Ok(None),
            Some("user") => (),
            _ => anyhow::bail!("CONCERT_REPORT_MODE must be off or user"),
        }
        anyhow::ensure!(
            grpc && shared_canonical,
            "USER reports require guarded gRPC and Root's shared canonical composition"
        );
        let cursor = secret(&required(&mut read, "CONCERT_REPORT_CURSOR_KEY_HEX")?)?;
        let raw = required(&mut read, "CONCERT_REPORT_RECEIPT_KEYS_JSON")?;
        anyhow::ensure!(
            raw.len() <= 16 * 1024,
            "Report key configuration is too large"
        );
        // Direct struct decoding rejects duplicate fields and unknown fields.
        let keys: Vec<Key> = serde_json::from_str(&raw)
            .map_err(|_| anyhow::anyhow!("Report keys require a closed JSON array"))?;
        anyhow::ensure!(
            (1..=32).contains(&keys.len()),
            "Report keys require one active and at most 31 retained versions"
        );
        let mut secrets = BTreeSet::from([cursor]);
        let mut versions = Vec::with_capacity(keys.len());
        for key in keys {
            let material = secret(&key.secret_hex)?;
            anyhow::ensure!(
                secrets.insert(material),
                "Report receipt versions and cursor secrets must be independent"
            );
            versions.push(
                SigningKey::new(&key.id, &material)
                    .map_err(|_| anyhow::anyhow!("Invalid Report receipt key version"))?,
            );
        }
        let active = versions.remove(0);
        let receipts = FingerprintKeys::new(active, versions)
            .map_err(|_| anyhow::anyhow!("Invalid Report receipt key versions"))?;
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
                Err(_) => anyhow::bail!("Invalid Report configuration encoding"),
            },
            grpc,
            shared_canonical,
        )
    }

    /// Root supplies its SAME Concert pool, canonical core/fence and base owner
    /// composition. The existing Participant shared assembly already mounts the
    /// actual NativeResolverUnion (including Operations); the core alone does
    /// not supply native COMMIT coverage. No report child-read settler is needed.
    pub fn compose_shared(
        self,
        pool: sqlx::PgPool,
        owners: &CanonicalOwnerComposition,
        base: ProductOwners,
    ) -> anyhow::Result<ProductOwners> {
        let mut operations_owners = OperationsOwners::default();
        operations_owners.roles = Some(Arc::new(PgOperationsRoles));
        operations_owners.review = Arc::new(PgOperationsReview);
        let runtime = Arc::new(OperationsRuntime::new(
            pool,
            operations_owners,
            self.receipts,
            &self.cursor,
        )?);
        let delivery = owners.core.delivery();
        let report = Arc::new(ScopedProductOwner::new(
            owners.fence.clone(),
            delivery.clone(),
            Arc::new(ReportProduct::new(runtime)),
        ));
        Ok(ProductOwners {
            product: Arc::new(ReportOverlay::new(base.product, report)),
            delivery,
        })
    }
}

/// Internal dispatch composition; the production constructor above supplies the
/// actual scoped report owner. Other already-admitted operations retain base.
pub(crate) struct ReportOverlay {
    base: Arc<dyn ProductOwner>,
    report: Arc<dyn ProductOwner>,
}
impl ReportOverlay {
    pub(crate) fn new(base: Arc<dyn ProductOwner>, report: Arc<dyn ProductOwner>) -> Self {
        Self { base, report }
    }
}
#[async_trait::async_trait]
impl ProductOwner for ReportOverlay {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        request: proto::ProductRequest,
    ) -> Result<tonic::Response<proto::ProductResponse>, tonic::Status> {
        match context.operation() {
            Operation::Report(_) | Operation::Operations(_) => {
                self.report.dispatch(context, request).await
            }
            _ => self.base.dispatch(context, request).await,
        }
    }
}
