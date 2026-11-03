use super::*;

#[derive(Clone, Copy, Serialize, Deserialize)]
#[serde(tag = "decision", deny_unknown_fields)]
enum SourceState {
    Available {
        visibility: Option<SourceVisibility>,
    },
    Suppressed,
}
impl SourceState {
    fn validate(self, binding: &NotificationBinding) -> Result<(), Denial> {
        match self {
            Self::Available { visibility } => binding.visibility(visibility),
            // Only created source lifecycle may suppress without Friend policy.
            Self::Suppressed => binding.visibility(Some(SourceVisibility::Public)),
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SourceEvidence {
    v: u8,
    kid: String,
    iss: String,
    aud: String,
    purpose: String,
    method: String,
    challenge: Uuid,
    binding_digest: String,
    body_digest: String,
    source: SourceState,
    iat: i64,
    exp: i64,
}
#[derive(Clone)]
pub struct NotificationOwnerSigner {
    key: Key,
    policy: LifetimePolicy,
}
impl NotificationOwnerSigner {
    pub fn new(seed: &[u8; 32], policy: LifetimePolicy) -> anyhow::Result<Self> {
        Ok(Self {
            key: Key::new(seed)?,
            policy,
        })
    }
    pub fn public_key(&self) -> [u8; 32] {
        self.key.public_key()
    }
    /// Owning service must first independently resolve actual persisted source,
    /// provenance, scope and current created-source lifecycle/visibility.
    pub fn attest(
        &self,
        binding: &NotificationBinding,
        body: &[u8],
        challenge: Uuid,
        visibility: Option<SourceVisibility>,
        checked_at: i64,
        now: i64,
    ) -> Result<String, Denial> {
        self.attest_source(
            binding,
            body,
            challenge,
            SourceState::Available { visibility },
            checked_at,
            now,
        )
    }
    /// Only a successful owning created-source lifecycle observation may call
    /// this. Storage failure and invalid ownership must remain errors.
    pub fn attest_suppressed(
        &self,
        binding: &NotificationBinding,
        body: &[u8],
        challenge: Uuid,
        checked_at: i64,
        now: i64,
    ) -> Result<String, Denial> {
        self.attest_source(
            binding,
            body,
            challenge,
            SourceState::Suppressed,
            checked_at,
            now,
        )
    }
    fn attest_source(
        &self,
        binding: &NotificationBinding,
        body: &[u8],
        challenge: Uuid,
        source: SourceState,
        checked_at: i64,
        now: i64,
    ) -> Result<String, Denial> {
        binding.validate_wire(body)?;
        source.validate(binding)?;
        if challenge.is_nil() {
            return Err(Denial::WrongBody);
        }
        let exp = binding
            .claim()
            .lease_expires_micros
            .min(checked_at.saturating_add(self.policy.0));
        valid_time(checked_at, exp, now, binding.claim().lease_expires_micros)?;
        self.key.sign(
            OWNER_CONTEXT,
            &SourceEvidence {
                v: 1,
                kid: self.key.id.clone(),
                iss: "post".into(),
                aud: "gateway".into(),
                purpose: "notification_source".into(),
                method: binding.method().into(),
                challenge,
                binding_digest: body_digest(&binding.encode()?),
                body_digest: body_digest(body),
                source,
                iat: checked_at,
                exp,
            },
        )
    }
}

pub struct PreparedNotification {
    binding: NotificationBinding,
    body: Vec<u8>,
    challenge: Uuid,
    prepared_at: i64,
    deadline_micros: i64,
}
impl PreparedNotification {
    pub fn binding(&self) -> &NotificationBinding {
        &self.binding
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }
    pub fn challenge(&self) -> Uuid {
        self.challenge
    }
    pub fn expires_micros(&self) -> i64 {
        self.deadline_micros
    }
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Grant {
    v: u8,
    kid: String,
    iss: String,
    pres: String,
    aud: String,
    purpose: String,
    method: String,
    binding: NotificationBinding,
    body_digest: String,
    visibility: Option<SourceVisibility>,
    iat: i64,
    exp: i64,
    jti: Uuid,
}
pub struct IssuedNotification {
    pub token: String,
    pub expires_micros: i64,
}
/// Suppressed is an acknowledged no-write result, not a transport failure.
pub enum NotificationIssue {
    Granted(IssuedNotification),
    Suppressed { expires_micros: i64 },
}
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum NotificationDecision {
    Allowed {
        visibility: Option<SourceVisibility>,
    },
    Suppressed,
}

/// Authenticated admission response data, not a locally minted capability.
/// The transport adapter must carry the minimum refreshed evidence deadline.
#[derive(Clone, Copy)]
pub struct NotificationAdmission {
    pub decision: NotificationDecision,
    pub expires_micros: i64,
}
impl NotificationAdmission {
    /// Validate a closed authenticated response against the exact requested
    /// purpose and effective deadline; shorter refreshed authority is valid.
    pub fn validate_for(
        self,
        binding: &NotificationBinding,
        deadline_micros: i64,
        now: i64,
    ) -> Result<Self, Denial> {
        if self.expires_micros > deadline_micros.min(binding.claim().lease_expires_micros)
            || now >= self.expires_micros
        {
            return Err(Denial::Expired);
        }
        if let NotificationDecision::Allowed { visibility } = self.decision {
            binding.visibility(visibility)?;
        }
        Ok(self)
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Membership {
    v: u8,
    kid: String,
    iss: String,
    aud: String,
    purpose: String,
    method: String,
    page: NotificationPage,
    members: Vec<Uuid>,
    next_cursor: Option<String>,
    iat: i64,
    exp: i64,
}
fn page_members(
    page: &NotificationPage,
    members: &[Uuid],
    next: Option<&str>,
) -> Result<(), Denial> {
    page.validate()?;
    let mut seen = std::collections::HashSet::new();
    if members.len() > page.limit as usize
        || members
            .iter()
            .any(|id| id.is_nil() || *id == page.author_id || !seen.insert(*id))
    {
        return Err(Denial::WrongBody);
    }
    if let Some(next) = cursor(next)? {
        if members.last() != Some(&next.1)
            || cursor(page.cursor.as_deref())?.is_some_and(|start| next >= start)
        {
            return Err(Denial::WrongBody);
        }
    }
    Ok(())
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyQuery {
    v: u8,
    kid: String,
    iss: String,
    pres: String,
    aud: String,
    purpose: String,
    method: String,
    event_id: Uuid,
    attempt_id: Uuid,
    recipient_id: Uuid,
    actor_id: Uuid,
    binding_digest: String,
    body_digest: String,
    visibility: Option<SourceVisibility>,
    nonce: Uuid,
    iat: i64,
    exp: i64,
    lease_expires_micros: i64,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PolicyEvidence {
    v: u8,
    kid: String,
    iss: String,
    aud: String,
    purpose: String,
    method: String,
    query_digest: String,
    allowed: bool,
    iat: i64,
    exp: i64,
}
/// Sealed result of verifying Gateway's dedicated policy query. The Friend
/// adapter must derive subjects from this, not separate caller-supplied UUIDs.
pub struct VerifiedNotificationPolicy {
    query: PolicyQuery,
    token: String,
}
impl VerifiedNotificationPolicy {
    pub fn recipient_id(&self) -> Uuid {
        self.query.recipient_id
    }
    pub fn actor_id(&self) -> Uuid {
        self.query.actor_id
    }
    pub fn requires_following(&self) -> bool {
        self.query.visibility == Some(SourceVisibility::Friends)
    }
    pub fn expires_micros(&self) -> i64 {
        self.query.exp
    }
}
pub struct ReadyNotificationPolicy {
    prepared: PreparedNotification,
    visibility: Option<SourceVisibility>,
    query: String,
    query_iat: i64,
    exp: i64,
}
impl ReadyNotificationPolicy {
    pub fn token(&self) -> &str {
        &self.query
    }
    pub fn expires_micros(&self) -> i64 {
        self.exp
    }
}

/// Sealed verified policy response plus a new owning-source challenge. Neither
/// issuance nor admission may finish using the pre-policy owner observation.
pub struct FinalNotificationPolicy {
    ready: ReadyNotificationPolicy,
    allowed: bool,
    policy_exp: i64,
    owner_request: PreparedNotification,
}
impl FinalNotificationPolicy {
    pub fn owner_request(&self) -> &PreparedNotification {
        &self.owner_request
    }
}

/// Friend owns both signatures, but their contexts and payloads cannot cross.
#[derive(Clone)]
pub struct NotificationFriendSigner {
    key: Key,
    gateways: Keys,
    policy: LifetimePolicy,
}
pub struct IssuedNotificationMembership {
    pub token: String,
    pub expires_micros: i64,
}
impl NotificationFriendSigner {
    pub fn new(
        seed: &[u8; 32],
        gateway_keys: &[[u8; 32]],
        policy: LifetimePolicy,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            key: Key::new(seed)?,
            gateways: Keys::new(gateway_keys)?,
            policy,
        })
    }
    pub fn public_key(&self) -> [u8; 32] {
        self.key.public_key()
    }
    /// Only a page admitted by the fresh-owner guard may originate membership.
    pub fn membership(
        &self,
        admitted: &AdmittedNotification,
        members: Vec<Uuid>,
        next_cursor: Option<String>,
        now: i64,
    ) -> Result<String, Denial> {
        self.membership_observed(admitted, members, next_cursor, now, now)
            .map(|issued| issued.token)
    }
    /// Actual page adapters capture checked_at before awaiting their owning
    /// query. Signing later must not re-date an older selected-page observation.
    pub fn membership_observed(
        &self,
        admitted: &AdmittedNotification,
        members: Vec<Uuid>,
        next_cursor: Option<String>,
        checked_at: i64,
        now: i64,
    ) -> Result<IssuedNotificationMembership, Denial> {
        let NotificationBinding::FollowerPage(page) = admitted.binding() else {
            return Err(Denial::WrongMethod);
        };
        if !matches!(admitted.decision(), NotificationDecision::Allowed { .. }) {
            return Err(Denial::WrongBody);
        }
        page_members(page, &members, next_cursor.as_deref())?;
        let exp = admitted
            .expires_micros()
            .min(checked_at.saturating_add(self.policy.0));
        valid_time(checked_at, exp, now, page.claim.lease_expires_micros)?;
        let token = self.key.sign(
            MEMBERSHIP_CONTEXT,
            &Membership {
                v: 1,
                kid: self.key.id.clone(),
                iss: "friend".into(),
                aud: "gateway".into(),
                purpose: "notification_selected_membership".into(),
                method: PAGE_METHOD.into(),
                page: page.clone(),
                members,
                next_cursor,
                iat: checked_at,
                exp,
            },
        )?;
        if token.len() > MAX_WITNESS {
            return Err(Denial::BodyTooLarge);
        }
        Ok(IssuedNotificationMembership {
            token,
            expires_micros: exp,
        })
    }
    pub fn verify_policy_query(
        &self,
        token: &str,
        presenter: Workload,
        method: &str,
        now: i64,
    ) -> Result<VerifiedNotificationPolicy, Denial> {
        if presenter != Workload::Gateway {
            return Err(Denial::WrongIssuer);
        }
        let q: PolicyQuery = self.gateways.verify(POLICY_QUERY_CONTEXT, token)?;
        if q.v != 1 || q.iss != "gateway" || q.pres != "gateway" {
            return Err(Denial::WrongIssuer);
        }
        if q.aud != "friend" {
            return Err(Denial::WrongAudience);
        }
        if q.purpose != "notification_recipient_policy"
            || q.method != POLICY_METHOD
            || method != POLICY_METHOD
        {
            return Err(Denial::WrongMethod);
        }
        if [
            q.event_id,
            q.attempt_id,
            q.recipient_id,
            q.actor_id,
            q.nonce,
        ]
        .iter()
        .any(Uuid::is_nil)
            || q.recipient_id == q.actor_id
        {
            return Err(Denial::WrongBody);
        }
        decode_digest(&q.binding_digest)?;
        decode_digest(&q.body_digest)?;
        valid_time(q.iat, q.exp, now, q.lease_expires_micros)?;
        Ok(VerifiedNotificationPolicy {
            query: q,
            token: token.into(),
        })
    }
    /// Friend must sample its actual owning relationship port before calling.
    /// blocked=true combines either direction; muted is recipient->actor only.
    pub fn policy(
        &self,
        query: VerifiedNotificationPolicy,
        blocked: bool,
        muted: bool,
        following: bool,
        checked_at: i64,
        now: i64,
    ) -> Result<String, Denial> {
        let q = &query.query;
        if checked_at.saturating_add(CLOCK_SKEW_MICROS) < q.iat {
            return Err(Denial::NotYetValid);
        }
        let exp = q.exp.min(checked_at.saturating_add(self.policy.0));
        valid_time(checked_at, exp, now, q.lease_expires_micros)?;
        self.key.sign(
            POLICY_CONTEXT,
            &PolicyEvidence {
                v: 1,
                kid: self.key.id.clone(),
                iss: "friend".into(),
                aud: "gateway".into(),
                purpose: "notification_current_policy".into(),
                method: POLICY_METHOD.into(),
                query_digest: body_digest(query.token.as_bytes()),
                allowed: !blocked && !muted && (!query.requires_following() || following),
                iat: checked_at,
                exp,
            },
        )
    }
}

#[derive(Clone)]
pub struct NotificationMembershipVerifier {
    keys: Keys,
}
impl NotificationMembershipVerifier {
    pub fn new(keys: &[[u8; 32]]) -> anyhow::Result<Self> {
        Ok(Self {
            keys: Keys::new(keys)?,
        })
    }
    fn witness(&self, token: &str, now: i64) -> Result<Membership, Denial> {
        if token.len() > MAX_WITNESS {
            return Err(Denial::BodyTooLarge);
        }
        let w: Membership = self.keys.verify(MEMBERSHIP_CONTEXT, token)?;
        if w.v != 1 || w.iss != "friend" {
            return Err(Denial::WrongIssuer);
        }
        if w.aud != "gateway" {
            return Err(Denial::WrongAudience);
        }
        if w.method != PAGE_METHOD || w.purpose != "notification_selected_membership" {
            return Err(Denial::WrongMethod);
        }
        page_members(&w.page, &w.members, w.next_cursor.as_deref())?;
        valid_time(w.iat, w.exp, now, w.page.claim.lease_expires_micros)?;
        Ok(w)
    }
    pub fn verify_page(
        &self,
        page: &NotificationPage,
        members: &[Uuid],
        next_cursor: Option<&str>,
        token: &str,
        now: i64,
    ) -> Result<(), Denial> {
        self.verify_page_expiry(page, members, next_cursor, token, now)
            .map(|_| ())
    }
    pub fn verify_page_expiry(
        &self,
        page: &NotificationPage,
        members: &[Uuid],
        next_cursor: Option<&str>,
        token: &str,
        now: i64,
    ) -> Result<i64, Denial> {
        let w = self.witness(token, now)?;
        if w.page != *page || w.members != members || w.next_cursor.as_deref() != next_cursor {
            return Err(Denial::WrongBody);
        }
        Ok(w.exp)
    }
    fn verify_recipient(&self, d: &NotificationDelivery, now: i64) -> Result<Option<i64>, Denial> {
        if d.kind != NotificationKind::Created {
            if d.membership.is_some() {
                return Err(Denial::WrongBody);
            }
            return Ok(None);
        }
        let w = self.witness(d.membership.as_deref().ok_or(Denial::Missing)?, now)?;
        if w.page.claim != d.claim
            || w.page.author_id != d.actor_id
            || !w.members.contains(&d.recipient_id)
        {
            return Err(Denial::WrongBody);
        }
        Ok(Some(w.exp))
    }
}

#[derive(Clone)]
pub struct NotificationIssuer {
    key: Key,
    owners: Keys,
    friends: Keys,
    membership: NotificationMembershipVerifier,
    policy: LifetimePolicy,
}
impl NotificationIssuer {
    pub fn new(
        seed: &[u8; 32],
        owner_keys: &[[u8; 32]],
        friend_keys: &[[u8; 32]],
        policy: LifetimePolicy,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            key: Key::new(seed)?,
            owners: Keys::new(owner_keys)?,
            friends: Keys::new(friend_keys)?,
            membership: NotificationMembershipVerifier::new(friend_keys)?,
            policy,
        })
    }
    pub fn public_key(&self) -> [u8; 32] {
        self.key.public_key()
    }
    pub fn prepare(
        &self,
        presenter: Workload,
        binding: NotificationBinding,
        body: Vec<u8>,
        now: i64,
    ) -> Result<PreparedNotification, Denial> {
        let deadline = binding.claim().lease_expires_micros;
        self.prepare_until(presenter, binding, body, deadline, now)
    }
    /// The original effect/admission budget also caps signed child authority.
    /// A caller-side timeout alone cannot prevent a grant outliving that budget.
    pub fn prepare_until(
        &self,
        presenter: Workload,
        binding: NotificationBinding,
        body: Vec<u8>,
        deadline_micros: i64,
        now: i64,
    ) -> Result<PreparedNotification, Denial> {
        if presenter != Workload::Post {
            return Err(Denial::WrongIssuer);
        }
        binding.validate_wire(&body)?;
        binding.encode()?;
        if deadline_micros > binding.claim().lease_expires_micros || now >= deadline_micros {
            return Err(Denial::Expired);
        }
        Ok(PreparedNotification {
            binding,
            body,
            challenge: Uuid::new_v4(),
            prepared_at: now,
            deadline_micros,
        })
    }
    fn source(
        &self,
        p: &PreparedNotification,
        evidence: &str,
        now: i64,
    ) -> Result<(SourceState, i64), Denial> {
        let e: SourceEvidence = self.owners.verify(OWNER_CONTEXT, evidence)?;
        if e.v != 1 || e.iss != "post" || e.aud != "gateway" {
            return Err(Denial::WrongIssuer);
        }
        if e.purpose != "notification_source" || e.method != p.binding.method() {
            return Err(Denial::WrongMethod);
        }
        if e.challenge != p.challenge
            || e.binding_digest != body_digest(&p.binding.encode()?)
            || e.body_digest != body_digest(&p.body)
        {
            return Err(Denial::WrongBody);
        }
        e.source.validate(&p.binding)?;
        if e.iat.saturating_add(CLOCK_SKEW_MICROS) < p.prepared_at {
            return Err(Denial::NotYetValid);
        }
        valid_time(e.iat, e.exp, now, p.binding.claim().lease_expires_micros)?;
        if now >= p.deadline_micros {
            return Err(Denial::Expired);
        }
        Ok((e.source, e.exp.min(p.deadline_micros)))
    }
    fn owner(
        &self,
        p: &PreparedNotification,
        evidence: &str,
        now: i64,
    ) -> Result<(Option<SourceVisibility>, i64), Denial> {
        let (source, exp) = self.source(p, evidence, now)?;
        match source {
            SourceState::Available { visibility } => Ok((visibility, exp)),
            SourceState::Suppressed => Err(Denial::WrongBody),
        }
    }
    /// Verify exact challenged owning evidence before skipping Friend policy.
    /// A suppressed source cannot mint an allowed page or delivery grant.
    pub fn source_suppression(
        &self,
        p: &PreparedNotification,
        evidence: &str,
        now: i64,
    ) -> Result<Option<NotificationAdmission>, Denial> {
        let (source, exp) = self.source(p, evidence, now)?;
        let exp = exp
            .min(now.saturating_add(self.policy.0))
            .min(p.deadline_micros);
        Ok(
            matches!(source, SourceState::Suppressed).then_some(NotificationAdmission {
                decision: NotificationDecision::Suppressed,
                expires_micros: exp,
            }),
        )
    }
    /// Refresh a live original proof with newly challenged source suppression.
    /// Exact proof binding/body and both deadlines still apply.
    pub fn refresh_source_suppression(
        &self,
        proof: &VerifiedNotification,
        p: &PreparedNotification,
        evidence: &str,
        now: i64,
    ) -> Result<Option<NotificationAdmission>, Denial> {
        if proof.binding() != &p.binding || proof.body() != p.body || now >= proof.expires_micros()
        {
            return Err(Denial::WrongBody);
        }
        Ok(self
            .source_suppression(p, evidence, now)?
            .map(|mut admission| {
                admission.expires_micros = admission.expires_micros.min(proof.expires_micros());
                admission
            }))
    }
    fn issue(
        &self,
        p: PreparedNotification,
        visibility: Option<SourceVisibility>,
        exp: i64,
        now: i64,
    ) -> Result<IssuedNotification, Denial> {
        let exp = exp
            .min(now.saturating_add(self.policy.0))
            .min(p.deadline_micros);
        valid_time(now, exp, now, p.binding.claim().lease_expires_micros)?;
        let token = self.key.sign(
            GRANT_CONTEXT,
            &Grant {
                v: 1,
                kid: self.key.id.clone(),
                iss: "gateway".into(),
                pres: "post".into(),
                aud: workload_name(p.binding.audience()).into(),
                purpose: "claimed_notification".into(),
                method: p.binding.method().into(),
                binding: p.binding,
                body_digest: body_digest(&p.body),
                visibility,
                iat: now,
                exp,
                jti: Uuid::new_v4(),
            },
        )?;
        Ok(IssuedNotification {
            token,
            expires_micros: exp,
        })
    }
    pub fn issue_page(
        &self,
        p: PreparedNotification,
        evidence: &str,
        now: i64,
    ) -> Result<IssuedNotification, Denial> {
        if !matches!(p.binding, NotificationBinding::FollowerPage(_)) {
            return Err(Denial::WrongMethod);
        }
        let (visibility, exp) = self.owner(&p, evidence, now)?;
        self.issue(p, visibility, exp, now)
    }
    /// Only verified owning source plus original selected membership can create
    /// a Friend policy query. The query itself never authorizes a write.
    pub fn prepare_policy(
        &self,
        p: PreparedNotification,
        evidence: &str,
        now: i64,
    ) -> Result<ReadyNotificationPolicy, Denial> {
        let NotificationBinding::Delivery(d) = &p.binding else {
            return Err(Denial::WrongMethod);
        };
        let (visibility, exp) = self.owner(&p, evidence, now)?;
        let exp = self
            .membership
            .verify_recipient(d, now)?
            .map_or(exp, |membership_exp| exp.min(membership_exp));
        let exp = exp.min(now.saturating_add(self.policy.0));
        valid_time(now, exp, now, d.claim.lease_expires_micros)?;
        let query = self.key.sign(
            POLICY_QUERY_CONTEXT,
            &PolicyQuery {
                v: 1,
                kid: self.key.id.clone(),
                iss: "gateway".into(),
                pres: "gateway".into(),
                aud: "friend".into(),
                purpose: "notification_recipient_policy".into(),
                method: POLICY_METHOD.into(),
                event_id: d.claim.event_id,
                attempt_id: d.claim.attempt_id,
                recipient_id: d.recipient_id,
                actor_id: d.actor_id,
                binding_digest: body_digest(&p.binding.encode()?),
                body_digest: body_digest(&p.body),
                visibility,
                nonce: Uuid::new_v4(),
                iat: now,
                exp,
                lease_expires_micros: d.claim.lease_expires_micros,
            },
        )?;
        Ok(ReadyNotificationPolicy {
            prepared: p,
            visibility,
            query,
            query_iat: now,
            exp,
        })
    }
    fn policy_result(
        &self,
        ready: &ReadyNotificationPolicy,
        evidence: &str,
        now: i64,
    ) -> Result<(bool, i64), Denial> {
        let e: PolicyEvidence = self.friends.verify(POLICY_CONTEXT, evidence)?;
        if e.v != 1 || e.iss != "friend" || e.aud != "gateway" {
            return Err(Denial::WrongIssuer);
        }
        if e.purpose != "notification_current_policy" || e.method != POLICY_METHOD {
            return Err(Denial::WrongMethod);
        }
        if e.query_digest != body_digest(ready.query.as_bytes())
            || e.iat.saturating_add(CLOCK_SKEW_MICROS) < ready.query_iat
            || e.exp > ready.exp
        {
            return Err(Denial::WrongBody);
        }
        valid_time(
            e.iat,
            e.exp,
            now,
            ready.prepared.binding.claim().lease_expires_micros,
        )?;
        Ok((e.allowed, e.exp))
    }
    /// After awaiting Friend, obtain a newly challenged Post observation. A
    /// visibility change invalidates this policy; the adapter may start again
    /// only within its unchanged original effect deadline and attempt budget.
    pub fn prepare_final_owner(
        &self,
        ready: ReadyNotificationPolicy,
        evidence: &str,
        now: i64,
    ) -> Result<FinalNotificationPolicy, Denial> {
        let (allowed, policy_exp) = self.policy_result(&ready, evidence, now)?;
        let owner_request = self.prepare_until(
            Workload::Post,
            ready.prepared.binding.clone(),
            ready.prepared.body.clone(),
            ready
                .prepared
                .deadline_micros
                .min(ready.exp)
                .min(policy_exp),
            now,
        )?;
        Ok(FinalNotificationPolicy {
            ready,
            allowed,
            policy_exp,
            owner_request,
        })
    }
    fn final_owner(
        &self,
        final_policy: &FinalNotificationPolicy,
        evidence: &str,
        now: i64,
    ) -> Result<(bool, i64), Denial> {
        let (source, owner_exp) = self.source(&final_policy.owner_request, evidence, now)?;
        if matches!(source, SourceState::Available { visibility } if visibility != final_policy.ready.visibility)
        {
            return Err(Denial::WrongBody);
        }
        let mut exp = owner_exp
            .min(final_policy.ready.exp)
            .min(final_policy.policy_exp);
        if let NotificationBinding::Delivery(d) = &final_policy.ready.prepared.binding {
            if let Some(member_exp) = self.membership.verify_recipient(d, now)? {
                exp = exp.min(member_exp);
            }
        }
        if now >= exp {
            return Err(Denial::Expired);
        }
        Ok((matches!(source, SourceState::Available { .. }), exp))
    }
    pub fn issue_delivery(
        &self,
        final_policy: FinalNotificationPolicy,
        owner_evidence: &str,
        now: i64,
    ) -> Result<NotificationIssue, Denial> {
        let (source_allowed, exp) = self.final_owner(&final_policy, owner_evidence, now)?;
        if !source_allowed || !final_policy.allowed {
            return Ok(NotificationIssue::Suppressed {
                expires_micros: exp,
            });
        }
        self.issue(
            final_policy.ready.prepared,
            final_policy.ready.visibility,
            exp,
            now,
        )
        .map(NotificationIssue::Granted)
    }
    pub fn refresh_page(
        &self,
        proof: &VerifiedNotification,
        p: PreparedNotification,
        evidence: &str,
        now: i64,
    ) -> Result<NotificationAdmission, Denial> {
        if proof.binding() != &p.binding
            || proof.body() != p.body
            || !matches!(p.binding, NotificationBinding::FollowerPage(_))
        {
            return Err(Denial::WrongBody);
        }
        let (source, owner_exp) = self.source(&p, evidence, now)?;
        let exp = proof.expires_micros().min(owner_exp);
        if now >= exp {
            return Err(Denial::Expired);
        }
        Ok(NotificationAdmission {
            decision: match source {
                SourceState::Available { visibility } => {
                    NotificationDecision::Allowed { visibility }
                }
                SourceState::Suppressed => NotificationDecision::Suppressed,
            },
            expires_micros: exp,
        })
    }
    pub fn refresh_delivery(
        &self,
        proof: &VerifiedNotification,
        final_policy: FinalNotificationPolicy,
        owner_evidence: &str,
        now: i64,
    ) -> Result<NotificationAdmission, Denial> {
        if proof.binding() != &final_policy.ready.prepared.binding
            || proof.body() != final_policy.ready.prepared.body
        {
            return Err(Denial::WrongBody);
        }
        let (source_allowed, exp) = self.final_owner(&final_policy, owner_evidence, now)?;
        let exp = exp.min(proof.expires_micros());
        if now >= exp {
            return Err(Denial::Expired);
        }
        Ok(NotificationAdmission {
            decision: if source_allowed && final_policy.allowed {
                NotificationDecision::Allowed {
                    visibility: final_policy.ready.visibility,
                }
            } else {
                NotificationDecision::Suppressed
            },
            expires_micros: exp,
        })
    }
}
fn workload_name(w: Workload) -> &'static str {
    match w {
        Workload::Friend => "friend",
        Workload::Notification => "notification",
        _ => "invalid",
    }
}

#[derive(Clone)]
pub struct VerifiedNotification {
    binding: NotificationBinding,
    body: Arc<[u8]>,
    token: Arc<str>,
    exp: i64,
    jti: Uuid,
}
impl VerifiedNotification {
    pub fn binding(&self) -> &NotificationBinding {
        &self.binding
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }
    pub fn token(&self) -> &str {
        &self.token
    }
    pub fn expires_micros(&self) -> i64 {
        self.exp
    }
}
#[derive(Clone)]
pub struct NotificationVerifier {
    keys: Keys,
    used: Arc<Mutex<HashMap<Uuid, i64>>>,
    capacity: usize,
}
impl NotificationVerifier {
    pub fn new(keys: &[[u8; 32]], capacity: usize) -> anyhow::Result<Self> {
        anyhow::ensure!(capacity > 0, "Notification replay capacity required");
        Ok(Self {
            keys: Keys::new(keys)?,
            used: Arc::new(Mutex::new(HashMap::new())),
            capacity,
        })
    }
    pub fn verify(
        &self,
        token: &str,
        presenter: Workload,
        method: &str,
        body: &[u8],
        now: i64,
    ) -> Result<VerifiedNotification, Denial> {
        if presenter != Workload::Post {
            return Err(Denial::WrongIssuer);
        }
        let g: Grant = self.keys.verify(GRANT_CONTEXT, token)?;
        if g.v != 1 || g.iss != "gateway" || g.pres != "post" {
            return Err(Denial::WrongIssuer);
        }
        if g.purpose != "claimed_notification" || g.method != method || method != g.binding.method()
        {
            return Err(Denial::WrongMethod);
        }
        if g.aud != workload_name(g.binding.audience()) {
            return Err(Denial::WrongAudience);
        }
        g.binding.validate_wire(body)?;
        g.binding.visibility(g.visibility)?;
        if g.body_digest != body_digest(body) || g.jti.is_nil() {
            return Err(Denial::WrongBody);
        }
        valid_time(g.iat, g.exp, now, g.binding.claim().lease_expires_micros)?;
        Ok(VerifiedNotification {
            binding: g.binding,
            body: body.into(),
            token: token.into(),
            exp: g.exp,
            jti: g.jti,
        })
    }
    pub(super) fn consume(
        &self,
        p: &VerifiedNotification,
        refreshed_exp: i64,
        mut clock: impl FnMut() -> i64,
    ) -> Result<(), Denial> {
        let mut used = self.used.lock().unwrap_or_else(|p| p.into_inner());
        let now = clock();
        let deadline = p.exp.min(refreshed_exp);
        if now >= deadline {
            return Err(Denial::Expired);
        }
        if used.len() >= self.capacity {
            used.retain(|_, exp| *exp > now);
        }
        if clock() >= deadline {
            return Err(Denial::Expired);
        }
        if used.contains_key(&p.jti) {
            return Err(Denial::Replayed);
        }
        if used.len() >= self.capacity {
            return Err(Denial::ReplayCacheFull);
        }
        used.insert(p.jti, p.exp);
        Ok(())
    }
}

#[cfg(test)]
#[path = "tests.rs"]
mod tests;
