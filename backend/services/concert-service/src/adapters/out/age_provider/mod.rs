//! BE26 owned SQL adapter. Root registration/provider/HTTP remain separate gates.
//! Integration composes the application module as crate::age_verification until exported.
use super::repository::receipt::{CommandProducer, PgCommandRepository};
use crate::age_verification::*;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::{age_policy::Context, commands::*, Clock};
use concert_domain::age_policy::{OwnerClaim, StoredStatus};
use ring::rand::{SecureRandom, SystemRandom};
use sqlx::{PgConnection, PgPool, Row};
use std::sync::Arc;
use uuid::Uuid;

pub const SCHEMA_V1: &str = include_str!("schema-v1.sql");
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Read,
    Start,
    Callback,
}
/// Resolve current canonical Auth identity/session/event/eligibility and hold the real
/// owner fence THROUGH this SQL commit. BE04 observations/UUIDs alone are insufficient.
/// A timestamp is not a fence. Missing production integration must remain unavailable.
#[async_trait]
pub trait Authority: Send + Sync {
    async fn authorize_in(
        &self,
        c: &mut PgConnection,
        context: Context,
        access: Access,
    ) -> Result<(), CommandError>;
}
pub struct UnavailableAuthority;
#[async_trait]
impl Authority for UnavailableAuthority {
    async fn authorize_in(
        &self,
        _: &mut PgConnection,
        _: Context,
        _: Access,
    ) -> Result<(), CommandError> {
        Err(CommandError::Unavailable)
    }
}
pub struct StartResult {
    pub receipt: SafeReceipt,
    pub authorization: Option<Authorization>,
}
/// One-shot private response. Never persisted in a receipt or log.
pub struct Authorization {
    pub verification_id: Uuid,
    pub expires_at: DateTime<Utc>,
    pub authorization_url: String,
}
pub struct CallbackResult {
    pub receipt: SafeReceipt,
    pub status: Option<OwnStatus>,
}
#[derive(Clone)]
pub struct PgAgeClaims {
    pool: PgPool,
    authority: Arc<dyn Authority>,
    verifier: Arc<dyn Verifier>,
    config: Option<ProviderConfig>,
    clock: Arc<dyn Clock>,
    keys: Arc<FingerprintKeys>,
}
impl PgAgeClaims {
    pub fn new(
        pool: PgPool,
        authority: Arc<dyn Authority>,
        verifier: Arc<dyn Verifier>,
        config: Option<ProviderConfig>,
        clock: Arc<dyn Clock>,
        keys: Arc<FingerprintKeys>,
    ) -> Self {
        Self {
            pool,
            authority,
            verifier,
            config,
            clock,
            keys,
        }
    }
    pub fn unavailable(pool: PgPool, clock: Arc<dyn Clock>, keys: Arc<FingerprintKeys>) -> Self {
        Self::new(
            pool,
            Arc::new(UnavailableAuthority),
            Arc::new(UnavailableVerifier),
            None,
            clock,
            keys,
        )
    }
    fn available(&self) -> bool {
        self.config.is_some() && self.verifier.available()
    }
    pub async fn start(
        &self,
        c: Context,
        key: Uuid,
        return_target: &str,
    ) -> Result<StartResult, Error> {
        bounded(c, Error::Pending, self.start_inner(c, key, return_target)).await
    }
    async fn start_inner(
        &self,
        c: Context,
        key: Uuid,
        return_target: &str,
    ) -> Result<StartResult, Error> {
        if !self.available() {
            return Err(Error::ProviderUnavailable);
        }
        check(c, self.clock.now())?;
        if key.is_nil() || return_target != RETURN_TARGET {
            return Err(Error::Invalid);
        }
        let config = self.config.as_ref().ok_or(Error::ProviderUnavailable)?;
        let mut bytes = [0; 32];
        SystemRandom::new()
            .fill(&mut bytes)
            .map_err(|_| Error::Unavailable)?;
        let token = StateToken::from_random_bytes(bytes);
        let digest = token.digest();
        let id = verification_id(c, key);
        let request = request(
            c,
            key,
            "startConcertAgeVerification",
            vec![
                ("return_target", RETURN_TARGET.into()),
                ("session", c.session.to_string()),
            ],
        )?;
        let producer = Producer {
            owner: self,
            c,
            action: Action::Start {
                digest,
                config: config.clone(),
            },
        };
        let receipt = PgCommandRepository::new(self.pool.clone())
            .execute(&request, id, &self.keys, &producer)
            .await
            .map_err(|e| {
                if e == CommandError::Unavailable {
                    Error::Pending
                } else {
                    command(e)
                }
            })?;
        // A replay can recover its safe receipt, but cannot reconstruct a digest-only raw
        // state or fabricate another URL. Unknown original delivery requires explicit UX.
        let mut tx = self.pool.begin().await.map_err(db)?;
        authorize(self, &mut tx, c, Access::Read)
            .await
            .map_err(command)?;
        let row=sqlx::query("select state_digest,expires_at,state from concert_age_verifications where verification_id=$1 and user_id=$2 and session_id=$3")
   .bind(id).bind(c.actor.uuid()).bind(c.session).fetch_one(&mut *tx).await.map_err(db)?;
        let stored: Option<Vec<u8>> = row.try_get("state_digest").map_err(db)?;
        let expires_at: DateTime<Utc> = row.try_get("expires_at").map_err(db)?;
        deadline(self, &mut tx, c).await.map_err(command)?;
        let authorization = if stored.as_deref() == Some(digest.as_slice())
            && row.try_get::<String, _>("state").map_err(db)? == "PENDING"
            && expires_at > self.clock.now()
        {
            Some(Authorization {
                verification_id: id,
                expires_at,
                authorization_url: config.authorization_url(id, &token),
            })
        } else {
            None
        };
        tx.commit().await.map_err(|_| Error::Pending)?;
        Ok(StartResult {
            receipt,
            authorization,
        })
    }
    pub async fn callback(
        &self,
        c: Context,
        key: Uuid,
        id: Uuid,
        state: StateToken,
        response: ProviderResponse,
    ) -> Result<CallbackResult, Error> {
        bounded(
            c,
            Error::Pending,
            self.callback_inner(c, key, id, state, response),
        )
        .await
    }
    async fn callback_inner(
        &self,
        c: Context,
        key: Uuid,
        id: Uuid,
        state: StateToken,
        response: ProviderResponse,
    ) -> Result<CallbackResult, Error> {
        check(c, self.clock.now())?;
        if key.is_nil() || id.is_nil() {
            return Err(Error::Invalid);
        }
        let request = request(
            c,
            key,
            "completeConcertAgeVerification",
            vec![
                ("verification_id", id.to_string()),
                ("session", c.session.to_string()),
                ("state_token", state.expose().into()),
                ("provider_response_token", response.expose().into()),
            ],
        )?;
        let attempt = Uuid::new_v4();
        let producer = Producer {
            owner: self,
            c,
            action: Action::Reserve {
                digest: state.digest(),
                attempt,
                available: self.available(),
            },
        };
        let receipt = PgCommandRepository::new(self.pool.clone())
            .execute(&request, id, &self.keys, &producer)
            .await
            .map_err(|e| {
                if e == CommandError::Unavailable && !self.available() {
                    Error::ProviderUnavailable
                } else if e == CommandError::Unavailable {
                    Error::Pending
                } else {
                    command(e)
                }
            })?;
        if receipt.state != ReceiptState::Pending {
            return Ok(CallbackResult {
                receipt,
                status: Some(self.status(c).await?),
            });
        }
        // Only the invocation that durably reserved this attempt may dispatch it once.
        let mut tx = self.pool.begin().await.map_err(db)?;
        authorize(self, &mut tx, c, Access::Callback)
            .await
            .map_err(command)?;
        let row=sqlx::query("update concert_age_verifications set state='EXCHANGING' where verification_id=$1 and attempt_id=$2 and callback_key=$3 and state='RESERVED' and user_id=$4 and session_id=$5 and expires_at>clock_timestamp() returning *")
   .bind(id).bind(attempt).bind(key).bind(c.actor.uuid()).bind(c.session).fetch_optional(&mut *tx).await.map_err(db)?;
        let Some(row) = row else {
            tx.rollback().await.map_err(db)?;
            return Ok(CallbackResult {
                receipt,
                status: None,
            });
        };
        let challenge = challenge(row)?;
        deadline(self, &mut tx, c).await.map_err(command)?;
        tx.commit().await.map_err(|_| Error::Pending)?; // Never call provider on uncertain dispatch commit.
        let config = self.config.as_ref().ok_or(Error::ProviderUnavailable)?;
        let until = c.expires_at.min(challenge.expires_at);
        let remaining = (until - Utc::now()).to_std().map_err(|_| Error::Expired)?;
        let proof = tokio::time::timeout(remaining, self.verifier.verify(&challenge, &response))
            .await
            .map_err(|_| Error::Pending)??;
        let claim = claim(proof, &challenge, config, self.clock.now())?;
        self.finish(c, key, attempt, &receipt, &challenge, claim)
            .await?;
        let recovery = Producer {
            owner: self,
            c,
            action: Action::Recover(Access::Callback),
        };
        let receipt = PgCommandRepository::new(self.pool.clone())
            .lookup(request.scope(), id, &recovery)
            .await
            .map_err(command)?
            .ok_or(Error::Pending)?;
        Ok(CallbackResult {
            receipt,
            status: Some(self.status(c).await?),
        })
    }
    async fn finish(
        &self,
        c: Context,
        key: Uuid,
        attempt: Uuid,
        receipt: &SafeReceipt,
        ch: &Challenge,
        value: OwnerClaim,
    ) -> Result<(), Error> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        authorize(self, &mut tx, c, Access::Callback)
            .await
            .map_err(command)?;
        let row = sqlx::query(
            "select * from concert_age_verifications where verification_id=$1 for update",
        )
        .bind(ch.binding.verification)
        .fetch_one(&mut *tx)
        .await
        .map_err(db)?;
        if row.try_get::<String, _>("state").map_err(db)? != "EXCHANGING"
            || row.try_get::<Uuid, _>("attempt_id").map_err(db)? != attempt
            || row.try_get::<Uuid, _>("callback_key").map_err(db)? != key
            || row.try_get::<Uuid, _>("receipt_id").map_err(db)? != receipt.id
            || challenge(row)?.binding != ch.binding
        {
            return Err(Error::Denied);
        }
        if self.clock.now() >= ch.expires_at {
            return Err(Error::Expired);
        }
        let existing:Option<(DateTime<Utc>,DateTime<Utc>,Uuid)>=sqlx::query_as("select verified_at,expires_at,claim_id from concert_age_claims where user_id=$1 order by verified_at desc,expires_at desc,claim_id desc limit 1")
   .bind(c.actor.uuid()).fetch_optional(&mut *tx).await.map_err(db)?;
        if existing.is_some_and(|old| old >= (value.verified_at, value.expires_at, value.claim_id))
        {
            return Err(Error::Conflict);
        }
        sqlx::query("update concert_age_claims set status='REPLACED' where user_id=$1 and status='VERIFIED'").bind(c.actor.uuid()).execute(&mut *tx).await.map_err(db)?;
        let status = if value.status == StoredStatus::Verified {
            "VERIFIED"
        } else {
            "UNSUPPORTED"
        };
        sqlx::query("insert into concert_age_claims(claim_id,user_id,event_id,verifier_id,source_revision,status,verified_at,expires_at,age_at_verification,next_birthday_at) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10)")
   .bind(value.claim_id).bind(c.actor.uuid()).bind(c.event.uuid()).bind(&value.verifier_id).bind(&value.source_revision).bind(status).bind(value.verified_at).bind(value.expires_at).bind(value.age_at_verification).bind(value.next_birthday_at).execute(&mut *tx).await.map_err(db)?;
        let result=sqlx::query("update concert_command_receipts set state='SUCCEEDED',resource_id=$2,resource_revision=1,result_status=200 where receipt_id=$1 and state='PENDING' and actor_id=$3 and event_id=$4 and operation_id='completeConcertAgeVerification' and idempotency_key=$5 and expires_at>clock_timestamp()")
   .bind(receipt.id).bind(value.claim_id).bind(c.actor.uuid()).bind(c.event.uuid()).bind(key).execute(&mut *tx).await.map_err(db)?;
        if result.rows_affected() != 1 {
            return Err(Error::Pending);
        }
        sqlx::query("update concert_age_verifications set state='DONE',state_digest=null where verification_id=$1").bind(value.claim_id).execute(&mut *tx).await.map_err(db)?;
        // Recheck wall time after authority/SQL waits; failures roll back claim+receipt together.
        let alive: bool = sqlx::query_scalar("select clock_timestamp()<$1")
            .bind(ch.expires_at)
            .fetch_one(&mut *tx)
            .await
            .map_err(db)?;
        if !alive {
            return Err(Error::Expired);
        }
        deadline(self, &mut tx, c).await.map_err(command)?;
        tx.commit().await.map_err(|_| Error::Pending)?;
        Ok(())
    }
    pub async fn status(&self, c: Context) -> Result<OwnStatus, Error> {
        bounded(c, Error::Unavailable, self.status_inner(c)).await
    }
    async fn status_inner(&self, c: Context) -> Result<OwnStatus, Error> {
        let mut tx = self.pool.begin().await.map_err(db)?;
        authorize(self, &mut tx, c, Access::Read)
            .await
            .map_err(command)?;
        // Canonical ORDER first, never filter status/expiry before selection.
        let row=sqlx::query("select * from concert_age_claims where user_id=$1 order by verified_at desc,expires_at desc,claim_id desc limit 1").bind(c.actor.uuid()).fetch_optional(&mut *tx).await.map_err(db)?;
        let claims = row
            .map(owner_claim)
            .transpose()?
            .into_iter()
            .collect::<Vec<_>>();
        deadline(self, &mut tx, c).await.map_err(command)?;
        let result = own_status(c, &claims, self.available(), self.clock.now())?;
        tx.commit().await.map_err(db)?;
        Ok(result)
    }
    /// Status only: absence is not permission to replay an uncertain external exchange.
    pub async fn recover(
        &self,
        c: Context,
        key: Uuid,
        id: Uuid,
        callback: bool,
    ) -> Result<Option<SafeReceipt>, Error> {
        bounded(
            c,
            Error::Unavailable,
            self.recover_inner(c, key, id, callback),
        )
        .await
    }
    async fn recover_inner(
        &self,
        c: Context,
        key: Uuid,
        id: Uuid,
        callback: bool,
    ) -> Result<Option<SafeReceipt>, Error> {
        let op = RegisteredOperation::find(if callback {
            "completeConcertAgeVerification"
        } else {
            "startConcertAgeVerification"
        })
        .map_err(command)?;
        let scope = Scope {
            actor: c.actor.uuid(),
            event: c.event.uuid(),
            key,
            operation: op,
        };
        PgCommandRepository::new(self.pool.clone())
            .lookup(
                scope,
                id,
                &Producer {
                    owner: self,
                    c,
                    action: Action::Recover(if callback {
                        Access::Callback
                    } else {
                        Access::Start
                    }),
                },
            )
            .await
            .map_err(command)
    }
    /// Bounded tombstone cleanup, not a scheduler or a license to replay exchange.
    pub async fn expire_states(&self, c: Context, limit: i64) -> Result<u64, Error> {
        bounded(c, Error::Pending, self.expire_states_inner(c, limit)).await
    }
    async fn expire_states_inner(&self, c: Context, limit: i64) -> Result<u64, Error> {
        if !(1..=100).contains(&limit) {
            return Err(Error::Invalid);
        }
        let mut tx = self.pool.begin().await.map_err(db)?;
        authorize(self, &mut tx, c, Access::Callback)
            .await
            .map_err(command)?;
        let changed=sqlx::query("with expired as(select verification_id from concert_age_verifications where user_id=$1 and expires_at<=clock_timestamp() and state<>'EXPIRED' order by expires_at,verification_id limit $2 for update skip locked) update concert_age_verifications v set state='EXPIRED',state_digest=null from expired e where v.verification_id=e.verification_id")
   .bind(c.actor.uuid()).bind(limit).execute(&mut *tx).await.map_err(db)?;
        sqlx::query("with expired as(select claim_id from concert_age_claims where user_id=$1 and expires_at<=clock_timestamp() and status<>'EXPIRED' order by expires_at,claim_id limit $2 for update skip locked) update concert_age_claims c set status='EXPIRED',age_at_verification=null,next_birthday_at=null from expired e where c.claim_id=e.claim_id")
            .bind(c.actor.uuid()).bind(limit).execute(&mut *tx).await.map_err(db)?;
        deadline(self, &mut tx, c).await.map_err(command)?;
        tx.commit().await.map_err(db)?;
        Ok(changed.rows_affected())
    }
}
enum Action {
    Start {
        digest: [u8; 32],
        config: ProviderConfig,
    },
    Reserve {
        digest: [u8; 32],
        attempt: Uuid,
        available: bool,
    },
    Recover(Access),
}
struct Producer<'a> {
    owner: &'a PgAgeClaims,
    c: Context,
    action: Action,
}
#[async_trait]
impl CommandProducer for Producer<'_> {
    async fn authorize(
        &self,
        conn: &mut PgConnection,
        s: Scope,
        id: Uuid,
    ) -> Result<(), CommandError> {
        let access = match self.action {
            Action::Start { .. } => Access::Start,
            Action::Reserve { .. } => Access::Callback,
            Action::Recover(a) => a,
        };
        let op = if access == Access::Start {
            "startConcertAgeVerification"
        } else {
            "completeConcertAgeVerification"
        };
        if s.actor != self.c.actor.uuid()
            || s.event != self.c.event.uuid()
            || s.key.is_nil()
            || s.operation.id() != op
        {
            return Err(CommandError::Denied);
        }
        authorize(self.owner, conn, self.c, access).await?;
        let row=sqlx::query("select user_id,event_id,session_id from concert_age_verifications where verification_id=$1 for update").bind(id).fetch_optional(&mut *conn).await.map_err(unavailable)?;
        if let Some(row) = row {
            if row.try_get::<Uuid, _>("user_id").map_err(unavailable)? != s.actor
                || row.try_get::<Uuid, _>("event_id").map_err(unavailable)? != s.event
                || row.try_get::<Uuid, _>("session_id").map_err(unavailable)? != self.c.session
            {
                return Err(CommandError::Denied);
            }
        } else if access != Access::Start || !matches!(self.action, Action::Start { .. }) {
            return Err(CommandError::Denied);
        }
        Ok(())
    }
    async fn apply(
        &self,
        conn: &mut PgConnection,
        s: Scope,
        id: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        match &self.action {
            Action::Start { digest, config } => {
                sqlx::query("insert into concert_age_verifications(verification_id,user_id,event_id,session_id,verifier_id,source_revision,state_digest,return_target,created_at,expires_at,state) select $1,$2,$3,$4,$5,$6,$7,'AGE_PRIVACY',t,t+interval '10 minutes','PENDING' from(select clock_timestamp() t) n")
     .bind(id).bind(s.actor).bind(s.event).bind(self.c.session).bind(&config.verifier).bind(&config.revision).bind(digest.as_slice()).execute(&mut *conn).await.map_err(unavailable)?;
                deadline(self.owner, conn, self.c).await?;
                Ok((
                    Completion::Succeeded {
                        reference: SafeReference {
                            resource: id,
                            revision: 1,
                        },
                        status: 200,
                    },
                    None,
                ))
            }
            Action::Reserve {
                digest,
                attempt,
                available,
            } => {
                if !available {
                    return Err(CommandError::Unavailable);
                }
                let changed=sqlx::query("update concert_age_verifications set state='RESERVED',attempt_id=$2,callback_key=$3,receipt_id=(select receipt_id from concert_command_receipts where actor_id=$4 and event_id=$5 and operation_id=$6 and idempotency_key=$3) where verification_id=$1 and state='PENDING' and state_digest=$7 and expires_at>clock_timestamp()")
     .bind(id).bind(attempt).bind(s.key).bind(s.actor).bind(s.event).bind(s.operation.id()).bind(digest.as_slice()).execute(&mut *conn).await.map_err(unavailable)?;
                if changed.rows_affected() != 1 {
                    return Err(CommandError::Denied);
                }
                deadline(self.owner, conn, self.c).await?;
                Ok((Completion::Pending, None))
            }
            Action::Recover(_) => Err(CommandError::InvalidState),
        }
    }
}
async fn authorize(
    owner: &PgAgeClaims,
    c: &mut PgConnection,
    context: Context,
    access: Access,
) -> Result<(), CommandError> {
    check(context, owner.clock.now()).map_err(|_| CommandError::Denied)?;
    // Adapted timeout clamp from BE24 at3810016; never widens shorter caller settings.
    sqlx::query("select set_config('lock_timeout',(extract(epoch from least(coalesce(nullif(current_setting('lock_timeout')::interval,interval '0'),interval '1500 milliseconds'),interval '1500 milliseconds'))*1000)::bigint::text||'ms',true),set_config('statement_timeout',(extract(epoch from least(coalesce(nullif(current_setting('statement_timeout')::interval,interval '0'),interval '5 seconds'),interval '5 seconds'))*1000)::bigint::text||'ms',true)").execute(&mut *c).await.map_err(unavailable)?;
    owner.authority.authorize_in(c, context, access).await?;
    let phase: Option<String> =
        sqlx::query_scalar("select phase from concert_events where event_id=$1 for share")
            .bind(context.event.uuid())
            .fetch_optional(&mut *c)
            .await
            .map_err(unavailable)?;
    if phase.is_none()
        || (access != Access::Read && !matches!(phase.as_deref(), Some("OPEN" | "LIVE")))
    {
        return Err(CommandError::Denied);
    }
    // Consistent per-canonical-user order BEFORE state/BE15 budget locks prevents cross-flow deadlocks.
    sqlx::query("insert into concert_age_owners(user_id) values($1) on conflict do nothing")
        .bind(context.actor.uuid())
        .execute(&mut *c)
        .await
        .map_err(unavailable)?;
    sqlx::query("select user_id from concert_age_owners where user_id=$1 for update")
        .bind(context.actor.uuid())
        .fetch_one(&mut *c)
        .await
        .map_err(unavailable)?;
    deadline(owner, c, context).await
}
async fn deadline(
    owner: &PgAgeClaims,
    c: &mut PgConnection,
    context: Context,
) -> Result<(), CommandError> {
    check(context, owner.clock.now()).map_err(|_| CommandError::Denied)?;
    let live: bool = sqlx::query_scalar("select clock_timestamp()<$1")
        .bind(context.expires_at)
        .fetch_one(c)
        .await
        .map_err(unavailable)?;
    if live {
        Ok(())
    } else {
        Err(CommandError::Unavailable)
    }
}
fn request(
    c: Context,
    key: Uuid,
    op: &str,
    fields: Vec<(&str, String)>,
) -> Result<CanonicalRequest, Error> {
    let op = RegisteredOperation::find(op).map_err(command)?;
    let body = CanonicalValue::Object(
        fields
            .into_iter()
            .map(|(k, v)| (k.into(), CanonicalValue::String(v)))
            .collect(),
    );
    CanonicalRequest::json(
        Scope {
            actor: c.actor.uuid(),
            event: c.event.uuid(),
            key,
            operation: op,
        },
        &[("eventId", &c.event.uuid().to_string())],
        &[],
        Some(&body),
    )
    .map_err(command)
}
fn challenge(r: sqlx::postgres::PgRow) -> Result<Challenge, Error> {
    let digest: Vec<u8> = r.try_get("state_digest").map_err(db)?;
    Ok(Challenge {
        binding: Binding {
            verification: r.try_get("verification_id").map_err(db)?,
            actor: r.try_get("user_id").map_err(db)?,
            event: r.try_get("event_id").map_err(db)?,
            session: r.try_get("session_id").map_err(db)?,
            verifier: r.try_get("verifier_id").map_err(db)?,
            state_digest: digest.try_into().map_err(|_| Error::Unavailable)?,
        },
        created_at: r.try_get("created_at").map_err(db)?,
        expires_at: r.try_get("expires_at").map_err(db)?,
        source_revision: r.try_get("source_revision").map_err(db)?,
    })
}
fn owner_claim(r: sqlx::postgres::PgRow) -> Result<OwnerClaim, Error> {
    let status = match r.try_get::<String, _>("status").map_err(db)?.as_str() {
        "VERIFIED" | "EXPIRED" => StoredStatus::Verified,
        "REPLACED" => StoredStatus::Replaced,
        "REVOKED" => StoredStatus::Revoked,
        "UNSUPPORTED" => StoredStatus::Unsupported,
        _ => return Err(Error::Unavailable),
    };
    Ok(OwnerClaim {
        claim_id: r.try_get("claim_id").map_err(db)?,
        user_id: concert_domain::AuthUserId::from_uuid(r.try_get("user_id").map_err(db)?),
        verifier_id: r.try_get("verifier_id").map_err(db)?,
        source_revision: r.try_get("source_revision").map_err(db)?,
        status,
        verified_at: r.try_get("verified_at").map_err(db)?,
        expires_at: r.try_get("expires_at").map_err(db)?,
        age_at_verification: r.try_get("age_at_verification").map_err(db)?,
        next_birthday_at: r.try_get("next_birthday_at").map_err(db)?,
    })
}
fn unavailable(_: sqlx::Error) -> CommandError {
    CommandError::Unavailable
}
fn db(_: sqlx::Error) -> Error {
    Error::Unavailable
}
fn command(e: CommandError) -> Error {
    match e {
        CommandError::Denied => Error::Denied,
        CommandError::InvalidInput => Error::Invalid,
        CommandError::Expired => Error::Expired,
        CommandError::Conflict => Error::Conflict,
        CommandError::Unavailable => Error::Unavailable,
        _ => Error::Unavailable,
    }
}

async fn bounded<T>(
    context: Context,
    on_timeout: Error,
    future: impl std::future::Future<Output = Result<T, Error>>,
) -> Result<T, Error> {
    let remaining = (context.expires_at - Utc::now())
        .to_std()
        .map_err(|_| Error::Expired)?;
    tokio::time::timeout(remaining, future)
        .await
        .map_err(|_| on_timeout)?
}
