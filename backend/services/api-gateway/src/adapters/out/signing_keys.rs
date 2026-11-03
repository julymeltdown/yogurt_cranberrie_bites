//! Keeps the Gateway's verification keys current with the issuer.
//!
//! Keys used to be read once from configuration, so rotating the signing key
//! required a synchronized redeploy of every Gateway: a key published after
//! startup stayed unknown, and a key withdrawn after a compromise kept
//! verifying until the next deploy. The refresher fetches the issuer's
//! published key set over the authenticated auth channel on a schedule, and
//! early when a token names a key this Gateway does not hold.
//!
//! Three rules bound what an outside caller can cause:
//!
//! - Keys come only from the issuer channel. A token cannot name a key
//!   location; the verifier refuses headers that try.
//! - An unknown `kid` is chosen by whoever sent the token, so it only asks for
//!   a refresh, and at most one such refresh starts per
//!   `unknown_kid_min_interval`, however many tokens arrive.
//! - A failed fetch, or a document that is not a usable key set, changes
//!   nothing: the last installed set keeps verifying.

use crate::adapters::out::jwt::{JwtVerifier, VerificationKeys};
use crate::application::ports::SigningKeySourcePort;
use crate::config::GatewayMetrics;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard, PoisonError};
use std::time::Duration;
use tokio::time::Instant;

/// Longest one fetch may take before it counts as failed.
///
/// The auth channel already carries its edge deadline. This bound also covers
/// a source that never answers, so one stuck fetch cannot hold every later
/// refresh behind it.
pub const FETCH_DEADLINE: Duration = Duration::from_secs(5);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefreshPolicy {
    /// Time between scheduled refreshes. It bounds how long a newly published
    /// key takes to reach this Gateway and how long a withdrawn key keeps
    /// verifying here.
    pub interval: Duration,
    /// Least time between refreshes that tokens naming an unknown key may start.
    pub unknown_kid_min_interval: Duration,
}

/// What started a refresh.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshTrigger {
    Scheduled,
    UnknownKid,
}

impl RefreshTrigger {
    pub fn label(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::UnknownKid => "unknown_kid",
        }
    }
}

/// How a refresh ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RefreshOutcome {
    /// The fetched set was installed.
    Installed,
    /// The fetch failed or timed out. The installed set is unchanged.
    FetchFailed,
    /// The issuer answered with a document that is not a usable key set. The
    /// installed set is unchanged.
    Rejected,
    /// A refresh for an unknown key was not started, because one started too
    /// recently.
    Suppressed,
}

impl RefreshOutcome {
    pub fn label(self) -> &'static str {
        match self {
            Self::Installed => "installed",
            Self::FetchFailed => "fetch_failed",
            Self::Rejected => "rejected",
            Self::Suppressed => "suppressed",
        }
    }
}

/// Fetches the issuer's key set and installs it into a verifier.
///
/// Clones share one state, so the scheduled loop and request handlers draw on
/// the same rate limit.
#[derive(Clone)]
pub struct SigningKeyRefresher {
    inner: Arc<Inner>,
}

struct Inner {
    verifier: JwtVerifier,
    source: Arc<dyn SigningKeySourcePort>,
    policy: RefreshPolicy,
    metrics: Option<GatewayMetrics>,
    /// When the most recent fetch started, whatever caused it.
    last_fetch_started: Mutex<Option<Instant>>,
    /// Held for the length of a fetch, so sets are installed in the order they
    /// were fetched and an older answer never replaces a newer one.
    fetching: tokio::sync::Mutex<()>,
    fetches: AtomicU64,
}

impl SigningKeyRefresher {
    pub fn new(
        verifier: JwtVerifier,
        source: Arc<dyn SigningKeySourcePort>,
        policy: RefreshPolicy,
        metrics: Option<GatewayMetrics>,
    ) -> Self {
        if let Some(metrics) = &metrics {
            metrics.set_signing_key_refresh_interval(policy.interval.as_secs());
        }
        Self {
            inner: Arc::new(Inner {
                verifier,
                source,
                policy,
                metrics,
                last_fetch_started: Mutex::new(None),
                fetching: tokio::sync::Mutex::new(()),
                fetches: AtomicU64::new(0),
            }),
        }
    }

    pub fn policy(&self) -> RefreshPolicy {
        self.inner.policy
    }

    /// Fetches started so far, whatever caused them.
    pub fn fetches(&self) -> u64 {
        self.inner.fetches.load(Ordering::Relaxed)
    }

    /// Fetch the issuer's key set now and install it if it is usable.
    pub async fn refresh(&self) -> RefreshOutcome {
        *self.last_fetch_started() = Some(Instant::now());
        self.fetch_and_install(RefreshTrigger::Scheduled).await
    }

