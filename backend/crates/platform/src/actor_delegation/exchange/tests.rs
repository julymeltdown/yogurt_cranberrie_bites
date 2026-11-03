use super::*;

const ROOT: &str = "/post.v1.PostService/GetFeed";
const FOLLOWING: &str = "/friend.v1.FriendService/ListFollowing";
const RELATION: &str = "/friend.v1.FriendService/GetRelationship";

fn actor() -> Uuid {
    Uuid::from_u128(1)
}
fn other() -> Uuid {
    Uuid::from_u128(2)
}
fn frame(field: u8, value: Uuid) -> Vec<u8> {
    let value = value.to_string();
    let message = [vec![field << 3 | 2, value.len() as u8], value.into_bytes()].concat();
    [
        vec![0],
        (message.len() as u32).to_be_bytes().to_vec(),
        message,
    ]
    .concat()
}
fn signer() -> ActorDelegationSigner {
    ActorDelegationSigner::local_parity(Workload::Gateway)
}
fn resource_signer() -> ResourceEvidenceSigner {
    ResourceEvidenceSigner::from_seed(&[11; 32]).unwrap()
}
fn resource_verifier() -> ResourceEvidenceVerifier {
    ResourceEvidenceVerifier::new(&[resource_signer().public_key()]).unwrap()
}
fn broker() -> DelegationExchange {
    DelegationExchange::new(
        signer(),
        ActorDelegationVerifier::local_parity(Workload::Post),
        resource_verifier(),
    )
    .unwrap()
}
fn parent<'a>(token: &'a str, body: &'a [u8]) -> ParentRequest<'a> {
    ParentRequest {
        token,
        method: ROOT,
        body,
        operation: None,
    }
}
fn child(body: &[u8]) -> ChildRequest<'_> {
    ChildRequest {
        audience: Workload::Friend,
        method: FOLLOWING,
        body,
        operation: None,
    }
}
fn expected(body: &[u8]) -> Expected<'_> {
    Expected {
        caller: Workload::Post,
        method: FOLLOWING,
        body,
        operation: None,
    }
}
fn root(body: &[u8]) -> String {
    signer().issue(Workload::Post, ROOT, actor(), None, body, 100)
}

#[test]
fn gateway_child_separates_presenter_from_issuer_and_is_consumed_once() {
    let body = frame(1, actor());
    let token = root(&body);
    let issued = broker()
        .issue(Workload::Post, parent(&token, &body), child(&body), 101)
        .unwrap();
    let owner = ActorDelegationVerifier::local_parity(Workload::Friend);
    let verified = owner
        .verify_child(&issued.token, &expected(&body), 101)
        .unwrap();
    assert_eq!(verified.actor, actor());
    bind_actor(&body, &[&[1]], verified.actor).unwrap();
    owner.consume(&verified, 101).unwrap();
    assert_eq!(owner.consume(&verified, 101), Err(Denial::Replayed));
    assert!(
        owner.verify(&issued.token, &expected(&body), 101).is_err(),
        "child cannot enter the unchanged direct-v1 path"
    );
    let post = ActorDelegationVerifier::local_parity(Workload::Post);
    assert!(post
        .verify(
            &token,
            &Expected {
                caller: Workload::Gateway,
                method: ROOT,
                body: &body,
                operation: None
            },
            101
        )
        .is_ok());
    assert!(
        post.verify_child(
            &token,
            &Expected {
                caller: Workload::Post,
                method: ROOT,
                body: &body,
                operation: None
            },
            101
        )
        .is_err(),
        "root cannot be presented as a child"
    );
}

#[test]
fn only_post_can_exchange_and_only_gateway_can_issue() {
    let body = frame(1, actor());
    let token = root(&body);
    let broker = broker();
    for caller in Workload::ALL {
        if caller != Workload::Post {
            assert!(matches!(
                broker.issue(caller, parent(&token, &body), child(&body), 101),
                Err(Denial::WrongIssuer)
            ));
        }
    }
    assert!(DelegationExchange::new(
        ActorDelegationSigner::local_parity(Workload::Post),
        ActorDelegationVerifier::local_parity(Workload::Post),
        resource_verifier(),
    )
    .is_err());
    assert!(DelegationExchange::new(
        signer(),
        ActorDelegationVerifier::local_parity(Workload::Friend),
        resource_verifier(),
    )
    .is_err());
    let untrusted = ActorDelegationSigner::from_seed(Workload::Gateway, &[9; 32]).unwrap();
    assert!(DelegationExchange::new(
        untrusted,
        ActorDelegationVerifier::local_parity(Workload::Post),
        resource_verifier(),
    )
    .is_err());
}

