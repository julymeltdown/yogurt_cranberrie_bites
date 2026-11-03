//! Bounded Argon2 password-work adapter.
//!
//! Argon2 is deliberately expensive. All work runs on one bounded blocking
//! pool. Signup and reset have capped classes, preserving capacity for login.
//! Saturation maps to `AuthError::Busy`; it is never reported as a credential
//! failure and does not consume a reset token because reset hashing occurs
//! before the atomic reset transaction.

use crate::application::errors::AuthError;
use crate::application::password_work::{
    PasswordWorkKind, PasswordWorkLimits, PasswordWorkPort, PasswordWorkStats,
};
use argon2::{Argon2, PasswordHasher, PasswordVerifier};
use async_trait::async_trait;
use platform::cpu::{AdmissionClass, CapacityExhausted, WorkerPool};
use rand::rngs::OsRng;
use std::sync::OnceLock;

const MAX_VERIFIED_PASSWORD_BYTES: usize = 1024;

#[derive(Clone)]
pub struct AdmittedArgon2PasswordWork {
    login: AdmissionClass,
    signup: AdmissionClass,
    reset: AdmissionClass,
}

impl AdmittedArgon2PasswordWork {
    pub fn new(limits: PasswordWorkLimits) -> anyhow::Result<Self> {
        limits.validate()?;
        let pool = WorkerPool::new(limits.workers)?;
        let work = Self {
            login: pool.class(limits.login_queue)?,
            signup: pool.capped_class(limits.signup_workers, limits.signup_queue)?,
            reset: pool.capped_class(limits.reset_workers, limits.reset_queue)?,
        };
        // Fail startup rather than making the first unknown-account login pay
        // the one-time dummy-hash initialization cost.
        dummy_password_hash();
        Ok(work)
    }

    /// Exposed for deterministic adapter saturation tests and diagnostics.
    pub fn admission(&self, kind: PasswordWorkKind) -> &AdmissionClass {
        match kind {
            PasswordWorkKind::Login => &self.login,
            PasswordWorkKind::Signup => &self.signup,
            PasswordWorkKind::Reset => &self.reset,
        }
    }
}

