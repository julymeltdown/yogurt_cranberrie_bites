//! BE27 durable introduction repository and transactional receipt producer.
//! Root module registration and production owner composition remain integration
//! gates; this file is compiled through exact test-path wiring until then.
use crate::adapters::out::repository::receipt::{CommandProducer, PgCommandRepository};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use concert_application::commands::*;
use concert_application::intro::{self, cursor};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

pub const SCHEMA_V1: &str = include_str!("intro_schema_v1.sql");

#[async_trait]
pub trait IntroPolicy: Send + Sync {
    /// Must lock current authority and set `mcr.intro_authority_until` on this
    /// transaction before any introduction mutation. No absent deadline is admitted.
    async fn check(
        &self,
        connection: &mut PgConnection,
        command: &intro::IntroCommand,
    ) -> Result<(), intro::IntroError>;
    async fn edit_consent(
        &self,
        connection: &mut PgConnection,
        command: &intro::IntroCommand,
        next: &intro::OwnerIntro,
    ) -> Result<(), intro::IntroError>;
}
pub struct UnavailableIntroPolicy;
#[async_trait]
impl IntroPolicy for UnavailableIntroPolicy {
    async fn check(
        &self,
        _: &mut PgConnection,
        _: &intro::IntroCommand,
    ) -> Result<(), intro::IntroError> {
        Err(intro::IntroError::Unavailable)
    }
    async fn edit_consent(
        &self,
        _: &mut PgConnection,
        _: &intro::IntroCommand,
        _: &intro::OwnerIntro,
    ) -> Result<(), intro::IntroError> {
        Err(intro::IntroError::Unavailable)
    }
}

