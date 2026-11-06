//! Explicit uploader-media factory over the existing shared canonical scope.
//! The runtime, producer and fence already exist; this module only composes them
//! behind the request-bound Product owner. No route activation and no fallback.
use super::{canonical_runtime::CanonicalOwnerComposition, product_runtime::ProductOwners};
use crate::{
    adapters::out::{
        media::codec::BoundedImageSanitizer,
        relationship_authority::guarded::OwnerGuardScope,
    },
    media_runtime::{
        canonical::CanonicalOwnerMediaFence, OwnerMediaIdentity, OwnerMediaReply,
        OwnerMediaRequest, OwnerMediaRuntime,
    },
    product_rpc::{
        proto,
        scoped::{ProductRepresentation, ScopedProductOwner, ScopedProductProducer},
        Operation, ProductOwner, VerifiedProduct,
    },
};
use async_trait::async_trait;
use concert_application::commands::{CommandError, FingerprintKeys, SigningKey};
use platform::cpu::WorkerPool;
use serde::Deserialize;
use sqlx::PgPool;
use std::{collections::BTreeSet, sync::Arc};
use tonic::Status;

/// Operator-supplied secrets deliberately have no Debug/Serialize implementation.
pub struct OwnerMediaSettings {
    receipts: Arc<FingerprintKeys>,
    sanitizer: Arc<BoundedImageSanitizer>,
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
        "Media receipt secrets require 32-byte lowercase hexadecimal values"
    );
    let mut out = [0; 32];
    for (target, pair) in out.iter_mut().zip(value.as_bytes().chunks_exact(2)) {
        let digit = |b| if b <= b'9' { b - b'0' } else { b - b'a' + 10 };
        *target = digit(pair[0]) * 16 + digit(pair[1]);
    }
    anyhow::ensure!(out != [0; 32], "Media receipt secrets must be nonzero");
    Ok(out)
}
fn required(
    read: &mut impl FnMut(&str) -> anyhow::Result<Option<String>>,
    key: &str,
) -> anyhow::Result<String> {
    read(key)?
        .filter(|v| !v.is_empty())
        .ok_or_else(|| anyhow::anyhow!("{key} is required for uploader media"))
}
fn bounded(
    read: &mut impl FnMut(&str) -> anyhow::Result<Option<String>>,
    key: &str,
    range: std::ops::RangeInclusive<usize>,
) -> anyhow::Result<usize> {
    let raw = required(read, key)?;
    anyhow::ensure!(
        raw.bytes().all(|b| b.is_ascii_digit()),
        "{key} requires a decimal integer"
    );
    let value = raw
        .parse::<usize>()
        .map_err(|_| anyhow::anyhow!("{key} requires a bounded decimal integer"))?;
    anyhow::ensure!(range.contains(&value), "{key} is outside its bound");
    Ok(value)
}

