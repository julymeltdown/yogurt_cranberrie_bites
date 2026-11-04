use super::super::RevisionVector;
use super::{
    contract::*,
    network::{Owner, OwnerClients},
    proto,
    registry::{ScopeRegistry, UnexposedAcquisition},
    wire,
};
use chrono::{DateTime, Utc};
use concert_domain::relationship::{Field, FreshAuthority, PolicyQuery, RelationshipError};
use platform::actor_delegation::{
    authority_deadline::AuthorityRpcDeadline, relationship_read::RelationshipReadSigner,
};
use ring::rand::{SecureRandom, SystemRandom};
use std::{
    collections::{BTreeMap, BTreeSet},
    future::Future,
    sync::{atomic::Ordering, Arc, Mutex, Weak},
    time::Duration,
};
use tonic::Status;
use uuid::Uuid;
#[derive(Clone)]
pub struct CanonicalOwnerFence {
    clients: OwnerClients,
    registry: Arc<ScopeRegistry>,
}
impl CanonicalOwnerFence {
    pub fn connect(
        auth_endpoint: &str,
        friend_endpoint: &str,
        credential: &str,
        tls: &platform::grpc_tls::TlsMaterial,
        signer: RelationshipReadSigner,
        registry: Arc<ScopeRegistry>,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            clients: OwnerClients::connect(
                auth_endpoint,
                friend_endpoint,
                credential,
                tls,
                signer,
            )?,
            registry,
        })
    }
    pub async fn acquire(&self, binding: GuardBinding) -> Result<Arc<OwnerGuardScope>, Status> {
        binding.check()?;
        let mut root_secret = [0; 32];
        SystemRandom::new()
            .fill(&mut root_secret)
            .map_err(|_| Status::unavailable("Authority entropy unavailable"))?;
        let producer = self.registry.reserve(&binding, &root_secret)?;
        let mut cleanup = UnexposedAcquisition::new(self.registry.clone(), producer.clone());
        let stop = binding.deadline().stop();
        let result = tokio::time::timeout_at(
            stop,
            self.acquire_registered(binding, producer.clone(), root_secret),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("Canonical acquisition deadline"))
        .and_then(|r| r);
        if result.is_err() {
            cleanup.settle().await;
        }
        // No await separates successful construction from disarming: a scope
        // capable of work cannot be mistaken for an unexposed cancellation.
        cleanup.disarm();
        result
    }
    async fn acquire_registered(
        &self,
        binding: GuardBinding,
        producer: proto::ProducerScope,
        root_secret: [u8; 32],
    ) -> Result<Arc<OwnerGuardScope>, Status> {
        self.registry
            .lifecycle
            .register(&producer, &binding)
            .await?;
        binding.check()?;
        let request = proto::AcquireRequest {
            binding: Some(binding.wire([])),
            response_scope: Some(producer.clone()),
        };
        let auth = self
            .clients
            .acquire(Owner::Auth, request.clone(), binding.deadline())
            .await?;
        let friend = self
            .clients
            .acquire(Owner::Friend, request, binding.deadline())
            .await?;
        let vector = validate_vector(RevisionVector {
            auth: wire::revision(auth.revision.as_ref())?,
            friend: wire::revision(friend.revision.as_ref())?,
        })?;
        let auth_view = auth
            .view
            .ok_or_else(|| Status::unavailable("Missing Auth guard"))?;
        let friend_view = friend
            .view
            .ok_or_else(|| Status::unavailable("Missing Friend guard"))?;
        wire::check_view(&auth_view, &binding)?;
        wire::check_view(&friend_view, &binding)?;
        if auth.completion_secret.len() != 32 || friend.completion_secret.len() != 32 {
            return Err(Status::unavailable("Invalid completion capability"));
        }
        let auth_states = states(auth.subjects)?;
        let friend_states = states(friend.subjects)?;
        validate_states(
            &auth_states,
            &friend_states,
            &[binding.actor()].into_iter().collect(),
        )?;
        if !auth.session_live
            || !friend.session_live
            || !auth_states[&binding.actor()].active
            || !friend_states[&binding.actor()].active
        {
            return Err(Status::permission_denied("Actor is not eligible"));
        }
        let scope = Arc::new(OwnerGuardScope {
            binding,
            producer,
            root_secret,
            clients: self.clients.clone(),
            registry: self.registry.weak(),
            vector,
            auth_view,
            friend_view,
            auth_completion: auth.completion_secret,
            friend_completion: friend.completion_secret,
            serial: tokio::sync::Mutex::new(()),
            settlement: tokio::sync::Mutex::new(()),
            state: Mutex::new(State {
                auth: auth_states,
                friend: friend_states,
                subjects: BTreeSet::new(),
                children: Vec::new(),
                completed_children: 0,
                child_intents: BTreeMap::new(),
                attached_children: BTreeSet::new(),
                registered: Vec::new(),
                policy_disclosure_deadline: None,
                owned_work_started: false,
                serialized: false,
                sealed: false,
                complete: false,
            }),
        });
        self.registry.activate(scope.clone())?;
        Ok(scope)
    }
}
// Constructed only after the supervised actual producer future is destroyed.
struct CancelledRootWork(());
struct State {
    auth: BTreeMap<Uuid, proto::SubjectState>,
    friend: BTreeMap<Uuid, proto::SubjectState>,
    subjects: BTreeSet<Uuid>,
    children: Vec<(super::ChildRecoveryReference, Arc<dyn HeldChildGuard>)>,
    completed_children: usize,
    child_intents: BTreeMap<(u8, Uuid), super::ChildRecoveryReference>,
    attached_children: BTreeSet<(u8, Uuid)>,
    registered: Vec<ProducerTransaction>,
    policy_disclosure_deadline: Option<AuthorityRpcDeadline>,
    owned_work_started: bool,
    serialized: bool,
    sealed: bool,
    complete: bool,
}
pub struct OwnerGuardScope {
    binding: GuardBinding,
    producer: proto::ProducerScope,
    root_secret: [u8; 32],
    clients: OwnerClients,
    registry: Weak<ScopeRegistry>,
    vector: RevisionVector,
    auth_view: proto::GuardView,
    friend_view: proto::GuardView,
    auth_completion: Vec<u8>,
    friend_completion: Vec<u8>,
    serial: tokio::sync::Mutex<()>,
    settlement: tokio::sync::Mutex<()>,
    state: Mutex<State>,
}
impl OwnerGuardScope {
    pub fn binding(&self) -> &GuardBinding {
        &self.binding
    }
    /// Only shorten this held response's disclosure window. This does not mint
    /// authority, renew a request, complete a scope or settle any transaction.
    /// Preserve the captured monotonic stop as well as wall expiry so a later
    /// wall-clock rollback cannot restore previously consumed disclosure time.
    pub fn limit_disclosure_until(&self, until: DateTime<Utc>) -> Result<(), Status> {
        self.binding.check()?;
        let original = self.binding.deadline();
        let mut state = self.state.lock().map_err(unavailable)?;
        if state.sealed {
            return Err(Status::permission_denied("Root scope sealed"));
        }
        let previous = state.policy_disclosure_deadline.unwrap_or(original);
        let limit = AuthorityRpcDeadline::within_original(
            until
                .timestamp_micros()
                .min(original.expires_micros())
                .min(previous.expires_micros()),
            original.stop().min(previous.stop()).into_std(),
            MAX_GUARD_DURATION,
        )?;
        state.policy_disclosure_deadline = Some(limit);
        Ok(())
    }
    /// Response-wide disclosure ceiling. This is not a fresh authorization or a
    /// settlement grant. Include every immutable intent (also unknown/lost ACK
    /// children), regardless of which subject the last projection queried.
    pub fn disclosure_ceiling(&self) -> Result<AuthorityRpcDeadline, Status> {
        self.binding.check()?;
        let original = self.binding.deadline();
        let state = self.state.lock().map_err(unavailable)?;
        if state.sealed {
            return Err(Status::permission_denied("Root scope sealed"));
        }
        let expires = state
            .child_intents
            .values()
            .chain(state.children.iter().map(|(reference, _)| reference))
            .fold(
                original
                    .expires_micros()
                    .min(self.auth_view.expires_unix_micros)
                    .min(self.friend_view.expires_unix_micros),
                |until, reference| until.min(reference.content_expires_unix_micros()),
            );
        let policy = state.policy_disclosure_deadline.unwrap_or(original);
        AuthorityRpcDeadline::within_original(
            expires.min(policy.expires_micros()),
            original.stop().min(policy.stop()).into_std(),
            MAX_GUARD_DURATION,
        )
    }
    pub fn revision_vector(&self) -> RevisionVector {
        self.vector
    }
    pub fn producer_scope(&self) -> proto::ProducerScope {
        self.producer.clone()
    }
    pub fn wire_binding(&self) -> proto::GuardBinding {
        self.binding.wire([])
    }
    pub fn auth_view(&self) -> proto::GuardView {
        self.auth_view.clone()
    }
    pub fn friend_view(&self) -> proto::GuardView {
        self.friend_view.clone()
    }
    pub fn root_read_view(&self) -> proto::RootReadView {
        proto::RootReadView {
            scope: Some(self.producer.clone()),
            view_secret: self.root_secret.to_vec(),
            binding: Some(self.wire_binding()),
        }
    }
    pub fn authority_revision(&self) -> Result<i64, Status> {
        self.security_revisions(self.binding.actor()).map(|r| r.0)
    }
    pub(super) fn security_revisions(&self, subject: Uuid) -> Result<(i64, i64), Status> {
        self.binding.check()?;
        let state = self.state.lock().map_err(unavailable)?;
        if state.sealed {
            return Err(Status::permission_denied("Root scope sealed"));
        }
        let actor = state
            .auth
            .get(&self.binding.actor())
            .ok_or_else(|| Status::unavailable("Missing Auth actor state"))?;
        let subject = state
            .auth
            .get(&subject)
            .ok_or_else(|| Status::unavailable("Subject not enrolled"))?;
        if actor.security_revision <= 0 || subject.security_revision <= 0 {
            return Err(Status::unavailable("Missing security generation"));
        }
        Ok((actor.security_revision, subject.security_revision))
    }
    pub async fn public_profile_subject(
        self: &Arc<Self>,
        subject: Uuid,
    ) -> Result<GuardedSubject, Status> {
        let query = PolicyQuery {
            actor: concert_domain::AuthUserId::from_uuid(self.binding.actor()),
            event: concert_domain::EventId::from_uuid(self.binding.event()),
            subject: concert_domain::AuthUserId::from_uuid(subject),
            field: Field::PublicProfile,
        };
        self.check_subject(query).await?;
        Ok(GuardedSubject {
            scope: self.clone(),
            subject,
        })
    }
    async fn refresh(&self, subject: Uuid) -> Result<(), Status> {
        self.binding.check()?;
        if subject.is_nil() {
            return Err(Status::invalid_argument("Invalid subject"));
        }
        let (needed, enrolled) = {
            let mut state = self.state.lock().map_err(unavailable)?;
            if state.sealed {
                return Err(Status::permission_denied("Root scope sealed"));
            }
            if subject != self.binding.actor()
                && !state.subjects.contains(&subject)
                && state.subjects.len() >= MAX_SUBJECTS
            {
                return Err(Status::resource_exhausted("Subject capacity"));
            }
            // Reserve the subject before dispatch: a lost/partial Enroll reply
            // must not make subsequent requests forget an owner-held subject.
            if subject != self.binding.actor() {
                state.subjects.insert(subject);
            }
            let needed = state.auth.len() != state.subjects.len() + 1
                || state.friend.len() != state.subjects.len() + 1;
            (
                needed,
                state
                    .subjects
                    .iter()
                    .map(Uuid::to_string)
                    .collect::<Vec<_>>(),
            )
        };
        let mut replies = Vec::new();
        for (owner, view, expected) in [
            (Owner::Auth, &self.auth_view, self.vector.auth),
            (Owner::Friend, &self.friend_view, self.vector.friend),
        ] {
            let reply = if needed {
                self.clients
                    .enroll(
                        owner,
                        proto::EnrollRequest {
                            view: Some(view.clone()),
                            binding: Some(self.wire_binding()),
                            subject_ids: enrolled.clone(),
                        },
                        self.binding.deadline(),
                    )
                    .await?
            } else {
                self.clients
                    .check(
                        owner,
                        proto::CheckRequest {
                            view: Some(view.clone()),
                            binding: Some(self.wire_binding()),
                        },
                        self.binding.deadline(),
                    )
                    .await?
            };
            if wire::revision(reply.revision.as_ref())? != expected || !reply.session_live {
                return Err(Status::permission_denied(
                    "Owner guard is no longer current",
                ));
            }
            replies.push(states(reply.subjects)?);
        }
        let friend = replies.pop().unwrap();
        let auth = replies.pop().unwrap();
        let mut state = self.state.lock().map_err(unavailable)?;
        let mut expected = state.subjects.clone();
        expected.insert(subject);
        expected.insert(self.binding.actor());
        validate_states(&auth, &friend, &expected)?;
        state.auth = auth;
        state.friend = friend;
        if subject != self.binding.actor() {
            state.subjects.insert(subject);
        }
        Ok(())
    }
    async fn fresh(
        &self,
        query: PolicyQuery,
        now: DateTime<Utc>,
    ) -> Result<FreshAuthority, Status> {
        if query.actor.uuid() != self.binding.actor()
            || query.event.uuid() != self.binding.event()
            || query.field == Field::PrivateHandle
            || (query.field == Field::SafetySelf && query.actor != query.subject)
        {
            return Err(Status::permission_denied("Authority query scope mismatch"));
        }
        let _serial = tokio::time::timeout_at(self.binding.deadline().stop(), self.serial.lock())
            .await
            .map_err(|_| Status::deadline_exceeded("Authority serialization deadline"))?;
        self.refresh(query.subject.uuid()).await?;
        let (mut authority, children) = {
            let state = self.state.lock().map_err(unavailable)?;
            let actor = &state.auth[&self.binding.actor()];
            let subject = &state.auth[&query.subject.uuid()];
            let friend_actor = &state.friend[&self.binding.actor()];
            let friend = &state.friend[&query.subject.uuid()];
            let expires = self
                .binding
                .deadline()
                .expires_micros()
                .min(self.auth_view.expires_unix_micros)
                .min(self.friend_view.expires_unix_micros);
            let until = DateTime::from_timestamp_micros(expires)
                .filter(|until| *until > Utc::now() && *until > now)
                .ok_or_else(|| Status::deadline_exceeded("Authority expired"))?;
            (
                FreshAuthority {
                    query,
                    checked_at: now,
                    valid_until: until,
                    revision: concert_domain::Revision::new(self.vector.auth.revision)
                        .map_err(unavailable)?,
                    actor_allowed: actor.active && friend_actor.active,
                    subject_allowed: subject.active && friend.active,
                    inherited_block: friend.actor_blocks_subject || friend.subject_blocks_actor,
                },
                state
                    .children
                    .iter()
                    .filter(|(reference, _)| {
                        reference.query().subject == query.subject.uuid()
                            || (matches!(
                                reference,
                                super::ChildRecoveryReference::LocalPublic { .. }
                                    | super::ChildRecoveryReference::SocialRead { .. }
                            ) && reference.query().subject == self.binding.actor())
                    })
                    .map(|(reference, child)| (reference.clone(), child.clone()))
                    .collect::<Vec<_>>(),
            )
        };
        // Only check existing children. Acquiring a local subject fence here
        // would recurse through PublicProfileReader.local_subject.
        for (reference, child) in children {
            reference.check_content_time()?;
            let until = DateTime::from_timestamp_micros(reference.content_expires_unix_micros())
                .ok_or_else(|| Status::unavailable("Invalid child disclosure expiry"))?;
            authority.valid_until = authority.valid_until.min(until);
            tokio::time::timeout_at(self.binding.deadline().stop(), child.check())
                .await
                .map_err(|_| Status::deadline_exceeded("Child authority deadline"))??;
            reference.check_content_time()?;
        }
        self.binding.check()?;
        Ok(authority)
    }

