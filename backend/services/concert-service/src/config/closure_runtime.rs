//! Root operator closure composition and the frozen public archive read.
//! Authority is the durable BE38 event-scoped role row (`0076_operations_authority.sql`
//! after the Ops lane renumber); this module seeds no role, grant or claim. The one
//! World `AdmissionBridge` is received from the serving World runtime, never rebuilt.
use crate::{
    config::{canonical_runtime::CanonicalOwnerComposition, product_runtime::ProductOwners},
    event_lifecycle::{
        archive::{
            ArchiveCursor, ArchivePost, PgPublicArchive, Position, PublicArchiveOwner, PublicPage,
        },
        read_only::ReadOnlyCommandDriver,
        Applied, AuthorityFence, AuthorityQuery, EffectKey, FencedLifecycleAuthority,
        LifecycleOwners,
    },
    product_rpc::{
        proto,
        scoped::{ScopedProductOwner, ScopedProductProducer},
        ProductOwner, VerifiedProduct,
    },
    world::admission_bridge::AdmissionBridge,
};
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::{
    commands::{CommandError, FingerprintKeys, SigningKey},
    event_lifecycle::Actor,
};
use concert_domain::event_lifecycle::Retention;
use platform::{
    actor_delegation::authority_deadline::AuthorityRpcDeadline,
    grpc_auth::{VerifiedCaller, Workload},
    grpc_method_policy::MethodPolicy,
};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use tonic::{Request, Response, Status};
use uuid::Uuid;

pub const ARCHIVE_POLICY: MethodPolicy = MethodPolicy {
    service: "concert.product.v1.ProductArchiveService",
    workload: Some(Workload::Concert),
    methods: &[("GetArchive", &[Workload::Gateway])],
    actor_fields: &[],
};
const AUTHORITY_SECONDS: i64 = 10;

#[derive(Clone)]
pub struct ClosureSettings {
    retention_hours: i64,
    policy_revision: i64,
    receipt_key: [u8; 32],
    cursor_key: [u8; 32],
    archive_limit: u16,
}

