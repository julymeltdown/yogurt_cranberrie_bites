//! Closed Feed command bindings and independent Concert-to-Post mutation signing.
//! These bindings authenticate intent; retained owner guards and the actual
//! effects transaction registration remain mandatory at the Post producer.
use crate::{
    actor_delegation::{ActorDelegationClient, ActorDelegationSigner, ActorDelegationVerifier},
    grpc_auth::Workload,
};
use tonic::Status;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FeedCommandRoute {
    CreatePost,
    EditPost(Uuid),
    DeletePost(Uuid, i64),
    CreateComment(Uuid),
    EditComment(Uuid),
    DeleteComment(Uuid, i64),
    Like(Uuid),
    Unlike(Uuid),
}
impl FeedCommandRoute {
    pub fn operation(self) -> &'static str {
        match self {
            Self::CreatePost => "createConcertPost",
            Self::EditPost(_) => "editConcertPost",
            Self::DeletePost(..) => "deleteConcertPost",
            Self::CreateComment(_) => "createConcertComment",
            Self::EditComment(_) => "editConcertComment",
            Self::DeleteComment(..) => "deleteConcertComment",
            Self::Like(_) => "likeConcertPost",
            Self::Unlike(_) => "unlikeConcertPost",
        }
    }
    pub fn target(self) -> Option<Uuid> {
        match self {
            Self::CreatePost => None,
            Self::EditPost(id)
            | Self::DeletePost(id, _)
            | Self::CreateComment(id)
            | Self::EditComment(id)
            | Self::DeleteComment(id, _)
            | Self::Like(id)
            | Self::Unlike(id) => Some(id),
        }
    }
    pub fn safety(self) -> bool {
        matches!(
            self,
            Self::DeletePost(..) | Self::DeleteComment(..) | Self::Unlike(_)
        )
    }
    pub fn bodyless(self) -> bool {
        self.safety()
    }
    pub fn revision(self) -> Option<i64> {
        match self {
            Self::DeletePost(_, n) | Self::DeleteComment(_, n) => Some(n),
            _ => None,
        }
    }
    /// The route is derived from the exact signed method/target. Query values are
    /// decoded once and retained as typed values; the original target is unchanged.
    pub fn parse(event: Uuid, method: &str, target: &str) -> Result<Self, Status> {
        if event.is_nil()
            || target.len() > 4096
            || target.contains('#')
            || target.chars().any(char::is_control)
        {
            return Err(invalid());
        }
        let prefix = format!("/api/concert/events/{event}/");
        let suffix = target.strip_prefix(&prefix).ok_or_else(invalid)?;
        let (path, query) = suffix
            .split_once('?')
            .map_or((suffix, None), |(p, q)| (p, Some(q)));
        let parts: Vec<_> = path.split('/').collect();
        let route = match (method, parts.as_slice()) {
            ("POST", ["posts"]) => Self::CreatePost,
            ("PATCH", ["posts", id]) => Self::EditPost(id_value(id)?),
            ("DELETE", ["posts", id]) => Self::DeletePost(id_value(id)?, revision(query)?),
            ("POST", ["posts", id, "comments"]) => Self::CreateComment(id_value(id)?),
            ("PATCH", ["comments", id]) => Self::EditComment(id_value(id)?),
            ("DELETE", ["comments", id]) => Self::DeleteComment(id_value(id)?, revision(query)?),
            ("PUT", ["posts", id, "reaction"]) => Self::Like(id_value(id)?),
            ("DELETE", ["posts", id, "reaction"]) => Self::Unlike(id_value(id)?),
            _ => return Err(invalid()),
        };
        if query.is_some() && route.revision().is_none() {
            return Err(invalid());
        }
        Ok(route)
    }
    /// Validate wire shape without rewriting the original signed body/target.
    pub fn validate_body(self, content_type: &str, bytes: &[u8]) -> Result<(), Status> {
        if self.bodyless() {
            if !bytes.is_empty() || !content_type.is_empty() {
                return Err(invalid());
            }
        } else if bytes.is_empty()
            || bytes.len() > 65_536
            || content_type.split(';').next().map(str::trim) != Some("application/json")
        {
            return Err(invalid());
        }
        Ok(())
    }
}
fn invalid() -> Status {
    Status::invalid_argument("Invalid Feed command binding")
}
fn id_value(value: &str) -> Result<Uuid, Status> {
    Uuid::parse_str(value)
        .ok()
        .filter(|v| !v.is_nil() && v.to_string() == value)
        .ok_or_else(invalid)
}
fn decode(value: &str) -> Result<String, Status> {
    let bytes = value.as_bytes();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] == b'+' {
            return Err(invalid());
        }
        if bytes[i] == b'%' {
            if i + 2 >= bytes.len()
                || !bytes[i + 1].is_ascii_hexdigit()
                || !bytes[i + 2].is_ascii_hexdigit()
            {
                return Err(invalid());
            }
            i += 2;
        }
        i += 1;
    }
    percent_encoding::percent_decode_str(value)
        .decode_utf8()
        .map(String::from)
        .map_err(|_| invalid())
}
fn revision(query: Option<&str>) -> Result<i64, Status> {
    let (name, value) = query
        .ok_or_else(invalid)?
        .split_once('=')
        .ok_or_else(invalid)?;
    let (name, value) = (decode(name)?, decode(value)?);
    if name != "expected_revision"
        || value.is_empty()
        || value.starts_with('0')
        || !value.bytes().all(|b| b.is_ascii_digit())
    {
        return Err(invalid());
    }
    value
        .parse::<i64>()
        .ok()
        .filter(|n| (1..=9_007_199_254_740_991).contains(n))
        .ok_or_else(invalid)
}