#[test]
fn exact_parent_actor_body_method_audience_and_operation_are_required() {
    let body = frame(1, actor());
    let token = root(&body);
    let broker = broker();
    let wrong = frame(1, other());
    assert!(matches!(
        broker.issue(Workload::Post, parent(&token, &wrong), child(&body), 101),
        Err(Denial::WrongBody)
    ));
    let mismatched = signer().issue(Workload::Post, ROOT, other(), None, &body, 100);
    assert!(matches!(
        broker.issue(
            Workload::Post,
            parent(&mismatched, &body),
            child(&body),
            101
        ),
        Err(Denial::ActorMismatch)
    ));
    let wrong_audience = signer().issue(Workload::Friend, ROOT, actor(), None, &body, 100);
    assert!(matches!(
        broker.issue(
            Workload::Post,
            parent(&wrong_audience, &body),
            child(&body),
            101
        ),
        Err(Denial::WrongAudience)
    ));
    let wrong_method = signer().issue(
        Workload::Post,
        "/post.v1.PostService/GetTrending",
        actor(),
        None,
        &body,
        100,
    );
    assert!(matches!(
        broker.issue(
            Workload::Post,
            parent(&wrong_method, &body),
            child(&body),
            101
        ),
        Err(Denial::WrongMethod)
    ));
    let mut request = parent(&token, &body);
    request.operation = Some(other());
    assert!(matches!(
        broker.issue(Workload::Post, request, child(&body), 101),
        Err(Denial::WrongOperation)
    ));
    let nil = frame(1, Uuid::nil());
    let token = signer().issue(Workload::Post, ROOT, Uuid::nil(), None, &nil, 100);
    assert!(matches!(
        broker.issue(Workload::Post, parent(&token, &nil), child(&nil), 101),
        Err(Denial::ActorMismatch)
    ));
}

#[test]
fn parent_authority_cannot_expand_to_another_actor_write_or_resource_kind() {
    let body = frame(1, actor());
    let token = root(&body);
    let broker = broker();
    let wrong = frame(1, other());
    assert!(matches!(
        broker.issue(Workload::Post, parent(&token, &body), child(&wrong), 101),
        Err(Denial::ActorMismatch)
    ));
    for (audience, method) in [
        (Workload::Friend, "/friend.v1.FriendService/FollowUser"),
        (Workload::Friend, "/friend.v1.FriendService/ListFollowers"),
        (Workload::Profile, "/profile.v1.ProfileService/GetProfile"),
        (
            Workload::Notification,
            "/notification.v1.NotificationService/CreateNotification",
        ),
    ] {
        let child = ChildRequest {
            audience,
            method,
            body: &body,
            operation: None,
        };
        assert!(matches!(
            broker.issue(Workload::Post, parent(&token, &body), child, 101),
            Err(Denial::WrongMethod)
        ));
    }
    let created = signer().issue(
        Workload::Post,
        "/post.v1.PostService/CreatePost",
        actor(),
        None,
        &body,
        100,
    );
    let parent = ParentRequest {
        token: &created,
        method: "/post.v1.PostService/CreatePost",
        body: &body,
        operation: None,
    };
    let profile = ChildRequest {
        audience: Workload::Profile,
        method: "/authority.v1.ProfileAuthorAuthorityService/CheckAuthorProfile",
        body: &body,
        operation: None,
    };
    assert!(
        matches!(
            broker.issue(
                Workload::Post,
                ParentRequest {
                    token: &created,
                    method: "/post.v1.PostService/CreatePost",
                    body: &body,
                    operation: None,
                },
                ChildRequest {
                    audience: Workload::Profile,
                    method: "/profile.v1.ProfileService/GetProfile",
                    body: &body,
                    operation: None
                },
                101
            ),
            Err(Denial::WrongMethod)
        ),
        "CreatePost cannot obtain a broad profile child"
    );
    let issued = broker.issue(Workload::Post, parent, profile, 101).unwrap();
    let owner = ActorDelegationVerifier::local_parity(Workload::Profile);
    assert_eq!(
        owner
            .verify_child(
                &issued.token,
                &Expected {
                    caller: Workload::Post,
                    method: "/authority.v1.ProfileAuthorAuthorityService/CheckAuthorProfile",
                    body: &body,
                    operation: None
                },
                101
            )
            .unwrap()
            .actor,
        actor()
    );
}

