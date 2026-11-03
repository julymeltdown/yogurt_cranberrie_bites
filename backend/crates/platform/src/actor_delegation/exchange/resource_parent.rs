//! Fresh signature proof for a resource lookup, deliberately NOT admission.
//! It cannot be scoped as handler authority or populate an admission extension.
use super::*;

pub enum ResourceParentScope<'a> {
    Posts(&'a [Uuid]),
    Discovery,
}
/// No public constructor, Debug, raw token retention, or task-scoping API.
/// A resource lookup additionally requires an already published owner stage.
pub struct VerifiedResourceParent {
    digest: [u8; 32],
    actor: Uuid,
    expires_at: i64,
    scope: ResourceScope,
}
impl VerifiedResourceParent {
    pub fn digest(&self) -> &[u8; 32] {
        &self.digest
    }
    pub fn actor(&self) -> Uuid {
        self.actor
    }
    pub fn expires_at(&self) -> i64 {
        self.expires_at
    }
    pub fn scope(&self) -> ResourceParentScope<'_> {
        match &self.scope {
            ResourceScope::Posts(ids) => ResourceParentScope::Posts(ids),
            ResourceScope::DiscoveryCandidates => ResourceParentScope::Discovery,
            ResourceScope::ExactAuthor(_) => {
                unreachable!("resource-free roots are rejected during verification")
            }
        }
    }
}
impl ActorDelegationVerifier {
    /// Reverify a Gateway-signed Post root for an independent owner resource
    /// lookup. Do not consume its replay ID, re-enter a use case, or admit it.
    pub fn verify_resource_parent(
        &self,
        parent: ParentRequest<'_>,
        now: i64,
    ) -> Result<VerifiedResourceParent, Denial> {
        if self.audience != Workload::Post {
            return Err(Denial::WrongAudience);
        }
        if parent.token.len() > MAX_TOKEN_BYTES || parent.body.len() > MAX_BODY_BYTES {
            return Err(Denial::BodyTooLarge);
        }
        if parent.operation.is_some_and(|id| id.is_nil()) {
            return Err(Denial::WrongOperation);
        }
        let (fields, _) = parent_scope(parent.method, parent.body)?;
        let verified = self.verify(
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
        bind_actor(parent.body, fields, verified.actor)?;
        if verified.exp <= now {
            return Err(Denial::Expired);
        }
        let scope = resource_scope(parent.method, parent.body)?;
        if matches!(scope, ResourceScope::ExactAuthor(_)) {
            return Err(Denial::WrongMethod);
        }
        Ok(VerifiedResourceParent {
            digest: Sha256::digest(parent.token.as_bytes()).into(),
            actor: verified.actor,
            expires_at: verified.exp,
            scope,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    const FEED: &str = "/post.v1.PostService/GetFeed";
    const POST: &str = "/post.v1.PostService/GetPost";
    fn actor() -> Uuid {
        Uuid::from_u128(1)
    }
    fn frame(fields: &[(u8, Uuid)]) -> Vec<u8> {
        let message: Vec<u8> = fields
            .iter()
            .flat_map(|(field, value)| {
                [vec![(field << 3) | 2, 36], value.to_string().into_bytes()].concat()
            })
            .collect();
        [
            vec![0],
            (message.len() as u32).to_be_bytes().to_vec(),
            message,
        ]
        .concat()
    }
    fn request<'a>(token: &'a str, method: &'a str, body: &'a [u8]) -> ParentRequest<'a> {
        ParentRequest {
            token,
            method,
            body,
            operation: None,
        }
    }
    #[test]
    fn lookup_signature_verification_does_not_admit_or_consume_the_root() {
        let signer = ActorDelegationSigner::local_parity(Workload::Gateway);
        let verifier = ActorDelegationVerifier::local_parity(Workload::Post);
        let body = frame(&[(1, actor())]);
        let token = signer.issue(Workload::Post, FEED, actor(), None, &body, 100);
        let first = verifier
            .verify_resource_parent(request(&token, FEED, &body), 100)
            .unwrap();
        let second = verifier
            .verify_resource_parent(request(&token, FEED, &body), 101)
            .unwrap();
        assert_eq!(first.digest(), second.digest());
        assert_eq!(first.actor(), actor());
        assert!(matches!(first.scope(), ResourceParentScope::Discovery));
        assert!(VerifiedParentAuthority::current().is_none());
        let verified = verifier
            .verify(
                &token,
                &Expected {
                    caller: Workload::Gateway,
                    method: FEED,
                    body: &body,
                    operation: None,
                },
                100,
            )
            .unwrap();
        verifier.consume(&verified, 100).unwrap();
        assert!(matches!(
            verifier.consume(&verified, 100),
            Err(Denial::Replayed)
        ));
        assert!(verifier
            .verify_resource_parent(request(&token, FEED, &body), 101)
            .is_ok());
        assert!(VerifiedParentAuthority::current().is_none());
    }
    #[test]
    fn lookup_rejects_wrong_actor_body_issuer_audience_method_and_expiry() {
        let signer = ActorDelegationSigner::local_parity(Workload::Gateway);
        let verifier = ActorDelegationVerifier::local_parity(Workload::Post);
        let body = frame(&[(1, actor())]);
        let token = signer.issue(Workload::Post, FEED, actor(), None, &body, 100);
        let changed = frame(&[(1, Uuid::from_u128(2))]);
        assert!(matches!(
            verifier.verify_resource_parent(request(&token, FEED, &changed), 100),
            Err(Denial::WrongBody)
        ));
        let wrong_actor = signer.issue(Workload::Post, FEED, Uuid::from_u128(2), None, &body, 100);
        assert!(matches!(
            verifier.verify_resource_parent(request(&wrong_actor, FEED, &body), 100),
            Err(Denial::ActorMismatch)
        ));
        let sibling = ActorDelegationSigner::local_parity(Workload::Post).issue(
            Workload::Post,
            FEED,
            actor(),
            None,
            &body,
            100,
        );
        assert!(matches!(
            verifier.verify_resource_parent(request(&sibling, FEED, &body), 100),
            Err(Denial::WrongIssuer)
        ));
        let wrong_audience = signer.issue(Workload::Friend, FEED, actor(), None, &body, 100);
        assert!(matches!(
            verifier.verify_resource_parent(request(&wrong_audience, FEED, &body), 100),
            Err(Denial::WrongAudience)
        ));
        assert!(matches!(
            verifier.verify_resource_parent(request(&token, FEED, &body), 130),
            Err(Denial::Expired)
        ));
        assert!(verifier
            .verify_resource_parent(
                request(&token, "/post.v1.PostService/GetAuthorPosts", &body),
                100
            )
            .is_err());
        let author_body = frame(&[(1, Uuid::from_u128(2)), (4, actor())]);
        let author_token = signer.issue(
            Workload::Post,
            "/post.v1.PostService/GetAuthorPosts",
            actor(),
            None,
            &author_body,
            100,
        );
        assert!(matches!(
            verifier.verify_resource_parent(
                request(
                    &author_token,
                    "/post.v1.PostService/GetAuthorPosts",
                    &author_body
                ),
                100
            ),
            Err(Denial::WrongMethod)
        ));
    }
    #[test]
    fn resource_attestation_requires_complete_fixed_scope_and_parent_capped_expiry() {
        let signer = ActorDelegationSigner::local_parity(Workload::Gateway);
        let verifier = ActorDelegationVerifier::local_parity(Workload::Post);
        let owner = ResourceEvidenceSigner::from_seed(&[11; 32]).unwrap();
        let body = frame(&[(1, Uuid::from_u128(2)), (2, actor())]);
        let token = signer.issue(Workload::Post, POST, actor(), None, &body, 100);
        let parent = verifier
            .verify_resource_parent(request(&token, POST, &body), 100)
            .unwrap();
        assert!(
            matches!(parent.scope(), ResourceParentScope::Posts(ids) if ids == [Uuid::from_u128(2)])
        );
        let stage = Uuid::from_u128(77);
        let row = PostAuthor {
            post_id: Uuid::from_u128(2),
            author_id: Uuid::from_u128(3),
        };
        assert!(matches!(
            owner.attest_resource_parent(&parent, stage, &[], &[], 100),
            Err(Denial::WrongBody)
        ));
        assert!(matches!(
            owner.attest_resource_parent(
                &parent,
                stage,
                &[PostAuthor {
                    post_id: Uuid::from_u128(4),
                    author_id: row.author_id
                }],
                &[],
                100
            ),
            Err(Denial::WrongBody)
        ));
        let attestation = owner
            .attest_resource_parent(&parent, stage, &[row], &[], 129)
            .unwrap();
        let resources = ResourceEvidenceVerifier::new(&[owner.public_key()]).unwrap();
        let resolution = resources
            .verify(&attestation, &body_digest(token.as_bytes()), stage, 129)
            .unwrap();
        assert_eq!(resolution.posts[0].post_id, Uuid::from_u128(2));
        assert!(matches!(
            owner.attest_resource_parent(&parent, stage, &[], &[Uuid::from_u128(2)], 130),
            Err(Denial::Expired)
        ));
        let bytes = attestation.split_once('.').unwrap().0;
        let claims: serde_json::Value =
            serde_json::from_slice(&Base64UrlUnpadded::decode_vec(bytes).unwrap()).unwrap();
        assert_eq!(claims["exp"], 130);
        assert!(owner
            .attest_resource_parent(&parent, stage, &[], &[Uuid::from_u128(2)], 129)
            .is_ok());
    }
}
