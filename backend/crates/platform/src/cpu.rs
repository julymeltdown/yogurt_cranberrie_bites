//! Bounded CPU execution: one shared worker pool, admitted per class.
//!
//! Blocking work such as password hashing runs on a fixed number of workers.
//! Callers belong to an admission class. Every class bounds how many of its
//! requests may be running or waiting at once and refuses the rest immediately,
//! so a burst turns into fast rejections instead of an unbounded queue.
//!
//! A class may also be capped to a share of the workers. A capped class can then
//! never hold the remaining workers, which stay reserved for the uncapped
//! classes: a flood of one kind of work cannot starve another kind.

use std::fmt;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Semaphore;

/// The class has no room for another request. Callers report this as a busy
/// server with a retry hint, never as a failure of the request itself.
#[derive(Debug)]
pub struct CapacityExhausted;

impl fmt::Display for CapacityExhausted {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CPU work capacity exhausted")
    }
}

impl std::error::Error for CapacityExhausted {}

const MAX_WORKERS: usize = 64;
const MAX_QUEUE: usize = 1024;

/// Blocking workers shared by every admission class built from this pool.
#[derive(Clone)]
pub struct WorkerPool {
    workers: Arc<Semaphore>,
    size: usize,
}

impl WorkerPool {
    pub fn new(size: usize) -> anyhow::Result<Self> {
        anyhow::ensure!(
            (1..=MAX_WORKERS).contains(&size),
            "CPU concurrency must be 1-{MAX_WORKERS}"
        );
        Ok(Self {
            workers: Arc::new(Semaphore::new(size)),
            size,
        })
    }

    pub fn size(&self) -> usize {
        self.size
    }

    /// A class that may use every worker. It admits at most the pool size plus
    /// `queue_limit` requests at a time.
    pub fn class(&self, queue_limit: usize) -> anyhow::Result<AdmissionClass> {
        self.build_class(None, queue_limit)
    }

    /// A class limited to `max_workers` concurrent computations. It admits at
    /// most `max_workers + queue_limit` requests at a time.
    ///
    /// A cap equal to the pool size would reserve nothing for other classes, so
    /// it is refused.
    pub fn capped_class(
        &self,
        max_workers: usize,
        queue_limit: usize,
    ) -> anyhow::Result<AdmissionClass> {
        anyhow::ensure!(
            (1..self.size).contains(&max_workers),
            "a capped CPU class needs 1 to {} workers so at least one stays reserved",
            self.size - 1
        );
        self.build_class(Some(max_workers), queue_limit)
    }

    fn build_class(
        &self,
        cap: Option<usize>,
        queue_limit: usize,
    ) -> anyhow::Result<AdmissionClass> {
        anyhow::ensure!(
            queue_limit <= MAX_QUEUE,
            "CPU queue must not exceed {MAX_QUEUE}"
        );
        let running = cap.unwrap_or(self.size);
        Ok(AdmissionClass {
            admission: Arc::new(Semaphore::new(running + queue_limit)),
            share: cap.map(|cap| Arc::new(Semaphore::new(cap))),
            workers: self.workers.clone(),
            counters: Arc::default(),
        })
    }
}

/// Admission counts for one class since the pool was built.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct AdmissionStats {
    /// Requests accepted for execution, including any still waiting.
    pub admitted: u64,
    /// Requests refused because the class was full.
    pub rejected: u64,
}

#[derive(Default)]
struct Counters {
    admitted: AtomicU64,
    rejected: AtomicU64,
}

/// One class of work admitted to a [`WorkerPool`].
#[derive(Clone)]
pub struct AdmissionClass {
    admission: Arc<Semaphore>,
    share: Option<Arc<Semaphore>>,
    workers: Arc<Semaphore>,
    counters: Arc<Counters>,
}