impl ClosureSettings {
    pub fn load(world: bool, canonical: bool, guarded: bool) -> anyhow::Result<Option<Self>> {
        let read = |key: &'static str| -> anyhow::Result<Option<String>> {
            match std::env::var(key) {
                Ok(value) => Ok(Some(value)),
                Err(std::env::VarError::NotPresent) => Ok(None),
                Err(_) => anyhow::bail!("{key} must contain valid Unicode"),
            }
        };
        Self::from_values(
            read("CONCERT_CLOSURE_MODE")?.as_deref(),
            read("CONCERT_CLOSURE_RETENTION_HOURS")?.as_deref(),
            read("CONCERT_CLOSURE_POLICY_REVISION")?.as_deref(),
            read("CONCERT_CLOSURE_RECEIPT_KEY_HEX")?.as_deref(),
            read("CONCERT_CLOSURE_ARCHIVE_CURSOR_KEY_HEX")?.as_deref(),
            read("CONCERT_CLOSURE_ARCHIVE_LIMIT")?.as_deref(),
            world,
            canonical,
            guarded,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn from_values(
        mode: Option<&str>,
        retention_hours: Option<&str>,
        policy_revision: Option<&str>,
        receipt_key: Option<&str>,
        cursor_key: Option<&str>,
        archive_limit: Option<&str>,
        world: bool,
        canonical: bool,
        guarded: bool,
    ) -> anyhow::Result<Option<Self>> {
        match mode {
            None | Some("off") => {
                anyhow::ensure!(
                    retention_hours.is_none()
                        && policy_revision.is_none()
                        && receipt_key.is_none()
                        && cursor_key.is_none()
                        && archive_limit.is_none(),
                    "disabled closure must not specify retained material"
                );
                Ok(None)
            }
            Some("operator") => {
                anyhow::ensure!(
                    world && canonical && guarded,
                    "operator closure requires the shared canonical and World composition"
                );
                let hours = bounded(
                    retention_hours.ok_or_else(|| anyhow::anyhow!("closure retention required"))?,
                    1,
                    24 * 365 * 10,
                )?;
                let revision = bounded(
                    policy_revision
                        .ok_or_else(|| anyhow::anyhow!("closure policy revision required"))?,
                    1,
                    9_007_199_254_740_991,
                )?;
                let limit = archive_limit
                    .map(|value| bounded(value, 1, 100))
                    .transpose()?
                    .unwrap_or(20);
                Ok(Some(Self {
                    retention_hours: hours,
                    policy_revision: revision,
                    receipt_key: key(receipt_key
                        .ok_or_else(|| anyhow::anyhow!("closure receipt key required"))?)?,
                    cursor_key: key(
                        cursor_key.ok_or_else(|| anyhow::anyhow!("archive cursor key required"))?
                    )?,
                    archive_limit: u16::try_from(limit).expect("bounded archive limit"),
                }))
            }
            _ => anyhow::bail!("CONCERT_CLOSURE_MODE must be off or operator"),
        }
    }

    pub fn compose(
        self,
        pool: PgPool,
        handoff: BridgeHandoff,
        owners: &CanonicalOwnerComposition,
        base: ProductOwners,
    ) -> anyhow::Result<(ProductOwners, ArchiveService)> {
        let keys = Arc::new(
            FingerprintKeys::new(
                SigningKey::new("concert-closure", &self.receipt_key)
                    .map_err(|_| anyhow::anyhow!("closure receipt key is invalid"))?,
                Vec::new(),
            )
            .map_err(|_| anyhow::anyhow!("closure receipt keys are invalid"))?,
        );
        let producer = ClosureProducer {
            handoff,
            authority: RootOperatorAuthority {
                retention_hours: self.retention_hours,
                policy_revision: self.policy_revision,
            },
            keys,
        };
        let closure = ScopedProductOwner::new(
            owners.fence.clone(),
            owners.core.delivery(),
            Arc::new(producer),
        );
        let archive = PgPublicArchive::new(
            pool,
            PostPublicArchive,
            ArchiveCursor::new(&self.cursor_key)
                .map_err(|_| anyhow::anyhow!("archive cursor key is invalid"))?,
        );
        Ok((
            ProductOwners {
                product: Arc::new(ClosureProductOwner {
                    closure,
                    base: base.product,
                }),
                delivery: base.delivery,
            },
            ArchiveService {
                archive,
                default_limit: self.archive_limit,
            },
        ))
    }
}

fn bounded(value: &str, minimum: i64, maximum: i64) -> anyhow::Result<i64> {
    let value: i64 = value
        .parse()
        .map_err(|_| anyhow::anyhow!("closure setting must be a decimal integer"))?;
    anyhow::ensure!(
        (minimum..=maximum).contains(&value),
        "closure setting is out of bounds"
    );
    Ok(value)
}

fn key(value: &str) -> anyhow::Result<[u8; 32]> {
    anyhow::ensure!(
        value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()),
        "closure key must be 64 hexadecimal characters"
    );
    let mut bytes = [0u8; 32];
    for (index, chunk) in value.as_bytes().chunks_exact(2).enumerate() {
        bytes[index] = (hex(chunk[0]).expect("checked hexadecimal") << 4)
            | hex(chunk[1]).expect("checked hexadecimal");
    }
    anyhow::ensure!(bytes != [0; 32], "closure key must be nonzero");
    Ok(bytes)
}