#[async_trait]
impl PasswordWorkPort for AdmittedArgon2PasswordWork {
    async fn verify_login(
        &self,
        password: &str,
        encoded_hash: &str,
    ) -> crate::application::errors::AuthResult<bool> {
        let result: anyhow::Result<_> =
            async { verify_on(&self.login, password, encoded_hash).await }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn equalize_login(&self, password: &str) -> crate::application::errors::AuthResult<()> {
        let result: anyhow::Result<_> = async {
            let _ = verify_on(&self.login, password, dummy_password_hash()).await?;
            Ok(())
        }
        .await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn hash_for_signup(
        &self,
        password: &str,
    ) -> crate::application::errors::AuthResult<String> {
        let result: anyhow::Result<_> = async { hash_on(&self.signup, password).await }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    async fn hash_for_reset(
        &self,
        password: &str,
    ) -> crate::application::errors::AuthResult<String> {
        let result: anyhow::Result<_> = async { hash_on(&self.reset, password).await }.await;
        result.map_err(crate::adapters::out::errors::boundary_error)
    }

    fn stats(&self, kind: PasswordWorkKind) -> PasswordWorkStats {
        let stats = self.admission(kind).stats();
        PasswordWorkStats {
            admitted: stats.admitted,
            rejected: stats.rejected,
        }
    }
}

/// Synchronous Argon2 hashing for fixtures and startup seeding only.
pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = argon2::password_hash::SaltString::generate(&mut OsRng);
    Ok(Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map_err(|error| anyhow::anyhow!(error.to_string()))?
        .to_string())
}

async fn hash_on(class: &AdmissionClass, password: &str) -> anyhow::Result<String> {
    let password = password.to_owned();
    busy_when_full(class.run(move || hash_password(&password)).await)
}

async fn verify_on(
    class: &AdmissionClass,
    password: &str,
    encoded_hash: &str,
) -> anyhow::Result<bool> {
    if password.len() > MAX_VERIFIED_PASSWORD_BYTES {
        return Ok(false);
    }
    let password = password.to_owned();
    let encoded_hash = encoded_hash.to_owned();
    busy_when_full(
        class
            .run(move || {
                let parsed_hash = argon2::PasswordHash::new(&encoded_hash)
                    .map_err(|_| anyhow::anyhow!("Invalid stored password hash"))?;
                Ok(Argon2::default()
                    .verify_password(password.as_bytes(), &parsed_hash)
                    .is_ok())
            })
            .await,
    )
}

fn busy_when_full<T>(result: anyhow::Result<T>) -> anyhow::Result<T> {
    result.map_err(|error| {
        if error.is::<CapacityExhausted>() {
            AuthError::Busy.into()
        } else {
            error
        }
    })
}

fn dummy_password_hash() -> &'static str {
    static DUMMY: OnceLock<String> = OnceLock::new();
    DUMMY.get_or_init(|| {
        hash_password("dummy-password-for-timing-equalization")
            .expect("the static dummy hash must compute")
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::mpsc;

    struct OccupiedSlot {
        release: mpsc::Sender<()>,
        task: tokio::task::JoinHandle<anyhow::Result<()>>,
    }

    impl OccupiedSlot {
        async fn release(self) {
            let _ = self.release.send(());
            self.task
                .await
                .expect("occupying task")
                .expect("occupying computation");
        }
    }

    async fn occupy(class: &AdmissionClass) -> OccupiedSlot {
        let (started_tx, started_rx) = tokio::sync::oneshot::channel();
        let (release, release_rx) = mpsc::channel::<()>();
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
        started_rx.await.expect("occupying computation started");
        OccupiedSlot { release, task }
    }

    fn small(login_queue: usize) -> AdmittedArgon2PasswordWork {
        AdmittedArgon2PasswordWork::new(PasswordWorkLimits {
            workers: 3,
            login_queue,
            signup_workers: 1,
            signup_queue: 0,
            reset_workers: 1,
            reset_queue: 0,
        })
        .expect("small limits")
    }

    #[tokio::test]
    async fn signup_and_reset_saturation_preserve_login_capacity() {
        let work = small(0);
        let encoded = hash_password("Passw0rd!").expect("hash");
        let signup = occupy(work.admission(PasswordWorkKind::Signup)).await;
        let reset = occupy(work.admission(PasswordWorkKind::Reset)).await;

        for refused in [
            work.hash_for_signup("NewPassw0rd!").await,
            work.hash_for_reset("NewPassw0rd!").await,
        ] {
            let error = refused.expect_err("capped class must reject");
            assert!(matches!(Some(&error), Some(AuthError::Busy)));
        }

        let verified = tokio::time::timeout(
            std::time::Duration::from_secs(60),
            work.verify_login("Passw0rd!", &encoded),
        )
        .await
        .expect("login must use reserved capacity")
        .expect("verification");
        assert!(verified);

        signup.release().await;
        reset.release().await;
    }

    #[tokio::test]
    async fn a_full_login_class_returns_busy_and_recovers_after_release() {
        let work = small(0);
        let encoded = hash_password("Passw0rd!").expect("hash");
        let mut slots = Vec::new();
        for _ in 0..3 {
            slots.push(occupy(work.admission(PasswordWorkKind::Login)).await);
        }

        let error = work
            .verify_login("Passw0rd!", &encoded)
            .await
            .expect_err("full login class must reject");
        assert!(matches!(Some(&error), Some(AuthError::Busy)));
        assert_eq!(work.stats(PasswordWorkKind::Login).rejected, 1);

        for slot in slots {
            slot.release().await;
        }
        assert!(work
            .verify_login("Passw0rd!", &encoded)
            .await
            .expect("capacity returns"));
    }

    #[tokio::test]
    async fn clones_share_admission_and_metrics() {
        let work = small(0);
        let clone = work.clone();
        let slot = occupy(work.admission(PasswordWorkKind::Reset)).await;
        let error = clone
            .hash_for_reset("NewPassw0rd!")
            .await
            .expect_err("clone shares the full reset class");
        assert!(matches!(Some(&error), Some(AuthError::Busy)));
        assert_eq!(work.stats(PasswordWorkKind::Reset).rejected, 1);
        slot.release().await;
    }
}
