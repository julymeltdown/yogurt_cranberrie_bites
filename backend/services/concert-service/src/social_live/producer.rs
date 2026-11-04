use super::{
    cursor,
    keys::ManagedSocialKeys,
    protocol::*,
    provenance::SocialClaimProvenance,
    read_child::{retain_projected_in, SocialReadSources},
    witness::SocialWitness,
};
use crate::{
    adapters::out::{
        relationship_authority::guarded::{GuardedAuthority, OwnerGuardScope},
        repository::receipt::{CommandProducer, PgCommandRepository},
    },
    product_rpc::{
        proto,
        scoped::{ProductRepresentation, ScopedProductProducer},
        VerifiedProduct,
    },
    relationship::{
        commands::BlockCommand,
        eligibility::{PgSocialEligibility, SharingVerifierStatus, VerifierStatus},
        social::{SocialEffect, SocialService, SocialTransactionHooks},
        FencedRelationshipAuthority, PgRelationshipPolicy,
    },
};
use chrono::{DateTime, SecondsFormat, Utc};
use concert_application::{age_policy::Context, commands::*};
use concert_domain::{
    relationship::{social::Platform, Field, PolicyQuery, RelationshipError},
    AuthUserId, EventId, ParticipantKey,
};
use prost::Message;
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use tonic::Status;
use uuid::Uuid;

