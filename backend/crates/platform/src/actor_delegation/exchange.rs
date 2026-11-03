//! Gateway-issued, one-hop child authority. Owners keep verification keys only.
//! Direct version-one delegation remains unchanged; child proofs use a distinct
//! signature context and explicitly separate issuer from authenticated presenter.
use super::*;

const CHILD_CONTEXT: &[u8] = b"port-rust actor delegation child v2\n";
const PARENT_CACHE_CAPACITY: usize = 4096;
const MAX_CHILDREN_PER_PARENT: usize = 256;
const MAX_TOKEN_BYTES: usize = 4096;

mod resource_parent;
mod resources;
pub use resource_parent::{ResourceParentScope, VerifiedResourceParent};
pub use resources::{
    ResourceEvidenceSigner, ResourceEvidenceVerifier, RESOURCE_EVIDENCE_SIGNING_SEED_ENV,
    RESOURCE_EVIDENCE_VERIFYING_KEYS_ENV,
};

/// Gateway obtains this evidence independently from authenticated Post. The
/// exchange RPC must never accept resource evidence supplied by its presenter.
pub struct PostAuthor {
    pub post_id: Uuid,
    pub author_id: Uuid,
}
struct ResourceResolution {
    parent_digest: String,
    witness_id: Uuid,
    posts: Vec<PostAuthor>,
    missing_post_ids: Vec<Uuid>,
}
pub enum ResourceLookup<'a> {
    None,
    Posts(&'a [Uuid]),
    /// Owner-proven candidates of this exact signed discovery query.
    DiscoveryCandidates,
}
enum ResourceScope {
    ExactAuthor(Uuid),
    Posts(Vec<Uuid>),
    DiscoveryCandidates,
}
/// Sealed preparation. Resource evidence and a fresh clock reading are
/// required before signing; callers cannot replace actor/body/scope bindings.
pub struct PreparedExchange {
    claims: ChildClaims,
    scope: ResourceScope,
    targets: Vec<Uuid>,
    parent_expiry: i64,
}
impl PreparedExchange {
    pub fn parent_digest(&self) -> &str {
        &self.claims.par
    }
    /// Exact child target authors, for the independently authenticated owner lookup.
    pub fn targets(&self) -> &[Uuid] {
        &self.targets
    }
    pub fn witness_id(&self) -> Option<Uuid> {
        self.claims.wit
    }
    pub fn resource_lookup(&self) -> ResourceLookup<'_> {
        match &self.scope {
            ResourceScope::ExactAuthor(_) => ResourceLookup::None,
            ResourceScope::Posts(ids) => ResourceLookup::Posts(ids),
            ResourceScope::DiscoveryCandidates => ResourceLookup::DiscoveryCandidates,
        }
    }
}

pub struct ParentRequest<'a> {
    pub token: &'a str,
    pub method: &'a str,
    pub body: &'a [u8],
    pub operation: Option<Uuid>,
}

pub struct ChildRequest<'a> {
    pub audience: Workload,
    pub method: &'a str,
    pub body: &'a [u8],
    pub operation: Option<Uuid>,
}

/// Private proof captured only after signature, field binding and replay checks.
/// Raw metadata cannot construct this extension. Never render the token/body.
#[derive(Clone)]
pub struct VerifiedParentAuthority {
    token: Arc<str>,
    method: Arc<str>,
    body: Arc<[u8]>,
    operation: Option<Uuid>,
    actor: Uuid,
    expires_at: i64,
}

tokio::task_local! {
    static PARENT_AUTHORITY: VerifiedParentAuthority;
}

impl VerifiedParentAuthority {
    pub(super) fn new(
        token: String,
        method: String,
        body: Vec<u8>,
        operation: Option<Uuid>,
        actor: Uuid,
        expires_at: i64,
    ) -> Self {
        Self {
            token: token.into(),
            method: method.into(),
            body: body.into(),
            operation,
            actor,
            expires_at,
        }
    }