pub trait IntroProfileProjection: Send + Sync {
    fn author(
        &self,
        connection: &mut PgConnection,
        event: Uuid,
        owner: Uuid,
    ) -> impl std::future::Future<Output = Result<intro::PublicAuthor, intro::IntroError>> + Send;
}
pub trait IntroProjectionExecutor: Send + Sync {
    /// Authorizes the empty/list request itself and retains its source locks through
    /// commit. Profile and claim rows are checked again by `project_public`.
    fn check_collection(
        &self,
        connection: &mut PgConnection,
        context: intro::IntroReadContext,
        now: DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<(), intro::IntroError>> + Send;
    fn check_owner(
        &self,
        connection: &mut PgConnection,
        context: intro::IntroReadContext,
        now: DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<(), intro::IntroError>> + Send;
    /// Revalidates every row in the source and owns the final acknowledgement.
    /// This is deliberately separate from BE26's current ClaimLease snapshot.
    fn project_public(
        &self,
        connection: &mut PgConnection,
        context: intro::IntroReadContext,
        row: &intro::PublicForumIntro,
        now: DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<(), intro::IntroError>> + Send;
    fn acknowledge(
        &self,
        connection: &mut PgConnection,
        context: intro::IntroReadContext,
        now: DateTime<Utc>,
    ) -> impl std::future::Future<Output = Result<(), intro::IntroError>> + Send;
    fn after_commit(
        &self,
    ) -> impl std::future::Future<Output = Result<(), intro::IntroError>> + Send {
        async { Ok(()) }
    }
}
pub struct UnavailableIntroProjection;
impl IntroProjectionExecutor for UnavailableIntroProjection {
    async fn check_collection(
        &self,
        _: &mut PgConnection,
        _: intro::IntroReadContext,
        _: DateTime<Utc>,
    ) -> Result<(), intro::IntroError> {
        Err(intro::IntroError::Unavailable)
    }
    async fn check_owner(
        &self,
        _: &mut PgConnection,
        _: intro::IntroReadContext,
        _: DateTime<Utc>,
    ) -> Result<(), intro::IntroError> {
        Err(intro::IntroError::Unavailable)
    }
    async fn project_public(
        &self,
        _: &mut PgConnection,
        _: intro::IntroReadContext,
        _: &intro::PublicForumIntro,
        _: DateTime<Utc>,
    ) -> Result<(), intro::IntroError> {
        Err(intro::IntroError::Unavailable)
    }
    async fn acknowledge(
        &self,
        _: &mut PgConnection,
        _: intro::IntroReadContext,
        _: DateTime<Utc>,
    ) -> Result<(), intro::IntroError> {
        Err(intro::IntroError::Unavailable)
    }
}
pub struct UnavailableIntroProfile;
impl IntroProfileProjection for UnavailableIntroProfile {
    async fn author(
        &self,
        _: &mut PgConnection,
        _: Uuid,
        _: Uuid,
    ) -> Result<intro::PublicAuthor, intro::IntroError> {
        Err(intro::IntroError::Unavailable)
    }
}

/// Lock ordering only; this does not grant actor authority. Write policies call
/// this before taking shared event/admission locks or the receipt-budget lock.
pub async fn lock_intro_event_for_write(
    c: &mut PgConnection,
    event: Uuid,
) -> Result<(), intro::IntroError> {
    let locked: Option<Uuid> = sqlx::query_scalar(
        "select event_id from public.concert_events where event_id=$1 for update",
    )
    .bind(event)
    .fetch_optional(c)
    .await
    .map_err(|_| intro::IntroError::Unavailable)?;
    locked.ok_or(intro::IntroError::Denied).map(|_| ())
}
#[derive(Clone)]
pub struct PgIntro<P, R, C, E = UnavailableIntroProjection> {
    pool: PgPool,
    policy: P,
    profiles: R,
    cursors: C,
    executor: E,
    keys: std::sync::Arc<FingerprintKeys>,
}
impl<P, R, C> PgIntro<P, R, C, UnavailableIntroProjection> {
    pub fn new(
        pool: PgPool,
        policy: P,
        profiles: R,
        cursors: C,
        keys: std::sync::Arc<FingerprintKeys>,
    ) -> Self {
        Self::with_executor(
            pool,
            policy,
            profiles,
            cursors,
            UnavailableIntroProjection,
            keys,
        )
    }
}
impl<P, R, C, E> PgIntro<P, R, C, E> {
    pub fn with_executor(
        pool: PgPool,
        policy: P,
        profiles: R,
        cursors: C,
        executor: E,
        keys: std::sync::Arc<FingerprintKeys>,
    ) -> Self {
        Self {
            pool,
            policy,
            profiles,
            cursors,
            executor,
            keys,
        }
    }
}

struct Producer<'a, P> {
    command: &'a intro::IntroCommand,
    policy: &'a P,
}
#[async_trait]
impl<P: IntroPolicy> CommandProducer for Producer<'_, P> {
    async fn authorize(
        &self,
        c: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(), CommandError> {
        if scope.actor != self.command.actor().uuid()
            || scope.event != self.command.event().uuid()
            || scope.key != self.command.key()
            || scope.operation.id() != self.command.operation()
            || target != self.command.intro()
        {
            return Err(CommandError::Denied);
        }
        self.policy.check(c, self.command).await.map_err(command)?;
        Ok(())
    }
    async fn apply(
        &self,
        c: &mut PgConnection,
        scope: Scope,
        target: Uuid,
    ) -> Result<(Completion, Option<Invalidation>), CommandError> {
        self.authorize(c, scope, target).await?;
        let event = scope.event;
        let actor = scope.actor;
        let phase: String = sqlx::query_scalar(
            "select phase from public.concert_events where event_id=$1 for update",
        )
        .bind(event)
        .fetch_one(&mut *c)
        .await
        .map_err(db)?;
        let ownership = self.command.operation() == "closeConcertIntro";
        if !ownership && phase == "READ_ONLY" {
            return Ok((Completion::Failed(SafeFailure::EventReadOnly), None));
        }
        if !ownership && !matches!(phase.as_str(), "OPEN" | "LIVE") {
            return Err(CommandError::Denied);
        }
        let db_now: DateTime<Utc> = sqlx::query_scalar("select statement_timestamp()")
            .fetch_one(&mut *c)
            .await
            .map_err(db)?;
        let event_cutoff: Option<DateTime<Utc>> = if ownership {
            None
        } else {
            let cutoff: Option<DateTime<Utc>> = sqlx::query_scalar(
                "select event_cutoff from public.concert_meeting_settings where event_id=$1 for share",
            )
            .bind(event)
            .fetch_optional(&mut *c)
            .await
            .map_err(db)?;
            Some(cutoff.ok_or(CommandError::Denied)?)
        };
        if event_cutoff.is_some_and(|cutoff| cutoff <= db_now) {
            return Err(CommandError::Denied);
        }
        let current = sqlx::query("select state,revision,created_at,expires_at,text from public.concert_intros where event_id=$1 and owner_id=$2 for update")
            .bind(event)
            .bind(actor)
            .fetch_optional(&mut *c)
            .await
            .map_err(db)?;
        let next = match self.command {
            intro::IntroCommand::Set {
                expected, draft, ..
            } => {
                let (minimum, maximum) = bounds(&draft.age_bounds)?;
                let current_revision = current
                    .as_ref()
                    .map(|r| r.try_get::<i64, _>("revision"))
                    .transpose()
                    .map_err(db)?;
                if current_revision != expected.revision() {
                    return Err(CommandError::Conflict);
                }
                let created: DateTime<Utc> = current
                    .as_ref()
                    .map(|r| r.try_get("created_at"))
                    .transpose()
                    .map_err(db)?
                    .unwrap_or(db_now);
                let revision = current_revision.unwrap_or(0) + 1;
                let state = current
                    .as_ref()
                    .map(|r| r.try_get::<String, _>("state"))
                    .transpose()
                    .map_err(db)?
                    .unwrap_or_else(|| "DRAFT".into());
                if state == "CLOSED" {
                    return Err(CommandError::InvalidState);
                }
                if draft
                    .expires_at
                    .is_some_and(|expiry| expiry > event_cutoff.expect("write cutoff"))
                {
                    return Err(CommandError::InvalidInput);
                }
                let next = intro::OwnerIntro {
                    intro_id: target,
                    event_id: event,
                    owner_id: actor,
                    text: draft.text.clone(),
                    age_bounds: draft.age_bounds.clone(),
                    expires_at: draft.expires_at,
                    created_at: created,
                    state: parse_state(&state).map_err(command)?,
                    revision: concert_domain::Revision::new(revision)
                        .map_err(|_| CommandError::InvalidState)?,
                };
                next.validate_at(db_now)
                    .map_err(|_| CommandError::InvalidInput)?;
                let insert = sqlx::query("insert into public.concert_intros(event_id,intro_id,owner_id,revision,state,text,minimum_age,maximum_age_exclusive,expires_at,created_at,updated_at) values($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,clock_timestamp()) on conflict(event_id,owner_id) do update set revision=excluded.revision,state=excluded.state,text=excluded.text,minimum_age=excluded.minimum_age,maximum_age_exclusive=excluded.maximum_age_exclusive,expires_at=excluded.expires_at,updated_at=clock_timestamp()")
                    .bind(event).bind(target).bind(actor).bind(revision).bind(&state).bind(&draft.text).bind(minimum).bind(maximum).bind(draft.expires_at).bind(created).execute(&mut *c).await;
                insert.map_err(db).map(|_| ())?;
                next
            }
            intro::IntroCommand::Publish { expected, .. } => {
                let row = current.ok_or(CommandError::Denied)?;
                let revision: i64 = row.try_get("revision").map_err(db)?;
                let state: String = row.try_get("state").map_err(db)?;
                let text: String = row.try_get("text").map_err(db)?;
                let expires_at: Option<chrono::DateTime<Utc>> =
                    row.try_get("expires_at").map_err(db)?;
                if revision != expected.get() {
                    return Err(CommandError::Conflict);
                }
                if state != "DRAFT" || text.trim().is_empty() {
                    return Err(CommandError::InvalidState);
                }
                if expires_at.is_some_and(|value| value <= db_now) {
                    return Err(CommandError::InvalidState);
                }
                sqlx::query("update public.concert_intros set state='PUBLISHED',revision=revision+1,updated_at=clock_timestamp() where event_id=$1 and intro_id=$2 and revision=$3")
                    .bind(event).bind(target).bind(revision).execute(&mut *c).await.map_err(db)?;
                load_owner(c, event, target, revision + 1).await?
            }
            intro::IntroCommand::Close { expected, .. } => {
                let row = current.ok_or(CommandError::Denied)?;
                let revision: i64 = row.try_get("revision").map_err(db)?;
                let state: String = row.try_get("state").map_err(db)?;
                if revision != expected.get() {
                    return Err(CommandError::Conflict);
                }
                if state == "CLOSED" {
                    return Err(CommandError::InvalidState);
                }
                sqlx::query("update public.concert_intros set state='CLOSED',revision=revision+1,updated_at=clock_timestamp() where event_id=$1 and intro_id=$2 and revision=$3")
                    .bind(event).bind(target).bind(revision).execute(&mut *c).await.map_err(db)?;
                load_owner(c, event, target, revision + 1).await?
            }
        };
        self.policy
            .edit_consent(c, self.command, &next)
            .await
            .map_err(command)?;
        let reference = SafeReference {
            resource: target,
            revision: next.revision.get(),
        };
        Ok((
            Completion::Succeeded {
                reference,
                status: 200,
            },
            Some(Invalidation { reference }),
        ))
    }
}

impl<
        P: IntroPolicy,
        R: IntroProfileProjection,
        C: cursor::CursorCodec,
        E: IntroProjectionExecutor,
    > PgIntro<P, R, C, E>
{
    pub async fn execute(
        &self,
        command: &intro::IntroCommand,
    ) -> Result<intro::OwnerCommandResult, intro::IntroError> {
        let request = command.request().map_err(|_| intro::IntroError::Invalid)?;
        let receipt = PgCommandRepository::new(self.pool.clone())
            .execute(
                &request,
                command.intro(),
                &self.keys,
                &Producer {
                    command,
                    policy: &self.policy,
                },
            )
            .await
            .map_err(error)?;
        if receipt.state != ReceiptState::Succeeded {
            return Err(receipt
                .failure
                .and_then(intro_failure)
                .unwrap_or(intro::IntroError::Unavailable));
        }
        let next = match self.owner(command).await {
            Ok(next) => next,
            Err(_) => return Err(intro::IntroError::Unavailable),
        };
        Ok(intro::OwnerCommandResult {
            receipt_id: receipt.id,
            state: command.operation(),
            revision: receipt
                .reference
                .map(|value| value.revision)
                .map(|v| concert_domain::Revision::new(v).expect("receipt revision validated")),
            owner: next,
        })
    }
    async fn owner(
        &self,
        command: &intro::IntroCommand,
    ) -> Result<intro::OwnerIntro, intro::IntroError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| intro::IntroError::Unavailable)?;
        self.policy.check(&mut tx, command).await?;
        let row = sqlx::query("select event_id,intro_id,owner_id,revision,state,text,minimum_age,maximum_age_exclusive,expires_at,created_at,updated_at from public.concert_intros where event_id=$1 and intro_id=$2 and owner_id=$3 for share")
            .bind(command.event().uuid()).bind(command.intro()).bind(command.actor().uuid()).fetch_optional(&mut *tx).await.map_err(|_| intro::IntroError::Unavailable)?
            .ok_or(intro::IntroError::NotFound)?;
        let owner = row_owner(&row)?;
        self.policy.check(&mut tx, command).await?;
        tx.commit()
            .await
            .map_err(|_| intro::IntroError::Unavailable)?;
        Ok(owner)
    }

