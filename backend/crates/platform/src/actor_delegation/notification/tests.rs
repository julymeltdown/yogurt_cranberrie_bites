use super::*;
use std::sync::atomic::{AtomicUsize, Ordering};
use tower::ServiceExt;

/// Cross-process evidence must survive bounded clock skew without widening the
/// lease, replay or expiry checks. The notification evidence travels between
/// Gateway, Post and Friend, whose clocks are not synchronised; measured on this
/// host one process ran 0.04-0.15 s ahead of another, which is far beyond the
/// exact microsecond ordering `valid_time` used to require.
#[test]
fn bounded_clock_skew_is_tolerated_but_expiry_stays_exact() {
    let iat = 1_000_000;
    let exp = iat + 10_000_000;
    let lease = exp;
    // A receiver reading its clock just before the issuer is tolerated inside
    // the same bound this crate already allows between a Gateway and an owner.
    assert!(valid_time(iat, exp, iat - CLOCK_SKEW_MICROS, lease).is_ok());
    // Outside the bound it is still refused, so the tolerance is bounded and
    // cannot be used to admit a token minted meaningfully in the future.
    assert_eq!(
        valid_time(iat, exp, iat - CLOCK_SKEW_MICROS - 1, lease),
        Err(Denial::NotYetValid)
    );
    // The expiry side is unchanged: no skew is granted past the lease, so a
    // stalled or replayed proof still expires exactly where it did before.
    assert_eq!(valid_time(iat, exp, exp, lease), Err(Denial::Expired));
    assert_eq!(
        valid_time(iat, exp, iat - CLOCK_SKEW_MICROS, lease - 1),
        Err(Denial::LifetimeTooLong)
    );
}

fn page() -> NotificationPage {
    NotificationPage {
        claim: NotificationClaim {
            event_id: Uuid::new_v4(),
            aggregate_id: Uuid::new_v4(),
            event_digest: body_digest(b"stored source including trace"),
            attempt_digest: body_digest(b"INTERNAL REPLAY SKIPPED count0"),
            lease_digest: body_digest(Uuid::new_v4().as_bytes()),
            attempt_id: Uuid::new_v4(),
            attempt: 0,
            lease_expires_micros: 50_000_000,
        },
        author_id: Uuid::new_v4(),
        cursor: None,
        limit: 100,
    }
}
struct Fixture {
    owner: NotificationOwnerSigner,
    friend: NotificationFriendSigner,
    issuer: NotificationIssuer,
    verifier: NotificationVerifier,
}
impl Fixture {
    fn final_check(
        &self,
        ready: ReadyNotificationPolicy,
        policy: &str,
        now: i64,
    ) -> Result<(FinalNotificationPolicy, String), Denial> {
        let final_policy = self.issuer.prepare_final_owner(ready, policy, now)?;
        let p = final_policy.owner_request();
        let evidence = self.owner.attest(
            p.binding(),
            p.body(),
            p.challenge(),
            final_policy.ready.visibility,
            now,
            now,
        )?;
        Ok((final_policy, evidence))
    }
    fn issue(
        &self,
        ready: ReadyNotificationPolicy,
        policy: &str,
        now: i64,
    ) -> Result<NotificationIssue, Denial> {
        let (final_policy, owner) = self.final_check(ready, policy, now)?;
        self.issuer.issue_delivery(final_policy, &owner, now)
    }
    fn refresh(
        &self,
        proof: &VerifiedNotification,
        ready: ReadyNotificationPolicy,
        policy: &str,
        now: i64,
    ) -> Result<NotificationAdmission, Denial> {
        let (final_policy, owner) = self.final_check(ready, policy, now)?;
        self.issuer
            .refresh_delivery(proof, final_policy, &owner, now)
    }
    fn new(capacity: usize) -> Self {
        let policy = LifetimePolicy::new(1_000_000).unwrap();
        let owner = NotificationOwnerSigner::new(&[61; 32], policy).unwrap();
        let gateway = Key::new(&[62; 32]).unwrap();
        let friend =
            NotificationFriendSigner::new(&[63; 32], &[gateway.public_key()], policy).unwrap();
        let issuer = NotificationIssuer::new(
            &[62; 32],
            &[owner.public_key()],
            &[friend.public_key()],
            policy,
        )
        .unwrap();
        let verifier = NotificationVerifier::new(&[issuer.public_key()], capacity).unwrap();
        Self {
            owner,
            friend,
            issuer,
            verifier,
        }
    }
    fn prepared(
        &self,
        binding: NotificationBinding,
        visibility: Option<SourceVisibility>,
        now: i64,
    ) -> (PreparedNotification, String) {
        let body = binding.frame().unwrap();
        let p = self
            .issuer
            .prepare(Workload::Post, binding, body, now)
            .unwrap();
        let evidence = self
            .owner
            .attest(p.binding(), p.body(), p.challenge(), visibility, now, now)
            .unwrap();
        (p, evidence)
    }
    fn page_grant(&self, page: NotificationPage, now: i64) -> IssuedNotification {
        let (p, e) = self.prepared(
            NotificationBinding::FollowerPage(page),
            Some(SourceVisibility::Public),
            now,
        );
        self.issuer.issue_page(p, &e, now).unwrap()
    }
    fn membership(&self, page: NotificationPage, members: Vec<Uuid>, now: i64, exp: i64) -> String {
        // Unit fixture signs a recorded Friend observation. Guard tests below
        // separately exercise the production sealed-admission signer method.
        self.friend
            .key
            .sign(
                MEMBERSHIP_CONTEXT,
                &Membership {
                    v: 1,
                    kid: self.friend.key.id.clone(),
                    iss: "friend".into(),
                    aud: "gateway".into(),
                    purpose: "notification_selected_membership".into(),
                    method: PAGE_METHOD.into(),
                    page,
                    members,
                    next_cursor: None,
                    iat: now,
                    exp,
                },
            )
            .unwrap()
    }
    fn delivery(&self, page: NotificationPage, recipient: Uuid, now: i64) -> NotificationDelivery {
        let membership = self.membership(page.clone(), vec![recipient], now, now + 900_000);
        NotificationDelivery {
            post_id: page.claim.aggregate_id,
            claim: page.claim,
            recipient_id: recipient,
            actor_id: page.author_id,
            kind: NotificationKind::Created,
            occurred_at: "2026-10-14T00:00:00+00:00".into(),
            membership: Some(membership),
        }
    }
    fn ready(
        &self,
        d: NotificationDelivery,
        visibility: Option<SourceVisibility>,
        now: i64,
    ) -> ReadyNotificationPolicy {
        let (p, e) = self.prepared(NotificationBinding::Delivery(d), visibility, now);
        self.issuer.prepare_policy(p, &e, now).unwrap()
    }
    fn policy(
        &self,
        ready: &ReadyNotificationPolicy,
        blocked: bool,
        muted: bool,
        following: bool,
        now: i64,
    ) -> String {
        let q = self
            .friend
            .verify_policy_query(ready.token(), Workload::Gateway, POLICY_METHOD, now)
            .unwrap();
        self.friend
            .policy(q, blocked, muted, following, now, now)
            .unwrap()
    }
    fn grant(
        &self,
        d: NotificationDelivery,
        visibility: Option<SourceVisibility>,
        now: i64,
    ) -> IssuedNotification {
        let ready = self.ready(d, visibility, now);
        let policy = self.policy(&ready, false, false, true, now);
        match self.issue(ready, &policy, now).unwrap() {
            NotificationIssue::Granted(g) => g,
            _ => panic!("unexpected suppression"),
        }
    }
}
fn direct() -> NotificationDelivery {
    let p = page();
    NotificationDelivery {
        post_id: p.claim.aggregate_id,
        claim: p.claim,
        recipient_id: Uuid::new_v4(),
        actor_id: p.author_id,
        kind: NotificationKind::Liked,
        occurred_at: "2026-10-14T00:00:00+00:00".into(),
        membership: None,
    }
}