fn hex(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Handoff of the exact serving World bridge. The channel is created before the
/// World runtime and the product listener only starts after World readiness.
#[derive(Clone)]
pub struct BridgeHandoff {
    slot: Arc<tokio::sync::OnceCell<Arc<AdmissionBridge>>>,
    receiver: tokio::sync::watch::Receiver<Option<Arc<AdmissionBridge>>>,
}

impl BridgeHandoff {
    pub fn channel() -> (
        Self,
        tokio::sync::watch::Sender<Option<Arc<AdmissionBridge>>>,
    ) {
        let (sender, receiver) = tokio::sync::watch::channel(None);
        (
            Self {
                slot: Arc::new(tokio::sync::OnceCell::new()),
                receiver,
            },
            sender,
        )
    }

    async fn get(&self, deadline: AuthorityRpcDeadline) -> Option<Arc<AdmissionBridge>> {
        if let Some(bridge) = self.slot.get() {
            return Some(bridge.clone());
        }
        let mut receiver = self.receiver.clone();
        let work = async {
            let value = receiver.wait_for(|value| value.is_some()).await.ok()?;
            let bridge = value.clone()?;
            let _ = self.slot.set(bridge.clone());
            Some(bridge)
        };
        tokio::time::timeout_at(deadline.stop(), work)
            .await
            .ok()
            .flatten()
    }
}

#[derive(Clone)]
pub struct RootOperatorAuthority {
    retention_hours: i64,
    policy_revision: i64,
}

impl RootOperatorAuthority {
    pub fn new(retention_hours: i64, policy_revision: i64) -> Self {
        Self {
            retention_hours,
            policy_revision,
        }
    }
}

#[async_trait]
impl FencedLifecycleAuthority for RootOperatorAuthority {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        query: AuthorityQuery,
        now: DateTime<Utc>,
    ) -> Result<AuthorityFence, CommandError> {
        if [query.actor.actor, query.actor.event, query.job]
            .iter()
            .any(Uuid::is_nil)
        {
            return Err(CommandError::Denied);
        }
        let revision: Option<i64> = sqlx::query_scalar(
            "select revision from public.concert_event_operator_roles where event_id=$1 and user_id=$2 and role='OPERATOR' and revoked_at is null for share",
        )
        .bind(query.actor.event)
        .bind(query.actor.actor)
        .fetch_optional(&mut *c)
        .await
        .map_err(|_| CommandError::Unavailable)?;
        let revision = revision.ok_or(CommandError::Denied)?;
        if !(1..=concert_domain::MAX_REVISION).contains(&revision) {
            return Err(CommandError::Denied);
        }
        let expires_at = now + Duration::seconds(AUTHORITY_SECONDS);
        sqlx::query("select set_config('mcr.relationship_authority_until',least(coalesce(nullif(current_setting('mcr.relationship_authority_until',true),'')::timestamptz,'infinity'::timestamptz),$1::timestamptz)::text,true)")
            .bind(expires_at)
            .execute(&mut *c)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        Ok(AuthorityFence {
            query,
            auth_revision: revision,
            role_revision: revision,
            policy_revision: self.policy_revision,
            checked_at: now,
            expires_at,
        })
    }

    async fn retention_in(
        &self,
        _: &mut PgConnection,
        _: Actor,
        closed_at: DateTime<Utc>,
    ) -> Result<Retention, CommandError> {
        Ok(Retention {
            policy_revision: self.policy_revision,
            closed_at,
            cleanup_after: closed_at + Duration::hours(self.retention_hours),
        })
    }
}

/// Later closure steps remain unavailable until their actual owning adapters are
/// composed; START and READ_ONLY do not depend on them.
#[derive(Clone)]
struct UnavailableLater;

#[async_trait]
impl LifecycleOwners for UnavailableLater {
    async fn apply_in(
        &self,
        _: &mut PgConnection,
        _: EffectKey,
        _: Retention,
    ) -> Result<Applied, CommandError> {
        Err(CommandError::Unavailable)
    }
}

struct ClosureProducer {
    handoff: BridgeHandoff,
    authority: RootOperatorAuthority,
    keys: Arc<FingerprintKeys>,
}