    pub async fn own(
        &self,
        context: intro::IntroReadContext,
    ) -> Result<intro::OwnerIntro, intro::IntroError> {
        let read_now = db_now(&self.pool).await?;
        context.check(read_now)?;
        if context.actor.uuid().is_nil() || context.event.uuid().is_nil() {
            return Err(intro::IntroError::Denied);
        }
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| intro::IntroError::Unavailable)?;
        self.executor
            .check_owner(&mut tx, context, read_now)
            .await?;
        let row = sqlx::query(
            "select event_id,intro_id,owner_id,revision,state,text,minimum_age,maximum_age_exclusive,expires_at,created_at,updated_at from public.concert_intros where event_id=$1 and owner_id=$2 for share",
        )
        .bind(context.event.uuid())
        .bind(context.actor.uuid())
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| intro::IntroError::Unavailable)?
        .ok_or(intro::IntroError::NotFound)?;
        let owner = row_owner(&row)?;
        let final_now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| intro::IntroError::Unavailable)?;
        self.executor
            .acknowledge(&mut tx, context, final_now)
            .await?;
        context.check(final_now)?;
        tx.commit()
            .await
            .map_err(|_| intro::IntroError::Unavailable)?;
        self.executor.after_commit().await?;
        context.check(Utc::now())?;
        Ok(owner)
    }

    pub async fn discover(
        &self,
        context: intro::IntroReadContext,
        query: intro::wire::Query,
    ) -> Result<
        (
            Vec<(intro::PublicForumIntro, intro::PublicAuthor)>,
            Option<String>,
        ),
        intro::IntroError,
    > {
        // The fields remain public for retained callers; never trust them to
        // have passed the transport constructor.
        let query = intro::wire::Query::validate(Some(query.limit), query.cursor, query.q)?;
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| intro::IntroError::Unavailable)?;
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| intro::IntroError::Unavailable)?;
        context.check(now)?;
        self.executor
            .check_collection(&mut tx, context, now)
            .await?;
        let binding = cursor::CursorBinding {
            actor: context.actor.uuid(),
            event: context.event.uuid(),
            operation: "listConcertIntros",
            collection: "intros",
            query: query.q.as_ref().map(hash_query),
        };
        let window = match &query.cursor {
            None => cursor::CursorWindow {
                upper: now,
                after: None,
            },
            Some(token) => self.cursors.open(&binding, token, now)?,
        };
        cursor::validate_binding(&binding, window, now)?;
        let mut after = window.after;
        let mut items = vec![];
        let mut more = false;
        let mut scanned_count = 0_usize;
        'scan: loop {
            let rows = match after {
                Some(position) => sqlx::query(
                    "select event_id,intro_id,owner_id,text,created_at,revision,expires_at from public.concert_intros where event_id=$1 and state='PUBLISHED' and created_at<=$2 and (expires_at is null or expires_at>$2) and (created_at,intro_id)<($3,$4) order by created_at desc,intro_id desc limit $5",
                )
                .bind(context.event.uuid())
                .bind(window.upper)
                .bind(position.created_at)
                .bind(position.intro_id)
                .bind(101_i64)
                .fetch_all(&mut *tx)
                .await,
                None => sqlx::query(
                    "select event_id,intro_id,owner_id,text,created_at,revision,expires_at from public.concert_intros where event_id=$1 and state='PUBLISHED' and created_at<=$2 and (expires_at is null or expires_at>$2) order by created_at desc,intro_id desc limit $3",
                )
                .bind(context.event.uuid())
                .bind(window.upper)
                .bind(101_i64)
                .fetch_all(&mut *tx)
                .await,
            }
            .map_err(|_| intro::IntroError::Unavailable)?;
            if rows.is_empty() {
                break;
            }
            let row_count = rows.len();
            let exhausted = row_count < 101;
            for (index, row) in rows.into_iter().enumerate() {
                // Each candidate can enroll a distinct owner. Respect the real
                // root scope's subject bound even when every row is filtered.
                // Continue after the last scanned row, never the last result.
                if scanned_count
                    == crate::adapters::out::relationship_authority::guarded::MAX_SUBJECTS
                {
                    more = true;
                    break 'scan;
                }
                scanned_count += 1;
                let value = row_public(&row)?;
                after = Some(intro::Position {
                    created_at: value.created_at,
                    intro_id: value.intro_id,
                });
                match self
                    .executor
                    .project_public(&mut tx, context, &value, now)
                    .await
                {
                    Ok(()) => {}
                    Err(intro::IntroError::Denied | intro::IntroError::NotFound) => continue,
                    Err(error) => return Err(error),
                }
                let author = match self
                    .profiles
                    .author(&mut tx, context.event.uuid(), value.author.uuid())
                    .await
                {
                    Ok(author) => author,
                    Err(intro::IntroError::NotFound) => continue,
                    Err(error) => return Err(error),
                };
                let display_name = validate_public_profile(&author.json, value.author)?;
                intro::validated_public(&value, author.clone())?;
                if query.q.as_ref().is_some_and(|needle| {
                    !value.text.to_lowercase().contains(&needle.to_lowercase())
                        && !display_name.to_lowercase().contains(&needle.to_lowercase())
                }) {
                    continue;
                }
                items.push((value, author));
                if items.len() == usize::from(query.limit) {
                    more = index + 1 < row_count || !exhausted;
                    break 'scan;
                }
            }
            if exhausted {
                break;
            }
        }
        let final_now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| intro::IntroError::Unavailable)?;
        self.executor
            .acknowledge(&mut tx, context, final_now)
            .await?;
        let next = if more {
            Some(self.cursors.seal(
                &binding,
                cursor::CursorWindow {
                    upper: window.upper,
                    after,
                },
                now,
            )?)
        } else {
            None
        };
        context.check(final_now)?;
        tx.commit()
            .await
            .map_err(|_| intro::IntroError::Unavailable)?;
        self.executor.after_commit().await?;
        context.check(Utc::now())?;
        Ok((items, next))
    }

    pub async fn direct(
        &self,
        context: intro::IntroReadContext,
        id: Uuid,
    ) -> Result<(intro::PublicForumIntro, intro::PublicAuthor), intro::IntroError> {
        let mut tx = self
            .pool
            .begin()
            .await
            .map_err(|_| intro::IntroError::Unavailable)?;
        let now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| intro::IntroError::Unavailable)?;
        context.check(now)?;
        self.executor
            .check_collection(&mut tx, context, now)
            .await?;
        let row = sqlx::query(
            "select event_id,intro_id,owner_id,text,created_at,revision,expires_at from public.concert_intros where event_id=$1 and intro_id=$2 and state='PUBLISHED' and created_at<=clock_timestamp() and (expires_at is null or expires_at>clock_timestamp()) for share",
        )
        .bind(context.event.uuid())
        .bind(id)
        .fetch_optional(&mut *tx)
        .await
        .map_err(|_| intro::IntroError::Unavailable)?
        .ok_or(intro::IntroError::NotFound)?;
        let value = row_public(&row)?;
        if let Err(error) = self
            .executor
            .project_public(&mut tx, context, &value, now)
            .await
        {
            return match error {
                intro::IntroError::Denied | intro::IntroError::NotFound => {
                    Err(intro::IntroError::NotFound)
                }
                error => Err(error),
            };
        }
        let author = self
            .profiles
            .author(&mut tx, context.event.uuid(), value.author.uuid())
            .await?;
        validate_public_profile(&author.json, value.author)?;
        intro::validated_public(&value, author.clone())?;
        let final_now: DateTime<Utc> = sqlx::query_scalar("select clock_timestamp()")
            .fetch_one(&mut *tx)
            .await
            .map_err(|_| intro::IntroError::Unavailable)?;
        self.executor
            .acknowledge(&mut tx, context, final_now)
            .await?;
        context.check(final_now)?;
        tx.commit()
            .await
            .map_err(|_| intro::IntroError::Unavailable)?;
        self.executor.after_commit().await?;
        context.check(Utc::now())?;
        Ok((value, author))
    }
}

