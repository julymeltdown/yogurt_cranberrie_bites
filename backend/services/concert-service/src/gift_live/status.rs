//! Readonly current-authorized status; no fabricated original mutation or code replay.
use super::{
    authority::ScopedGiftAuthority,
    protocol::{canonical_uuid, parse_query, GiftIdentity, GiftOperation, GiftReply},
    queries,
    resources::GiftMediaLink,
    response,
    runtime::GiftRuntime,
};
use crate::{
    adapters::out::{
        relationship_authority::guarded::OwnerGuardScope,
        repository::receipt::{CommandProducer, PgCommandRepository},
    },
    community::{giveaway::GiftAuthority, CommunityAccess},
};
use async_trait::async_trait;
use concert_application::{commands::*, community::resource_id};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, Row};
use std::sync::Arc;
use tonic::Status;
use uuid::Uuid;

pub(crate) struct StatusContext {
    pub identity: GiftIdentity,
    pub operation: GiftOperation,
    pub receipt_scope: Scope,
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid Gift command status")
}
fn query(
    event: Uuid,
    target: &str,
    operation: RegisteredOperation,
    key: Uuid,
) -> Result<GiftOperation, Status> {
    let op = GiftOperation::parse(operation.id()).map_err(|_| invalid())?;
    if !op.mutation() || key.is_nil() || target.len() > 4096 || target.chars().any(char::is_control)
    {
        return Err(invalid());
    }
    let (path, raw) = target.split_once('?').ok_or_else(invalid)?;
    if path != format!("/api/concert/events/{event}/commands") {
        return Err(invalid());
    }
    let params = parse_query(Some(raw)).map_err(|_| invalid())?;
    if params.len() != 2
        || params.get("operation_id").map(String::as_str) != Some(operation.id())
        || params
            .get("idempotency_key")
            .and_then(|v| canonical_uuid(v).ok())
            != Some(key)
    {
        return Err(invalid());
    }
    Ok(op)
}
impl StatusContext {
    fn from_scope(
        scope: &OwnerGuardScope,
        operation: RegisteredOperation,
        key: Uuid,
    ) -> Result<Self, Status> {
        let b = scope.binding();
        b.check()?;
        if b.operation() != "getConcertCommandStatus"
            || b.http_method() != "GET"
            || b.body_digest() != <[u8; 32]>::from(Sha256::digest([]))
        {
            return Err(Status::permission_denied("Gift status scope mismatch"));
        }
        let op = query(b.event(), b.http_target(), operation, key)?;
        let identity = GiftIdentity {
            actor: b.actor(),
            event: b.event(),
            session_family: b.session_family(),
            request_id: b.request_id(),
            deadline: b.deadline(),
        };
        identity
            .check()
            .map_err(|_| Status::deadline_exceeded("Gift status deadline"))?;
        Ok(Self {
            identity,
            operation: op,
            receipt_scope: Scope {
                actor: b.actor(),
                event: b.event(),
                operation,
                key,
            },
        })
    }
}
struct NoStatusMedia;
#[async_trait]
impl GiftMediaLink for NoStatusMedia {
    async fn link_in(
        &self,
        _: &mut PgConnection,
        _: GiftIdentity,
        _: Uuid,
        _: Uuid,
    ) -> Result<(), CommandError> {
        Err(CommandError::Denied)
    }
}
struct StatusProducer<'a, A> {
    context: &'a StatusContext,
    target: Uuid,
    root: Uuid,
    authority: A,
}
fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
#[async_trait]
impl<A: GiftAuthority> CommandProducer for StatusProducer<'_, A> {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if scope != self.context.receipt_scope || target != self.target {
            return Err(CommandError::Denied);
        }
        let access = if self.context.operation == GiftOperation::Cancel {
            CommunityAccess::SafetyGifts
        } else {
            CommunityAccess::ReadGifts
        };
        self.authority
            .actor_in(c, scope.actor, scope.event, access)
            .await?;
        let row=sqlx::query("select * from concert_community_giveaways where event_id=$1 and giveaway_id=$2 for share")
            .bind(scope.event).bind(self.root).fetch_optional(&mut *c).await.map_err(db)?;
        let mut subjects = vec![scope.actor];
        if let Some(row) = row {
            let g = queries::parse(&row, scope.event).map_err(|_| CommandError::Unavailable)?;
            match self.context.operation {
                GiftOperation::Create | GiftOperation::Update | GiftOperation::SetMeeting => {
                    if g.owner != scope.actor || self.root != target {
                        return Err(CommandError::Denied);
                    }
                }
                GiftOperation::Reserve => {
                    if resource_id(scope.actor, scope.event, scope.key, scope.operation.id())
                        != target
                    {
                        return Err(CommandError::Denied);
                    }
                    if let Some(r) = g.reservations.iter().find(|r| r.id == target) {
                        if r.recipient != scope.actor {
                            return Err(CommandError::Denied);
                        }
                    }
                }
                GiftOperation::Cancel | GiftOperation::IssueCode | GiftOperation::Claim => {
                    let r = g
                        .reservations
                        .iter()
                        .find(|r| r.id == target)
                        .ok_or(CommandError::Denied)?;
                    let permitted = match self.context.operation {
                        GiftOperation::Cancel => {
                            scope.actor == g.owner || scope.actor == r.recipient
                        }
                        GiftOperation::IssueCode => scope.actor == r.recipient,
                        GiftOperation::Claim => scope.actor == g.owner,
                        _ => false,
                    };
                    if !permitted {
                        return Err(CommandError::Denied);
                    }
                    let actual:Option<Uuid>=sqlx::query_scalar("select recipient_id from concert_community_reservations where event_id=$1 and giveaway_id=$2 and reservation_id=$3 for share")
                        .bind(scope.event).bind(self.root).bind(target).fetch_optional(&mut *c).await.map_err(db)?;
                    if actual != Some(r.recipient) {
                        return Err(CommandError::Unavailable);
                    }
                    subjects.push(r.recipient);
                }
                _ => return Err(CommandError::Denied),
            }
            subjects.push(g.owner);
        } else if self.context.operation != GiftOperation::Create
            || self.root != target
            || resource_id(scope.actor, scope.event, scope.key, scope.operation.id()) != target
        {
            return Err(CommandError::Denied);
        }
        subjects.sort();
        subjects.dedup();
        for subject in subjects {
            self.authority
                .authorize_in(c, scope.actor, scope.event, subject, access)
                .await?;
        }
        Ok(())
    }
    async fn apply(
        &self,
        _: &mut PgConnection,
        _: Scope,
        _: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        Err(CommandError::Denied)
    }
}
impl GiftRuntime {
    pub async fn command_status(
        &self,
        scope: Arc<OwnerGuardScope>,
        operation: RegisteredOperation,
        key: Uuid,
    ) -> Result<GiftReply, Status> {
        let context = StatusContext::from_scope(&scope, operation, key)?;
        scope.run(async {
            let result=async {
                // Internal correlated hint only; PgCommandRepository reauthorizes
                // the immutable actual target under its own current transaction.
                let row=sqlx::query("select t.target_id,t.giveaway_id from concert_gift_command_targets t join concert_command_receipts r on r.receipt_id=t.receipt_id and r.actor_id=t.actor_id and r.event_id=t.event_id and r.operation_id=t.operation_id and r.idempotency_key=t.idempotency_key and r.target_id=t.target_id where t.actor_id=$1 and t.event_id=$2 and t.operation_id=$3 and t.idempotency_key=$4")
                    .bind(context.identity.actor).bind(context.identity.event).bind(operation.id()).bind(key)
                    .fetch_optional(&self.pool).await.map_err(db)?.ok_or(CommandError::Denied)?;
                let authority=ScopedGiftAuthority::for_status(self.pool.clone(),&context,scope.clone(),NoStatusMedia)?;
                let producer=StatusProducer {context:&context,target:row.try_get("target_id").map_err(db)?,root:row.try_get("giveaway_id").map_err(db)?,authority};
                let receipt=PgCommandRepository::new(self.pool.clone()).lookup(context.receipt_scope,producer.target,&producer)
                    .await?.ok_or(CommandError::Denied)?;
                let body=response::receipt(context.identity,operation.id(),key,&receipt).map_err(|_|CommandError::Unavailable)?;
                response::status_reply(body).map_err(|_|CommandError::Unavailable)
            }.await;
            scope.binding().check()?;
            Ok(match result {Ok(reply)=>reply,Err(e)=>response::Failure::from(e).reply(context.identity.request_id)})
        }).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn binds_exact_closed_query_to_gift_mutation_and_canonical_key() {
        let event = Uuid::from_u128(7);
        let key = Uuid::from_u128(8);
        let op = RegisteredOperation::find("reserveConcertGiveaway").unwrap();
        let target = format!(
            "/api/concert/events/{event}/commands?operation_id={}&idempotency_key={key}",
            op.id()
        );
        assert_eq!(
            query(event, &target, op, key).unwrap(),
            GiftOperation::Reserve
        );
        assert!(query(event, &target, op, Uuid::from_u128(9)).is_err());
        for extra in [
            "&target_id=untrusted",
            "&operation_id=reserveConcertGiveaway",
            "&%6fperation_id=reserveConcertGiveaway",
            "&",
            "&idempotency_key=bad",
        ] {
            assert!(query(event, &format!("{target}{extra}"), op, key).is_err());
        }
        assert!(query(Uuid::from_u128(9), &target, op, key).is_err());
        let other = RegisteredOperation::find("createConcertParty").unwrap();
        assert!(query(event, &target, other, key).is_err());
    }
}