pub const SIGNING_SEED_ENV: &str = "CONCERT_POST_COMMAND_SIGNING_SEED";
pub const VERIFYING_KEYS_ENV: &str = "CONCERT_POST_COMMAND_VERIFYING_KEYS";
#[derive(Clone)]
pub struct FeedCommandSigner(ActorDelegationSigner);
pub struct FeedCommandVerifier {
    verifier: ActorDelegationVerifier,
    keys: Vec<[u8; 32]>,
}
impl std::fmt::Debug for FeedCommandSigner {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FeedCommandSigner([REDACTED])")
    }
}
impl std::fmt::Debug for FeedCommandVerifier {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("FeedCommandVerifier([REDACTED])")
    }
}
fn independent(key: &[u8; 32], others: &ActorDelegationVerifier) -> anyhow::Result<()> {
    anyhow::ensure!(
        !others.trusts(key)
            && *key != ActorDelegationSigner::local_parity(Workload::Gateway).public_key(),
        "Feed command keys must be independent of Gateway/parity keys"
    );
    Ok(())
}
impl FeedCommandSigner {
    pub fn from_seed(seed: &[u8; 32], gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        anyhow::ensure!(*seed != [0; 32], "Feed command seed must be nonzero");
        let signer = ActorDelegationSigner::from_seed(Workload::Concert, seed)?;
        independent(&signer.public_key(), gateway)?;
        Ok(Self(signer))
    }
    pub fn from_env(gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        let value = std::env::var(SIGNING_SEED_ENV)
            .map_err(|_| anyhow::anyhow!("{SIGNING_SEED_ENV} is required"))?;
        let seed = crate::actor_delegation::decode_key::<32>(SIGNING_SEED_ENV, &value)?;
        Self::from_seed(&seed, gateway)
    }
    pub fn public_key(&self) -> [u8; 32] {
        self.0.public_key()
    }
    pub fn client<S>(&self, inner: S) -> ActorDelegationClient<S> {
        ActorDelegationClient::new(inner, self.0.clone(), Workload::Post)
    }
}
impl FeedCommandVerifier {
    pub fn new(keys: &[[u8; 32]], gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        anyhow::ensure!(
            gateway.audience() == Workload::Post,
            "Feed command verifier requires Post audience"
        );
        anyhow::ensure!(!keys.is_empty(), "Feed command keys are empty");
        for key in keys {
            independent(key, gateway)?;
        }
        Ok(Self {
            verifier: ActorDelegationVerifier::new(Workload::Post, keys)?,
            keys: keys.to_vec(),
        })
    }
    pub fn from_env(gateway: &ActorDelegationVerifier) -> anyhow::Result<Self> {
        let raw = std::env::var(VERIFYING_KEYS_ENV)
            .map_err(|_| anyhow::anyhow!("{VERIFYING_KEYS_ENV} is required"))?;
        let keys = raw
            .split(',')
            .map(|v| crate::actor_delegation::decode_key::<32>(VERIFYING_KEYS_ENV, v.trim()))
            .collect::<anyhow::Result<Vec<_>>>()?;
        Self::new(&keys, gateway)
    }
    pub fn require_independent(&self, reads: &ActorDelegationVerifier) -> anyhow::Result<()> {
        anyhow::ensure!(
            self.keys.iter().all(|key| !reads.trusts(key)),
            "Feed command verifying keys must be independent of qualified read keys"
        );
        Ok(())
    }
    pub fn require_independent_key(&self, callback: &[u8; 32]) -> anyhow::Result<()> {
        anyhow::ensure!(
            !self.keys.contains(callback),
            "Feed command verifying keys must be independent of callback keys"
        );
        Ok(())
    }
    pub fn into_inner(self) -> ActorDelegationVerifier {
        self.verifier
    }
}