#[test]
fn child_expiry_is_capped_at_parent_and_skew_cannot_refresh_it() {
    let body = frame(1, actor());
    let token = root(&body);
    let broker = broker();
    let issued = broker
        .issue(Workload::Post, parent(&token, &body), child(&body), 129)
        .unwrap();
    assert_eq!(issued.expires_at, 130);
    assert!(matches!(
        broker.issue(Workload::Post, parent(&token, &body), child(&body), 130),
        Err(Denial::Expired)
    ));
    assert!(matches!(
        broker.issue(Workload::Post, parent(&token, &body), child(&body), 131),
        Err(Denial::Expired)
    ));
    let owner = ActorDelegationVerifier::local_parity(Workload::Friend);
    assert!(matches!(
        owner.verify_child(&issued.token, &expected(&body), 136),
        Err(Denial::Expired)
    ));
    assert!(matches!(
        broker.issue(Workload::Post, parent(&token, &body), child(&body), 90),
        Err(Denial::NotYetValid)
    ));
}

#[test]
fn child_binds_actual_presenter_audience_method_body_and_operation() {
    let body = frame(1, actor());
    let token = root(&body);
    let broker = broker();
    let issued = broker
        .issue(Workload::Post, parent(&token, &body), child(&body), 101)
        .unwrap();
    let owner = ActorDelegationVerifier::local_parity(Workload::Friend);
    let mut expected = expected(&body);
    expected.caller = Workload::Gateway;
    assert!(matches!(
        owner.verify_child(&issued.token, &expected, 101),
        Err(Denial::WrongIssuer)
    ));
    expected.caller = Workload::Profile;
    assert!(matches!(
        owner.verify_child(&issued.token, &expected, 101),
        Err(Denial::WrongIssuer)
    ));
    expected.caller = Workload::Post;
    expected.method = RELATION;
    assert!(matches!(
        owner.verify_child(&issued.token, &expected, 101),
        Err(Denial::WrongMethod)
    ));
    expected.method = FOLLOWING;
    expected.operation = Some(other());
    assert!(matches!(
        owner.verify_child(&issued.token, &expected, 101),
        Err(Denial::WrongOperation)
    ));
    let wrong = frame(1, other());
    expected.operation = None;
    expected.body = &wrong;
    assert!(matches!(
        owner.verify_child(&issued.token, &expected, 101),
        Err(Denial::WrongBody)
    ));
    assert!(matches!(
        ActorDelegationVerifier::local_parity(Workload::Profile).verify_child(
            &issued.token,
            &self::expected(&body),
            101
        ),
        Err(Denial::WrongAudience)
    ));
    let (payload, signature) = issued.token.split_once('.').unwrap();
    let mut claims: ChildClaims =
        serde_json::from_slice(&Base64UrlUnpadded::decode_vec(payload).unwrap()).unwrap();
    claims.sub = other();
    let changed = Base64UrlUnpadded::encode_string(&serde_json::to_vec(&claims).unwrap());
    assert!(matches!(
        owner.verify_child(
            &format!("{changed}.{signature}"),
            &self::expected(&body),
            101
        ),
        Err(Denial::BadSignature)
    ));
}

#[test]
fn child_operation_must_not_replace_parent_operation() {
    let body = frame(1, actor());
    let operation = actor();
    let token = signer().issue(Workload::Post, ROOT, actor(), Some(operation), &body, 100);
    let broker = broker();
    let parent = || ParentRequest {
        token: &token,
        method: ROOT,
        body: &body,
        operation: Some(operation),
    };
    let mut wrong = child(&body);
    wrong.operation = Some(other());
    assert!(matches!(
        broker.issue(Workload::Post, parent(), wrong, 101),
        Err(Denial::WrongOperation)
    ));
    let mut request = child(&body);
    request.operation = Some(operation);
    let issued = broker
        .issue(Workload::Post, parent(), request, 101)
        .unwrap();
    let owner = ActorDelegationVerifier::local_parity(Workload::Friend);
    let mut expected = expected(&body);
    expected.operation = Some(operation);
    assert_eq!(
        owner
            .verify_child(&issued.token, &expected, 101)
            .unwrap()
            .actor,
        actor()
    );
}

