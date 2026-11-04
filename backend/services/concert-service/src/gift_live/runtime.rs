//! Actual Gift business facade. Root owns canonical acquisition and HTTP completion.
use super::{
    authority::{validate_binding, ScopedGiftAuthority},
    cursor::{Collection, CursorBinding, CursorCodec, CursorError, Window},
    media::GiftMediaAdapter,
    protocol::*,
    queries::{GiftReads, Page},
    response,
};
use crate::{
    adapters::out::relationship_authority::guarded::{GuardedSubject, OwnerGuardScope},
    community::giveaway::{
        GiftFilter, GiftServiceError, GiftSummary, PgGifts, TrustedPickupContext,
    },
};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{
    commands::{CommandError, FingerprintKeys, ReceiptState},
    community::giveaway::{GiftCommand, PickupKeys},
    feed::PublicAuthor,
    media::ImageSanitizer,
};
use concert_domain::{
    community::giveaway::{GiftState, ReservationState},
    feed::FeedError,
};
use serde_json::{json, Value};
use sqlx::PgPool;
use std::{collections::BTreeMap, sync::Arc};
use tonic::Status;
use uuid::Uuid;

/// Only the actual canonical public-profile reader belongs in production.
/// The sealed SDK subject cannot be manufactured from UUIDs/booleans/observations.
#[async_trait]
pub trait GiftProfiles: Send + Sync {
    async fn read(&self, subject: &GuardedSubject) -> Result<PublicAuthor, FeedError>;
}
pub struct GiftRuntime {
    pool: PgPool,
    receipts: Arc<FingerprintKeys>,
    pickup: Arc<PickupKeys>,
    profiles: Arc<dyn GiftProfiles>,
    sanitizer: Arc<dyn ImageSanitizer>,
    cursor: CursorCodec,
}
type Authority = ScopedGiftAuthority<GiftMediaAdapter>;
impl GiftRuntime {
    pub fn new(
        pool: PgPool,
        receipts: Arc<FingerprintKeys>,
        pickup: Arc<PickupKeys>,
        profiles: Arc<dyn GiftProfiles>,
        sanitizer: Arc<dyn ImageSanitizer>,
        cursor_key: &[u8; 32],
    ) -> Result<Self, CommandError> {
        Ok(Self {
            pool,
            receipts,
            pickup,
            profiles,
            sanitizer,
            cursor: CursorCodec::new(cursor_key).map_err(|_| CommandError::Unavailable)?,
        })
    }
    pub async fn dispatch(
        &self,
        scope: Arc<OwnerGuardScope>,
        request: GiftRequest,
        network: Option<TrustedPickupContext>,
    ) -> Result<GiftReply, Status> {
        validate_binding(&request, &scope)?;
        let i = request.identity();
        let op = request.operation();
        // run-return never releases the root or its children. Only the actual
        // Gateway HTTP-body terminal capability may request that settlement.
        scope
            .run(async {
                let result = self
                    .dispatch_scoped(&scope, request, network.as_ref())
                    .await;

                scope.binding().check()?;
                match result {
                    Ok((status, body)) => response::checked_reply(op, status, body)
                        .map_err(|_| Status::unavailable("Gift projection unavailable")),
                    Err(e) => Ok(response::Failure::from(e).reply(i.request_id)),
                }
            })
            .await
    }
    async fn dispatch_scoped(
        &self,
        scope: &Arc<OwnerGuardScope>,
        request: GiftRequest,
        network: Option<&TrustedPickupContext>,
    ) -> Result<(u16, Value), GiftServiceError> {
        let i = request.identity();
        let op = request.operation();
        let key = request.idempotency_key();

        if op == GiftOperation::Claim && network.is_none() {
            return Err(CommandError::Unavailable.into());
        }
        let media = GiftMediaAdapter::new(
            self.pool.clone(),
            &request,
            scope.clone(),
            self.receipts.clone(),
            self.sanitizer.clone(),
        )?;
        let authority =
            ScopedGiftAuthority::new(self.pool.clone(), &request, scope.clone(), media)?;
        let gifts = PgGifts::new(self.pool.clone(), authority.clone());
        let reads = GiftReads {
            pool: &self.pool,
            authority: &authority,
            gifts: &gifts,
            identity: i,
            cursor: &self.cursor,
        };
        match request.into_input() {
            GiftInput::Read { target, query } => self
                .read(op, i, target, query, &scope, &gifts, &reads)
                .await
                .map(|v| (200, v)),
            GiftInput::Write {
                giveaway,
                reservation,
                action,
                code,
            } => {
                let root = if let Some(r) = reservation {
                    reads
                        .root_for_reservation(
                            r,
                            op == GiftOperation::Cancel,
                            op == GiftOperation::Claim,
                            op == GiftOperation::IssueCode,
                        )
                        .await?
                } else {
                    giveaway.unwrap_or(Uuid::nil())
                };
                let cmd = GiftCommand {
                    actor: i.actor,
                    event: i.event,
                    key: key.ok_or(CommandError::InvalidInput)?,
                    giveaway: root,
                    action,
                    code,
                };
                let result = gifts
                    .execute(&cmd, &self.receipts, &self.pickup, network)
                    .await?;
                if result.receipt.state == ReceiptState::Pending {
                    let body = response::receipt(i, op.id(), cmd.key, &result.receipt)
                        .map_err(projection)?;
                    return Ok((202, body));
                }
                if result.receipt.state != ReceiptState::Succeeded {
                    return Err(CommandError::Unavailable.into());
                }
                let body = if op == GiftOperation::IssueCode {
                    // Never report SUCCEEDED as PENDING or regenerate a lost code.
                    let issued = result.issued_code.ok_or(CommandError::InvalidState)?;
                    response::issued(issued).map_err(projection)?
                } else {
                    match op {
                        GiftOperation::Create | GiftOperation::Update => {
                            let current = reads.owned(cmd.giveaway_id()).await?;
                            self.summary(GiftSummary::from(&current), &scope).await?
                        }
                        GiftOperation::SetMeeting => {
                            let current = reads.owned(cmd.giveaway_id()).await?;
                            let meeting = current.meeting.ok_or(CommandError::Unavailable)?;
                            json!({"giveaway_id":current.id,"revision":current.revision,"meeting":meeting})
                        }
                        GiftOperation::Reserve | GiftOperation::Cancel | GiftOperation::Claim => {
                            let r = gifts
                                .reservation(i.actor, i.event, cmd.giveaway_id(), cmd.target())
                                .await?;
                            if op == GiftOperation::Claim {
                                response::claimed(cmd.giveaway_id(), &r).map_err(projection)?
                            } else {
                                response::reservation(i.event, cmd.giveaway_id(), &r)
                                    .map_err(projection)?
                            }
                        }
                        _ => return Err(CommandError::InvalidInput.into()),
                    }
                };
                Ok((200, body))
            }
        }
    }
    async fn summary(
        &self,
        g: GiftSummary,
        scope: &Arc<OwnerGuardScope>,
    ) -> Result<Value, GiftServiceError> {
        let subject = scope
            .public_profile_subject(g.owner)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        let author = self.profiles.read(&subject).await.map_err(|e| match e {
            FeedError::PermissionDenied | FeedError::ResourceUnavailable => CommandError::Denied,
            _ => CommandError::Unavailable,
        })?;
        subject
            .check()
            .await
            .map_err(|_| CommandError::Unavailable)?;
        response::giveaway(g, author).map_err(projection)
    }
    async fn next(
        &self,
        b: &CursorBinding,
        w: Option<Window>,
    ) -> Result<Option<String>, GiftServiceError> {
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&self.pool)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        w.map(|w| self.cursor.seal(b, w, now).map_err(cursor))
            .transpose()
    }
    async fn gift_page(
        &self,
        b: &CursorBinding,
        page: Page<GiftSummary>,
        scope: &Arc<OwnerGuardScope>,
    ) -> Result<Value, GiftServiceError> {
        let mut items = Vec::with_capacity(page.items.len());
        for g in page.items {
            items.push(self.summary(g, scope).await?);
        }
        Ok(json!({"items":items,"next_cursor":self.next(b,page.next).await?}))
    }
    async fn read(
        &self,
        op: GiftOperation,
        i: GiftIdentity,
        target: Option<Uuid>,
        q: BTreeMap<String, String>,
        scope: &Arc<OwnerGuardScope>,
        gifts: &PgGifts<Authority>,
        reads: &GiftReads<'_, Authority>,
    ) -> Result<Value, GiftServiceError> {
        match op {
            GiftOperation::List => {
                let b = binding(i, op, Collection::Public, None, &q)?;
                let page = reads
                    .list(&b, q.get("cursor").map(String::as_str), filter(&q)?)
                    .await?;
                self.gift_page(&b, page, scope).await
            }
            GiftOperation::Detail => {
                let id = target.ok_or(CommandError::InvalidInput)?;
                let g = match gifts.detail(i.actor, i.event, id).await {
                    Ok(g) => g,
                    Err(GiftServiceError::Command(CommandError::Denied)) => {
                        GiftSummary::from(&reads.owned(id).await?)
                    }
                    Err(e) => return Err(e),
                };
                self.summary(g, scope).await
            }
            GiftOperation::Management => {
                let id = target.ok_or(CommandError::InvalidInput)?;
                let b = binding(i, op, Collection::Management, Some(id), &q)?;
                let state = q
                    .get("state")
                    .map(|v| {
                        serde_json::from_value::<ReservationState>(json!(v))
                            .map_err(|_| CommandError::InvalidInput)
                    })
                    .transpose()?;
                let page = reads
                    .management(id, &b, q.get("cursor").map(String::as_str), state)
                    .await?;
                let giveaway = self.summary(page.giveaway, scope).await?;
                let reservations = page
                    .reservations
                    .iter()
                    .map(|r| response::reservation(i.event, id, r).map_err(projection))
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(
                    json!({"giveaway":giveaway,"reserved_unclaimed":page.reserved,"quantity_claimed":page.claimed,"reservations":reservations,"next_cursor":self.next(&b,page.next).await?}),
                )
            }
            GiftOperation::Activity => {
                let giving = binding(i, op, Collection::Giving, None, &q)?;
                let receiving = binding(i, op, Collection::Receiving, None, &q)?;
                let g = reads
                    .list(
                        &giving,
                        q.get("giving_cursor").map(String::as_str),
                        filter(&BTreeMap::new())?,
                    )
                    .await?;
                let r = reads
                    .receiving(&receiving, q.get("receiving_cursor").map(String::as_str))
                    .await?;
                let giving = self.gift_page(&giving, g, scope).await?;
                let items = r
                    .items
                    .iter()
                    .map(|r| {
                        response::reservation(i.event, r.giveaway, &r.value).map_err(projection)
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                Ok(
                    json!({"giving":giving,"receiving":{"items":items,"next_cursor":self.next(&receiving,r.next).await?}}),
                )
            }
            GiftOperation::Reservation | GiftOperation::Pickup => {
                let id = target.ok_or(CommandError::InvalidInput)?;
                let root = reads
                    .root_for_reservation(id, false, false, op == GiftOperation::Pickup)
                    .await?;
                if op == GiftOperation::Pickup {
                    let v = gifts.pickup(i.actor, i.event, root, id).await?;
                    response::pickup(i.event, root, &v.reservation, &v.meeting).map_err(projection)
                } else {
                    let r = gifts.reservation(i.actor, i.event, root, id).await?;
                    response::reservation(i.event, root, &r).map_err(projection)
                }
            }
            _ => Err(CommandError::InvalidInput.into()),
        }
    }
}
fn projection(_: response::ProjectionError) -> GiftServiceError {
    CommandError::Unavailable.into()
}
fn cursor(e: CursorError) -> GiftServiceError {
    match e {
        CursorError::Invalid => CommandError::InvalidInput,
        CursorError::Unavailable => CommandError::Unavailable,
    }
    .into()
}
fn binding(
    i: GiftIdentity,
    op: GiftOperation,
    collection: Collection,
    giveaway: Option<Uuid>,
    q: &BTreeMap<String, String>,
) -> Result<CursorBinding, GiftServiceError> {
    let limit = q
        .get("limit")
        .map(|v| v.parse::<u16>().map_err(|_| CommandError::InvalidInput))
        .transpose()?
        .unwrap_or(20);
    let filters = q
        .iter()
        .filter(|(k, _)| {
            !matches!(
                k.as_str(),
                "cursor" | "giving_cursor" | "receiving_cursor" | "limit"
            )
        })
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();
    Ok(CursorBinding {
        actor: i.actor,
        event: i.event,
        operation: op.id().into(),
        collection,
        giveaway,
        filters,
        limit,
    })
}
fn filter(q: &BTreeMap<String, String>) -> Result<GiftFilter, GiftServiceError> {
    Ok(GiftFilter {
        state: q
            .get("state")
            .map(|v| {
                serde_json::from_value::<GiftState>(json!(v))
                    .map_err(|_| CommandError::InvalidInput)
            })
            .transpose()?,
        in_stock: q
            .get("in_stock")
            .map(|v| v.parse::<bool>().map_err(|_| CommandError::InvalidInput))
            .transpose()?,
        pickup_after: q
            .get("pickup_after")
            .map(|v| {
                v.parse::<DateTime<Utc>>()
                    .map_err(|_| CommandError::InvalidInput)
            })
            .transpose()?,
        pickup_before: q
            .get("pickup_before")
            .map(|v| {
                v.parse::<DateTime<Utc>>()
                    .map_err(|_| CommandError::InvalidInput)
            })
            .transpose()?,
        public_place_id: q
            .get("public_place_id")
            .map(|v| canonical_uuid(v).map_err(|_| CommandError::InvalidInput))
            .transpose()?,
        after: None,
    })
}
