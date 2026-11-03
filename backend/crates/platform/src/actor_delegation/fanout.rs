//! Explicit system authority for one claimed timeline-fanout page. This is
//! neither actor delegation nor persisted-event/notification authority.
use super::*;
use crate::grpc_auth::VerifiedCaller;
use http_body_util::{BodyExt, Full, Limited};
use tonic::server::NamedService;

pub const METHOD: &str = "/authority.v1.FanoutProjectionService/ListFanoutFollowers";
pub const HEADER: &str = "x-fanout-grant";
const OWNER_CONTEXT: &[u8] = b"port-rust claimed fanout owner v1\n";
const GRANT_CONTEXT: &[u8] = b"port-rust claimed fanout gateway grant v1\n";
const PURPOSE: &str = "timeline_fanout_page";
const MAX_BYTES: usize = 16 * 1024;
const MAX_LIFETIME_MICROS: i64 = 60_000_000;

/// Explicit trusted lifetime policy, with no deployment default. There is no
/// positive expiry skew: a proof never extends a stored lease.
#[derive(Clone, Copy)]
pub struct LifetimePolicy(i64);
impl LifetimePolicy {
    pub fn new(microseconds: i64) -> Result<Self, Denial> {
        if !(1..=MAX_LIFETIME_MICROS).contains(&microseconds) {
            return Err(Denial::LifetimeTooLong);
        }
        Ok(Self(microseconds))
    }
}

