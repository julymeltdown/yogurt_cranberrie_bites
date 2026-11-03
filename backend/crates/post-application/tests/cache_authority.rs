//! Behavioral unit tests. Doubles here do not establish persistence/network acceptance.
use async_trait::async_trait;
use chrono::Utc;
use post_application::cache_authority::*;
use post_application::delivery_authority::{DeliveryAttemptProvenanceDto, DeliveryLeaseKindDto};
use post_application::delivery_authority::{
    DeliveryAuthorityErrorDto as Error, DeliveryAuthorityRepositoryPort, DeliveryClaimLookupDto,
    OwnedDeliveryClaimDto,
};
use post_application::delivery_effect::{DeliveryEffectDeadlineDto, DeliveryEffectFailure};
use post_application::dto::delivery_schedule_dto::TargetDeliveryClaimDto;
use post_application::ports::{FeedCachePort, PostCachePort};
use post_domain::post::{DeliveryTarget, StoredEvent};
use post_domain::{
    feed::FeedCachePage,
    post::{DeliveryAttemptSource, Post},
};
use serde_json::json;
use std::sync::Arc;
use std::{
    collections::{HashMap, VecDeque},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Mutex,
    },
};
use uuid::Uuid;

/// Polls one runtime-free future to completion on the test thread.
///
/// The application core has no async runtime dependency, and these doubles
/// perform no timer or I/O work, so a parking executor is sufficient. It keeps
/// the core's dependency closure free of an executor even for tests.
fn block_on<F: std::future::Future>(future: F) -> F::Output {
    use std::task::{Context, Poll, Wake, Waker};
    struct ThreadWaker(std::thread::Thread);
    impl Wake for ThreadWaker {
        fn wake(self: Arc<Self>) {
            self.0.unpark();
        }
    }
    let waker = Waker::from(Arc::new(ThreadWaker(std::thread::current())));
    let mut context = Context::from_waker(&waker);
    let mut future = std::pin::pin!(future);
    loop {
        match future.as_mut().poll(&mut context) {
            Poll::Ready(output) => return output,
            Poll::Pending => std::thread::park(),
        }
    }
}

fn event(kind: &str) -> StoredEvent {
    let post = Uuid::from_u128(11);
    let actor = Uuid::from_u128(12);
    let author = Uuid::from_u128(13);
    let payload = match kind {
        "POST_CREATED" | "POST_DELETED" => json!({"postId":post,"authorId":actor,"actorId":actor}),
        "POST_COMMENTED" => {
            json!({"postId":post,"authorId":actor,"actorId":actor,"notifyUserId":author})
        }
        "POST_COMMENT_DELETED" => {
            json!({"postId":post,"authorId":actor,"actorId":actor,"postAuthorId":author})
        }
        _ => json!({"postId":post,"actorId":actor,"authorId":author,"notifyUserId":author}),
    };
    StoredEvent {
        event_id: Uuid::new_v4(),
        event_type: kind.into(),
        aggregate_id: post,
        aggregate_version: Some(1),
        origin_traceparent: Some("retained trace".into()),
        actor_id: Some(actor),
        attributes: vec!["one".into(), "two".into()],
        payload: payload.to_string(),
        occurred_at: Utc::now(),
        created_at: Utc::now(),
        published: false,
        published_at: None,
        replay_resolution_id: None,
    }
}
fn claim(kind: &str) -> TargetDeliveryClaimDto {
    TargetDeliveryClaimDto {
        event: event(kind),
        target: DeliveryTarget::Cache,
        lease_token: Uuid::new_v4(),
        attempt_id: Uuid::new_v4(),
        attempt_number: 1,
        lease_expires_at: Utc::now() + chrono::Duration::minutes(1),
    }
}
fn owned(claim: TargetDeliveryClaimDto) -> OwnedDeliveryClaimDto {
    OwnedDeliveryClaimDto::from_persisted_claim(
        claim,
        DeliveryAttemptProvenanceDto {
            lease_kind: DeliveryLeaseKindDto::Automatic,
            source: DeliveryAttemptSource::OutboxWorker,
            resolution_id: None,
            max_attempts: 3,
            progress_credits: 0,
            target_required: true,
        },
        Utc::now(),
    )
    .unwrap()
}
fn cursor(id: Uuid, second: u32) -> String {
    format!("2026-10-14T00:00:{second:02}Z|{id}")
}
fn request() -> CachePageLookupDto {
    let c = claim("POST_CREATED");
    let author = c.event.actor_id.unwrap();
    CachePageLookupDto::new(owned(c), author, None, 2).unwrap()
}

