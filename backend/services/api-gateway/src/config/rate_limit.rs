//! Admission budgets: per class and identity scope, shared across instances.
//!
//! Production keeps budgets in PostgreSQL (`gateway_admit`, migration 0005) so
//! every Gateway instance spends the same network, anonymous-identity, and
//! authenticated-principal budgets. Without a store, as in route tests and
//! parity mode, the same rules run in process.
//!
//! Three rules hold on both paths:
//! - A client's budget is never reset early. Capacity pressure does not evict
//!   a live identity, because eviction would hand it a fresh quota.
//! - When detailed capacity is full, an identity without a budget is admitted
//!   only from one shared overflow budget per class, and gets no budget of its
//!   own, so churning identities cannot mint quota.
//! - When the store cannot answer, only reads continue, on a small per-instance
//!   allowance. Every other class fails closed.

use crate::domain::admission::{
    AdmissionOutcome, AdmissionPolicies, RouteClass, StoreFailurePolicy, ADMISSION_WINDOW_SECONDS,
};
use sqlx::{PgPool, Row};
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};
use tokio::sync::{OwnedSemaphorePermit, Semaphore, TryAcquireError};

/// Identities held in process when no store is attached.
const MAX_KEYS: usize = 16_384;
const MAX_KEY_BYTES: usize = 128;
const PRUNE_INTERVAL: Duration = Duration::from_secs(1);
/// Live identities per store bucket; the store has 64 buckets.
pub const STORE_BUCKET_CAPACITY: i32 = 256;
const FALLBACK_WINDOW: Duration = Duration::from_secs(1);

#[derive(Clone)]
pub struct InMemoryRateLimiter {
    policies: Arc<AdmissionPolicies>,
    window: Duration,
    state: Arc<Mutex<RateLimitState>>,
    in_flight: Arc<[Arc<Semaphore>; 8]>,
    /// Per-instance read allowance used only while the store is unavailable.
    fallback: Arc<Mutex<HashMap<RouteClass, Window>>>,
    shared_pool: Option<PgPool>,
    bucket_capacity: i32,
}

struct RateLimitState {
    entries: HashMap<String, Window>,
    overflow: HashMap<String, Window>,
    capacity: usize,
    last_pruned_at: Instant,
}

#[derive(Clone, Copy)]
struct Window {
    started_at: Instant,
    count: u32,
}

impl Window {
    fn new(now: Instant) -> Self {
        Self {
            started_at: now,
            count: 0,
        }
    }

    /// Spend one request if `limit` allows it in the current window.
    fn take(&mut self, now: Instant, window: Duration, limit: u32) -> bool {
        if now.saturating_duration_since(self.started_at) >= window {
            *self = Self::new(now);
        }
        if self.count >= limit {
            return false;
        }
        self.count += 1;
        true
    }
}

impl InMemoryRateLimiter {
    /// Every class with the same budget per `window`.
    pub fn new(max_requests: u32, window: Duration) -> Self {
        Self::with_policies(AdmissionPolicies::uniform(max_requests), window)
    }

    pub fn with_policies(policies: AdmissionPolicies, window: Duration) -> Self {
        let in_flight = Arc::new(std::array::from_fn(|index| {
            Arc::new(Semaphore::new(
                policies.max_in_flight(RouteClass::ALL[index]),
            ))
        }));
        Self {
            policies: Arc::new(policies),
            window,
            in_flight,
            state: Arc::new(Mutex::new(RateLimitState {
                entries: HashMap::new(),
                overflow: HashMap::new(),
                capacity: MAX_KEYS,
                last_pruned_at: Instant::now(),
            })),
            fallback: Arc::default(),
            shared_pool: None,
            bucket_capacity: STORE_BUCKET_CAPACITY,
        }
    }

    /// The production policies over a one-minute window.
    pub fn production(policies: AdmissionPolicies) -> Self {
        Self::with_policies(
            policies,
            Duration::from_secs(u64::from(ADMISSION_WINDOW_SECONDS)),
        )
    }

    /// Keep budgets in the shared store. Production attaches a small pool with
    /// short acquire and statement timeouts, so a slow store is a quick failure.
    pub fn with_database(mut self, pool: PgPool) -> Self {
        self.shared_pool = Some(pool);
        self
    }

