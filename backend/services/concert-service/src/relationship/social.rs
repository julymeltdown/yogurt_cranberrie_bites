//! Private storage/consent/reveal. No public router or default authority is registered.
use super::{crypto::*, *};
use crate::adapters::out::repository::receipt::{CommandProducer, PgCommandRepository};
use concert_application::{commands::*, relationship::social::*};
use concert_domain::{relationship::social::*, AuthUserId, EventId, ParticipantKey, MAX_REVISION};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use uuid::Uuid;

/// Safe outcome identity only. Never include a handle, ciphertext, key ID or
/// handle fingerprint. Projection witnesses attest completed authorization and
/// projection, not a new aggregate mutation.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct SocialEffect {
    pub operation: &'static str,
    pub resource: Uuid,
    pub revision: i64,
    pub projection: bool,
}
/// Trusted runtime observer on the retained service's actual transaction.
/// Registration must complete before policy/effects. A success witness belongs
/// to this same transaction and rolls back with every failed commit.
#[async_trait]
pub trait SocialTransactionHooks: Send + Sync {
    async fn register(&self, c: &mut PgConnection) -> Result<(), RelationshipError>;
    async fn succeeded(
        &self,
        c: &mut PgConnection,
        effect: SocialEffect,
    ) -> Result<(), RelationshipError>;
}

/// Trusted BE27 write adapter only; this does not authorize an introduction edit.
/// Call after its actual Event FOR UPDATE -> Intro FOR UPDATE/mutation, inside
/// PgCommandRepository::execute. That retained producer must return the exact
/// Intro SafeReference/Invalidation: the kernel queues the existing BE15 outbox
/// after this hook, in this same transaction. No extra receipt/queue is created.
pub async fn revoke_intro_consent_in(
    c: &mut PgConnection,
    event: EventId,
    intro_id: Uuid,
    owner: AuthUserId,
    new_revision: Revision,
) -> Result<(), RelationshipError> {
    // Idempotent for the required caller-held root. NOWAIT prevents a misplaced
    // caller from introducing an Intro -> waiting Event lock-order cycle.
    sqlx::query("select event_id from concert_events where event_id=$1 for update nowait")
        .bind(event.uuid())
        .fetch_optional(&mut *c)
        .await
        .map_err(unavailable)?
        .ok_or(RelationshipError::Denied)?;
    let actual: bool = sqlx::query_scalar("select exists(select 1 from concert_intros where event_id=$1 and intro_id=$2 and owner_id=$3 and revision=$4 and public.concert_social_own_visible_xmin(xmin))")
        .bind(event.uuid()).bind(intro_id).bind(owner.uuid()).bind(new_revision.get())
        .fetch_one(&mut *c).await.map_err(unavailable)?;
    let receipt: bool = sqlx::query_scalar("select exists(select 1 from concert_command_receipts where actor_id=$1 and event_id=$2 and target_id=$3 and operation_id in ('setConcertIntro','publishConcertIntro','closeConcertIntro') and state='PENDING' and xmin::text=mod(pg_current_xact_id()::text::numeric,4294967296)::text)")
        .bind(owner.uuid()).bind(event.uuid()).bind(intro_id).fetch_one(&mut *c).await.map_err(unavailable)?;
    if !actual || !receipt {
        return Err(RelationshipError::Unavailable);
    }
    // The event root excludes every retained Social write/read before these
    // request/grant locks. There is no pair lock or independent transaction.
    let ids: Vec<Uuid> = sqlx::query_scalar("select request_id from concert_social_requests where event_id=$1 and intro_id=$2 and recipient_id=$3 and intro_revision<$4 order by request_id for update")
        .bind(event.uuid()).bind(intro_id).bind(owner.uuid()).bind(new_revision.get()).fetch_all(&mut *c).await.map_err(unavailable)?;
    for id in ids {
        sqlx::query("update concert_social_grants set state='REVOKED',revision=revision+1 where event_id=$1 and request_id=$2 and state='ACTIVE'")
            .bind(event.uuid()).bind(id).execute(&mut *c).await.map_err(unavailable)?;
        sqlx::query("update concert_social_requests set state='REVOKED',revision=revision+1 where event_id=$1 and request_id=$2 and state in ('PENDING','APPROVED')")
            .bind(event.uuid()).bind(id).execute(&mut *c).await.map_err(unavailable)?;
    }
    Ok(())
}

