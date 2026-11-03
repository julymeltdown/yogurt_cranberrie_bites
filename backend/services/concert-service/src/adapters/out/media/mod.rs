//! BE24 durable producer adapter. HTTP composition and real fenced authority are injected.
pub mod body;
pub mod codec;
use super::repository::receipt::{CommandProducer, PgCommandRepository};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{commands::*, media::*};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

/// Must lock/recheck current canonical identity, event, BE37 block/restriction and purpose
/// policy on `c` through commit. For linked reads also reauthorize current target state,
/// audience and REPORT reporter/moderator access. Context is server-captured, not browser
/// authority. A stale fence, deleted/revoked account or unknown owner fails closed.
#[async_trait]
pub trait MediaAuthority: Send + Sync {
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        context: Context,
        access: Access,
        owner: Uuid,
        target: Option<Target>,
    ) -> Result<(), CommandError>;
}
pub struct UnavailableMediaAuthority;
#[async_trait]
impl MediaAuthority for UnavailableMediaAuthority {
    async fn authorize_in(
        &self,
        _: &mut PgConnection,
        _: Context,
        _: Access,
        _: Uuid,
        _: Option<Target>,
    ) -> Result<(), CommandError> {
        Err(CommandError::Unavailable)
    }
}
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Metadata {
    pub media_id: Uuid,
    pub event: Uuid,
    pub purpose: Purpose,
    pub mime: Mime,
    pub revision: i64,
    pub width: u32,
    pub height: u32,
}
impl Metadata {
    pub fn read_path(&self) -> String {
        format!(
            "/api/concert/events/{}/media/{}/content",
            self.event, self.media_id
        )
    }
}
/// Only sanitized data may cross the adapter. Intentionally no Debug/Serialize.
pub struct PrivateImage {
    pub metadata: Metadata,
    pub bytes: Vec<u8>,
}
impl PrivateImage {
    pub fn headers(&self) -> [(&'static str, &'static str); 5] {
        [
            ("content-type", self.metadata.mime.as_str()),
            ("cache-control", "private, no-store"),
            ("x-content-type-options", "nosniff"),
            ("content-disposition", "inline"),
            ("cross-origin-resource-policy", "same-origin"),
        ]
    }
}
#[derive(Clone)]
pub struct PgMedia {
    pool: PgPool,
    authority: Arc<dyn MediaAuthority>,
    sanitizer: Arc<dyn ImageSanitizer>,
    keys: Arc<FingerprintKeys>,
}
enum Action {
    Initialize(InitUpload),
    Put { mime: Mime, bytes: Vec<u8> },
    Finalize { expected: i64 },
    Recover(MediaOperation),
}
impl Action {
    fn operation(&self) -> &'static str {
        match self {
            Self::Initialize(_) => "initConcertUpload",
            Self::Recover(v) => v.id(),
            Self::Put { .. } => "putConcertUploadBytes",
            Self::Finalize { .. } => "finalizeConcertUpload",
        }
    }
    fn access(&self, purpose: Purpose) -> Access {
        match self {
            Self::Initialize(v) => Access::Initialize(v.purpose),
            _ => Access::Write(purpose),
        }
    }
}
struct Producer<'a> {
    context: Context,
    action: Action,
    authority: &'a dyn MediaAuthority,
    sanitizer: &'a dyn ImageSanitizer,
}
struct Stored {
    owner: Uuid,
    purpose: Purpose,
    mime: Mime,
    state: String,
    revision: i64,
    policy: PolicyFence,
    expired: bool,
    orphaned: bool,
    target: Option<Target>,
}
async fn stored(
    c: &mut PgConnection,
    event: Uuid,
    id: Uuid,
) -> Result<Option<Stored>, CommandError> {
    let row=sqlx::query("select owner_id,purpose,content_type,state,revision,policy_event_revision,policy_relationship_revision,policy_authority_revision,expires_at<=clock_timestamp() expired,orphan_at<=clock_timestamp() orphaned,target_id from public.concert_media where event_id=$1 and media_id=$2 for update")
 .bind(event).bind(id).fetch_optional(c).await.map_err(unavailable)?;
    row.map(|r| {
        let purpose = Purpose::parse(&r.try_get::<String, _>("purpose").map_err(invalid)?)?;
        Ok(Stored {
            owner: r.try_get("owner_id").map_err(invalid)?,
            purpose,
            mime: Mime::parse(&r.try_get::<String, _>("content_type").map_err(invalid)?)?,
            state: r.try_get("state").map_err(invalid)?,
            revision: r.try_get("revision").map_err(invalid)?,
            policy: PolicyFence {
                event_revision: r.try_get("policy_event_revision").map_err(invalid)?,
                relationship_revision: r
                    .try_get("policy_relationship_revision")
                    .map_err(invalid)?,
                authority_revision: r.try_get("policy_authority_revision").map_err(invalid)?,
            },
            expired: r.try_get("expired").map_err(invalid)?,
            orphaned: r.try_get("orphaned").map_err(invalid)?,
            target: r
                .try_get::<Option<Uuid>, _>("target_id")
                .map_err(invalid)?
                .map(|resource| Target { resource, purpose }),
        })
    })
    .transpose()
}
fn live(v: &Stored) -> Result<(), CommandError> {
    if v.state == "DELETED" {
        return Err(CommandError::Denied);
    }
    if (v.state != "READY" && v.expired) || (v.target.is_none() && v.orphaned) {
        return Err(CommandError::Expired);
    }
    Ok(())
}
#[async_trait]
impl CommandProducer for Producer<'_> {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        s: Scope,
        id: Uuid,
    ) -> Result<(), CommandError> {
        self.context.validate()?;
        if s.actor != self.context.actor
            || s.event != self.context.event
            || s.operation.id() != self.action.operation()
        {
            return Err(CommandError::Denied);
        }
        // Bound lock waits and SQL execution for this transaction; these are not a new request budget.
        limit_sql_in(c).await?;
        let phase: Option<String> = sqlx::query_scalar(
            "select phase from public.concert_events where event_id=$1 for share",
        )
        .bind(s.event)
        .fetch_optional(&mut *c)
        .await
        .map_err(unavailable)?;
        if !matches!(phase.as_deref(), Some("OPEN" | "LIVE"))
            && !(matches!(self.action, Action::Recover(_)) && phase.as_deref() == Some("READ_ONLY"))
        {
            return Err(CommandError::Denied);
        }
        let value = stored(c, s.event, id).await?;
        let (owner, target) = if let Some(v) = &value {
            if v.owner != s.actor
                || (!matches!(self.action, Action::Recover(_)) && v.policy != self.context.policy)
            {
                return Err(CommandError::Denied);
            }
            live(v)?;
            if let Action::Put { mime, .. } = &self.action {
                if *mime != v.mime {
                    return Err(CommandError::InvalidInput);
                }
            };
            (v.owner, v.target)
        } else if matches!(self.action, Action::Initialize(_)) {
            (s.actor, None)
        } else {
            return Err(CommandError::Denied);
        };
        let access = if matches!(
            self.action,
            Action::Initialize(_) | Action::Recover(MediaOperation::Initialize)
        ) {
            Access::Initialize(value.as_ref().map(|v| v.purpose).unwrap_or_else(|| {
                match &self.action {
                    Action::Initialize(v) => v.purpose,
                    _ => unreachable!(),
                }
            }))
        } else {
            self.action
                .access(value.as_ref().ok_or(CommandError::Denied)?.purpose)
        };
        self.authority
            .authorize_in(c, self.context, access, owner, target)
            .await
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        s: Scope,
        id: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        let revision = match &self.action {
            Action::Recover(_) => return Err(CommandError::InvalidState),
            Action::Initialize(input) => {
                input.validate()?;
                // Global BE15 admission lock is held. Cap retained bytes as well as receipt count.
                let count: i64 = sqlx::query_scalar(
                    "select count(*) from public.concert_media where state<>'DELETED'",
                )
                .fetch_one(&mut *c)
                .await
                .map_err(unavailable)?;
                let own:i64=sqlx::query_scalar("select count(*) from public.concert_media where event_id=$1 and owner_id=$2 and target_id is null and state<>'DELETED'").bind(s.event).bind(s.actor).fetch_one(&mut *c).await.map_err(unavailable)?;
                if count >= 10000 || (count + 1) * MAX_BYTES as i64 > 512 * 1024 * 1024 || own >= 16
                {
                    return Err(CommandError::Capacity);
                }
                sqlx::query("insert into public.concert_media(media_id,event_id,owner_id,purpose,content_type,declared_bytes,input_sha256,policy_event_revision,policy_relationship_revision,policy_authority_revision,state) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,'PENDING')")
     .bind(id).bind(s.event).bind(s.actor).bind(input.purpose.as_str()).bind(input.mime.as_str()).bind(input.byte_length as i32).bind(input.sha256.as_slice()).bind(self.context.policy.event_revision).bind(self.context.policy.relationship_revision).bind(self.context.policy.authority_revision).execute(&mut *c).await.map_err(unavailable)?;
                1
            }
            Action::Put { mime, bytes } => {
                let v = stored(c, s.event, id).await?.ok_or(CommandError::Denied)?;
                live(&v)?;
                if v.state != "PENDING" {
                    return Err(CommandError::InvalidState);
                }
                let (length,digest):(i32,Vec<u8>)=sqlx::query_as("select declared_bytes,input_sha256 from public.concert_media where media_id=$1").bind(id).fetch_one(&mut *c).await.map_err(unavailable)?;
                if *mime != v.mime
                    || bytes.is_empty()
                    || bytes.len() > MAX_BYTES
                    || bytes.len() != length as usize
                    || Sha256::digest(bytes).as_slice() != digest
                {
                    return Err(CommandError::InvalidInput);
                }
                let changed=sqlx::query("update public.concert_media set staged=$2,state='UPLOADED',revision=revision+1 where media_id=$1 and expires_at>clock_timestamp() and orphan_at>clock_timestamp()").bind(id).bind(bytes).execute(&mut *c).await.map_err(unavailable)?;
                if changed.rows_affected() != 1 {
                    return Err(CommandError::Expired);
                }
                v.revision + 1
            }
            Action::Finalize { expected } => {
                let v = stored(c, s.event, id).await?.ok_or(CommandError::Denied)?;
                live(&v)?;
                if v.state != "UPLOADED" {
                    return Err(CommandError::InvalidState);
                }
                if v.revision != *expected {
                    return Err(CommandError::Conflict);
                }
                let bytes: Vec<u8> =
                    sqlx::query_scalar("select staged from public.concert_media where media_id=$1")
                        .bind(id)
                        .fetch_one(&mut *c)
                        .await
                        .map_err(unavailable)?;
                let clean = self.sanitizer.sanitize(bytes, v.mime).await?;
                if clean.mime != v.mime
                    || clean.bytes.is_empty()
                    || clean.bytes.len() > MAX_BYTES
                    || clean.width == 0
                    || clean.height == 0
                    || clean.width > MAX_DIMENSION
                    || clean.height > MAX_DIMENSION
                    || u64::from(clean.width) * u64::from(clean.height) > MAX_PIXELS
                {
                    return Err(CommandError::InvalidState);
                }
                // CPU completion is not permission: recheck expiry and live authority before publication.
                live(&stored(c, s.event, id).await?.ok_or(CommandError::Denied)?)?;
                self.authority
                    .authorize_in(c, self.context, Access::Write(v.purpose), v.owner, v.target)
                    .await?;
                let changed=sqlx::query("update public.concert_media set staged=null,sanitized=$2,width=$3,height=$4,state='READY',revision=revision+1 where media_id=$1 and expires_at>clock_timestamp() and orphan_at>clock_timestamp()")
     .bind(id).bind(clean.bytes).bind(clean.width as i32).bind(clean.height as i32).execute(&mut *c).await.map_err(unavailable)?;
                if changed.rows_affected() != 1 {
                    return Err(CommandError::Expired);
                }
                v.revision + 1
            }
        };
        let reference = SafeReference {
            resource: id,
            revision,
        };
        Ok((
            Completion::Succeeded {
                reference,
                status: 200,
            },
            Some(Invalidation { reference }),
        ))
    }
}
impl PgMedia {
    pub fn new(
        pool: PgPool,
        authority: Arc<dyn MediaAuthority>,
        sanitizer: Arc<dyn ImageSanitizer>,
        keys: Arc<FingerprintKeys>,
    ) -> Self {
        Self {
            pool,
            authority,
            sanitizer,
            keys,
        }
    }
    async fn execute(
        &self,
        context: Context,
        key: Uuid,
        id: Uuid,
        action: Action,
    ) -> Result<SafeReceipt, CommandError> {
        context.validate()?;
        if key.is_nil() || id.is_nil() {
            return Err(CommandError::InvalidInput);
        }
        let scope = Scope {
            actor: context.actor,
            event: context.event,
            key,
            operation: RegisteredOperation::find(action.operation())?,
        };
        let event = context.event.to_string();
        let media = id.to_string();
        let mut params = vec![("eventId", event.as_str())];
        if !matches!(action, Action::Initialize(_)) {
            params.push(("mediaId", media.as_str()))
        }
        let request = match &action {
            Action::Recover(_) => return Err(CommandError::InvalidInput),
            Action::Initialize(v) => {
                v.validate()?;
                CanonicalRequest::json(
                    scope,
                    &params,
                    &[],
                    Some(&CanonicalValue::Object(vec![
                        (
                            "purpose".into(),
                            CanonicalValue::String(v.purpose.as_str().into()),
                        ),
                        (
                            "content_type".into(),
                            CanonicalValue::String(v.mime.as_str().into()),
                        ),
                        (
                            "byte_length".into(),
                            CanonicalValue::Integer(v.byte_length as i64),
                        ),
                        ("sha256".into(), CanonicalValue::String(hex(&v.sha256))),
                    ])),
                )?
            }
            Action::Put { bytes, .. } => CanonicalRequest::binary(scope, &params, &[], bytes)?,
            Action::Finalize { expected } => {
                if !(1..=9007199254740991).contains(expected) {
                    return Err(CommandError::InvalidInput);
                }
                CanonicalRequest::json(
                    scope,
                    &params,
                    &[],
                    Some(&CanonicalValue::Object(vec![(
                        "expected_revision".into(),
                        CanonicalValue::Integer(*expected),
                    )])),
                )?
            }
        };
        PgCommandRepository::new(self.pool.clone())
            .execute(
                &request,
                id,
                &self.keys,
                &Producer {
                    context,
                    action,
                    authority: self.authority.as_ref(),
                    sanitizer: self.sanitizer.as_ref(),
                },
            )
            .await
    }
    /// Stable scoped ID recovers an initialize outcome without inventing a new command.
    pub fn initialized_id(context: Context, key: Uuid) -> Uuid {
        let mut hash = Sha256::new();
        hash.update(b"MCR-BE24-init-v1");
        hash.update(context.actor.as_bytes());
        hash.update(context.event.as_bytes());
        hash.update(key.as_bytes());
        let hash = hash.finalize();
        let mut bytes = [0; 16];
        bytes.copy_from_slice(&hash[..16]);
        bytes[6] = (bytes[6] & 15) | 0x80;
        bytes[8] = (bytes[8] & 63) | 0x80;
        Uuid::from_bytes(bytes)
    }
    pub async fn initialize(
        &self,
        context: Context,
        key: Uuid,
        input: InitUpload,
    ) -> Result<SafeReceipt, CommandError> {
        self.execute(
            context,
            key,
            Self::initialized_id(context, key),
            Action::Initialize(input),
        )
        .await
    }
    pub async fn put(
        &self,
        context: Context,
        key: Uuid,
        id: Uuid,
        mime: Mime,
        bytes: Vec<u8>,
    ) -> Result<SafeReceipt, CommandError> {
        self.execute(context, key, id, Action::Put { mime, bytes })
            .await
    }
    pub async fn finalize(
        &self,
        context: Context,
        key: Uuid,
        id: Uuid,
        expected: i64,
    ) -> Result<SafeReceipt, CommandError> {
        self.execute(context, key, id, Action::Finalize { expected })
            .await
    }
    /// Safe receipt status only; never reexecutes a mutation or treats missing as rollback.
    pub async fn recover(
        &self,
        context: Context,
        key: Uuid,
        id: Uuid,
        operation: MediaOperation,
    ) -> Result<Option<SafeReceipt>, CommandError> {
        context.validate()?;
        let scope = Scope {
            actor: context.actor,
            event: context.event,
            key,
            operation: RegisteredOperation::find(operation.id())?,
        };
        PgCommandRepository::new(self.pool.clone())
            .lookup(
                scope,
                id,
                &Producer {
                    context,
                    action: Action::Recover(operation),
                    authority: self.authority.as_ref(),
                    sanitizer: self.sanitizer.as_ref(),
                },
            )
            .await
    }
    /// Must be called by a bounded streaming HTTP adapter; neither this API nor a Content-Length
    /// check grants permission to buffer an unlimited body before dispatch.
    pub async fn read(&self, context: Context, id: Uuid) -> Result<PrivateImage, CommandError> {
        context.validate()?;
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        limit_sql_in(&mut tx).await?;
        let v = stored(&mut tx, context.event, id)
            .await?
            .ok_or(CommandError::Denied)?;
        live(&v)?;
        if v.state != "READY" || (v.owner != context.actor && v.target.is_none()) {
            return Err(CommandError::Denied);
        }
        self.authority
            .authorize_in(&mut tx, context, Access::Read(v.purpose), v.owner, v.target)
            .await?;
        let row = sqlx::query(
            "select sanitized,width,height from public.concert_media where media_id=$1 and (target_id is not null or orphan_at>clock_timestamp())",
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(unavailable)?.ok_or(CommandError::Expired)?;
        let bytes: Vec<u8> = row.try_get("sanitized").map_err(invalid)?;
        let metadata = Metadata {
            media_id: id,
            event: context.event,
            purpose: v.purpose,
            mime: v.mime,
            revision: v.revision,
            width: row.try_get::<i32, _>("width").map_err(invalid)? as u32,
            height: row.try_get::<i32, _>("height").map_err(invalid)? as u32,
        };
        tx.commit().await.map_err(unavailable)?;
        Ok(PrivateImage { metadata, bytes })
    }
    /// Metadata uses the same current authority and expiry checks as content, without
    /// copying image bytes into a JSON/RPC metadata response.
    pub async fn metadata(&self, context: Context, id: Uuid) -> Result<Metadata, CommandError> {
        context.validate()?;
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        limit_sql_in(&mut tx).await?;
        let v = stored(&mut tx, context.event, id)
            .await?
            .ok_or(CommandError::Denied)?;
        live(&v)?;
        if v.state != "READY" || (v.owner != context.actor && v.target.is_none()) {
            return Err(CommandError::Denied);
        }
        self.authority
            .authorize_in(&mut tx, context, Access::Read(v.purpose), v.owner, v.target)
            .await?;
        let row = sqlx::query("select width,height from public.concert_media where media_id=$1 and (target_id is not null or orphan_at>clock_timestamp())")
            .bind(id).fetch_optional(&mut *tx).await.map_err(unavailable)?.ok_or(CommandError::Expired)?;
        let result = Metadata {
            media_id: id,
            event: context.event,
            purpose: v.purpose,
            mime: v.mime,
            revision: v.revision,
            width: row.try_get::<i32, _>("width").map_err(invalid)? as u32,
            height: row.try_get::<i32, _>("height").map_err(invalid)? as u32,
        };
        tx.commit().await.map_err(unavailable)?;
        Ok(result)
    }

    /// Internal canonical command-status resolver. The resource is obtained from the
    /// actor/event/operation/key receipt scope, never a new public query parameter.
    /// Current target authorization is repeated by recover before any receipt escapes.
    pub async fn recover_owned(
        &self,
        context: Context,
        key: Uuid,
        operation: MediaOperation,
    ) -> Result<Option<SafeReceipt>, CommandError> {
        context.validate()?;
        if key.is_nil() {
            return Err(CommandError::InvalidInput);
        }
        let id: Option<Uuid> = sqlx::query_scalar("select target_id from public.concert_command_receipts where actor_id=$1 and event_id=$2 and operation_id=$3 and idempotency_key=$4")
            .bind(context.actor).bind(context.event).bind(operation.id()).bind(key)
            .fetch_optional(&self.pool).await.map_err(unavailable)?;
        match id {
            Some(id) => self.recover(context, key, id, operation).await,
            None => Ok(None),
        }
    }

    /// Atomic producer port for post/giveaway/report ownership, purpose and current policy.
    /// Caller retains this same Concert connection/transaction through its local commit.
    /// This does not fence a separate Post database publication or read.
    pub async fn link_owned_in(
        &self,
        c: &mut PgConnection,
        context: Context,
        id: Uuid,
        target: Target,
    ) -> Result<SafeReference, CommandError> {
        context.validate()?;
        limit_sql_in(c).await?;
        if target.resource.is_nil() {
            return Err(CommandError::InvalidInput);
        }
        let v = stored(c, context.event, id)
            .await?
            .ok_or(CommandError::Denied)?;
        live(&v)?;
        if v.owner != context.actor
            || v.purpose != target.purpose
            || v.state != "READY"
            || v.policy != context.policy
            || v.target.is_some_and(|t| t != target)
        {
            return Err(CommandError::Denied);
        }
        self.authority
            .authorize_in(
                c,
                context,
                Access::Link(target.purpose),
                v.owner,
                Some(target),
            )
            .await?;
        if v.target.is_none() {
            let changed=sqlx::query("update public.concert_media set target_id=$2,revision=revision+1 where media_id=$1 and orphan_at>clock_timestamp()").bind(id).bind(target.resource).execute(c).await.map_err(unavailable)?;
            if changed.rows_affected() != 1 {
                return Err(CommandError::Expired);
            }
        }
        Ok(SafeReference {
            resource: id,
            revision: v.revision + if v.target.is_none() { 1 } else { 0 },
        })
    }
    /// Privileged account-deletion transaction port. Not a browser delete endpoint.
    /// Caller owns global deletion authority and its receipt/outbox; physically clears both
    /// byte columns, leaves a non-readable tombstone so old receipts never resurrect media.
    pub async fn erase_owner_in(
        c: &mut PgConnection,
        event: Uuid,
        owner: Uuid,
    ) -> Result<u64, CommandError> {
        sqlx::query("update public.concert_media set state='DELETED',staged=null,sanitized=null,width=null,height=null,target_id=null,revision=revision+1 where event_id=$1 and owner_id=$2 and state<>'DELETED'").bind(event).bind(owner).execute(c).await.map(|r|r.rows_affected()).map_err(unavailable)
    }
    /// Bounded maintenance. Schedule explicitly; no detached background task or public route.
    pub async fn cleanup(&self, limit: i64) -> Result<u64, CommandError> {
        if !(1..=100).contains(&limit) {
            return Err(CommandError::InvalidInput);
        }
        sqlx::query("with expired as (select media_id from public.concert_media where state<>'DELETED' and target_id is null and ((state in ('PENDING','UPLOADED') and expires_at<=clock_timestamp()) or orphan_at<=clock_timestamp()) order by orphan_at,media_id for update skip locked limit $1) update public.concert_media m set state='DELETED',staged=null,sanitized=null,width=null,height=null,revision=revision+1 from expired e where m.media_id=e.media_id").bind(limit).execute(&self.pool).await.map(|r|r.rows_affected()).map_err(unavailable)
    }
    pub async fn intent(
        &self,
        context: Context,
        id: Uuid,
    ) -> Result<(i64, DateTime<Utc>), CommandError> {
        context.validate()?;
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        limit_sql_in(&mut tx).await?;
        let v = stored(&mut tx, context.event, id)
            .await?
            .ok_or(CommandError::Denied)?;
        live(&v)?;
        if v.owner != context.actor || v.state != "PENDING" || v.policy != context.policy {
            return Err(CommandError::Denied);
        }
        self.authority
            .authorize_in(
                &mut tx,
                context,
                Access::Initialize(v.purpose),
                v.owner,
                None,
            )
            .await?;
        let expires =
            sqlx::query_scalar("select expires_at from public.concert_media where media_id=$1 and expires_at>clock_timestamp() and orphan_at>clock_timestamp()")
                .bind(id)
                .fetch_optional(&mut *tx)
                .await
                .map_err(unavailable)?.ok_or(CommandError::Expired)?;
        tx.commit().await.map_err(unavailable)?;
        Ok((v.revision, expires))
    }
}
fn hex(v: &[u8]) -> String {
    v.iter().map(|v| format!("{v:02x}")).collect()
}
fn unavailable(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
fn invalid(_: sqlx::Error) -> CommandError {
    CommandError::InvalidState
}

async fn limit_sql_in(c: &mut PgConnection) -> Result<(), CommandError> {
    // Clamp existing owner limits; never lengthen a caller's shorter SQL deadline.
    sqlx::query("select set_config('lock_timeout',(extract(epoch from least(coalesce(nullif(current_setting('lock_timeout')::interval,interval '0'),interval '1500 milliseconds'),interval '1500 milliseconds'))*1000)::bigint::text||'ms',true),set_config('statement_timeout',(extract(epoch from least(coalesce(nullif(current_setting('statement_timeout')::interval,interval '0'),interval '5 seconds'),interval '5 seconds'))*1000)::bigint::text||'ms',true)")
 .execute(c).await.map_err(unavailable)?;
    Ok(())
}
