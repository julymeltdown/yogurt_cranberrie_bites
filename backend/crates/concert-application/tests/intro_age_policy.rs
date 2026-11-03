use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use concert_application::{age_policy::*, intro_policy::*, Clock};
use concert_domain::{
    age_policy::{AgeBounds, AgeStatus, OwnerClaim, StoredStatus},
    intro_policy::{IntroRecord, IntroState},
    AuthUserId, EventId, Revision,
};
use std::collections::HashSet;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc, Mutex,
};
use uuid::Uuid;
fn time(s: &str) -> DateTime<Utc> {
    s.parse().unwrap()
}
fn start() -> DateTime<Utc> {
    time("2026-01-31T14:59:59Z")
}
fn user(id: u128) -> AuthUserId {
    AuthUserId::from_uuid(Uuid::from_u128(id))
}
#[derive(Clone)]
struct TestClock(Arc<Mutex<DateTime<Utc>>>);
impl TestClock {
    fn new() -> Self {
        Self(Arc::new(Mutex::new(start())))
    }
    fn advance(&self, d: Duration) {
        *self.0.lock().unwrap() += d;
    }
}
impl Clock for TestClock {
    fn now(&self) -> DateTime<Utc> {
        *self.0.lock().unwrap()
    }
}
fn context() -> Context {
    Context {
        actor: user(1),
        event: EventId::from_uuid(Uuid::from_u128(2)),
        session: Uuid::from_u128(3),
        request: Uuid::from_u128(4),
        expires_at: start() + Duration::hours(1),
    }
}
fn intro(id: u128, owner: u128) -> IntroRecord {
    IntroRecord {
        intro_id: Uuid::from_u128(id),
        event_id: context().event,
        owner_id: user(owner),
        text: "Hello from the lobby".into(),
        age_bounds: AgeBounds::new(Some(25), Some(26)).unwrap(),
        expires_at: None,
        created_at: start(),
        state: IntroState::Published,
        revision: Revision::initial(),
    }
}
fn claim() -> OwnerClaim {
    OwnerClaim {
        claim_id: Uuid::from_u128(7),
        user_id: context().actor,
        verifier_id: "fixture".into(),
        source_revision: "source-v1".into(),
        status: StoredStatus::Verified,
        verified_at: time("2026-01-01T00:00:00Z"),
        expires_at: time("2026-04-01T00:00:00Z"),
        age_at_verification: Some(25),
        next_birthday_at: Some("2026-02-01".parse().unwrap()),
    }
}
#[derive(Default)]
struct State {
    invalid_session: bool,
    member_missing: bool,
    blocked_forward: HashSet<AuthUserId>,
    blocked_reverse: HashSet<AuthUserId>,
    restricted: HashSet<AuthUserId>,
    revision_changed: bool,
    revoked: bool,
}
type Shared = Arc<Mutex<State>>;
fn check_authority(s: &State, q: ReadQuery, now: DateTime<Utc>) -> Result<(), PolicyError> {
    q.context.check(now)?;
    if s.invalid_session || s.member_missing || s.restricted.contains(&q.context.actor) {
        return Err(PolicyError::Denied);
    }
    if let ReadTarget::Intro { subject, .. } = q.target {
        if s.revision_changed
            || s.blocked_forward.contains(&subject)
            || s.blocked_reverse.contains(&subject)
            || s.restricted.contains(&subject)
        {
            return Err(PolicyError::Denied);
        }
    }
    Ok(())
}
struct Guard {
    query: ReadQuery,
    state: Shared,
}
impl ReadLease for Guard {
    fn check(&self, q: ReadQuery, now: DateTime<Utc>) -> Result<(), PolicyError> {
        if q != self.query {
            return Err(PolicyError::Unavailable);
        }
        check_authority(&self.state.lock().unwrap(), q, now)
    }
}
#[derive(Clone, Copy)]
enum Change {
    Birthday,
    BlockFirst,
    RevokeClaim,
}
struct Authority {
    state: Shared,
    clock: TestClock,
    reads: AtomicUsize,
    change: Option<(usize, Change)>,
}
impl Authority {
    fn new(state: Shared, clock: TestClock) -> Self {
        Self {
            state,
            clock,
            reads: AtomicUsize::new(0),
            change: None,
        }
    }
}
#[async_trait]
impl IntroAuthority for Authority {
    async fn read(&self, q: ReadQuery) -> Result<Box<dyn ReadLease>, PolicyError> {
        let n = self.reads.fetch_add(1, Ordering::SeqCst) + 1;
        if let Some((at, change)) = self.change {
            if at == n {
                match change {
                    Change::Birthday => self.clock.advance(Duration::seconds(1)),
                    Change::BlockFirst => {
                        self.state.lock().unwrap().blocked_reverse.insert(user(10));
                    }
                    Change::RevokeClaim => self.state.lock().unwrap().revoked = true,
                }
            }
        }
        check_authority(&self.state.lock().unwrap(), q, self.clock.now())?;
        Ok(Box::new(Guard {
            query: q,
            state: self.state.clone(),
        }))
    }
}
struct ClaimGuard {
    query: ClaimQuery,
    state: Shared,
}
impl ClaimLease for ClaimGuard {
    fn check(&self, q: ClaimQuery, now: DateTime<Utc>) -> Result<(), PolicyError> {
        q.context.check(now)?;
        if q != self.query {
            return Err(PolicyError::Unavailable);
        }
        let state = self.state.lock().unwrap();
        if state.revoked || state.invalid_session || state.member_missing {
            return Err(PolicyError::Denied);
        }
        Ok(())
    }
}
#[derive(Clone, Copy)]
enum Fault {
    None,
    Session,
    Subject,
    Event,
    Stale,
    Future,
    Deadline,
    ExpiredObservation,
}
struct Claims {
    state: Shared,
    clock: TestClock,
    claims: Vec<OwnerClaim>,
    available: bool,
    fault: Fault,
    calls: Arc<AtomicUsize>,
}
impl Claims {
    fn new(state: Shared, clock: TestClock) -> Self {
        Self {
            state,
            clock,
            claims: vec![claim()],
            available: true,
            fault: Fault::None,
            calls: Arc::new(AtomicUsize::new(0)),
        }
    }
}
#[async_trait]
impl ClaimReader for Claims {
    async fn read(&self, mut q: ClaimQuery) -> Result<ClaimSnapshot, PolicyError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        let now = self.clock.now();
        let mut checked = now;
        let mut until = q.context.expires_at;
        match self.fault {
            Fault::None => (),
            Fault::Session => q.context.session = Uuid::from_u128(999),
            Fault::Subject => q.subject = user(999),
            Fault::Event => q.context.event = EventId::from_uuid(Uuid::from_u128(999)),
            Fault::Stale => checked -= Duration::seconds(1),
            Fault::Future => checked += Duration::seconds(1),
            Fault::Deadline => until += Duration::seconds(1),
            Fault::ExpiredObservation => until = now,
        }
        Ok(ClaimSnapshot {
            lease: Box::new(ClaimGuard {
                query: q,
                state: self.state.clone(),
            }),
            query: q,
            checked_at: checked,
            valid_until: until,
            claims: self.claims.clone(),
            verification_available: self.available,
        })
    }
}
fn fixture() -> (Shared, TestClock) {
    (Arc::new(Mutex::new(State::default())), TestClock::new())
}
#[tokio::test]
async fn unavailable_ports_fail_closed_including_empty_list_and_own_status() {
    let policy = IntroPolicy::new(
        UnavailableClaims,
        UnavailableIntroAuthority,
        TestClock::new(),
    );
    assert!(matches!(
        policy.direct(context(), &intro(8, 10)).await,
        Err(PolicyError::Unavailable)
    ));
    assert!(matches!(
        policy.list(context(), &[]).await,
        Err(PolicyError::Unavailable)
    ));
    assert!(matches!(
        policy.own(context(), &intro(8, 1)).await,
        Err(PolicyError::Unavailable)
    ));
    assert!(matches!(
        own_status(&UnavailableClaims, &TestClock::new(), context()).await,
        Err(PolicyError::Unavailable)
    ));
}
#[tokio::test]
async fn valid_direct_and_list_agree_on_half_open_bounds() {
    let (s, c) = fixture();
    let policy = IntroPolicy::new(
        Claims::new(s.clone(), c.clone()),
        Authority::new(s, c.clone()),
        c.clone(),
    );
    let i = intro(8, 10);
    assert_eq!(policy.direct(context(), &i).await.unwrap().id(), i.intro_id);
    assert_eq!(policy.list(context(), &[i.clone()]).await.unwrap().len(), 1);
    c.advance(Duration::seconds(1));
    assert!(matches!(
        policy.direct(context(), &i).await,
        Err(PolicyError::Denied)
    ));
    assert!(policy.list(context(), &[i]).await.unwrap().is_empty());
}
#[tokio::test]
async fn both_block_directions_restrictions_membership_and_revision_deny() {
    let (s, c) = fixture();
    let policy = IntroPolicy::new(
        Claims::new(s.clone(), c.clone()),
        Authority::new(s.clone(), c.clone()),
        c,
    );
    let i = intro(8, 10);
    for case in 0..7 {
        *s.lock().unwrap() = State::default();
        {
            let mut st = s.lock().unwrap();
            match case {
                0 => {
                    st.blocked_forward.insert(user(10));
                }
                1 => {
                    st.blocked_reverse.insert(user(10));
                }
                2 => {
                    st.restricted.insert(user(10));
                }
                3 => {
                    st.restricted.insert(user(1));
                }
                4 => st.member_missing = true,
                5 => st.invalid_session = true,
                _ => st.revision_changed = true,
            }
        }
        assert!(matches!(
            policy.direct(context(), &i).await,
            Err(PolicyError::Denied)
        ));
        match policy.list(context(), &[i.clone()]).await {
            Ok(v) => assert!(v.is_empty()),
            Err(e) => assert_eq!(e, PolicyError::Denied),
        }
    }
}
#[tokio::test]
async fn wrong_event_and_owner_cannot_project_private_intro() {
    let (s, c) = fixture();
    let policy = IntroPolicy::new(UnavailableClaims, Authority::new(s, c.clone()), c);
    assert!(matches!(
        policy.own(context(), &intro(8, 10)).await,
        Err(PolicyError::Denied)
    ));
    let mut other = context();
    other.event = EventId::from_uuid(Uuid::from_u128(999));
    assert!(matches!(
        policy.direct(other, &intro(8, 10)).await,
        Err(PolicyError::Denied)
    ));
    assert!(policy.own(context(), &intro(8, 1)).await.is_ok());
}
#[tokio::test]
async fn snapshot_binding_staleness_future_and_deadline_are_rejected() {
    for fault in [
        Fault::Session,
        Fault::Subject,
        Fault::Event,
        Fault::Stale,
        Fault::Future,
        Fault::Deadline,
        Fault::ExpiredObservation,
    ] {
        let (s, c) = fixture();
        let mut reader = Claims::new(s, c.clone());
        reader.fault = fault;
        assert!(matches!(
            own_status(&reader, &c, context()).await,
            Err(PolicyError::Unavailable)
        ));
    }
}
#[tokio::test]
async fn unknown_unbounded_discovery_does_not_call_age_provider() {
    let (s, c) = fixture();
    let mut reader = Claims::new(s.clone(), c.clone());
    reader.claims.clear();
    let calls = reader.calls.clone();
    let policy = IntroPolicy::new(reader, Authority::new(s, c.clone()), c);
    let mut i = intro(8, 10);
    i.age_bounds = AgeBounds::new(None, None).unwrap();
    assert!(policy.direct(context(), &i).await.is_ok());
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    i.age_bounds = AgeBounds::new(Some(14), Some(101)).unwrap();
    assert!(matches!(
        policy.direct(context(), &i).await,
        Err(PolicyError::Denied)
    ));
}
#[tokio::test]
async fn own_status_exposes_only_status_timestamps_and_provider_availability() {
    let (s, c) = fixture();
    let mut reader = Claims::new(s, c.clone());
    reader.claims.clear();
    reader.available = false;
    let status = own_status(&reader, &c, context()).await.unwrap();
    assert_eq!(status.status(), AgeStatus::ProviderUnavailable);
    assert_eq!(status.user_id(), context().actor.uuid());
    assert_eq!(status.event_id(), context().event.uuid());
    assert_eq!(status.timestamps(), (None, None));
    assert!(!status.verification_available());
    reader.claims = vec![claim()];
    assert_eq!(
        own_status(&reader, &c, context()).await.unwrap().status(),
        AgeStatus::Verified
    );
    reader.claims[0].expires_at = start();
    assert_eq!(
        own_status(&reader, &c, context()).await.unwrap().status(),
        AgeStatus::Expired
    );
    reader.claims[0].status = StoredStatus::Revoked;
    assert_eq!(
        own_status(&reader, &c, context()).await.unwrap().status(),
        AgeStatus::Revoked
    );
}
#[tokio::test]
async fn earlier_list_row_is_removed_when_blocked_during_later_read() {
    let (s, c) = fixture();
    let mut authority = Authority::new(s.clone(), c.clone());
    authority.change = Some((3, Change::BlockFirst));
    let policy = IntroPolicy::new(Claims::new(s, c.clone()), authority, c);
    let views = policy
        .list(context(), &[intro(8, 10), intro(9, 11)])
        .await
        .unwrap();
    assert_eq!(views.len(), 1);
    assert_eq!(views[0].id(), Uuid::from_u128(9));
}
#[tokio::test]
async fn earlier_list_age_is_not_reused_across_seoul_birthday() {
    let (s, c) = fixture();
    let mut authority = Authority::new(s.clone(), c.clone());
    authority.change = Some((3, Change::Birthday));
    let policy = IntroPolicy::new(Claims::new(s, c.clone()), authority, c);
    assert!(policy
        .list(context(), &[intro(8, 10), intro(9, 11)])
        .await
        .unwrap()
        .is_empty());
}
#[tokio::test]
async fn claim_revocation_invalidates_retained_list_observations() {
    let (s, c) = fixture();
    let mut authority = Authority::new(s.clone(), c.clone());
    authority.change = Some((3, Change::RevokeClaim));
    let policy = IntroPolicy::new(Claims::new(s, c.clone()), authority, c);
    assert!(policy
        .list(context(), &[intro(8, 10), intro(9, 11)])
        .await
        .unwrap()
        .is_empty());
}
#[tokio::test]
async fn original_request_deadline_and_identity_are_mandatory() {
    let (s, c) = fixture();
    let policy = IntroPolicy::new(
        Claims::new(s.clone(), c.clone()),
        Authority::new(s, c.clone()),
        c,
    );
    for field in 0..5 {
        let mut ctx = context();
        match field {
            0 => ctx.actor = user(0),
            1 => ctx.event = EventId::from_uuid(Uuid::nil()),
            2 => ctx.session = Uuid::nil(),
            3 => ctx.request = Uuid::nil(),
            _ => ctx.expires_at = start(),
        };
        assert!(matches!(
            policy.list(ctx, &[]).await,
            Err(PolicyError::Unavailable)
        ));
    }
}
#[tokio::test]
async fn read_lease_checks_bind_the_exact_query() {
    let (s, c) = fixture();
    let authority = Authority::new(s, c.clone());
    let q = ReadQuery {
        context: context(),
        target: ReadTarget::Collection,
        requested_at: c.now(),
    };
    let lease = authority.read(q).await.unwrap();
    let mut other = q;
    other.context.request = Uuid::from_u128(999);
    assert_eq!(lease.check(other, c.now()), Err(PolicyError::Unavailable));
}