impl OwnerMediaSettings {
    /// Disabled mode reads no unrelated configuration or dependency credentials.
    pub fn from_values(
        mut read: impl FnMut(&str) -> anyhow::Result<Option<String>>,
        guarded_grpc: bool,
        shared_canonical: bool,
    ) -> anyhow::Result<Option<Self>> {
        match read("CONCERT_OWNER_MEDIA_MODE")?.as_deref() {
            None | Some("off") => return Ok(None),
            Some("uploader") => (),
            _ => anyhow::bail!("CONCERT_OWNER_MEDIA_MODE must be off or uploader"),
        }
        anyhow::ensure!(
            guarded_grpc && shared_canonical,
            "Uploader media requires guarded gRPC and the shared canonical owner composition"
        );
        let workers = bounded(&mut read, "CONCERT_MEDIA_SANITIZER_WORKERS", 1..=64)?;
        let queue = bounded(&mut read, "CONCERT_MEDIA_SANITIZER_QUEUE", 0..=1024)?;
        let raw = required(&mut read, "CONCERT_MEDIA_RECEIPT_KEYS_JSON")?;
        anyhow::ensure!(
            raw.len() <= 16 * 1024,
            "Media receipt configuration is too large"
        );
        let keys: Vec<Key> = serde_json::from_str(&raw)
            .map_err(|_| anyhow::anyhow!("Media receipt keys require a closed JSON array"))?;
        anyhow::ensure!(
            (1..=32).contains(&keys.len()),
            "Media receipt keys require one active and at most 31 retained keys"
        );
        let mut ids = BTreeSet::new();
        let mut values = BTreeSet::new();
        let mut keys = keys
            .into_iter()
            .map(|key| {
                let value = secret(&key.secret_hex)?;
                anyhow::ensure!(
                    ids.insert(key.id.clone()) && values.insert(value),
                    "Media receipt IDs and secrets must be independent"
                );
                Ok(SigningKey::new(&key.id, &value)?)
            })
            .collect::<anyhow::Result<Vec<_>>>()?;
        let active = keys.remove(0);
        let class = WorkerPool::new(workers)?.class(queue)?;
        Ok(Some(Self {
            receipts: Arc::new(FingerprintKeys::new(active, keys)?),
            sanitizer: Arc::new(BoundedImageSanitizer::new(class)),
        }))
    }
    pub fn load(guarded_grpc: bool, shared_canonical: bool) -> anyhow::Result<Option<Self>> {
        Self::from_values(
            |key| match std::env::var(key) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => anyhow::bail!("Invalid media configuration encoding"),
            },
            guarded_grpc,
            shared_canonical,
        )
    }
    /// Reuse the canonical fence, delivery publisher and database pool; do not
    /// create a second owner core, scope registry or listener.
    pub fn compose(
        self,
        pool: PgPool,
        owners: &CanonicalOwnerComposition,
        base: ProductOwners,
    ) -> anyhow::Result<ProductOwners> {
        let producer = OwnerMediaProducer {
            pool,
            sanitizer: self.sanitizer,
            keys: self.receipts,
        };
        let delivery = owners.core.delivery();
        Ok(ProductOwners {
            product: Arc::new(MediaProductOwner {
                media: ScopedProductOwner::new(
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
fn handles(operation: Operation) -> bool {
    matches!(
        operation,
        Operation::InitializeUpload
            | Operation::MediaMetadata(_)
            | Operation::UploadBytes(_)
            | Operation::FinalizeUpload(_)
            | Operation::PrivateContent(_)
    )
}
struct MediaProductOwner {
    media: ScopedProductOwner,
    base: Arc<dyn ProductOwner>,
}
#[async_trait]
impl ProductOwner for MediaProductOwner {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        request: proto::ProductRequest,
    ) -> Result<tonic::Response<proto::ProductResponse>, Status> {
        if handles(context.operation()) {
            self.media.dispatch(context, request).await
        } else {
            self.base.dispatch(context, request).await
        }
    }
}
struct OwnerMediaProducer {
    pool: PgPool,
    sanitizer: Arc<BoundedImageSanitizer>,
    keys: Arc<FingerprintKeys>,
}
#[async_trait]
impl ScopedProductProducer for OwnerMediaProducer {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        let operation = match context.operation() {
            Operation::InitializeUpload => "initConcertUpload",
            Operation::UploadBytes(_) => "putConcertUploadBytes",
            Operation::FinalizeUpload(_) => "finalizeConcertUpload",
            Operation::MediaMetadata(_) => "getConcertMedia",
            Operation::PrivateContent(_) => "readConcertMedia",
            _ => return Err(Status::permission_denied("Unsupported media operation")),
        };
        let input = OwnerMediaRequest::from_verified(
            OwnerMediaIdentity {
                actor: context.actor(),
                event: context.event(),
                session_family: context.session(),
                request_id: context.request(),
                deadline: context.deadline(),
            },
            operation,
            &request.http_method,
            &request.http_target,
            &request.content_type,
            &request.idempotency_key,
            request.http_body,
        )
        .map_err(failure)?;
        // The fence is bound to this exact verified frame and the scope Root
        // already acquired; construction grants nothing on its own.
        let fence = CanonicalOwnerMediaFence::from_request(scope, input.binding()).map_err(failure)?;
        let runtime = OwnerMediaRuntime::new(
            self.pool.clone(),
            self.sanitizer.clone(),
            self.keys.clone(),
            Arc::new(fence),
        );
        let reply = runtime.dispatch(input).await.map_err(failure)?;
        Ok(match reply {
            OwnerMediaReply::Json { status, body } => ProductRepresentation {
                status: status.into(),
                content_type: "application/json".into(),
                body: serde_json::to_vec(&body)
                    .map_err(|_| Status::internal("Invalid media representation"))?,
            },
            OwnerMediaReply::Image(image) => ProductRepresentation {
                status: 200,
                content_type: image.metadata.mime.as_str().into(),
                body: image.bytes,
            },
        })
    }
}
fn failure(error: CommandError) -> Status {
    match error {
        CommandError::InvalidInput => Status::invalid_argument("Invalid media request"),
        CommandError::Denied => Status::permission_denied("Media request denied"),
        CommandError::Unavailable => Status::unavailable("Media owner unavailable"),
        CommandError::Conflict => Status::aborted("Media idempotency conflict"),
        CommandError::Expired => Status::aborted("Media idempotency expired"),
        CommandError::Capacity => Status::resource_exhausted("Media capacity unavailable"),
        CommandError::InvalidState => Status::aborted("Invalid media transition"),
    }
}
