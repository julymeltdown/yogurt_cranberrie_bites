use super::contract::*;
use crate::adapters::out::persistence::{
    auth_transactions::{register_email_connection, verify_email_connection},
    onboarding_transactions::resend_verification_connection,
    user_repository::PgUserRepository,
};
use crate::application::use_cases::signup::SignupUseCase;
use crate::application::verification_crypto::VerificationCryptoPort;
use chrono::{DateTime, Duration, Utc};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Postgres, Row, Transaction};
use std::sync::Arc;
use uuid::Uuid;

#[derive(Clone)]
pub struct EnrollmentStore {
    pool: PgPool,
    verification_url: Option<reqwest::Url>,
}
impl EnrollmentStore {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            verification_url: None,
        }
    }

    pub fn with_verification_url(
        pool: PgPool,
        url: &str,
        production: bool,
    ) -> Result<Self, EnrollmentError> {
        let url = reqwest::Url::parse(url).map_err(|_| EnrollmentError::InvalidInput)?;
        let local = url.host_str().is_some_and(|h| {
            h == "localhost"
                || h.parse::<std::net::IpAddr>()
                    .is_ok_and(|ip| ip.is_loopback())
        });
        if url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
            || url.query().is_some()
            || url.fragment().is_some()
            || url.path() != "/verify-email"
            || !(url.scheme() == "https" || (!production && local && url.scheme() == "http"))
        {
            return Err(EnrollmentError::InvalidInput);
        }
        Ok(Self {
            pool,
            verification_url: Some(url),
        })
    }
    pub fn from_env(pool: PgPool, production: bool) -> anyhow::Result<Self> {
        match std::env::var("AUTH_ENROLLMENT_VERIFICATION_URL") {
            Ok(url) => Self::with_verification_url(pool, &url, production).map_err(Into::into),
            Err(std::env::VarError::NotPresent) => Ok(Self::new(pool)),
            Err(_) => anyhow::bail!("AUTH_ENROLLMENT_VERIFICATION_URL must be Unicode"),
        }
    }
    fn verification_link(&self, attempt: Uuid, code: &str) -> Result<String, EnrollmentError> {
        let mut url = self
            .verification_url
            .clone()
            .ok_or(EnrollmentError::Unavailable)?;
        url.query_pairs_mut()
            .append_pair("attemptId", &attempt.to_string())
            .append_pair("code", code);
        Ok(url.into())
    }

    async fn transaction(&self) -> Result<Transaction<'_, Postgres>, EnrollmentError> {
        let mut tx = self.pool.begin().await.map_err(unavailable)?;
        sqlx::query("select set_config('lock_timeout','1500ms',true), set_config('statement_timeout','2s',true)")
            .execute(&mut *tx).await.map_err(unavailable)?;
        Ok(tx)
    }

    /// Explicit operator action only. There is no public policy-publication route.
    /// Revision content is immutable, including its approval provenance.
    pub async fn publish(&self, publication: &ApprovedPublication) -> Result<(), EnrollmentError> {
        let mut tx = self.transaction().await?;
        // The singleton row exists in the migration, even before publication.
        sqlx::query("select singleton from mcr_enrollment_current where singleton=true for update")
            .fetch_one(&mut *tx)
            .await
            .map_err(unavailable)?;
        let now = database_time(&mut tx).await?;
        publication.validate(now)?;
        let value = serde_json::to_value(publication).map_err(unavailable)?;
        let existing: Option<serde_json::Value> =
            sqlx::query_scalar("select publication from mcr_enrollment_policies where revision=$1")
                .bind(publication.revision)
                .fetch_optional(&mut *tx)
                .await
                .map_err(unavailable)?;
        if let Some(existing) = existing {
            if existing != value {
                return Err(EnrollmentError::IdempotencyConflict);
            }
        } else {
            sqlx::query("insert into mcr_enrollment_policies(revision,publication,published_at,expires_at) values($1,$2,$3,$4)")
                .bind(publication.revision).bind(value).bind(publication.published_at).bind(publication.expires_at)
                .execute(&mut *tx).await.map_err(unavailable)?;
        }
        sqlx::query("update mcr_enrollment_current set revision=$1 where singleton=true")
            .bind(publication.revision)
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)
    }

    pub async fn current_policy(&self) -> Result<CurrentPolicy, EnrollmentError> {
        let mut tx = self.transaction().await?;
        let now = database_time(&mut tx).await?;
        let policy = current(&mut tx, now).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(CurrentPolicy {
            contract_version: CONTRACT_VERSION.into(),
            revision: policy.revision,
            documents: policy.documents,
            published_at: policy.published_at,
            expires_at: policy.expires_at,
            server_time: now,
        })
    }

    pub async fn begin(
        &self,
        binding: &str,
        input: BeginRequest,
    ) -> Result<AttemptReceipt, EnrollmentError> {
        let binding_hash = binding_digest(binding)?;
        let mut tx = self.transaction().await?;
        let now = database_time(&mut tx).await?;
        let policy = current(&mut tx, now).await?;
        if input.policy_revision != policy.revision {
            return Err(EnrollmentError::PolicyStale);
        }
        let id = Uuid::new_v4();
        let expires = (now + Duration::seconds(ATTEMPT_SECONDS)).min(policy.expires_at);
        sqlx::query("insert into mcr_enrollment_attempts(id,binding_hash,revision,created_at,expires_at,state,resend_available_at) values($1,$2,$3,$4,$5,'STARTED',$4)")
            .bind(id).bind(binding_hash).bind(policy.revision).bind(now).bind(expires)
            .execute(&mut *tx).await.map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(AttemptReceipt {
            attempt_id: id,
            policy_revision: policy.revision,
            expires_at: expires,
            server_time: now,
        })
    }

    pub async fn signup(
        &self,
        owner: &SignupUseCase<Arc<PgUserRepository>>,
        binding: &str,
        input: SignupRequest,
    ) -> Result<AcceptedReceipt, EnrollmentError> {
        if input.attempt_id.is_nil() || input.idempotency_key.is_nil() {
            return Err(EnrollmentError::InvalidInput);
        }
        self.verification_url
            .as_ref()
            .ok_or(EnrollmentError::Unavailable)?;
        let fingerprint = signup_fingerprint(binding, &input)?;
        // Check freshness and replay before admitted expensive work; release every
        // database lock before Argon2, then recheck all mutable facts atomically.
        let mut tx = self.transaction().await?;
        let now = database_time(&mut tx).await?;
        let policy = current(&mut tx, now).await?;
        let attempt =
            lock_attempt(&mut tx, binding, input.attempt_id, policy.revision, now).await?;
        if input.policy_revision != policy.revision {
            return Err(EnrollmentError::PolicyStale);
        }
        policy.check_acknowledgements(&input.acknowledgement_ids)?;
        if let Some(receipt) = signup_replay(&attempt, input.idempotency_key, &fingerprint)? {
            tx.commit().await.map_err(unavailable)?;
            return Ok(receipt);
        }
        tx.rollback().await.map_err(unavailable)?;
        let (user, code, resume) = owner
            .prepare_email_signup(&input.email, &input.password)
            .await
            .map_err(|error| match error {
                crate::application::errors::AuthError::InvalidInput(_) => {
                    EnrollmentError::InvalidInput
                }
                _ => EnrollmentError::Unavailable,
            })?;
        let mut tx = self.transaction().await?;
        let now = database_time(&mut tx).await?;
        let policy = current(&mut tx, now).await?;
        let attempt =
            lock_attempt(&mut tx, binding, input.attempt_id, policy.revision, now).await?;
        if input.policy_revision != policy.revision {
            return Err(EnrollmentError::PolicyStale);
        }
        policy.check_acknowledgements(&input.acknowledgement_ids)?;
        if let Some(receipt) = signup_replay(&attempt, input.idempotency_key, &fingerprint)? {
            tx.commit().await.map_err(unavailable)?;
            return Ok(receipt);
        }
        // Same namespace as canonical OAuth email ownership. Unique constraints
        // remain the final arbiter against older signup callers without this lock.
        sqlx::query("select pg_advisory_xact_lock(7012,hashtext($1))")
            .bind(&user.primary_email)
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        let existing: bool =
            sqlx::query_scalar("select exists(select 1 from users where primary_email=$1)")
                .bind(&user.primary_email)
                .fetch_one(&mut *tx)
                .await
                .map_err(unavailable)?;
        let mut enrolled_user = None;
        if !existing {
            sqlx::query("savepoint enrollment_identity")
                .execute(&mut *tx)
                .await
                .map_err(unavailable)?;
            match register_email_connection(&mut tx, &user, &code.code).await {
                Ok(()) => {
                    enrolled_user = Some(user.id);
                    bind_enrollment_mail(
                        &mut tx,
                        user.id,
                        input.attempt_id,
                        &code.code,
                        &self.verification_link(input.attempt_id, &code.code)?,
                        attempt.expires_at,
                    )
                    .await?;
                }
                Err(error) => {
                    let collision = error
                        .downcast_ref::<sqlx::Error>()
                        .and_then(|e| e.as_database_error())
                        .and_then(|e| e.code())
                        .as_deref()
                        == Some("23505");
                    if !collision {
                        return Err(EnrollmentError::Unavailable);
                    }
                    sqlx::query("rollback to savepoint enrollment_identity")
                        .execute(&mut *tx)
                        .await
                        .map_err(unavailable)?;
                    let exists: bool = sqlx::query_scalar(
                        "select exists(select 1 from users where primary_email=$1)",
                    )
                    .bind(&user.primary_email)
                    .fetch_one(&mut *tx)
                    .await
                    .map_err(unavailable)?;
                    if !exists {
                        return Err(EnrollmentError::Unavailable);
                    }
                }
            }
            sqlx::query("release savepoint enrollment_identity")
                .execute(&mut *tx)
                .await
                .map_err(unavailable)?;
        }
        if existing {
            if let Some(snapshot) = resume {
                let same:Option<Uuid>=sqlx::query_scalar("select id from users where id=$1 and primary_email=$2 and password_hash=$3 and status='PENDING_VERIFICATION' and deleted_at is null for update")
                    .bind(snapshot.id).bind(&snapshot.primary_email).bind(&snapshot.password_hash)
                    .fetch_optional(&mut *tx).await.map_err(unavailable)?;
                if same == Some(snapshot.id) {
                    resend_verification_connection(&mut tx, &snapshot.primary_email, &code.code)
                        .await
                        .map_err(unavailable)?;
                    bind_enrollment_mail(
                        &mut tx,
                        snapshot.id,
                        input.attempt_id,
                        &code.code,
                        &self.verification_link(input.attempt_id, &code.code)?,
                        attempt.expires_at,
                    )
                    .await?;
                    enrolled_user = Some(snapshot.id);
                }
            }
        }
        let receipt = AcceptedReceipt {
            status: "ACCEPTED".into(),
            attempt_id: input.attempt_id,
            expires_at: attempt.expires_at,
            resend_available_at: now + Duration::seconds(RESEND_SECONDS),
            server_time: now,
        };
        sqlx::query("update mcr_enrollment_attempts set state=$2,user_id=$3,signup_key=$4,request_hash=$5,signup_receipt=$6,resend_available_at=$7 where id=$1")
            .bind(input.attempt_id).bind(if enrolled_user.is_some() {"PENDING"} else {"SHADOW"})
            .bind(enrolled_user).bind(input.idempotency_key).bind(fingerprint)
            .bind(serde_json::to_value(&receipt).map_err(unavailable)?).bind(receipt.resend_available_at)
            .execute(&mut *tx).await.map_err(unavailable)?;
        if let Some(enrolled_user) = enrolled_user {
            accept(
                &mut tx,
                enrolled_user,
                &policy,
                &input.acknowledgement_ids,
                Some(input.attempt_id),
                now,
            )
            .await?;
        }
        ensure_fresh(&mut tx, policy.expires_at, attempt.expires_at).await?;
        tx.commit().await.map_err(unavailable)?;
        Ok(receipt)
    }

    pub async fn verify(
        &self,
        binding: &str,
        input: VerifyRequest,
    ) -> Result<VerifiedReceipt, EnrollmentError> {
        if input.code.len() != 6 || !input.code.bytes().all(|b| b.is_ascii_digit()) {
            return Err(EnrollmentError::VerificationFailed);
        }
        let mut tx = self.transaction().await?;
        let now = database_time(&mut tx).await?;
        let policy = current(&mut tx, now).await?;
        let attempt =
            lock_attempt(&mut tx, binding, input.attempt_id, policy.revision, now).await?;
        if let Some(value) = attempt.verified_receipt {
            let receipt = serde_json::from_value(value).map_err(unavailable)?;
            tx.commit().await.map_err(unavailable)?;
            return Ok(receipt);
        }
        let Some(user_id) = attempt.user_id else {
            return Err(EnrollmentError::VerificationFailed);
        };
        let email:Option<String> = sqlx::query_scalar("select primary_email from users where id=$1 and status='PENDING_VERIFICATION' and deleted_at is null")
            .bind(user_id).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let Some(email) = email else {
            return Err(EnrollmentError::VerificationFailed);
        };
        let verified = verify_email_connection(&mut tx, &email, &input.code)
            .await
            .map_err(unavailable)?;
        if verified != Some(user_id) {
            // Wrong-code attempt increments are durable even though the command is denied.
            tx.commit().await.map_err(unavailable)?;
            return Err(EnrollmentError::VerificationFailed);
        }
        ensure_fresh(&mut tx, policy.expires_at, attempt.expires_at).await?;
        let receipt = VerifiedReceipt {
            status: "VERIFIED".into(),
            attempt_id: input.attempt_id,
            server_time: now,
        };
        sqlx::query(
            "update mcr_enrollment_attempts set state='VERIFIED',verified_receipt=$2 where id=$1",
        )
        .bind(input.attempt_id)
        .bind(serde_json::to_value(&receipt).map_err(unavailable)?)
        .execute(&mut *tx)
        .await
        .map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(receipt)
    }

    pub async fn resend(
        &self,
        binding: &str,
        input: ResendRequest,
    ) -> Result<AcceptedReceipt, EnrollmentError> {
        if input.idempotency_key.is_nil() {
            return Err(EnrollmentError::InvalidInput);
        }
        let code = crate::adapters::out::security::email_verification::SecureEmailVerification
            .generate()
            .map_err(unavailable)?;
        let mut tx = self.transaction().await?;
        let now = database_time(&mut tx).await?;
        let policy = current(&mut tx, now).await?;
        let attempt =
            lock_attempt(&mut tx, binding, input.attempt_id, policy.revision, now).await?;
        if attempt.signup_receipt.is_none() {
            return Err(EnrollmentError::AttemptInvalid);
        }
        let replay:Option<serde_json::Value>=sqlx::query_scalar("select receipt from mcr_enrollment_resend_requests where attempt_id=$1 and idempotency_key=$2")
            .bind(input.attempt_id).bind(input.idempotency_key).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        if let Some(value) = replay {
            let receipt = serde_json::from_value(value).map_err(unavailable)?;
            tx.commit().await.map_err(unavailable)?;
            return Ok(receipt);
        }
        let next = if now < attempt.resend_available_at {
            attempt.resend_available_at
        } else {
            now + Duration::seconds(RESEND_SECONDS)
        };
        if now >= attempt.resend_available_at && attempt.state == "PENDING" {
            if let Some(user) = attempt.user_id {
                let email:Option<String>=sqlx::query_scalar("select primary_email from users where id=$1 and status='PENDING_VERIFICATION' and deleted_at is null")
                    .bind(user).fetch_optional(&mut *tx).await.map_err(unavailable)?;
                if let Some(email) = email {
                    resend_verification_connection(&mut tx, &email, &code.code)
                        .await
                        .map_err(unavailable)?;
                    bind_enrollment_mail(
                        &mut tx,
                        user,
                        input.attempt_id,
                        &code.code,
                        &self.verification_link(input.attempt_id, &code.code)?,
                        attempt.expires_at,
                    )
                    .await?;
                }
            }
        }
        ensure_fresh(&mut tx, policy.expires_at, attempt.expires_at).await?;
        let receipt = AcceptedReceipt {
            status: "ACCEPTED".into(),
            attempt_id: input.attempt_id,
            expires_at: attempt.expires_at,
            resend_available_at: next,
            server_time: now,
        };
        sqlx::query("update mcr_enrollment_attempts set resend_available_at=$2 where id=$1")
            .bind(input.attempt_id)
            .bind(next)
            .execute(&mut *tx)
            .await
            .map_err(unavailable)?;
        sqlx::query("insert into mcr_enrollment_resend_requests(attempt_id,idempotency_key,receipt) values($1,$2,$3)")
            .bind(input.attempt_id).bind(input.idempotency_key).bind(serde_json::to_value(&receipt).map_err(unavailable)?)
            .execute(&mut *tx).await.map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(receipt)
    }

    /// Authenticated caller's UUID comes from verified actor delegation, never JSON.
    pub async fn admission(&self, user: Uuid) -> Result<AdmissionReceipt, EnrollmentError> {
        if user.is_nil() {
            return Err(EnrollmentError::InvalidInput);
        }
        let mut tx = self.transaction().await?;
        let now = database_time(&mut tx).await?;
        // Serialize with publication, so decision and acceptance use one revision.
        let policy = match current(&mut tx, now).await {
            Ok(policy) => Some(policy),
            Err(EnrollmentError::PolicyUnavailable) => None,
            Err(error) => return Err(error),
        };
        let active: bool = sqlx::query_scalar("select exists(select 1 from users where id=$1 and status='ACTIVE' and deleted_at is null)")
            .bind(user).fetch_one(&mut *tx).await.map_err(unavailable)?;
        let revision = policy.as_ref().map(|p| p.revision);
        let accepted: Option<Uuid> = sqlx::query_scalar("select revision from mcr_enrollment_acceptances where user_id=$1 order by accepted_at desc,revision limit 1")
            .bind(user).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        let current_accepted: bool = sqlx::query_scalar("select exists(select 1 from mcr_enrollment_acceptances where user_id=$1 and revision=$2)")
            .bind(user).bind(revision).fetch_one(&mut *tx).await.map_err(unavailable)?;
        let decision = if !active {
            AdmissionDecision::AccountInactive
        } else if policy.is_none() {
            AdmissionDecision::PolicyUnavailable
        } else if !current_accepted {
            AdmissionDecision::ConsentRequired
        } else {
            AdmissionDecision::Admitted
        };
        tx.commit().await.map_err(unavailable)?;
        Ok(AdmissionReceipt {
            decision,
            policy_revision: revision,
            accepted_revision: if current_accepted { revision } else { accepted },
            server_time: now,
        })
    }

    pub async fn consent(
        &self,
        user: Uuid,
        input: ConsentRequest,
    ) -> Result<AdmissionReceipt, EnrollmentError> {
        if user.is_nil() || input.idempotency_key.is_nil() {
            return Err(EnrollmentError::InvalidInput);
        }
        let mut tx = self.transaction().await?;
        let now = database_time(&mut tx).await?;
        let policy = current(&mut tx, now).await?;
        if policy.revision != input.policy_revision {
            return Err(EnrollmentError::PolicyStale);
        }
        policy.check_acknowledgements(&input.acknowledgement_ids)?;
        let active: Option<Uuid> = sqlx::query_scalar("select id from users where id=$1 and status='ACTIVE' and deleted_at is null for update")
            .bind(user).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        if active.is_none() {
            return Err(EnrollmentError::AccountInactive);
        }
        let replay = sqlx::query("select revision,receipt from mcr_enrollment_consent_requests where user_id=$1 and idempotency_key=$2")
            .bind(user).bind(input.idempotency_key).fetch_optional(&mut *tx).await.map_err(unavailable)?;
        if let Some(row) = replay {
            if row.try_get::<Uuid, _>("revision").map_err(unavailable)? != policy.revision {
                return Err(EnrollmentError::IdempotencyConflict);
            }
            let receipt = serde_json::from_value(row.try_get("receipt").map_err(unavailable)?)
                .map_err(unavailable)?;
            tx.commit().await.map_err(unavailable)?;
            return Ok(receipt);
        }
        accept(
            &mut tx,
            user,
            &policy,
            &input.acknowledgement_ids,
            None,
            now,
        )
        .await?;
        ensure_fresh(&mut tx, policy.expires_at, policy.expires_at).await?;
        let receipt = AdmissionReceipt {
            decision: AdmissionDecision::Admitted,
            policy_revision: Some(policy.revision),
            accepted_revision: Some(policy.revision),
            server_time: now,
        };
        sqlx::query("insert into mcr_enrollment_consent_requests(user_id,idempotency_key,revision,receipt) values($1,$2,$3,$4)")
            .bind(user).bind(input.idempotency_key).bind(policy.revision)
            .bind(serde_json::to_value(&receipt).map_err(unavailable)?)
            .execute(&mut *tx).await.map_err(unavailable)?;
        tx.commit().await.map_err(unavailable)?;
        Ok(receipt)
    }
}