#[test]
fn producer_specific_effect_matrix_and_stored_unlike_preserve_exact_targets() {
    for (kind, evicts, author) in [
        ("POST_CREATED", false, Some(12)),
        ("POST_DELETED", true, Some(12)),
        ("POST_COMMENTED", true, Some(13)),
        ("POST_COMMENT_DELETED", true, Some(13)),
        ("POST_LIKED", true, None),
        ("POST_UNLIKED", true, None),
        ("FUTURE_EVENT", false, None),
    ] {
        let e = event(kind);
        let plan = CacheInvalidationPlanDto::from_event(&e).unwrap();
        assert_eq!(plan.post_id(), evicts.then_some(e.aggregate_id), "{kind}");
        assert_eq!(plan.feed_author_id(), author.map(Uuid::from_u128), "{kind}");
    }
    let mut e = event("POST_UNLIKED");
    e.payload =
        json!({"postId":e.aggregate_id,"authorId":e.actor_id.unwrap(),"likeCount":0}).to_string();
    assert!(CacheInvalidationPlanDto::from_event(&e).is_ok());
    e.actor_id = Some(Uuid::new_v4());
    assert!(CacheInvalidationPlanDto::from_event(&e).is_err());
}
#[test]
fn malformed_known_payloads_and_conflicting_producer_identity_deny_before_plan() {
    for kind in [
        "POST_CREATED",
        "POST_DELETED",
        "POST_COMMENTED",
        "POST_COMMENT_DELETED",
        "POST_LIKED",
        "POST_UNLIKED",
    ] {
        for variant in 0..5 {
            let mut e = event(kind);
            let mut payload: serde_json::Value = serde_json::from_str(&e.payload).unwrap();
            match variant {
                0 => payload["postId"] = json!(Uuid::new_v4()),
                1 => e.actor_id = None,
                2 => payload["actorId"] = json!(Uuid::new_v4()),
                3 => payload["authorId"] = json!(Uuid::nil()),
                _ => payload["postId"] = json!(Uuid::nil()),
            }
            e.payload = payload.to_string();
            assert!(
                CacheInvalidationPlanDto::from_event(&e).is_err(),
                "{kind}/{variant}"
            );
        }
    }
    let mut e = event("FUTURE_EVENT");
    e.payload = "malformed".into();
    assert!(CacheInvalidationPlanDto::from_event(&e).is_err());
}
#[test]
fn event_identity_binds_exact_payload_attribute_order_and_trace_but_not_publication() {
    let e = event("POST_CREATED");
    let original = cache_event_identity(&e).unwrap();
    for variant in 0..7 {
        let mut changed = e.clone();
        match variant {
            0 => changed.payload.push(' '),
            1 => changed.attributes.reverse(),
            2 => changed.origin_traceparent = None,
            3 => changed.actor_id = Some(Uuid::new_v4()),
            4 => changed.aggregate_version = None,
            5 => changed.created_at += chrono::Duration::microseconds(1),
            _ => changed.event_type = "POST_DELETED".into(),
        }
        assert_ne!(cache_event_identity(&changed).unwrap(), original);
    }
    let mut changed = e;
    changed.published = true;
    changed.published_at = Some(Utc::now());
    changed.replay_resolution_id = Some(Uuid::new_v4());
    assert_eq!(cache_event_identity(&changed).unwrap(), original);
}
#[test]
fn attempt_identity_retains_recovery_provenance_and_frozen_budget() {
    let o = owned(claim("POST_CREATED"));
    let original = cache_attempt_identity(&o).unwrap();
    for variant in 0..2 {
        let mut p = o.provenance();
        if variant == 0 {
            p.max_attempts += 1;
        } else {
            p.source = DeliveryAttemptSource::AdminReplay;
            p.resolution_id = Some(Uuid::new_v4());
        }
        let other =
            OwnedDeliveryClaimDto::from_persisted_claim(o.lookup().claim().clone(), p, Utc::now())
                .unwrap();
        assert_ne!(cache_attempt_identity(&other).unwrap(), original);
    }
}
#[test]
fn page_rejects_wrong_author_duplicates_self_nil_oversize_and_invalid_progress() {
    let r = request();
    let a = Uuid::from_u128(21);
    let b = Uuid::from_u128(22);
    for (author, ids, next) in [
        (Uuid::new_v4(), vec![a], None),
        (r.author_id(), vec![a, a], None),
        (r.author_id(), vec![r.author_id()], None),
        (r.author_id(), vec![Uuid::nil()], None),
        (r.author_id(), vec![a, b, Uuid::new_v4()], None),
        (r.author_id(), vec![], Some(cursor(a, 10))),
        (r.author_id(), vec![a], Some(cursor(b, 10))),
        (r.author_id(), vec![a], Some(format!(" {}", cursor(a, 10)))),
    ] {
        assert!(
            CacheFollowerPageDto::new(&r, author, ids, next, r.deadline().expires_at()).is_err()
        );
    }
    let r =
        CachePageLookupDto::new(r.owner().clone(), r.author_id(), Some(cursor(a, 10)), 2).unwrap();
    assert!(CacheFollowerPageDto::new(
        &r,
        r.author_id(),
        vec![a],
        Some(cursor(a, 10)),
        r.deadline().expires_at()
    )
    .is_err());
    assert!(CacheFollowerPageDto::new(
        &r,
        r.author_id(),
        vec![b],
        Some(cursor(b, 11)),
        r.deadline().expires_at()
    )
    .is_err());
    assert!(CacheFollowerPageDto::new(
        &r,
        r.author_id(),
        vec![b],
        Some(cursor(b, 9)),
        r.deadline().expires_at()
    )
    .is_ok());
}