#[async_trait]
impl ScopedProductProducer for ClosureProducer {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        _scope: Arc<crate::adapters::out::relationship_authority::guarded::OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<crate::product_rpc::scoped::ProductRepresentation, Status> {
        context.check()?;
        if context.operation() != crate::product_rpc::Operation::Closure {
            return Err(Status::permission_denied("Unsupported closure operation"));
        }
        let bridge = self
            .handoff
            .get(context.deadline())
            .await
            .ok_or_else(|| Status::unavailable("Closure World owner unavailable"))?;
        let driver = ReadOnlyCommandDriver::new(self.authority.clone(), UnavailableLater, bridge)
            .map_err(|_| Status::unavailable("Closure World owner unavailable"))?;
        let actor = Actor {
            actor: context.actor(),
            event: context.event(),
        };
        let key = crate::product_rpc::canonical_uuid(&request.idempotency_key)?;
        let response = driver
            .close_response(actor, key, &request.http_body, &self.keys)
            .await;
        let mut representation = representation(response).await?;
        if representation.status == 404 {
            representation.status = 403;
            representation.body = serde_json::to_vec(&serde_json::json!({
                "code": "CONCERT_403",
                "message": "Concert request unavailable",
                "traceId": context.request().to_string(),
                "timestamp": chrono::Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true),
                "details": {"reason": "PERMISSION_DENIED"},
            }))
            .map_err(|_| Status::internal("Invalid closure representation"))?;
        }
        Ok(representation)
    }
}

async fn representation(
    response: axum::response::Response,
) -> Result<crate::product_rpc::scoped::ProductRepresentation, Status> {
    let status = u32::from(response.status().as_u16());
    let content_type = response
        .headers()
        .get(axum::http::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .unwrap_or("")
        .to_owned();
    let body = axum::body::to_bytes(response.into_body(), crate::product_rpc::MAX_BODY)
        .await
        .map_err(|_| Status::internal("Invalid closure representation"))?
        .to_vec();
    Ok(crate::product_rpc::scoped::ProductRepresentation {
        status,
        content_type,
        body,
    })
}

struct ClosureProductOwner {
    closure: ScopedProductOwner,
    base: Arc<dyn ProductOwner>,
}

#[async_trait]
impl ProductOwner for ClosureProductOwner {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        request: proto::ProductRequest,
    ) -> Result<Response<proto::ProductResponse>, Status> {
        match context.operation() {
            crate::product_rpc::Operation::Closure => self.closure.dispatch(context, request).await,
            _ => self.base.dispatch(context, request).await,
        }
    }
}

pub struct ArchiveService {
    archive: PgPublicArchive<PostPublicArchive>,
    default_limit: u16,
}

#[tonic::async_trait]
impl proto::product_archive_service_server::ProductArchiveService for ArchiveService {
    async fn get_archive(
        &self,
        request: Request<proto::ArchiveRequest>,
    ) -> Result<Response<proto::ArchiveResponse>, Status> {
        if !matches!(
            request.extensions().get::<VerifiedCaller>(),
            Some(VerifiedCaller(Workload::Gateway))
        ) {
            return Err(Status::unauthenticated("Verified Gateway required"));
        }
        let input = request.into_inner();
        let event = crate::product_rpc::canonical_uuid(&input.event_id)?;
        let limit = if input.limit == 0 {
            self.default_limit
        } else {
            u16::try_from(input.limit)
                .map_err(|_| Status::invalid_argument("Invalid archive page"))?
        };
        if !(1..=100).contains(&limit) {
            return Err(Status::invalid_argument("Invalid archive page"));
        }
        let cursor = (!input.cursor.is_empty()).then_some(input.cursor.as_str());
        let response =
            crate::event_lifecycle::http::archive_response(&self.archive, event, limit, cursor)
                .await;
        let representation = representation(response).await?;
        let mut result = Response::new(proto::ArchiveResponse {
            status: representation.status,
            content_type: representation.content_type,
            body: representation.body,
        });
        result.metadata_mut().insert(
            "cache-control",
            "no-store".parse().expect("static metadata"),
        );
        Ok(result)
    }
}

