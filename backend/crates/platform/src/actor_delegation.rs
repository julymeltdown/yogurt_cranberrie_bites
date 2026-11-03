//! Owner-verifiable actor delegation for Gateway-forwarded internal RPCs.
//!
//! Workload authentication proves which service is calling. It does not prove
//! which user the call acts for: an owner used to trust the actor field of any
//! request presented with the Gateway credential. The Gateway now signs a
//! short-lived Ed25519 delegation for each such call. The token binds:
//!
//! - the issuing workload, which must equal the authenticated caller;
//! - the audience workload that serves the call;
//! - the full gRPC method path;
//! - the SHA-256 digest of the exact request body, which fixes the resource;
//! - the acting user, which must equal every actor field the owner declares
//!   for the method;
//! - the owner operation ID when the request carries one;
//! - an issue and expiry time, and a unique token ID.
//!
//! Owners hold only public keys, so a service that verifies delegations cannot
//! mint one for another user. A token ID is accepted once per owner process. A
//! captured token replayed to another replica can only repeat the identical
//! signed command, for the same operation, before it expires.

use crate::grpc_auth::Workload;
use base64ct::{Base64, Base64UrlUnpadded, Encoding};
use http_body_util::{BodyExt, Full, Limited};
use ring::signature::{Ed25519KeyPair, KeyPair, UnparsedPublicKey, ED25519};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::future::Future;
use std::pin::Pin;
use std::sync::{Arc, Mutex};
use std::task::{Context, Poll};
use tonic::body::Body;
use tonic::Status;
use uuid::Uuid;

pub mod authority_deadline;
pub mod cache;
pub mod exchange;
pub mod fanout;
pub mod notification;

/// Metadata header carrying the signed delegation.
pub const HEADER: &str = "x-actor-delegation";
/// Gateway signing seed: standard Base64 of 32 random bytes.
pub const SIGNING_SEED_ENV: &str = "ACTOR_DELEGATION_SIGNING_SEED";
/// Owner verification keys: comma-separated standard Base64 Ed25519 public
/// keys. List the current key and, during rotation, the previous one.
pub const VERIFYING_KEYS_ENV: &str = "ACTOR_DELEGATION_VERIFYING_KEYS";
/// Lifetime of an issued delegation.
pub const LIFETIME_SECONDS: i64 = 30;
/// Longest lifetime an owner accepts, whatever the token claims.
pub const MAX_LIFETIME_SECONDS: i64 = 60;
/// Tolerated clock difference between the Gateway and an owner.
pub const CLOCK_SKEW_SECONDS: i64 = 5;
/// Largest delegated request body, equal to tonic's default decode limit.
pub const MAX_BODY_BYTES: usize = 4 * 1024 * 1024;
const REPLAY_CACHE_CAPACITY: usize = 65_536;
const SIGNATURE_CONTEXT: &[u8] = b"port-rust actor delegation v1\n";
const VERSION: u8 = 1;
const OPERATION_HEADER: &str = crate::mutation_context::OPERATION_ID_HEADER;
/// Non-secret development key material. Production configuration rejects it.
const LOCAL_PARITY_SEED: [u8; 32] = *b"local-parity-actor-delegation-00";

/// Client-side request extension: sign this call on behalf of the actor.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct DelegatedActor {
    pub actor: Uuid,
}

/// Server-side request extension, present only after a delegation verified
/// and every declared actor field matched it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct VerifiedActor(pub Uuid);

/// Mark an outbound request as acting for the actor. The Gateway's delegation
/// client signs it; an owner that requires a delegation rejects calls without one.
#[allow(clippy::result_large_err)]
pub fn on_behalf_of<T>(
    mut request: tonic::Request<T>,
    actor: &str,
) -> Result<tonic::Request<T>, Status> {
    let actor = Uuid::parse_str(actor)
        .ok()
        .filter(|actor| !actor.is_nil())
        .ok_or_else(|| Status::invalid_argument("Invalid actor"))?;
    request.extensions_mut().insert(DelegatedActor { actor });
    Ok(request)
}