struct Owner {
    calls: AtomicUsize,
    deny_at: usize,
}
#[async_trait]
impl DeliveryAuthorityRepositoryPort for Owner {
    async fn resolve_claimed_delivery(
        &self,
        r: &DeliveryClaimLookupDto,
    ) -> Result<OwnedDeliveryClaimDto, Error> {
        let count = self.calls.fetch_add(1, Ordering::SeqCst) + 1;
        if count == self.deny_at {
            Err(Error::Denied)
        } else {
            Ok(owned(r.claim().clone()))
        }
    }
}
enum Page {
    Data(Vec<Uuid>, Option<String>),
    Foreign(CacheFollowerPageDto),
    Fail,
}
struct Pages {
    queue: Mutex<VecDeque<Page>>,
    cursors: Mutex<Vec<Option<String>>>,
}
#[async_trait]
impl ClaimedCachePageReadPort for Pages {
    async fn fetch_cache_page(
        &self,
        r: &CachePageLookupDto,
    ) -> anyhow::Result<CacheFollowerPageDto> {
        self.cursors
            .lock()
            .unwrap()
            .push(r.cursor().map(str::to_owned));
        match self
            .queue
            .lock()
            .unwrap()
            .pop_front()
            .expect("unexpected page read")
        {
            Page::Data(ids, next) => {
                CacheFollowerPageDto::new(r, r.author_id(), ids, next, (r).deadline().expires_at())
            }
            Page::Fail => anyhow::bail!("page read failed"),
            Page::Foreign(page) => Ok(page),
        }
    }
}

#[test]
fn constructed_page_for_another_author_limit_or_cursor_is_checked_against_actual_request() {
    block_on(
        constructed_page_for_another_author_limit_or_cursor_is_checked_against_actual_request_body(
        ),
    );
}