pub(super) fn unavailable<E>(_: E) -> EnrollmentError {
    EnrollmentError::Unavailable
}
async fn database_time(connection: &mut PgConnection) -> Result<DateTime<Utc>, EnrollmentError> {
    sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(connection)
        .await
        .map_err(unavailable)
}
async fn current(
    connection: &mut PgConnection,
    now: DateTime<Utc>,
) -> Result<ApprovedPublication, EnrollmentError> {
    let row = sqlx::query("select p.publication from mcr_enrollment_current c left join mcr_enrollment_policies p on p.revision=c.revision where c.singleton=true for share of c")
        .fetch_one(&mut *connection).await.map_err(unavailable)?;
    let now = database_time(connection).await?.max(now);
    let value: Option<serde_json::Value> = row.try_get("publication").map_err(unavailable)?;
    let policy: ApprovedPublication =
        serde_json::from_value(value.ok_or(EnrollmentError::PolicyUnavailable)?)
            .map_err(unavailable)?;
    policy
        .validate(now)
        .map_err(|_| EnrollmentError::PolicyUnavailable)?;
    Ok(policy)
}
async fn accept(
    connection: &mut PgConnection,
    user: Uuid,
    policy: &ApprovedPublication,
    acknowledgements: &[String],
    attempt: Option<Uuid>,
    now: DateTime<Utc>,
) -> Result<(), EnrollmentError> {
    sqlx::query("insert into mcr_enrollment_acceptances(user_id,revision,acknowledgement_ids,attempt_id,accepted_at) values($1,$2,$3,$4,$5) on conflict(user_id,revision) do nothing")
        .bind(user).bind(policy.revision).bind(acknowledgements).bind(attempt).bind(now)
        .execute(connection).await.map_err(unavailable)?;
    Ok(())
}
fn binding_digest(binding: &str) -> Result<Vec<u8>, EnrollmentError> {
    if binding.len() != 64
        || !binding
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(EnrollmentError::AttemptInvalid);
    }
    Ok(Sha256::digest(binding.as_bytes()).to_vec())
}

