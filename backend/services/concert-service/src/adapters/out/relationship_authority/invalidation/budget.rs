use super::ReadError;
use std::{
    sync::Arc,
    time::{Duration, Instant},
};

pub const MAX_OPERATION: Duration = Duration::from_secs(1);
#[derive(Clone)]
pub struct ReplayCancellation(Arc<tokio::sync::watch::Sender<bool>>);
impl Default for ReplayCancellation {
    fn default() -> Self {
        let (send, _) = tokio::sync::watch::channel(false);
        Self(Arc::new(send))
    }
}
impl ReplayCancellation {
    pub fn cancel(&self) {
        self.0.send_replace(true);
    }
    pub fn is_cancelled(&self) -> bool {
        *self.0.borrow()
    }
    pub(super) async fn cancelled(&self) {
        let mut rx = self.0.subscribe();
        loop {
            if *rx.borrow() {
                return;
            }
            if rx.changed().await.is_err() {
                return;
            }
        }
    }
}
#[derive(Clone)]
pub struct ReplayBudget {
    stop: Instant,
    cancel: ReplayCancellation,
}
impl ReplayBudget {
    pub fn new(stop: Instant, cancel: ReplayCancellation) -> Result<Self, ReadError> {
        let now = Instant::now();
        if stop <= now {
            return Err(ReadError::DeadlineExceeded);
        }
        if stop.saturating_duration_since(now) > MAX_OPERATION {
            return Err(ReadError::InvalidInput);
        }
        let stop = platform::call_deadline::current().map_or(stop, |ambient| ambient.min(stop));
        let result = Self { stop, cancel };
        result.check(stop)?;
        Ok(result)
    }
    pub fn for_duration(duration: Duration, cancel: ReplayCancellation) -> Result<Self, ReadError> {
        if duration.is_zero() || duration > MAX_OPERATION {
            return Err(ReadError::InvalidInput);
        }
        Self::new(Instant::now() + duration, cancel)
    }
    pub fn deadline(&self) -> Instant {
        self.stop
    }
    pub(super) fn operation_stop(&self) -> Result<Instant, ReadError> {
        let stop =
            platform::call_deadline::current().map_or(self.stop, |ambient| ambient.min(self.stop));
        self.check(stop)?;
        Ok(stop)
    }
    pub(super) fn check(&self, stop: Instant) -> Result<(), ReadError> {
        if self.cancel.is_cancelled() {
            return Err(ReadError::Cancelled);
        }
        if Instant::now() >= stop {
            return Err(ReadError::DeadlineExceeded);
        }
        Ok(())
    }
    pub(super) async fn run<T, F>(&self, stop: Instant, future: F) -> Result<T, ReadError>
    where
        F: std::future::Future<Output = Result<T, ReadError>>,
    {
        self.check(stop)?;
        let result=platform::call_deadline::scope(stop,async {
            tokio::select! {biased;
                _=self.cancel.cancelled()=>Err(ReadError::Cancelled),
                result=tokio::time::timeout_at(stop.into(),future)=>result.map_err(|_|ReadError::DeadlineExceeded)?,
            }
        }).await;
        self.check(stop)?;
        result
    }
}