async fn constructed_page_for_another_author_limit_or_cursor_is_checked_against_actual_request_body(
) {
    for case in 0..3 {
        let c = claim("POST_CREATED");
        let mut foreign_claim = c.clone();
        if case == 0 {
            let other = Uuid::new_v4();
            foreign_claim.event.actor_id = Some(other);
            foreign_claim.event.payload =
                json!({"postId":c.event.aggregate_id,"authorId":other,"actorId":other}).to_string();
        }
        let author = foreign_claim.event.actor_id.unwrap();
        let foreign = CachePageLookupDto::new(owned(foreign_claim), author, None, 100).unwrap();
        let a = Uuid::from_u128(31);
        let b = Uuid::from_u128(32);
        let bad = match case {
            0 => CacheFollowerPageDto::new(
                &foreign,
                author,
                vec![a],
                None,
                foreign.deadline().expires_at(),
            )
            .unwrap(),
            1 => CacheFollowerPageDto::new(
                &foreign,
                author,
                vec![a, b, Uuid::from_u128(33)],
                None,
                foreign.deadline().expires_at(),
            )
            .unwrap(),
            _ => CacheFollowerPageDto::new(
                &foreign,
                author,
                vec![b],
                Some(cursor(b, 11)),
                foreign.deadline().expires_at(),
            )
            .unwrap(),
        };
        let mut pages = vec![];
        if case == 2 {
            pages.push(Page::Data(vec![a], Some(cursor(a, 10))));
        }
        pages.push(Page::Foreign(bad));
        let (app, _, _, e) = fixture(0, pages, Effects::default());
        let error = app
            .invalidate_claimed_cache(&c, test_deadline())
            .await
            .unwrap_err();
        assert_eq!(e.feeds.lock().unwrap().len(), usize::from(case == 2));
        assert!(e.posts.lock().unwrap().is_empty());
        assert_eq!(
            DeliveryEffectFailure::classify(&error),
            if case == 2 {
                DeliveryEffectFailure::Uncertain
            } else {
                DeliveryEffectFailure::ConfirmedNoEffect
            }
        );
    }
}
#[derive(Default)]
struct Effects {
    posts: Mutex<Vec<Uuid>>,
    feeds: Mutex<Vec<Vec<Uuid>>>,
    fail_post: bool,
    fail_feed: bool,
}
#[async_trait]
impl FeedCachePort for Effects {
    async fn get_feed(
        &self,
        _: Uuid,
        _: Option<&str>,
        _: i32,
    ) -> anyhow::Result<Option<FeedCachePage>> {
        panic!("unexpected read")
    }
    async fn current_generation(&self, _: Uuid) -> anyhow::Result<u64> {
        panic!("unexpected read")
    }
    async fn set_feed(
        &self,
        _: Uuid,
        _: Option<&str>,
        _: i32,
        _: &FeedCachePage,
        _: u64,
    ) -> anyhow::Result<()> {
        panic!("unexpected write")
    }
    async fn invalidate_users(&self, ids: &[Uuid]) -> anyhow::Result<()> {
        self.feeds.lock().unwrap().push(ids.to_vec());
        anyhow::ensure!(!self.fail_feed, "uncertain Redis write");
        Ok(())
    }
}
#[async_trait]
impl PostCachePort for Effects {
    async fn get_posts(&self, _: &[Uuid]) -> anyhow::Result<HashMap<Uuid, Post>> {
        panic!("unexpected read")
    }
    async fn set_post(&self, _: &Post) -> anyhow::Result<()> {
        panic!("unexpected write")
    }
    async fn set_posts(&self, _: &[Post]) -> anyhow::Result<()> {
        panic!("unexpected write")
    }
    async fn evict(&self, id: Uuid) -> anyhow::Result<()> {
        self.posts.lock().unwrap().push(id);
        anyhow::ensure!(!self.fail_post, "uncertain Redis eviction");
        Ok(())
    }
}
fn fixture(
    deny_at: usize,
    pages: Vec<Page>,
    effects: Effects,
) -> (
    InvalidateClaimedCacheUseCase,
    Arc<Owner>,
    Arc<Pages>,
    Arc<Effects>,
) {
    let o = Arc::new(Owner {
        calls: AtomicUsize::new(0),
        deny_at,
    });
    let p = Arc::new(Pages {
        queue: Mutex::new(pages.into()),
        cursors: Mutex::new(vec![]),
    });
    let e = Arc::new(effects);
    let app = InvalidateClaimedCacheUseCase::new(o.clone(), p.clone(), e.clone(), 2).unwrap();
    (app, o, p, e)
}
#[test]
fn author_only_created_event_invalidates_author_without_post_eviction() {
    block_on(author_only_created_event_invalidates_author_without_post_eviction_body());
}

async fn author_only_created_event_invalidates_author_without_post_eviction_body() {
    let c = claim("POST_CREATED");
    let (app, o, p, e) = fixture(0, vec![Page::Data(vec![], None)], Effects::default());
    app.invalidate_claimed_cache(&c, test_deadline())
        .await
        .unwrap();
    assert_eq!(
        *e.feeds.lock().unwrap(),
        vec![vec![c.event.actor_id.unwrap()]]
    );
    assert!(e.posts.lock().unwrap().is_empty());
    assert_eq!(*p.cursors.lock().unwrap(), vec![None]);
    assert_eq!(o.calls.load(Ordering::SeqCst), 2);
}
#[test]
fn deletion_processes_all_pages_and_adds_author_exactly_once() {
    block_on(deletion_processes_all_pages_and_adds_author_exactly_once_body());
}