struct Attempt {
    expires_at: DateTime<Utc>,
    state: String,
    user_id: Option<Uuid>,
    signup_key: Option<Uuid>,
    request_hash: Option<Vec<u8>>,
    signup_receipt: Option<serde_json::Value>,
    verified_receipt: Option<serde_json::Value>,
    resend_available_at: DateTime<Utc>,
}
async fn lock_attempt(
    connection: &mut PgConnection,
    binding: &str,
    id: Uuid,
    revision: Uuid,
    now: DateTime<Utc>,
) -> Result<Attempt, EnrollmentError> {
    let hash = binding_digest(binding)?;
    let row = sqlx::query(
        "select * from mcr_enrollment_attempts where id=$1 and binding_hash=$2 for update",
    )
    .bind(id)
    .bind(hash)
    .fetch_optional(&mut *connection)
    .await
    .map_err(unavailable)?
    .ok_or(EnrollmentError::AttemptInvalid)?;
    let now = database_time(connection).await?.max(now);
    let expires_at = row.try_get("expires_at").map_err(unavailable)?;
    if expires_at <= now {
        return Err(EnrollmentError::AttemptExpired);
    }
    if row.try_get::<Uuid, _>("revision").map_err(unavailable)? != revision {
        return Err(EnrollmentError::PolicyStale);
    }
    Ok(Attempt {
        expires_at,
        state: row.try_get("state").map_err(unavailable)?,
        user_id: row.try_get("user_id").map_err(unavailable)?,
        signup_key: row.try_get("signup_key").map_err(unavailable)?,
        request_hash: row.try_get("request_hash").map_err(unavailable)?,
        signup_receipt: row.try_get("signup_receipt").map_err(unavailable)?,
        verified_receipt: row.try_get("verified_receipt").map_err(unavailable)?,
        resend_available_at: row.try_get("resend_available_at").map_err(unavailable)?,
    })
}
fn signup_replay(
    attempt: &Attempt,
    key: Uuid,
    fingerprint: &[u8],
) -> Result<Option<AcceptedReceipt>, EnrollmentError> {
    if attempt.state == "STARTED" {
        return Ok(None);
    }
    if attempt.signup_key != Some(key) || attempt.request_hash.as_deref() != Some(fingerprint) {
        return Err(EnrollmentError::IdempotencyConflict);
    }
    Ok(Some(
        serde_json::from_value(
            attempt
                .signup_receipt
                .clone()
                .ok_or(EnrollmentError::Unavailable)?,
        )
        .map_err(unavailable)?,
    ))
}
fn signup_fingerprint(binding: &str, input: &SignupRequest) -> Result<Vec<u8>, EnrollmentError> {
    binding_digest(binding)?;
    let mut acks = input.acknowledgement_ids.clone();
    acks.sort();
    let bytes = serde_json::to_vec(&(
        "MCR_ENROLLMENT_SIGNUP_V1",
        input.attempt_id,
        input.policy_revision,
        acks,
        crate::domain::user::User::normalized_email(&input.email),
        &input.password,
    ))
    .map_err(unavailable)?;
    // HMAC-SHA256 with the unpersisted 256-bit browser secret. A database-only
    // reader cannot use this receipt hash as an offline password oracle.
    Ok(hmac_sha256(binding.as_bytes(), &bytes))
}
fn hmac_sha256(key: &[u8], message: &[u8]) -> Vec<u8> {
    let mut padded = [0u8; 64];
    if key.len() > 64 {
        padded[..32].copy_from_slice(&Sha256::digest(key));
    } else {
        padded[..key.len()].copy_from_slice(key);
    }
    let mut inner = Sha256::new();
    inner.update(padded.map(|b| b ^ 0x36));
    inner.update(message);
    let mut outer = Sha256::new();
    outer.update(padded.map(|b| b ^ 0x5c));
    outer.update(inner.finalize());
    outer.finalize().to_vec()
}

