//! Per-request BE24 target adapter. Prelocks are distinct from final permission.
use super::producer::{binding_in, PhotoFrame, PhotoMutation, PhotoTransactionGuard};
use crate::{
    adapters::out::{media::MediaAuthority, repository::receipt::CommandProducer},
    meeting::{
        beacon,
        commands::{command_error, BeaconProducer},
        MeetingAuthority,
    },
};
use async_trait::async_trait;
use concert_application::{commands::*, media::*};
use sqlx::PgConnection;
use std::sync::Arc;
use uuid::Uuid;
#[derive(Clone)]
pub(crate) enum PhotoMode {
    Mutation(PhotoMutation),
    Read { media: Uuid },
}
pub(crate) struct PhotoMediaAuthority<A, G> {
    pub authority: Arc<A>,
    pub guard: Arc<G>,
    pub frame: PhotoFrame,
    pub mode: PhotoMode,
}
impl<A: MeetingAuthority, G: PhotoTransactionGuard> PhotoMediaAuthority<A, G> {
    async fn context(
        &self,
        c: &mut PgConnection,
        ctx: Context,
        target: Option<Target>,
    ) -> Result<Target, CommandError> {
        if self.guard.register_in(c, &self.frame).await? != ctx {
            return Err(CommandError::Denied);
        }
        let target = target.ok_or(CommandError::Denied)?;
        if target.purpose != Purpose::Beacon || target.resource != self.frame.beacon {
            return Err(CommandError::Denied);
        }
        Ok(target)
    }
}
#[async_trait]
impl<A: MeetingAuthority, G: PhotoTransactionGuard> MediaAuthority for PhotoMediaAuthority<A, G> {
    async fn prelock_target_in(
        &self,
        c: &mut PgConnection,
        ctx: Context,
        owner: Uuid,
        target: Option<Target>,
    ) -> Result<(), CommandError> {
        let target = self.context(c, ctx, target).await?;
        match &self.mode {
            PhotoMode::Mutation(m) => {
                if owner != self.frame.actor.actor {
                    return Err(CommandError::Denied);
                }
                let text = m.text();
                let s = Scope {
                    actor: ctx.actor,
                    event: ctx.event,
                    key: self.frame.key.ok_or(CommandError::Denied)?,
                    operation: RegisteredOperation::find(self.frame.operation)?,
                };
                BeaconProducer {
                    authority: &*self.authority,
                    actor: self.frame.actor,
                    id: target.resource,
                    mutation: &text,
                }
                .authorize(c, s, target.resource)
                .await
            }
            PhotoMode::Read { .. } => {
                let b = beacon::read_in(c, &*self.authority, self.frame.actor, target.resource)
                    .await
                    .map_err(command_error)?;
                if b.owner_id != owner {
                    return Err(CommandError::Denied);
                }
                Ok(())
            }
        }
    }
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        ctx: Context,
        access: Access,
        owner: Uuid,
        target: Option<Target>,
    ) -> Result<(), CommandError> {
        let target = self.context(c, ctx, target).await?;
        match (&self.mode, access) {
            (PhotoMode::Read { media }, Access::Read(Purpose::Beacon)) => {
                let b = beacon::read_in(c, &*self.authority, self.frame.actor, target.resource)
                    .await
                    .map_err(command_error)?;
                let link = binding_in(c, ctx.event, target.resource, false)
                    .await?
                    .ok_or(CommandError::Denied)?;
                if owner != b.owner_id
                    || link.media != *media
                    || link.revision != b.revision
                    || link.expires != b.meeting.expires_at
                {
                    return Err(CommandError::Denied);
                }
                Ok(())
            }
            (PhotoMode::Mutation(m), Access::Link(Purpose::Beacon)) if m.media_id().is_some() => {
                if owner != ctx.actor {
                    return Err(CommandError::Denied);
                }
                let b = beacon::read_in(c, &*self.authority, self.frame.actor, target.resource)
                    .await
                    .map_err(command_error)?;
                if b.owner_id != owner {
                    return Err(CommandError::Denied);
                }
                Ok(())
            }
            (PhotoMode::Mutation(_), Access::Delete(Purpose::Beacon)) => {
                // Privacy-reducing removal must not require continued old audience access.
                self.authority
                    .actor_in(c, self.frame.actor)
                    .await
                    .map_err(command_error)?;
                let b = beacon::load(c, ctx.event, target.resource, false)
                    .await
                    .map_err(command_error)?;
                if owner != ctx.actor || b.owner_id != owner {
                    return Err(CommandError::Denied);
                }
                Ok(())
            }
            _ => Err(CommandError::Denied),
        }
    }
}