#[test]
fn invalid_exchanges_do_not_consume_credit_and_parent_cache_is_bounded() {
    let body = frame(1, actor());
    let token = root(&body);
    let broker = broker();
    let wrong = frame(1, other());
    for _ in 0..MAX_CHILDREN_PER_PARENT {
        assert!(broker
            .issue(Workload::Post, parent(&token, &body), child(&wrong), 101)
            .is_err());
    }
    for _ in 0..MAX_CHILDREN_PER_PARENT {
        broker
            .issue(Workload::Post, parent(&token, &body), child(&body), 101)
            .unwrap();
    }
    assert!(matches!(
        broker.issue(Workload::Post, parent(&token, &body), child(&body), 101),
        Err(Denial::ReplayCacheFull)
    ));
    let next = signer().issue(Workload::Post, ROOT, actor(), None, &body, 130);
    assert!(broker
        .issue(Workload::Post, parent(&next, &body), child(&body), 131)
        .is_ok());
    assert_eq!(
        broker.usage.lock().unwrap().len(),
        1,
        "expired parent budgets retire"
    );
    let mut usage = broker.usage.lock().unwrap();
    for index in 1..PARENT_CACHE_CAPACITY {
        usage.insert(index.to_string(), (160, 1));
    }
    drop(usage);
    let third = signer().issue(Workload::Post, ROOT, actor(), None, &body, 130);
    assert!(matches!(
        broker.issue(Workload::Post, parent(&third, &body), child(&body), 131),
        Err(Denial::ReplayCacheFull)
    ));
}

#[test]
fn rotation_accepts_retained_parent_key_and_issues_only_current_child_key() {
    let previous = signer();
    let current = ActorDelegationSigner::from_seed(Workload::Gateway, &[7; 32]).unwrap();
    let keys = [previous.public_key(), current.public_key()];
    let broker = DelegationExchange::new(
        current.clone(),
        ActorDelegationVerifier::new(Workload::Post, &keys).unwrap(),
        resource_verifier(),
    )
    .unwrap();
    let body = frame(1, actor());
    let token = root(&body);
    let issued = broker
        .issue(Workload::Post, parent(&token, &body), child(&body), 101)
        .unwrap();
    let owner = ActorDelegationVerifier::new(Workload::Friend, &[current.public_key()]).unwrap();
    assert!(owner
        .verify_child(&issued.token, &expected(&body), 101)
        .is_ok());
    assert!(matches!(
        ActorDelegationVerifier::new(Workload::Friend, &[previous.public_key()])
            .unwrap()
            .verify_child(&issued.token, &expected(&body), 101),
        Err(Denial::UnknownKey)
    ));
}

#[test]
fn unknown_roots_malformed_frames_and_oversized_inputs_fail_closed() {
    let body = frame(1, actor());
    let token = root(&body);
    let broker = broker();
    let mut parent = parent(&token, &body);
    parent.method = "/post.v1.PostService/ApplyUserDeletion";
    assert!(matches!(
        broker.issue(Workload::Post, parent, child(&body), 101),
        Err(Denial::WrongMethod)
    ));
    let mut compressed = body.clone();
    compressed[0] = 1;
    let token = root(&compressed);
    assert!(broker
        .issue(
            Workload::Post,
            self::parent(&token, &compressed),
            child(&body),
            101
        )
        .is_err());
    let too_large = vec![0; MAX_BODY_BYTES + 1];
    assert!(matches!(
        broker.issue(
            Workload::Post,
            self::parent(&token, &body),
            child(&too_large),
            101
        ),
        Err(Denial::BodyTooLarge)
    ));
    let too_large_token = "a".repeat(MAX_TOKEN_BYTES + 1);
    assert!(matches!(
        broker.issue(
            Workload::Post,
            self::parent(&too_large_token, &body),
            child(&body),
            101
        ),
        Err(Denial::BodyTooLarge)
    ));
}

fn fields(values: &[(u8, Uuid)]) -> Vec<u8> {
    let mut message = Vec::new();
    for (field, value) in values {
        let text = value.to_string();
        message.extend([field << 3 | 2, text.len() as u8]);
        message.extend(text.as_bytes());
    }
    [
        vec![0],
        (message.len() as u32).to_be_bytes().to_vec(),
        message,
    ]
    .concat()
}