/// Current intro/claim eligibility, bound to exact users/event/intro revision and fenced
/// through commit. No client claim or profile existence can implement this port.
#[async_trait]
pub trait SocialEligibility: Send + Sync {
    async fn check_in(
        &self,
        c: &mut PgConnection,
        context: SharingContext,
        now: DateTime<Utc>,
    ) -> Result<FreshSharing, RelationshipError>;
}
pub struct UnavailableSocialEligibility;
#[async_trait]
impl SocialEligibility for UnavailableSocialEligibility {
    async fn check_in(
        &self,
        _: &mut PgConnection,
        _: SharingContext,
        _: DateTime<Utc>,
    ) -> Result<FreshSharing, RelationshipError> {
        Err(RelationshipError::Unavailable)
    }
}
pub struct SocialService<A, E> {
    pool: PgPool,
    policy: PgRelationshipPolicy<A>,
    eligibility: E,
    keys: HandleKeys,
    ttl_seconds: i64,
    hooks: Option<Arc<dyn SocialTransactionHooks>>,
}
impl<A: FencedRelationshipAuthority, E: SocialEligibility> SocialService<A, E> {
    pub fn new(
        pool: PgPool,
        authority: A,
        eligibility: E,
        keys: HandleKeys,
        ttl_seconds: i64,
    ) -> Result<Self, RelationshipError> {
        if !(1..=86400).contains(&ttl_seconds) {
            return Err(RelationshipError::Unavailable);
        }
        Ok(Self {
            policy: PgRelationshipPolicy::new(pool.clone(), authority),
            pool,
            eligibility,
            keys,
            ttl_seconds,
            hooks: None,
        })
    }
    pub fn with_transaction_hooks(mut self, hooks: Arc<dyn SocialTransactionHooks>) -> Self {
        self.hooks = Some(hooks);
        self
    }
    async fn register_transaction(&self, c: &mut PgConnection) -> Result<(), RelationshipError> {
        if let Some(hooks) = &self.hooks {
            hooks.register(c).await?;
        }
        Ok(())
    }
    async fn record_effect(
        &self,
        c: &mut PgConnection,
        effect: SocialEffect,
    ) -> Result<(), RelationshipError> {
        if let Some(hooks) = &self.hooks {
            hooks.succeeded(c, effect).await?;
        }
        Ok(())
    }
    /// Owner-only metadata. No ciphertext, key identifiers or plaintext in this result.
    pub async fn own_handles(
        &self,
        actor: ParticipantKey,
    ) -> Result<Vec<HandleState>, RelationshipError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        self.register_transaction(&mut tx).await?;
        let query = PolicyQuery {
            event: actor.event_id,
            actor: actor.user_id,
            subject: actor.user_id,
            field: Field::PublicProfile,
        };
        self.policy.authorize_in(&mut tx, query, None).await?;
        let rows = sqlx::query("select platform,revision,ciphertext is not null as configured from concert_social_handles where event_id=$1 and user_id=$2 order by platform for share")
            .bind(actor.event_id.uuid()).bind(actor.user_id.uuid()).fetch_all(&mut *tx).await.map_err(unavailable)?;
        let result = [Platform::Instagram, Platform::X]
            .into_iter()
            .map(|platform| {
                let row = rows
                    .iter()
                    .find(|r| r.get::<String, _>("platform") == platform.as_str());
                HandleState {
                    event: actor.event_id,
                    owner: actor.user_id,
                    platform,
                    version: row.map_or(0, |r| r.get("revision")),
                    configured: row.is_some_and(|r| r.get("configured")),
                }
            })
            .collect();
        self.record_effect(
            &mut tx,
            SocialEffect {
                operation: "listOwnConcertHandles",
                resource: actor.user_id.uuid(),
                revision: 0,
                projection: true,
            },
        )
        .await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(result)
    }
    pub async fn set_handle(
        &self,
        actor: ParticipantKey,
        platform: Platform,
        expected: i64,
        handle: PrivateHandle,
        key: Uuid,
        fingerprints: &FingerprintKeys,
    ) -> Result<SafeReceipt, CommandError> {
        if !(0..MAX_REVISION).contains(&expected) {
            return Err(CommandError::InvalidInput);
        }
        let handle =
            PrivateHandle::normalize(platform, handle.expose_for_reveal()).map_err(error)?;
        self.command(
            actor,
            actor.user_id.uuid(),
            key,
            Action::SetHandle {
                platform,
                expected,
                handle,
            },
            fingerprints,
        )
        .await
    }
    pub async fn remove_handle(
        &self,
        actor: ParticipantKey,
        platform: Platform,
        expected: Revision,
        key: Uuid,
        fingerprints: &FingerprintKeys,
    ) -> Result<SafeReceipt, CommandError> {
        self.command(
            actor,
            actor.user_id.uuid(),
            key,
            Action::RemoveHandle { platform, expected },
            fingerprints,
        )
        .await
    }
    pub async fn create(
        &self,
        actor: ParticipantKey,
        input: CreateSharing,
        key: Uuid,
        fingerprints: &FingerprintKeys,
    ) -> Result<SafeReceipt, CommandError> {
        input.validate().map_err(error)?;
        if actor.event_id != input.context.event || actor.user_id != input.context.requester {
            return Err(CommandError::Denied);
        }
        let id = request_id(actor, key);
        self.command(actor, id, key, Action::Create(input), fingerprints)
            .await
    }
    pub async fn decide(
        &self,
        actor: ParticipantKey,
        id: Uuid,
        input: SharingDecision,
        key: Uuid,
        fingerprints: &FingerprintKeys,
    ) -> Result<SafeReceipt, CommandError> {
        input.validate().map_err(error)?;
        self.command(actor, id, key, Action::Decide(input), fingerprints)
            .await
    }
    pub async fn revoke(
        &self,
        actor: ParticipantKey,
        id: Uuid,
        expected: Revision,
        key: Uuid,
        fingerprints: &FingerprintKeys,
    ) -> Result<SafeReceipt, CommandError> {
        self.command(actor, id, key, Action::Revoke(expected), fingerprints)
            .await
    }
    pub async fn cancel(
        &self,
        actor: ParticipantKey,
        id: Uuid,
        expected: Revision,
        key: Uuid,
        fingerprints: &FingerprintKeys,
    ) -> Result<SafeReceipt, CommandError> {
        self.command(actor, id, key, Action::Cancel(expected), fingerprints)
            .await
    }
    async fn command(
        &self,
        actor: ParticipantKey,
        target: Uuid,
        key: Uuid,
        action: Action,
        fingerprints: &FingerprintKeys,
    ) -> Result<SafeReceipt, CommandError> {
        let operation = RegisteredOperation::find(action.operation())?;
        let scope = Scope {
            actor: actor.user_id.uuid(),
            event: actor.event_id.uuid(),
            operation,
            key,
        };
        let event = scope.event.to_string();
        let target_text = target.to_string();
        let mut parameters = vec![("eventId", event.as_str())];
        if let Some(platform) = action.platform() {
            parameters.push(("platform", platform.as_str()))
        } else {
            parameters.push(("requestId", target_text.as_str()))
        }
        // Creation has no requestId path parameter.
        if matches!(action, Action::Create(_)) {
            parameters.truncate(1)
        }
        let body = action.body();
        let query = if matches!(action, Action::RemoveHandle { .. } | Action::Cancel(_)) {
            vec![("expected_revision", action.expected().to_string())]
        } else {
            vec![]
        };
        let borrowed: Vec<_> = query.iter().map(|(k, v)| (*k, v.as_str())).collect();
        let request = CanonicalRequest::json(scope, &parameters, &borrowed, body.as_ref())?;
        PgCommandRepository::new(self.pool.clone())
            .execute(
                &request,
                target,
                fingerprints,
                &Producer {
                    service: self,
                    action,
                },
            )
            .await
    }
    async fn check_sharing(
        &self,
        c: &mut PgConnection,
        context: SharingContext,
        now: DateTime<Utc>,
    ) -> Result<FreshSharing, CommandError> {
        let proof = self
            .eligibility
            .check_in(c, context, now)
            .await
            .map_err(error)?;
        let finished: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(db)?;
        if proof.context != context || proof.checked_at != now || proof.expires_at <= finished {
            return Err(CommandError::Unavailable);
        }
        if !proof.allowed {
            return Err(CommandError::Denied);
        }
        pin_deadline(c, proof.expires_at).await.map_err(error)?;
        Ok(proof)
    }
    /// Explicit user-triggered, no-store fetch only. Caller routes must disable prefetch/retry
    /// and serialize only this dedicated output; ordinary handle/request DTOs have no secret.
    pub async fn reveal(
        &self,
        actor: ParticipantKey,
        id: Uuid,
        platform: Platform,
    ) -> Result<PrivateReveal, RelationshipError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        self.register_transaction(&mut tx).await?;
        // Event first; shared lock serializes closure and every social mutation before reads.
        lock_event(&mut tx, actor.event_id, false)
            .await
            .map_err(rel)?;
        let row = read_request(&mut tx, actor.event_id, id)
            .await
            .map_err(rel)?;
        let context = context(&row).map_err(rel)?;
        let peer = peer(actor, context).map_err(rel)?;
        let q = PolicyQuery {
            event: actor.event_id,
            actor: actor.user_id,
            subject: peer,
            // Frozen reveal is READ, including READ_ONLY. This relationship
            // read never grants private access; reciprocal checks below do.
            field: Field::PublicProfile,
        };
        let decision = self.policy.authorize_in(&mut tx, q, None).await?;
        let sharing = self
            .check_sharing(&mut tx, context, decision.checked_at())
            .await
            .map_err(rel)?;
        if row.get::<String, _>("state") != "APPROVED"
            || row.get::<DateTime<Utc>, _>("expires_at") <= decision.checked_at()
            || (row.get::<i64, _>("relationship_revision")
                != decision.stamp().relationship_revision.get()
                || row.get::<i64, _>("authority_revision")
                    != decision.stamp().authority_revision.get())
        {
            return Err(RelationshipError::Denied);
        }
        let binding=sqlx::query("select * from concert_social_bindings where event_id=$1 and request_id=$2 and platform=$3 and approved for share").bind(actor.event_id.uuid()).bind(id).bind(platform.as_str()).fetch_optional(&mut *tx).await.map_err(unavailable)?.ok_or(RelationshipError::Denied)?;
        let req_ver = revision(binding.get("requester_version"))?;
        let rec_ver = revision(binding.get("recipient_version"))?;
        let grants=sqlx::query("select * from concert_social_grants where event_id=$1 and request_id=$2 and platform=$3 order by owner_id for share").bind(actor.event_id.uuid()).bind(id).bind(platform.as_str()).fetch_all(&mut *tx).await.map_err(unavailable)?;
        if grants.len() != 2 {
            return Err(RelationshipError::Denied);
        }
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(unavailable)?;
        let expires = row.get::<DateTime<Utc>, _>("expires_at");
        if now >= expires || now >= decision.valid_until() || now >= sharing.expires_at {
            return Err(RelationshipError::Denied);
        }
        for (owner, viewer, version, peer_version) in [
            (context.requester, context.recipient, req_ver, rec_ver),
            (context.recipient, context.requester, rec_ver, req_ver),
        ] {
            let g = grants
                .iter()
                .find(|g| {
                    g.get::<Uuid, _>("owner_id") == owner.uuid()
                        && g.get::<Uuid, _>("viewer_id") == viewer.uuid()
                })
                .ok_or(RelationshipError::Denied)?;
            if g.get::<String, _>("state") != "ACTIVE"
                || g.get::<i64, _>("handle_version") != version.get()
                || g.get::<i64, _>("peer_handle_version") != peer_version.get()
                || g.get::<DateTime<Utc>, _>("expires_at") != expires
            {
                return Err(RelationshipError::Denied);
            }
            check_handle(&mut tx, context.event, owner, platform, version)
                .await
                .map_err(rel)?;
        }
        let version = if peer == context.requester {
            req_ver
        } else {
            rec_ver
        };
        let h=sqlx::query("select key_id,nonce,ciphertext from concert_social_handles where event_id=$1 and user_id=$2 and platform=$3 for share").bind(actor.event_id.uuid()).bind(peer.uuid()).bind(platform.as_str()).fetch_one(&mut *tx).await.map_err(unavailable)?;
        let handle_context = HandleContext {
            event: actor.event_id,
            owner: peer,
            platform,
            version,
        };
        let handle = self.keys.decrypt(
            handle_context,
            EncryptedHandle {
                key_id: h.get("key_id"),
                nonce: h.get("nonce"),
                ciphertext: h.get("ciphertext"),
            },
        )?;
        let grant = grants
            .iter()
            .find(|g| g.get::<Uuid, _>("owner_id") == peer.uuid())
            .ok_or(RelationshipError::Denied)?;
        let output = PrivateReveal {
            request_id: id,
            context: handle_context,
            handle,
            grant_revision: revision(grant.get("revision"))?,
            expires_at: expires,
        };
        let ready: bool = sqlx::query_scalar("select clock_timestamp() < nullif(current_setting('mcr.relationship_authority_until',true),'')::timestamptz and clock_timestamp() < $1")
            .bind(expires).fetch_one(&mut *tx).await.map_err(unavailable)?;
        if !ready {
            return Err(RelationshipError::Denied);
        }
        self.record_effect(
            &mut tx,
            SocialEffect {
                operation: "revealConcertSocialHandle",
                resource: id,
                revision: output.grant_revision.get(),
                projection: true,
            },
        )
        .await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(output)
    }
}
fn error(e: RelationshipError) -> CommandError {
    match e {
        RelationshipError::Denied => CommandError::Denied,
        RelationshipError::Unavailable => CommandError::Unavailable,
        RelationshipError::Stale => CommandError::Conflict,
    }
}
fn rel(e: CommandError) -> RelationshipError {
    match e {
        CommandError::Denied => RelationshipError::Denied,
        CommandError::Conflict => RelationshipError::Stale,
        _ => RelationshipError::Unavailable,
    }
}
fn db(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
fn revision(n: i64) -> Result<Revision, RelationshipError> {
    Revision::new(n).map_err(|_| RelationshipError::Unavailable)
}
fn request_id(actor: ParticipantKey, key: Uuid) -> Uuid {
    let mut h = Sha256::new();
    h.update(b"MCR-social-request-v1");
    h.update(actor.event_id.uuid().as_bytes());
    h.update(actor.user_id.uuid().as_bytes());
    h.update(key.as_bytes());
    let digest = h.finalize();
    let mut out = [0; 16];
    out.copy_from_slice(&digest[..16]);
    Uuid::from_bytes(out)
}
async fn lock_event(c: &mut PgConnection, event: EventId, write: bool) -> Result<(), CommandError> {
    let sql = if write {
        "select event_id from concert_events where event_id=$1 for update"
    } else {
        "select event_id from concert_events where event_id=$1 for share"
    };
    if sqlx::query(sql)
        .bind(event.uuid())
        .fetch_optional(c)
        .await
        .map_err(db)?
        .is_none()
    {
        return Err(CommandError::Denied);
    }
    Ok(())
}
async fn read_request(
    c: &mut PgConnection,
    event: EventId,
    id: Uuid,
) -> Result<sqlx::postgres::PgRow, CommandError> {
    sqlx::query(
        "select * from concert_social_requests where event_id=$1 and request_id=$2 for update",
    )
    .bind(event.uuid())
    .bind(id)
    .fetch_optional(c)
    .await
    .map_err(db)?
    .ok_or(CommandError::Denied)
}
fn context(r: &sqlx::postgres::PgRow) -> Result<SharingContext, CommandError> {
    Ok(SharingContext {
        event: EventId::from_uuid(r.try_get("event_id").map_err(db)?),
        requester: AuthUserId::from_uuid(r.try_get("requester_id").map_err(db)?),
        recipient: AuthUserId::from_uuid(r.try_get("recipient_id").map_err(db)?),
        intro_id: r.try_get("intro_id").map_err(db)?,
        intro_revision: Revision::new(r.try_get("intro_revision").map_err(db)?)
            .map_err(|_| CommandError::InvalidState)?,
    })
}
fn peer(actor: ParticipantKey, c: SharingContext) -> Result<AuthUserId, CommandError> {
    if actor.event_id != c.event {
        return Err(CommandError::Denied);
    }
    if actor.user_id == c.requester {
        Ok(c.recipient)
    } else if actor.user_id == c.recipient {
        Ok(c.requester)
    } else {
        Err(CommandError::Denied)
    }
}
async fn check_handle(
    c: &mut PgConnection,
    event: EventId,
    user: AuthUserId,
    platform: Platform,
    version: Revision,
) -> Result<(), CommandError> {
    let n:Option<i64>=sqlx::query_scalar("select revision from concert_social_handles where event_id=$1 and user_id=$2 and platform=$3 and ciphertext is not null for share").bind(event.uuid()).bind(user.uuid()).bind(platform.as_str()).fetch_optional(c).await.map_err(db)?;
    if n != Some(version.get()) {
        return Err(CommandError::Denied);
    }
    Ok(())
}

enum Action {
    SetHandle {
        platform: Platform,
        expected: i64,
        handle: PrivateHandle,
    },
    RemoveHandle {
        platform: Platform,
        expected: Revision,
    },
    Create(CreateSharing),
    Decide(SharingDecision),
    Revoke(Revision),
    Cancel(Revision),
}
impl Action {
    fn operation(&self) -> &'static str {
        match self {
            Self::SetHandle { .. } => "setConcertHandle",
            Self::RemoveHandle { .. } => "removeConcertHandle",
            Self::Create(_) => "createConcertSocialRequest",
            Self::Decide(_) => "decideConcertSocialRequest",
            Self::Revoke(_) => "revokeConcertSocialRequest",
            Self::Cancel(_) => "cancelConcertSocialRequest",
        }
    }
    fn platform(&self) -> Option<Platform> {
        match self {
            Self::SetHandle { platform, .. } | Self::RemoveHandle { platform, .. } => {
                Some(*platform)
            }
            _ => None,
        }
    }
    fn expected(&self) -> i64 {
        match self {
            Self::SetHandle { expected, .. } => *expected,
            Self::RemoveHandle { expected, .. }
            | Self::Revoke(expected)
            | Self::Cancel(expected) => expected.get(),
            Self::Decide(d) => d.expected.get(),
            _ => 0,
        }
    }
    fn body(&self) -> Option<CanonicalValue> {
        fn text(s: &str) -> CanonicalValue {
            CanonicalValue::String(s.to_owned())
        }
        fn obj(p: Vec<(&str, CanonicalValue)>) -> CanonicalValue {
            CanonicalValue::Object(p.into_iter().map(|(k, v)| (k.to_owned(), v)).collect())
        }
        match self {
            Self::SetHandle {
                expected, handle, ..
            } => Some(obj(vec![
                ("expected_revision", CanonicalValue::Integer(*expected)),
                ("handle", text(handle.expose_for_reveal())),
            ])),
            Self::RemoveHandle { .. } | Self::Cancel(_) => None,
            Self::Create(c) => Some(obj(vec![
                (
                    "recipient_id",
                    text(&c.context.recipient.uuid().to_string()),
                ),
                ("intro_id", text(&c.context.intro_id.to_string())),
                (
                    "intro_revision",
                    CanonicalValue::Integer(c.context.intro_revision.get()),
                ),
                (
                    "consents",
                    CanonicalValue::Array(
                        c.consents
                            .iter()
                            .map(|b| {
                                obj(vec![
                                    ("platform", text(b.platform.as_str())),
                                    (
                                        "requester_handle_version",
                                        CanonicalValue::Integer(b.requester_version.get()),
                                    ),
                                    (
                                        "recipient_handle_version",
                                        CanonicalValue::Integer(b.recipient_version.get()),
                                    ),
                                    (
                                        "requester_consents",
                                        CanonicalValue::Bool(b.requester_consents),
                                    ),
                                ])
                            })
                            .collect(),
                    ),
                ),
            ])),
            Self::Decide(d) => Some(obj(vec![
                (
                    "expected_revision",
                    CanonicalValue::Integer(d.expected.get()),
                ),
                (
                    "decision",
                    text(if d.approve { "APPROVED" } else { "REJECTED" }),
                ),
                (
                    "approved_platforms",
                    CanonicalValue::Array(
                        d.approved_platforms
                            .iter()
                            .map(|p| text(p.as_str()))
                            .collect(),
                    ),
                ),
                (
                    "recipient_consents",
                    CanonicalValue::Bool(d.recipient_consents),
                ),
            ])),
            Self::Revoke(r) => Some(obj(vec![(
                "expected_revision",
                CanonicalValue::Integer(r.get()),
            )])),
        }
    }
}
struct Producer<'a, A, E> {
    service: &'a SocialService<A, E>,
    action: Action,
}
#[async_trait]
impl<A: FencedRelationshipAuthority, E: SocialEligibility> CommandProducer for Producer<'_, A, E> {
    async fn completed(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
        receipt: &SafeReceipt,
    ) -> Result<(), CommandError> {
        if receipt.scope != s || receipt.state != ReceiptState::Succeeded {
            return Err(CommandError::Unavailable);
        }
        let reference = receipt.reference.ok_or(CommandError::InvalidState)?;
        if reference.resource != target || s.operation.id() != self.action.operation() {
            return Err(CommandError::InvalidState);
        }
        // A replay projects an already verified receipt; it does not attest a
        // second aggregate mutation. The hook retains an existing same-TX
        // mutation witness when apply already wrote one.
        self.service
            .record_effect(
                c,
                SocialEffect {
                    operation: self.action.operation(),
                    resource: target,
                    revision: reference.revision,
                    projection: true,
                },
            )
            .await
            .map_err(error)
    }
    async fn authorize(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if s.operation.id() != self.action.operation() {
            return Err(CommandError::Denied);
        }
        self.service.register_transaction(c).await.map_err(error)?;
        lock_event(c, EventId::from_uuid(s.event), true).await?;
        let actor = AuthUserId::from_uuid(s.actor);
        let event = EventId::from_uuid(s.event);
        let (subject, sharing) = match &self.action {
            Action::SetHandle { .. } | Action::RemoveHandle { .. } => {
                if target != s.actor {
                    return Err(CommandError::Denied);
                }
                (actor, None)
            }
            Action::Create(input) => {
                if target
                    != request_id(
                        ParticipantKey {
                            event_id: event,
                            user_id: actor,
                        },
                        s.key,
                    )
                    || input.context.requester != actor
                    || input.context.event != event
                {
                    return Err(CommandError::Denied);
                }
                (input.context.recipient, Some(input.context))
            }
            _ => {
                let r = read_request(c, event, target).await?;
                let ctx = context(&r)?;
                let peer = peer(
                    ParticipantKey {
                        event_id: event,
                        user_id: actor,
                    },
                    ctx,
                )?;
                if matches!(self.action, Action::Decide(_)) && ctx.recipient != actor {
                    return Err(CommandError::Denied);
                }
                if matches!(self.action, Action::Cancel(_)) && ctx.requester != actor {
                    return Err(CommandError::Denied);
                }
                (peer, Some(ctx))
            }
        };
        let safety = matches!(
            self.action,
            Action::RemoveHandle { .. } | Action::Revoke(_) | Action::Cancel(_)
        );
        let q = PolicyQuery {
            event,
            actor,
            subject,
            field: if safety {
                Field::SafetyInteraction
            } else {
                Field::Interaction
            },
        };
        if safety {
            self.service
                .policy
                .safety_authority(c, q)
                .await
                .map_err(error)?;
        } else {
            let decision = self
                .service
                .policy
                .authorize_in(c, q, None)
                .await
                .map_err(error)?;
            if let Some(ctx) = sharing {
                self.service
                    .check_sharing(c, ctx, decision.checked_at())
                    .await?;
            }
        }
        Ok(())
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        s: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        self.authorize(c, s, target).await?;
        let revision = match &self.action {
            Action::SetHandle {
                platform,
                expected,
                handle,
            } => {
                self.change_handle(c, s, *platform, *expected, Some(handle))
                    .await?
            }
            Action::RemoveHandle { platform, expected } => {
                self.change_handle(c, s, *platform, expected.get(), None)
                    .await?
            }
            Action::Create(input) => self.create(c, s, target, input).await?,
            Action::Decide(input) => self.decide(c, s, target, input).await?,
            Action::Revoke(expected) | Action::Cancel(expected) => {
                let r = read_request(c, EventId::from_uuid(s.event), target).await?;
                if r.get::<i64, _>("revision") != expected.get() {
                    return Err(CommandError::Conflict);
                }
                let state = r.get::<String, _>("state");
                let next = if matches!(self.action, Action::Cancel(_)) {
                    if state != "PENDING" {
                        return Err(CommandError::InvalidState);
                    }
                    "CANCELLED"
                } else {
                    if matches!(
                        state.as_str(),
                        "REJECTED" | "CANCELLED" | "REVOKED" | "EXPIRED"
                    ) {
                        return Err(CommandError::InvalidState);
                    }
                    "REVOKED"
                };
                let rev:i64=sqlx::query_scalar("update concert_social_requests set state=$3,revision=revision+1 where event_id=$1 and request_id=$2 returning revision").bind(s.event).bind(target).bind(next).fetch_one(&mut *c).await.map_err(db)?;
                revoke_grants(c, s.event, target).await?;
                rev
            }
        };
        let reference = SafeReference {
            resource: target,
            revision,
        };
        self.service
            .record_effect(
                c,
                SocialEffect {
                    operation: self.action.operation(),
                    resource: target,
                    revision,
                    projection: false,
                },
            )
            .await
            .map_err(error)?;
        Ok((
            Completion::Succeeded {
                reference,
                status: if matches!(self.action, Action::Create(_)) {
                    201
                } else {
                    200
                },
            },
            Some(Invalidation { reference }),
        ))
    }
}
impl<A: FencedRelationshipAuthority, E: SocialEligibility> Producer<'_, A, E> {
    async fn change_handle(
        &self,
        c: &mut PgConnection,
        s: Scope,
        platform: Platform,
        expected: i64,
        handle: Option<&PrivateHandle>,
    ) -> Result<i64, CommandError> {
        let existing:Option<i64>=sqlx::query_scalar("select revision from concert_social_handles where event_id=$1 and user_id=$2 and platform=$3 for update").bind(s.event).bind(s.actor).bind(platform.as_str()).fetch_optional(&mut *c).await.map_err(db)?;
        if existing.unwrap_or(0) != expected || handle.is_none() && existing.is_none() {
            return Err(CommandError::Conflict);
        }
        if handle.is_none() {
            let configured: bool = sqlx::query_scalar("select ciphertext is not null from concert_social_handles where event_id=$1 and user_id=$2 and platform=$3")
                .bind(s.event).bind(s.actor).bind(platform.as_str()).fetch_one(&mut *c).await.map_err(db)?;
            if !configured {
                return Err(CommandError::Denied);
            }
        }
        let next = Revision::new(expected + 1).map_err(|_| CommandError::InvalidState)?;
        let encrypted = handle
            .map(|h| {
                self.service.keys.encrypt(
                    HandleContext {
                        event: EventId::from_uuid(s.event),
                        owner: AuthUserId::from_uuid(s.actor),
                        platform,
                        version: next,
                    },
                    h,
                )
            })
            .transpose()
            .map_err(error)?;
        let key = encrypted.as_ref().map(|e| e.key_id.as_str());
        let nonce = encrypted.as_ref().map(|e| e.nonce.as_slice());
        let cipher = encrypted.as_ref().map(|e| e.ciphertext.as_slice());
        if existing.is_some() {
            sqlx::query("update concert_social_handles set revision=revision+1,key_id=$4,nonce=$5,ciphertext=$6 where event_id=$1 and user_id=$2 and platform=$3").bind(s.event).bind(s.actor).bind(platform.as_str()).bind(key).bind(nonce).bind(cipher).execute(&mut *c).await.map_err(db)?;
        } else {
            sqlx::query("insert into concert_social_handles(event_id,user_id,platform,key_id,nonce,ciphertext) values($1,$2,$3,$4,$5,$6)").bind(s.event).bind(s.actor).bind(platform.as_str()).bind(key).bind(nonce).bind(cipher).execute(&mut *c).await.map_err(db)?;
        }
        // Both pending requests and all granted platforms die on any bound-handle change.
        sqlx::query("update concert_social_grants g set state='REVOKED',revision=revision+1 where g.event_id=$1 and g.state='ACTIVE' and g.request_id in (select r.request_id from concert_social_requests r join concert_social_bindings b using(event_id,request_id) where r.event_id=$1 and (r.requester_id=$2 or r.recipient_id=$2) and b.platform=$3)").bind(s.event).bind(s.actor).bind(platform.as_str()).execute(&mut *c).await.map_err(db)?;
        sqlx::query("update concert_social_requests r set state='REVOKED',revision=revision+1 where r.event_id=$1 and (r.requester_id=$2 or r.recipient_id=$2) and r.state in ('PENDING','APPROVED') and exists(select 1 from concert_social_bindings b where b.event_id=r.event_id and b.request_id=r.request_id and b.platform=$3)").bind(s.event).bind(s.actor).bind(platform.as_str()).execute(&mut *c).await.map_err(db)?;
        Ok(next.get())
    }
    async fn create(
        &self,
        c: &mut PgConnection,
        s: Scope,
        id: Uuid,
        input: &CreateSharing,
    ) -> Result<i64, CommandError> {
        let q = PolicyQuery {
            event: input.context.event,
            actor: input.context.requester,
            subject: input.context.recipient,
            field: Field::Interaction,
        };
        let d = self
            .service
            .policy
            .authorize_in(c, q, None)
            .await
            .map_err(error)?;
        let proof = self
            .service
            .check_sharing(c, input.context, d.checked_at())
            .await?;
        for binding in &input.consents {
            check_handle(
                c,
                input.context.event,
                input.context.requester,
                binding.platform,
                binding.requester_version,
            )
            .await?;
            check_handle(
                c,
                input.context.event,
                input.context.recipient,
                binding.platform,
                binding.recipient_version,
            )
            .await?;
        }
        let expiry = proof
            .expires_at
            .min(d.checked_at() + chrono::Duration::seconds(self.service.ttl_seconds));
        sqlx::query("insert into concert_social_requests(event_id,request_id,requester_id,recipient_id,intro_id,intro_revision,relationship_revision,authority_revision,state,expires_at) values($1,$2,$3,$4,$5,$6,$7,$8,'PENDING',$9)").bind(s.event).bind(id).bind(s.actor).bind(input.context.recipient.uuid()).bind(input.context.intro_id).bind(input.context.intro_revision.get()).bind(d.stamp().relationship_revision.get()).bind(d.stamp().authority_revision.get()).bind(expiry).execute(&mut *c).await.map_err(db)?;
        for b in &input.consents {
            sqlx::query("insert into concert_social_bindings(event_id,request_id,platform,requester_version,recipient_version,requester_consents) values($1,$2,$3,$4,$5,true)").bind(s.event).bind(id).bind(b.platform.as_str()).bind(b.requester_version.get()).bind(b.recipient_version.get()).execute(&mut *c).await.map_err(db)?;
        }
        Ok(1)
    }
    async fn decide(
        &self,
        c: &mut PgConnection,
        s: Scope,
        id: Uuid,
        input: &SharingDecision,
    ) -> Result<i64, CommandError> {
        let r = read_request(c, EventId::from_uuid(s.event), id).await?;
        let ctx = context(&r)?;
        if r.get::<i64, _>("revision") != input.expected.get() {
            return Err(CommandError::Conflict);
        }
        let q = PolicyQuery {
            event: ctx.event,
            actor: ctx.recipient,
            subject: ctx.requester,
            field: Field::Interaction,
        };
        let d = self
            .service
            .policy
            .authorize_in(c, q, None)
            .await
            .map_err(error)?;
        if r.get::<String, _>("state") != "PENDING"
            || r.get::<DateTime<Utc>, _>("expires_at") <= d.checked_at()
            || (r.get::<i64, _>("relationship_revision") != d.stamp().relationship_revision.get()
                || r.get::<i64, _>("authority_revision") != d.stamp().authority_revision.get())
        {
            return Err(CommandError::Denied);
        }
        let bindings=sqlx::query("select * from concert_social_bindings where event_id=$1 and request_id=$2 order by platform for update").bind(s.event).bind(id).fetch_all(&mut *c).await.map_err(db)?;
        for platform in &input.approved_platforms {
            let b = bindings
                .iter()
                .find(|b| b.get::<String, _>("platform") == platform.as_str())
                .ok_or(CommandError::Denied)?;
            if !b.get::<bool, _>("requester_consents") {
                return Err(CommandError::Denied);
            }
            let av = Revision::new(b.get("requester_version"))
                .map_err(|_| CommandError::InvalidState)?;
            let bv = Revision::new(b.get("recipient_version"))
                .map_err(|_| CommandError::InvalidState)?;
            check_handle(c, ctx.event, ctx.requester, *platform, av).await?;
            check_handle(c, ctx.event, ctx.recipient, *platform, bv).await?;
            sqlx::query("update concert_social_bindings set approved=true where event_id=$1 and request_id=$2 and platform=$3").bind(s.event).bind(id).bind(platform.as_str()).execute(&mut *c).await.map_err(db)?;
            for (owner, viewer, version, peer_version) in [
                (ctx.requester, ctx.recipient, av, bv),
                (ctx.recipient, ctx.requester, bv, av),
            ] {
                sqlx::query("insert into concert_social_grants(event_id,request_id,owner_id,viewer_id,platform,handle_version,peer_handle_version,state,expires_at) values($1,$2,$3,$4,$5,$6,$7,'ACTIVE',$8)").bind(s.event).bind(id).bind(owner.uuid()).bind(viewer.uuid()).bind(platform.as_str()).bind(version.get()).bind(peer_version.get()).bind(r.get::<DateTime<Utc>,_>("expires_at")).execute(&mut *c).await.map_err(db)?;
            }
        }
        let rev:i64=sqlx::query_scalar("update concert_social_requests set state=$3,revision=revision+1 where event_id=$1 and request_id=$2 returning revision").bind(s.event).bind(id).bind(if input.approve{"APPROVED"}else{"REJECTED"}).fetch_one(&mut *c).await.map_err(db)?;
        Ok(rev)
    }
}
async fn revoke_grants(c: &mut PgConnection, event: Uuid, id: Uuid) -> Result<(), CommandError> {
    sqlx::query("update concert_social_grants set state='REVOKED',revision=revision+1 where event_id=$1 and request_id=$2 and state='ACTIVE'").bind(event).bind(id).execute(c).await.map_err(db)?;
    Ok(())
}
