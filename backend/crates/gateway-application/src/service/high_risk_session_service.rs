//! Confirms high-risk actions against the issuer's current session state.

use crate::dto::facade_dto::{HighRiskActionDto, SessionDecisionDto};
use crate::dto::issuer_dto::{SessionAuthorityRequestDto, SessionStateDto};
use crate::ports::{AuthClientPort, GatewayRuntimePort, SessionAuthorityPort};
use async_trait::async_trait;
use gateway_domain::high_risk::SessionState;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

/// Most sessions remembered at once. The cache only saves issuer round trips;
/// when it is full, expired entries are dropped and, failing that, all of them.
pub const MAX_CACHED_SESSIONS: usize = 4096;

/// Longest configurable freshness window. It is also the longest a logged-out
/// or demoted session can keep performing high-risk actions after the issuer
/// last confirmed it.
pub const MAX_SESSION_FRESHNESS: Duration = Duration::from_secs(30);

type Clock = Arc<dyn Fn() -> Instant + Send + Sync>;

/// Checks sessions with the issuer before high-risk actions.
///
/// A live session's answer is reused for at most `freshness`, so a session
/// that is logged out, revoked, or loses a role is refused within that bound.
/// An ended session is never remembered, and an issuer that cannot answer
/// refuses the action instead of letting the token decide.
#[derive(Clone)]
pub struct HighRiskSessionGuard {
    authority: Arc<dyn SessionAuthorityPort>,
    freshness: Duration,
    cache: Arc<Mutex<HashMap<(String, String), CachedSession>>>,
    clock: Clock,
}

#[derive(Clone)]
struct CachedSession {
    roles: Vec<String>,
    confirmed_at: Instant,
}

/// What a check decided and whether the issuer was asked.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SessionVerdict {
    pub decision: SessionDecisionDto,
    /// True when a recent confirmation was reused instead of asking the issuer.
    pub from_cache: bool,
}

impl HighRiskSessionGuard {
    /// `freshness` is capped at [`MAX_SESSION_FRESHNESS`].
    pub fn new(
        authority: Arc<dyn SessionAuthorityPort>,
        freshness: Duration,
        runtime: Arc<dyn GatewayRuntimePort>,
    ) -> Self {
        Self {
            authority,
            freshness: freshness.min(MAX_SESSION_FRESHNESS),
            cache: Arc::default(),
            clock: Arc::new(move || runtime.monotonic_now()),
        }
    }

    /// Ask the issuer through the auth client.
    pub fn over_auth_client(
        client: Arc<dyn AuthClientPort>,
        freshness: Duration,
        runtime: Arc<dyn GatewayRuntimePort>,
    ) -> Self {
        Self::new(Arc::new(AuthClientSessions(client)), freshness, runtime)
    }

    /// Replace the clock, for tests of the freshness window.
    pub fn with_clock(mut self, clock: impl Fn() -> Instant + Send + Sync + 'static) -> Self {
        self.clock = Arc::new(clock);
        self
    }

    pub fn freshness(&self) -> Duration {
        self.freshness
    }

    /// Decide whether session `session_id` of `user_id` may perform `action`.
    ///
    /// An error means the issuer could not be asked; the caller must refuse.
    pub async fn check(
        &self,
        user_id: &str,
        session_id: &str,
        action: HighRiskActionDto,
    ) -> crate::service_error::GatewayServiceResult<SessionVerdict> {
        let key = (user_id.to_owned(), session_id.to_owned());
        let now = (self.clock)();
        if let Some(roles) = self.recent(&key, now) {
            return Ok(SessionVerdict {
                decision: SessionState::Active { roles }.permits(action.into()).into(),
                from_cache: true,
            });
        }
        let request = SessionAuthorityRequestDto {
            user_id: user_id.to_owned(),
            session_id: session_id.to_owned(),
        };
        let state = SessionState::from(self.authority.session_state(&request).await?);
        match &state {
            SessionState::Active { roles } => self.remember(key, roles.clone(), now),
            SessionState::Ended => {
                self.lock().remove(&key);
            }
        }
        Ok(SessionVerdict {
            decision: state.permits(action.into()).into(),
            from_cache: false,
        })
    }

    fn recent(&self, key: &(String, String), now: Instant) -> Option<Vec<String>> {
        let cache = self.lock();
        let cached = cache.get(key)?;
        (now.saturating_duration_since(cached.confirmed_at) < self.freshness)
            .then(|| cached.roles.clone())
    }

    fn remember(&self, key: (String, String), roles: Vec<String>, now: Instant) {
        if self.freshness.is_zero() {
            return;
        }
        let mut cache = self.lock();
        if cache.len() >= MAX_CACHED_SESSIONS && !cache.contains_key(&key) {
            let freshness = self.freshness;
            cache
                .retain(|_, cached| now.saturating_duration_since(cached.confirmed_at) < freshness);
            if cache.len() >= MAX_CACHED_SESSIONS {
                cache.clear();
            }
        }
        cache.insert(
            key,
            CachedSession {
                roles,
                confirmed_at: now,
            },
        );
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<(String, String), CachedSession>> {
        self.cache.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Sessions currently remembered, for tests and diagnostics.
    pub fn cached_sessions(&self) -> usize {
        self.lock().len()
    }
}

/// Session answers from the auth client, which asks the issuer over the
/// authenticated internal channel.
struct AuthClientSessions(Arc<dyn AuthClientPort>);

#[async_trait]
impl SessionAuthorityPort for AuthClientSessions {
    async fn session_state(
        &self,
        request: &SessionAuthorityRequestDto,
    ) -> crate::client_error::GatewayClientResult<SessionStateDto> {
        self.0.session_state(request).await
    }
}