    /// Live identities per store bucket. Lowering it is how tests and an
    /// operator under memory pressure reach the overflow path.
    pub fn with_bucket_capacity(mut self, capacity: i32) -> Self {
        self.bucket_capacity = capacity.max(0);
        self
    }

    /// Identities held in process when no store is attached.
    pub fn with_local_capacity(self, capacity: usize) -> Self {
        self.lock_state().capacity = capacity;
        self
    }

    pub fn policies(&self) -> &AdmissionPolicies {
        &self.policies
    }

    /// Length of one admission window. A refused client cannot be admitted
    /// again until the window it is spending resets, so this is the honest
    /// `Retry-After` hint for a quota rejection.
    pub fn window_seconds(&self) -> u64 {
        self.window.as_secs().max(1)
    }

    /// Acquire a non-queuing, per-class request permit for this Gateway process.
    pub fn try_acquire_in_flight(
        &self,
        class: RouteClass,
    ) -> Result<OwnedSemaphorePermit, TryAcquireError> {
        self.in_flight[class.index()].clone().try_acquire_owned()
    }

    /// Decide one request of `class` from client network `network`.
    pub async fn admit(&self, class: RouteClass, network: &str) -> AdmissionOutcome {
        self.admit_observed(class, network).await.0
    }

    /// Decide one network-scoped request and return any PostgreSQL advisory-lock
    /// wait observed while spending the shared budget.
    pub async fn admit_observed(
        &self,
        class: RouteClass,
        network: &str,
    ) -> (AdmissionOutcome, Option<Duration>) {
        let budget = self.policies.budget(class);
        if budget == 0 {
            return (AdmissionOutcome::Admitted, None);
        }
        if network.is_empty() {
            return (AdmissionOutcome::RejectedCardinality, None);
        }
        let key = format!("{class}:{network}");
        self.admit_key(class, &key, budget, class.as_str()).await
    }

    /// Decide the independent budget for a pre-HMACed normalized identity.
    /// Callers must never pass a raw account identifier to this method.
    pub async fn admit_identity(
        &self,
        class: RouteClass,
        identity_digest: &str,
    ) -> AdmissionOutcome {
        self.admit_identity_observed(class, identity_digest).await.0
    }

    /// Decide one identity-scoped request and return any PostgreSQL advisory-lock
    /// wait observed while spending the shared budget.
    pub async fn admit_identity_observed(
        &self,
        class: RouteClass,
        identity_digest: &str,
    ) -> (AdmissionOutcome, Option<Duration>) {
        let budget = self.policies.identity_budget(class);
        if budget == 0 {
            return (AdmissionOutcome::Admitted, None);
        }
        if identity_digest.len() != 64
            || !identity_digest.as_bytes().iter().all(u8::is_ascii_hexdigit)
        {
            return (AdmissionOutcome::RejectedCardinality, None);
        }
        let key = format!("{class}:identity:{identity_digest}");
        let overflow_scope = format!("identity:{class}");
        self.admit_key(class, &key, budget, &overflow_scope).await
    }

    /// Decide the independent budget for a pre-HMACed, verified JWT subject.
    /// Callers must never pass a client-supplied subject or a raw identifier.
    pub async fn admit_principal(
        &self,
        class: RouteClass,
        principal_digest: &str,
    ) -> AdmissionOutcome {
        self.admit_principal_observed(class, principal_digest)
            .await
            .0
    }

    /// Decide one authenticated-principal request and return any PostgreSQL
    /// advisory-lock wait observed while spending the shared budget.
    pub async fn admit_principal_observed(
        &self,
        class: RouteClass,
        principal_digest: &str,
    ) -> (AdmissionOutcome, Option<Duration>) {
        let budget = self.policies.principal_budget(class);
        if budget == 0 {
            return (AdmissionOutcome::Admitted, None);
        }
        if principal_digest.len() != 64
            || !principal_digest
                .as_bytes()
                .iter()
                .all(u8::is_ascii_hexdigit)
        {
            return (AdmissionOutcome::RejectedCardinality, None);
        }
        let key = format!("{class}:principal:{principal_digest}");
        let overflow_scope = format!("principal:{class}");
        self.admit_key(class, &key, budget, &overflow_scope).await
    }

