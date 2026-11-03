use super::*;
fn scope() -> FanoutScope {
    FanoutScope {
        job_id: Uuid::new_v4(),
        event_id: Uuid::new_v4(),
        post_id: Uuid::new_v4(),
        author_id: Uuid::new_v4(),
        lease_digest: FanoutScope::lease_digest(Uuid::new_v4()).unwrap(),
        post_created_micros: 10,
        lease_expires_micros: 10_000_000,
        attempt: 3,
        page_attempt: 2,
        max_page_attempts: 2,
        cursor: None,
        limit: 10,
    }
}
fn body() -> Vec<u8> {
    vec![0, 0, 0, 0, 2, 8, 1]
}
fn setup(capacity: usize) -> (FanoutOwnerSigner, FanoutIssuer, FanoutVerifier) {
    let policy = LifetimePolicy::new(1_000_000).unwrap();
    let owner = FanoutOwnerSigner::new(&[31; 32], policy).unwrap();
    let issuer = FanoutIssuer::new(&[32; 32], &[owner.public_key()], policy).unwrap();
    let verifier = FanoutVerifier::new(&[issuer.public_key()], capacity).unwrap();
    (owner, issuer, verifier)
}
fn issue(
    owner: &FanoutOwnerSigner,
    issuer: &FanoutIssuer,
    scope: FanoutScope,
    now: i64,
) -> IssuedFanout {
    let prepared = issuer
        .prepare(Workload::Post, scope.clone(), body(), now)
        .unwrap();
    let evidence = owner
        .attest(scope, &body(), prepared.challenge(), now, now)
        .unwrap();
    issuer.finish(prepared, &evidence, now).unwrap()
}
#[test]
fn exact_fresh_challenge_binds_owner_evidence_to_one_issuance() {
    let (owner, issuer, _) = setup(2);
    let scope = scope();
    let first = issuer
        .prepare(Workload::Post, scope.clone(), body(), 100)
        .unwrap();
    let evidence = owner
        .attest(scope.clone(), &body(), first.challenge(), 100, 100)
        .unwrap();
    let second = issuer.prepare(Workload::Post, scope, body(), 100).unwrap();
    assert_ne!(first.challenge(), second.challenge());
    assert_eq!(
        issuer.finish(second, &evidence, 100).err(),
        Some(Denial::WrongBody)
    );
    assert!(issuer.finish(first, &evidence, 100).is_ok());
}
#[test]
fn every_scope_field_and_exact_nullable_cursor_must_match_the_owner() {
    let (owner, issuer, _) = setup(2);
    let original = scope();
    for case in 0..13 {
        let prepared = issuer
            .prepare(Workload::Post, original.clone(), body(), 100)
            .unwrap();
        let mut changed = original.clone();
        match case {
            0 => changed.job_id = Uuid::new_v4(),
            1 => changed.event_id = Uuid::new_v4(),
            2 => changed.post_id = Uuid::new_v4(),
            3 => changed.author_id = Uuid::new_v4(),
            4 => changed.lease_digest = FanoutScope::lease_digest(Uuid::new_v4()).unwrap(),
            5 => changed.post_created_micros += 1,
            6 => changed.lease_expires_micros += 1,
            7 => changed.attempt += 1,
            8 => changed.page_attempt = 1,
            9 => changed.max_page_attempts = 3,
            10 => changed.cursor = Some(format!("2026-10-14T00:00:00+00:00|{}", Uuid::new_v4())),
            11 => changed.limit += 1,
            _ => {
                let evidence = owner
                    .attest(changed, &[0, 0, 0, 0, 0], prepared.challenge(), 100, 100)
                    .unwrap();
                assert_eq!(
                    issuer.finish(prepared, &evidence, 100).err(),
                    Some(Denial::WrongBody)
                );
                continue;
            }
        }
        let evidence = owner
            .attest(changed, &body(), prepared.challenge(), 100, 100)
            .unwrap();
        assert_eq!(
            issuer.finish(prepared, &evidence, 100).err(),
            Some(Denial::WrongBody)
        );
    }
    let mut original = original;
    let id = Uuid::new_v4();
    original.cursor = Some(format!("2026-10-14T00:00:00+00:00|{id}"));
    let prepared = issuer
        .prepare(Workload::Post, original.clone(), body(), 100)
        .unwrap();
    original.cursor = Some(format!("2026-10-14T00:00:00Z|{id}"));
    let evidence = owner
        .attest(original, &body(), prepared.challenge(), 100, 100)
        .unwrap();
    assert_eq!(
        issuer.finish(prepared, &evidence, 100).err(),
        Some(Denial::WrongBody)
    );
}
#[test]
fn expiry_is_capped_by_lease_and_rechecked_after_owner_wait_without_skew() {
    let (owner, issuer, verifier) = setup(2);
    let mut scope = scope();
    scope.lease_expires_micros = 150;
    let issued = issue(&owner, &issuer, scope.clone(), 100);
    assert_eq!(issued.expires_micros, 150);
    assert!(verifier
        .verify(&issued.token, Workload::Post, METHOD, &body(), 149)
        .is_ok());
    assert_eq!(
        verifier
            .verify(&issued.token, Workload::Post, METHOD, &body(), 150)
            .err(),
        Some(Denial::Expired)
    );
    let prepared = issuer
        .prepare(Workload::Post, scope.clone(), body(), 100)
        .unwrap();
    let evidence = owner
        .attest(scope, &body(), prepared.challenge(), 100, 100)
        .unwrap();
    assert_eq!(
        issuer.finish(prepared, &evidence, 150).err(),
        Some(Denial::Expired)
    );
}
#[test]
fn presenter_method_body_signature_and_system_purpose_are_mandatory() {
    let (owner, issuer, verifier) = setup(2);
    let scope = scope();
    let issued = issue(&owner, &issuer, scope.clone(), 100);
    for caller in Workload::ALL {
        assert_eq!(
            verifier
                .verify(&issued.token, caller, METHOD, &body(), 100)
                .is_ok(),
            caller == Workload::Post
        );
        assert_eq!(
            issuer.prepare(caller, scope.clone(), body(), 100).is_ok(),
            caller == Workload::Post
        );
    }
    assert_eq!(
        verifier
            .verify(
                &issued.token,
                Workload::Post,
                "/friend.v1.FriendService/ListFollowers",
                &body(),
                100
            )
            .err(),
        Some(Denial::WrongMethod)
    );
    assert_eq!(
        verifier
            .verify(&issued.token, Workload::Post, METHOD, &[0, 0, 0, 0, 0], 100)
            .err(),
        Some(Denial::WrongBody)
    );
    assert_eq!(
        verifier
            .verify("", Workload::Post, METHOD, &body(), 100)
            .err(),
        Some(Denial::Malformed)
    );
    let mut grant: Grant = issuer_verification(&issuer)
        .verify(GRANT_CONTEXT, &issued.token)
        .unwrap();
    for case in 0..5 {
        let original = (
            grant.iss.clone(),
            grant.pres.clone(),
            grant.aud.clone(),
            grant.purpose.clone(),
            grant.method.clone(),
        );
        match case {
            0 => grant.iss = "post".into(),
            1 => grant.pres = "gateway".into(),
            2 => grant.aud = "profile".into(),
            3 => grant.purpose = "notification_recipients".into(),
            _ => grant.method = "/other/Method".into(),
        }
        let signed = issuer.key.sign(GRANT_CONTEXT, &grant);
        assert!(verifier
            .verify(&signed, Workload::Post, METHOD, &body(), 100)
            .is_err());
        (
            grant.iss,
            grant.pres,
            grant.aud,
            grant.purpose,
            grant.method,
        ) = original;
    }
    let wrong_context = issuer.key.sign(OWNER_CONTEXT, &grant);
    assert_eq!(
        verifier
            .verify(&wrong_context, Workload::Post, METHOD, &body(), 100)
            .err(),
        Some(Denial::BadSignature)
    );
    let actor = ActorDelegationSigner::from_seed(Workload::Gateway, &[32; 32])
        .unwrap()
        .issue(Workload::Friend, METHOD, Uuid::new_v4(), None, &body(), 100);
    assert!(verifier
        .verify(&actor, Workload::Post, METHOD, &body(), 100)
        .is_err());
    let forger = SigningKey::new(&[33; 32]).unwrap();
    grant.kid = forger.id.clone();
    assert_eq!(
        verifier
            .verify(
                &forger.sign(GRANT_CONTEXT, &grant),
                Workload::Post,
                METHOD,
                &body(),
                100
            )
            .err(),
        Some(Denial::UnknownKey)
    );
}
fn issuer_verification(issuer: &FanoutIssuer) -> VerificationKeys {
    VerificationKeys::new(&[issuer.public_key()]).unwrap()
}
#[test]
fn live_replay_entries_are_never_evicted_and_expired_entries_can_be_reclaimed() {
    let (owner, issuer, verifier) = setup(1);
    let mut scope = scope();
    scope.lease_expires_micros = 150;
    let first = issue(&owner, &issuer, scope.clone(), 100);
    let proof = verifier
        .verify(&first.token, Workload::Post, METHOD, &body(), 100)
        .unwrap();
    verifier.consume(&proof, || 100).unwrap();
    let second = issue(&owner, &issuer, scope, 100);
    let another = verifier
        .verify(&second.token, Workload::Post, METHOD, &body(), 100)
        .unwrap();
    assert_eq!(
        verifier.consume(&another, || 100),
        Err(Denial::ReplayCacheFull)
    );
    assert_eq!(verifier.consume(&proof, || 100), Err(Denial::Replayed));
    assert_eq!(verifier.consume(&proof, || 150), Err(Denial::Expired));
    let newer = issue(&owner, &issuer, self::scope(), 151);
    let proof = verifier
        .verify(&newer.token, Workload::Post, METHOD, &body(), 151)
        .unwrap();
    verifier.consume(&proof, || 151).unwrap();
}
#[test]
fn concurrent_replay_consumption_has_exactly_one_winner_and_reissuance_keeps_budget() {
    let (owner, issuer, verifier) = setup(32);
    let scope = scope();
    let first = issue(&owner, &issuer, scope.clone(), 100);
    let second = issue(&owner, &issuer, scope.clone(), 100);
    let proof = verifier
        .verify(&first.token, Workload::Post, METHOD, &body(), 100)
        .unwrap();
    let other = verifier
        .verify(&second.token, Workload::Post, METHOD, &body(), 100)
        .unwrap();
    assert_ne!(proof.jti, other.jti);
    assert!(proof.scope == other.scope);
    assert_eq!(other.scope.page_attempt, scope.page_attempt);
    let barrier = Arc::new(std::sync::Barrier::new(16));
    let threads: Vec<_> = (0..16)
        .map(|_| {
            let verifier = verifier.clone();
            let proof = proof.clone();
            let barrier = barrier.clone();
            std::thread::spawn(move || {
                barrier.wait();
                verifier.consume(&proof, || 100)
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
    verifier.consume(&other, || 100).unwrap();
}
#[test]
fn invalid_policy_shape_compression_and_untrusted_owner_keys_fail_closed() {
    for lifetime in [0, -1, MAX_LIFETIME_MICROS + 1] {
        assert!(LifetimePolicy::new(lifetime).is_err());
    }
    assert!(FanoutVerifier::new(&[], 1).is_err());
    assert!(FanoutVerifier::new(&[[0; 32]], 0).is_err());
    let (owner, issuer, _) = setup(2);
    let mut invalid = scope();
    invalid.page_attempt = 3;
    assert!(issuer
        .prepare(Workload::Post, invalid, body(), 100)
        .is_err());
    for body in [vec![], vec![1, 0, 0, 0, 0], vec![0, 0, 0, 0, 1]] {
        assert!(issuer.prepare(Workload::Post, scope(), body, 100).is_err());
    }
    let prepared = issuer
        .prepare(Workload::Post, scope(), body(), 100)
        .unwrap();
    let forged = FanoutOwnerSigner::new(&[34; 32], owner.policy)
        .unwrap()
        .attest(
            prepared.scope.clone(),
            &body(),
            prepared.challenge(),
            100,
            100,
        )
        .unwrap();
    assert_eq!(
        issuer.finish(prepared, &forged, 100).err(),
        Some(Denial::UnknownKey)
    );
}

#[test]
fn replay_clock_is_sampled_under_lock_and_again_after_cleanup() {
    let (owner, issuer, verifier) = setup(1);
    let issued = issue(&owner, &issuer, scope(), 100);
    let proof = verifier
        .verify(&issued.token, Workload::Post, METHOD, &body(), 100)
        .unwrap();
    // Force the saturated-cache retain branch, rather than checking time
    // again after an empty-cache no-op.
    verifier.used.lock().unwrap().insert(Uuid::new_v4(), 99);
    let mut samples = 0;
    assert_eq!(
        verifier.consume(&proof, || {
            // Acquiring the replay mutex before reading time is part of admission.
            assert!(verifier.used.try_lock().is_err());
            samples += 1;
            if samples == 1 {
                100
            } else {
                proof.exp
            }
        }),
        Err(Denial::Expired)
    );
    assert_eq!(samples, 2);
    assert!(verifier.used.lock().unwrap().is_empty());
}

#[derive(Clone)]
struct CountingProjection(Arc<std::sync::atomic::AtomicUsize>);
impl NamedService for CountingProjection {
    const NAME: &'static str = "authority.v1.FanoutProjectionService";
}
impl tower::Service<http::Request<Body>> for CountingProjection {
    type Response = http::Response<Body>;
    type Error = std::convert::Infallible;
    type Future = std::future::Ready<Result<Self::Response, Self::Error>>;
    fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
        Poll::Ready(Ok(()))
    }
    fn call(&mut self, _: http::Request<Body>) -> Self::Future {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        std::future::ready(Ok(http::Response::new(Body::empty())))
    }
}
struct FreshAdmission(Mutex<Option<tokio::sync::oneshot::Sender<()>>>);
impl FanoutAdmissionCheck for FreshAdmission {
    fn check<'a>(&'a self, proof: &'a VerifiedFanout) -> AdmissionFuture<'a> {
        Box::pin(async move {
            if let Some(sender) = self.0.lock().unwrap().take() {
                let _ = sender.send(());
            }
            Ok(proof.expires_micros())
        })
    }
}
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn expiration_during_replay_mutex_contention_denies_before_projection_dispatch() {
    use tower::ServiceExt;
    let (owner, issuer, verifier) = setup(2);
    let now = chrono::Utc::now().timestamp_micros();
    let mut scope = scope();
    scope.lease_expires_micros = now + 300_000;
    let issued = issue(&owner, &issuer, scope, now);
    let (locked, acquired) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let cache = verifier.used.clone();
    let locker = std::thread::spawn(move || {
        let _held = cache.lock().unwrap();
        let _ = locked.send(());
        released
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
    });
    acquired.await.unwrap();
    let (fresh, checked) = tokio::sync::oneshot::channel();
    let entries = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let guard = FanoutGuard::new(
        CountingProjection(entries.clone()),
        verifier.clone(),
        Arc::new(FreshAdmission(Mutex::new(Some(fresh)))),
    )
    .unwrap();
    let mut request = http::Request::builder()
        .uri(METHOD)
        .header(HEADER, &issued.token)
        .body(Body::new(Full::new(tonic::codegen::Bytes::from(body()))))
        .unwrap();
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Post));
    let task = tokio::spawn(guard.oneshot(request));
    tokio::time::timeout(std::time::Duration::from_secs(2), checked)
        .await
        .unwrap()
        .unwrap();
    assert!(!task.is_finished());
    let remaining = issued
        .expires_micros
        .saturating_sub(chrono::Utc::now().timestamp_micros())
        .max(0) as u64;
    tokio::time::sleep(std::time::Duration::from_micros(remaining + 20_000)).await;
    release.send(()).unwrap();
    locker.join().unwrap();
    let response = tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert_eq!(response.headers().get("grpc-status").unwrap(), "7");
    assert_eq!(entries.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(verifier.used.lock().unwrap().is_empty());
}

struct ShortAdmission {
    expiry: i64,
    signal: Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
}
impl FanoutAdmissionCheck for ShortAdmission {
    fn check<'a>(&'a self, _proof: &'a VerifiedFanout) -> AdmissionFuture<'a> {
        Box::pin(async move {
            if let Some(signal) = self.signal.lock().unwrap().take() {
                let _ = signal.send(());
            }
            Ok(self.expiry)
        })
    }
}
#[test]
fn refreshed_owner_lifetime_and_original_operation_cap_are_both_retained() {
    let (owner, issuer, verifier) = setup(4);
    let now = 100;
    let grant = issue(&owner, &issuer, scope(), now);
    let proof = verifier
        .verify(&grant.token, Workload::Post, METHOD, &body(), now)
        .unwrap();
    let short = FanoutOwnerSigner::new(&[31; 32], LifetimePolicy::new(40).unwrap()).unwrap();
    let prepared = issuer
        .prepare_until(
            Workload::Post,
            proof.scope().clone(),
            body(),
            now,
            proof.expires_micros(),
        )
        .unwrap();
    let evidence = short
        .attest(
            proof.scope().clone(),
            &body(),
            prepared.challenge(),
            now,
            now,
        )
        .unwrap();
    assert_eq!(
        issuer.check_fresh_owner(prepared, &evidence, now).unwrap(),
        140
    );
    let prepared = issuer
        .prepare_until(Workload::Post, proof.scope().clone(), body(), now, 120)
        .unwrap();
    let evidence = short
        .attest(
            proof.scope().clone(),
            &body(),
            prepared.challenge(),
            now,
            now,
        )
        .unwrap();
    assert_eq!(
        issuer.check_fresh_owner(prepared, &evidence, now).unwrap(),
        120
    );
    assert!(issuer
        .prepare_until(Workload::Post, proof.scope().clone(), body(), now, 0)
        .is_err());
}
#[tokio::test]
async fn admission_seal_carries_shorter_expiry_but_nonce_retains_original_expiry() {
    use tower::ServiceExt;
    #[derive(Clone)]
    struct Seal(std::sync::Arc<std::sync::atomic::AtomicI64>);
    impl NamedService for Seal {
        const NAME: &'static str = "authority.v1.FanoutProjectionService";
    }
    impl tower::Service<http::Request<Body>> for Seal {
        type Response = http::Response<Body>;
        type Error = std::convert::Infallible;
        type Future = std::future::Ready<Result<Self::Response, Self::Error>>;
        fn poll_ready(&mut self, _: &mut Context<'_>) -> Poll<Result<(), Self::Error>> {
            Poll::Ready(Ok(()))
        }
        fn call(&mut self, request: http::Request<Body>) -> Self::Future {
            let admitted = request.extensions().get::<AdmittedFanout>().unwrap();
            self.0.store(
                admitted.expires_micros(),
                std::sync::atomic::Ordering::SeqCst,
            );
            assert_eq!(
                admitted.deadline().unwrap().expires_micros(),
                admitted.expires_micros()
            );
            std::future::ready(Ok(http::Response::new(Body::empty())))
        }
    }
    let (owner, issuer, verifier) = setup(4);
    let now = chrono::Utc::now().timestamp_micros();
    let mut scope = scope();
    scope.lease_expires_micros = now + 900_000;
    let issued = issue(&owner, &issuer, scope, now);
    let expiry = now + 250_000;
    let seal = std::sync::Arc::new(std::sync::atomic::AtomicI64::new(0));
    let guard = FanoutGuard::new(
        Seal(seal.clone()),
        verifier.clone(),
        Arc::new(ShortAdmission {
            expiry,
            signal: Mutex::new(None),
        }),
    )
    .unwrap();
    let mut request = http::Request::builder()
        .uri(METHOD)
        .header(HEADER, &issued.token)
        .body(Body::new(Full::new(tonic::codegen::Bytes::from(body()))))
        .unwrap();
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Post));
    guard.oneshot(request).await.unwrap();
    assert_eq!(seal.load(std::sync::atomic::Ordering::SeqCst), expiry);
    let proof = verifier
        .verify(&issued.token, Workload::Post, METHOD, &body(), expiry + 1)
        .unwrap();
    assert_eq!(
        *verifier.used.lock().unwrap().get(&proof.jti).unwrap(),
        issued.expires_micros
    );
    assert_eq!(
        verifier.consume(&proof, || expiry + 1),
        Err(Denial::Replayed)
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn shorter_refresh_expires_while_original_grant_is_live_under_nonce_contention() {
    use tower::ServiceExt;
    let (owner, issuer, verifier) = setup(2);
    let now = chrono::Utc::now().timestamp_micros();
    let mut scope = scope();
    scope.lease_expires_micros = now + 900_000;
    let issued = issue(&owner, &issuer, scope, now);
    let (locked, acquired) = tokio::sync::oneshot::channel();
    let (release, released) = std::sync::mpsc::channel();
    let cache = verifier.used.clone();
    let locker = std::thread::spawn(move || {
        let _held = cache.lock().unwrap();
        let _ = locked.send(());
        released
            .recv_timeout(std::time::Duration::from_secs(5))
            .unwrap();
    });
    acquired.await.unwrap();
    let (fresh, checked) = tokio::sync::oneshot::channel();
    let entries = Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let guard = FanoutGuard::new(
        CountingProjection(entries.clone()),
        verifier.clone(),
        Arc::new(ShortAdmission {
            expiry: now + 150_000,
            signal: Mutex::new(Some(fresh)),
        }),
    )
    .unwrap();
    let mut request = http::Request::builder()
        .uri(METHOD)
        .header(HEADER, &issued.token)
        .body(Body::new(Full::new(tonic::codegen::Bytes::from(body()))))
        .unwrap();
    request
        .extensions_mut()
        .insert(VerifiedCaller(Workload::Post));
    let task = tokio::spawn(guard.oneshot(request));
    tokio::time::timeout(std::time::Duration::from_secs(2), checked)
        .await
        .unwrap()
        .unwrap();
    assert!(!task.is_finished());
    let remaining = (now + 150_000)
        .saturating_sub(chrono::Utc::now().timestamp_micros())
        .max(0) as u64;
    tokio::time::sleep(std::time::Duration::from_micros(remaining + 20_000)).await;
    release.send(()).unwrap();
    locker.join().unwrap();
    let response = tokio::time::timeout(std::time::Duration::from_secs(2), task)
        .await
        .unwrap()
        .unwrap()
        .unwrap();
    assert!(
        chrono::Utc::now().timestamp_micros() < issued.expires_micros,
        "original grant must still be live"
    );
    assert_eq!(response.headers().get("grpc-status").unwrap(), "7");
    assert_eq!(entries.load(std::sync::atomic::Ordering::SeqCst), 0);
    assert!(verifier.used.lock().unwrap().is_empty());
}