#[test]
fn owning_source_suppression_is_challenged_bounded_and_cannot_issue_a_grant() {
    let f = Fixture::new(8);
    let binding = NotificationBinding::FollowerPage(page());
    let (p, _) = f.prepared(binding.clone(), Some(SourceVisibility::Public), 100);
    let e = f
        .owner
        .attest_suppressed(p.binding(), p.body(), p.challenge(), 101, 101)
        .unwrap();
    let a = f.issuer.source_suppression(&p, &e, 101).unwrap().unwrap();
    assert!(a.decision == NotificationDecision::Suppressed);
    assert_eq!(a.expires_micros, 1_000_101);
    assert!(f.issuer.issue_page(p, &e, 101).is_err());
    let (other, _) = f.prepared(binding, Some(SourceVisibility::Public), 102);
    assert!(f.issuer.source_suppression(&other, &e, 102).is_err());
    let (d, _) = f.prepared(NotificationBinding::Delivery(direct()), None, 100);
    assert!(f
        .owner
        .attest_suppressed(d.binding(), d.body(), d.challenge(), 100, 100)
        .is_err());
}

#[test]
fn suppressed_page_refresh_keeps_original_deadline_and_requires_live_exact_evidence() {
    let f = Fixture::new(8);
    let page = page();
    let body = page.frame().unwrap();
    let g = f.page_grant(page.clone(), 100);
    let proof = f
        .verifier
        .verify(&g.token, Workload::Post, PAGE_METHOD, &body, 101)
        .unwrap();
    let (p, _) = f.prepared(
        NotificationBinding::FollowerPage(page.clone()),
        Some(SourceVisibility::Public),
        200,
    );
    let e = f
        .owner
        .attest_suppressed(p.binding(), p.body(), p.challenge(), 201, 201)
        .unwrap();
    let a = f.issuer.refresh_page(&proof, p, &e, 201).unwrap();
    assert!(a.decision == NotificationDecision::Suppressed);
    assert_eq!(a.expires_micros, g.expires_micros);
    let (p, _) = f.prepared(
        NotificationBinding::FollowerPage(page),
        Some(SourceVisibility::Public),
        202,
    );
    assert!(f
        .issuer
        .refresh_source_suppression(&proof, &p, &e, 202)
        .is_err());
    let e = f
        .owner
        .attest_suppressed(p.binding(), p.body(), p.challenge(), 203, 203)
        .unwrap();
    assert!(f
        .issuer
        .refresh_source_suppression(&proof, &p, &e, g.expires_micros)
        .is_err());
}

#[test]
fn source_suppressed_after_friend_policy_cannot_become_an_allowed_write() {
    let f = Fixture::new(8);
    let delivery = f.delivery(page(), Uuid::new_v4(), 100);
    let grant = f.grant(delivery.clone(), Some(SourceVisibility::Public), 100);
    let proof = f
        .verifier
        .verify(
            &grant.token,
            Workload::Post,
            WRITE_METHOD,
            &delivery.frame().unwrap(),
            101,
        )
        .unwrap();
    for admission in [false, true] {
        let ready = f.ready(delivery.clone(), Some(SourceVisibility::Public), 102);
        let policy = f.policy(&ready, false, false, true, 103);
        let final_policy = f.issuer.prepare_final_owner(ready, &policy, 104).unwrap();
        let p = final_policy.owner_request();
        let short_owner =
            NotificationOwnerSigner::new(&[61; 32], LifetimePolicy::new(10).unwrap()).unwrap();
        let e = short_owner
            .attest_suppressed(p.binding(), p.body(), p.challenge(), 105, 105)
            .unwrap();
        if admission {
            let a = f
                .issuer
                .refresh_delivery(&proof, final_policy, &e, 105)
                .unwrap();
            assert!(a.decision == NotificationDecision::Suppressed);
            assert_eq!(a.expires_micros, 115);
        } else {
            assert!(matches!(
                f.issuer.issue_delivery(final_policy, &e, 105).unwrap(),
                NotificationIssue::Suppressed {
                    expires_micros: 115
                }
            ));
        }
    }
}

#[test]
fn initial_suppressed_delivery_refresh_is_exact_and_capped_without_policy_fabrication() {
    let f = Fixture::new(8);
    let delivery = f.delivery(page(), Uuid::new_v4(), 100);
    let grant = f.grant(delivery.clone(), Some(SourceVisibility::Public), 100);
    let proof = f
        .verifier
        .verify(
            &grant.token,
            Workload::Post,
            WRITE_METHOD,
            &delivery.frame().unwrap(),
            101,
        )
        .unwrap();
    let (p, _) = f.prepared(
        NotificationBinding::Delivery(delivery),
        Some(SourceVisibility::Public),
        102,
    );
    let e = f
        .owner
        .attest_suppressed(p.binding(), p.body(), p.challenge(), 103, 103)
        .unwrap();
    let a = f
        .issuer
        .refresh_source_suppression(&proof, &p, &e, 103)
        .unwrap()
        .unwrap();
    assert!(a.decision == NotificationDecision::Suppressed);
    assert_eq!(a.expires_micros, grant.expires_micros);
    assert!(f.issuer.prepare_policy(p, &e, 103).is_err());
}

#[test]
fn initial_suppression_respects_gateway_lifetime_even_with_longer_owner_evidence() {
    let mut f = Fixture::new(8);
    f.issuer.policy = LifetimePolicy::new(10).unwrap();
    let (p, _) = f.prepared(
        NotificationBinding::FollowerPage(page()),
        Some(SourceVisibility::Public),
        100,
    );
    let evidence = f
        .owner
        .attest_suppressed(p.binding(), p.body(), p.challenge(), 101, 101)
        .unwrap();
    let result = f
        .issuer
        .source_suppression(&p, &evidence, 102)
        .unwrap()
        .unwrap();
    assert!(result.decision == NotificationDecision::Suppressed);
    assert_eq!(result.expires_micros, 112);
}
fn token_value(token: &str) -> serde_json::Value {
    serde_json::from_slice(
        &Base64UrlUnpadded::decode_vec(token.split_once('.').unwrap().0).unwrap(),
    )
    .unwrap()
}