    async fn admit_key(
        &self,
        class: RouteClass,
        key: &str,
        budget: u32,
        overflow_scope: &str,
    ) -> (AdmissionOutcome, Option<Duration>) {
        if budget == 0 {
            return (AdmissionOutcome::Admitted, None);
        }
        if key.len() > MAX_KEY_BYTES || key.is_empty() {
            return (AdmissionOutcome::RejectedCardinality, None);
        }
        let overflow_budget = budget.saturating_mul(self.policies.overflow_factor);
        let Some(pool) = &self.shared_pool else {
            return (
                self.admit_locally(key, budget, overflow_budget, overflow_scope, Instant::now()),
                None,
            );
        };
        match self
            .admit_in_store(pool, key, budget, overflow_budget, overflow_scope)
            .await
        {
            Ok((outcome, lock_wait)) => (outcome, Some(lock_wait)),
            Err(error) => {
                tracing::warn!(
                    error = ?error,
                    class = class.as_str(),
                    scope = overflow_scope,
                    "admission store unavailable"
                );
                (self.on_store_failure(class, Instant::now()), None)
            }
        }
    }

    /// One round trip: the function takes its bucket lock, spends the budget,
    /// and answers with the outcome.
    async fn admit_in_store(
        &self,
        pool: &PgPool,
        key: &str,
        budget: u32,
        overflow_budget: u32,
        overflow_scope: &str,
    ) -> anyhow::Result<(AdmissionOutcome, Duration)> {
        let limit = i32::try_from(budget)?;
        let overflow = i32::try_from(overflow_budget).unwrap_or(i32::MAX);
        let window = i32::try_from(self.window.as_secs())?;
        let row = sqlx::query(
            "select outcome, lock_wait_seconds from gateway_admit_with_lock_wait($1, $2, $3, $4, $5, $6)",
        )
            .bind(key)
            .bind(overflow_scope)
            .bind(limit)
            .bind(window)
            .bind(self.bucket_capacity)
            .bind(overflow)
            .fetch_one(pool)
            .await?;
        let outcome: String = row.try_get("outcome")?;
        let lock_wait_seconds: f64 = row.try_get("lock_wait_seconds")?;
        let lock_wait = Duration::try_from_secs_f64(lock_wait_seconds)?;
        match outcome.as_str() {
            "admitted" => Ok((AdmissionOutcome::Admitted, lock_wait)),
            "overflow" => Ok((AdmissionOutcome::AdmittedOverflow, lock_wait)),
            "quota" => Ok((AdmissionOutcome::RejectedQuota, lock_wait)),
            "cardinality" => Ok((AdmissionOutcome::RejectedCardinality, lock_wait)),
            other => anyhow::bail!("admission store answered {other:?}"),
        }
    }

    fn on_store_failure(&self, class: RouteClass, now: Instant) -> AdmissionOutcome {
        match class.store_failure() {
            StoreFailurePolicy::Reject => AdmissionOutcome::RejectedStoreUnavailable,
            StoreFailurePolicy::BoundedLocalFallback => {
                let allowance = self.policies.fallback_per_second;
                if allowance == 0 {
                    return AdmissionOutcome::RejectedStoreUnavailable;
                }
                let mut fallback = self.fallback.lock().unwrap_or_else(PoisonError::into_inner);
                let window = fallback.entry(class).or_insert_with(|| Window::new(now));
                if window.take(now, FALLBACK_WINDOW, allowance) {
                    AdmissionOutcome::AdmittedFallback
                } else {
                    AdmissionOutcome::RejectedFallbackExhausted
                }
            }
        }
    }

    fn admit_locally(
        &self,
        key: &str,
        budget: u32,
        overflow_budget: u32,
        overflow_scope: &str,
        now: Instant,
    ) -> AdmissionOutcome {
        let window = self.window;
        let mut state = self.lock_state();
        if now.saturating_duration_since(state.last_pruned_at) >= PRUNE_INTERVAL {
            state
                .entries
                .retain(|_, entry| now.saturating_duration_since(entry.started_at) < window);
            state.last_pruned_at = now;
        }
        if let Some(entry) = state.entries.get_mut(key) {
            return if entry.take(now, window, budget) {
                AdmissionOutcome::Admitted
            } else {
                AdmissionOutcome::RejectedQuota
            };
        }
        if state.entries.len() < state.capacity {
            let mut entry = Window::new(now);
            entry.take(now, window, budget);
            state.entries.insert(key.to_owned(), entry);
            return AdmissionOutcome::Admitted;
        }
        let overflow = state
            .overflow
            .entry(overflow_scope.to_owned())
            .or_insert_with(|| Window::new(now));
        if overflow.take(now, window, overflow_budget) {
            AdmissionOutcome::AdmittedOverflow
        } else {
            AdmissionOutcome::RejectedCardinality
        }
    }

