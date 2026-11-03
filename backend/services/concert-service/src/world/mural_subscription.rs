//! Pull-driven bounded mural stream on the incumbent world connection, not a transport.
//! No wire events are invented. Composition maps these values to approved contracts.
use concert_application::mural::{MuralActor, MuralError, MuralReader};
use concert_domain::mural::{MuralDelta, MuralSnapshot};
use std::collections::VecDeque;
use uuid::Uuid;

#[derive(Clone, Debug)]
pub enum MuralDelivery {
    Snapshot { value: MuralSnapshot, reset: bool },
    Delta(MuralDelta),
}
impl MuralDelivery {
    fn revision(&self) -> i64 {
        match self {
            Self::Snapshot { value, .. } => value.revision,
            Self::Delta(value) => value.to_revision,
        }
    }
}
pub struct MuralSubscription {
    actor: MuralActor,
    epoch: Uuid,
    delivered: Option<i64>,
    queue: VecDeque<MuralDelivery>,
    capacity: usize,
    reset: bool,
    high_water: i64,
}
impl MuralSubscription {
    pub fn new(actor: MuralActor, epoch: Uuid, capacity: usize) -> Result<Self, MuralError> {
        if !actor.valid() || epoch.is_nil() || !(1..=16).contains(&capacity) {
            return Err(MuralError::InvalidInput);
        }
        Ok(Self {
            actor,
            epoch,
            delivered: None,
            queue: VecDeque::new(),
            capacity,
            reset: false,
            high_water: 1,
        })
    }
    pub fn queued(&self) -> usize {
        self.queue.len()
    }
    pub fn delivered_revision(&self) -> Option<i64> {
        self.delivered
    }
    /// New connection/process epoch is server-generated and never accepted from a client.
    fn fence_epoch(&mut self, epoch: Uuid) -> Result<(), MuralError> {
        if epoch.is_nil() {
            self.invalidate();
            return Err(MuralError::InvalidInput);
        }
        if self.epoch != epoch {
            self.epoch = epoch;
            self.invalidate();
        }
        Ok(())
    }
    pub fn invalidate(&mut self) {
        self.queue.clear();
        self.delivered = None;
        self.reset = true;
    }
    /// Loads at most one delta or replacement snapshot. Slow consumers lose obsolete
    /// queued updates and resnapshot rather than allocate an unbounded operation queue.
    pub async fn poll<R: MuralReader>(
        &mut self,
        source: &R,
        epoch: Uuid,
    ) -> Result<(), MuralError> {
        self.fence_epoch(epoch)?;
        if let Err(error) = self.poll_in(source).await {
            self.invalidate();
            return Err(error);
        }
        Ok(())
    }
    async fn poll_in<R: MuralReader>(&mut self, source: &R) -> Result<(), MuralError> {
        source.authorize(self.actor).await?;
        let after = self
            .queue
            .back()
            .map(MuralDelivery::revision)
            .or(self.delivered);
        if let Some(after) = after {
            let delta = source.delta(self.actor, after).await?;
            if delta.event_id != self.actor.event
                || delta.from_revision != after
                || !delta.contiguous()
            {
                self.invalidate();
            } else if !delta.operations.is_empty() {
                if self.queue.len() >= self.capacity {
                    self.invalidate();
                } else {
                    self.queue.push_back(MuralDelivery::Delta(delta));
                    return Ok(());
                }
            } else {
                return Ok(());
            }
        }
        let snapshot = source.snapshot(self.actor).await?;
        if !snapshot.valid()
            || snapshot.event_id != self.actor.event
            || snapshot.revision < self.high_water
        {
            return Err(MuralError::InvalidState);
        }
        self.queue.push_back(MuralDelivery::Snapshot {
            value: snapshot,
            reset: self.reset,
        });
        Ok(())
    }
    /// Reauthorize before EVERY dequeue, including frames queued before a revocation.
    /// Call only when incumbent connection accepts the frame; transport failure must
    /// invalidate/reconnect, never assume an undelivered frame was acknowledged.
    pub async fn next<R: MuralReader>(
        &mut self,
        source: &R,
        epoch: Uuid,
    ) -> Result<Option<MuralDelivery>, MuralError> {
        self.fence_epoch(epoch)?;
        if let Err(error) = source.authorize(self.actor).await {
            self.invalidate();
            return Err(error);
        }
        if let Some(queued) = self.queue.front() {
            let check = match source.delta(self.actor, queued.revision()).await {
                Ok(value) => value,
                Err(error) => {
                    self.invalidate();
                    return Err(error);
                }
            };
            if check.event_id != self.actor.event
                || check.from_revision != queued.revision()
                || !check.contiguous()
            {
                self.invalidate();
                self.poll(source, epoch).await?;
            }
        }
        let next = self.queue.pop_front();
        if let Some(value) = &next {
            self.delivered = Some(value.revision());
            self.high_water = self.high_water.max(value.revision());
        }
        Ok(next)
    }
}