#[cfg(test)]
mod tests {
    #[test]
    fn hmac_uses_standard_rfc4231_vector() {
        let output = super::hmac_sha256(&[0x0b; 20], b"Hi There");
        assert_eq!(
            output
                .iter()
                .map(|b| format!("{b:02x}"))
                .collect::<String>(),
            "b0344c61d8db38535ca8afceaf0bf12b881dc200c9833da726e9376c2e32cff7"
        );
    }
}

async fn ensure_fresh(
    connection: &mut PgConnection,
    policy_expiry: DateTime<Utc>,
    attempt_expiry: DateTime<Utc>,
) -> Result<(), EnrollmentError> {
    let now = database_time(connection).await?;
    if now >= policy_expiry {
        return Err(EnrollmentError::PolicyUnavailable);
    }
    if now >= attempt_expiry {
        return Err(EnrollmentError::AttemptExpired);
    }
    Ok(())
}

async fn bind_enrollment_mail(
    connection: &mut PgConnection,
    user: Uuid,
    attempt: Uuid,
    code: &str,
    link: &str,
    expiry: DateTime<Utc>,
) -> Result<(), EnrollmentError> {
    // Only the fresh challenge's unleased pending mail can be decorated. If
    // canonical resend throttled without enqueueing, an older message is untouched.
    sqlx::query("update auth_mail_outbox set payload_json=(payload_json::jsonb || jsonb_build_object('enrollment_attempt_id',$2::text,'verification_link',$3::text))::text,expires_at=least(expires_at,$5) where user_id=$1 and kind='VERIFY_EMAIL' and delivered_at is null and dead_lettered_at is null and leased_until is null and payload_json::jsonb->>'code'=$4")
        .bind(user).bind(attempt.to_string()).bind(link).bind(code).bind(expiry)
        .execute(connection).await.map_err(unavailable)?;
    Ok(())
}