#[test]
fn zero_count_replay_is_admitted_without_charging_or_changing_claim() {
    let f = Fixture::new(2);
    let p = page();
    let body = p.frame().unwrap();
    let grant = f.page_grant(p.clone(), 100);
    let proof = f
        .verifier
        .verify(&grant.token, Workload::Post, PAGE_METHOD, &body, 100)
        .unwrap();
    assert!(proof.binding() == &NotificationBinding::FollowerPage(p));
    assert_eq!(proof.binding().claim().attempt, 0);
    assert!(!grant.token.contains("lease_token"));
}
#[test]
fn independent_fresh_owner_challenge_cannot_be_reused_for_another_request() {
    let f = Fixture::new(2);
    let b = NotificationBinding::FollowerPage(page());
    let (first, e) = f.prepared(b.clone(), Some(SourceVisibility::Public), 100);
    let (second, _) = f.prepared(b, Some(SourceVisibility::Public), 100);
    assert_ne!(first.challenge(), second.challenge());
    assert_eq!(
        f.issuer.issue_page(second, &e, 100).err(),
        Some(Denial::WrongBody)
    );
    assert!(f.issuer.issue_page(first, &e, 100).is_ok());
}
#[test]
fn every_source_attempt_and_page_field_is_bound_to_the_owner_signature() {
    let f = Fixture::new(2);
    let p = page();
    for case in 0..12 {
        let (original, _) = f.prepared(
            NotificationBinding::FollowerPage(p.clone()),
            Some(SourceVisibility::Public),
            100,
        );
        let mut changed = p.clone();
        match case {
            0 => changed.claim.event_id = Uuid::new_v4(),
            1 => changed.claim.aggregate_id = Uuid::new_v4(),
            2 => changed.claim.event_digest = body_digest(b"other event"),
            3 => changed.claim.attempt_digest = body_digest(b"AUTO"),
            4 => changed.claim.lease_digest = body_digest(b"other lease"),
            5 => changed.claim.attempt_id = Uuid::new_v4(),
            6 => changed.claim.attempt = 1,
            7 => changed.claim.lease_expires_micros += 1,
            8 => changed.author_id = Uuid::new_v4(),
            9 => changed.cursor = Some(format!("2026-10-14T00:00:00Z|{}", Uuid::new_v4())),
            10 => changed.limit = 99,
            _ => {}
        }
        let b = NotificationBinding::FollowerPage(changed);
        let body = b.frame().unwrap();
        let e = f
            .owner
            .attest(
                &b,
                &body,
                original.challenge(),
                Some(if case == 11 {
                    SourceVisibility::Friends
                } else {
                    SourceVisibility::Public
                }),
                100,
                100,
            )
            .unwrap();
        if case == 11 {
            // Visibility is an owner observation; fresh FRIENDS is valid and
            // must replace PUBLIC rather than be taken from the original grant.
            assert!(f.issuer.issue_page(original, &e, 100).is_ok());
        } else {
            assert_eq!(
                f.issuer.issue_page(original, &e, 100).err(),
                Some(Denial::WrongBody)
            );
        }
    }
}
#[test]
fn noncanonical_envelopes_and_noncanonical_wire_bodies_fail_before_issuance() {
    let f = Fixture::new(2);
    let b = NotificationBinding::FollowerPage(page());
    let canonical = b.encode().unwrap();
    assert!(NotificationBinding::decode(&canonical).is_ok());
    let mut whitespace = canonical.clone();
    whitespace.push(b' ');
    assert!(NotificationBinding::decode(&whitespace).is_err());
    let mut value: serde_json::Value = serde_json::from_slice(&canonical).unwrap();
    value["unknown"] = true.into();
    assert!(NotificationBinding::decode(&serde_json::to_vec(&value).unwrap()).is_err());
    let mut duplicate = canonical;
    duplicate.splice(1..1, b"\"purpose\":\"FollowerPage\",".iter().copied());
    assert!(NotificationBinding::decode(&duplicate).is_err());
    let body = b.frame().unwrap();
    let mut compressed = body.clone();
    compressed[0] = 1;
    let mut unknown = body.clone();
    unknown.extend([120, 1]);
    let len = (unknown.len() - 5) as u32;
    unknown[1..5].copy_from_slice(&len.to_be_bytes());
    let mut doubled = body.clone();
    doubled.extend_from_slice(&body);
    for body in [vec![], compressed, unknown, doubled] {
        assert!(f
            .issuer
            .prepare(Workload::Post, b.clone(), body, 100)
            .is_err());
    }
}
#[test]
fn invalid_subjects_defaults_and_visibility_shapes_are_rejected() {
    let f = Fixture::new(2);
    for case in 0..11 {
        let mut d = direct();
        match case {
            0 => d.recipient_id = Uuid::nil(),
            1 => d.actor_id = Uuid::nil(),
            2 => d.recipient_id = d.actor_id,
            3 => d.claim.attempt = -1,
            4 => d.claim.attempt_id = Uuid::nil(),
            5 => d.post_id = Uuid::new_v4(),
            6 => d.occurred_at = String::new(),
            7 => d.occurred_at.push(' '),
            8 => d.membership = Some("untrusted".into()),
            9 => d.claim.event_digest = "wrong".into(),
            _ => d.claim.lease_expires_micros = 0,
        }
        assert!(d.frame().is_err());
    }
    let (p, _) = f.prepared(NotificationBinding::Delivery(direct()), None, 100);
    assert!(f
        .owner
        .attest(
            p.binding(),
            p.body(),
            p.challenge(),
            Some(SourceVisibility::Public),
            100,
            100
        )
        .is_err());
    let b = NotificationBinding::FollowerPage(page());
    assert!(f
        .owner
        .attest(&b, &b.frame().unwrap(), Uuid::new_v4(), None, 100, 100)
        .is_err());
    for lifetime in [0, -1, MAX_LIFETIME + 1] {
        assert!(LifetimePolicy::new(lifetime).is_err());
    }
    assert!(NotificationVerifier::new(&[], 1).is_err());
    assert!(NotificationVerifier::new(&[[0; 32]], 0).is_err());
}
#[test]
fn direct_and_legacy_intents_preserve_identity_without_requiring_membership() {
    let f = Fixture::new(8);
    for kind in [
        NotificationKind::Liked,
        NotificationKind::Commented,
        NotificationKind::LegacyComment,
    ] {
        let mut d = direct();
        d.kind = kind;
        if kind == NotificationKind::LegacyComment {
            d.post_id = Uuid::new_v4();
        }
        let body = d.frame().unwrap();
        let g = f.grant(d.clone(), None, 100);
        let proof = f
            .verifier
            .verify(&g.token, Workload::Post, WRITE_METHOD, &body, 100)
            .unwrap();
        assert!(proof.binding() == &NotificationBinding::Delivery(d));
    }
}
#[test]
fn selected_public_follower_can_unfollow_but_arbitrary_recipient_is_denied() {
    let f = Fixture::new(2);
    let d = f.delivery(page(), Uuid::new_v4(), 100);
    let ready = f.ready(d.clone(), Some(SourceVisibility::Public), 100);
    let policy = f.policy(&ready, false, false, false, 101);
    assert!(matches!(
        f.issue(ready, &policy, 102),
        Ok(NotificationIssue::Granted(_))
    ));
    let mut outsider = d;
    outsider.recipient_id = Uuid::new_v4();
    let (p, e) = f.prepared(
        NotificationBinding::Delivery(outsider),
        Some(SourceVisibility::Public),
        103,
    );
    assert_eq!(
        f.issuer.prepare_policy(p, &e, 103).err(),
        Some(Denial::WrongBody)
    );
}
#[test]
fn membership_binds_exact_claim_author_and_page_and_refuses_forged_or_cross_context_tokens() {
    let f = Fixture::new(2);
    let p = page();
    let id = Uuid::new_v4();
    let token = f.membership(p.clone(), vec![id], 100, 900_000);
    assert!(f
        .issuer
        .membership
        .verify_page(&p, &[id], None, &token, 100)
        .is_ok());
    let mut another = p.clone();
    another.limit = 99;
    assert!(f
        .issuer
        .membership
        .verify_page(&another, &[id], None, &token, 100)
        .is_err());
    assert!(f
        .issuer
        .membership
        .verify_page(&p, &[Uuid::new_v4()], None, &token, 100)
        .is_err());
    for case in 0..5 {
        let mut d = f.delivery(p.clone(), id, 100);
        match case {
            0 => d.claim.attempt_id = Uuid::new_v4(),
            1 => d.actor_id = Uuid::new_v4(),
            2 => {
                let mut s = d.membership.take().unwrap();
                s.push('a');
                d.membership = Some(s);
            }
            3 => {
                d.membership = Some(
                    f.friend
                        .key
                        .sign(POLICY_CONTEXT, &token_value(&token))
                        .unwrap(),
                )
            }
            _ => {
                let key = Key::new(&[64; 32]).unwrap();
                let mut value = token_value(&token);
                value["kid"] = key.id.clone().into();
                d.membership = Some(key.sign(MEMBERSHIP_CONTEXT, &value).unwrap());
            }
        }
        let (p, e) = f.prepared(
            NotificationBinding::Delivery(d),
            Some(SourceVisibility::Public),
            100,
        );
        assert!(f.issuer.prepare_policy(p, &e, 100).is_err());
    }
}
#[test]
fn page_membership_rejects_duplicates_self_nil_overflow_and_cursor_regression() {
    let mut p = page();
    let id = Uuid::new_v4();
    for members in [
        vec![id, id],
        vec![p.author_id],
        vec![Uuid::nil()],
        vec![id; 101],
    ] {
        assert!(page_members(&p, &members, None).is_err());
    }
    assert!(page_members(
        &p,
        &[id],
        Some(&format!("2026-10-14T00:00:00Z|{}", Uuid::new_v4()))
    )
    .is_err());
    p.cursor = Some(format!("2026-10-14T00:00:00Z|{id}"));
    assert!(page_members(&p, &[id], p.cursor.as_deref()).is_err());
    assert!(page_members(&p, &[id], Some(&format!("2026-10-13T00:00:00Z|{id}"))).is_ok());
    p.cursor = Some(String::new());
    assert!(p.frame().is_err());
}
#[test]
fn membership_expiry_caps_policy_query_and_delivery_grant() {
    let f = Fixture::new(2);
    let p = page();
    let id = Uuid::new_v4();
    let mut d = f.delivery(p.clone(), id, 100);
    d.membership = Some(f.membership(p, vec![id], 100, 150));
    let ready = f.ready(d.clone(), Some(SourceVisibility::Public), 101);
    assert_eq!(ready.expires_micros(), 150);
    let policy = f.policy(&ready, false, false, false, 102);
    let NotificationIssue::Granted(g) = f.issue(ready, &policy, 103).unwrap() else {
        panic!("suppressed")
    };
    assert_eq!(g.expires_micros, 150);
    assert_eq!(
        f.verifier
            .verify(
                &g.token,
                Workload::Post,
                WRITE_METHOD,
                &d.frame().unwrap(),
                150
            )
            .err(),
        Some(Denial::Expired)
    );
    let (p, e) = f.prepared(
        NotificationBinding::Delivery(d),
        Some(SourceVisibility::Public),
        150,
    );
    assert_eq!(
        f.issuer.prepare_policy(p, &e, 150).err(),
        Some(Denial::Expired)
    );
}
#[test]
fn every_relationship_denial_is_successful_suppression_and_friends_requires_following() {
    let f = Fixture::new(2);
    for (visibility, blocked, muted, following, allowed) in [
        (SourceVisibility::Public, false, false, false, true),
        (SourceVisibility::Public, true, false, true, false),
        (SourceVisibility::Public, false, true, true, false),
        (SourceVisibility::Friends, false, false, false, false),
        (SourceVisibility::Friends, false, false, true, true),
    ] {
        let d = f.delivery(page(), Uuid::new_v4(), 100);
        let ready = f.ready(d, Some(visibility), 100);
        let policy = f.policy(&ready, blocked, muted, following, 101);
        assert_eq!(
            matches!(
                f.issue(ready, &policy, 102).unwrap(),
                NotificationIssue::Granted(_)
            ),
            allowed
        );
    }
}
#[test]
fn fresh_policy_nonce_prevents_reusing_a_valid_response_for_another_delivery_or_visibility() {
    let f = Fixture::new(2);
    let d = f.delivery(page(), Uuid::new_v4(), 100);
    let ready = f.ready(d.clone(), Some(SourceVisibility::Public), 100);
    let old = f.policy(&ready, false, false, false, 100);
    let another = f.ready(d.clone(), Some(SourceVisibility::Public), 100);
    assert_ne!(ready.token(), another.token());
    assert_eq!(f.issue(another, &old, 100).err(), Some(Denial::WrongBody));
    let friends = f.ready(d, Some(SourceVisibility::Friends), 101);
    assert_eq!(f.issue(friends, &old, 101).err(), Some(Denial::WrongBody));
}
#[test]
fn policy_query_requires_gateway_identity_exact_method_pinned_key_and_live_lifetime() {
    let f = Fixture::new(2);
    let ready = f.ready(direct(), None, 100);
    assert_eq!(
        f.friend
            .verify_policy_query(ready.token(), Workload::Post, POLICY_METHOD, 100)
            .err(),
        Some(Denial::WrongIssuer)
    );
    assert_eq!(
        f.friend
            .verify_policy_query(ready.token(), Workload::Gateway, WRITE_METHOD, 100)
            .err(),
        Some(Denial::WrongMethod)
    );
    assert_eq!(
        f.friend
            .verify_policy_query(ready.token(), Workload::Gateway, POLICY_METHOD, ready.exp)
            .err(),
        Some(Denial::Expired)
    );
    let other = NotificationFriendSigner::new(
        &[63; 32],
        &[Key::new(&[64; 32]).unwrap().public_key()],
        f.friend.policy,
    )
    .unwrap();
    assert_eq!(
        other
            .verify_policy_query(ready.token(), Workload::Gateway, POLICY_METHOD, 100)
            .err(),
        Some(Denial::UnknownKey)
    );
    for field in [
        "nonce",
        "actor_id",
        "binding_digest",
        "purpose",
        "aud",
        "pres",
    ] {
        let mut value = token_value(ready.token());
        value[field] = "invalid".into();
        let forged = f.issuer.key.sign(POLICY_QUERY_CONTEXT, &value).unwrap();
        assert!(f
            .friend
            .verify_policy_query(&forged, Workload::Gateway, POLICY_METHOD, 100)
            .is_err());
    }
}
#[test]
fn admission_refresh_uses_current_friends_policy_instead_of_earlier_public_observation() {
    let f = Fixture::new(2);
    let d = f.delivery(page(), Uuid::new_v4(), 100);
    let body = d.frame().unwrap();
    let g = f.grant(d.clone(), Some(SourceVisibility::Public), 100);
    let proof = f
        .verifier
        .verify(&g.token, Workload::Post, WRITE_METHOD, &body, 100)
        .unwrap();
    let ready = f.ready(d.clone(), Some(SourceVisibility::Friends), 101);
    let policy = f.policy(&ready, false, false, false, 101);
    assert!(matches!(
        f.refresh(&proof, ready, &policy, 102),
        Ok(NotificationAdmission {
            decision: NotificationDecision::Suppressed,
            ..
        })
    ));
    let ready = f.ready(d, Some(SourceVisibility::Friends), 102);
    let policy = f.policy(&ready, false, false, true, 102);
    assert!(matches!(
        f.refresh(&proof, ready, &policy, 103),
        Ok(NotificationAdmission {
            decision: NotificationDecision::Allowed {
                visibility: Some(SourceVisibility::Friends)
            },
            ..
        })
    ));
}
#[test]
fn page_refresh_requires_new_owning_challenge_and_exact_original_body() {
    let f = Fixture::new(2);
    let p = page();
    let body = p.frame().unwrap();
    let g = f.page_grant(p.clone(), 100);
    let proof = f
        .verifier
        .verify(&g.token, Workload::Post, PAGE_METHOD, &body, 100)
        .unwrap();
    let (ready, e) = f.prepared(
        NotificationBinding::FollowerPage(p.clone()),
        Some(SourceVisibility::Friends),
        101,
    );
    assert!(matches!(
        f.issuer.refresh_page(&proof, ready, &e, 102),
        Ok(NotificationAdmission {
            decision: NotificationDecision::Allowed {
                visibility: Some(SourceVisibility::Friends)
            },
            ..
        })
    ));
    let mut changed = p;
    changed.limit = 99;
    let (ready, e) = f.prepared(
        NotificationBinding::FollowerPage(changed),
        Some(SourceVisibility::Public),
        103,
    );
    assert_eq!(
        f.issuer.refresh_page(&proof, ready, &e, 103).err(),
        Some(Denial::WrongBody)
    );
}
#[test]
fn grant_rejects_wrong_presenter_method_body_key_signature_and_claim_metadata() {
    let f = Fixture::new(2);
    let d = direct();
    let body = d.frame().unwrap();
    let g = f.grant(d, None, 100);
    assert_eq!(
        f.verifier
            .verify(&g.token, Workload::Gateway, WRITE_METHOD, &body, 100)
            .err(),
        Some(Denial::WrongIssuer)
    );
    assert_eq!(
        f.verifier
            .verify(&g.token, Workload::Post, PAGE_METHOD, &body, 100)
            .err(),
        Some(Denial::WrongMethod)
    );
    let mut changed = body.clone();
    *changed.last_mut().unwrap() ^= 1;
    assert_eq!(
        f.verifier
            .verify(&g.token, Workload::Post, WRITE_METHOD, &changed, 100)
            .err(),
        Some(Denial::WrongBody)
    );
    for (field, value) in [
        ("iss", "post"),
        ("aud", "friend"),
        ("pres", "gateway"),
        ("purpose", "claimed_cache"),
        ("jti", "00000000-0000-0000-0000-000000000000"),
    ] {
        let mut grant = token_value(&g.token);
        grant[field] = value.into();
        let forged = f.issuer.key.sign(GRANT_CONTEXT, &grant).unwrap();
        assert!(f
            .verifier
            .verify(&forged, Workload::Post, WRITE_METHOD, &body, 100)
            .is_err());
    }
    let mut token = g.token.clone();
    token.push('a');
    assert!(f
        .verifier
        .verify(&token, Workload::Post, WRITE_METHOD, &body, 100)
        .is_err());
    let other = NotificationVerifier::new(&[Key::new(&[64; 32]).unwrap().public_key()], 2).unwrap();
    assert_eq!(
        other
            .verify(&g.token, Workload::Post, WRITE_METHOD, &body, 100)
            .err(),
        Some(Denial::UnknownKey)
    );
}
#[test]
fn cache_and_notification_grants_cannot_cross_even_with_identical_signing_keys() {
    use crate::actor_delegation::cache;
    let f = Fixture::new(2);
    let p = page();
    let policy = cache::LifetimePolicy::new(1_000_000).unwrap();
    let owner = cache::CacheOwnerSigner::new(&[61; 32], policy).unwrap();
    let issuer = cache::CacheIssuer::new(&[62; 32], &[owner.public_key()], policy).unwrap();
    let verifier = cache::CacheVerifier::new(&[issuer.public_key()], 2).unwrap();
    let scope = cache::CacheScope {
        event_id: p.claim.event_id,
        attempt_id: p.claim.attempt_id,
        post_id: p.claim.aggregate_id,
        author_id: p.author_id,
        lease_digest: p.claim.lease_digest.clone(),
        event_digest: p.claim.event_digest.clone(),
        attempt_digest: p.claim.attempt_digest.clone(),
        lease_expires_micros: p.claim.lease_expires_micros,
        attempt: 1,
        cursor: None,
        limit: 100,
    };
    let body = vec![0, 0, 0, 0, 0];
    let prepared = issuer
        .prepare(Workload::Post, scope.clone(), body.clone(), 100)
        .unwrap();
    let evidence = owner
        .attest(scope, &body, prepared.challenge(), 100, 100)
        .unwrap();
    let g = issuer.finish(prepared, &evidence, 100).unwrap();
    assert!(verifier
        .verify(&g.token, Workload::Post, cache::METHOD, &body, 100)
        .is_ok());
    assert!(f
        .verifier
        .verify(
            &g.token,
            Workload::Post,
            PAGE_METHOD,
            &p.frame().unwrap(),
            100
        )
        .is_err());
    let n = f.page_grant(p, 100);
    assert!(verifier
        .verify(&n.token, Workload::Post, cache::METHOD, &body, 100)
        .is_err());
}
#[test]
fn hundred_member_witness_and_embedded_delivery_grant_fit_explicit_bounds() {
    let f = Fixture::new(2);
    let p = page();
    let members: Vec<_> = (0..100).map(|_| Uuid::new_v4()).collect();
    let token = f.membership(p.clone(), members.clone(), 100, 900_000);
    assert!(token.len() <= MAX_WITNESS);
    f.issuer
        .membership
        .verify_page(&p, &members, None, &token, 100)
        .unwrap();
    let mut d = f.delivery(p, members[99], 100);
    d.membership = Some(token);
    let g = f.grant(d, Some(SourceVisibility::Public), 100);
    assert!(g.token.len() <= MAX_BYTES);
}
#[test]
fn live_replay_entries_are_not_evicted_and_expired_entries_are_reclaimed() {
    let f = Fixture::new(1);
    let mut p = page();
    p.claim.lease_expires_micros = 150;
    let body = p.frame().unwrap();
    let first = f.page_grant(p.clone(), 100);
    let a = f
        .verifier
        .verify(&first.token, Workload::Post, PAGE_METHOD, &body, 100)
        .unwrap();
    f.verifier.consume(&a, a.exp, || 100).unwrap();
    let second = f.page_grant(p, 100);
    let b = f
        .verifier
        .verify(&second.token, Workload::Post, PAGE_METHOD, &body, 100)
        .unwrap();
    assert_eq!(
        f.verifier.consume(&b, b.exp, || 100),
        Err(Denial::ReplayCacheFull)
    );
    assert_eq!(f.verifier.consume(&a, a.exp, || 100), Err(Denial::Replayed));
    let p = page();
    let g = f.page_grant(p.clone(), 151);
    let c = f
        .verifier
        .verify(
            &g.token,
            Workload::Post,
            PAGE_METHOD,
            &p.frame().unwrap(),
            151,
        )
        .unwrap();
    f.verifier.consume(&c, c.exp, || 151).unwrap();
}
#[test]
fn cloned_verifiers_share_replay_state_and_concurrent_consumption_has_one_winner() {
    let f = Fixture::new(2);
    let p = page();
    let g = f.page_grant(p.clone(), 100);
    let proof = f
        .verifier
        .verify(
            &g.token,
            Workload::Post,
            PAGE_METHOD,
            &p.frame().unwrap(),
            100,
        )
        .unwrap();
    let barrier = Arc::new(std::sync::Barrier::new(12));
    let threads: Vec<_> = (0..12)
        .map(|_| {
            let v = f.verifier.clone();
            let p = proof.clone();
            let b = barrier.clone();
            std::thread::spawn(move || {
                b.wait();
                v.consume(&p, p.exp, || 100)
            })
        })
        .collect();
    assert_eq!(
        threads
            .into_iter()
            .map(|t| t.join().unwrap())
            .filter(Result::is_ok)
            .count(),
        1
    );
}
#[test]
fn replay_clock_is_sampled_under_mutex_and_after_expired_entry_cleanup() {
    let f = Fixture::new(1);
    let p = page();
    let g = f.page_grant(p.clone(), 100);
    let proof = f
        .verifier
        .verify(
            &g.token,
            Workload::Post,
            PAGE_METHOD,
            &p.frame().unwrap(),
            100,
        )
        .unwrap();
    f.verifier.used.lock().unwrap().insert(Uuid::new_v4(), 99);
    let mut count = 0;
    assert_eq!(
        f.verifier.consume(&proof, proof.exp, || {
            assert!(f.verifier.used.try_lock().is_err());
            count += 1;
            if count == 1 {
                100
            } else {
                proof.exp
            }
        }),
        Err(Denial::Expired)
    );
    assert_eq!(count, 2);
    assert!(f.verifier.used.lock().unwrap().is_empty());
}

