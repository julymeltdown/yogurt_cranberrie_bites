//! Party-owned withdrawal and the sole actual Party/Gift category composition.
use crate::{
    meeting::account_deletion::{db, finish_owner, open_owner},
    world::account_deletion::{
        DeletionAuthority, DeletionFailure, VerifiedDeletionSource, WorldDeletionCommand,
    },
};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tokio::time::Instant;

#[derive(Clone, PartialEq, Eq)]
pub struct PartyPrivateInputReceipt {
    pub command: WorldDeletionCommand,
    pub source_binding: [u8; 32],
    pub owner_revision: u64,
    pub complete: bool,
}
impl PartyPrivateInputReceipt {
    pub fn verifies(&self, command: &WorldDeletionCommand, source_binding: &[u8; 32]) -> bool {
        self.command == *command
            && self.source_binding == *source_binding
            && ((self.complete && self.owner_revision == 1)
                || (!self.complete && self.owner_revision == 0))
    }
}
pub struct PartyPrivateInputDeletion {
    pool: PgPool,
    authority: Arc<dyn DeletionAuthority>,
}
impl PartyPrivateInputDeletion {
    pub fn new(pool: PgPool, authority: Arc<dyn DeletionAuthority>) -> Self {
        Self { pool, authority }
    }
    pub async fn apply(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<PartyPrivateInputReceipt, DeletionFailure> {
        tokio::time::timeout_at(deadline,async {
            let (mut tx,was_complete)=open_owner(&self.pool,self.authority.as_ref(),command,source,deadline,"PARTY").await?;
            if !was_complete {
                let rows=sqlx::query("select event_id,party_id from public.concert_community_parties p where owner_id=$1 or aggregate->'applications' @> jsonb_build_array(jsonb_build_object('user',$1::uuid)) or aggregate->'members' @> jsonb_build_array(jsonb_build_object('user',$1::uuid)) or exists(select 1 from public.concert_community_applications a where a.event_id=p.event_id and a.party_id=p.party_id and a.user_id=$1) or exists(select 1 from public.concert_community_members m where m.event_id=p.event_id and m.party_id=p.party_id and m.user_id=$1) order by event_id,party_id limit 32")
                    .bind(command.user_id).fetch_all(&mut *tx).await.map_err(db)?;
                for row in rows {
                    let event=row.try_get("event_id").map_err(db)?;let id=row.try_get("party_id").map_err(db)?;
                    let mut party=crate::party_live::wire::load_for_update(&mut tx,event,id).await.map_err(db)?;
                    if party.owner==command.user_id {
                        // Withdraw the entire owned aggregate, children first.
                        // This leaves no active orphan or fabricated canonical
                        // owner. Public event/place and old safe receipts remain.
                        sqlx::query("delete from public.concert_community_applications where event_id=$1 and party_id=$2").bind(event).bind(id).execute(&mut *tx).await.map_err(db)?;
                        sqlx::query("delete from public.concert_community_members where event_id=$1 and party_id=$2").bind(event).bind(id).execute(&mut *tx).await.map_err(db)?;
                        sqlx::query("delete from public.concert_community_parties where event_id=$1 and party_id=$2").bind(event).bind(id).execute(&mut *tx).await.map_err(db)?;
                    } else {
                        party.applications.retain(|a|a.user!=command.user_id);
                        party.members.retain(|m|m.user!=command.user_id);
                        party.revision=concert_domain::community::party::bump(party.revision).map_err(db)?;
                        party.audience_revision=concert_domain::community::party::bump(party.audience_revision).map_err(db)?;
                        party.validate().map_err(db)?;
                        sqlx::query("delete from public.concert_community_applications where event_id=$1 and party_id=$2 and user_id=$3").bind(event).bind(id).bind(command.user_id).execute(&mut *tx).await.map_err(db)?;
                        sqlx::query("delete from public.concert_community_members where event_id=$1 and party_id=$2 and user_id=$3").bind(event).bind(id).bind(command.user_id).execute(&mut *tx).await.map_err(db)?;
                        crate::party_live::save_party(&mut tx,&party).await.map_err(db)?;
                    }
                }
            }
            let remaining:bool=sqlx::query_scalar("select public.concert_party_has_private_input($1)")
                .bind(command.user_id).fetch_one(&mut *tx).await.map_err(db)?;
            let complete=!remaining;
            finish_owner(tx,self.authority.as_ref(),command,source,deadline,"PARTY",was_complete,complete).await?;
            Ok(PartyPrivateInputReceipt {command:command.clone(),source_binding:*source.source_binding(),
                owner_revision:if complete{1}else{0},complete})
        }).await.map_err(|_|DeletionFailure::Unavailable)?
    }
}

/// One category adapter, backed by both actual stores. A Party-only terminal
/// receipt never certifies Gift absence. Both children retain the original
/// command, verified Auth source and attempt deadline.
pub struct PartyAndGiftInputDeletion {
    party: PartyPrivateInputDeletion,
    gift: crate::gift_live::account_deletion::GiftPrivateInputDeletion,
}
impl PartyAndGiftInputDeletion {
    pub fn new(pool: PgPool, authority: Arc<dyn DeletionAuthority>) -> Self {
        Self {
            party: PartyPrivateInputDeletion::new(pool.clone(), authority.clone()),
            gift: crate::gift_live::account_deletion::GiftPrivateInputDeletion::new(
                pool, authority,
            ),
        }
    }
}
#[async_trait::async_trait]
impl crate::account_private_data::PrivateDataCategoryOwner for PartyAndGiftInputDeletion {
    async fn apply_category(
        &self,
        command: &WorldDeletionCommand,
        source: &VerifiedDeletionSource,
        deadline: Instant,
    ) -> Result<crate::account_private_data::PrivateDataCategoryReceipt, DeletionFailure> {
        use crate::account_private_data::{
            PrivateDataCategory, PrivateDataCategoryReceipt, CATEGORY_VERSION,
        };
        tokio::time::timeout_at(deadline, async {
            source.check(command, deadline)?;
            // The Party child establishes the canonical common fence before
            // the Gift child consumes that same fence in its own transaction.
            let party = self.party.apply(command, source, deadline).await?;
            if !party.verifies(command, source.source_binding()) {
                return Err(DeletionFailure::InvalidCommand);
            }
            source.check(command, deadline)?;
            let gift = self.gift.apply(command, source, deadline).await?;
            if !gift.verifies(command, source.source_binding()) {
                return Err(DeletionFailure::InvalidCommand);
            }
            source.check(command, deadline)?;
            let complete = party.complete && gift.complete;
            let progress_revision = if complete {
                party
                    .owner_revision
                    .checked_add(gift.owner_revision)
                    .ok_or(DeletionFailure::Unavailable)?
            } else {
                0
            };
            Ok(PrivateDataCategoryReceipt {
                command: command.clone(),
                category: PrivateDataCategory::PartyAndGiftInput,
                version: CATEGORY_VERSION,
                source_binding: *source.source_binding(),
                progress_revision,
                complete,
            })
        })
        .await
        .map_err(|_| DeletionFailure::Unavailable)?
    }
}