pub struct SocialRuntime {
    pool: PgPool,
    keys: Arc<ManagedSocialKeys>,
    verifier: Arc<SocialClaimProvenance>,
    ttl_seconds: i64,
}
impl SocialRuntime {
    /// `verifier` recognizes trusted persisted-claim provenance, independently
    /// of new verification availability. Missing trusted configuration uses
    /// SocialClaimProvenance::Unavailable; an outage alone does not revoke valid
    /// claims. Hold this immutable snapshot until all its scopes have drained.
    pub fn new(
        pool: PgPool,
        keys: Arc<ManagedSocialKeys>,
        verifier: Arc<SocialClaimProvenance>,
        ttl_seconds: i64,
    ) -> Result<Self, Status> {
        if !(1..=86400).contains(&ttl_seconds) {
            return Err(unavailable());
        }
        Ok(Self {
            pool,
            keys,
            verifier,
            ttl_seconds,
        })
    }
    async fn execute(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        raw: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        context.check()?;
        let binding = scope.binding();
        binding.check()?;
        let parent = proto::ProductRequest::decode(
            context
                .parent_authority()
                .message()
                .map_err(|_| Status::permission_denied("Social parent mismatch"))?,
        )
        .map_err(|_| Status::permission_denied("Social parent mismatch"))?;
        if parent != raw
            || (
                binding.actor(),
                binding.event(),
                binding.session_family(),
                binding.request_id(),
            ) != (
                context.actor(),
                context.event(),
                context.session(),
                context.request(),
            )
            || binding.parent_digest() != context.parent_authority().parent_digest()
            || binding.http_method() != raw.http_method
            || binding.http_target() != raw.http_target
            || binding.body_digest() != <[u8; 32]>::from(Sha256::digest(&raw.http_body))
            || binding.deadline().expires_micros() > context.deadline().expires_micros()
        {
            return Err(Status::permission_denied("Social scope mismatch"));
        }
        let input = SocialRequest::parse(
            context.actor(),
            context.event(),
            &raw.http_method,
            &raw.http_target,
            &raw.http_body,
            &raw.content_type,
            &raw.idempotency_key,
        )?;
        let op = input.operation;
        if op.id() != binding.operation() || op.id() != context.operation().contract_id() {
            return Err(Status::permission_denied("Social operation mismatch"));
        }
        let actor = ParticipantKey {
            event_id: EventId::from_uuid(context.event()),
            user_id: AuthUserId::from_uuid(context.actor()),
        };
        let retained = match (&op, &input.input) {
            (SocialOperation::Handles, _) => Some(SocialReadSources::Metadata { subjects: vec![] }),
            (SocialOperation::Reveal(id), SocialInput::Reveal(platform)) => {
                Some(SocialReadSources::Reveal {
                    request: *id,
                    platform: *platform,
                })
            }
            _ => None,
        };
        let hooks = Arc::new(RuntimeHooks {
            witness: Arc::new(SocialWitness::new(scope.clone(), input.key)),
            write_gate: Arc::new(ReadOnlyGate::default()),
            command_transaction: true,
            scope: scope.clone(),
            pool: self.pool.clone(),
            operation: op,
            retained,
        });
        let service = SocialService::new(
            self.pool.clone(),
            GuardedAuthority::new(scope.clone()),
            PgSocialEligibility::from_guarded(scope.clone(), Verifier(self.verifier.clone()))
                .map_err(rel)?,
            self.keys.handles()?,
            self.ttl_seconds,
        )
        .map_err(rel)?
        .with_transaction_hooks(hooks.clone());
        let command_keys = self.keys.commands()?;
        let key = input.key;
        let mut receipt = None;
        match (op, input.input) {
            (SocialOperation::Handles, SocialInput::Read) => {
                let handles = service.own_handles(actor).await.map_err(rel)?;
                return representation(
                    200,
                    json!({"items":handles.into_iter().map(|h|json!({"user_id":h.owner.uuid(),"platform":h.platform.as_str(),"version":h.version,"configured":h.configured})).collect::<Vec<_>>()}),
                );
            }
            (SocialOperation::Reveal(id), SocialInput::Reveal(platform)) => {
                let r = service.reveal(actor, id, platform).await.map_err(rel)?;
                return representation(
                    200,
                    json!({"request_id":r.request_id,"platform":r.context.platform.as_str(),"owner_id":r.context.owner.uuid(),"handle_version":r.context.version.get(),"handle":r.handle.expose_for_reveal(),"grant_revision":r.grant_revision.get(),"expires_at":timestamp(r.expires_at)}),
                );
            }
            (SocialOperation::SetHandle(platform), SocialInput::Set { expected, handle }) => {
                receipt = Some(
                    service
                        .set_handle(
                            actor,
                            platform,
                            expected,
                            handle,
                            key.ok_or_else(unavailable)?,
                            &command_keys,
                        )
                        .await
                        .map_err(|error| hooks.command(error))?,
                )
            }
            (SocialOperation::RemoveHandle(platform), SocialInput::Remove(expected)) => {
                receipt = Some(
                    service
                        .remove_handle(
                            actor,
                            platform,
                            expected,
                            key.ok_or_else(unavailable)?,
                            &command_keys,
                        )
                        .await
                        .map_err(|error| hooks.command(error))?,
                )
            }
            (SocialOperation::Create, SocialInput::Create(input)) => {
                receipt = Some(
                    service
                        .create(actor, input, key.ok_or_else(unavailable)?, &command_keys)
                        .await
                        .map_err(|error| hooks.command(error))?,
                );
            }
            (SocialOperation::Decide(id), SocialInput::Decide(input)) => {
                receipt = Some(
                    service
                        .decide(
                            actor,
                            id,
                            input,
                            key.ok_or_else(unavailable)?,
                            &command_keys,
                        )
                        .await
                        .map_err(|error| hooks.command(error))?,
                )
            }
            (SocialOperation::Cancel(id), SocialInput::Cancel(expected)) => {
                receipt = Some(
                    service
                        .cancel(
                            actor,
                            id,
                            expected,
                            key.ok_or_else(unavailable)?,
                            &command_keys,
                        )
                        .await
                        .map_err(|error| hooks.command(error))?,
                )
            }
            (SocialOperation::Revoke(id), SocialInput::Revoke(expected)) => {
                receipt = Some(
                    service
                        .revoke(
                            actor,
                            id,
                            expected,
                            key.ok_or_else(unavailable)?,
                            &command_keys,
                        )
                        .await
                        .map_err(|error| hooks.command(error))?,
                )
            }
            (
                SocialOperation::Block(id) | SocialOperation::Unblock(id),
                SocialInput::Block(blocked),
            ) => {
                let policy = PgRelationshipPolicy::new(
                    self.pool.clone(),
                    GuardedAuthority::new(scope.clone()),
                );
                let producer = ObservedBlock {
                    inner: BlockCommand {
                        policy: &policy,
                        query: query(actor, id, Field::SafetyInteraction),
                        blocked,
                    },
                    hooks: hooks.clone(),
                };
                let scope = Scope {
                    actor: context.actor(),
                    event: context.event(),
                    operation: RegisteredOperation::find(op.id())
                        .map_err(|error| hooks.command(error))?,
                    key: key.ok_or_else(unavailable)?,
                };
                let event = context.event().to_string();
                let subject = id.to_string();
                let empty = CanonicalValue::Object(vec![]);
                let request = CanonicalRequest::json(
                    scope,
                    &[("eventId", &event), ("userId", &subject)],
                    &[],
                    if blocked { Some(&empty) } else { None },
                )
                .map_err(|error| hooks.command(error))?;
                receipt = Some(
                    PgCommandRepository::new(self.pool.clone())
                        .execute(&request, id, &command_keys, &producer)
                        .await
                        .map_err(|error| hooks.command(error))?,
                );
            }
            (SocialOperation::Requests | SocialOperation::Blocks, SocialInput::Page(page)) => {
                let value = self.list(actor, op, &page, scope, hooks).await?;
                return representation(200, value);
            }
            (SocialOperation::Request(id), SocialInput::Read) => {
                let value = self.detail(actor, id, None, scope, hooks, op).await?;
                return representation(200, value);
            }
            _ => return Err(Status::invalid_argument("Invalid social input")),
        }
        let receipt = receipt.ok_or_else(unavailable)?;
        if receipt.state != ReceiptState::Succeeded {
            return Err(unavailable());
        }
        let reference = receipt.reference.ok_or_else(unavailable)?;
        // Projection is a new read transaction after the command commits.
        // Do not classify post-command reads as new NORMAL commands.
        let hooks = hooks.for_projection();
        let value = match op {
            SocialOperation::SetHandle(platform) | SocialOperation::RemoveHandle(platform) => {
                self.handle_state(actor, platform, reference.revision, scope, hooks, op)
                    .await?
            }
            SocialOperation::Block(id) | SocialOperation::Unblock(id) => {
                self.block_state(actor, id, reference.revision, scope, hooks, op)
                    .await?
            }
            _ => {
                self.detail(
                    actor,
                    reference.resource,
                    Some(reference.revision),
                    scope,
                    hooks,
                    op,
                )
                .await?
            }
        };
        representation(200, value)
    }
    async fn begin(
        &self,
        actor: ParticipantKey,
        scope: Arc<OwnerGuardScope>,
        hooks: &RuntimeHooks,
    ) -> Result<
        (
            sqlx::Transaction<'_, sqlx::Postgres>,
            PgRelationshipPolicy<GuardedAuthority>,
        ),
        Status,
    > {
        let mut tx = self.pool.begin().await.map_err(|_| unavailable())?;
        hooks.register(&mut tx).await.map_err(rel)?;
        let ms = scope
            .binding()
            .deadline()
            .remaining(
                std::time::Duration::from_secs(5),
                scope.binding().deadline().expires_micros(),
            )?
            .as_millis()
            .max(1)
            .to_string();
        sqlx::query(
            "select set_config('statement_timeout',$1,true),set_config('lock_timeout',$1,true)",
        )
        .bind(ms)
        .execute(&mut *tx)
        .await
        .map_err(|_| unavailable())?;
        sqlx::query("select event_id from concert_events where event_id=$1 for share")
            .bind(actor.event_id.uuid())
            .fetch_optional(&mut *tx)
            .await
            .map_err(|_| unavailable())?
            .ok_or_else(|| Status::not_found("Social resource unavailable"))?;
        Ok((
            tx,
            PgRelationshipPolicy::new(self.pool.clone(), GuardedAuthority::new(scope)),
        ))
    }
    async fn detail(
        &self,
        actor: ParticipantKey,
        id: Uuid,
        expected: Option<i64>,
        scope: Arc<OwnerGuardScope>,
        hooks: Arc<RuntimeHooks>,
        op: SocialOperation,
    ) -> Result<Value, Status> {
        let (mut tx, policy) = self.begin(actor, scope, &hooks).await?;
        let row=sqlx::query("select request_id,event_id,requester_id,recipient_id,intro_id,intro_revision,state,revision,expires_at,created_at from concert_social_requests where event_id=$1 and request_id=$2 and (requester_id=$3 or recipient_id=$3) for share")
            .bind(actor.event_id.uuid()).bind(id).bind(actor.user_id.uuid()).fetch_optional(&mut *tx).await.map_err(|_|unavailable())?.ok_or_else(||Status::not_found("Social resource unavailable"))?;
        let revision = row.get::<i64, _>("revision");
        if expected.is_some_and(|n| n != revision) {
            return Err(Status::aborted("Social revision changed"));
        }
        let peer = peer(&row, actor.user_id.uuid());
        if matches!(op, SocialOperation::Cancel(_) | SocialOperation::Revoke(_)) {
            policy
                .authorize_actor_in(&mut tx, actor)
                .await
                .map_err(rel)?;
        } else {
            policy
                .authorize_in(&mut tx, query(actor, peer, metadata_field(op)), None)
                .await
                .map_err(rel)?;
        }
        let (value, until) = request_value(&mut tx, &row).await?;
        hooks
            .succeeded(
                &mut tx,
                SocialEffect {
                    operation: op.id(),
                    resource: id,
                    revision,
                    projection: true,
                },
            )
            .await
            .map_err(rel)?;
        let subjects = if matches!(op, SocialOperation::Cancel(_) | SocialOperation::Revoke(_)) {
            vec![]
        } else {
            vec![peer]
        };
        hooks
            .retain(&mut tx, SocialReadSources::Metadata { subjects }, until)
            .await?;
        tx.commit().await.map_err(|_| unavailable())?;
        Ok(value)
    }
    async fn handle_state(
        &self,
        actor: ParticipantKey,
        platform: Platform,
        expected: i64,
        scope: Arc<OwnerGuardScope>,
        hooks: Arc<RuntimeHooks>,
        op: SocialOperation,
    ) -> Result<Value, Status> {
        let (mut tx, policy) = self.begin(actor, scope, &hooks).await?;
        policy
            .authorize_in(
                &mut tx,
                query(actor, actor.user_id.uuid(), Field::PublicProfile),
                None,
            )
            .await
            .map_err(rel)?;
        let row=sqlx::query("select revision,ciphertext is not null as configured from concert_social_handles where event_id=$1 and user_id=$2 and platform=$3 for share").bind(actor.event_id.uuid()).bind(actor.user_id.uuid()).bind(platform.as_str()).fetch_one(&mut *tx).await.map_err(|_|unavailable())?;
        if row.get::<i64, _>("revision") != expected {
            return Err(Status::aborted("Social revision changed"));
        }
        let value = json!({"user_id":actor.user_id.uuid(),"platform":platform.as_str(),"version":expected,"configured":row.get::<bool,_>("configured")});
        hooks
            .succeeded(
                &mut tx,
                SocialEffect {
                    operation: op.id(),
                    resource: actor.user_id.uuid(),
                    revision: expected,
                    projection: true,
                },
            )
            .await
            .map_err(rel)?;
        hooks
            .retain(
                &mut tx,
                SocialReadSources::Metadata { subjects: vec![] },
                None,
            )
            .await?;
        tx.commit().await.map_err(|_| unavailable())?;
        Ok(value)
    }
    async fn block_state(
        &self,
        actor: ParticipantKey,
        id: Uuid,
        expected: i64,
        scope: Arc<OwnerGuardScope>,
        hooks: Arc<RuntimeHooks>,
        op: SocialOperation,
    ) -> Result<Value, Status> {
        let (mut tx, policy) = self.begin(actor, scope, &hooks).await?;
        // The actor may inspect its own safety action even when the pair is blocked.
        policy
            .authorize_actor_in(&mut tx, actor)
            .await
            .map_err(rel)?;
        let row=sqlx::query("select revision,case when low_user=$2 then low_blocks_high else high_blocks_low end as blocked from concert_relationship_pairs where event_id=$1 and low_user=least($2,$3) and high_user=greatest($2,$3) for share")
            .bind(actor.event_id.uuid()).bind(actor.user_id.uuid()).bind(id).fetch_one(&mut *tx).await.map_err(|_|unavailable())?;
        if row.get::<i64, _>("revision") != expected {
            return Err(Status::aborted("Social revision changed"));
        }
        let value = json!({"target_user_id":id,"blocked_by_me":row.get::<bool,_>("blocked"),"revision":expected});
        hooks
            .succeeded(
                &mut tx,
                SocialEffect {
                    operation: op.id(),
                    resource: id,
                    revision: expected,
                    projection: true,
                },
            )
            .await
            .map_err(rel)?;
        hooks
            .retain(
                &mut tx,
                SocialReadSources::Metadata { subjects: vec![] },
                None,
            )
            .await?;
        tx.commit().await.map_err(|_| unavailable())?;
        Ok(value)
    }
    async fn list(
        &self,
        actor: ParticipantKey,
        op: SocialOperation,
        page: &Page,
        scope: Arc<OwnerGuardScope>,
        hooks: Arc<RuntimeHooks>,
    ) -> Result<Value, Status> {
        let after = cursor::read(
            &self.keys,
            actor.user_id.uuid(),
            actor.event_id.uuid(),
            op.id(),
            page,
        )?;
        let (mut tx, policy) = self.begin(actor, scope, &hooks).await?;
        policy
            .authorize_actor_in(&mut tx, actor)
            .await
            .map_err(rel)?;
        let mut items = vec![];
        let mut subjects = vec![];
        let mut until: Option<DateTime<Utc>> = None;
        let mut last = None;
        let more = if op == SocialOperation::Requests {
            let direction = page.direction.map(|d| {
                if d == Direction::Incoming {
                    "INCOMING"
                } else {
                    "OUTGOING"
                }
            });
            let rows=sqlx::query("select request_id,event_id,requester_id,recipient_id,intro_id,intro_revision,state,revision,expires_at,created_at from concert_social_requests where event_id=$1 and (requester_id=$2 or recipient_id=$2) and ($3::text is null or ($3='INCOMING' and recipient_id=$2) or ($3='OUTGOING' and requester_id=$2)) and ($4::text is null or (case when state in ('PENDING','APPROVED') and expires_at<=clock_timestamp() then 'EXPIRED' else state end)=$4) and request_id>$5 order by request_id limit $6 for share")
                .bind(actor.event_id.uuid()).bind(actor.user_id.uuid()).bind(direction).bind(&page.state).bind(after).bind(i64::from(page.limit)+1).fetch_all(&mut *tx).await.map_err(|_|unavailable())?;
            let more = rows.len() > usize::from(page.limit);
            for row in rows.iter().take(usize::from(page.limit)) {
                last = Some(row.get::<Uuid, _>("request_id"));
                match policy
                    .authorize_in(
                        &mut tx,
                        query(actor, peer(row, actor.user_id.uuid()), Field::PublicProfile),
                        None,
                    )
                    .await
                {
                    Ok(_) => {
                        let (value, expiry) = request_value(&mut tx, row).await?;
                        items.push(value);
                        subjects.push(peer(row, actor.user_id.uuid()));
                        if let Some(expiry) = expiry {
                            until = Some(until.map_or(expiry, |old| old.min(expiry)));
                        }
                    }
                    Err(RelationshipError::Denied) => {}
                    Err(e) => return Err(rel(e)),
                }
            }
            more
        } else {
            let rows=sqlx::query("select case when low_user=$2 then high_user else low_user end as subject,revision from concert_relationship_pairs where event_id=$1 and ((low_user=$2 and low_blocks_high) or(high_user=$2 and high_blocks_low)) and (case when low_user=$2 then high_user else low_user end)>$3 order by subject limit $4 for share")
                .bind(actor.event_id.uuid()).bind(actor.user_id.uuid()).bind(after).bind(i64::from(page.limit)+1).fetch_all(&mut *tx).await.map_err(|_|unavailable())?;
            let more = rows.len() > usize::from(page.limit);
            for row in rows.iter().take(usize::from(page.limit)) {
                let id = row.get::<Uuid, _>("subject");
                last = Some(id);
                items.push(json!({"target_user_id":id,"blocked_by_me":true,"revision":row.get::<i64,_>("revision")}));
            }
            more
        };
        let next = if more {
            Some(cursor::write(
                &self.keys,
                actor.user_id.uuid(),
                actor.event_id.uuid(),
                op.id(),
                page,
                last.ok_or_else(unavailable)?,
            )?)
        } else {
            None
        };
        let value = json!({"items":items,"next_cursor":next});
        hooks
            .succeeded(
                &mut tx,
                SocialEffect {
                    operation: op.id(),
                    resource: actor.user_id.uuid(),
                    revision: 0,
                    projection: true,
                },
            )
            .await
            .map_err(rel)?;
        hooks
            .retain(&mut tx, SocialReadSources::Metadata { subjects }, until)
            .await?;
        tx.commit().await.map_err(|_| unavailable())?;
        Ok(value)
    }
}
#[async_trait::async_trait]
impl ScopedProductProducer for SocialRuntime {
    async fn dispatch(
        &self,
        context: &VerifiedProduct,
        scope: Arc<OwnerGuardScope>,
        request: proto::ProductRequest,
    ) -> Result<ProductRepresentation, Status> {
        match self.execute(context, scope, request).await {
            Ok(value) => Ok(value),
            Err(error) => {
                let (status, reason) = response_error(&error);
                representation(
                    status,
                    json!({"code":format!("CONCERT_{status}"),"message":"The requested action is unavailable.","traceId":context.request(),"timestamp":timestamp(Utc::now()),"details":{"reason":reason}}),
                )
            }
        }
    }
}
struct Verifier(Arc<SocialClaimProvenance>);
#[async_trait::async_trait]
impl SharingVerifierStatus for Verifier {
    async fn current_in(
        &self,
        c: &mut PgConnection,
        request: Context,
    ) -> Result<VerifierStatus, RelationshipError> {
        self.0.current_in(c, request).await
    }
}
/// Service-internal read projections retain their child before the service
/// commits. Command hooks only record effects; their separately authorized
/// post-command projection retains the child after the write transaction ends.
struct RuntimeHooks {
    witness: Arc<SocialWitness>,
    write_gate: Arc<ReadOnlyGate>,
    command_transaction: bool,
    scope: Arc<OwnerGuardScope>,
    pool: PgPool,
    operation: SocialOperation,
    retained: Option<SocialReadSources>,
}
impl RuntimeHooks {
    fn for_projection(&self) -> Arc<Self> {
        Arc::new(Self {
            witness: self.witness.clone(),
            write_gate: self.write_gate.clone(),
            command_transaction: false,
            scope: self.scope.clone(),
            pool: self.pool.clone(),
            operation: self.operation,
            retained: self.retained.clone(),
        })
    }
    fn command(&self, error: CommandError) -> Status {
        command_rejection(error, &self.write_gate)
    }
    async fn retain(
        &self,
        c: &mut PgConnection,
        sources: SocialReadSources,
        until: Option<DateTime<Utc>>,
    ) -> Result<(), Status> {
        retain_projected_in(&self.pool, &self.scope, c, self.operation, sources, until).await
    }
}
#[async_trait::async_trait]
impl SocialTransactionHooks for RuntimeHooks {
    async fn register(&self, c: &mut PgConnection) -> Result<(), RelationshipError> {
        self.witness.register(c).await?;
        if self.command_transaction && normal_mutation(self.operation) {
            let binding = self.scope.binding();
            let milliseconds = binding
                .deadline()
                .remaining(
                    std::time::Duration::from_secs(5),
                    binding.deadline().expires_micros(),
                )
                .map_err(|_| RelationshipError::Unavailable)?
                .as_millis()
                .max(1)
                .to_string();
            sqlx::query(
                "select set_config('statement_timeout',$1,true),set_config('lock_timeout',$1,true)",
            )
            .bind(milliseconds)
            .execute(&mut *c)
            .await
            .map_err(|_| RelationshipError::Unavailable)?;
            let policy = PgRelationshipPolicy::new(
                self.pool.clone(),
                GuardedAuthority::new(self.scope.clone()),
            );
            self.write_gate
                .check_in(
                    c,
                    &policy,
                    ParticipantKey {
                        event_id: EventId::from_uuid(binding.event()),
                        user_id: AuthUserId::from_uuid(binding.actor()),
                    },
                    self.operation,
                )
                .await?;
        }
        Ok(())
    }
    async fn succeeded(
        &self,
        c: &mut PgConnection,
        effect: SocialEffect,
    ) -> Result<(), RelationshipError> {
        self.witness.succeeded(c, effect).await?;
        if effect.projection {
            if let Some(sources) = self.retained.clone() {
                self.retain(c, sources, None)
                    .await
                    .map_err(|_| RelationshipError::Unavailable)?;
            }
        }
        Ok(())
    }
}
struct ObservedBlock<'a> {
    inner: BlockCommand<'a, GuardedAuthority>,
    hooks: Arc<RuntimeHooks>,
}
#[async_trait::async_trait]
impl CommandProducer for ObservedBlock<'_> {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        self.hooks
            .register(c)
            .await
            .map_err(|_| CommandError::Unavailable)?;
        self.inner.authorize(c, s, target).await
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        let result = self.inner.apply(c, s, target).await?;
        let Completion::Succeeded { reference, .. } = result.0 else {
            return Err(CommandError::Unavailable);
        };
        self.hooks
            .succeeded(
                c,
                SocialEffect {
                    operation: s.operation.id(),
                    resource: reference.resource,
                    revision: reference.revision,
                    projection: false,
                },
            )
            .await
            .map_err(|_| CommandError::Unavailable)?;
        Ok(result)
    }
    async fn completed(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
        r: &SafeReceipt,
    ) -> Result<(), CommandError> {
        let reference = r.reference.ok_or(CommandError::Unavailable)?;
        if r.scope != s || r.state != ReceiptState::Succeeded || reference.resource != target {
            return Err(CommandError::Unavailable);
        }
        self.hooks
            .succeeded(
                c,
                SocialEffect {
                    operation: s.operation.id(),
                    resource: target,
                    revision: reference.revision,
                    projection: true,
                },
            )
            .await
            .map_err(|_| CommandError::Unavailable)
    }
}
fn query(actor: ParticipantKey, subject: Uuid, field: Field) -> PolicyQuery {
    PolicyQuery {
        event: actor.event_id,
        actor: actor.user_id,
        subject: AuthUserId::from_uuid(subject),
        field,
    }
}
fn peer(row: &sqlx::postgres::PgRow, actor: Uuid) -> Uuid {
    let requester = row.get("requester_id");
    if requester == actor {
        row.get("recipient_id")
    } else {
        requester
    }
}
fn metadata_field(operation: SocialOperation) -> Field {
    if matches!(operation, SocialOperation::Request(_)) {
        Field::PublicProfile
    } else {
        Field::Interaction
    }
}
async fn request_value(
    c: &mut PgConnection,
    row: &sqlx::postgres::PgRow,
) -> Result<(Value, Option<DateTime<Utc>>), Status> {
    let id: Uuid = row.get("request_id");
    let event: Uuid = row.get("event_id");
    let rows=sqlx::query("select platform,requester_version,recipient_version,requester_consents,approved from concert_social_bindings where event_id=$1 and request_id=$2 order by platform for share").bind(event).bind(id).fetch_all(&mut *c).await.map_err(|_|unavailable())?;
    if !(1..=2).contains(&rows.len()) {
        return Err(unavailable());
    }
    let consents:Vec<_>=rows.iter().map(|r|json!({"platform":r.get::<String,_>("platform"),"requester_handle_version":r.get::<i64,_>("requester_version"),"recipient_handle_version":r.get::<i64,_>("recipient_version"),"requester_consents":r.get::<bool,_>("requester_consents")})).collect();
    let approved: Vec<String> = rows
        .iter()
        .filter(|r| r.get::<bool, _>("approved"))
        .map(|r| r.get("platform"))
        .collect();
    let expires: DateTime<Utc> = row.get("expires_at");
    let state: String = row.get("state");
    let expired: bool = sqlx::query_scalar("select clock_timestamp()>=$1")
        .bind(expires)
        .fetch_one(&mut *c)
        .await
        .map_err(|_| unavailable())?;
    let active = matches!(state.as_str(), "PENDING" | "APPROVED");
    Ok((
        json!({"request_id":id,"event_id":event,"requester_id":row.get::<Uuid,_>("requester_id"),"recipient_id":row.get::<Uuid,_>("recipient_id"),"intro_id":row.get::<Uuid,_>("intro_id"),"intro_revision":row.get::<i64,_>("intro_revision"),"state":if expired&&active{"EXPIRED"}else{&state},"revision":row.get::<i64,_>("revision"),"expires_at":timestamp(expires),"created_at":timestamp(row.get("created_at")),"consents":consents,"approved_platforms":approved}),
        (active && !expired).then_some(expires),
    ))
}
// Private local discriminator; never infer this reason from a generic owner denial.
const READ_ONLY_ERROR: &[u8] = b"mcr.social.event-read-only.v1";
fn normal_mutation(operation: SocialOperation) -> bool {
    matches!(
        operation,
        SocialOperation::SetHandle(_) | SocialOperation::Create | SocialOperation::Decide(_)
    )
}
#[derive(Default)]
struct ReadOnlyGate(AtomicBool);
impl ReadOnlyGate {
    fn rejected(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
    /// Called only after native registration, inside the actual command transaction.
    /// Event UPDATE precedes all participant/pair locks. This check grants no target
    /// or operator authority; every successful path still runs the retained producer.
    async fn check_in<A: FencedRelationshipAuthority>(
        &self,
        c: &mut PgConnection,
        policy: &PgRelationshipPolicy<A>,
        actor: ParticipantKey,
        operation: SocialOperation,
    ) -> Result<(), RelationshipError> {
        if !normal_mutation(operation) {
            return Ok(());
        }
        let phase: String =
            sqlx::query_scalar("select phase from concert_events where event_id=$1 for update")
                .bind(actor.event_id.uuid())
                .fetch_optional(&mut *c)
                .await
                .map_err(|_| RelationshipError::Unavailable)?
                .ok_or(RelationshipError::Denied)?;
        // Do not expose the phase reason to nonmembers, revoked actors or an
        // unavailable canonical owner. This is current actor eligibility only.
        policy.authorize_actor_in(c, actor).await?;
        if phase == "READ_ONLY" {
            self.0.store(true, Ordering::Release);
            return Err(RelationshipError::Denied);
        }
        Ok(())
    }
}
fn command_rejection(error: CommandError, gate: &ReadOnlyGate) -> Status {
    if matches!(error, CommandError::Denied) && gate.rejected() {
        Status::with_details(
            tonic::Code::FailedPrecondition,
            "Event is read-only",
            READ_ONLY_ERROR.into(),
        )
    } else {
        command(error)
    }
}
fn response_error(error: &Status) -> (u32, &'static str) {
    match error.code() {
        tonic::Code::InvalidArgument => (400, "INVALID_INPUT"),
        tonic::Code::PermissionDenied | tonic::Code::NotFound => (404, "RESOURCE_UNAVAILABLE"),
        tonic::Code::Aborted => (409, "VERSION_CONFLICT"),
        tonic::Code::AlreadyExists => (409, "IDEMPOTENCY_CONFLICT"),
        tonic::Code::FailedPrecondition if error.details() == READ_ONLY_ERROR => {
            (409, "EVENT_READ_ONLY")
        }
        tonic::Code::FailedPrecondition => (409, "INVALID_TRANSITION"),
        tonic::Code::OutOfRange => (409, "IDEMPOTENCY_EXPIRED"),
        tonic::Code::ResourceExhausted => (429, "RATE_LIMITED"),
        tonic::Code::DeadlineExceeded => (504, "DEADLINE_EXCEEDED"),
        _ => (503, "DEPENDENCY_UNAVAILABLE"),
    }
}
fn rel(error: RelationshipError) -> Status {
    match error {
        RelationshipError::Denied => Status::not_found("Social resource unavailable"),
        RelationshipError::Stale => Status::aborted("Social revision changed"),
        RelationshipError::Unavailable => unavailable(),
    }
}
fn command(error: CommandError) -> Status {
    match error {
        CommandError::Denied => Status::not_found("Social resource unavailable"),
        CommandError::InvalidInput => Status::invalid_argument("Invalid social input"),
        CommandError::Conflict => Status::already_exists("Social idempotency conflict"),
        CommandError::InvalidState => Status::failed_precondition("Social transition unavailable"),
        CommandError::Expired => Status::out_of_range("Social idempotency expired"),
        CommandError::Capacity => Status::resource_exhausted("Social command capacity"),
        _ => unavailable(),
    }
}
fn unavailable() -> Status {
    Status::unavailable("Social dependency unavailable")
}
fn timestamp(value: DateTime<Utc>) -> String {
    value.to_rfc3339_opts(SecondsFormat::Millis, true)
}
fn representation(status: u32, value: Value) -> Result<ProductRepresentation, Status> {
    Ok(ProductRepresentation {
        status,
        content_type: "application/json".into(),
        body: serde_json::to_vec(&value).map_err(|_| unavailable())?,
    })
}

#[cfg(test)]
#[path = "../../tests/social_live_support/write_policy.rs"]
mod write_policy_tests;