    pub fn request(&self) -> ParentRequest<'_> {
        ParentRequest {
            token: &self.token,
            method: &self.method,
            body: &self.body,
            operation: self.operation,
        }
    }

    pub fn actor(&self) -> Uuid {
        self.actor
    }

    /// The already admitted uncompressed unary message for typed root mapping.
    pub fn message(&self) -> Result<&[u8], Denial> {
        unary_message(&self.body)
    }

    /// Fingerprints expose no raw authority; storage remains parent-bound.
    pub fn parent_digest(&self) -> [u8; 32] {
        Sha256::digest(self.token.as_bytes()).into()
    }
    pub fn request_digest(&self) -> [u8; 32] {
        Sha256::digest(&self.body).into()
    }
    pub fn expires_at(&self) -> i64 {
        self.expires_at
    }
    /// Domain-separated fingerprint of the actual internal query inputs.
    pub fn query_digest(&self, purpose: &str, input: &[u8]) -> [u8; 32] {
        let mut digest = Sha256::new();
        digest.update(b"port-rust owned query witness v1\n");
        digest.update(self.parent_digest());
        digest.update((purpose.len() as u64).to_be_bytes());
        digest.update(purpose.as_bytes());
        digest.update(input);
        digest.finalize().into()
    }

    /// Authority of the currently polled admitted handler. It is not inherited
    /// by spawned tasks: an adapter must explicitly capture and scope such work.
    pub fn current() -> Option<Self> {
        PARENT_AUTHORITY.try_with(Clone::clone).ok()
    }

    /// Explicitly propagate this sealed authority to work owned by the admitted
    /// handler. Issuance still rechecks the signed parent's expiry and scope.
    pub async fn scope<F: Future>(self, future: F) -> F::Output {
        PARENT_AUTHORITY.scope(self, future).await
    }
}

