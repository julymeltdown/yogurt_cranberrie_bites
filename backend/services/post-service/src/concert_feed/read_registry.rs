//! Read-only producer settlement. This is not owner authority: authenticated
//! canonical guard admission must precede enter, and owner release must consume
//! its guarded settlement response. Unknown/cancelled work never settles here.
use chrono::Utc;
use post_application::concert_feed::Context;
use post_domain::concert_feed::Error;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use tokio::sync::Notify;
use uuid::Uuid;

#[derive(Clone)]
pub struct ReadRegistry {
    inner: Arc<Inner>,
}
struct Inner {
    epoch: Uuid,
    capacity: usize,
    scopes: Mutex<HashMap<Uuid, Entry>>,
    changed: Notify,
}
struct Entry {
    context: Context,
    active: usize,
    uncertain: bool,
    sealed: bool,
}
pub struct ReadPermit {
    inner: Arc<Inner>,
    guard: Uuid,
    finished: bool,
}
/// Local terminal evidence only. RPC callers must authenticate its origin and
/// bind its process epoch to the producer observed before any guarded work.
pub struct SettledRead {
    epoch: Uuid,
    guard: Uuid,
    context: Context,
}
impl SettledRead {
    pub fn process_epoch(&self) -> Uuid {
        self.epoch
    }
    pub fn guard_id(&self) -> Uuid {
        self.guard
    }
    pub fn context(&self) -> &Context {
        &self.context
    }
}
impl ReadRegistry {
    pub fn new(capacity: usize) -> Result<Self, Error> {
        if !(1..=4096).contains(&capacity) {
            return Err(Error::Invalid);
        }
        Ok(Self {
            inner: Arc::new(Inner {
                epoch: Uuid::new_v4(),
                capacity,
                scopes: Mutex::new(HashMap::new()),
                changed: Notify::new(),
            }),
        })
    }
    pub fn process_epoch(&self) -> Uuid {
        self.inner.epoch
    }
    /// Call only after a sealed canonical child has been checked for this exact
    /// guard/context/method/body. Context/digest alone cannot grant a read.
    pub fn enter(&self, guard: Uuid, context: &Context) -> Result<ReadPermit, Error> {
        context.validate(Utc::now())?;
        if guard.is_nil() {
            return Err(Error::Invalid);
        }
        let mut scopes = self.inner.scopes.lock().map_err(|_| Error::Unavailable)?;
        // Completed tombstones outlive all valid original child deadlines. An
        // uncertain guard is never evicted to manufacture apparent settlement.
        let now = Utc::now();
        scopes.retain(|_, e| {
            !(e.sealed && !e.uncertain && e.active == 0 && e.context.expires_at <= now)
        });
        if !scopes.contains_key(&guard) {
            if scopes.len() >= self.inner.capacity {
                return Err(Error::Unavailable);
            }
            scopes.insert(
                guard,
                Entry {
                    context: context.clone(),
                    active: 0,
                    uncertain: false,
                    sealed: false,
                },
            );
        }
        let entry = scopes.get_mut(&guard).ok_or(Error::Unavailable)?;
        if entry.context != *context || entry.sealed || entry.uncertain {
            return Err(Error::Authority);
        }
        if entry.active >= 128 {
            return Err(Error::Unavailable);
        }
        entry.active += 1;
        Ok(ReadPermit {
            inner: self.inner.clone(),
            guard,
            finished: false,
        })
    }
    /// Atomically seal admission before draining active work. The expected epoch
    /// must come from the authenticated producer before its first operation.
    /// A restart cannot manufacture proof about the previous process's work.
    pub async fn seal(
        &self,
        expected_epoch: Uuid,
        guard: Uuid,
        context: &Context,
        stop: tokio::time::Instant,
    ) -> Result<SettledRead, Error> {
        if expected_epoch != self.inner.epoch || guard.is_nil() {
            return Err(Error::Authority);
        }
        {
            let mut scopes = self.inner.scopes.lock().map_err(|_| Error::Unavailable)?;
            // An absent scope is deliberately not positive terminal evidence.
            let entry = scopes.get_mut(&guard).ok_or(Error::Authority)?;
            if entry.context != *context {
                return Err(Error::Authority);
            }
            entry.sealed = true;
        }
        loop {
            let changed = self.inner.changed.notified();
            tokio::pin!(changed);
            changed.as_mut().enable();
            {
                let scopes = self.inner.scopes.lock().map_err(|_| Error::Unavailable)?;
                let entry = scopes.get(&guard).ok_or(Error::Authority)?;
                if entry.context != *context || entry.uncertain {
                    return Err(Error::Authority);
                }
                if entry.active == 0 {
                    return Ok(SettledRead {
                        epoch: self.inner.epoch,
                        guard,
                        context: context.clone(),
                    });
                }
            }
            tokio::time::timeout_at(stop, changed)
                .await
                .map_err(|_| Error::Unavailable)?;
        }
    }
}
impl ReadPermit {
    /// Only after actual checked_commit and response DTO assembly have returned.
    /// Error paths and dropped SQL transactions cannot call this as rollback proof.
    pub fn finish(mut self) -> Result<(), Error> {
        {
            let mut scopes = self.inner.scopes.lock().map_err(|_| Error::Unavailable)?;
            let entry = scopes.get_mut(&self.guard).ok_or(Error::Authority)?;
            entry.active = entry.active.checked_sub(1).ok_or(Error::Unavailable)?;
            self.finished = true;
        }
        self.inner.changed.notify_waiters();
        Ok(())
    }
}
impl Drop for ReadPermit {
    fn drop(&mut self) {
        if !self.finished {
            if let Ok(mut scopes) = self.inner.scopes.lock() {
                if let Some(entry) = scopes.get_mut(&self.guard) {
                    entry.uncertain = true;
                    entry.active = entry.active.saturating_sub(1);
                }
            }
            self.inner.changed.notify_waiters();
        }
    }
}