impl AdmissionClass {
    /// Run `operation` on a pool worker, or refuse at once with
    /// [`CapacityExhausted`] when the class is full.
    pub async fn run<F, T>(&self, operation: F) -> anyhow::Result<T>
    where
        F: FnOnce() -> anyhow::Result<T> + Send + 'static,
        T: Send + 'static,
    {
        let Ok(admission) = self.admission.clone().try_acquire_owned() else {
            self.counters.rejected.fetch_add(1, Ordering::Relaxed);
            return Err(CapacityExhausted.into());
        };
        self.counters.admitted.fetch_add(1, Ordering::Relaxed);
        // Take the class share before a worker. A capped class therefore never
        // waits on the shared workers with more requests than its cap, and the
        // workers beyond every cap remain available to uncapped classes.
        let share = match &self.share {
            Some(share) => Some(
                share
                    .clone()
                    .acquire_owned()
                    .await
                    .map_err(|_| CapacityExhausted)?,
            ),
            None => None,
        };
        let worker = self
            .workers
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| CapacityExhausted)?;
        tokio::task::spawn_blocking(move || {
            // Cancelling the caller must not release any permit before the
            // blocking computation has actually stopped: a hash that is still
            // running still occupies its worker.
            let _permits = (admission, share, worker);
            operation()
        })
        .await?
    }

    pub fn stats(&self) -> AdmissionStats {
        AdmissionStats {
            admitted: self.counters.admitted.load(Ordering::Relaxed),
            rejected: self.counters.rejected.load(Ordering::Relaxed),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;
    use tokio::sync::oneshot;
    use tokio::time::{timeout, Duration};

    /// Start a computation that blocks until the returned sender is used.
    async fn occupy(
        class: &AdmissionClass,
    ) -> (
        mpsc::Sender<()>,
        tokio::task::JoinHandle<anyhow::Result<()>>,
    ) {
        let (started_tx, started_rx) = oneshot::channel();
        let (release_tx, release_rx) = mpsc::channel();
        let class = class.clone();
        let task = tokio::spawn(async move {
            class
                .run(move || {
                    let _ = started_tx.send(());
                    release_rx.recv()?;
                    Ok(())
                })
                .await
        });
        started_rx.await.expect("blocking computation started");
        (release_tx, task)
    }

    #[tokio::test]
    async fn returns_results_and_releases_capacity_after_an_error() {
        let class = WorkerPool::new(1).expect("pool").class(0).expect("class");
        assert_eq!(class.run(|| Ok(42)).await.expect("result"), 42);
        assert!(class
            .run::<_, ()>(|| anyhow::bail!("failed computation"))
            .await
            .is_err());
        assert_eq!(class.admission.available_permits(), 1);
        assert_eq!(
            class.stats(),
            AdmissionStats {
                admitted: 2,
                rejected: 0
            }
        );
    }

    #[tokio::test]
    async fn a_full_class_is_refused_at_once_and_counted() {
        let class = WorkerPool::new(1).expect("pool").class(0).expect("class");
        let (release, running) = occupy(&class).await;

        let error = class
            .run(|| Ok(()))
            .await
            .expect_err("a full class must refuse instead of queueing");
        assert!(error.is::<CapacityExhausted>());
        assert_eq!(class.stats().rejected, 1);

        release.send(()).expect("release");
        running.await.expect("task").expect("computation");
        assert_eq!(class.admission.available_permits(), 1);
    }

    #[tokio::test]
    async fn a_capped_class_cannot_take_the_reserved_workers() {
        // Two workers: the capped class may use one, so one stays reserved.
        let pool = WorkerPool::new(2).expect("pool");
        let capped = pool.capped_class(1, 1).expect("capped class");
        let reserved = pool.class(0).expect("uncapped class");

        // Given: the capped class's only worker is busy and a second, equally
        // long capped request is admitted and waiting for it.
        let (release_first, first) = occupy(&capped).await;
        let (second_started_tx, mut second_started_rx) = oneshot::channel();
        let (release_second, second_release_rx) = mpsc::channel::<()>();
        let queued_capped = capped.clone();
        let queued = tokio::spawn(async move {
            queued_capped
                .run(move || {
                    let _ = second_started_tx.send(());
                    second_release_rx.recv()?;
                    Ok("queued")
                })
                .await
        });
        while capped.stats().admitted < 2 {
            tokio::task::yield_now().await;
        }

        // When: uncapped work arrives.
        // Then: it runs on the reserved worker instead of waiting behind the
        // capped class. Without the cap the second capped request would hold
        // that worker, and this would not complete while both are blocked.
        let reserved_result = timeout(Duration::from_secs(5), reserved.run(|| Ok("reserved")))
            .await
            .expect("the reserved worker must be available")
            .expect("reserved work");
        assert_eq!(reserved_result, "reserved");
        assert!(
            second_started_rx.try_recv().is_err(),
            "the second capped request waits for the class share"
        );

        // And: the capped class is full (one running, one queued).
        let refused = capped
            .run(|| Ok("third"))
            .await
            .expect_err("capped class is full");
        assert!(refused.is::<CapacityExhausted>());

        release_first.send(()).expect("release");
        first.await.expect("task").expect("computation");
        second_started_rx.await.expect("second request starts");
        release_second.send(()).expect("release");
        assert_eq!(queued.await.expect("task").expect("queued work"), "queued");
        assert_eq!(
            capped.stats(),
            AdmissionStats {
                admitted: 2,
                rejected: 1
            }
        );
        assert_eq!(reserved.stats().admitted, 1);
    }

    #[tokio::test]
    async fn cancelling_the_caller_keeps_capacity_until_the_work_stops() {
        let class = WorkerPool::new(1).expect("pool").class(0).expect("class");
        let (release, running) = occupy(&class).await;

        // The caller gives up, but the blocking computation is still running.
        running.abort();
        assert!(running.await.expect_err("aborted").is_cancelled());
        assert!(class
            .run(|| Ok(()))
            .await
            .expect_err("running work still holds its worker")
            .is::<CapacityExhausted>());

        release.send(()).expect("release");
        timeout(Duration::from_secs(5), async {
            while class.admission.available_permits() != 1 {
                tokio::time::sleep(Duration::from_millis(5)).await;
            }
        })
        .await
        .expect("capacity returns once the computation stops");
    }

    #[test]
    fn rejects_invalid_bounds() {
        assert!(WorkerPool::new(0).is_err());
        assert!(WorkerPool::new(65).is_err());
        let pool = WorkerPool::new(2).expect("pool");
        assert!(pool.class(1025).is_err());
        assert!(pool.capped_class(0, 0).is_err());
        assert!(
            pool.capped_class(2, 0).is_err(),
            "a cap must reserve a worker"
        );
        assert!(pool.capped_class(1, 1024).is_ok());
    }
}
