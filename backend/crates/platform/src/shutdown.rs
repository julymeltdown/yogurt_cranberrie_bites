//! Process-wide stop admission, cycle-boundary worker cancellation and draining.

use std::future::Future;
use std::sync::OnceLock;
use std::time::Duration;
use tokio::sync::watch;
use tokio::time::Interval;
use tracing::{field, Instrument};

#[derive(Clone)]
pub struct Shutdown {
    sender: watch::Sender<bool>,
}

impl Default for Shutdown {
    fn default() -> Self {
        Self::new()
    }
}

impl Shutdown {
    pub fn new() -> Self {
        let (sender, _) = watch::channel(false);
        Self { sender }
    }

    pub fn request(&self) {
        self.sender.send_replace(true);
    }
    pub fn is_requested(&self) -> bool {
        *self.sender.borrow()
    }

    pub fn cancelled(&self) -> impl Future<Output = ()> + Send + 'static {
        let mut receiver = self.sender.subscribe();
        async move {
            if *receiver.borrow_and_update() {
                return;
            }
            while receiver.changed().await.is_ok() {
                if *receiver.borrow_and_update() {
                    return;
                }
            }
        }
    }

    pub async fn tick(&self, interval: &mut Interval) -> bool {
        tokio::select! {
            biased;
            _ = self.cancelled() => false,
            _ = interval.tick() => !self.is_requested(),
        }
    }

    pub async fn sleep(&self, duration: Duration) -> bool {
        tokio::select! {
            biased;
            _ = self.cancelled() => false,
            _ = tokio::time::sleep(duration) => !self.is_requested(),
        }
    }

    pub async fn periodic<F, Fut, T>(
        &self,
        name: &'static str,
        interval: Duration,
        mut cycle: F,
    ) -> anyhow::Result<()>
    where
        F: FnMut() -> Fut,
        Fut: Future<Output = anyhow::Result<T>>,
    {
        anyhow::ensure!(!interval.is_zero(), "Worker interval must be positive");
        let mut ticker = tokio::time::interval(interval);
        ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        while self.tick(&mut ticker).await {
            // Deliberately do not race cancellation against the current cycle.
            // Its transaction/checkpoint finishes before another cycle is denied.
            let span = tracing::info_span!(
                "background_worker_cycle",
                worker = name,
                outcome = field::Empty,
            );
            let cycle_span = span.clone();
            async {
                if let Err(error) = cycle().await {
                    cycle_span.record("outcome", "failure");
                    tracing::error!(worker = name, error = ?error, "worker cycle failed");
                } else {
                    cycle_span.record("outcome", "success");
                    tracing::debug!(worker = name, "worker cycle completed");
                }
            }
            .instrument(span)
            .await;
        }
        Ok(())
    }
}

pub fn global() -> Shutdown {
    static SHUTDOWN: OnceLock<Shutdown> = OnceLock::new();
    SHUTDOWN.get_or_init(Shutdown::new).clone()
}

pub async fn periodic<F, Fut, T>(
    name: &'static str,
    interval: Duration,
    cycle: F,
) -> anyhow::Result<()>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = anyhow::Result<T>>,
{
    global().periodic(name, interval, cycle).await
}

pub async fn idle() -> anyhow::Result<()> {
    global().cancelled().await;
    Ok(())
}

pub fn install_signal_handlers() -> anyhow::Result<()> {
    let shutdown = global();
    #[cfg(unix)]
    {
        use tokio::signal::unix::{signal, SignalKind};
        let mut terminate = signal(SignalKind::terminate())?;
        let mut interrupt = signal(SignalKind::interrupt())?;
        tokio::spawn(async move {
            tokio::select! { _ = terminate.recv() => {}, _ = interrupt.recv() => {} }
            shutdown.request();
        });
    }
    #[cfg(not(unix))]
    tokio::spawn(async move {
        let _ = tokio::signal::ctrl_c().await;
        shutdown.request();
    });
    Ok(())
}

pub async fn supervise<F, T>(operation: F) -> anyhow::Result<T>
where
    F: Future<Output = anyhow::Result<T>>,
{
    let shutdown = global();
    tokio::pin!(operation);
    tokio::select! {
        result = &mut operation => result,
        _ = shutdown.cancelled() => {
            tokio::time::timeout(Duration::from_secs(30), operation).await
                .map_err(|_| anyhow::anyhow!("Graceful shutdown deadline exceeded"))?
        }
    }
}

/// Dependency failure is readiness failure, never a reason to fail liveness.
pub async fn is_ready<F, E>(probe: F) -> bool
where
    F: Future<Output = Result<(), E>>,
{
    let shutdown = global();
    if shutdown.is_requested() {
        return false;
    }
    matches!(
        tokio::time::timeout(Duration::from_millis(250), probe).await,
        Ok(Ok(()))
    ) && !shutdown.is_requested()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};

    #[tokio::test]
    async fn cancellation_is_observed_by_existing_and_late_waiters() {
        let shutdown = Shutdown::new();
        let first = shutdown.cancelled();
        shutdown.request();
        tokio::time::timeout(Duration::from_secs(1), first)
            .await
            .expect("first waiter");
        tokio::time::timeout(Duration::from_secs(1), shutdown.cancelled())
            .await
            .expect("late waiter");
        shutdown.request();
        assert!(shutdown.is_requested());
    }

    #[tokio::test]
    async fn draining_prevents_another_worker_cycle() {
        let shutdown = Shutdown::new();
        let mut interval = tokio::time::interval(Duration::from_secs(3600));
        shutdown.request();
        assert!(!shutdown.tick(&mut interval).await);
        assert!(!shutdown.sleep(Duration::from_secs(3600)).await);
    }

    #[tokio::test]
    async fn an_inflight_cycle_finishes_before_shutdown_returns() {
        let shutdown = Shutdown::new();
        let completed = AtomicUsize::new(0);
        shutdown
            .periodic("test", Duration::from_millis(1), || async {
                shutdown.request();
                tokio::time::sleep(Duration::from_millis(5)).await;
                completed.fetch_add(1, Ordering::SeqCst);
                Ok(())
            })
            .await
            .expect("drained worker");
        assert_eq!(completed.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn dependency_failure_is_not_ready() {
        assert!(!is_ready(async { Err::<(), _>("database unavailable") }).await);
        assert!(is_ready(async { Ok::<(), ()>(()) }).await);
    }
}