/// Build a request for the message that acts for the actor.
#[allow(clippy::result_large_err)]
pub fn request_for<T>(message: T, actor: &str) -> Result<tonic::Request<T>, Status> {
    on_behalf_of(tonic::Request::new(message), actor)
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Claims {
    v: u8,
    kid: String,
    iss: String,
    aud: String,
    mth: String,
    sub: Uuid,
    dig: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    op: Option<Uuid>,
    iat: i64,
    exp: i64,
    jti: String,
}

fn key_id(public_key: &[u8]) -> String {
    Sha256::digest(public_key)[..8]
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn body_digest(body: &[u8]) -> String {
    Base64UrlUnpadded::encode_string(&Sha256::digest(body))
}

fn signed_bytes(payload: &str) -> Vec<u8> {
    [SIGNATURE_CONTEXT, payload.as_bytes()].concat()
}

fn decode_key<const N: usize>(name: &str, value: &str) -> anyhow::Result<[u8; N]> {
    let bytes = Base64::decode_vec(value.trim())
        .map_err(|_| anyhow::anyhow!("{name} must be standard Base64"))?;
    bytes
        .try_into()
        .map_err(|_| anyhow::anyhow!("{name} must decode to {N} bytes"))
}

/// Gateway-held signing key. Only the edge can establish a user identity, so
/// it is the only issuer.
#[derive(Clone)]
pub struct ActorDelegationSigner {
    issuer: Workload,
    key_id: String,
    key: Arc<Ed25519KeyPair>,
}

impl std::fmt::Debug for ActorDelegationSigner {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ActorDelegationSigner")
            .field("issuer", &self.issuer)
            .field("key_id", &self.key_id)
            .finish_non_exhaustive()
    }
}

impl ActorDelegationSigner {
    pub fn from_seed(issuer: Workload, seed: &[u8; 32]) -> anyhow::Result<Self> {
        let key = Ed25519KeyPair::from_seed_unchecked(seed)
            .map_err(|_| anyhow::anyhow!("invalid actor delegation signing seed"))?;
        Ok(Self {
            issuer,
            key_id: key_id(key.public_key().as_ref()),
            key: Arc::new(key),
        })
    }

    /// Development key shared with [ActorDelegationVerifier::local_parity].
    pub fn local_parity(issuer: Workload) -> Self {
        Self::from_seed(issuer, &LOCAL_PARITY_SEED).expect("valid parity seed")
    }

    /// Load from [SIGNING_SEED_ENV]. Outside production a missing value
    /// selects the local parity key; production requires a real seed.
    pub fn from_env(is_production: bool, issuer: Workload) -> anyhow::Result<Self> {
        Self::from_lookup(is_production, issuer, crate::config::optional_env)
    }

    fn from_lookup(
        is_production: bool,
        issuer: Workload,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> anyhow::Result<Self> {
        match lookup(SIGNING_SEED_ENV) {
            Some(value) => {
                let seed = decode_key::<32>(SIGNING_SEED_ENV, &value)?;
                anyhow::ensure!(
                    !is_production || seed != LOCAL_PARITY_SEED,
                    "{SIGNING_SEED_ENV} must not use the local parity key in production"
                );
                Self::from_seed(issuer, &seed)
            }
            None if is_production => {
                anyhow::bail!("{SIGNING_SEED_ENV} is required in production")
            }
            None => Ok(Self::local_parity(issuer)),
        }
    }

    pub fn public_key(&self) -> [u8; 32] {
        self.key
            .public_key()
            .as_ref()
            .try_into()
            .expect("Ed25519 public key length")
    }

    pub fn key_id(&self) -> &str {
        &self.key_id
    }

    /// Issue a delegation for one request body. The time is Unix seconds.
    pub fn issue(
        &self,
        audience: Workload,
        method: &str,
        actor: Uuid,
        operation: Option<Uuid>,
        body: &[u8],
        now: i64,
    ) -> String {
        self.issue_with_lifetime(
            audience,
            method,
            actor,
            operation,
            body,
            now,
            LIFETIME_SECONDS,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn issue_with_lifetime(
        &self,
        audience: Workload,
        method: &str,
        actor: Uuid,
        operation: Option<Uuid>,
        body: &[u8],
        now: i64,
        lifetime: i64,
    ) -> String {
        let claims = Claims {
            v: VERSION,
            kid: self.key_id.clone(),
            iss: self.issuer.as_str().to_owned(),
            aud: audience.as_str().to_owned(),
            mth: method.to_owned(),
            sub: actor,
            dig: body_digest(body),
            op: operation,
            iat: now,
            exp: now + lifetime,
            jti: Uuid::new_v4().simple().to_string(),
        };
        let payload = Base64UrlUnpadded::encode_string(
            &serde_json::to_vec(&claims).expect("claims serialize"),
        );
        let signature = self.key.sign(&signed_bytes(&payload));
        format!(
            "{payload}.{}",
            Base64UrlUnpadded::encode_string(signature.as_ref())
        )
    }
}

/// Why a delegation was rejected. Codes are stable log values.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Denial {
    Missing,
    Malformed,
    UnknownKey,
    BadSignature,
    Expired,
    NotYetValid,
    LifetimeTooLong,
    Replayed,
    WrongIssuer,
    WrongAudience,
    WrongMethod,
    WrongBody,
    WrongOperation,
    ActorMismatch,
    BodyTooLarge,
    ReplayCacheFull,
}

impl Denial {
    pub fn code(self) -> &'static str {
        match self {
            Denial::Missing => "DELEGATION_MISSING",
            Denial::Malformed => "DELEGATION_MALFORMED",
            Denial::UnknownKey => "DELEGATION_UNKNOWN_KEY",
            Denial::BadSignature => "DELEGATION_BAD_SIGNATURE",
            Denial::Expired => "DELEGATION_EXPIRED",
            Denial::NotYetValid => "DELEGATION_NOT_YET_VALID",
            Denial::LifetimeTooLong => "DELEGATION_LIFETIME_TOO_LONG",
            Denial::Replayed => "DELEGATION_REPLAYED",
            Denial::WrongIssuer => "DELEGATION_WRONG_ISSUER",
            Denial::WrongAudience => "DELEGATION_WRONG_AUDIENCE",
            Denial::WrongMethod => "DELEGATION_WRONG_METHOD",
            Denial::WrongBody => "DELEGATION_WRONG_RESOURCE",
            Denial::WrongOperation => "DELEGATION_WRONG_OPERATION",
            Denial::ActorMismatch => "DELEGATION_ACTOR_MISMATCH",
            Denial::BodyTooLarge => "DELEGATION_BODY_TOO_LARGE",
            Denial::ReplayCacheFull => "DELEGATION_REPLAY_CACHE_FULL",
        }
    }

    /// Credential problems are unauthenticated; a valid delegation for a
    /// different call is permission denied.
    pub fn status(self) -> Status {
        match self {
            Denial::Missing
            | Denial::Malformed
            | Denial::UnknownKey
            | Denial::BadSignature
            | Denial::Expired
            | Denial::NotYetValid
            | Denial::LifetimeTooLong
            | Denial::Replayed => Status::unauthenticated("actor delegation required"),
            Denial::WrongIssuer
            | Denial::WrongAudience
            | Denial::WrongMethod
            | Denial::WrongBody
            | Denial::WrongOperation
            | Denial::ActorMismatch => {
                Status::permission_denied("actor delegation does not cover this request")
            }
            Denial::BodyTooLarge => Status::resource_exhausted("delegated request too large"),
            Denial::ReplayCacheFull => Status::unavailable("delegation check is saturated"),
        }
    }
}

/// What the owner observed about the request being verified.
pub struct Expected<'a> {
    pub caller: Workload,
    pub method: &'a str,
    pub body: &'a [u8],
    pub operation: Option<Uuid>,
}

/// A delegation that passed every check except actor binding and replay.
#[derive(Debug)]
pub struct Verified {
    pub actor: Uuid,
    jti: String,
    exp: i64,
}

/// Owner-held verification keys and replay record.
#[derive(Clone)]
pub struct ActorDelegationVerifier {
    audience: Workload,
    keys: Arc<Vec<(String, [u8; 32])>>,
    used: Arc<Mutex<HashMap<String, i64>>>,
}

impl std::fmt::Debug for ActorDelegationVerifier {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ActorDelegationVerifier")
            .field("audience", &self.audience)
            .field(
                "key_ids",
                &self.keys.iter().map(|(id, _)| id).collect::<Vec<_>>(),
            )
            .finish_non_exhaustive()
    }
}

