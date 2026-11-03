//! Per-edge in-flight ceilings for internal gRPC calls.
//!
//! Every edge call holds one permit for as long as its future is alive. The
//! permit is released when the call finishes, fails, times out or is dropped by
//! a cancelled caller, so cancellation returns capacity without extra cleanup.
//! A call that finds no permit is refused immediately: waiting would only move
//! the unbounded queue from the peer into this process.

use crate::edge::Edge;
use std::collections::HashMap;
use std::sync::{Arc, OnceLock};
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// One semaphore per declared edge.
pub struct EdgeLimiter {
    permits: HashMap<Edge, Arc<Semaphore>>,
}

impl EdgeLimiter {
    /// A limiter with `limit(edge)` permits for every declared edge.
    pub fn new(limit: impl Fn(Edge) -> usize) -> Self {
        Self {
            permits: Edge::ALL
                .into_iter()
                .map(|edge| (edge, Arc::new(Semaphore::new(limit(edge)))))
                .collect(),
        }
    }

    /// The process-wide limiter, sized by [Edge::max_in_flight].
    pub fn global() -> &'static EdgeLimiter {
        static GLOBAL: OnceLock<EdgeLimiter> = OnceLock::new();
        GLOBAL.get_or_init(|| EdgeLimiter::new(Edge::max_in_flight))
    }

    /// A separate pool for calls made while serving another workload's
    /// in-flight request on the same edge, such as the Gateway's owner-evidence
    /// lookup to Post during an exchange that Post requested while handling a
    /// Gateway call. Outer calls holding every [EdgeLimiter::global] permit
    /// cannot then refuse the callbacks they are waiting for. The pool has the
    /// same per-edge ceiling, so callbacks stay bounded too.
    pub fn callbacks() -> &'static EdgeLimiter {
        static CALLBACKS: OnceLock<EdgeLimiter> = OnceLock::new();
        CALLBACKS.get_or_init(|| EdgeLimiter::new(Edge::max_in_flight))
    }

    /// A permit if the edge is below its ceiling, without waiting.
    pub fn try_acquire(&self, edge: Edge) -> Option<OwnedSemaphorePermit> {
        self.permits[&edge].clone().try_acquire_owned().ok()
    }

    /// Permits not currently held on this edge.
    pub fn available(&self, edge: Edge) -> usize {
        self.permits[&edge].available_permits()
    }
}

#[cfg(test)]
mod tests {
    use super::EdgeLimiter;
    use crate::edge::Edge;

    #[test]
    fn every_declared_edge_has_a_positive_ceiling() {
        for edge in Edge::ALL {
            assert!(edge.max_in_flight() > 0, "{edge:?}");
            assert!(EdgeLimiter::global().available(edge) <= edge.max_in_flight());
        }
    }

    #[test]
    fn a_full_edge_refuses_and_a_released_permit_is_reusable() {
        let limiter = EdgeLimiter::new(|_| 2);
        let first = limiter.try_acquire(Edge::PostFriend).expect("first");
        let _second = limiter.try_acquire(Edge::PostFriend).expect("second");
        assert!(limiter.try_acquire(Edge::PostFriend).is_none());
        assert!(
            limiter.try_acquire(Edge::PostProfile).is_some(),
            "edges are independent"
        );
        drop(first);
        assert!(limiter.try_acquire(Edge::PostFriend).is_some());
    }
}