#[test]
fn post_policy_owner_challenge_rejects_stale_source_and_visibility_changes() {
    let f = Fixture::new(4);
    let d = f.delivery(page(), Uuid::new_v4(), 100);
    for following in [false, true] {
        let ready = f.ready(d.clone(), Some(SourceVisibility::Public), 100);
        let old_owner = f
            .owner
            .attest(
                ready.prepared.binding(),
                ready.prepared.body(),
                ready.prepared.challenge(),
                Some(SourceVisibility::Public),
                100,
                100,
            )
            .unwrap();
        let policy = f.policy(&ready, false, false, following, 101);
        let final_policy = f.issuer.prepare_final_owner(ready, &policy, 102).unwrap();
        assert_eq!(
            f.issuer.issue_delivery(final_policy, &old_owner, 102).err(),
            Some(Denial::WrongBody)
        );
        let ready = f.ready(d.clone(), Some(SourceVisibility::Public), 103);
        let policy = f.policy(&ready, false, false, following, 104);
        let final_policy = f.issuer.prepare_final_owner(ready, &policy, 105).unwrap();
        let p = final_policy.owner_request();
        let changed = f
            .owner
            .attest(
                p.binding(),
                p.body(),
                p.challenge(),
                Some(SourceVisibility::Friends),
                106,
                106,
            )
            .unwrap();
        assert_eq!(
            f.issuer.issue_delivery(final_policy, &changed, 106).err(),
            Some(Denial::WrongBody)
        );
    }
    // Suppression is fenced too: a policy denial cannot bypass the final owner.
    let ready = f.ready(d, Some(SourceVisibility::Public), 110);
    let denied = f.policy(&ready, true, false, false, 111);
    let final_policy = f.issuer.prepare_final_owner(ready, &denied, 112).unwrap();
    assert!(f
        .issuer
        .issue_delivery(final_policy, "no owning response", 113)
        .is_err());
}
#[test]
fn refreshed_owner_and_policy_expiry_are_carried_to_admission_and_sealed_lifetime() {
    let mut f = Fixture::new(4);
    let d = direct();
    let body = d.frame().unwrap();
    let g = f.grant(d.clone(), None, 100);
    let proof = f
        .verifier
        .verify(&g.token, Workload::Post, WRITE_METHOD, &body, 100)
        .unwrap();
    f.friend.policy = LifetimePolicy::new(20).unwrap();
    let ready = f.ready(d, None, 101);
    let policy = f.policy(&ready, false, false, false, 102);
    let final_policy = f.issuer.prepare_final_owner(ready, &policy, 103).unwrap();
    let p = final_policy.owner_request();
    let short_owner =
        NotificationOwnerSigner::new(&[61; 32], LifetimePolicy::new(10).unwrap()).unwrap();
    let owner = short_owner
        .attest(p.binding(), p.body(), p.challenge(), None, 104, 104)
        .unwrap();
    let admission = f
        .issuer
        .refresh_delivery(&proof, final_policy, &owner, 105)
        .unwrap();
    assert_eq!(admission.expires_micros, 114);
    assert!(proof.exp > admission.expires_micros);
    assert_eq!(
        f.verifier.consume(&proof, admission.expires_micros, || 114),
        Err(Denial::Expired)
    );
    // Page admission also retains its refreshed owner's shorter expiry.
    let p = page();
    let g = f.page_grant(p.clone(), 100);
    let proof = f
        .verifier
        .verify(
            &g.token,
            Workload::Post,
            PAGE_METHOD,
            &p.frame().unwrap(),
            100,
        )
        .unwrap();
    let b = NotificationBinding::FollowerPage(p);
    let prepared = f
        .issuer
        .prepare(Workload::Post, b.clone(), b.frame().unwrap(), 101)
        .unwrap();
    let owner = short_owner
        .attest(
            prepared.binding(),
            prepared.body(),
            prepared.challenge(),
            Some(SourceVisibility::Public),
            102,
            102,
        )
        .unwrap();
    assert_eq!(
        f.issuer
            .refresh_page(&proof, prepared, &owner, 103)
            .unwrap()
            .expires_micros,
        112
    );
}
#[test]
fn policy_expiry_during_final_post_await_denies_issuance_and_admission() {
    let mut f = Fixture::new(4);
    let d = direct();
    let g = f.grant(d.clone(), None, 100);
    let proof = f
        .verifier
        .verify(
            &g.token,
            Workload::Post,
            WRITE_METHOD,
            &d.frame().unwrap(),
            100,
        )
        .unwrap();
    f.friend.policy = LifetimePolicy::new(10).unwrap();
    for admission in [false, true] {
        let ready = f.ready(d.clone(), None, 101);
        let policy = f.policy(&ready, false, false, false, 102);
        let final_policy = f.issuer.prepare_final_owner(ready, &policy, 103).unwrap();
        let p = final_policy.owner_request();
        let owner = f
            .owner
            .attest(p.binding(), p.body(), p.challenge(), None, 113, 113)
            .unwrap();
        let error = if admission {
            f.issuer
                .refresh_delivery(&proof, final_policy, &owner, 113)
                .err()
        } else {
            f.issuer.issue_delivery(final_policy, &owner, 113).err()
        };
        assert_eq!(error, Some(Denial::Expired));
    }
}
#[test]
fn refreshed_deadline_is_checked_after_cleanup_and_replay_retention_uses_original_grant_expiry() {
    let f = Fixture::new(1);
    let p = page();
    let g = f.page_grant(p.clone(), 100);
    let proof = f
        .verifier
        .verify(
            &g.token,
            Workload::Post,
            PAGE_METHOD,
            &p.frame().unwrap(),
            100,
        )
        .unwrap();
    f.verifier.used.lock().unwrap().insert(Uuid::new_v4(), 99);
    let mut samples = 0;
    assert_eq!(
        f.verifier.consume(&proof, 150, || {
            samples += 1;
            if samples == 1 {
                100
            } else {
                150
            }
        }),
        Err(Denial::Expired)
    );
    assert!(f.verifier.used.lock().unwrap().is_empty());
    f.verifier.consume(&proof, 150, || 100).unwrap();
    assert_eq!(
        f.verifier.used.lock().unwrap().get(&proof.jti),
        Some(&proof.exp)
    );
    assert_eq!(
        f.verifier.consume(&proof, proof.exp, || 151),
        Err(Denial::Replayed)
    );
}

