//! Distinct Post-owned resource attestations. This key cannot mint a Gateway
//! actor token. Only an already verified Post parent context can be attested.
use super::*;

const CONTEXT: &[u8] = b"port-rust owned post resources v1\n";
const MAX_RESOURCE_TOKEN_BYTES: usize = 32 * 1024;
/// Post signing seed for resource evidence: standard Base64 of 32 random bytes.
pub const RESOURCE_EVIDENCE_SIGNING_SEED_ENV: &str = "RESOURCE_EVIDENCE_SIGNING_SEED";
/// Gateway verification keys for Post resource evidence: comma-separated
/// standard Base64 Ed25519 public keys, current first and, during rotation, the
/// previous one.
pub const RESOURCE_EVIDENCE_VERIFYING_KEYS_ENV: &str = "RESOURCE_EVIDENCE_VERIFYING_KEYS";
/// Non-secret development key material. Production configuration rejects it.
const LOCAL_PARITY_RESOURCE_SEED: [u8; 32] = *b"local-parity-resource-evidence-0";

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ResourceClaims {
    v: u8,
    kid: String,
    iss: String,
    par: String,
    wit: Uuid,
    posts: Vec<[Uuid; 2]>,
    missing: Vec<Uuid>,
    iat: i64,
    exp: i64,
}