async fn deletion_processes_all_pages_and_adds_author_exactly_once_body() {
    let c = claim("POST_DELETED");
    let a = Uuid::from_u128(21);
    let b = Uuid::from_u128(22);
    let d = Uuid::from_u128(23);
    let next = cursor(b, 10);
    let (app, _, p, e) = fixture(
        0,
        vec![
            Page::Data(vec![a, b], Some(next.clone())),
            Page::Data(vec![d], None),
        ],
        Effects::default(),
    );
    app.invalidate_claimed_cache(&c, test_deadline())
        .await
        .unwrap();
    assert_eq!(*e.posts.lock().unwrap(), vec![c.event.aggregate_id]);
    assert_eq!(
        *e.feeds.lock().unwrap(),
        vec![vec![a, b, c.event.actor_id.unwrap()], vec![d]]
    );
    assert_eq!(*p.cursors.lock().unwrap(), vec![None, Some(next)]);
}
#[test]
fn likes_and_unlikes_require_owner_recheck_and_never_request_friend_pages() {
    block_on(likes_and_unlikes_require_owner_recheck_and_never_request_friend_pages_body());
}

async fn likes_and_unlikes_require_owner_recheck_and_never_request_friend_pages_body() {
    for kind in ["POST_LIKED", "POST_UNLIKED"] {
        let c = claim(kind);
        let (app, o, p, e) = fixture(0, vec![], Effects::default());
        app.invalidate_claimed_cache(&c, test_deadline())
            .await
            .unwrap();
        assert_eq!(*e.posts.lock().unwrap(), vec![c.event.aggregate_id]);
        assert!(e.feeds.lock().unwrap().is_empty());
        assert!(p.cursors.lock().unwrap().is_empty());
        assert_eq!(o.calls.load(Ordering::SeqCst), 2);
    }
}
#[test]
fn missing_owner_or_loss_after_projection_prevents_first_effect() {
    block_on(missing_owner_or_loss_after_projection_prevents_first_effect_body());
}

async fn missing_owner_or_loss_after_projection_prevents_first_effect_body() {
    for deny in [1, 2] {
        let (app, _, _, e) = fixture(deny, vec![Page::Data(vec![], None)], Effects::default());
        let error = app
            .invalidate_claimed_cache(&claim("POST_CREATED"), test_deadline())
            .await
            .unwrap_err();
        assert_eq!(
            DeliveryEffectFailure::classify(&error),
            DeliveryEffectFailure::ConfirmedNoEffect
        );
        assert!(e.posts.lock().unwrap().is_empty());
        assert!(e.feeds.lock().unwrap().is_empty());
    }
    let (app, _, _, e) = fixture(2, vec![], Effects::default());
    let error = app
        .invalidate_claimed_cache(&claim("POST_DELETED"), test_deadline())
        .await
        .unwrap_err();
    assert_eq!(
        DeliveryEffectFailure::classify(&error),
        DeliveryEffectFailure::ConfirmedNoEffect
    );
    assert!(e.posts.lock().unwrap().is_empty());
}
#[test]
fn malformed_late_feed_scope_cannot_evict_post_before_complete_plan_validation() {
    block_on(malformed_late_feed_scope_cannot_evict_post_before_complete_plan_validation_body());
}

async fn malformed_late_feed_scope_cannot_evict_post_before_complete_plan_validation_body() {
    let mut c = claim("POST_COMMENT_DELETED");
    let mut payload: serde_json::Value = serde_json::from_str(&c.event.payload).unwrap();
    payload["postAuthorId"] = json!(Uuid::nil());
    c.event.payload = payload.to_string();
    let (app, _, p, e) = fixture(0, vec![], Effects::default());
    let error = app
        .invalidate_claimed_cache(&c, test_deadline())
        .await
        .unwrap_err();
    assert_eq!(
        DeliveryEffectFailure::classify(&error),
        DeliveryEffectFailure::ConfirmedNoEffect
    );
    assert!(e.posts.lock().unwrap().is_empty());
    assert!(p.cursors.lock().unwrap().is_empty());
}
#[test]
fn page_failure_is_confirmed_only_before_any_cache_effect() {
    block_on(page_failure_is_confirmed_only_before_any_cache_effect_body());
}

async fn page_failure_is_confirmed_only_before_any_cache_effect_body() {
    for kind in ["POST_CREATED", "POST_DELETED"] {
        let (app, _, _, e) = fixture(0, vec![Page::Fail], Effects::default());
        let error = app
            .invalidate_claimed_cache(&claim(kind), test_deadline())
            .await
            .unwrap_err();
        assert_eq!(
            DeliveryEffectFailure::classify(&error),
            if kind == "POST_CREATED" {
                DeliveryEffectFailure::ConfirmedNoEffect
            } else {
                DeliveryEffectFailure::Uncertain
            }
        );
        assert_eq!(
            e.posts.lock().unwrap().len(),
            usize::from(kind == "POST_DELETED")
        );
    }
}
#[test]
fn later_page_failure_and_lost_owner_remain_uncertain_after_first_feed_write() {
    block_on(later_page_failure_and_lost_owner_remain_uncertain_after_first_feed_write_body());
}