fn verified_root(token: &str, method: &str, body: &[u8]) -> VerifiedParentAuthority {
    // Only this unit fixture can construct owner evidence without a real
    // guarded RPC. Runtime acceptance must exercise guard + repository producer.
    VerifiedParentAuthority::new(
        token.into(),
        method.into(),
        body.to_vec(),
        None,
        actor(),
        130,
    )
}

#[test]
fn explicit_author_scope_rejects_a_different_target_with_the_same_actor() {
    let method = "/post.v1.PostService/GetAuthorPosts";
    let body = fields(&[(1, other()), (4, actor())]);
    let token = signer().issue(Workload::Post, method, actor(), None, &body, 100);
    let broker = broker();
    let parent = || ParentRequest {
        token: &token,
        method,
        body: &body,
        operation: None,
    };
    let wrong = fields(&[(1, actor()), (2, Uuid::from_u128(3))]);
    assert!(matches!(
        broker.prepare(
            Workload::Post,
            parent(),
            ChildRequest {
                audience: Workload::Friend,
                method: RELATION,
                body: &wrong,
                operation: None
            },
            101
        ),
        Err(Denial::WrongBody)
    ));
    let right = fields(&[(1, actor()), (2, other())]);
    let prepared = broker
        .prepare(
            Workload::Post,
            parent(),
            ChildRequest {
                audience: Workload::Friend,
                method: RELATION,
                body: &right,
                operation: None,
            },
            101,
        )
        .unwrap();
    assert!(matches!(prepared.resource_lookup(), ResourceLookup::None));
    assert!(broker.finish(prepared, None, 102).is_ok());
}

#[test]
fn post_scopes_require_owner_signed_exact_parent_resource_resolution() {
    let method = "/post.v1.PostService/GetPost";
    let post_id = Uuid::from_u128(4);
    let author = other();
    let body = fields(&[(1, post_id), (2, actor())]);
    let token = signer().issue(Workload::Post, method, actor(), None, &body, 100);
    let context = verified_root(&token, method, &body);
    let broker = broker();
    let child_body = fields(&[(1, actor()), (2, author)]);
    let prepare = || {
        broker
            .prepare_with_witness(
                Workload::Post,
                ParentRequest {
                    token: &token,
                    method,
                    body: &body,
                    operation: None,
                },
                ChildRequest {
                    audience: Workload::Friend,
                    method: RELATION,
                    body: &child_body,
                    operation: None,
                },
                Uuid::from_u128(77),
                101,
            )
            .unwrap()
    };
    assert!(matches!(prepare().resource_lookup(), ResourceLookup::Posts(ids) if ids == [post_id]));
    assert!(matches!(
        broker.finish(prepare(), None, 102),
        Err(Denial::Missing)
    ));
    let evidence = resource_signer()
        .attest(
            &context,
            Uuid::from_u128(77),
            &[PostAuthor {
                post_id,
                author_id: author,
            }],
            &[],
            101,
        )
        .unwrap();
    let issued = broker.finish(prepare(), Some(&evidence), 102).unwrap();
    let owner = ActorDelegationVerifier::local_parity(Workload::Friend);
    assert_eq!(
        owner
            .verify_child(
                &issued.token,
                &Expected {
                    caller: Workload::Post,
                    method: RELATION,
                    body: &child_body,
                    operation: None
                },
                102
            )
            .unwrap()
            .actor,
        actor()
    );
    let unrelated = resource_signer()
        .attest(
            &context,
            Uuid::from_u128(77),
            &[PostAuthor {
                post_id: Uuid::from_u128(5),
                author_id: author,
            }],
            &[],
            101,
        )
        .unwrap();
    assert!(matches!(
        broker.finish(prepare(), Some(&unrelated), 102),
        Err(Denial::WrongBody)
    ));
    let missing = resource_signer()
        .attest(&context, Uuid::from_u128(77), &[], &[post_id], 101)
        .unwrap();
    assert!(matches!(
        broker.finish(prepare(), Some(&missing), 102),
        Err(Denial::WrongBody)
    ));
    let wrong_author = resource_signer()
        .attest(
            &context,
            Uuid::from_u128(77),
            &[PostAuthor {
                post_id,
                author_id: Uuid::from_u128(6),
            }],
            &[],
            101,
        )
        .unwrap();
    assert!(matches!(
        broker.finish(prepare(), Some(&wrong_author), 102),
        Err(Denial::WrongBody)
    ));
    assert!(
        matches!(
            broker.finish(prepare(), Some(&evidence), 130),
            Err(Denial::Expired)
        ),
        "lookup cannot refresh parent expiry"
    );
}

