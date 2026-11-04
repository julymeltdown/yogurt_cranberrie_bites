use super::{
    authority::{PhotoMediaAuthority, PhotoMode},
    wire::*,
};
use crate::{
    adapters::out::{
        media::{Metadata, PgMedia, PrivateImage},
        repository::receipt::{CommandProducer, PgCommandRepository},
    },
    meeting::{
        beacon,
        commands::{canonical, command_error, BeaconProducer},
        MeetingAuthority,
    },
};
use async_trait::async_trait;
use concert_application::{
    beacon::*,
    commands::*,
    media::{Context, ImageSanitizer, Purpose, Target},
};
use concert_domain::beacon::BeaconState;
use serde_json::Value;
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub enum PhotoMutation {
    Create(CreatePhotoBeacon),
    Update(UpdatePhotoBeacon),
    Revoke { expected_revision: i64 },
}
impl PhotoMutation {
    pub fn text(&self) -> BeaconMutation {
        match self {
            Self::Create(v) => v.text_mutation(),
            Self::Update(v) => v.text_mutation(),
            Self::Revoke { expected_revision } => BeaconMutation::Revoke {
                expected_revision: *expected_revision,
            },
        }
    }
    pub fn media_id(&self) -> Option<Uuid> {
        match self {
            Self::Create(v) => v.meeting.media_id,
            Self::Update(v) => v.meeting.media_id,
            _ => None,
        }
    }
    fn body(&self) -> Result<Option<Value>, CommandError> {
        match self {
            Self::Create(v) => serde_json::to_value(v).map(Some),
            Self::Update(v) => serde_json::to_value(v).map(Some),
            _ => Ok(None),
        }
        .map_err(|_| CommandError::InvalidInput)
    }
    fn validate(&self) -> Result<(), CommandError> {
        match self {
            Self::Create(v) => v.validate(),
            Self::Update(v) => v.validate(),
            Self::Revoke { .. } => {
                if self.text().valid() {
                    Ok(())
                } else {
                    Err(MeetingError::InvalidInput)
                }
            }
        }
        .map_err(command_error)
    }
}
/// Trusted verified transport binding. No Debug or raw guidance in diagnostics.
#[derive(Clone)]
pub struct PhotoFrame {
    pub actor: MeetingActor,
    pub beacon: Uuid,
    pub key: Option<Uuid>,
    pub operation: &'static str,
    pub method: &'static str,
    pub target: String,
    pub body_sha256: [u8; 32],
    intent_sha256: Option<[u8; 32]>,
}
impl PhotoFrame {
    pub fn mutation(
        actor: MeetingActor,
        beacon: Uuid,
        key: Uuid,
        mutation: &PhotoMutation,
        raw: &[u8],
    ) -> Result<Self, CommandError> {
        mutation.validate()?;
        if [actor.actor, actor.event, beacon, key]
            .iter()
            .any(Uuid::is_nil)
        {
            return Err(CommandError::InvalidInput);
        }
        // Match raw bytes to the exact closed typed input before binding authority.
        match mutation {
            PhotoMutation::Create(v) => {
                let p = CreatePhotoBeacon::parse(raw).map_err(command_error)?;
                if serde_json::to_value(p).ok() != serde_json::to_value(v).ok() {
                    return Err(CommandError::InvalidInput);
                }
            }
            PhotoMutation::Update(v) => {
                let p = UpdatePhotoBeacon::parse(raw).map_err(command_error)?;
                if serde_json::to_value(p).ok() != serde_json::to_value(v).ok() {
                    return Err(CommandError::InvalidInput);
                }
            }
            PhotoMutation::Revoke { .. } => {
                if !raw.is_empty() {
                    return Err(CommandError::InvalidInput);
                }
            }
        }
        let operation = mutation.text().operation_id();
        let root = format!("/api/concert/events/{}/beacons", actor.event);
        let (method, target) = match mutation {
            PhotoMutation::Create(_) => ("POST", root),
            PhotoMutation::Update(_) => ("PATCH", format!("{root}/{beacon}")),
            PhotoMutation::Revoke { expected_revision } => (
                "DELETE",
                format!("{root}/{beacon}?expected_revision={expected_revision}"),
            ),
        };
        Ok(Self {
            actor,
            beacon,
            key: Some(key),
            operation,
            method,
            target,
            body_sha256: Sha256::digest(raw).into(),
            intent_sha256: Some(mutation_digest(mutation)?),
        })
    }
    pub fn read(
        actor: MeetingActor,
        beacon: Uuid,
        media: Option<(Uuid, bool)>,
    ) -> Result<Self, CommandError> {
        if [actor.actor, actor.event, beacon].iter().any(Uuid::is_nil)
            || media.is_some_and(|v| v.0.is_nil())
        {
            return Err(CommandError::InvalidInput);
        }
        let (operation, target) = match media {
            Some((id, true)) => (
                "readConcertMedia",
                format!("/api/concert/events/{}/media/{id}/content", actor.event),
            ),
            Some((id, false)) => (
                "getConcertMedia",
                format!("/api/concert/events/{}/media/{id}", actor.event),
            ),
            None => (
                "getConcertBeacon",
                format!("/api/concert/events/{}/beacons/{beacon}", actor.event),
            ),
        };
        Ok(Self {
            actor,
            beacon,
            key: None,
            operation,
            method: "GET",
            target,
            body_sha256: Sha256::digest([]).into(),
            intent_sha256: None,
        })
    }
}
/// Implement using the actual owner-held scope; registration must durably Track
/// this exact connection before returning. A Context/cached boolean is not a grant.
#[async_trait]
pub trait PhotoTransactionGuard: Send + Sync {
    async fn register_in(
        &self,
        c: &mut PgConnection,
        frame: &PhotoFrame,
    ) -> Result<Context, CommandError>;
}
pub struct PhotoBeacons<A, G> {
    pool: PgPool,
    authority: Arc<A>,
    guard: Arc<G>,
    sanitizer: Arc<dyn ImageSanitizer>,
    keys: Arc<FingerprintKeys>,
}
impl<A: MeetingAuthority + 'static, G: PhotoTransactionGuard + 'static> PhotoBeacons<A, G> {
    pub fn new(
        pool: PgPool,
        authority: Arc<A>,
        guard: Arc<G>,
        sanitizer: Arc<dyn ImageSanitizer>,
        keys: Arc<FingerprintKeys>,
    ) -> Self {
        Self {
            pool,
            authority,
            guard,
            sanitizer,
            keys,
        }
    }
    fn media(&self, frame: PhotoFrame, mode: PhotoMode) -> PgMedia {
        PgMedia::new(
            self.pool.clone(),
            Arc::new(PhotoMediaAuthority {
                authority: self.authority.clone(),
                guard: self.guard.clone(),
                frame,
                mode,
            }),
            self.sanitizer.clone(),
            self.keys.clone(),
        )
    }
    pub async fn execute(
        &self,
        frame: PhotoFrame,
        mutation: PhotoMutation,
    ) -> Result<SafeReceipt, CommandError> {
        mutation.validate()?;
        let text = mutation.text();
        if frame.operation != text.operation_id()
            || frame.key.is_none()
            || frame.intent_sha256 != Some(mutation_digest(&mutation)?)
        {
            return Err(CommandError::Denied);
        }
        let scope = Scope {
            actor: frame.actor.actor,
            event: frame.actor.event,
            key: frame.key.unwrap(),
            operation: RegisteredOperation::find(frame.operation)?,
        };
        let event = frame.actor.event.to_string();
        let id = frame.beacon.to_string();
        let mut parameters = vec![("eventId", event.as_str())];
        if !matches!(mutation, PhotoMutation::Create(_)) {
            parameters.push(("beaconId", id.as_str()))
        }
        let revision = match mutation {
            PhotoMutation::Revoke { expected_revision } => Some(expected_revision.to_string()),
            _ => None,
        };
        let query = revision
            .as_ref()
            .map(|v| vec![("expected_revision", v.as_str())])
            .unwrap_or_default();
        let body = mutation.body()?.as_ref().map(canonical).transpose()?;
        let request = CanonicalRequest::json(scope, &parameters, &query, body.as_ref())?;
        let media = self.media(frame.clone(), PhotoMode::Mutation(mutation.clone()));
        PgCommandRepository::new(self.pool.clone())
            .execute(
                &request,
                frame.beacon,
                &self.keys,
                &PhotoProducer {
                    authority: &*self.authority,
                    guard: &*self.guard,
                    frame: &frame,
                    mutation: &mutation,
                    text: &text,
                    media: &media,
                },
            )
            .await
    }
    pub async fn read(&self, frame: PhotoFrame) -> Result<PhotoBeacon, CommandError> {
        if frame.operation != "getConcertBeacon" || frame.key.is_some() {
            return Err(CommandError::Denied);
        }
        let mut tx = self.pool.begin().await.map_err(storage)?;
        self.guard.register_in(&mut tx, &frame).await?;
        let value = beacon::read_in(&mut tx, &*self.authority, frame.actor, frame.beacon)
            .await
            .map_err(command_error)?;
        let binding = binding_in(&mut tx, frame.actor.event, frame.beacon, false).await?;
        let media_id = match binding {
            Some(b) => {
                if b.revision != value.revision || b.expires != value.meeting.expires_at {
                    return Err(CommandError::Denied);
                };
                let media = self.media(frame.clone(), PhotoMode::Read { media: b.media });
                let ctx = self.guard.register_in(&mut tx, &frame).await?;
                media.metadata_in(&mut tx, ctx, b.media).await?;
                Some(b.media)
            }
            None => None,
        };
        beacon::live_time(&mut tx, &value)
            .await
            .map_err(command_error)?;
        crate::meeting::ensure_authority_current(&mut tx)
            .await
            .map_err(command_error)?;
        tx.commit().await.map_err(storage)?;
        Ok(PhotoBeacon::from_authorized(value, media_id))
    }
    /// Internal association lookup only; a hint is NEVER a target/content grant.
    /// Root derives this from stored linkage, not a new public query parameter.
    pub async fn target_hint(&self, event: Uuid, media: Uuid) -> Result<Uuid, CommandError> {
        sqlx::query_scalar(
            "select beacon_id from concert_meeting_beacon_photos where event_id=$1 and media_id=$2",
        )
        .bind(event)
        .bind(media)
        .fetch_optional(&self.pool)
        .await
        .map_err(storage)?
        .ok_or(CommandError::Denied)
    }
    pub async fn metadata(&self, frame: PhotoFrame, id: Uuid) -> Result<Metadata, CommandError> {
        if frame.operation != "getConcertMedia" || frame.key.is_some() {
            return Err(CommandError::Denied);
        }
        let media = self.media(frame.clone(), PhotoMode::Read { media: id });
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let ctx = self.guard.register_in(&mut tx, &frame).await?;
        let value = media.metadata_in(&mut tx, ctx, id).await?;
        self.fresh_after_read(&mut tx, &frame).await?;
        tx.commit().await.map_err(storage)?;
        Ok(value)
    }
    pub async fn content(&self, frame: PhotoFrame, id: Uuid) -> Result<PrivateImage, CommandError> {
        if frame.operation != "readConcertMedia" || frame.key.is_some() {
            return Err(CommandError::Denied);
        }
        let media = self.media(frame.clone(), PhotoMode::Read { media: id });
        let mut tx = self.pool.begin().await.map_err(storage)?;
        let ctx = self.guard.register_in(&mut tx, &frame).await?;
        let value = media.read_in(&mut tx, ctx, id).await?;
        self.fresh_after_read(&mut tx, &frame).await?;
        tx.commit().await.map_err(storage)?;
        Ok(value)
    }
    /// Bounded one-target maintenance port, never a public delete endpoint.
    /// Caller supplies a qualified current owner/job scope and owns this same TX.
    /// Expiry is rechecked after locking; no old audience Read grant is required.
    pub async fn cleanup_expired_in(
        &self,
        c: &mut PgConnection,
        frame: PhotoFrame,
    ) -> Result<u64, CommandError> {
        if frame.operation != "revokeConcertBeacon"
            || frame.method != "DELETE"
            || frame.key.is_none()
        {
            return Err(CommandError::Denied);
        }
        let ctx = self.guard.register_in(c, &frame).await?;
        self.authority
            .actor_in(c, frame.actor)
            .await
            .map_err(command_error)?;
        let b = beacon::load(c, frame.actor.event, frame.beacon, true)
            .await
            .map_err(command_error)?;
        if b.owner_id != frame.actor.actor {
            return Err(CommandError::Denied);
        }
        if frame.target
            != format!(
                "/api/concert/events/{}/beacons/{}?expected_revision={}",
                frame.actor.event, frame.beacon, b.revision
            )
        {
            return Err(CommandError::Conflict);
        }
        let expired:bool=sqlx::query_scalar("select clock_timestamp()>=least($1,event_cutoff) from concert_meeting_settings where event_id=$2 for share")
            .bind(b.meeting.expires_at).bind(b.event_id).fetch_one(&mut *c).await.map_err(storage)?;
        if !expired {
            return Ok(0);
        }
        let old = binding_in(c, b.event_id, b.beacon_id, true).await?;
        if let Some(old) = old {
            let media = self.media(
                frame,
                PhotoMode::Mutation(PhotoMutation::Revoke {
                    expected_revision: b.revision,
                }),
            );
            let target = Target {
                resource: b.beacon_id,
                purpose: Purpose::Beacon,
            };
            media.lock_owned_in(c, ctx, &[old.media], target).await?;
            media.erase_target_in(c, ctx, old.media, target).await?;
            sqlx::query(
                "delete from concert_meeting_beacon_photos where event_id=$1 and beacon_id=$2",
            )
            .bind(b.event_id)
            .bind(b.beacon_id)
            .execute(c)
            .await
            .map_err(storage)?;
            Ok(1)
        } else {
            Ok(0)
        }
    }
    async fn fresh_after_read(
        &self,
        c: &mut PgConnection,
        f: &PhotoFrame,
    ) -> Result<(), CommandError> {
        beacon::read_in(c, &*self.authority, f.actor, f.beacon)
            .await
            .map_err(command_error)?;
        Ok(())
    }
}
pub(crate) struct Binding {
    pub media: Uuid,
    pub revision: i64,
    pub expires: chrono::DateTime<chrono::Utc>,
}
pub(crate) async fn binding_in(
    c: &mut PgConnection,
    event: Uuid,
    id: Uuid,
    write: bool,
) -> Result<Option<Binding>, CommandError> {
    let lock = if write { "for update" } else { "for share" };
    sqlx::query(&format!("select media_id,beacon_revision,expires_at from public.concert_meeting_beacon_photos where event_id=$1 and beacon_id=$2 {lock}")).bind(event).bind(id).fetch_optional(c).await.map_err(storage)?.map(|r|Ok(Binding{media:r.try_get("media_id").map_err(storage)?,revision:r.try_get("beacon_revision").map_err(storage)?,expires:r.try_get("expires_at").map_err(storage)?})).transpose()
}
struct PhotoProducer<'a, A, G> {
    authority: &'a A,
    guard: &'a G,
    frame: &'a PhotoFrame,
    mutation: &'a PhotoMutation,
    text: &'a BeaconMutation,
    media: &'a PgMedia,
}
impl<A: MeetingAuthority, G: PhotoTransactionGuard> PhotoProducer<'_, A, G> {
    fn base(&self) -> BeaconProducer<'_, A> {
        BeaconProducer {
            authority: self.authority,
            actor: self.frame.actor,
            id: self.frame.beacon,
            mutation: self.text,
        }
    }
}
#[async_trait]
impl<A: MeetingAuthority, G: PhotoTransactionGuard> CommandProducer for PhotoProducer<'_, A, G> {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        let ctx = self.guard.register_in(c, self.frame).await?;
        self.base().authorize(c, s, target).await?;
        let old = binding_in(c, s.event, target, true).await?;
        let mut ids = old.iter().map(|b| b.media).collect::<Vec<_>>();
        if let Some(id) = self.mutation.media_id() {
            ids.push(id)
        }
        if !ids.is_empty() {
            self.media
                .lock_owned_in(
                    c,
                    ctx,
                    &ids,
                    Target {
                        resource: target,
                        purpose: Purpose::Beacon,
                    },
                )
                .await?;
        }
        Ok(())
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        let prior = binding_in(c, s.event, target, true).await?;
        let result = self.base().apply(c, s, target).await?;
        if !matches!(&result.0, Completion::Succeeded { .. }) {
            return Ok(result);
        }
        let ctx = self.guard.register_in(c, self.frame).await?;
        let t = Target {
            resource: target,
            purpose: Purpose::Beacon,
        };
        if let Some(id) = self.mutation.media_id() {
            let linked = self.media.link_owned_in(c, ctx, id, t).await?;
            let beacon = beacon::load(c, s.event, target, false)
                .await
                .map_err(command_error)?;
            if beacon.state != BeaconState::Active {
                return Err(CommandError::Denied);
            }
            sqlx::query("insert into public.concert_meeting_beacon_photos(event_id,beacon_id,owner_id,media_id,beacon_revision,media_revision,permission_acknowledged,expires_at) values($1,$2,$3,$4,$5,$6,true,$7) on conflict(beacon_id) do update set media_id=excluded.media_id,beacon_revision=excluded.beacon_revision,media_revision=excluded.media_revision,permission_acknowledged=true,permission_confirmed_at=clock_timestamp(),expires_at=excluded.expires_at")
    .bind(s.event).bind(target).bind(s.actor).bind(id).bind(beacon.revision).bind(linked.revision).bind(beacon.meeting.expires_at).execute(&mut *c).await.map_err(storage)?;
        } else {
            sqlx::query("delete from public.concert_meeting_beacon_photos where event_id=$1 and beacon_id=$2").bind(s.event).bind(target).execute(&mut *c).await.map_err(storage)?;
        }
        if let Some(old) = prior {
            if Some(old.media) != self.mutation.media_id() {
                self.media.erase_target_in(c, ctx, old.media, t).await?;
            }
        }
        Ok(result)
    }
}
pub(crate) fn storage(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}

fn mutation_digest(m: &PhotoMutation) -> Result<[u8; 32], CommandError> {
    let value = match m {
        PhotoMutation::Revoke { expected_revision } => {
            serde_json::json!({"expected_revision":expected_revision})
        }
        _ => m.body()?.ok_or(CommandError::InvalidInput)?,
    };
    Ok(Sha256::digest(serde_json::to_vec(&value).map_err(|_| CommandError::InvalidInput)?).into())
}
impl PhotoFrame {
    pub fn digest(&self) -> [u8; 32] {
        let mut h = Sha256::new();
        h.update(b"MCR-meeting-photo-frame-v1");
        for id in [self.actor.actor, self.actor.event, self.beacon] {
            h.update(id.as_bytes());
        }
        h.update([u8::from(self.key.is_some())]);
        if let Some(key) = self.key {
            h.update(key.as_bytes());
        }
        for part in [self.operation, self.method, self.target.as_str()] {
            h.update((part.len() as u64).to_be_bytes());
            h.update(part.as_bytes());
        }
        h.update(self.body_sha256);
        h.update([u8::from(self.intent_sha256.is_some())]);
        if let Some(d) = self.intent_sha256 {
            h.update(d);
        }
        h.finalize().into()
    }
}
