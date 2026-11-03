use super::ReadError;
use std::{
    fmt,
    future::Future,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};
#[derive(Clone, Default)]
pub struct ReadCancellation(Arc<AtomicBool>);
impl ReadCancellation {
    pub fn cancel(&self) {
        self.0.store(true, Ordering::Release);
    }
    pub fn is_cancelled(&self) -> bool {
        self.0.load(Ordering::Acquire)
    }
}
impl fmt::Debug for ReadCancellation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReadCancellation([REDACTED])")
    }
}
#[derive(Clone)]
pub struct ReadBudget {
    stop: Instant,
    cancellation: ReadCancellation,
}
impl ReadBudget {
    pub const MAX_DURATION: Duration = Duration::from_secs(1);
    /// Import the operation's existing absolute stop; never reset on retry/page.
    pub fn new(stop: Instant, cancellation: ReadCancellation) -> Result<Self, ReadError> {
        if stop.saturating_duration_since(Instant::now()) > Self::MAX_DURATION {
            return Err(ReadError::InvalidInput);
        }
        let stop = platform::call_deadline::current().map_or(stop, |parent| stop.min(parent));
        let budget = Self { stop, cancellation };
        budget.check()?;
        Ok(budget)
    }
    pub fn deadline(&self) -> Instant {
        self.stop
    }
    pub fn cancellation(&self) -> ReadCancellation {
        self.cancellation.clone()
    }
    pub fn check(&self) -> Result<(), ReadError> {
        if self.cancellation.is_cancelled() {
            Err(ReadError::Cancelled)
        } else if Instant::now() >= self.stop
            || platform::call_deadline::current().is_some_and(|p| Instant::now() >= p)
        {
            Err(ReadError::DeadlineExceeded)
        } else {
            Ok(())
        }
    }
    pub fn remaining(&self) -> Result<Duration, ReadError> {
        self.check()?;
        let stop = platform::call_deadline::current().map_or(self.stop, |p| self.stop.min(p));
        let remaining = stop.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            Err(ReadError::DeadlineExceeded)
        } else {
            Ok(remaining)
        }
    }
    pub(crate) async fn run<T>(
        &self,
        future: impl Future<Output = Result<T, ReadError>>,
    ) -> Result<T, ReadError> {
        self.check()?;
        let stop = platform::call_deadline::current().map_or(self.stop, |p| self.stop.min(p));
        let cancelled = async {
            loop {
                if self.cancellation.is_cancelled() {
                    break;
                }
                tokio::time::sleep(Duration::from_millis(2)).await;
            }
        };
        let result = tokio::select! {biased;
            _=cancelled=>Err(ReadError::Cancelled),
            _=tokio::time::sleep_until(tokio::time::Instant::from_std(stop))=>Err(ReadError::DeadlineExceeded),
            result=future=>result,
        };
        self.check()?;
        result
    }
}
impl fmt::Debug for ReadBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ReadBudget([REDACTED])")
    }
}