#[test]
fn resource_proofs_are_not_user_tokens_and_cannot_move_between_parent_requests() {
    let body = frame(1, actor());
    let token = root(&body);
    let context = verified_root(&token, ROOT, &body);
    let evidence = resource_signer()
        .attest(
            &context,
            Uuid::from_u128(77),
            &[PostAuthor {
                post_id: other(),
                author_id: other(),
            }],
            &[],
            101,
        )
        .unwrap();
    let owner = ActorDelegationVerifier::local_parity(Workload::Friend);
    assert!(owner
        .verify_child(&evidence, &expected(&body), 101)
        .is_err());
    assert!(owner.verify(&evidence, &expected(&body), 101).is_err());
    let child_body = fields(&[(1, actor()), (2, other())]);
    let prepare = |root_token: &str| {
        broker()
            .prepare_with_witness(
                Workload::Post,
                parent(root_token, &body),
                ChildRequest {
                    audience: Workload::Friend,
                    method: RELATION,
                    body: &child_body,
                    operation: None,
                },
                Uuid::from_u128(77),
                101,
            )
            .unwrap()
    };
    let broker = broker();
    assert!(broker.finish(prepare(&token), Some(&evidence), 102).is_ok());
    let another = root(&body);
    assert!(matches!(
        broker.finish(prepare(&another), Some(&evidence), 102),
        Err(Denial::WrongBody)
    ));
    let fake_owner = ResourceEvidenceSigner::from_seed(&[12; 32])
        .unwrap()
        .attest(
            &context,
            Uuid::from_u128(77),
            &[PostAuthor {
                post_id: other(),
                author_id: other(),
            }],
            &[],
            101,
        )
        .unwrap();
    assert!(matches!(
        broker.finish(prepare(&token), Some(&fake_owner), 102),
        Err(Denial::UnknownKey)
    ));
}

#[test]
fn discovery_needs_attested_candidate_rows_and_batches_are_bounded() {
    let body = frame(1, actor());
    let token = root(&body);
    let broker = broker();
    let child_body = fields(&[(1, actor()), (2, other())]);
    let prepare = || {
        broker
            .prepare_with_witness(
                Workload::Post,
                parent(&token, &body),
                ChildRequest {
                    audience: Workload::Friend,
                    method: RELATION,
                    body: &child_body,
                    operation: None,
                },
                Uuid::from_u128(77),
                101,
            )
            .unwrap()
    };
    assert!(matches!(
        prepare().resource_lookup(),
        ResourceLookup::DiscoveryCandidates
    ));
    let context = verified_root(&token, ROOT, &body);
    let empty = resource_signer()
        .attest(&context, Uuid::from_u128(77), &[], &[], 101)
        .unwrap();
    assert!(matches!(
        broker.finish(prepare(), Some(&empty), 102),
        Err(Denial::WrongBody)
    ));
    let wrong_candidate = resource_signer()
        .attest(
            &context,
            Uuid::from_u128(77),
            &[PostAuthor {
                post_id: Uuid::from_u128(9),
                author_id: Uuid::from_u128(8),
            }],
            &[],
            101,
        )
        .unwrap();
    assert!(matches!(
        broker.finish(prepare(), Some(&wrong_candidate), 102),
        Err(Denial::WrongBody)
    ));
    let batch_method = "/friend.v1.FriendService/BatchGetRelationships";
    let duplicate = fields(&[(1, actor()), (2, other()), (2, other())]);
    assert!(broker
        .prepare(
            Workload::Post,
            parent(&token, &body),
            ChildRequest {
                audience: Workload::Friend,
                method: batch_method,
                body: &duplicate,
                operation: None
            },
            101
        )
        .is_err());
    let mut excessive = vec![(1, actor())];
    excessive.extend((2..103).map(|n| (2, Uuid::from_u128(n))));
    let excessive = fields(&excessive);
    assert!(broker
        .prepare(
            Workload::Post,
            parent(&token, &body),
            ChildRequest {
                audience: Workload::Friend,
                method: batch_method,
                body: &excessive,
                operation: None
            },
            101
        )
        .is_err());
}