/// No Debug/Serde: an issued token is authority, not diagnostic metadata.
pub struct IssuedChild {
    pub token: String,
    pub expires_at: i64,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ChildClaims {
    v: u8,
    kid: String,
    iss: String,
    pres: String,
    aud: String,
    mth: String,
    sub: Uuid,
    dig: String,
    op: Option<Uuid>,
    par: String,
    wit: Option<Uuid>,
    iat: i64,
    exp: i64,
    jti: String,
}

#[derive(Clone)]
pub struct DelegationExchange {
    signer: ActorDelegationSigner,
    parents: ActorDelegationVerifier,
    resources: ResourceEvidenceVerifier,
    usage: Arc<Mutex<HashMap<String, (i64, usize)>>>,
}

impl DelegationExchange {
    pub fn new(
        signer: ActorDelegationSigner,
        parents: ActorDelegationVerifier,
        resources: ResourceEvidenceVerifier,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            signer.issuer == Workload::Gateway,
            "only Gateway may issue child authority"
        );
        anyhow::ensure!(
            parents.audience == Workload::Post,
            "exchange verifies Post parent authority"
        );
        anyhow::ensure!(
            parents
                .keys
                .iter()
                .any(|(_, key)| *key == signer.public_key()),
            "exchange verification keys must include the current Gateway signing key"
        );
        Ok(Self {
            signer,
            parents,
            resources,
            usage: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// `presenter` must come from authenticated workload identity, not the wire
    /// payload. The issuer rechecks the original exact parent body and actor.
    pub fn prepare(
        &self,
        presenter: Workload,
        parent: ParentRequest<'_>,
        child: ChildRequest<'_>,
        now: i64,
    ) -> Result<PreparedExchange, Denial> {
        self.prepare_for_stage(presenter, parent, child, None, now)
    }

    /// Resolve this immutable owner witness independently. The ID is a lookup
    /// locator, never evidence of membership or permission on its own.
    pub fn prepare_with_witness(
        &self,
        presenter: Workload,
        parent: ParentRequest<'_>,
        child: ChildRequest<'_>,
        witness_id: Uuid,
        now: i64,
    ) -> Result<PreparedExchange, Denial> {
        if witness_id.is_nil() {
            return Err(Denial::WrongBody);
        }
        self.prepare_for_stage(presenter, parent, child, Some(witness_id), now)
    }

    fn prepare_for_stage(
        &self,
        presenter: Workload,
        parent: ParentRequest<'_>,
        child: ChildRequest<'_>,
        witness_id: Option<Uuid>,
        now: i64,
    ) -> Result<PreparedExchange, Denial> {
        if presenter != Workload::Post {
            return Err(Denial::WrongIssuer);
        }
        if parent.token.len() > MAX_TOKEN_BYTES
            || parent.body.len() > MAX_BODY_BYTES
            || child.body.len() > MAX_BODY_BYTES
        {
            return Err(Denial::BodyTooLarge);
        }
        let (root_fields, create_profile) = parent_scope(parent.method, parent.body)?;
        let verified = self.parents.verify(
            parent.token,
            &Expected {
                caller: Workload::Gateway,
                method: parent.method,
                body: parent.body,
                operation: parent.operation,
            },
            now,
        )?;
        if verified.actor.is_nil() {
            return Err(Denial::ActorMismatch);
        }
        bind_actor(parent.body, root_fields, verified.actor)?;
        // Verification allows bounded clock skew. Issuance cannot extend an
        // expired parent's authority by issuing a fresh child during that skew.
        if verified.exp <= now {
            return Err(Denial::Expired);
        }
        if child.operation.is_some() && child.operation != parent.operation {
            return Err(Denial::WrongOperation);
        }
        match (child.audience, child.method) {
            (
                Workload::Profile,
                "/authority.v1.ProfileAuthorAuthorityService/CheckAuthorProfile",
            ) if create_profile => {
                bind_actor(child.body, &[&[1]], verified.actor)?;
            }
            (Workload::Friend, "/friend.v1.FriendService/ListFollowing")
                if parent.method == "/post.v1.PostService/GetFeed" =>
            {
                bind_actor(child.body, &[&[1]], verified.actor)?;
            }
            (
                Workload::Friend,
                "/friend.v1.FriendService/GetRelationship"
                | "/friend.v1.FriendService/BatchGetRelationships",
            ) if !create_profile => {
                bind_actor(child.body, &[&[1]], verified.actor)?;
            }
            _ => return Err(Denial::WrongMethod),
        }
        let targets = match child.method {
            "/friend.v1.FriendService/BatchGetRelationships" => {
                uuid_values(unary_message(child.body)?, 2, 100)?
            }
            "/friend.v1.FriendService/GetRelationship" => {
                vec![uuid_field(unary_message(child.body)?, &[2])?]
            }
            _ => vec![verified.actor],
        };
        let scope = if child.method == "/friend.v1.FriendService/ListFollowing" || create_profile {
            ResourceScope::ExactAuthor(verified.actor)
        } else {
            resource_scope(parent.method, parent.body)?
        };
        if let ResourceScope::ExactAuthor(author) = &scope {
            if witness_id.is_some() || targets.iter().any(|target| target != author) {
                return Err(Denial::WrongBody);
            }
        } else if witness_id.is_none() {
            return Err(Denial::Missing);
        }
        Ok(PreparedExchange {
            claims: ChildClaims {
                v: 2,
                kid: self.signer.key_id.clone(),
                iss: Workload::Gateway.as_str().into(),
                pres: presenter.as_str().into(),
                aud: child.audience.as_str().into(),
                mth: child.method.into(),
                sub: verified.actor,
                dig: body_digest(child.body),
                op: child.operation,
                par: body_digest(parent.token.as_bytes()),
                wit: witness_id,
                iat: now,
                exp: verified.exp.min(now + LIFETIME_SECONDS),
                jti: Uuid::new_v4().simple().to_string(),
            },
            scope,
            targets,
            parent_expiry: verified.exp,
        })
    }

    /// Gateway's resolver must verify owner identity and response provenance.
    /// Expiry is rechecked after the lookup, immediately before signing.
    pub fn finish(
        &self,
        mut prepared: PreparedExchange,
        resource_token: Option<&str>,
        now: i64,
    ) -> Result<IssuedChild, Denial> {
        if prepared.claims.kid != self.signer.key_id {
            return Err(Denial::WrongIssuer);
        }
        if now < prepared.claims.iat {
            return Err(Denial::NotYetValid);
        }
        if now >= prepared.parent_expiry {
            return Err(Denial::Expired);
        }
        match prepared.scope {
            ResourceScope::ExactAuthor(_) if resource_token.is_none() => {}
            ResourceScope::ExactAuthor(_) => return Err(Denial::WrongBody),
            _ => {
                let resolution = self.resources.verify(
                    resource_token.ok_or(Denial::Missing)?,
                    &prepared.claims.par,
                    prepared.claims.wit.ok_or(Denial::Missing)?,
                    now,
                )?;
                validate_resolution(&prepared, &resolution)?;
            }
        }
        let parent_binding = prepared.claims.par.clone();
        let mut usage = self.usage.lock().map_err(|_| Denial::ReplayCacheFull)?;
        usage.retain(|_, (expiry, _)| *expiry > now);
        if !usage.contains_key(&parent_binding) && usage.len() >= PARENT_CACHE_CAPACITY {
            return Err(Denial::ReplayCacheFull);
        }
        let count = usage
            .entry(parent_binding.clone())
            .or_insert((prepared.parent_expiry, 0));
        if count.1 >= MAX_CHILDREN_PER_PARENT {
            return Err(Denial::ReplayCacheFull);
        }
        count.1 += 1;
        drop(usage);
        let expires_at = prepared.parent_expiry.min(now + LIFETIME_SECONDS);
        prepared.claims.iat = now;
        prepared.claims.exp = expires_at;
        let payload = Base64UrlUnpadded::encode_string(
            &serde_json::to_vec(&prepared.claims).expect("child claims serialize"),
        );
        let signature = self.signer.key.sign(&child_signed_bytes(&payload));
        Ok(IssuedChild {
            token: format!(
                "{payload}.{}",
                Base64UrlUnpadded::encode_string(signature.as_ref())
            ),
            expires_at,
        })
    }

    #[cfg(test)]
    fn issue(
        &self,
        presenter: Workload,
        parent: ParentRequest<'_>,
        child: ChildRequest<'_>,
        now: i64,
    ) -> Result<IssuedChild, Denial> {
        let prepared = self.prepare(presenter, parent, child, now)?;
        self.finish(prepared, None, now)
    }
}

fn child_signed_bytes(payload: &str) -> Vec<u8> {
    [CHILD_CONTEXT, payload.as_bytes()].concat()
}

fn uuid_field(message: &[u8], path: &[u32]) -> Result<Uuid, Denial> {
    field_bytes(message, path)?
        .and_then(|bytes| std::str::from_utf8(bytes).ok())
        .and_then(|text| Uuid::parse_str(text).ok())
        .filter(|id| !id.is_nil())
        .ok_or(Denial::WrongBody)
}

fn uuid_values(message: &[u8], field: u32, limit: usize) -> Result<Vec<Uuid>, Denial> {
    let mut input = message;
    let mut values = Vec::new();
    let mut seen = std::collections::HashSet::new();
    while !input.is_empty() {
        let key = read_varint(&mut input).ok_or(Denial::Malformed)?;
        let (number, wire) = (key >> 3, key & 7);
        if number == 0 {
            return Err(Denial::Malformed);
        }
        let value = match wire {
            0 => {
                read_varint(&mut input).ok_or(Denial::Malformed)?;
                None
            }
            1 | 5 => {
                let width = if wire == 1 { 8 } else { 4 };
                input = input.get(width..).ok_or(Denial::Malformed)?;
                None
            }
            2 => {
                let length = usize::try_from(read_varint(&mut input).ok_or(Denial::Malformed)?)
                    .map_err(|_| Denial::Malformed)?;
                let value = input.get(..length).ok_or(Denial::Malformed)?;
                input = &input[length..];
                Some(value)
            }
            _ => return Err(Denial::Malformed),
        };
        if number == u64::from(field) {
            let id = value
                .and_then(|bytes| std::str::from_utf8(bytes).ok())
                .and_then(|text| Uuid::parse_str(text).ok())
                .filter(|id| !id.is_nil())
                .ok_or(Denial::WrongBody)?;
            if !seen.insert(id) || values.len() >= limit {
                return Err(Denial::WrongBody);
            }
            values.push(id);
        }
    }
    if values.is_empty() {
        return Err(Denial::WrongBody);
    }
    Ok(values)
}

fn resource_scope(method: &str, body: &[u8]) -> Result<ResourceScope, Denial> {
    let message = unary_message(body)?;
    match method {
        "/post.v1.PostService/GetAuthorPosts" => {
            Ok(ResourceScope::ExactAuthor(uuid_field(message, &[1])?))
        }
        "/post.v1.PostService/GetFeed"
        | "/post.v1.PostService/GetTrending"
        | "/post.v1.PostService/SearchPosts" => Ok(ResourceScope::DiscoveryCandidates),
        "/post.v1.PostService/AuthorizeNotificationPosts" => {
            Ok(ResourceScope::Posts(uuid_values(message, 2, 100)?))
        }
        "/post.v1.PostService/GetPost"
        | "/post.v1.PostService/CreateComment"
        | "/post.v1.PostService/DeletePost"
        | "/post.v1.PostService/LikePost"
        | "/post.v1.PostService/UnlikePost" => {
            Ok(ResourceScope::Posts(vec![uuid_field(message, &[1])?]))
        }
        "/post.v1.PostService/ReplayMutation" => {
            let command = (2..=5)
                .find(|field| field_bytes(message, &[*field]).ok().flatten().is_some())
                .ok_or(Denial::WrongBody)?;
            Ok(ResourceScope::Posts(vec![uuid_field(
                message,
                &[command, 1],
            )?]))
        }
        _ => Err(Denial::WrongMethod),
    }
}

fn validate_resolution(
    prepared: &PreparedExchange,
    resolution: &ResourceResolution,
) -> Result<(), Denial> {
    if resolution.parent_digest != prepared.claims.par
        || Some(resolution.witness_id) != prepared.claims.wit
    {
        return Err(Denial::WrongBody);
    }
    if matches!(prepared.scope, ResourceScope::ExactAuthor(_)) {
        return if resolution.posts.is_empty() && resolution.missing_post_ids.is_empty() {
            Ok(())
        } else {
            Err(Denial::WrongBody)
        };
    }
    if resolution.posts.len() > 100 || resolution.missing_post_ids.len() > 100 {
        return Err(Denial::WrongBody);
    }
    let mut ids = std::collections::HashSet::new();
    let mut authors = std::collections::HashSet::new();
    for post in &resolution.posts {
        if post.post_id.is_nil() || post.author_id.is_nil() || !ids.insert(post.post_id) {
            return Err(Denial::WrongBody);
        }
        authors.insert(post.author_id);
    }
    for id in &resolution.missing_post_ids {
        if id.is_nil() || !ids.insert(*id) {
            return Err(Denial::WrongBody);
        }
    }
    match &prepared.scope {
        ResourceScope::Posts(expected) if ids != expected.iter().copied().collect() => {
            return Err(Denial::WrongBody)
        }
        ResourceScope::DiscoveryCandidates if !resolution.missing_post_ids.is_empty() => {
            return Err(Denial::WrongBody)
        }
        _ => {}
    }
    if prepared
        .targets
        .iter()
        .any(|target| !authors.contains(target))
    {
        return Err(Denial::WrongBody);
    }
    Ok(())
}

fn parent_scope(method: &str, body: &[u8]) -> Result<(&'static [&'static [u32]], bool), Denial> {
    match method {
        "/post.v1.PostService/CreatePost" => Ok((&[&[1]], true)),
        "/post.v1.PostService/ReplayMutation" => {
            let message = unary_message(body)?;
            let present = (1..=5)
                .map(|field| field_bytes(message, &[field]))
                .collect::<Result<Vec<_>, _>>()?;
            if present.iter().filter(|value| value.is_some()).count() != 1 {
                return Err(Denial::Malformed);
            }
            Ok((
                &[&[1, 1], &[2, 2], &[3, 2], &[4, 2], &[5, 2]],
                present[0].is_some(),
            ))
        }
        "/post.v1.PostService/GetFeed"
        | "/post.v1.PostService/GetTrending"
        | "/post.v1.PostService/AuthorizeNotificationPosts" => Ok((&[&[1]], false)),
        "/post.v1.PostService/GetAuthorPosts" | "/post.v1.PostService/SearchPosts" => {
            Ok((&[&[4]], false))
        }
        "/post.v1.PostService/GetPost"
        | "/post.v1.PostService/CreateComment"
        | "/post.v1.PostService/DeleteComment"
        | "/post.v1.PostService/DeletePost"
        | "/post.v1.PostService/LikePost"
        | "/post.v1.PostService/UnlikePost" => Ok((&[&[2]], false)),
        _ => Err(Denial::WrongMethod),
    }
}

impl ActorDelegationVerifier {
    /// Verify a Gateway child proof presented by Post. This does not consume
    /// its replay identity; owner actor/resource checks must precede consumption.
    pub fn verify_child(
        &self,
        token: &str,
        expected: &Expected<'_>,
        now: i64,
    ) -> Result<Verified, Denial> {
        if token.len() > MAX_TOKEN_BYTES || expected.body.len() > MAX_BODY_BYTES {
            return Err(Denial::BodyTooLarge);
        }
        let (payload, signature) = token.split_once('.').ok_or(Denial::Malformed)?;
        let signature = Base64UrlUnpadded::decode_vec(signature).map_err(|_| Denial::Malformed)?;
        let claims: ChildClaims = Base64UrlUnpadded::decode_vec(payload)
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .ok_or(Denial::Malformed)?;
        if claims.v != 2
            || claims.jti.len() < 16
            || claims.jti.len() > 64
            || claims.sub.is_nil()
            || claims.wit.is_some_and(|id| id.is_nil())
            || Base64UrlUnpadded::decode_vec(&claims.par).map_or(true, |bytes| bytes.len() != 32)
        {
            return Err(Denial::Malformed);
        }
        let (_, key) = self
            .keys
            .iter()
            .find(|(id, _)| *id == claims.kid)
            .ok_or(Denial::UnknownKey)?;
        UnparsedPublicKey::new(&ED25519, key)
            .verify(&child_signed_bytes(payload), &signature)
            .map_err(|_| Denial::BadSignature)?;
        if claims.iss != Workload::Gateway.as_str()
            || expected.caller != Workload::Post
            || claims.pres != expected.caller.as_str()
        {
            return Err(Denial::WrongIssuer);
        }
        if claims.aud != self.audience.as_str() {
            return Err(Denial::WrongAudience);
        }
        if claims.mth != expected.method {
            return Err(Denial::WrongMethod);
        }
        if claims.exp <= claims.iat || claims.exp - claims.iat > MAX_LIFETIME_SECONDS {
            return Err(Denial::LifetimeTooLong);
        }
        if now + CLOCK_SKEW_SECONDS < claims.iat {
            return Err(Denial::NotYetValid);
        }
        if now > claims.exp + CLOCK_SKEW_SECONDS {
            return Err(Denial::Expired);
        }
        if claims.dig != body_digest(expected.body) {
            return Err(Denial::WrongBody);
        }
        if claims.op != expected.operation {
            return Err(Denial::WrongOperation);
        }
        Ok(Verified {
            actor: claims.sub,
            jti: claims.jti,
            exp: claims.exp,
        })
    }
}

#[cfg(test)]
mod tests;
