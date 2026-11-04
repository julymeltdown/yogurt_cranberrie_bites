//! Gift-only atomic link authority. Uses uploader policy semantics, never pair-stamp substitution.
use super::{
    authority::TrackedCanonical,
    protocol::{GiftIdentity, GiftInput, GiftRequest},
    resources::{current_settings_in, GiftMediaLink},
};
use crate::{
    adapters::out::{
        media::{MediaAuthority, PgMedia},
        relationship_authority::guarded::OwnerGuardScope,
    },
    media_runtime::{capture_owner_context_in, OwnerMediaContextMode, OwnerMediaIdentity},
    relationship::PgRelationshipPolicy,
};
use async_trait::async_trait;
use concert_application::{
    commands::{CommandError, FingerprintKeys},
    community::giveaway::GiftCommand,
    media::{Access, Context, ImageSanitizer, Purpose, Target},
};
use concert_domain::{
    community::giveaway::{GiftAction, Giveaway},
    relationship::{Field, PolicyQuery},
    AuthUserId, EventId,
};
use sqlx::{PgConnection, PgPool};
use std::sync::Arc;
use uuid::Uuid;

struct Intent {
    identity: GiftIdentity,
    command: Option<GiftCommand>,
    scope: Arc<OwnerGuardScope>,
    canonical: TrackedCanonical,
    pool: PgPool,
}
impl Intent {
    fn command(&self) -> Result<&GiftCommand, CommandError> {
        self.command.as_ref().ok_or(CommandError::Denied)
    }
    async fn context_in(&self, c: &mut PgConnection) -> Result<Context, CommandError> {
        self.canonical.register_in(c).await?;
        let i = self.identity;
        let security = self
            .scope
            .authority_revision()
            .map_err(|_| CommandError::Unavailable)?;
        capture_owner_context_in(
            c,
            OwnerMediaIdentity {
                actor: i.actor,
                event: i.event,
                session_family: i.session_family,
                request_id: i.request_id,
                deadline: i.deadline,
            },
            security,
            OwnerMediaContextMode::Mutation,
        )
        .await
    }
    async fn authorize_target_in(&self, c: &mut PgConnection) -> Result<(), CommandError> {
        let cmd = self.command()?;
        let enabled: Option<bool> = sqlx::query_scalar(
            "select giveaways from concert_event_settings where event_id=$1 for share",
        )
        .bind(cmd.event)
        .fetch_optional(&mut *c)
        .await
        .map_err(db)?;
        if enabled != Some(true) {
            return Err(CommandError::Denied);
        }
        PgRelationshipPolicy::new(self.pool.clone(), self.canonical.clone())
            .authorize_in(
                c,
                PolicyQuery {
                    actor: AuthUserId::from_uuid(cmd.actor),
                    event: EventId::from_uuid(cmd.event),
                    subject: AuthUserId::from_uuid(cmd.actor),
                    field: Field::Interaction,
                },
                None,
            )
            .await
            .map_err(|e| match e {
                concert_domain::relationship::RelationshipError::Denied => CommandError::Denied,
                _ => CommandError::Unavailable,
            })?;
        let settings = current_settings_in(c, cmd.event).await?;
        let now = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(db)?;
        let stored:Option<serde_json::Value>=sqlx::query_scalar("select aggregate from concert_community_giveaways where event_id=$1 and giveaway_id=$2 for update").bind(cmd.event).bind(cmd.giveaway_id()).fetch_optional(&mut *c).await.map_err(db)?;
        let gift = match &cmd.action {
            GiftAction::Create(fields) => {
                if stored.is_some() {
                    return Err(CommandError::Denied);
                }
                // Qualified same-transaction create intent: exact server-derived
                // target + validated authenticated command, before retained save.
                Giveaway::create(
                    cmd.giveaway_id(),
                    cmd.event,
                    cmd.actor,
                    fields.clone(),
                    settings.pickup_grace_seconds,
                    now,
                )
                .map_err(|_| CommandError::InvalidInput)?
            }
            GiftAction::Update { .. } => {
                let mut g: Giveaway = serde_json::from_value(stored.ok_or(CommandError::Denied)?)
                    .map_err(|_| CommandError::Unavailable)?;
                if (g.id, g.event, g.owner) != (cmd.giveaway_id(), cmd.event, cmd.actor) {
                    return Err(CommandError::Denied);
                }
                g.change(cmd.actor, &cmd.action, cmd.created_id(), now)
                    .map_err(|_| CommandError::Denied)?;
                g
            }
            _ => return Err(CommandError::Denied),
        };
        gift.validate().map_err(|_| CommandError::Unavailable)?;
        if gift.expires_at() > settings.event_cutoff {
            return Err(CommandError::Denied);
        }
        Ok(())
    }
}
fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
struct LinkAuthority(Arc<Intent>);
#[async_trait]
impl MediaAuthority for LinkAuthority {
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        context: Context,
        access: Access,
        owner: Uuid,
        target: Option<Target>,
    ) -> Result<(), CommandError> {
        let cmd = self.0.command()?;
        if access != Access::Link(Purpose::Giveaway)
            || owner != cmd.actor
            || target
                != Some(Target {
                    resource: cmd.giveaway_id(),
                    purpose: Purpose::Giveaway,
                })
        {
            return Err(CommandError::Denied);
        }
        let actual = self.0.context_in(c).await?;
        if context != actual {
            return Err(CommandError::Denied);
        }
        self.0.authorize_target_in(c).await
    }
}
/// Created from the exact parsed request and actual root scope. No supplied target
/// UUID, permissive authority or uploader-private Link exception is accepted.
pub struct GiftMediaAdapter {
    media: PgMedia,
    intent: Arc<Intent>,
}
impl GiftMediaAdapter {
    pub fn new(
        pool: PgPool,
        request: &GiftRequest,
        scope: Arc<OwnerGuardScope>,
        keys: Arc<FingerprintKeys>,
        sanitizer: Arc<dyn ImageSanitizer>,
    ) -> Result<Self, CommandError> {
        let canonical =
            TrackedCanonical::new(request, scope.clone()).map_err(|_| CommandError::Denied)?;
        let i = request.identity();
        let command = match request.input() {
            GiftInput::Write {
                giveaway, action, ..
            } if matches!(action, GiftAction::Create(_) | GiftAction::Update { .. }) => {
                Some(GiftCommand {
                    actor: i.actor,
                    event: i.event,
                    key: request
                        .idempotency_key()
                        .ok_or(CommandError::InvalidInput)?,
                    giveaway: giveaway.unwrap_or(Uuid::nil()),
                    action: action.clone(),
                    code: None,
                })
            }
            _ => None,
        };
        let intent = Arc::new(Intent {
            identity: i,
            command,
            scope,
            canonical,
            pool: pool.clone(),
        });
        let media = PgMedia::new(
            pool,
            Arc::new(LinkAuthority(intent.clone())),
            sanitizer,
            keys,
        );
        Ok(Self { media, intent })
    }
}
#[async_trait]
impl GiftMediaLink for GiftMediaAdapter {
    async fn link_in(
        &self,
        c: &mut PgConnection,
        identity: GiftIdentity,
        giveaway: Uuid,
        media: Uuid,
    ) -> Result<(), CommandError> {
        let i = self.intent.identity;
        let cmd = self.intent.command()?;
        if (
            identity.actor,
            identity.event,
            identity.session_family,
            identity.request_id,
            identity.deadline.expires_micros(),
            identity.deadline.stop(),
        ) != (
            i.actor,
            i.event,
            i.session_family,
            i.request_id,
            i.deadline.expires_micros(),
            i.deadline.stop(),
        ) || giveaway != cmd.giveaway_id()
        {
            return Err(CommandError::Denied);
        }
        let selected = match &cmd.action {
            GiftAction::Create(f) | GiftAction::Update { fields: f, .. } => f.image_media_id,
            _ => None,
        };
        if selected != Some(media) || media.is_nil() {
            return Err(CommandError::Denied);
        }
        let context = self.intent.context_in(c).await?;
        self.media
            .link_owned_in(
                c,
                context,
                media,
                Target {
                    resource: giveaway,
                    purpose: Purpose::Giveaway,
                },
            )
            .await?;
        Ok(())
    }
}