async fn load_owner(
    c: &mut PgConnection,
    event: Uuid,
    id: Uuid,
    revision: i64,
) -> Result<intro::OwnerIntro, CommandError> {
    let row = sqlx::query("select event_id,intro_id,owner_id,revision,state,text,minimum_age,maximum_age_exclusive,expires_at,created_at,updated_at from public.concert_intros where event_id=$1 and intro_id=$2 and revision=$3 for share")
        .bind(event).bind(id).bind(revision).fetch_optional(c).await.map_err(db)?
        .ok_or(CommandError::InvalidState)?;
    row_owner(&row).map_err(|_| CommandError::InvalidState)
}
fn row_owner(row: &sqlx::postgres::PgRow) -> Result<intro::OwnerIntro, intro::IntroError> {
    let minimum: Option<i16> = row
        .try_get("minimum_age")
        .map_err(|_| intro::IntroError::Unavailable)?;
    let maximum: Option<i16> = row
        .try_get("maximum_age_exclusive")
        .map_err(|_| intro::IntroError::Unavailable)?;
    let bounds = match (minimum, maximum) {
        (None, None) => "null:null".to_owned(),
        (Some(a), Some(b)) => format!("{a}:{b}"),
        _ => return Err(intro::IntroError::Invalid),
    };
    let state: String = row
        .try_get("state")
        .map_err(|_| intro::IntroError::Unavailable)?;
    Ok(intro::OwnerIntro {
        intro_id: row
            .try_get("intro_id")
            .map_err(|_| intro::IntroError::Unavailable)?,
        event_id: row
            .try_get("event_id")
            .map_err(|_| intro::IntroError::Unavailable)?,
        owner_id: row
            .try_get("owner_id")
            .map_err(|_| intro::IntroError::Unavailable)?,
        text: row
            .try_get("text")
            .map_err(|_| intro::IntroError::Unavailable)?,
        age_bounds: bounds,
        expires_at: row
            .try_get("expires_at")
            .map_err(|_| intro::IntroError::Unavailable)?,
        created_at: row
            .try_get("created_at")
            .map_err(|_| intro::IntroError::Unavailable)?,
        state: parse_state(&state)?,
        revision: concert_domain::Revision::new(
            row.try_get("revision")
                .map_err(|_| intro::IntroError::Unavailable)?,
        )
        .map_err(|_| intro::IntroError::Unavailable)?,
    })
}
pub(crate) fn row_public(
    row: &sqlx::postgres::PgRow,
) -> Result<intro::PublicForumIntro, intro::IntroError> {
    Ok(intro::PublicForumIntro {
        intro_id: row
            .try_get("intro_id")
            .map_err(|_| intro::IntroError::Unavailable)?,
        event_id: row
            .try_get("event_id")
            .map_err(|_| intro::IntroError::Unavailable)?,
        author: concert_domain::AuthUserId::from_uuid(
            row.try_get("owner_id")
                .map_err(|_| intro::IntroError::Unavailable)?,
        ),
        text: row
            .try_get("text")
            .map_err(|_| intro::IntroError::Unavailable)?,
        created_at: row
            .try_get("created_at")
            .map_err(|_| intro::IntroError::Unavailable)?,
        revision: concert_domain::Revision::new(
            row.try_get("revision")
                .map_err(|_| intro::IntroError::Unavailable)?,
        )
        .map_err(|_| intro::IntroError::Unavailable)?,
    })
}
fn hash_query(value: &String) -> u64 {
    let digest = Sha256::digest(value.as_bytes());
    u64::from_be_bytes(digest[..8].try_into().expect("fixed SHA-256 prefix"))
}
pub(crate) fn validate_public_profile(
    bytes: &[u8],
    owner: concert_domain::AuthUserId,
) -> Result<String, intro::IntroError> {
    if bytes.is_empty() || bytes.len() > intro::INTRO_PUBLIC_PROFILE_MAX {
        return Err(intro::IntroError::Unavailable);
    }
    let profile: Value =
        serde_json::from_slice(bytes).map_err(|_| intro::IntroError::Unavailable)?;
    // One canonical C03 validator covers both approved avatar representations.
    crate::adapters::out::public_profile::validate(&profile)
        .map_err(|_| intro::IntroError::Unavailable)?;
    if profile["user_id"].as_str() != Some(owner.uuid().to_string().as_str()) {
        return Err(intro::IntroError::Unavailable);
    }
    let display = profile["display_name"]
        .as_str()
        .ok_or(intro::IntroError::Unavailable)?;
    if display.trim().is_empty()
        || display.chars().any(char::is_control)
        || profile["bio"]
            .as_str()
            .is_some_and(|value| value.chars().any(char::is_control))
    {
        return Err(intro::IntroError::Unavailable);
    }
    profile["display_name"]
        .as_str()
        .map(str::to_owned)
        .ok_or(intro::IntroError::Unavailable)
}
pub fn owner_dto(value: &intro::OwnerIntro) -> Result<Value, intro::IntroError> {
    let (minimum, maximum) = value
        .age_bounds
        .split_once(':')
        .ok_or(intro::IntroError::Invalid)?;
    let bound = |field: &str| {
        if field == "null" {
            Ok(Value::Null)
        } else {
            field
                .parse::<i16>()
                .map(Value::from)
                .map_err(|_| intro::IntroError::Invalid)
        }
    };
    validate_owner_projection(value, Utc::now())?;
    Ok(json!({
        "intro_id": value.intro_id,
        "event_id": value.event_id,
        "owner_id": value.owner_id,
        "text": value.text,
        "age_bounds": {"minimum_age": bound(minimum)?, "maximum_age_exclusive": bound(maximum)?},
        "expires_at": value.expires_at,
        "created_at": value.created_at,
        "state": value.state,
        "revision": value.revision.get(),
    }))
}
pub fn public_dto(
    value: &intro::PublicForumIntro,
    author: intro::PublicAuthor,
) -> Result<Value, intro::IntroError> {
    intro::validated_public(value, author.clone())?;
    validate_public_profile(&author.json, value.author)?;
    let profile: Value =
        serde_json::from_slice(&author.json).map_err(|_| intro::IntroError::Unavailable)?;
    Ok(json!({
        "intro_id": value.intro_id,
        "event_id": value.event_id,
        "author": profile,
        "text": value.text,
        "created_at": value.created_at,
        "state": "PUBLISHED",
        "revision": value.revision.get(),
    }))
}
pub fn public_page_dto(
    items: Vec<(intro::PublicForumIntro, intro::PublicAuthor)>,
    next_cursor: Option<String>,
) -> Result<Value, intro::IntroError> {
    if items.len() > usize::from(intro::INTRO_PAGE_MAX)
        || next_cursor
            .as_ref()
            .is_some_and(|token| token.is_empty() || token.len() > intro::INTRO_CURSOR_MAX)
    {
        return Err(intro::IntroError::Invalid);
    }
    let items = items
        .into_iter()
        .map(|(row, author)| public_dto(&row, author))
        .collect::<Result<Vec<_>, _>>()?;
    Ok(json!({"items": items, "next_cursor": next_cursor}))
}
fn bounds(value: &str) -> Result<(Option<i16>, Option<i16>), CommandError> {
    let (minimum, maximum) = value.split_once(':').ok_or(CommandError::InvalidInput)?;
    let parse = |value: &str| {
        if value == "null" {
            Ok(None)
        } else {
            value
                .parse::<i16>()
                .map(Some)
                .map_err(|_| CommandError::InvalidInput)
        }
    };
    Ok((parse(minimum)?, parse(maximum)?))
}
fn parse_state(value: &str) -> Result<concert_domain::intro_policy::IntroState, intro::IntroError> {
    match value {
        "DRAFT" => Ok(concert_domain::intro_policy::IntroState::Draft),
        "PUBLISHED" => Ok(concert_domain::intro_policy::IntroState::Published),
        "CLOSED" => Ok(concert_domain::intro_policy::IntroState::Closed),
        _ => Err(intro::IntroError::Unavailable),
    }
}
fn command(value: intro::IntroError) -> CommandError {
    match value {
        intro::IntroError::Denied => CommandError::Denied,
        intro::IntroError::Invalid => CommandError::InvalidInput,
        intro::IntroError::Conflict => CommandError::Conflict,
        intro::IntroError::Expired => CommandError::Expired,
        intro::IntroError::ReadOnly => CommandError::Denied,
        intro::IntroError::Unavailable | intro::IntroError::NotFound => CommandError::Unavailable,
    }
}
fn error(value: CommandError) -> intro::IntroError {
    match value {
        CommandError::Denied => intro::IntroError::Denied,
        CommandError::InvalidInput => intro::IntroError::Invalid,
        CommandError::Conflict => intro::IntroError::Conflict,
        CommandError::InvalidState => intro::IntroError::Conflict,
        CommandError::Expired => intro::IntroError::Expired,
        _ => intro::IntroError::Unavailable,
    }
}
fn intro_failure(value: SafeFailure) -> Option<intro::IntroError> {
    match value {
        SafeFailure::InvalidTransition => Some(intro::IntroError::Conflict),
        SafeFailure::VersionConflict => Some(intro::IntroError::Conflict),
        SafeFailure::DependencyUnavailable => Some(intro::IntroError::Unavailable),
        SafeFailure::InvalidInput => Some(intro::IntroError::Invalid),
        SafeFailure::EventReadOnly => Some(intro::IntroError::ReadOnly),
        _ => None,
    }
}
fn db(error: sqlx::Error) -> CommandError {
    let _ = error;
    CommandError::Unavailable
}
async fn db_now(pool: &PgPool) -> Result<DateTime<Utc>, intro::IntroError> {
    sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(pool)
        .await
        .map_err(|_| intro::IntroError::Unavailable)
}

fn validate_owner_projection(
    value: &intro::OwnerIntro,
    now: DateTime<Utc>,
) -> Result<(), intro::IntroError> {
    // Validate stored private shape without imposing public expiry on owner recovery.
    intro::OwnerIntro {
        intro_id: value.intro_id,
        event_id: value.event_id,
        owner_id: value.owner_id,
        text: value.text.clone(),
        age_bounds: value.age_bounds.clone(),
        expires_at: None,
        created_at: value.created_at,
        state: value.state,
        revision: value.revision,
    }
    .validate_at(now)
}