/// Permitted anonymous projection. Media ids are never projected from private
/// storage; post text is only current PUBLISHED content and never deleted rows.
pub struct PostPublicArchive;

fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}

#[async_trait]
impl PublicArchiveOwner for PostPublicArchive {
    async fn page_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        upper: DateTime<Utc>,
        after: Option<Position>,
        limit: u16,
    ) -> Result<PublicPage, CommandError> {
        let source_revision: i64 = sqlx::query_scalar(
            "select coalesce(sum(revision),0)::bigint from post_concert_posts where event_id=$1",
        )
        .bind(event)
        .fetch_one(&mut *c)
        .await
        .map_err(db)?;
        let rows = sqlx::query(
            "select post_id,content->>'text' as text,created_at from post_concert_posts where event_id=$1 and state='PUBLISHED' and jsonb_typeof(content->'text')='string' and created_at<=$2 and ($3::timestamptz is null or (created_at,post_id)<($3,$4)) order by created_at desc,post_id desc limit $5",
        )
        .bind(event)
        .bind(upper)
        .bind(after.map(|position| position.created_at))
        .bind(after.map(|position| position.post_id))
        .bind(i64::from(limit) + 1)
        .fetch_all(c)
        .await
        .map_err(db)?;
        let has_more = rows.len() > usize::from(limit);
        let items = rows
            .into_iter()
            .take(usize::from(limit))
            .map(|row| {
                Ok(ArchivePost {
                    post_id: row
                        .try_get("post_id")
                        .map_err(|_| CommandError::InvalidState)?,
                    text: row
                        .try_get("text")
                        .map_err(|_| CommandError::InvalidState)?,
                    media_ids: Vec::new(),
                    created_at: row
                        .try_get("created_at")
                        .map_err(|_| CommandError::InvalidState)?,
                })
            })
            .collect::<Result<Vec<_>, CommandError>>()?;
        Ok(PublicPage {
            source_revision,
            items,
            has_more,
        })
    }

    async fn fence_in(
        &self,
        c: &mut PgConnection,
        event: Uuid,
        revision: i64,
    ) -> Result<(), CommandError> {
        let actual: i64 = sqlx::query_scalar(
            "select coalesce(sum(revision),0)::bigint from post_concert_posts where event_id=$1",
        )
        .bind(event)
        .fetch_one(c)
        .await
        .map_err(db)?;
        if actual == revision {
            Ok(())
        } else {
            Err(CommandError::Unavailable)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn closure_settings_require_complete_nonzero_operator_composition() {
        assert!(
            ClosureSettings::from_values(None, None, None, None, None, None, true, true, true)
                .unwrap()
                .is_none()
        );
        assert!(ClosureSettings::from_values(
            Some("off"),
            Some("720"),
            None,
            None,
            None,
            None,
            true,
            true,
            true
        )
        .is_err());
        let key = |byte: &str| byte.repeat(32);
        assert!(ClosureSettings::from_values(
            Some("operator"),
            Some("720"),
            Some("3"),
            Some(&key("11")),
            Some(&key("22")),
            None,
            true,
            true,
            true
        )
        .unwrap()
        .is_some());
        assert!(ClosureSettings::from_values(
            Some("operator"),
            Some("720"),
            Some("3"),
            Some(&key("11")),
            Some(&key("22")),
            None,
            true,
            false,
            true
        )
        .is_err());
        assert!(ClosureSettings::from_values(
            Some("operator"),
            Some("720"),
            Some("3"),
            Some(&key("00")),
            Some(&key("22")),
            None,
            true,
            true,
            true
        )
        .is_err());
        assert!(ClosureSettings::from_values(
            Some("operator"),
            Some("0"),
            Some("3"),
            Some(&key("11")),
            Some(&key("22")),
            None,
            true,
            true,
            true
        )
        .is_err());
    }
}