#[test]
fn immutable_witness_ids_prevent_cross_batch_evidence_and_are_not_authority() {
    let body = frame(1, actor());
    let token = root(&body);
    let context = verified_root(&token, ROOT, &body);
    let broker = broker();
    let child_body = fields(&[(1, actor()), (2, other())]);
    let request = || ChildRequest {
        audience: Workload::Friend,
        method: RELATION,
        body: &child_body,
        operation: None,
    };
    assert!(matches!(
        broker.prepare(Workload::Post, parent(&token, &body), request(), 101),
        Err(Denial::Missing)
    ));
    assert!(matches!(
        broker.prepare_with_witness(
            Workload::Post,
            parent(&token, &body),
            request(),
            Uuid::nil(),
            101
        ),
        Err(Denial::WrongBody)
    ));
    let stage = Uuid::from_u128(77);
    let prepare = || {
        broker
            .prepare_with_witness(Workload::Post, parent(&token, &body), request(), stage, 101)
            .unwrap()
    };
    assert_eq!(prepare().witness_id(), Some(stage));
    assert!(
        matches!(broker.finish(prepare(), None, 102), Err(Denial::Missing)),
        "an ID alone is not proof"
    );
    let rows = [PostAuthor {
        post_id: Uuid::from_u128(9),
        author_id: other(),
    }];
    let other_stage = resource_signer()
        .attest(&context, Uuid::from_u128(78), &rows, &[], 101)
        .unwrap();
    assert!(
        matches!(
            broker.finish(prepare(), Some(&other_stage), 102),
            Err(Denial::WrongBody)
        ),
        "same parent and same rows cannot migrate between stages"
    );
    let evidence = resource_signer()
        .attest(&context, stage, &rows, &[], 101)
        .unwrap();
    let child = broker.finish(prepare(), Some(&evidence), 102).unwrap();
    let (payload, _) = child.token.split_once('.').unwrap();
    let claims: ChildClaims =
        serde_json::from_slice(&Base64UrlUnpadded::decode_vec(payload).unwrap()).unwrap();
    assert_eq!(claims.wit, Some(stage));
    assert!(
        matches!(
            broker.prepare_with_witness(
                Workload::Post,
                parent(&token, &body),
                super::tests::child(&body),
                stage,
                101
            ),
            Err(Denial::WrongBody)
        ),
        "resource-free reads cannot smuggle a witness"
    );
}

#[tokio::test]
async fn parent_context_is_isolated_across_interleaved_futures_and_spawned_work() {
    assert!(VerifiedParentAuthority::current().is_none());
    let make = |actor: Uuid| {
        let body = frame(1, actor);
        let token = signer().issue(Workload::Post, ROOT, actor, None, &body, 100);
        VerifiedParentAuthority::new(token, ROOT.into(), body, None, actor, 130)
    };
    let barrier = Arc::new(tokio::sync::Barrier::new(2));
    let check = |context: VerifiedParentAuthority, expected: Uuid| {
        let barrier = barrier.clone();
        context.scope(async move {
            assert_eq!(
                VerifiedParentAuthority::current().unwrap().actor(),
                expected
            );
            barrier.wait().await;
            tokio::task::yield_now().await;
            assert_eq!(
                VerifiedParentAuthority::current().unwrap().actor(),
                expected
            );
            let detached = tokio::spawn(async { VerifiedParentAuthority::current() })
                .await
                .unwrap();
            assert!(
                detached.is_none(),
                "spawn cannot inherit a user's authority"
            );
        })
    };
    tokio::join!(check(make(actor()), actor()), check(make(other()), other()));
    assert!(VerifiedParentAuthority::current().is_none());
}

#[tokio::test]
async fn dropping_a_scoped_handler_restores_context_and_preserves_outer_authority() {
    let body = frame(1, actor());
    let token = root(&body);
    let outer = verified_root(&token, ROOT, &body);
    let other_body = frame(1, other());
    let other_token = signer().issue(Workload::Post, ROOT, other(), None, &other_body, 100);
    let inner =
        VerifiedParentAuthority::new(other_token, ROOT.into(), other_body, None, other(), 130);
    outer
        .scope(async {
            let mut handler = Box::pin(inner.scope(async {
                assert_eq!(VerifiedParentAuthority::current().unwrap().actor(), other());
                std::future::pending::<()>().await;
            }));
            assert!(matches!(
                std::future::poll_fn(|cx| Poll::Ready(handler.as_mut().poll(cx))).await,
                Poll::Pending
            ));
            drop(handler);
            assert_eq!(VerifiedParentAuthority::current().unwrap().actor(), actor());
        })
        .await;
    assert!(VerifiedParentAuthority::current().is_none());
}
