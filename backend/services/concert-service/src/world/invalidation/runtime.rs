//! Supervised Auth/Friend replay. Signals only suppress existing authority.
use super::{
    checkpoint,
    remote::{RemoteReadiness, MAX_BRIDGES, POLL},
    AdmissionBridge, ApplyError, Removal, Scope,
};
use crate::adapters::out::relationship_authority::invalidation::{
    InvalidationOwner, InvalidationSource, OwnerCursor, ReplayBudget, ReplayCancellation,
    VerifiedOwnerPage,
};
use sqlx::PgPool;
use std::{
    collections::BTreeSet,
    sync::Arc,
    time::{Duration, Instant},
};
use uuid::Uuid;

pub(crate) struct RemoteInvalidationRuntime {
    pool: PgPool,
    consumer: Uuid,
    run: Uuid,
    marker: Uuid,
    sources: [InvalidationSource; 2],
    cursors: [Option<OwnerCursor>; 2],
    bridges: Vec<Arc<AdmissionBridge>>,
    readiness: Arc<RemoteReadiness>,
    cancel: ReplayCancellation,
    booted: bool,
}
impl RemoteInvalidationRuntime {
    pub(crate) fn new(
        pool: PgPool,
        consumer: Uuid,
        auth: InvalidationSource,
        friend: InvalidationSource,
        bridges: Vec<Arc<AdmissionBridge>>,
    ) -> Result<Self, ApplyError> {
        if consumer.is_nil()
            || bridges.is_empty()
            || bridges.len() > MAX_BRIDGES
            || auth.owner() != InvalidationOwner::Auth
            || friend.owner() != InvalidationOwner::Friend
        {
            return Err(ApplyError::UnqualifiedSource);
        }
        let mut events = BTreeSet::new();
        for bridge in &bridges {
            let (_, owner, _, event) = bridge.invalidation_context();
            if owner != consumer || !events.insert(event) {
                return Err(ApplyError::Unbound);
            }
        }
        let readiness = Arc::new(RemoteReadiness::new());
        for bridge in &bridges {
            bridge
                .bind_remote(readiness.clone())
                .map_err(|_| ApplyError::Unavailable)?;
        }
        Ok(Self {
            pool,
            consumer,
            run: Uuid::new_v4(),
            marker: Uuid::new_v4(),
            sources: [auth, friend],
            cursors: [None, None],
            bridges,
            readiness,
            cancel: ReplayCancellation::default(),
            booted: false,
        })
    }
    fn fence_all(&self) -> Result<(), ApplyError> {
        self.readiness.close()?;
        for bridge in &self.bridges {
            let (_, _, _, event) = bridge.invalidation_context();
            bridge
                .begin_invalidation(self.marker, Removal::Scope(Scope::Event(event)))
                .map_err(|_| ApplyError::Unavailable)?;
        }
        Ok(())
    }
    fn bindings(&self) -> serde_json::Value {
        serde_json::Value::Array(
            self.bridges
                .iter()
                .map(|bridge| {
                    let (_, _, epoch, event) = bridge.invalidation_context();
                    serde_json::json!({"event":event,"epoch":epoch})
                })
                .collect(),
        )
    }
    fn budget(&self) -> Result<ReplayBudget, ApplyError> {
        ReplayBudget::for_duration(Duration::from_secs(1), self.cancel.clone())
            .map_err(|_| ApplyError::Unavailable)
    }
    /// Explicit full invalidating bootstrap. No old cache survives a head read.
    /// With no retained publications, new authority still requires admission's
    /// actual current Auth/Friend/Profile/Concert reads after catch-up.
    pub(crate) async fn bootstrap(&mut self) -> Result<(), ApplyError> {
        self.booted = false;
        self.fence_all()?;
        let budget = self.budget()?;
        let initial = async {
            let mut gates = Vec::new();
            for bridge in &self.bridges {
                gates.push(bridge.invalidation_admission_gate().lock().await);
            }
            for bridge in &self.bridges {
                bridge
                    .pause_invalidation()
                    .await
                    .map_err(|_| ApplyError::Unavailable)?;
            }
            let events: Vec<_> = self
                .bridges
                .iter()
                .map(|b| b.invalidation_context().3)
                .collect();
            checkpoint::boot(&self.pool, self.consumer, self.run, &events).await?;
            for bridge in &self.bridges {
                bridge
                    .reset_remote_removals()
                    .map_err(|_| ApplyError::Unavailable)?;
            }
            drop(gates);
            // Heads are independent source evidence. Neither can substitute for
            // the other, and no call gets a fresh budget after channel waiting.
            for i in 0..2 {
                let head = self.sources[i]
                    .read_head(self.consumer, &budget)
                    .await
                    .map_err(|error| {
                        eprintln!(
                            "World replay bootstrap unavailable: {:?} from source {} as consumer {}",
                            error, i, self.consumer
                        );
                        match error {
                            crate::adapters::out::relationship_authority::invalidation::ReadError::Unauthenticated
                            | crate::adapters::out::relationship_authority::invalidation::ReadError::PermissionDenied
                            | crate::adapters::out::relationship_authority::invalidation::ReadError::InvalidConfiguration => {
                                ApplyError::UnqualifiedSource
                            }
                            _ => ApplyError::Unavailable,
                        }
                    })?;
                self.cursors[i] =
                    Some(checkpoint::baseline(&self.pool, self.consumer, self.run, &head).await?);
            }
            Ok::<(), ApplyError>(())
        };
        let result = tokio::time::timeout_at(budget.deadline().into(), initial)
            .await
            .map_err(|_| ApplyError::Unavailable)
            .and_then(|r| r);
        if let Err(e) = result {
            self.fence_all()?;
            return Err(e);
        }
        // Each operation applies at most one bounded page per owner. Backlog is
        // paged immediately; it never enables admission or native publication.
        loop {
            match self.poll_inner().await {
                Ok(true) => break,
                Ok(false) => tokio::task::yield_now().await,
                Err(e) => {
                    self.fence_all()?;
                    return Err(e);
                }
            }
        }
        for bridge in &self.bridges {
            bridge
                .finish_invalidation(self.marker)
                .map_err(|_| ApplyError::Unavailable)?;
        }
        if self.readiness.snapshot().is_none() {
            self.fence_all()?;
            return Err(ApplyError::Unavailable);
        }
        self.booted = true;
        Ok(())
    }
    async fn apply_page(
        &self,
        expected: OwnerCursor,
        page: &VerifiedOwnerPage,
        budget: &ReplayBudget,
    ) -> Result<OwnerCursor, ApplyError> {
        // The sealed page is already known before waiting for local delivery
        // serialization or SQL. Suppress new handoffs immediately on knowledge.
        if !page.signals().is_empty() || page.has_more() {
            self.readiness.close()?;
        }
        let mut deliveries = Vec::new();
        for bridge in &self.bridges {
            deliveries.push(bridge.invalidation_delivery_gate().lock().await);
        }
        let prepared =
            checkpoint::prepare(&self.pool, self.consumer, self.run, expected, page).await?;
        let ids: Vec<_> = prepared
            .signals
            .iter()
            .map(|s| s.fence_id(self.consumer))
            .collect();
        for (i, signal) in prepared.signals.iter().enumerate() {
            let removal = match &prepared.previous {
                Some(old) => Removal::RemoteVictims(old[i].clone()),
                None => Removal::RemoteScope(signal.scope.clone()),
            };
            for bridge in &self.bridges {
                bridge
                    .begin_invalidation_if(ids[i], removal.clone(), || {
                        Instant::now() < budget.deadline() && !self.cancel.is_cancelled()
                    })
                    .map_err(|_| ApplyError::Unavailable)?;
            }
        }
        if ids.is_empty() {
            // Empty caught-up reads only validate/heartbeat the checkpoint. They
            // must not wait behind unrelated typed membership refresh I/O.
            return prepared.commit(self.bindings()).await;
        }
        let mut admissions = Vec::new();
        for bridge in &self.bridges {
            admissions.push(bridge.invalidation_admission_gate().lock().await);
        }
        if !ids.is_empty() {
            for bridge in &self.bridges {
                bridge
                    .pause_invalidation()
                    .await
                    .map_err(|_| ApplyError::Unavailable)?;
            }
        }
        if Instant::now() >= budget.deadline() || self.cancel.is_cancelled() {
            return Err(ApplyError::Unavailable);
        }
        let next = prepared.commit(self.bindings()).await?;
        // Commit failure/cancellation leaves removal markers in place. Success
        // only clears markers: it reinstalls no membership, payload or lease.
        for id in ids {
            for bridge in &self.bridges {
                bridge
                    .finish_invalidation(id)
                    .map_err(|_| ApplyError::Unavailable)?;
            }
        }
        Ok(next)
    }
    async fn poll_inner(&mut self) -> Result<bool, ApplyError> {
        let budget = self.budget()?;
        let operation = async {
            let mut caught = true;
            let mut checked = Instant::now();
            for i in 0..2 {
                let after = self.cursors[i].ok_or(ApplyError::Unbound)?;
                let page = self.sources[i]
                    .read_after(self.consumer, after, 256, &budget)
                    .await
                    .map_err(|_| ApplyError::UnqualifiedSource)?;
                let received = Instant::now();
                if i == 0 {
                    checked = received;
                }
                caught &= !page.has_more();
                self.cursors[i] = Some(self.apply_page(after, &page, &budget).await?);
            }
            if caught {
                self.readiness.open(checked)?;
            } else {
                self.readiness.close()?;
            }
            Ok(caught)
        };
        tokio::time::timeout_at(budget.deadline().into(), operation)
            .await
            .map_err(|_| ApplyError::Unavailable)?
    }
    pub(crate) async fn poll(&mut self) -> Result<bool, ApplyError> {
        if !self.booted {
            return Err(ApplyError::Unavailable);
        }
        match self.poll_inner().await {
            Ok(caught) => Ok(caught),
            Err(e) => {
                self.booted = false;
                self.fence_all()?;
                Err(e)
            }
        }
    }
    pub(crate) async fn run(&mut self) -> Result<(), ApplyError> {
        if !self.booted {
            return Err(ApplyError::Unavailable);
        }
        let mut tick = tokio::time::interval(POLL);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            tokio::select! {biased;
                _=platform::shutdown::global().cancelled()=>return Ok(()),
                _=tick.tick()=> {
                    while !self.poll().await? { tokio::task::yield_now().await; }
                }
            }
        }
    }
    pub(crate) async fn shutdown(&mut self) -> Result<(), ApplyError> {
        self.cancel.cancel();
        self.booted = false;
        self.fence_all()?;
        for bridge in &self.bridges {
            bridge
                .pause_invalidation()
                .await
                .map_err(|_| ApplyError::Unavailable)?;
        }
        checkpoint::stop(&self.pool, self.consumer, self.run).await
    }
}
impl Drop for RemoteInvalidationRuntime {
    fn drop(&mut self) {
        self.cancel.cancel();
        let _ = self.fence_all();
    }
}

#[cfg(test)]
#[path = "../../../tests/world_remote_invalidation_support/qualified.rs"]
mod tests;