/// Owning Post's resource key is distinct from Gateway's user-authority key.
#[derive(Clone)]
pub struct ResourceEvidenceSigner {
    key: Arc<Ed25519KeyPair>,
    key_id: String,
}
impl ResourceEvidenceSigner {
    pub fn from_seed(seed: &[u8; 32]) -> anyhow::Result<Self> {
        let key = Ed25519KeyPair::from_seed_unchecked(seed)
            .map_err(|_| anyhow::anyhow!("invalid resource evidence seed"))?;
        let key_id = key_id(key.public_key().as_ref());
        Ok(Self {
            key: Arc::new(key),
            key_id,
        })
    }
    /// Development key shared with [ResourceEvidenceVerifier::local_parity].
    pub fn local_parity() -> Self {
        Self::from_seed(&LOCAL_PARITY_RESOURCE_SEED).expect("valid parity seed")
    }
    /// Load from [RESOURCE_EVIDENCE_SIGNING_SEED_ENV]. Outside production a
    /// missing value selects the local parity key; production requires a real
    /// seed that is neither resource nor actor-delegation parity material.
    pub fn from_env(is_production: bool) -> anyhow::Result<Self> {
        Self::from_lookup(is_production, crate::config::optional_env)
    }
    fn from_lookup(
        is_production: bool,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> anyhow::Result<Self> {
        let name = RESOURCE_EVIDENCE_SIGNING_SEED_ENV;
        match lookup(name) {
            Some(value) => {
                let seed = crate::actor_delegation::decode_key::<32>(name, &value)?;
                anyhow::ensure!(
                    !is_production
                        || (seed != LOCAL_PARITY_RESOURCE_SEED
                            && seed != crate::actor_delegation::LOCAL_PARITY_SEED),
                    "{name} must not use local parity key material in production"
                );
                Self::from_seed(&seed)
            }
            None if is_production => anyhow::bail!("{name} is required in production"),
            None => Ok(Self::local_parity()),
        }
    }
    pub fn public_key(&self) -> [u8; 32] {
        self.key
            .public_key()
            .as_ref()
            .try_into()
            .expect("Ed25519 key length")
    }
    /// Only an independent owner lookup of the already published stage supplies
    /// these refreshed rows. This fresh proof is not handler admission.
    pub fn attest_resource_parent(
        &self,
        parent: &VerifiedResourceParent,
        witness_id: Uuid,
        posts: &[PostAuthor],
        missing: &[Uuid],
        now: i64,
    ) -> Result<String, Denial> {
        let observed: std::collections::HashSet<_> = posts
            .iter()
            .map(|row| row.post_id)
            .chain(missing.iter().copied())
            .collect();
        match parent.scope() {
            ResourceParentScope::Posts(expected)
                if observed != expected.iter().copied().collect() =>
            {
                return Err(Denial::WrongBody)
            }
            ResourceParentScope::Discovery if !missing.is_empty() => return Err(Denial::WrongBody),
            _ => {}
        }
        self.attest_binding(
            parent.digest(),
            parent.expires_at(),
            witness_id,
            posts,
            missing,
            now,
        )
    }

    /// Rows must come from the owning repository under the verified root query.
    /// SQL/projection provenance remains the producer's acceptance requirement.
    pub fn attest(
        &self,
        parent: &VerifiedParentAuthority,
        witness_id: Uuid,
        posts: &[PostAuthor],
        missing: &[Uuid],
        now: i64,
    ) -> Result<String, Denial> {
        self.attest_binding(
            &parent.parent_digest(),
            parent.expires_at,
            witness_id,
            posts,
            missing,
            now,
        )
    }
    fn attest_binding(
        &self,
        parent_digest: &[u8; 32],
        parent_expiry: i64,
        witness_id: Uuid,
        posts: &[PostAuthor],
        missing: &[Uuid],
        now: i64,
    ) -> Result<String, Denial> {
        if witness_id.is_nil() {
            return Err(Denial::WrongBody);
        }
        if now >= parent_expiry {
            return Err(Denial::Expired);
        }
        if posts.len() > 100 || missing.len() > 100 {
            return Err(Denial::BodyTooLarge);
        }
        let mut seen = std::collections::HashSet::new();
        for post in posts {
            if post.post_id.is_nil() || post.author_id.is_nil() || !seen.insert(post.post_id) {
                return Err(Denial::WrongBody);
            }
        }
        for id in missing {
            if id.is_nil() || !seen.insert(*id) {
                return Err(Denial::WrongBody);
            }
        }
        let claims = ResourceClaims {
            v: 1,
            kid: self.key_id.clone(),
            iss: Workload::Post.as_str().into(),
            par: Base64UrlUnpadded::encode_string(parent_digest),
            wit: witness_id,
            posts: posts
                .iter()
                .map(|row| [row.post_id, row.author_id])
                .collect(),
            missing: missing.to_vec(),
            iat: now,
            exp: parent_expiry.min(now + LIFETIME_SECONDS),
        };
        let payload = Base64UrlUnpadded::encode_string(
            &serde_json::to_vec(&claims).expect("resource claims serialize"),
        );
        let signature = self.key.sign(&signed(&payload));
        Ok(format!(
            "{payload}.{}",
            Base64UrlUnpadded::encode_string(signature.as_ref())
        ))
    }
}

#[derive(Clone)]
pub struct ResourceEvidenceVerifier {
    keys: Arc<Vec<(String, [u8; 32])>>,
}
impl ResourceEvidenceVerifier {
    pub fn new(public_keys: &[[u8; 32]]) -> anyhow::Result<Self> {
        anyhow::ensure!(!public_keys.is_empty(), "resource owner keys are required");
        Ok(Self {
            keys: Arc::new(public_keys.iter().map(|key| (key_id(key), *key)).collect()),
        })
    }
    /// Verifier for [ResourceEvidenceSigner::local_parity].
    pub fn local_parity() -> Self {
        Self::new(&[ResourceEvidenceSigner::local_parity().public_key()]).expect("one parity key")
    }
    /// Load from [RESOURCE_EVIDENCE_VERIFYING_KEYS_ENV]. Outside production a
    /// missing value selects the local parity key; production requires real
    /// keys and rejects the parity key.
    pub fn from_env(is_production: bool) -> anyhow::Result<Self> {
        Self::from_lookup(is_production, crate::config::optional_env)
    }
    fn from_lookup(
        is_production: bool,
        lookup: impl Fn(&str) -> Option<String>,
    ) -> anyhow::Result<Self> {
        let name = RESOURCE_EVIDENCE_VERIFYING_KEYS_ENV;
        let Some(value) = lookup(name) else {
            anyhow::ensure!(!is_production, "{name} is required in production");
            return Ok(Self::local_parity());
        };
        let keys = value
            .split(',')
            .filter(|key| !key.trim().is_empty())
            .map(|key| crate::actor_delegation::decode_key::<32>(name, key))
            .collect::<anyhow::Result<Vec<_>>>()?;
        let parity = ResourceEvidenceSigner::local_parity().public_key();
        anyhow::ensure!(
            !is_production || !keys.contains(&parity),
            "{name} must not include the local parity key in production"
        );
        Self::new(&keys)
    }
    /// Whether this verifier would accept evidence signed by the key. Used to
    /// refuse a configuration that reuses a user-authority key for resources.
    pub fn trusts(&self, public_key: &[u8; 32]) -> bool {
        self.keys.iter().any(|(_, key)| key == public_key)
    }
    /// Whether any resource key is also trusted for actor authority. A shared
    /// key would let the resource signer mint user authority, so composition
    /// must refuse it, including for keys kept during a rotation.
    pub fn shares_key_with(
        &self,
        actors: &crate::actor_delegation::ActorDelegationVerifier,
    ) -> bool {
        self.keys.iter().any(|(_, key)| actors.trusts(key))
    }
    pub(super) fn verify(
        &self,
        token: &str,
        parent_digest: &str,
        witness_id: Uuid,
        now: i64,
    ) -> Result<ResourceResolution, Denial> {
        if token.len() > MAX_RESOURCE_TOKEN_BYTES {
            return Err(Denial::BodyTooLarge);
        }
        let (payload, signature) = token.split_once('.').ok_or(Denial::Malformed)?;
        let signature = Base64UrlUnpadded::decode_vec(signature).map_err(|_| Denial::Malformed)?;
        let claims: ResourceClaims = Base64UrlUnpadded::decode_vec(payload)
            .ok()
            .and_then(|value| serde_json::from_slice(&value).ok())
            .ok_or(Denial::Malformed)?;
        if claims.v != 1 || claims.posts.len() > 100 || claims.missing.len() > 100 {
            return Err(Denial::Malformed);
        }
        let (_, key) = self
            .keys
            .iter()
            .find(|(id, _)| *id == claims.kid)
            .ok_or(Denial::UnknownKey)?;
        UnparsedPublicKey::new(&ED25519, key)
            .verify(&signed(payload), &signature)
            .map_err(|_| Denial::BadSignature)?;
        if claims.iss != Workload::Post.as_str() {
            return Err(Denial::WrongIssuer);
        }
        if claims.par != parent_digest || claims.wit != witness_id || witness_id.is_nil() {
            return Err(Denial::WrongBody);
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
        Ok(ResourceResolution {
            parent_digest: claims.par,
            witness_id: claims.wit,
            posts: claims
                .posts
                .into_iter()
                .map(|[post_id, author_id]| PostAuthor { post_id, author_id })
                .collect(),
            missing_post_ids: claims.missing,
        })
    }
}
fn signed(payload: &str) -> Vec<u8> {
    [CONTEXT, payload.as_bytes()].concat()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn owner_resource_evidence_enforces_signature_time_and_row_limits() {
        let owner = ResourceEvidenceSigner::from_seed(&[11; 32]).unwrap();
        let verifier = ResourceEvidenceVerifier::new(&[owner.public_key()]).unwrap();
        let parent = VerifiedParentAuthority::new(
            "parent-token".into(),
            "root".into(),
            vec![],
            None,
            Uuid::from_u128(1),
            130,
        );
        let digest = body_digest(parent.token.as_bytes());
        let row = || PostAuthor {
            post_id: Uuid::from_u128(2),
            author_id: Uuid::from_u128(3),
        };
        let token = owner
            .attest(&parent, Uuid::from_u128(77), &[row()], &[], 100)
            .unwrap();
        assert!(verifier
            .verify(&token, &digest, Uuid::from_u128(77), 100)
            .is_ok());
        assert!(matches!(
            verifier.verify(&token, &digest, Uuid::from_u128(77), 136),
            Err(Denial::Expired)
        ));
        assert!(matches!(
            verifier.verify(&token, &digest, Uuid::from_u128(77), 94),
            Err(Denial::NotYetValid)
        ));
        assert!(matches!(
            owner.attest(&parent, Uuid::from_u128(77), &[row()], &[], 130),
            Err(Denial::Expired)
        ));
        assert!(matches!(
            owner.attest(&parent, Uuid::from_u128(77), &[row(), row()], &[], 100),
            Err(Denial::WrongBody)
        ));
        assert!(matches!(
            owner.attest(
                &parent,
                Uuid::from_u128(77),
                &[row()],
                &[row().post_id],
                100
            ),
            Err(Denial::WrongBody)
        ));
        assert!(matches!(
            owner.attest(
                &parent,
                Uuid::from_u128(77),
                &[PostAuthor {
                    post_id: Uuid::nil(),
                    author_id: row().author_id
                }],
                &[],
                100
            ),
            Err(Denial::WrongBody)
        ));
        let too_many: Vec<_> = (1..=101)
            .map(|n| PostAuthor {
                post_id: Uuid::from_u128(n),
                author_id: row().author_id,
            })
            .collect();
        assert!(matches!(
            owner.attest(&parent, Uuid::from_u128(77), &too_many, &[], 100),
            Err(Denial::BodyTooLarge)
        ));
        let (payload, signature) = token.split_once('.').unwrap();
        let mut claims: ResourceClaims =
            serde_json::from_slice(&Base64UrlUnpadded::decode_vec(payload).unwrap()).unwrap();
        claims.posts[0][1] = Uuid::from_u128(4);
        let changed = Base64UrlUnpadded::encode_string(&serde_json::to_vec(&claims).unwrap());
        assert!(matches!(
            verifier.verify(
                &format!("{changed}.{signature}"),
                &digest,
                Uuid::from_u128(77),
                100
            ),
            Err(Denial::BadSignature)
        ));
        assert!(ResourceEvidenceVerifier::new(&[]).is_err());
    }

    fn lookup(
        entries: &'static [(&'static str, String)],
    ) -> impl Fn(&str) -> Option<String> + 'static {
        move |name| {
            entries
                .iter()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.clone())
        }
    }

    fn leaked(entries: Vec<(&'static str, String)>) -> &'static [(&'static str, String)] {
        Box::leak(entries.into_boxed_slice())
    }

    #[test]
    fn resource_keys_fail_closed_in_production_and_default_to_parity_elsewhere() {
        use base64ct::{Base64, Encoding};
        let none = leaked(vec![]);
        assert!(ResourceEvidenceSigner::from_lookup(true, lookup(none)).is_err());
        assert!(ResourceEvidenceVerifier::from_lookup(true, lookup(none)).is_err());
        let parity_signer = ResourceEvidenceSigner::from_lookup(false, lookup(none)).unwrap();
        let parity_verifier = ResourceEvidenceVerifier::from_lookup(false, lookup(none)).unwrap();
        assert!(parity_verifier.trusts(&parity_signer.public_key()));

        for parity in [
            LOCAL_PARITY_RESOURCE_SEED,
            crate::actor_delegation::LOCAL_PARITY_SEED,
        ] {
            let entries = leaked(vec![(
                RESOURCE_EVIDENCE_SIGNING_SEED_ENV,
                Base64::encode_string(&parity),
            )]);
            assert!(ResourceEvidenceSigner::from_lookup(true, lookup(entries)).is_err());
            assert!(ResourceEvidenceSigner::from_lookup(false, lookup(entries)).is_ok());
        }
        let parity_key = leaked(vec![(
            RESOURCE_EVIDENCE_VERIFYING_KEYS_ENV,
            Base64::encode_string(&parity_signer.public_key()),
        )]);
        assert!(ResourceEvidenceVerifier::from_lookup(true, lookup(parity_key)).is_err());

        let seed = [42u8; 32];
        let owner = ResourceEvidenceSigner::from_seed(&seed).unwrap();
        let previous = ResourceEvidenceSigner::from_seed(&[43u8; 32]).unwrap();
        let configured = leaked(vec![
            (
                RESOURCE_EVIDENCE_SIGNING_SEED_ENV,
                Base64::encode_string(&seed),
            ),
            (
                RESOURCE_EVIDENCE_VERIFYING_KEYS_ENV,
                format!(
                    "{}, {}",
                    Base64::encode_string(&owner.public_key()),
                    Base64::encode_string(&previous.public_key())
                ),
            ),
        ]);
        let signer = ResourceEvidenceSigner::from_lookup(true, lookup(configured)).unwrap();
        let verifier = ResourceEvidenceVerifier::from_lookup(true, lookup(configured)).unwrap();
        assert_eq!(signer.public_key(), owner.public_key());
        assert!(verifier.trusts(&owner.public_key()));
        assert!(verifier.trusts(&previous.public_key()));
        assert!(!verifier.trusts(&parity_signer.public_key()));

        let malformed = leaked(vec![(
            RESOURCE_EVIDENCE_VERIFYING_KEYS_ENV,
            "not-base64".to_owned(),
        )]);
        assert!(ResourceEvidenceVerifier::from_lookup(false, lookup(malformed)).is_err());
        let empty = leaked(vec![(
            RESOURCE_EVIDENCE_VERIFYING_KEYS_ENV,
            " , ".to_owned(),
        )]);
        assert!(ResourceEvidenceVerifier::from_lookup(false, lookup(empty)).is_err());
    }
}