async fn later_page_failure_and_lost_owner_remain_uncertain_after_first_feed_write_body() {
    let follower = Uuid::from_u128(21);
    for denied in [0, 3] {
        let second = if denied == 0 {
            Page::Fail
        } else {
            Page::Data(vec![], None)
        };
        let (app, _, _, e) = fixture(
            denied,
            vec![
                Page::Data(vec![follower], Some(cursor(follower, 10))),
                second,
            ],
            Effects::default(),
        );
        let error = app
            .invalidate_claimed_cache(&claim("POST_CREATED"), test_deadline())
            .await
            .unwrap_err();
        assert_eq!(
            DeliveryEffectFailure::classify(&error),
            DeliveryEffectFailure::Uncertain
        );
        assert_eq!(e.feeds.lock().unwrap().len(), 1);
    }
}
#[test]
fn repeated_recipient_on_later_page_cannot_trigger_a_second_feed_write() {
    block_on(repeated_recipient_on_later_page_cannot_trigger_a_second_feed_write_body());
}

async fn repeated_recipient_on_later_page_cannot_trigger_a_second_feed_write_body() {
    let a = Uuid::from_u128(21);
    let b = Uuid::from_u128(22);
    let (app, _, _, e) = fixture(
        0,
        vec![
            Page::Data(vec![a], Some(cursor(a, 10))),
            Page::Data(vec![a, b], None),
        ],
        Effects::default(),
    );
    let error = app
        .invalidate_claimed_cache(&claim("POST_CREATED"), test_deadline())
        .await
        .unwrap_err();
    assert_eq!(
        DeliveryEffectFailure::classify(&error),
        DeliveryEffectFailure::Uncertain
    );
    assert_eq!(e.feeds.lock().unwrap().len(), 1);
}
#[test]
fn cache_write_errors_are_uncertain_even_without_successful_acknowledgement() {
    block_on(cache_write_errors_are_uncertain_even_without_successful_acknowledgement_body());
}

async fn cache_write_errors_are_uncertain_even_without_successful_acknowledgement_body() {
    for post in [false, true] {
        let (app, _, _, _) = fixture(
            0,
            vec![Page::Data(vec![], None)],
            Effects {
                fail_post: post,
                fail_feed: !post,
                ..Effects::default()
            },
        );
        let error = app
            .invalidate_claimed_cache(
                &claim(if post { "POST_DELETED" } else { "POST_CREATED" }),
                test_deadline(),
            )
            .await
            .unwrap_err();
        assert_eq!(
            DeliveryEffectFailure::classify(&error),
            DeliveryEffectFailure::Uncertain
        );
    }
}

fn test_deadline() -> DeliveryEffectDeadlineDto {
    DeliveryEffectDeadlineDto::new(
        Utc::now() + chrono::Duration::seconds(30),
        std::time::Instant::now() + std::time::Duration::from_secs(30),
    )
    .unwrap()
}
#[async_trait]
impl ClaimedCacheEffectsPort for Effects {
    async fn evict_post(
        &self,
        id: Uuid,
        _deadline: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<()> {
        self.evict(id).await
    }
    async fn invalidate_feeds(
        &self,
        ids: &[Uuid],
        _deadline: DeliveryEffectDeadlineDto,
    ) -> anyhow::Result<()> {
        self.invalidate_users(ids).await
    }
}

#[test]
fn empty_page_rejects_missing_expired_and_extended_authority_but_accepts_shorter() {
    let request = request();
    let author = request.author_id();
    for expiry in [
        chrono::DateTime::from_timestamp_micros(0).unwrap(),
        Utc::now() - chrono::Duration::microseconds(1),
        request.deadline().expires_at() + chrono::Duration::microseconds(1),
    ] {
        assert!(CacheFollowerPageDto::new(&request, author, vec![], None, expiry).is_err());
    }
    let expiry = Utc::now() + chrono::Duration::seconds(1);
    let page = CacheFollowerPageDto::new(&request, author, vec![], None, expiry).unwrap();
    assert_eq!(page.into_parts().2, expiry);
}