    pub async fn check_subject(&self, query: PolicyQuery) -> Result<(), Status> {
        // NotFound is reserved at this API boundary for an actual subject-only
        // denial. An unknown owner view or child handle must fail the whole read,
        // never become a skippable author or a success-shaped empty page.
        let fresh = self.fresh(query, Utc::now()).await.map_err(|status| {
            if status.code() == tonic::Code::NotFound {
                Status::unavailable("Owner or child authority unresolved")
            } else {
                status
            }
        })?;
        if !fresh.actor_allowed {
            return Err(Status::permission_denied("Actor is not eligible"));
        }
        if !matches!(query.field, Field::SafetySelf | Field::SafetyInteraction)
            && (!fresh.subject_allowed || fresh.inherited_block)
        {
            return Err(Status::not_found("Subject is unavailable"));
        }
        Ok(())
    }
    pub async fn register_child_intent(
        &self,
        reference: &super::ChildRecoveryReference,
    ) -> Result<(), Status> {
        self.binding.check()?;
        reference.validate_for(&self.binding)?;
        reference.check_content_time()?;
        if let super::ChildRecoveryReference::NotificationInbox { query, .. } = reference {
            if query.subject != self.binding.actor() {
                return Err(Status::permission_denied(
                    "Notification inbox child is not actor-owned",
                ));
            }
        }
        if let super::ChildRecoveryReference::LocalPublic { native, .. }
        | super::ChildRecoveryReference::GiftPrivate { native, .. }
        | super::ChildRecoveryReference::SocialRead { native, .. } = reference
        {
            if native.producer_instance_id.to_string() != self.producer.instance_id {
                return Err(Status::permission_denied("Child producer mismatch"));
            }
        }
        let _serial = tokio::time::timeout_at(self.binding.deadline().stop(), self.serial.lock())
            .await
            .map_err(|_| Status::deadline_exceeded("Child intent admission deadline"))?;
        let key = reference.key()?;
        {
            let state = self.state.lock().map_err(unavailable)?;
            if state.sealed {
                return Err(Status::permission_denied("Root scope sealed"));
            }
            if !state.auth.contains_key(&reference.query().subject) {
                return Err(Status::permission_denied("Child subject not enrolled"));
            }
            if let Some(old) = state.child_intents.get(&key) {
                return if old == reference {
                    Ok(())
                } else {
                    Err(Status::permission_denied("Child intent rebound"))
                };
            }
            if state.child_intents.len() >= 202 {
                return Err(Status::resource_exhausted("Child intent capacity"));
            }
        }
        let registry = self
            .registry
            .upgrade()
            .ok_or_else(|| Status::unavailable("Root registry unavailable"))?;
        tokio::time::timeout_at(
            self.binding.deadline().stop(),
            registry
                .lifecycle
                .register_child_intent(&self.producer, reference),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("Child intent persistence deadline"))??;
        self.binding.check()?;
        self.state
            .lock()
            .map_err(unavailable)?
            .child_intents
            .insert(key, reference.clone());
        Ok(())
    }
    pub async fn retain_child(&self, child: Arc<dyn HeldChildGuard>) -> Result<(), Status> {
        self.binding.check()?;
        let reference = child.recovery_reference()?;
        reference.validate_for(&self.binding)?;
        reference.check_content_time()?;
        let key = reference.key()?;
        let mut state = self.state.lock().map_err(unavailable)?;
        if state.sealed {
            return Err(Status::permission_denied("Root scope sealed"));
        }
        if state.child_intents.get(&key) != Some(&reference) {
            return Err(Status::permission_denied(
                "Child has no exact pre-dispatch intent",
            ));
        }
        if state.attached_children.contains(&key) {
            return Ok(());
        }
        if state.children.len() >= 202 {
            return Err(Status::resource_exhausted("Child guard capacity"));
        }
        state.children.push((reference, child));
        state.attached_children.insert(key);
        Ok(())
    }
    pub async fn register_transaction_in(
        &self,
        connection: &mut sqlx::PgConnection,
        effect_binding_digest: [u8; 32],
        settlement_reference: Vec<u8>,
    ) -> Result<(), Status> {
        self.binding.check()?;
        let instance = wire::canonical_uuid(&self.producer.instance_id)?;
        tokio::time::timeout_at(self.binding.deadline().stop(), async {
            let identity = ProducerTransaction::capture_concert_in(
                connection,
                effect_binding_digest,
                instance,
                settlement_reference,
            )
            .await?;
            self.track_transaction(identity).await
        })
        .await
        .map_err(|_| Status::deadline_exceeded("Transaction admission deadline"))?
    }
    pub async fn track_transaction(&self, transaction: ProducerTransaction) -> Result<(), Status> {
        self.binding.check()?;
        tokio::time::timeout_at(
            self.binding.deadline().stop(),
            self.track_inner(transaction),
        )
        .await
        .map_err(|_| Status::deadline_exceeded("Transaction registration deadline"))?
    }
    async fn track_inner(&self, transaction: ProducerTransaction) -> Result<(), Status> {
        let _serial = self.serial.lock().await;
        self.binding.check()?;
        if transaction.producer != ProducerKind::Concert
            || transaction.producer_instance_id.to_string() != self.producer.instance_id
        {
            return Err(Status::permission_denied("Transaction producer mismatch"));
        }
        let wire = transaction.wire()?;
        {
            let state = self.state.lock().map_err(unavailable)?;
            if state.sealed {
                return Err(Status::permission_denied("Root scope sealed"));
            }
            // Cache only an EXACT full identity after both owner acknowledgements
            // and durable confirmation. A partial/lost Track is retried through
            // the owners' idempotent registration, never treated as a grant.
            if state
                .registered
                .iter()
                .any(|known| known.wire().is_ok_and(|registered| registered == wire))
            {
                return Ok(());
            }
        }
        let registry = self
            .registry
            .upgrade()
            .ok_or_else(|| Status::unavailable("Root registry unavailable"))?;
        registry
            .lifecycle
            .register_transaction(&self.producer, &wire)
            .await?;
        let mut registrations = Vec::new();
        for (owner, view) in [
            (Owner::Auth, &self.auth_view),
            (Owner::Friend, &self.friend_view),
        ] {
            let response = self
                .clients
                .track(
                    owner,
                    proto::TrackRequest {
                        view: Some(view.clone()),
                        binding: Some(self.wire_binding()),
                        transaction: Some(wire.clone()),
                    },
                    self.binding.deadline(),
                )
                .await?;
            wire::canonical_uuid(&response.registration_id)?;
            registry
                .lifecycle
                .record_tracking(
                    &self.producer,
                    &wire,
                    match owner {
                        Owner::Auth => platform::grpc_auth::Workload::Auth,
                        Owner::Friend => platform::grpc_auth::Workload::Friend,
                    },
                    &response.registration_id,
                )
                .await?;
            registrations.push(response.registration_id);
        }
        registry
            .lifecycle
            .confirm_tracking(&self.producer, &wire, &registrations[0], &registrations[1])
            .await?;
        self.binding.check()?;
        self.state
            .lock()
            .map_err(unavailable)?
            .registered
            .push(transaction);
        Ok(())
    }
    /// Runs request-owned work under the original deadline. Returning NEVER
    /// completes owner guards; actual response-body completion owns that step.
    pub async fn run<T>(
        &self,
        future: impl Future<Output = Result<T, Status>>,
    ) -> Result<T, Status> {
        self.binding.check()?;
        tokio::time::timeout_at(self.binding.deadline().stop(), future)
            .await
            .map_err(|_| Status::deadline_exceeded("Root request deadline"))?
    }
    /// Own the entire post-acquisition producer future, including delivery
    /// publication. No independent task may retain producer work outside it;
    /// remote work must have its durable pre-dispatch registration. Caller Drop
    /// only closes the supervisor channel. The supervisor explicitly destroys
    /// the actual future before obtaining the private cancellation proof.
    pub async fn run_owned<T, F>(self: &Arc<Self>, future: F) -> Result<T, Status>
    where
        T: Send + 'static,
        F: Future<Output = Result<T, Status>> + Send + 'static,
    {
        {
            let mut state = self.state.lock().map_err(unavailable)?;
            if state.owned_work_started || state.sealed || state.serialized {
                return Err(Status::failed_precondition(
                    "Root producer work already owned",
                ));
            }
            state.owned_work_started = true;
        }
        let stop = platform::call_deadline::current()
            .map(tokio::time::Instant::from_std)
            .map_or(self.binding.deadline().stop(), |outer| {
                outer.min(self.binding.deadline().stop())
            });
        let scope = self.clone();
        let (mut sender, receiver) = tokio::sync::oneshot::channel();
        tokio::spawn(async move {
            let mut work = Box::pin(platform::call_deadline::scope(stop.into_std(), future));
            let result = tokio::select! {
                biased;
                _ = sender.closed() => None,
                _ = tokio::time::sleep_until(stop) => {
                    Some(Err(Status::deadline_exceeded("Root producer deadline")))
                }
                result = &mut work => Some(result),
            };
            // This is the actual owned future, not an observation that a caller
            // disappeared. Panics bypass the proof and preserve quarantine.
            drop(work);
            match result {
                Some(Ok(value)) if tokio::time::Instant::now() < stop => {
                    // A successful send transfers response ownership to the
                    // existing serialization/HTTP path. A subsequent lost body
                    // remains quarantined under the existing rules.
                    if let Err(undelivered) = sender.send(Ok(value)) {
                        drop(undelivered);
                        let _ = scope.finish_cancelled(CancelledRootWork(())).await;
                    }
                }
                Some(Ok(expired)) => {
                    drop(expired);
                    let _ = scope.finish_cancelled(CancelledRootWork(())).await;
                    let _ = sender.send(Err(Status::deadline_exceeded("Root producer deadline")));
                }
                failure => {
                    let _ = scope.finish_cancelled(CancelledRootWork(())).await;
                    let error = match failure {
                        Some(Err(error)) => error,
                        _ => Status::cancelled("Root producer caller cancelled"),
                    };
                    let _ = sender.send(Err(error));
                }
            }
        });
        receiver
            .await
            .map_err(|_| Status::unavailable("Root producer supervisor unresolved"))?
    }
    /// Marks only Concert-to-Gateway serialization. It cannot release authority.
    pub async fn mark_serialized(&self) -> Result<(), Status> {
        let _serial = tokio::time::timeout(Duration::from_secs(2), self.serial.lock())
            .await
            .map_err(|_| Status::unavailable("Serialization admission busy"))?;
        if self.state.lock().map_err(unavailable)?.sealed {
            return Err(Status::failed_precondition("Root scope sealed"));
        }
        let registry = self
            .registry
            .upgrade()
            .ok_or_else(|| Status::unavailable("Root registry unavailable"))?;
        tokio::time::timeout(
            Duration::from_secs(2),
            registry.lifecycle.mark_serialized(&self.producer),
        )
        .await
        .map_err(|_| Status::unavailable("Serialization marker unresolved"))??;
        self.state.lock().map_err(unavailable)?.serialized = true;
        Ok(())
    }
    /// Invoked only by the root-owned exact Gateway HTTP terminal capability
    /// handler. Internal gRPC serialization, run-return, deadline and Drop are
    /// never callers of this transition.
    pub async fn finish_response(&self) -> Result<(), Status> {
        self.settle_terminal(false).await
    }
    async fn finish_cancelled(&self, _proof: CancelledRootWork) -> Result<(), Status> {
        self.settle_terminal(true).await
    }
    async fn settle_terminal(&self, cancelled: bool) -> Result<(), Status> {
        let budget = AuthorityRpcDeadline::new(
            Utc::now().timestamp_micros() + 2_000_000,
            Duration::from_secs(2),
        )?;
        let _settlement = tokio::time::timeout_at(budget.stop(), self.settlement.lock())
            .await
            .map_err(|_| Status::unavailable("Scope settlement already running"))?;
        let serial = tokio::time::timeout_at(budget.stop(), self.serial.lock())
            .await
            .map_err(|_| Status::unavailable("Scope settlement busy"))?;
        {
            let mut state = self.state.lock().map_err(unavailable)?;
            if cancelled == state.serialized {
                return Err(Status::failed_precondition("Root terminal proof mismatch"));
            }
            if state.complete {
                return Ok(());
            }
            state.sealed = true;
        }
        let registry = self
            .registry
            .upgrade()
            .ok_or_else(|| Status::unavailable("Root registry unavailable"))?;
        let inflight = registry.seal(&self.producer)?;
        // A previously admitted callback may still need the admission mutex to
        // observe sealed state and return. Never hold it while draining permits.
        drop(serial);
        let settlement = async {
            registry.lifecycle.seal(&self.producer).await?;
            while inflight.load(Ordering::Acquire) != 0 {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
            if cancelled {
                registry
                    .lifecycle
                    .finish_cancelled_root_work(&self.producer)
                    .await?;
            } else {
                registry.lifecycle.finish_root_work(&self.producer).await?;
            }
            for (owner, view, secret) in [
                (Owner::Auth, &self.auth_view, &self.auth_completion),
                (Owner::Friend, &self.friend_view, &self.friend_completion),
            ] {
                let response = self
                    .clients
                    .complete(
                        owner,
                        proto::CompleteRequest {
                            view: Some(view.clone()),
                            binding: Some(self.wire_binding()),
                            completion_secret: secret.clone(),
                        },
                        budget,
                    )
                    .await?;
                if !response.completed {
                    return Err(Status::unavailable("Owner settlement incomplete"));
                }
            }
            let (children, completed) = {
                let state = self.state.lock().map_err(unavailable)?;
                (state.children.clone(), state.completed_children)
            };
            for (reference, child) in children.into_iter().skip(completed) {
                child.complete_settled().await?;
                registry
                    .lifecycle
                    .record_child_completed(&self.producer, &reference)
                    .await?;
                self.state.lock().map_err(unavailable)?.completed_children += 1;
            }
            registry
                .lifecycle
                .settle_child_intents(&self.producer)
                .await?;
            if cancelled {
                registry
                    .lifecycle
                    .record_cancellation_settled(&self.producer)
                    .await?;
            }
            self.state.lock().map_err(unavailable)?.complete = true;
            registry.retire(&self.producer)?;
            Ok(())
        };
        tokio::time::timeout_at(budget.stop(), settlement)
            .await
            .map_err(|_| Status::unavailable("Scope settlement unresolved"))?
    }
}
#[derive(Clone)]
pub struct GuardedAuthority {
    scope: Arc<OwnerGuardScope>,
}
impl GuardedAuthority {
    pub fn new(scope: Arc<OwnerGuardScope>) -> Self {
        Self { scope }
    }
    pub fn scope(&self) -> &Arc<OwnerGuardScope> {
        &self.scope
    }
}
#[async_trait::async_trait]
impl crate::relationship::FencedRelationshipAuthority for GuardedAuthority {
    async fn check_in(
        &self,
        c: &mut sqlx::PgConnection,
        query: PolicyQuery,
        now: DateTime<Utc>,
    ) -> Result<FreshAuthority, RelationshipError> {
        let instance = wire::canonical_uuid(&self.scope.producer.instance_id)
            .map_err(relationship_unavailable)?;
        let actual = ProducerTransaction::capture_concert_in(
            c,
            self.scope.binding.digest(),
            instance,
            Vec::new(),
        )
        .await
        .map_err(relationship_unavailable)?;
        let registered = self
            .scope
            .state
            .lock()
            .map_err(|_| RelationshipError::Unavailable)?
            .registered
            .iter()
            .any(|known| {
                known.system_identifier == actual.system_identifier
                    && known.database_oid == actual.database_oid
                    && known.full_xid == actual.full_xid
                    && known.backend_pid == actual.backend_pid
                    && known.backend_started == actual.backend_started
            });
        if !registered {
            return Err(RelationshipError::Unavailable);
        }
        self.scope
            .fresh(query, now)
            .await
            .map_err(relationship_unavailable)
    }
}
fn states(values: Vec<proto::SubjectState>) -> Result<BTreeMap<Uuid, proto::SubjectState>, Status> {
    let mut states = BTreeMap::new();
    for state in values {
        let id = wire::canonical_uuid(&state.user_id)?;
        if states.insert(id, state).is_some() {
            return Err(Status::unavailable("Duplicate owner subject"));
        }
    }
    if states.len() > 101 {
        return Err(Status::unavailable("Oversized owner subject response"));
    }
    Ok(states)
}
fn validate_states(
    auth: &BTreeMap<Uuid, proto::SubjectState>,
    friend: &BTreeMap<Uuid, proto::SubjectState>,
    expected: &BTreeSet<Uuid>,
) -> Result<(), Status> {
    if auth.keys().copied().collect::<BTreeSet<_>>() != *expected
        || friend.keys().copied().collect::<BTreeSet<_>>() != *expected
        || auth.values().any(|s| {
            s.security_revision < 0
                || (s.active && s.security_revision == 0)
                || s.actor_blocks_subject
                || s.subject_blocks_actor
        })
        || friend.values().any(|s| s.security_revision != 0)
    {
        return Err(Status::unavailable("Incomplete canonical owner states"));
    }
    Ok(())
}