/// Signed canonical scope. Lease tokens are hashed; exact nullable cursor
/// bytes and all persisted generation/budget fields are retained. No Debug.
#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FanoutScope {
    pub job_id: Uuid,
    pub event_id: Uuid,
    pub post_id: Uuid,
    pub author_id: Uuid,
    pub lease_digest: String,
    pub post_created_micros: i64,
    pub lease_expires_micros: i64,
    pub attempt: i32,
    pub page_attempt: i32,
    pub max_page_attempts: i32,
    pub cursor: Option<String>,
    pub limit: i32,
}
impl FanoutScope {
    pub fn lease_digest(token: Uuid) -> Result<String, Denial> {
        if token.is_nil() {
            return Err(Denial::WrongBody);
        }
        Ok(body_digest(token.as_bytes()))
    }
    pub fn validate(&self) -> Result<(), Denial> {
        if [self.job_id, self.event_id, self.post_id, self.author_id]
            .iter()
            .any(Uuid::is_nil)
            || self.attempt < 1
            || !(1..=100).contains(&self.max_page_attempts)
            || !(1..=self.max_page_attempts).contains(&self.page_attempt)
            || !(1..=100).contains(&self.limit)
            || Base64UrlUnpadded::decode_vec(&self.lease_digest).map_or(true, |v| v.len() != 32)
        {
            return Err(Denial::WrongBody);
        }
        if let Some(cursor) = &self.cursor {
            if cursor.len() > 128
                || !cursor.split_once('|').is_some_and(|(t, id)| {
                    chrono::DateTime::parse_from_rfc3339(t).is_ok()
                        && Uuid::parse_str(id).is_ok_and(|id| !id.is_nil())
                })
            {
                return Err(Denial::WrongBody);
            }
        }
        Ok(())
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Evidence {
    v: u8,
    kid: String,
    iss: String,
    aud: String,
    purpose: String,
    challenge: Uuid,
    method: String,
    digest: String,
    scope: FanoutScope,
    checked_at: i64,
    exp: i64,
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
    digest: String,
    scope: FanoutScope,
    iat: i64,
    exp: i64,
    jti: Uuid,
}

#[derive(Clone)]
struct SigningKey {
    key: Arc<Ed25519KeyPair>,
    id: String,
}
impl SigningKey {
    fn new(seed: &[u8; 32]) -> anyhow::Result<Self> {
        let key = Ed25519KeyPair::from_seed_unchecked(seed)
            .map_err(|_| anyhow::anyhow!("Invalid fanout signing seed"))?;
        let id = key_id(key.public_key().as_ref());
        Ok(Self {
            key: Arc::new(key),
            id,
        })
    }
    fn public_key(&self) -> [u8; 32] {
        self.key
            .public_key()
            .as_ref()
            .try_into()
            .expect("Ed25519 public key")
    }
    fn sign(&self, context: &[u8], claims: &impl Serialize) -> String {
        let payload = Base64UrlUnpadded::encode_string(
            &serde_json::to_vec(claims).expect("Fanout claims serialize"),
        );
        let signature = self.key.sign(&[context, payload.as_bytes()].concat());
        format!(
            "{payload}.{}",
            Base64UrlUnpadded::encode_string(signature.as_ref())
        )
    }
}
#[derive(Clone)]
struct VerificationKeys(Arc<Vec<(String, [u8; 32])>>);
impl VerificationKeys {
    fn new(keys: &[[u8; 32]]) -> anyhow::Result<Self> {
        anyhow::ensure!(!keys.is_empty(), "Fanout verification keys required");
        Ok(Self(Arc::new(
            keys.iter().map(|k| (key_id(k), *k)).collect(),
        )))
    }
    fn verify<T: serde::de::DeserializeOwned>(
        &self,
        context: &[u8],
        token: &str,
    ) -> Result<T, Denial> {
        if token.len() > MAX_BYTES {
            return Err(Denial::BodyTooLarge);
        }
        let (payload, signature) = token.split_once('.').ok_or(Denial::Malformed)?;
        let bytes = Base64UrlUnpadded::decode_vec(payload).map_err(|_| Denial::Malformed)?;
        let value: serde_json::Value =
            serde_json::from_slice(&bytes).map_err(|_| Denial::Malformed)?;
        let id = value
            .get("kid")
            .and_then(serde_json::Value::as_str)
            .ok_or(Denial::Malformed)?;
        let (_, key) = self
            .0
            .iter()
            .find(|(kid, _)| kid == id)
            .ok_or(Denial::UnknownKey)?;
        let signature = Base64UrlUnpadded::decode_vec(signature).map_err(|_| Denial::Malformed)?;
        UnparsedPublicKey::new(&ED25519, key)
            .verify(&[context, payload.as_bytes()].concat(), &signature)
            .map_err(|_| Denial::BadSignature)?;
        serde_json::from_slice(&bytes).map_err(|_| Denial::Malformed)
    }
}

/// Owning Post signs only data obtained from its independently authenticated
/// repository lookup. The producer must establish provenance before calling.
#[derive(Clone)]
pub struct FanoutOwnerSigner {
    key: SigningKey,
    policy: LifetimePolicy,
}
impl FanoutOwnerSigner {
    pub fn new(seed: &[u8; 32], policy: LifetimePolicy) -> anyhow::Result<Self> {
        Ok(Self {
            key: SigningKey::new(seed)?,
            policy,
        })
    }
    pub fn public_key(&self) -> [u8; 32] {
        self.key.public_key()
    }
    pub fn attest(
        &self,
        scope: FanoutScope,
        body: &[u8],
        challenge: Uuid,
        checked_at: i64,
        now: i64,
    ) -> Result<String, Denial> {
        scope.validate()?;
        unary_message(body)?;
        if body.len() > MAX_BYTES || challenge.is_nil() {
            return Err(Denial::WrongBody);
        }
        let exp = scope
            .lease_expires_micros
            .min(checked_at.saturating_add(self.policy.0));
        valid_time(checked_at, exp, now, scope.lease_expires_micros)?;
        Ok(self.key.sign(
            OWNER_CONTEXT,
            &Evidence {
                v: 1,
                kid: self.key.id.clone(),
                iss: "post".into(),
                aud: "gateway".into(),
                purpose: PURPOSE.into(),
                challenge,
                method: METHOD.into(),
                digest: body_digest(body),
                scope,
                checked_at,
                exp,
            },
        ))
    }
}

/// Sealed per-issuance preparation; the fresh challenge must go to Post through
/// an independent authenticated lookup, never be supplied by the presenter.
pub struct PreparedFanout {
    scope: FanoutScope,
    body: Vec<u8>,
    challenge: Uuid,
    expires_micros: i64,
    stop: tokio::time::Instant,
}
impl PreparedFanout {
    pub fn deadline(&self) -> Result<super::authority_deadline::AuthorityRpcDeadline, Denial> {
        let remaining = self
            .expires_micros
            .saturating_sub(chrono::Utc::now().timestamp_micros());
        if remaining <= 0 {
            return Err(Denial::Expired);
        }
        super::authority_deadline::AuthorityRpcDeadline::within_original(
            self.expires_micros,
            self.stop.into_std(),
            std::time::Duration::from_micros(remaining as u64),
        )
        .map_err(|_| Denial::Expired)
    }

    pub fn challenge(&self) -> Uuid {
        self.challenge
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }
}
pub struct IssuedFanout {
    pub token: String,
    pub expires_micros: i64,
}
#[derive(Clone)]
pub struct FanoutIssuer {
    key: SigningKey,
    owners: VerificationKeys,
    policy: LifetimePolicy,
}
impl FanoutIssuer {
    pub fn new(
        seed: &[u8; 32],
        owner_keys: &[[u8; 32]],
        policy: LifetimePolicy,
    ) -> anyhow::Result<Self> {
        Ok(Self {
            key: SigningKey::new(seed)?,
            owners: VerificationKeys::new(owner_keys)?,
            policy,
        })
    }
    pub fn public_key(&self) -> [u8; 32] {
        self.key.public_key()
    }
    pub fn prepare(
        &self,
        presenter: Workload,
        scope: FanoutScope,
        body: Vec<u8>,
        now: i64,
    ) -> Result<PreparedFanout, Denial> {
        let deadline = scope.lease_expires_micros;
        self.prepare_until(presenter, scope, body, now, deadline)
    }
    pub fn prepare_until(
        &self,
        presenter: Workload,
        scope: FanoutScope,
        body: Vec<u8>,
        now: i64,
        deadline: i64,
    ) -> Result<PreparedFanout, Denial> {
        if presenter != Workload::Post {
            return Err(Denial::WrongIssuer);
        }
        scope.validate()?;
        unary_message(&body)?;
        if body.len() > MAX_BYTES {
            return Err(Denial::BodyTooLarge);
        }
        let expires_micros = deadline
            .min(scope.lease_expires_micros)
            .min(now.saturating_add(self.policy.0));
        if now >= expires_micros {
            return Err(Denial::Expired);
        }
        Ok(PreparedFanout {
            scope,
            body,
            challenge: Uuid::new_v4(),
            expires_micros,
            stop: tokio::time::Instant::now()
                + std::time::Duration::from_micros((expires_micros - now) as u64),
        })
    }
    fn owner_expiry(
        &self,
        request: &PreparedFanout,
        evidence: &str,
        now: i64,
    ) -> Result<i64, Denial> {
        let owned: Evidence = self.owners.verify(OWNER_CONTEXT, evidence)?;
        if owned.v != 1
            || owned.iss != "post"
            || owned.aud != "gateway"
            || owned.purpose != PURPOSE
            || owned.method != METHOD
        {
            return Err(Denial::WrongIssuer);
        }
        if owned.challenge != request.challenge
            || owned.digest != body_digest(&request.body)
            || owned.scope != request.scope
        {
            return Err(Denial::WrongBody);
        }
        valid_time(
            owned.checked_at,
            owned.exp,
            now,
            owned.scope.lease_expires_micros,
        )?;
        Ok(owned.exp)
    }
    pub fn finish(
        &self,
        request: PreparedFanout,
        evidence: &str,
        now: i64,
    ) -> Result<IssuedFanout, Denial> {
        let exp = self
            .owner_expiry(&request, evidence, now)?
            .min(now.saturating_add(self.policy.0))
            .min(request.expires_micros);
        valid_time(now, exp, now, request.scope.lease_expires_micros)?;
        let token = self.key.sign(
            GRANT_CONTEXT,
            &Grant {
                v: 1,
                kid: self.key.id.clone(),
                iss: "gateway".into(),
                pres: "post".into(),
                aud: "friend".into(),
                purpose: PURPOSE.into(),
                method: METHOD.into(),
                digest: body_digest(&request.body),
                scope: request.scope,
                iat: now,
                exp,
                jti: Uuid::new_v4(),
            },
        );
        Ok(IssuedFanout {
            token,
            expires_micros: exp,
        })
    }
    /// Admission uses a second fresh challenge/owner lookup. It does not mint
    /// a replacement grant or reset persisted attempts.
    pub fn check_fresh_owner(
        &self,
        request: PreparedFanout,
        evidence: &str,
        now: i64,
    ) -> Result<i64, Denial> {
        let expiry = self
            .owner_expiry(&request, evidence, now)?
            .min(request.expires_micros);
        if now >= expiry {
            return Err(Denial::Expired);
        }
        Ok(expiry)
    }
}
fn valid_time(iat: i64, exp: i64, now: i64, lease: i64) -> Result<(), Denial> {
    if exp <= iat || exp.saturating_sub(iat) > MAX_LIFETIME_MICROS || exp > lease {
        return Err(Denial::LifetimeTooLong);
    }
    if now < iat {
        return Err(Denial::NotYetValid);
    }
    if now >= exp {
        return Err(Denial::Expired);
    }
    Ok(())
}

/// Signature-verified proposal, still requiring fresh owner admission and
/// atomic replay consumption before dispatch. No Debug/Serialize.
#[derive(Clone)]
pub struct VerifiedFanout {
    scope: FanoutScope,
    token: Arc<str>,
    body: Arc<[u8]>,
    exp: i64,
    jti: Uuid,
}
impl VerifiedFanout {
    pub fn scope(&self) -> &FanoutScope {
        &self.scope
    }
    pub fn token(&self) -> &str {
        &self.token
    }
    pub fn body(&self) -> &[u8] {
        &self.body
    }
    pub fn expires_micros(&self) -> i64 {
        self.exp
    }
}
/// Sealed extension installed only after the guard's fresh owner lookup and
/// replay consumption. A handler registered without that guard denies.
#[derive(Clone)]
pub struct AdmittedFanout {
    proof: VerifiedFanout,
    expires_micros: i64,
    stop: tokio::time::Instant,
}
impl AdmittedFanout {
    pub fn expires_micros(&self) -> i64 {
        self.expires_micros
    }
    pub fn deadline(&self) -> Result<super::authority_deadline::AuthorityRpcDeadline, Denial> {
        let remaining = self
            .expires_micros
            .saturating_sub(chrono::Utc::now().timestamp_micros());
        if remaining <= 0 {
            return Err(Denial::Expired);
        }
        super::authority_deadline::AuthorityRpcDeadline::within_original(
            self.expires_micros,
            self.stop.into_std(),
            std::time::Duration::from_micros(remaining as u64),
        )
        .map_err(|_| Denial::Expired)
    }

    pub fn scope(&self) -> &FanoutScope {
        self.proof.scope()
    }
}
#[derive(Clone)]
pub struct FanoutVerifier {
    keys: VerificationKeys,
    used: Arc<Mutex<HashMap<Uuid, i64>>>,
    capacity: usize,
}
impl FanoutVerifier {
    pub fn new(keys: &[[u8; 32]], replay_capacity: usize) -> anyhow::Result<Self> {
        anyhow::ensure!(replay_capacity > 0, "Fanout replay capacity required");
        Ok(Self {
            keys: VerificationKeys::new(keys)?,
            used: Arc::new(Mutex::new(HashMap::new())),
            capacity: replay_capacity,
        })
    }
    pub fn verify(
        &self,
        token: &str,
        presenter: Workload,
        method: &str,
        body: &[u8],
        now: i64,
    ) -> Result<VerifiedFanout, Denial> {
        if presenter != Workload::Post {
            return Err(Denial::WrongIssuer);
        }
        let grant: Grant = self.keys.verify(GRANT_CONTEXT, token)?;
        if grant.v != 1 || grant.iss != "gateway" || grant.pres != "post" {
            return Err(Denial::WrongIssuer);
        }
        if grant.aud != "friend" {
            return Err(Denial::WrongAudience);
        }
        if method != METHOD || grant.method != METHOD || grant.purpose != PURPOSE {
            return Err(Denial::WrongMethod);
        }
        unary_message(body)?;
        if body.len() > MAX_BYTES {
            return Err(Denial::BodyTooLarge);
        }
        if grant.digest != body_digest(body) || grant.jti.is_nil() {
            return Err(Denial::WrongBody);
        }
        grant.scope.validate()?;
        valid_time(grant.iat, grant.exp, now, grant.scope.lease_expires_micros)?;
        Ok(VerifiedFanout {
            scope: grant.scope,
            token: token.into(),
            body: body.into(),
            exp: grant.exp,
            jti: grant.jti,
        })
    }
    fn consume(
        &self,
        proof: &VerifiedFanout,
        mut clock: impl FnMut() -> i64,
    ) -> Result<(), Denial> {
        let mut used = self.used.lock().unwrap_or_else(|p| p.into_inner());
        // Sample after mutex acquisition too: a verified request can spend
        // its remaining lease waiting behind another replay-cache operation.
        let now = clock();
        if now >= proof.exp {
            return Err(Denial::Expired);
        }
        if used.len() >= self.capacity {
            used.retain(|_, exp| *exp > now);
        }
        // Cleanup itself can also exhaust a very short remaining lifetime.
        if clock() >= proof.exp {
            return Err(Denial::Expired);
        }
        if used.contains_key(&proof.jti) {
            return Err(Denial::Replayed);
        }
        if used.len() >= self.capacity {
            return Err(Denial::ReplayCacheFull);
        }
        used.insert(proof.jti, proof.exp);
        Ok(())
    }
}

pub type AdmissionFuture<'a> = Pin<Box<dyn Future<Output = Result<i64, Denial>> + Send + 'a>>;
/// Must call authenticated Gateway, which performs a new Post-owner lookup.
/// A process-local proof/cache is never an implementation of fresh admission.
pub trait FanoutAdmissionCheck: Send + Sync {
    fn check<'a>(&'a self, proof: &'a VerifiedFanout) -> AdmissionFuture<'a>;
}

#[derive(Clone)]
pub struct FanoutGuard<S> {
    inner: S,
    verifier: FanoutVerifier,
    admission: Arc<dyn FanoutAdmissionCheck>,
}
impl<S: NamedService> FanoutGuard<S> {
    pub fn new(
        inner: S,
        verifier: FanoutVerifier,
        admission: Arc<dyn FanoutAdmissionCheck>,
    ) -> anyhow::Result<Self> {
        anyhow::ensure!(
            S::NAME == "authority.v1.FanoutProjectionService",
            "Fanout service required"
        );
        Ok(Self {
            inner,
            verifier,
            admission,
        })
    }
}
impl<S: NamedService> NamedService for FanoutGuard<S> {
    const NAME: &'static str = S::NAME;
}
impl<S, B, R> tower::Service<http::Request<B>> for FanoutGuard<S>
where
    S: tower::Service<http::Request<Body>, Response = http::Response<R>> + Clone + Send + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
    B: http_body::Body<Data = tonic::codegen::Bytes> + Send + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
    R: Default + Send + 'static,
{
    type Response = http::Response<R>;
    type Error = S::Error;
    type Future = Pin<Box<dyn Future<Output = Result<Self::Response, Self::Error>> + Send>>;
    fn poll_ready(&mut self, cx: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(cx)
    }
    fn call(&mut self, request: http::Request<B>) -> Self::Future {
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        let verifier = self.verifier.clone();
        let admission = self.admission.clone();
        Box::pin(async move {
            let (mut parts, body) = request.into_parts();
            let Some(VerifiedCaller(caller)) = parts.extensions.get::<VerifiedCaller>().copied()
            else {
                return Ok(Status::unauthenticated("Verified fanout caller required").into_http());
            };
            if caller != Workload::Post || parts.uri.path() != METHOD {
                return Ok(status(Denial::WrongIssuer).into_http());
            }
            let tokens: Vec<_> = parts.headers.get_all(HEADER).iter().collect();
            if tokens.len() != 1 {
                return Ok(status(Denial::Missing).into_http());
            }
            let Ok(token) = tokens[0].to_str() else {
                return Ok(status(Denial::Malformed).into_http());
            };
            let Ok(collected) = Limited::new(body, MAX_BYTES).collect().await else {
                return Ok(status(Denial::BodyTooLarge).into_http());
            };
            let bytes = collected.to_bytes();
            let proof = match verifier.verify(
                token,
                caller,
                parts.uri.path(),
                &bytes,
                chrono::Utc::now().timestamp_micros(),
            ) {
                Ok(proof) => proof,
                Err(error) => return Ok(status(error).into_http()),
            };
            let original = match super::authority_deadline::AuthorityRpcDeadline::new(
                proof.expires_micros(),
                std::time::Duration::from_micros(MAX_LIFETIME_MICROS as u64),
            ) {
                Ok(d) => d,
                Err(_) => return Ok(status(Denial::Expired).into_http()),
            };
            let expiry =
                match tokio::time::timeout_at(original.stop(), admission.check(&proof)).await {
                    Ok(Ok(expiry)) if expiry <= proof.expires_micros() => expiry,
                    Ok(Err(error)) => return Ok(status(error).into_http()),
                    Ok(Ok(_)) => return Ok(status(Denial::WrongBody).into_http()),
                    Err(_) => return Ok(status(Denial::Expired).into_http()),
                };
            let refreshed = match super::authority_deadline::AuthorityRpcDeadline::within_original(
                expiry,
                original.stop().into_std(),
                std::time::Duration::from_micros(MAX_LIFETIME_MICROS as u64),
            ) {
                Ok(d) => d,
                Err(_) => return Ok(status(Denial::Expired).into_http()),
            };
            // Check refreshed expiry under the replay mutex, before consuming.
            let mut expired = false;
            let result = verifier.consume(&proof, || {
                let now = chrono::Utc::now().timestamp_micros();
                if now >= expiry || tokio::time::Instant::now() >= refreshed.stop() {
                    expired = true;
                    proof.expires_micros()
                } else {
                    now
                }
            });
            if expired {
                return Ok(status(Denial::Expired).into_http());
            }
            if let Err(error) = result {
                return Ok(status(error).into_http());
            }
            parts.extensions.insert(AdmittedFanout {
                proof,
                expires_micros: expiry,
                stop: refreshed.stop(),
            });
            inner
                .call(http::Request::from_parts(
                    parts,
                    Body::new(Full::new(bytes)),
                ))
                .await
        })
    }
}

/// Controlled system errors never include proof, SQL, cursor or lease values.
pub fn status(error: Denial) -> Status {
    match error {
        Denial::ReplayCacheFull => Status::unavailable("Fanout admission unavailable"),
        Denial::BodyTooLarge => Status::resource_exhausted("Fanout request too large"),
        _ => Status::permission_denied("Fanout authority denied"),
    }
}

#[cfg(test)]
mod tests;