#[derive(Clone)]
struct Handler<const PAGE: bool> {
    calls: Arc<AtomicUsize>,
    admitted: Arc<Mutex<Option<AdmittedNotification>>>,
}
impl<const PAGE: bool> Handler<PAGE> {
    fn new() -> Self {
        Self {
            calls: Arc::new(AtomicUsize::new(0)),
            admitted: Arc::new(Mutex::new(None)),
        }
    }
}
impl<const PAGE: bool> NamedService for Handler<PAGE> {
    const NAME: &'static str = if PAGE {
        "authority.v1.NotificationProjectionService"
    } else {
        "notification.v1.NotificationService"
    };
}
impl<const PAGE: bool> tower::Service<http::Request<Body>> for Handler<PAGE> {
    type Response = http::Response<Body>;
    type Error = std::convert::Infallible;
    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, request: http::Request<Body>) -> Self::Future {
        self.calls.fetch_add(1, Ordering::SeqCst);
        *self.admitted.lock().unwrap() =
            request.extensions().get::<AdmittedNotification>().cloned();
        std::future::ready(Ok(http::Response::new(Body::empty())))
    }
}
struct Decision {
    result: Result<NotificationDecision, Denial>,
    expires_micros: i64,
    calls: AtomicUsize,
    delay: std::time::Duration,
}
impl NotificationAdmissionCheck for Decision {
    fn check<'a>(&'a self, _: &'a VerifiedNotification) -> NotificationAdmissionFuture<'a> {
        Box::pin(async move {
            self.calls.fetch_add(1, Ordering::SeqCst);
            if !self.delay.is_zero() {
                tokio::time::sleep(self.delay).await;
            }
            self.result.map(|decision| NotificationAdmission {
                decision,
                expires_micros: self.expires_micros,
            })
        })
    }
}
fn admission(result: Result<NotificationDecision, Denial>) -> Arc<Decision> {
    Arc::new(Decision {
        result,
        expires_micros: chrono::Utc::now().timestamp_micros() + 5_000_000,
        calls: AtomicUsize::new(0),
        delay: std::time::Duration::ZERO,
    })
}
fn request(
    method: &str,
    token: Option<&str>,
    body: Vec<u8>,
    caller: Option<Workload>,
) -> http::Request<Body> {
    let mut builder = http::Request::builder().uri(method);
    if let Some(t) = token {
        builder = builder.header(HEADER, t);
    }
    let mut req = builder
        .body(Body::new(Full::new(tonic::codegen::Bytes::from(body))))
        .unwrap();
    if let Some(c) = caller {
        req.extensions_mut().insert(VerifiedCaller(c));
    }
    req
}
#[tokio::test]
async fn original_write_guard_denies_missing_duplicate_forged_wrong_caller_and_oversize_before_handler(
) {
    let f = Fixture::new(8);
    let now = chrono::Utc::now().timestamp_micros();
    let mut d = direct();
    d.claim.lease_expires_micros = now + 5_000_000;
    let body = d.frame().unwrap();
    let g = f.grant(d, None, now);
    let handler = Handler::<false>::new();
    let calls = handler.calls.clone();
    let check = admission(Ok(NotificationDecision::Allowed { visibility: None }));
    let guard = NotificationGuard::new(handler, f.verifier.clone(), check.clone()).unwrap();
    let mut duplicate = request(
        WRITE_METHOD,
        Some(&g.token),
        body.clone(),
        Some(Workload::Post),
    );
    duplicate
        .headers_mut()
        .append(HEADER, http::HeaderValue::from_str(&g.token).unwrap());
    for req in [
        request(WRITE_METHOD, None, body.clone(), Some(Workload::Post)),
        request(WRITE_METHOD, Some(&g.token), body.clone(), None),
        request(
            WRITE_METHOD,
            Some(&g.token),
            body.clone(),
            Some(Workload::Gateway),
        ),
        request(
            WRITE_METHOD,
            Some("invalid"),
            body.clone(),
            Some(Workload::Post),
        ),
        request(
            WRITE_METHOD,
            Some(&g.token),
            vec![0; MAX_BYTES + 1],
            Some(Workload::Post),
        ),
        duplicate,
    ] {
        let r = guard.clone().oneshot(req).await.unwrap();
        assert_ne!(r.headers().get("grpc-status").unwrap(), "0");
    }
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(check.calls.load(Ordering::SeqCst), 0);
    let r = guard
        .oneshot(request(
            WRITE_METHOD,
            Some(&g.token),
            body,
            Some(Workload::Post),
        ))
        .await
        .unwrap();
    assert!(r.headers().get("grpc-status").is_none());
    assert_eq!(calls.load(Ordering::SeqCst), 1);
}
#[tokio::test]
async fn suppressed_write_returns_original_success_frame_and_trailers_without_calling_handler() {
    let f = Fixture::new(2);
    let now = chrono::Utc::now().timestamp_micros();
    let mut d = direct();
    d.claim.lease_expires_micros = now + 5_000_000;
    let body = d.frame().unwrap();
    let g = f.grant(d, None, now);
    let handler = Handler::<false>::new();
    let calls = handler.calls.clone();
    let guard = NotificationGuard::new(
        handler,
        f.verifier.clone(),
        admission(Ok(NotificationDecision::Suppressed)),
    )
    .unwrap();
    let r = guard
        .clone()
        .oneshot(request(
            WRITE_METHOD,
            Some(&g.token),
            body.clone(),
            Some(Workload::Post),
        ))
        .await
        .unwrap();
    assert_eq!(r.headers()[http::header::CONTENT_TYPE], "application/grpc");
    let collected = r.into_body().collect().await.unwrap();
    assert_eq!(collected.trailers().unwrap()["grpc-status"], "0");
    assert_eq!(collected.to_bytes().as_ref(), &[0, 0, 0, 0, 2, 16, 1]);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let replay = guard
        .oneshot(request(
            WRITE_METHOD,
            Some(&g.token),
            body,
            Some(Workload::Post),
        ))
        .await
        .unwrap();
    assert_eq!(replay.headers()["grpc-status"], "7");
}
#[tokio::test]
async fn admitted_page_alone_can_sign_exact_bounded_membership_and_suppressed_page_cannot() {
    let f = Fixture::new(2);
    let now = chrono::Utc::now().timestamp_micros();
    let mut p = page();
    p.claim.lease_expires_micros = now + 5_000_000;
    let g = f.page_grant(p.clone(), now);
    let handler = Handler::<true>::new();
    let admitted = handler.admitted.clone();
    let guard = NotificationGuard::new(
        handler,
        f.verifier.clone(),
        admission(Ok(NotificationDecision::Allowed {
            visibility: Some(SourceVisibility::Public),
        })),
    )
    .unwrap();
    guard
        .oneshot(request(
            PAGE_METHOD,
            Some(&g.token),
            p.frame().unwrap(),
            Some(Workload::Post),
        ))
        .await
        .unwrap();
    let seal = admitted.lock().unwrap().take().unwrap();
    let id = Uuid::new_v4();
    let token = f.friend.membership(&seal, vec![id], None, now).unwrap();
    f.issuer
        .membership
        .verify_page(&p, &[id], None, &token, now)
        .unwrap();
    assert!(f.friend.membership(&seal, vec![id, id], None, now).is_err());
    let g = f.page_grant(p.clone(), now);
    let handler = Handler::<true>::new();
    let admitted = handler.admitted.clone();
    let guard = NotificationGuard::new(
        handler,
        f.verifier,
        admission(Ok(NotificationDecision::Suppressed)),
    )
    .unwrap();
    guard
        .oneshot(request(
            PAGE_METHOD,
            Some(&g.token),
            p.frame().unwrap(),
            Some(Workload::Post),
        ))
        .await
        .unwrap();
    assert!(f
        .friend
        .membership(
            &admitted.lock().unwrap().take().unwrap(),
            vec![id],
            None,
            now
        )
        .is_err());
}
#[tokio::test]
async fn unavailable_admission_denies_without_consuming_grant_and_other_rpcs_keep_outer_policy() {
    let f = Fixture::new(2);
    let now = chrono::Utc::now().timestamp_micros();
    let mut d = direct();
    d.claim.lease_expires_micros = now + 5_000_000;
    let body = d.frame().unwrap();
    let g = f.grant(d, None, now);
    let handler = Handler::<false>::new();
    let calls = handler.calls.clone();
    let guard = NotificationGuard::new(
        handler.clone(),
        f.verifier.clone(),
        admission(Err(Denial::WrongBody)),
    )
    .unwrap();
    let r = guard
        .clone()
        .oneshot(request(
            WRITE_METHOD,
            Some(&g.token),
            body.clone(),
            Some(Workload::Post),
        ))
        .await
        .unwrap();
    assert_eq!(r.headers()["grpc-status"], "7");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(f.verifier.used.lock().unwrap().is_empty());
    guard
        .oneshot(request(
            "/notification.v1.NotificationService/ListNotifications",
            None,
            vec![],
            None,
        ))
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    let guard = NotificationGuard::new(
        handler,
        f.verifier,
        admission(Ok(NotificationDecision::Allowed { visibility: None })),
    )
    .unwrap();
    guard
        .oneshot(request(
            WRITE_METHOD,
            Some(&g.token),
            body,
            Some(Workload::Post),
        ))
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
#[tokio::test]
async fn expiry_during_fresh_admission_denies_before_write_and_does_not_consume() {
    let f = Fixture::new(2);
    let now = chrono::Utc::now().timestamp_micros();
    let mut d = direct();
    d.claim.lease_expires_micros = now + 100_000;
    let body = d.frame().unwrap();
    let g = f.grant(d, None, now);
    let handler = Handler::<false>::new();
    let calls = handler.calls.clone();
    let check = Arc::new(Decision {
        result: Ok(NotificationDecision::Allowed { visibility: None }),
        expires_micros: now + 5_000_000,
        calls: AtomicUsize::new(0),
        delay: std::time::Duration::from_millis(120),
    });
    let guard = NotificationGuard::new(handler, f.verifier.clone(), check.clone()).unwrap();
    let r = guard
        .oneshot(request(
            WRITE_METHOD,
            Some(&g.token),
            body,
            Some(Workload::Post),
        ))
        .await
        .unwrap();
    assert_eq!(r.headers()["grpc-status"], "7");
    assert_eq!(check.calls.load(Ordering::SeqCst), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(f.verifier.used.lock().unwrap().is_empty());
}

#[tokio::test]
async fn expired_refreshed_policy_denies_while_original_grant_is_live_and_seal_keeps_shorter_deadline(
) {
    let f = Fixture::new(4);
    let now = chrono::Utc::now().timestamp_micros();
    let mut d = direct();
    d.claim.lease_expires_micros = now + 5_000_000;
    let body = d.frame().unwrap();
    let g = f.grant(d, None, now);
    let handler = Handler::<false>::new();
    let calls = handler.calls.clone();
    let admitted = handler.admitted.clone();
    let check = Arc::new(Decision {
        result: Ok(NotificationDecision::Allowed { visibility: None }),
        expires_micros: now + 100_000,
        calls: AtomicUsize::new(0),
        delay: std::time::Duration::from_millis(120),
    });
    let guard = NotificationGuard::new(handler.clone(), f.verifier.clone(), check).unwrap();
    let r = guard
        .oneshot(request(
            WRITE_METHOD,
            Some(&g.token),
            body.clone(),
            Some(Workload::Post),
        ))
        .await
        .unwrap();
    assert_eq!(r.headers()["grpc-status"], "7");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(chrono::Utc::now().timestamp_micros() < g.expires_micros);
    assert!(f.verifier.used.lock().unwrap().is_empty());
    let fresh_exp = chrono::Utc::now().timestamp_micros() + 500_000;
    let check = Arc::new(Decision {
        result: Ok(NotificationDecision::Allowed { visibility: None }),
        expires_micros: fresh_exp,
        calls: AtomicUsize::new(0),
        delay: std::time::Duration::ZERO,
    });
    let guard = NotificationGuard::new(handler, f.verifier, check).unwrap();
    guard
        .oneshot(request(
            WRITE_METHOD,
            Some(&g.token),
            body,
            Some(Workload::Post),
        ))
        .await
        .unwrap();
    assert_eq!(
        admitted.lock().unwrap().as_ref().unwrap().expires_micros(),
        fresh_exp
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn refreshed_policy_expiring_under_replay_mutex_denies_while_original_grant_is_live() {
    struct Signal {
        sent: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
        exp: i64,
    }
    impl NotificationAdmissionCheck for Signal {
        fn check<'a>(&'a self, _: &'a VerifiedNotification) -> NotificationAdmissionFuture<'a> {
            Box::pin(async move {
                if let Some(tx) = self.sent.lock().unwrap().take() {
                    let _ = tx.send(());
                }
                Ok(NotificationAdmission {
                    decision: NotificationDecision::Allowed { visibility: None },
                    expires_micros: self.exp,
                })
            })
        }
    }
    let f = Fixture::new(4);
    let now = chrono::Utc::now().timestamp_micros();
    let mut d = direct();
    d.claim.lease_expires_micros = now + 5_000_000;
    let body = d.frame().unwrap();
    let g = f.grant(d, None, now);
    let fresh_exp = now + 150_000;
    let (locked, acquired) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let used = f.verifier.used.clone();
    let locker = std::thread::spawn(move || {
        let _guard = used.lock().unwrap();
        let _ = locked.send(());
        released
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
    });
    acquired.await.unwrap();
    let (signal, checked) = tokio::sync::oneshot::channel();
    let handler = Handler::<false>::new();
    let calls = handler.calls.clone();
    let guard = NotificationGuard::new(
        handler,
        f.verifier.clone(),
        Arc::new(Signal {
            sent: Mutex::new(Some(signal)),
            exp: fresh_exp,
        }),
    )
    .unwrap();
    let task = tokio::spawn(guard.oneshot(request(
        WRITE_METHOD,
        Some(&g.token),
        body,
        Some(Workload::Post),
    )));
    tokio::time::timeout(std::time::Duration::from_secs(2), checked)
        .await
        .unwrap()
        .unwrap();
    assert!(!task.is_finished());
    let remaining = fresh_exp
        .saturating_sub(chrono::Utc::now().timestamp_micros())
        .max(0) as u64;
    tokio::time::sleep(std::time::Duration::from_micros(remaining + 20_000)).await;
    release.send(()).unwrap();
    locker.join().unwrap();
    let r = tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(r.headers()["grpc-status"], "7");
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(f.verifier.used.lock().unwrap().is_empty());
    assert!(chrono::Utc::now().timestamp_micros() < g.expires_micros);
}

#[test]
fn short_original_deadline_caps_page_policy_final_owner_and_every_suppression_branch() {
    let f = Fixture::new(8);
    let cutoff = 500;
    let prepare = |b: NotificationBinding| {
        f.issuer
            .prepare_until(Workload::Post, b.clone(), b.frame().unwrap(), cutoff, 100)
            .unwrap()
    };
    let b = NotificationBinding::FollowerPage(page());
    let p = prepare(b.clone());
    let e = f
        .owner
        .attest(
            p.binding(),
            p.body(),
            p.challenge(),
            Some(SourceVisibility::Public),
            101,
            101,
        )
        .unwrap();
    assert_eq!(
        f.issuer.issue_page(p, &e, 101).unwrap().expires_micros,
        cutoff
    );
    let p = prepare(b);
    let e = f
        .owner
        .attest_suppressed(p.binding(), p.body(), p.challenge(), 101, 101)
        .unwrap();
    assert_eq!(
        f.issuer
            .source_suppression(&p, &e, 101)
            .unwrap()
            .unwrap()
            .expires_micros,
        cutoff
    );
    assert_eq!(
        f.issuer.source_suppression(&p, &e, cutoff).err(),
        Some(Denial::Expired)
    );
    for blocked in [false, true] {
        let d = f.delivery(page(), Uuid::new_v4(), 100);
        let p = prepare(NotificationBinding::Delivery(d));
        let e = f
            .owner
            .attest(
                p.binding(),
                p.body(),
                p.challenge(),
                Some(SourceVisibility::Public),
                101,
                101,
            )
            .unwrap();
        let ready = f.issuer.prepare_policy(p, &e, 101).unwrap();
        assert_eq!(ready.expires_micros(), cutoff);
        let policy = f.policy(&ready, blocked, false, true, 102);
        let final_policy = f.issuer.prepare_final_owner(ready, &policy, 103).unwrap();
        assert_eq!(final_policy.owner_request().expires_micros(), cutoff);
        let p = final_policy.owner_request();
        let e = f
            .owner
            .attest(
                p.binding(),
                p.body(),
                p.challenge(),
                Some(SourceVisibility::Public),
                104,
                104,
            )
            .unwrap();
        match f.issuer.issue_delivery(final_policy, &e, 104).unwrap() {
            NotificationIssue::Granted(g) => {
                assert!(!blocked);
                assert_eq!(g.expires_micros, cutoff);
            }
            NotificationIssue::Suppressed { expires_micros } => {
                assert!(blocked);
                assert_eq!(expires_micros, cutoff);
            }
        }
    }
    let p = prepare(NotificationBinding::Delivery(f.delivery(
        page(),
        Uuid::new_v4(),
        100,
    )));
    let e = f
        .owner
        .attest(
            p.binding(),
            p.body(),
            p.challenge(),
            Some(SourceVisibility::Public),
            101,
            101,
        )
        .unwrap();
    let ready = f.issuer.prepare_policy(p, &e, 101).unwrap();
    let policy = f.policy(&ready, false, false, true, 102);
    let final_policy = f.issuer.prepare_final_owner(ready, &policy, 103).unwrap();
    let p = final_policy.owner_request();
    let e = f
        .owner
        .attest_suppressed(p.binding(), p.body(), p.challenge(), 104, 104)
        .unwrap();
    assert!(matches!(
        f.issuer.issue_delivery(final_policy, &e, 104),
        Ok(NotificationIssue::Suppressed {
            expires_micros: 500
        })
    ));
}

#[test]
fn authenticated_admission_accepts_shorter_expiry_and_rejects_wrong_visibility_shape() {
    let page = NotificationBinding::FollowerPage(page());
    let direct = NotificationBinding::Delivery(direct());
    for (binding, visibility, valid) in [
        (&page, None, false),
        (&page, Some(SourceVisibility::Public), true),
        (&page, Some(SourceVisibility::Friends), true),
        (&direct, None, true),
        (&direct, Some(SourceVisibility::Public), false),
    ] {
        let a = NotificationAdmission {
            decision: NotificationDecision::Allowed { visibility },
            expires_micros: 400,
        };
        assert_eq!(a.validate_for(binding, 500, 100).is_ok(), valid);
        assert!(a.validate_for(binding, 300, 100).is_err());
        assert!(a.validate_for(binding, 500, 400).is_err());
    }
    let a = NotificationAdmission {
        decision: NotificationDecision::Suppressed,
        expires_micros: 400,
    };
    assert_eq!(a.validate_for(&page, 500, 100).unwrap().expires_micros, 400);
    assert!(a.validate_for(&direct, 500, 100).is_ok());
}

#[tokio::test]
async fn selected_membership_signing_preserves_pre_query_clock_and_admitted_deadline() {
    let f = Fixture::new(8);
    let now = chrono::Utc::now().timestamp_micros();
    let mut page = page();
    page.claim.lease_expires_micros = now + 30_000_000;
    let grant = f.page_grant(page.clone(), now);
    let handler = Handler::<true>::new();
    let captured = handler.admitted.clone();
    let guard = NotificationGuard::new(
        handler,
        f.verifier,
        Arc::new(Decision {
            result: Ok(NotificationDecision::Allowed {
                visibility: Some(SourceVisibility::Public),
            }),
            expires_micros: grant.expires_micros,
            calls: AtomicUsize::new(0),
            delay: std::time::Duration::ZERO,
        }),
    )
    .unwrap();
    guard
        .oneshot(request(
            PAGE_METHOD,
            Some(&grant.token),
            page.frame().unwrap(),
            Some(Workload::Post),
        ))
        .await
        .unwrap();
    let admitted = captured.lock().unwrap().clone().unwrap();
    let recipient = Uuid::new_v4();
    let observed = f
        .friend
        .membership_observed(&admitted, vec![recipient], None, now + 1, now + 800_000)
        .unwrap();
    assert_eq!(observed.expires_micros, grant.expires_micros);
    let verifier = NotificationMembershipVerifier::new(&[f.friend.public_key()]).unwrap();
    assert_eq!(
        verifier
            .verify_page_expiry(&page, &[recipient], None, &observed.token, now + 800_000)
            .unwrap(),
        grant.expires_micros
    );
    assert_eq!(
        f.friend
            .membership_observed(
                &admitted,
                vec![recipient],
                None,
                now + 1,
                grant.expires_micros
            )
            .err(),
        Some(Denial::Expired)
    );
}