impl ActorDelegationVerifier {
    pub fn new(audience: Workload, public_keys: &[[u8; 32]]) -> anyhow::Result<Self> {
        anyhow::ensure!(
            !public_keys.is_empty(),
            "at least one actor delegation verification key is required"
        );
        Ok(Self {
            audience,
            keys: Arc::new(public_keys.iter().map(|key| (key_id(key), *key)).collect()),
            used: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Verifier for [ActorDelegationSigner::local_parity].
    pub fn local_parity(audience: Workload) -> Self {
        let public = ActorDelegationSigner::local_parity(Workload::Gateway).public_key();
        Self::new(audience, &[public]).expect("one parity key")
    }

    /// Load from [VERIFYING_KEYS_ENV]. Outside production a missing value
    /// selects the local parity key; production requires real keys.
    pub fn from_env(is_production: bool, audience: Workload) -> anyhow::Result<Self> {
        Self::from_lookup(is_production, audience, crate::config::optional_env)
    }

    fn from_lookup(
        is_production: bool,
        audience: Workload,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> anyhow::Result<Self> {
        let Some(value) = lookup(VERIFYING_KEYS_ENV) else {
            anyhow::ensure!(
                !is_production,
                "{VERIFYING_KEYS_ENV} is required in production"
            );
            return Ok(Self::local_parity(audience));
        };
        let parity = ActorDelegationSigner::local_parity(Workload::Gateway).public_key();
        let keys = value
            .split(',')
            .filter(|key| !key.trim().is_empty())
            .map(|key| decode_key::<32>(VERIFYING_KEYS_ENV, key))
            .collect::<anyhow::Result<Vec<_>>>()?;
        anyhow::ensure!(
            !is_production || !keys.contains(&parity),
            "{VERIFYING_KEYS_ENV} must not include the local parity key in production"
        );
        Self::new(audience, &keys)
    }

    pub fn audience(&self) -> Workload {
        self.audience
    }

    /// Whether this verifier accepts delegations signed by the key. Used to
    /// refuse resource-evidence keys that equal any trusted actor key,
    /// including a previous key retained during rotation.
    pub fn trusts(&self, public_key: &[u8; 32]) -> bool {
        self.keys.iter().any(|(_, key)| key == public_key)
    }

    /// Check signature, issuer, audience, method, body digest, operation and
    /// time window. The time is Unix seconds.
    pub fn verify(
        &self,
        token: &str,
        expected: &Expected<'_>,
        now: i64,
    ) -> Result<Verified, Denial> {
        let (payload, signature) = token.split_once('.').ok_or(Denial::Malformed)?;
        let signature = Base64UrlUnpadded::decode_vec(signature).map_err(|_| Denial::Malformed)?;
        let claims: Claims = Base64UrlUnpadded::decode_vec(payload)
            .ok()
            .and_then(|json| serde_json::from_slice(&json).ok())
            .ok_or(Denial::Malformed)?;
        if claims.v != VERSION || claims.jti.len() < 16 || claims.jti.len() > 64 {
            return Err(Denial::Malformed);
        }
        let (_, key) = self
            .keys
            .iter()
            .find(|(id, _)| *id == claims.kid)
            .ok_or(Denial::UnknownKey)?;
        UnparsedPublicKey::new(&ED25519, key)
            .verify(&signed_bytes(payload), &signature)
            .map_err(|_| Denial::BadSignature)?;
        if claims.iss != expected.caller.as_str() {
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

    /// Accept a verified token ID once. Call only after every other check
    /// passed, so a rejected request does not burn a valid token.
    pub fn consume(&self, verified: &Verified, now: i64) -> Result<(), Denial> {
        let mut used = self
            .used
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if used.len() >= REPLAY_CACHE_CAPACITY {
            used.retain(|_, exp| *exp + CLOCK_SKEW_SECONDS >= now);
        }
        if used.contains_key(&verified.jti) {
            return Err(Denial::Replayed);
        }
        if used.len() >= REPLAY_CACHE_CAPACITY {
            return Err(Denial::ReplayCacheFull);
        }
        used.insert(verified.jti.clone(), verified.exp);
        Ok(())
    }
}

fn read_varint(input: &mut &[u8]) -> Option<u64> {
    let mut value = 0u64;
    for shift in (0..64).step_by(7) {
        let (&byte, rest) = input.split_first()?;
        *input = rest;
        value |= u64::from(byte & 0x7f) << shift;
        if byte & 0x80 == 0 {
            return Some(value);
        }
    }
    None
}

/// Bytes of the length-delimited field at the path of protobuf field
/// numbers, outermost first. A malformed message, a repeated occurrence, or a
/// value of another wire type at a path step is rejected.
fn field_bytes<'a>(message: &'a [u8], path: &[u32]) -> Result<Option<&'a [u8]>, Denial> {
    let Some((&field, rest_path)) = path.split_first() else {
        return Ok(Some(message));
    };
    let field = u64::from(field);
    let mut input = message;
    let mut found = None;
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
        if number == field {
            match (value, found) {
                (Some(value), None) => found = Some(value),
                _ => return Err(Denial::ActorMismatch),
            }
        }
    }
    match found {
        Some(value) => field_bytes(value, rest_path),
        None => Ok(None),
    }
}

/// The protobuf message inside one uncompressed unary gRPC frame.
fn unary_message(body: &[u8]) -> Result<&[u8], Denial> {
    let (header, message) = body.split_at_checked(5).ok_or(Denial::Malformed)?;
    let length = u32::from_be_bytes([header[1], header[2], header[3], header[4]]);
    if header[0] != 0 || usize::try_from(length).ok() != Some(message.len()) {
        return Err(Denial::Malformed);
    }
    Ok(message)
}

/// Every declared actor field that is present must name the actor, and at
/// least one must be present.
fn bind_actor(body: &[u8], paths: &[&[u32]], actor: Uuid) -> Result<(), Denial> {
    check_actor_fields(unary_message(body)?, paths, actor)
}

/// Check the actor fields of one encoded protobuf message: every declared
/// field that is present must name the actor, and at least one must be
/// present. Services use this to test their tables against generated types.
pub fn check_actor_fields(message: &[u8], paths: &[&[u32]], actor: Uuid) -> Result<(), Denial> {
    let mut present = 0;
    for path in paths {
        if let Some(value) = field_bytes(message, path)? {
            let named = std::str::from_utf8(value)
                .ok()
                .and_then(|value| Uuid::parse_str(value).ok())
                .ok_or(Denial::ActorMismatch)?;
            if named != actor {
                return Err(Denial::ActorMismatch);
            }
            present += 1;
        }
    }
    if present == 0 {
        return Err(Denial::ActorMismatch);
    }
    Ok(())
}

fn now() -> i64 {
    chrono::Utc::now().timestamp()
}

/// Owner side: verify the delegation on a request for an actor-bound method.
/// On success the request is rebuilt over its buffered body and carries
/// [VerifiedActor].
pub(crate) async fn verify_request<B>(
    request: http::Request<B>,
    caller: Workload,
    actor_fields: &[&[u32]],
    verifier: &ActorDelegationVerifier,
) -> Result<http::Request<Body>, Denial>
where
    B: http_body::Body<Data = tonic::codegen::Bytes> + Send + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    verify_request_kind(request, caller, actor_fields, verifier, false).await
}

/// Forwarded reads require a child proof even when their target is a resource
/// rather than an actor field. There is no direct-token or workload-only fallback.
pub(crate) async fn verify_forwarded_request<B>(
    request: http::Request<B>,
    caller: Workload,
    actor_fields: &[&[u32]],
    verifier: &ActorDelegationVerifier,
) -> Result<http::Request<Body>, Denial>
where
    B: http_body::Body<Data = tonic::codegen::Bytes> + Send + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    verify_request_kind(request, caller, actor_fields, verifier, true).await
}

async fn verify_request_kind<B>(
    request: http::Request<B>,
    caller: Workload,
    actor_fields: &[&[u32]],
    verifier: &ActorDelegationVerifier,
    forwarded: bool,
) -> Result<http::Request<Body>, Denial>
where
    B: http_body::Body<Data = tonic::codegen::Bytes> + Send + 'static,
    B::Error: Into<Box<dyn std::error::Error + Send + Sync>>,
{
    let (mut parts, body) = request.into_parts();
    let single = |name: &str| -> Result<Option<String>, Denial> {
        let mut values = parts.headers.get_all(name).iter();
        let value = values.next();
        if values.next().is_some() {
            return Err(Denial::Malformed);
        }
        value
            .map(|value| {
                value
                    .to_str()
                    .map(str::to_owned)
                    .map_err(|_| Denial::Malformed)
            })
            .transpose()
    };
    let token = single(HEADER)?.ok_or(Denial::Missing)?;
    if token.len() > 4096 {
        return Err(Denial::BodyTooLarge);
    }
    let operation = single(OPERATION_HEADER)?
        .map(|value| Uuid::parse_str(&value).map_err(|_| Denial::Malformed))
        .transpose()?;
    let bytes = Limited::new(body, MAX_BODY_BYTES)
        .collect()
        .await
        .map_err(|_| Denial::BodyTooLarge)?
        .to_bytes();
    let now = now();
    let expected = Expected {
        caller,
        method: parts.uri.path(),
        body: &bytes,
        operation,
    };
    let verified = if forwarded {
        verifier.verify_child(&token, &expected, now)?
    } else {
        verifier.verify(&token, &expected, now)?
    };
    if forwarded && actor_fields.is_empty() {
        // Still require exactly one uncompressed unary frame. The target is
        // covered by the issuer's resource check and the exact body digest.
        unary_message(&bytes)?;
    } else {
        bind_actor(&bytes, actor_fields, verified.actor)?;
    }
    verifier.consume(&verified, now)?;
    parts.extensions.insert(VerifiedActor(verified.actor));
    if !forwarded && verifier.audience() == Workload::Post {
        parts
            .extensions
            .insert(exchange::VerifiedParentAuthority::new(
                token,
                parts.uri.path().to_owned(),
                bytes.to_vec(),
                operation,
                verified.actor,
                verified.exp,
            ));
    }
    Ok(http::Request::from_parts(
        parts,
        Body::new(Full::new(bytes)),
    ))
}

/// Gateway side: signs every outbound request that carries [DelegatedActor].
/// Wrap the transport channel with it.
#[derive(Clone)]
pub struct ActorDelegationClient<S> {
    inner: S,
    signer: ActorDelegationSigner,
    audience: Workload,
}

impl<S> ActorDelegationClient<S> {
    pub fn new(inner: S, signer: ActorDelegationSigner, audience: Workload) -> Self {
        Self {
            inner,
            signer,
            audience,
        }
    }
}

type BoxedResponse<E> = Pin<Box<dyn Future<Output = Result<http::Response<Body>, E>> + Send>>;

impl<S> tower::Service<http::Request<Body>> for ActorDelegationClient<S>
where
    S: tower::Service<http::Request<Body>, Response = http::Response<Body>>
        + Clone
        + Send
        + 'static,
    S::Future: Send + 'static,
    S::Error: Send + 'static,
{
    type Response = http::Response<Body>;
    type Error = S::Error;
    type Future = BoxedResponse<S::Error>;

    fn poll_ready(&mut self, context: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        self.inner.poll_ready(context)
    }

    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        let Some(delegated) = request.extensions().get::<DelegatedActor>().copied() else {
            return Box::pin(self.inner.call(request));
        };
        // The ready service moves into the future; a fresh clone stays behind
        // for the next poll_ready.
        let clone = self.inner.clone();
        let mut inner = std::mem::replace(&mut self.inner, clone);
        let signer = self.signer.clone();
        let audience = self.audience;
        Box::pin(async move {
            let (mut parts, body) = request.into_parts();
            let operation = parts
                .headers
                .get(OPERATION_HEADER)
                .and_then(|value| value.to_str().ok())
                .and_then(|value| Uuid::parse_str(value).ok());
            let Ok(collected) = Limited::new(body, MAX_BODY_BYTES).collect().await else {
                return Ok(Status::internal("delegated request could not be signed").into_http());
            };
            let bytes = collected.to_bytes();
            let token = signer.issue(
                audience,
                parts.uri.path(),
                delegated.actor,
                operation,
                &bytes,
                now(),
            );
            let Ok(value) = http::HeaderValue::from_str(&token) else {
                return Ok(Status::internal("delegated request could not be signed").into_http());
            };
            parts.headers.insert(HEADER, value);
            inner
                .call(http::Request::from_parts(
                    parts,
                    Body::new(Full::new(bytes)),
                ))
                .await
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const METHOD: &str = "/post.v1.PostService/CreatePost";

    fn frame(message: &[u8]) -> Vec<u8> {
        let mut body = vec![0];
        body.extend_from_slice(&u32::try_from(message.len()).unwrap().to_be_bytes());
        body.extend_from_slice(message);
        body
    }

    fn string_field(number: u32, value: &[u8]) -> Vec<u8> {
        let mut bytes = vec![
            u8::try_from((number << 3) | 2).unwrap(),
            u8::try_from(value.len()).unwrap(),
        ];
        bytes.extend_from_slice(value);
        bytes
    }

    fn uuid_field(number: u32, value: Uuid) -> Vec<u8> {
        string_field(number, value.to_string().as_bytes())
    }

    fn setup() -> (
        ActorDelegationSigner,
        ActorDelegationVerifier,
        Uuid,
        Vec<u8>,
    ) {
        let signer = ActorDelegationSigner::from_seed(Workload::Gateway, &[7; 32]).unwrap();
        let verifier =
            ActorDelegationVerifier::new(Workload::Post, &[signer.public_key()]).unwrap();
        let actor = Uuid::new_v4();
        let body = frame(&uuid_field(1, actor));
        (signer, verifier, actor, body)
    }

    fn expected(body: &[u8]) -> Expected<'_> {
        Expected {
            caller: Workload::Gateway,
            method: METHOD,
            body,
            operation: None,
        }
    }

    #[test]
    fn a_valid_delegation_binds_the_actor_once() {
        let (signer, verifier, actor, body) = setup();
        let token = signer.issue(Workload::Post, METHOD, actor, None, &body, 1_000);
        let verified = verifier.verify(&token, &expected(&body), 1_000).unwrap();
        assert_eq!(verified.actor, actor);
        bind_actor(&body, &[&[1]], verified.actor).unwrap();
        verifier.consume(&verified, 1_000).unwrap();
        let again = verifier.verify(&token, &expected(&body), 1_001).unwrap();
        assert_eq!(verifier.consume(&again, 1_001), Err(Denial::Replayed));
    }

    #[test]
    fn spoofed_wrongly_bound_and_expired_delegations_are_rejected() {
        let (signer, verifier, actor, body) = setup();
        let token = signer.issue(Workload::Post, METHOD, actor, None, &body, 1_000);
        let check = |token: &str, expected: &Expected<'_>, now| {
            verifier.verify(token, expected, now).map(|v| v.actor)
        };
        // A key the owner does not trust cannot mint a delegation.
        let forger = ActorDelegationSigner::from_seed(Workload::Gateway, &[9; 32]).unwrap();
        let forged = forger.issue(Workload::Post, METHOD, actor, None, &body, 1_000);
        assert_eq!(
            check(&forged, &expected(&body), 1_000),
            Err(Denial::UnknownKey)
        );
        // A trusted key id over altered claims fails the signature.
        let (payload, signature) = token.split_once('.').unwrap();
        let mut claims: serde_json::Value =
            serde_json::from_slice(&Base64UrlUnpadded::decode_vec(payload).unwrap()).unwrap();
        claims["sub"] = serde_json::json!(Uuid::new_v4());
        let tampered = format!(
            "{}.{signature}",
            Base64UrlUnpadded::encode_string(&serde_json::to_vec(&claims).unwrap())
        );
        assert_eq!(
            check(&tampered, &expected(&body), 1_000),
            Err(Denial::BadSignature)
        );
        assert_eq!(
            check("garbage", &expected(&body), 1_000),
            Err(Denial::Malformed)
        );
        // Wrong caller, audience, method, body and operation.
        let mut wrong = expected(&body);
        wrong.caller = Workload::Post;
        assert_eq!(check(&token, &wrong, 1_000), Err(Denial::WrongIssuer));
        let other_audience = signer.issue(Workload::Friend, METHOD, actor, None, &body, 1_000);
        assert_eq!(
            check(&other_audience, &expected(&body), 1_000),
            Err(Denial::WrongAudience)
        );
        let mut wrong = expected(&body);
        wrong.method = "/post.v1.PostService/DeletePost";
        assert_eq!(check(&token, &wrong, 1_000), Err(Denial::WrongMethod));
        let other_body = frame(&uuid_field(1, Uuid::new_v4()));
        assert_eq!(
            check(&token, &expected(&other_body), 1_000),
            Err(Denial::WrongBody)
        );
        let mut wrong = expected(&body);
        wrong.operation = Some(Uuid::new_v4());
        assert_eq!(check(&token, &wrong, 1_000), Err(Denial::WrongOperation));
        // Time window.
        let late = 1_000 + LIFETIME_SECONDS + CLOCK_SKEW_SECONDS + 1;
        assert_eq!(check(&token, &expected(&body), late), Err(Denial::Expired));
        let early = 1_000 - CLOCK_SKEW_SECONDS - 1;
        assert_eq!(
            check(&token, &expected(&body), early),
            Err(Denial::NotYetValid)
        );
        let long = signer.issue_with_lifetime(
            Workload::Post,
            METHOD,
            actor,
            None,
            &body,
            1_000,
            MAX_LIFETIME_SECONDS + 1,
        );
        assert_eq!(
            check(&long, &expected(&body), 1_000),
            Err(Denial::LifetimeTooLong)
        );
    }

    #[test]
    fn actor_binding_checks_every_declared_field() {
        let actor = Uuid::new_v4();
        let other = Uuid::new_v4();
        let both = [uuid_field(1, actor), uuid_field(2, actor)].concat();
        assert_eq!(bind_actor(&frame(&both), &[&[1], &[2]], actor), Ok(()));
        let mixed = [uuid_field(1, actor), uuid_field(2, other)].concat();
        assert_eq!(
            bind_actor(&frame(&mixed), &[&[1], &[2]], actor),
            Err(Denial::ActorMismatch)
        );
        // Missing actor field, duplicated field, and a non-UUID value.
        assert_eq!(
            bind_actor(&frame(&string_field(3, b"x")), &[&[1]], actor),
            Err(Denial::ActorMismatch)
        );
        let duplicate = [uuid_field(1, actor), uuid_field(1, actor)].concat();
        assert_eq!(
            bind_actor(&frame(&duplicate), &[&[1]], actor),
            Err(Denial::ActorMismatch)
        );
        assert_eq!(
            bind_actor(&frame(&string_field(1, b"me")), &[&[1]], actor),
            Err(Denial::ActorMismatch)
        );
        // Nested oneof command: field 4 holds a message whose field 2 is the actor.
        let nested = string_field(4, &uuid_field(2, actor));
        assert_eq!(
            bind_actor(&frame(&nested), &[&[1, 1], &[4, 2]], actor),
            Ok(())
        );
        // Compressed or truncated frames are malformed.
        let mut compressed = frame(&both);
        compressed[0] = 1;
        assert_eq!(
            bind_actor(&compressed, &[&[1]], actor),
            Err(Denial::Malformed)
        );
        assert_eq!(
            bind_actor(&frame(&both)[..8], &[&[1]], actor),
            Err(Denial::Malformed)
        );
    }

    #[test]
    fn production_rejects_missing_and_parity_keys() {
        let none = |_: &str| None;
        assert!(ActorDelegationSigner::from_lookup(true, Workload::Gateway, none).is_err());
        assert!(ActorDelegationVerifier::from_lookup(true, Workload::Post, none).is_err());
        let parity_seed = Base64::encode_string(&LOCAL_PARITY_SEED);
        let parity_public = Base64::encode_string(
            &ActorDelegationSigner::local_parity(Workload::Gateway).public_key(),
        );
        assert!(
            ActorDelegationSigner::from_lookup(true, Workload::Gateway, |_| Some(
                parity_seed.clone()
            ))
            .is_err()
        );
        assert!(
            ActorDelegationVerifier::from_lookup(true, Workload::Post, |_| Some(
                parity_public.clone()
            ))
            .is_err()
        );
        let real = Base64::encode_string(&[3u8; 32]);
        let signer =
            ActorDelegationSigner::from_lookup(true, Workload::Gateway, |_| Some(real.clone()))
                .unwrap();
        let keys = format!(
            "{},{}",
            Base64::encode_string(&signer.public_key()),
            Base64::encode_string(&[4u8; 32])
        );
        let verifier =
            ActorDelegationVerifier::from_lookup(true, Workload::Post, |_| Some(keys.clone()))
                .unwrap();
        assert_eq!(verifier.keys.len(), 2);
        assert!(ActorDelegationSigner::from_lookup(false, Workload::Gateway, none).is_ok());
        assert!(
            ActorDelegationSigner::from_lookup(false, Workload::Gateway, |_| Some("short".into()))
                .is_err()
        );
    }
}