    fn lock_state(&self) -> MutexGuard<'_, RateLimitState> {
        self.state.lock().unwrap_or_else(PoisonError::into_inner)
    }

    #[cfg(test)]
    fn admit_at(&self, class: RouteClass, network: &str, now: Instant) -> AdmissionOutcome {
        let budget = self.policies.budget(class);
        if budget == 0 {
            return AdmissionOutcome::Admitted;
        }
        if network.is_empty() {
            return AdmissionOutcome::RejectedCardinality;
        }
        let overflow_budget = budget.saturating_mul(self.policies.overflow_factor);
        self.admit_locally(
            &format!("{class}:{network}"),
            budget,
            overflow_budget,
            class.as_str(),
            now,
        )
    }

    #[cfg(test)]
    fn admit_identity_at(
        &self,
        class: RouteClass,
        identity_digest: &str,
        now: Instant,
    ) -> AdmissionOutcome {
        let budget = self.policies.identity_budget(class);
        if budget == 0 {
            return AdmissionOutcome::Admitted;
        }
        if identity_digest.len() != 64
            || !identity_digest.as_bytes().iter().all(u8::is_ascii_hexdigit)
        {
            return AdmissionOutcome::RejectedCardinality;
        }
        let overflow_budget = budget.saturating_mul(self.policies.overflow_factor);
        self.admit_locally(
            &format!("{class}:identity:{identity_digest}"),
            budget,
            overflow_budget,
            &format!("identity:{class}"),
            now,
        )
    }

    #[cfg(test)]
    fn admit_principal_at(
        &self,
        class: RouteClass,
        principal_digest: &str,
        now: Instant,
    ) -> AdmissionOutcome {
        let budget = self.policies.principal_budget(class);
        if budget == 0 {
            return AdmissionOutcome::Admitted;
        }
        if principal_digest.len() != 64
            || !principal_digest
                .as_bytes()
                .iter()
                .all(u8::is_ascii_hexdigit)
        {
            return AdmissionOutcome::RejectedCardinality;
        }
        let overflow_budget = budget.saturating_mul(self.policies.overflow_factor);
        self.admit_locally(
            &format!("{class}:principal:{principal_digest}"),
            budget,
            overflow_budget,
            &format!("principal:{class}"),
            now,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use AdmissionOutcome::*;

    fn policies() -> AdmissionPolicies {
        AdmissionPolicies::default()
            .with_budget(RouteClass::Login, 2)
            .with_budget(RouteClass::Reset, 1)
            .with_budget(RouteClass::Read, 3)
    }

    #[test]
    fn network_and_identity_budgets_are_independent_dimensions() {
        let policies = AdmissionPolicies::default()
            .with_budget(RouteClass::Login, 10)
            .with_identity_budget(RouteClass::Login, 1);
        let limiter = InMemoryRateLimiter::with_policies(policies, Duration::from_secs(60));
        let now = Instant::now();
        let digest = "a".repeat(64);

        assert_eq!(
            limiter.admit_at(RouteClass::Login, "203.0.113.1", now),
            Admitted
        );
        assert_eq!(
            limiter.admit_at(RouteClass::Login, "203.0.113.2", now),
            Admitted
        );
        assert_eq!(
            limiter.admit_identity_at(RouteClass::Login, &digest, now),
            Admitted
        );
        assert_eq!(
            limiter.admit_identity_at(RouteClass::Login, &digest, now),
            RejectedQuota
        );
        assert_eq!(
            limiter.admit_identity_at(RouteClass::Login, &"b".repeat(64), now),
            Admitted
        );
    }

    #[tokio::test]
    async fn identity_limiter_refuses_raw_unkeyed_identifiers() {
        let policies = AdmissionPolicies::default().with_identity_budget(RouteClass::Login, 1);
        let limiter = InMemoryRateLimiter::with_policies(policies, Duration::from_secs(60));
        assert_eq!(
            limiter
                .admit_identity(RouteClass::Login, "user@example.com")
                .await,
            RejectedCardinality
        );
    }

    #[tokio::test]
    async fn principal_limiter_refuses_raw_subjects_and_shares_only_verified_digests() {
        let policies = AdmissionPolicies::default().with_principal_budget(RouteClass::Search, 1);
        let limiter = InMemoryRateLimiter::with_policies(policies, Duration::from_secs(60));
        assert_eq!(
            limiter
                .admit_principal(RouteClass::Search, "alice@example.test")
                .await,
            RejectedCardinality
        );
        let digest = "a".repeat(64);
        assert_eq!(
            limiter.admit_principal(RouteClass::Search, &digest).await,
            Admitted
        );
        assert_eq!(
            limiter.admit_principal(RouteClass::Search, &digest).await,
            RejectedQuota
        );
        assert_eq!(
            limiter
                .admit_principal(RouteClass::Search, &"b".repeat(64))
                .await,
            Admitted
        );
    }

    #[test]
    fn network_and_principal_budgets_are_independent_dimensions() {
        let policies = AdmissionPolicies::default()
            .with_budget(RouteClass::Search, 1)
            .with_principal_budget(RouteClass::Search, 1);
        let limiter = InMemoryRateLimiter::with_policies(policies, Duration::from_secs(60));
        let now = Instant::now();
        let digest = "a".repeat(64);

        assert_eq!(
            limiter.admit_at(RouteClass::Search, "203.0.113.1", now),
            Admitted
        );
        assert_eq!(
            limiter.admit_at(RouteClass::Search, "198.51.100.1", now),
            Admitted
        );
        assert_eq!(
            limiter.admit_principal_at(RouteClass::Search, &digest, now),
            Admitted
        );
        assert_eq!(
            limiter.admit_principal_at(RouteClass::Search, &digest, now),
            RejectedQuota
        );
        assert_eq!(
            limiter.admit_principal_at(RouteClass::Search, &"b".repeat(64), now),
            Admitted
        );
    }

    #[tokio::test]
    async fn disabled_budgets_are_noops_before_identity_validation() {
        let limiter = InMemoryRateLimiter::with_policies(
            AdmissionPolicies::default().with_budget(RouteClass::Login, 0),
            Duration::from_secs(60),
        );
        assert_eq!(
            limiter
                .admit_identity(RouteClass::Login, "raw-email@example.test")
                .await,
            Admitted
        );
        assert_eq!(limiter.admit(RouteClass::Login, "").await, Admitted);
    }

    #[test]
    fn in_flight_caps_are_independent_per_route_class_and_release_on_drop() {
        let policies = AdmissionPolicies::default()
            .with_max_in_flight(RouteClass::Login, 1)
            .with_max_in_flight(RouteClass::Reset, 1);
        let limiter = InMemoryRateLimiter::with_policies(policies, Duration::from_secs(60));
        let held_login = limiter
            .try_acquire_in_flight(RouteClass::Login)
            .expect("first login permit");

        assert!(limiter.try_acquire_in_flight(RouteClass::Login).is_err());
        assert!(limiter.try_acquire_in_flight(RouteClass::Reset).is_ok());

        drop(held_login);
        assert!(limiter.try_acquire_in_flight(RouteClass::Login).is_ok());
    }

    #[test]
    fn classes_spend_independent_budgets_and_windows_reset() {
        let limiter = InMemoryRateLimiter::with_policies(policies(), Duration::from_secs(60));
        let now = Instant::now();
        assert_eq!(limiter.admit_at(RouteClass::Reset, "client", now), Admitted);
        assert_eq!(
            limiter.admit_at(RouteClass::Reset, "client", now),
            RejectedQuota
        );
        // A reset flood leaves login from the same network untouched.
        assert_eq!(limiter.admit_at(RouteClass::Login, "client", now), Admitted);
        assert_eq!(
            limiter.clone().admit_at(RouteClass::Login, "client", now),
            Admitted
        );
        assert_eq!(
            limiter.admit_at(RouteClass::Login, "client", now),
            RejectedQuota
        );
        let later = now + Duration::from_secs(60);
        assert_eq!(
            limiter.admit_at(RouteClass::Reset, "client", later),
            Admitted
        );
    }

    #[test]
    fn full_capacity_admits_unseen_identities_only_from_a_bounded_overflow() {
        // Given: capacity for two identities, both already spending budgets.
        let limiter = InMemoryRateLimiter::with_policies(policies(), Duration::from_secs(60))
            .with_local_capacity(2);
        let now = Instant::now();
        for client in ["a", "b"] {
            for _ in 0..3 {
                assert_eq!(limiter.admit_at(RouteClass::Read, client, now), Admitted);
            }
        }

        // When: fifty new identities arrive.
        let outcomes: Vec<_> = (0..50)
            .map(|index| limiter.admit_at(RouteClass::Read, &format!("churn-{index}"), now))
            .collect();

        // Then: they share one overflow budget of factor × class budget, get
        // no budget of their own, and the existing identities keep theirs.
        let overflow = policies().overflow_budget(RouteClass::Read) as usize;
        assert_eq!(
            outcomes.iter().filter(|o| **o == AdmittedOverflow).count(),
            overflow
        );
        assert_eq!(
            outcomes
                .iter()
                .filter(|o| **o == RejectedCardinality)
                .count(),
            50 - overflow
        );
        assert_eq!(limiter.admit_at(RouteClass::Read, "a", now), RejectedQuota);
        assert_eq!(limiter.lock_state().entries.len(), 2);
        // Another class has its own overflow budget.
        assert_eq!(
            limiter.admit_at(RouteClass::Login, "churn-0", now),
            AdmittedOverflow
        );
    }

    #[test]
    fn expired_keys_are_reclaimed_without_needing_to_revisit_them() {
        let limiter = InMemoryRateLimiter::new(1, Duration::from_secs(60));
        let now = Instant::now();
        assert_eq!(limiter.admit_at(RouteClass::Read, "old", now), Admitted);
        let later = now + Duration::from_secs(61);
        assert_eq!(limiter.admit_at(RouteClass::Read, "new", later), Admitted);
        let state = limiter.lock_state();
        assert_eq!(state.entries.len(), 1);
        assert!(!state.entries.contains_key("read:old"));
    }

    #[tokio::test]
    async fn oversized_and_empty_networks_are_refused_before_allocation() {
        let limiter = InMemoryRateLimiter::new(1, Duration::from_secs(60));
        assert_eq!(
            limiter
                .admit(RouteClass::Read, &"x".repeat(MAX_KEY_BYTES))
                .await,
            RejectedCardinality
        );
        assert_eq!(
            limiter.admit(RouteClass::Read, "").await,
            RejectedCardinality
        );
        assert!(limiter.lock_state().entries.is_empty());
    }

    #[tokio::test]
    async fn a_zero_budget_leaves_the_class_unlimited_without_allocating() {
        let limiter = InMemoryRateLimiter::new(0, Duration::from_secs(60));
        assert_eq!(limiter.admit(RouteClass::Login, "client").await, Admitted);
        assert!(limiter.lock_state().entries.is_empty());
    }

    fn unreachable_store() -> PgPool {
        // Port 1 refuses connections at once, which is a store outage.
        sqlx::postgres::PgPoolOptions::new()
            .acquire_timeout(Duration::from_millis(100))
            .connect_lazy("postgres://gateway:gateway@127.0.0.1:1/gateway")
            .expect("lazy pool")
    }

    #[tokio::test]
    async fn without_a_store_only_reads_continue_on_a_bounded_local_allowance() {
        let mut policies = policies();
        policies.fallback_per_second = 2;
        let limiter = InMemoryRateLimiter::with_policies(policies, Duration::from_secs(60))
            .with_database(unreachable_store());

        for class in [
            RouteClass::Login,
            RouteClass::Signup,
            RouteClass::Reset,
            RouteClass::VerificationResend,
            RouteClass::Refresh,
            RouteClass::Search,
            RouteClass::Write,
        ] {
            assert_eq!(
                limiter.admit(class, "client").await,
                RejectedStoreUnavailable
            );
        }
        // The allowance is per instance and per second, not per client, so no
        // client gains a fresh quota from the outage.
        assert_eq!(limiter.admit(RouteClass::Read, "a").await, AdmittedFallback);
        assert_eq!(limiter.admit(RouteClass::Read, "b").await, AdmittedFallback);
        assert_eq!(
            limiter.admit(RouteClass::Read, "c").await,
            RejectedFallbackExhausted
        );
    }

    #[tokio::test]
    async fn a_zero_allowance_makes_reads_fail_closed_too() {
        let mut policies = policies();
        policies.fallback_per_second = 0;
        let limiter = InMemoryRateLimiter::with_policies(policies, Duration::from_secs(60))
            .with_database(unreachable_store());
        assert_eq!(
            limiter.admit(RouteClass::Read, "client").await,
            RejectedStoreUnavailable
        );
    }
}