    /// Ask for a refresh because a token named a key this Gateway does not hold.
    ///
    /// Returns `true` only when this call fetched and installed a key set, so
    /// the caller may check its token once more. A call within
    /// `unknown_kid_min_interval` of the previous fetch starts nothing and
    /// returns `false`, and the token is judged against the installed keys.
    pub async fn refresh_for_unknown_kid(&self) -> bool {
        {
            let mut last_started = self.last_fetch_started();
            let now = Instant::now();
            let too_soon = last_started.is_some_and(|started| {
                now.saturating_duration_since(started) < self.inner.policy.unknown_kid_min_interval
            });
            if too_soon {
                drop(last_started);
                self.record(RefreshTrigger::UnknownKid, RefreshOutcome::Suppressed);
                return false;
            }
            // Claimed before the fetch is awaited, so concurrent callers are
            // suppressed instead of queueing a fetch each behind this one.
            *last_started = Some(now);
        }
        self.fetch_and_install(RefreshTrigger::UnknownKid).await == RefreshOutcome::Installed
    }

    /// Refresh on the configured interval until shutdown is requested.
    ///
    /// The first refresh runs immediately, so a Gateway started without
    /// configured keys obtains them as soon as the issuer answers.
    ///
    /// Until it holds a key, the Gateway cannot verify any token and reports
    /// itself unready, so a failed first fetch is retried after
    /// `unknown_kid_min_interval` instead of a whole `interval`. Container
    /// orchestrators start the Gateway and the issuer in no fixed order; an
    /// issuer that comes up a moment later no longer costs a full interval of
    /// unreadiness. Once keys are installed the ordinary schedule resumes.
    pub async fn run(self) -> anyhow::Result<()> {
        let interval = self.inner.policy.interval;
        let shutdown = platform::shutdown::global();
        if self.inner.verifier.key_count() == 0 {
            while self.refresh().await != RefreshOutcome::Installed {
                if !shutdown
                    .sleep(self.inner.policy.unknown_kid_min_interval)
                    .await
                {
                    return Ok(());
                }
            }
            // The installing fetch was this cycle's refresh; the schedule
            // continues one interval later rather than fetching again at once.
            if !shutdown.sleep(interval).await {
                return Ok(());
            }
        }
        platform::shutdown::periodic("jwt-signing-keys", interval, move || {
            let refresher = self.clone();
            async move {
                refresher.refresh().await;
                Ok::<(), anyhow::Error>(())
            }
        })
        .await
    }

    async fn fetch_and_install(&self, trigger: RefreshTrigger) -> RefreshOutcome {
        let _fetching = self.inner.fetching.lock().await;
        self.inner.fetches.fetch_add(1, Ordering::Relaxed);
        let fetched = tokio::time::timeout(FETCH_DEADLINE, self.inner.source.signing_keys()).await;
        let outcome = match fetched {
            Err(_) => {
                tracing::warn!(
                    trigger = trigger.label(),
                    "signing key fetch timed out; keeping the installed keys"
                );
                RefreshOutcome::FetchFailed
            }
            Ok(Err(error)) => {
                tracing::warn!(
                    trigger = trigger.label(),
                    error = %error,
                    "signing key fetch failed; keeping the installed keys"
                );
                RefreshOutcome::FetchFailed
            }
            Ok(Ok(document)) => match VerificationKeys::from_jwks_json(&document.jwks_json) {
                Err(error) => {
                    tracing::warn!(
                        trigger = trigger.label(),
                        error = %error,
                        "issuer published an unusable key set; keeping the installed keys"
                    );
                    RefreshOutcome::Rejected
                }
                Ok(keys) => {
                    let previous = self.inner.verifier.key_ids();
                    let installed = keys.key_ids();
                    self.inner.verifier.install(keys);
                    if previous != installed {
                        tracing::info!(
                            trigger = trigger.label(),
                            previous = ?previous,
                            installed = ?installed,
                            "installed a changed issuer key set"
                        );
                    }
                    RefreshOutcome::Installed
                }
            },
        };
        self.record(trigger, outcome);
        outcome
    }

    fn record(&self, trigger: RefreshTrigger, outcome: RefreshOutcome) {
        let Some(metrics) = &self.inner.metrics else {
            return;
        };
        metrics.inc_signing_key_refresh(trigger.label(), outcome.label());
        if outcome == RefreshOutcome::Installed {
            metrics.set_signing_keys_installed(
                self.inner.verifier.key_count(),
                chrono::Utc::now().timestamp(),
            );
        }
    }

    fn last_fetch_started(&self) -> MutexGuard<'_, Option<Instant>> {
        self.inner
            .last_fetch_started
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
    }
}
