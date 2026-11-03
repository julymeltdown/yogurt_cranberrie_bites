use super::event_outbox::PgEventOutbox;
use crate::application::dto::delivery_schedule_dto::{
    DeliveryLeaseOutcomeDto, DeliveryLeaseStatusDto, DeliveryScheduleConfigDto,
    TargetDeliveryBatchDto, TargetDeliveryClaimDto,
};
use crate::application::ports::DeliverySchedulePort;
use crate::domain::post::DeliveryTarget;
use anyhow::{ensure, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sqlx::{PgPool, Row};
use std::sync::Arc;
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Clone, Copy)]
struct ProgressScanPosition {
    expiry: DateTime<Utc>,
    event: Uuid,
}

#[derive(Clone, Copy)]
struct DependencySweep {
    after: Option<ProgressScanPosition>,
    ceiling: ProgressScanPosition,
}

#[derive(Clone)]
pub struct PgDeliverySchedule {
    pool: PgPool,
    // Scheduling position only, never ownership/effect evidence. Clones share
    // it; a restart safely begins another sweep of durable unchecked identities.
    progress_scan_position: Arc<Mutex<Option<ProgressScanPosition>>>,
    dependency_scan_position: Arc<Mutex<Option<DependencySweep>>>,
}

impl PgDeliverySchedule {
    pub fn new(pool: PgPool) -> Self {
        Self {
            pool,
            progress_scan_position: Arc::new(Mutex::new(None)),
            dependency_scan_position: Arc::new(Mutex::new(None)),
        }
    }

    async fn progress_candidates(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        limit: i32,
    ) -> Result<Vec<Uuid>> {
        use super::timeline_progress_sql::{AFTER_CANDIDATES_SQL, CANDIDATES_SQL};
        let mut position = self.progress_scan_position.lock().await;
        let mut rows = if let Some(ProgressScanPosition { expiry, event }) = *position {
            sqlx::query(AFTER_CANDIDATES_SQL)
                .bind(i64::from(limit.min(100)))
                .bind(expiry)
                .bind(event)
                .fetch_all(&mut **tx)
                .await?
        } else {
            sqlx::query(CANDIDATES_SQL)
                .bind(i64::from(limit.min(100)))
                .fetch_all(&mut **tx)
                .await?
        };
        if rows.is_empty() && position.is_some() {
            rows = sqlx::query(CANDIDATES_SQL)
                .bind(i64::from(limit.min(100)))
                .fetch_all(&mut **tx)
                .await?;
        }
        // Advance over selected candidates even when SKIP LOCKED could not
        // acquire their parents. Otherwise one busy oldest row starves the
        // independent maintenance window. Failed transactions lose no durable
        // work: untouched candidates remain indexed and are visited on wrap.
        *position = rows.last().map(|row| ProgressScanPosition {
            expiry: row.get("candidate_expiry"),
            event: row.get("candidate_event_id"),
        });
        Ok(rows
            .iter()
            .filter_map(|row| row.get::<Option<Uuid>, _>("event_id"))
            .collect())
    }

    async fn dependency_candidates(
        &self,
        tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
        older_than: DateTime<Utc>,
        limit: i32,
    ) -> Result<Vec<Uuid>> {
        use super::timeline_dependency_sql::{AFTER_CANDIDATES_SQL, CANDIDATES_SQL};
        let mut position = self.dependency_scan_position.lock().await;
        // A fixed sweep ceiling guarantees wrap even if producers keep adding
        // newer events. It is a fairness boundary, never a source watermark.
        for _ in 0..2 {
            if position.is_none() {
                let ceiling = sqlx::query("select created_at,event_id from post_event_outbox where not published and created_at<=$1 order by created_at desc,event_id desc limit 1")
                    .bind(older_than).fetch_optional(&mut **tx).await?;
                let Some(ceiling) = ceiling else {
                    return Ok(Vec::new());
                };
                *position = Some(DependencySweep {
                    after: None,
                    ceiling: ProgressScanPosition {
                        expiry: ceiling.get("created_at"),
                        event: ceiling.get("event_id"),
                    },
                });
            }
            let sweep = position.expect("dependency sweep initialized");
            let mut query = sqlx::query(if sweep.after.is_some() {
                AFTER_CANDIDATES_SQL
            } else {
                CANDIDATES_SQL
            })
            .bind("TIMELINE")
            .bind(older_than)
            .bind(100i64)
            .bind(sweep.ceiling.expiry)
            .bind(sweep.ceiling.event);
            if let Some(after) = sweep.after {
                query = query.bind(after.expiry).bind(after.event);
            }
            let rows = query.fetch_all(&mut **tx).await?;
            if !rows.is_empty() {
                let mut selected = Vec::new();
                let mut last = rows.last().expect("nonempty candidate window");
                for row in &rows {
                    if let Some(id) = row.get::<Option<Uuid>, _>("event_id") {
                        selected.push(id);
                        if selected.len() == limit.min(100) as usize {
                            // Do not seek past ready heads that were not handed
                            // to this caller. The worker requests one lease just
                            // before its effect, while scanning up to100 waits.
                            last = row;
                            break;
                        }
                    }
                }
                *position = Some(DependencySweep {
                    after: Some(ProgressScanPosition {
                        expiry: last.get("candidate_created_at"),
                        event: last.get("candidate_event_id"),
                    }),
                    ..sweep
                });
                return Ok(selected);
            }
            // Advance over waits and locks, then restart only when this bounded
            // sweep is exhausted. Rollback leaves every candidate durable.
            *position = None;
        }
        Ok(Vec::new())
    }

    async fn finish(
        &self,
        claim: &TargetDeliveryClaimDto,
        success: bool,
    ) -> Result<DeliveryLeaseOutcomeDto> {
        let mut tx = self.pool.begin().await?;
        // Every path locks parent before job. No lock is held during network I/O.
        let event =
            sqlx::query("select payload from post_event_outbox where event_id=$1 for update")
                .bind(claim.event.event_id)
                .fetch_optional(&mut *tx)
                .await?;
        let Some(event) = event else {
            return Ok(DeliveryLeaseOutcomeDto::Lost);
        };
        // UPDATE may evaluate its expiry predicate before waiting for the job
        // row lock. Acquire that lock first, then evaluate the fencing CAS in a
        // separate statement using the current database clock.
        let job = sqlx::query("select event_id,lease_kind,replay_restore_state from post_event_delivery_jobs where event_id=$1 and target=$2 for update")
            .bind(claim.event.event_id).bind(claim.target.as_str()).fetch_optional(&mut *tx).await?;
        let Some(job) = job else {
            return Ok(DeliveryLeaseOutcomeDto::Lost);
        };
        let completed_replay = job.get::<String, _>("lease_kind") == "REPLAY"
            && job
                .get::<Option<String>, _>("replay_restore_state")
                .is_some_and(|state| matches!(state.as_str(), "SUCCEEDED" | "SKIPPED"));
        let updated = sqlx::query(
            r#"update post_event_delivery_jobs
               set state=case when $5 then 'SUCCEEDED'
                              when lease_kind='REPLAY' and replay_restore_state in ('SUCCEEDED','SKIPPED') then replay_restore_state
                              when attempt_count-progress_credits>=max_attempts then 'DEAD' else 'RETRY' end,
                   next_attempt_at=case when $5 or (lease_kind='REPLAY' and replay_restore_state in ('SUCCEEDED','SKIPPED')) then next_attempt_at else
                       clock_timestamp() + (greatest(1,floor(least(retry_cap_ms::double precision,
                           retry_base_ms::double precision * power(2.0,least(greatest(attempt_count-progress_credits-1,0),31)))
                           * (0.75 + random()*0.25))) * interval '1 millisecond') end,
                   lease_token=null,attempt_id=null,lease_expires_at=null,lease_kind='AUTO',replay_restore_state=null
               where event_id=$1 and target=$2 and lease_token=$3 and attempt_id=$4
                 and state='LEASED' and lease_expires_at>clock_timestamp()
               returning attempt_count,state"#,
        )
        .bind(claim.event.event_id)
        .bind(claim.target.as_str())
        .bind(claim.lease_token)
        .bind(claim.attempt_id)
        .bind(success)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(updated) = updated else {
            return Ok(DeliveryLeaseOutcomeDto::Lost);
        };
        // Source and resolution come from the immutable STARTED marker, not
        // caller-supplied fields in the returned claim DTO.
        let terminal = sqlx::query(
            r#"insert into post_event_delivery_attempts(attempt_id,event_id,target,source,phase,occurred_at,resolution_id)
               select attempt_id,event_id,target,source,$2,clock_timestamp(),resolution_id
               from post_event_delivery_attempts
               where attempt_id=$1 and phase='STARTED' and event_id=$3 and target=$4"#,
        ).bind(claim.attempt_id).bind(if success { "SUCCEEDED" } else { "FAILED" })
         .bind(claim.event.event_id).bind(claim.target.as_str()).execute(&mut *tx).await?;
        ensure!(
            terminal.rows_affected() == 1,
            "delivery lease has no matching STARTED marker"
        );
        if success {
            sqlx::query("insert into event_publications(event_id,target,published_at,payload) values($1,$2,clock_timestamp(),$3) on conflict(event_id,target) do nothing")
                .bind(claim.event.event_id).bind(claim.target.as_str()).bind(event.get::<String,_>("payload")).execute(&mut *tx).await?;
        } else if !completed_replay {
            let dead = updated.get::<String, _>("state") == "DEAD";
            sqlx::query("update post_event_outbox set attempt_count=greatest(attempt_count,$2),last_error='TARGET_DELIVERY_FAILED',dead_lettered_at=case when $3 then coalesce(dead_lettered_at,clock_timestamp()) else dead_lettered_at end where event_id=$1")
                .bind(claim.event.event_id).bind(updated.get::<i32,_>("attempt_count")).bind(dead).execute(&mut *tx).await?;
        }
        let published = finalize_parent(&mut tx, claim.event.event_id).await?;
        tx.commit().await?;
        Ok(DeliveryLeaseOutcomeDto::Recorded {
            event_published: published,
        })
    }
}

async fn expire_replay_leases(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_id: Uuid,
) -> Result<()> {
    credit_timeline_progress(tx, event_id).await?;
    // Expiry never manufactures terminal history. Completed target state and
    // its automatic budget survive an uncertain explicit replay attempt.
    let restored=sqlx::query("update post_event_delivery_jobs set state=case when replay_restore_state in ('SUCCEEDED','SKIPPED') then replay_restore_state when attempt_count-progress_credits>=max_attempts then 'DEAD' else 'RETRY' end,lease_kind='AUTO',replay_restore_state=null,lease_token=null,attempt_id=null,lease_expires_at=null where event_id=$1 and state='LEASED' and lease_kind='REPLAY' and lease_expires_at<=clock_timestamp() returning state")
        .bind(event_id).fetch_all(&mut **tx).await?;
    if restored
        .iter()
        .any(|row| row.get::<String, _>("state") == "DEAD")
    {
        sqlx::query("update post_event_outbox set dead_lettered_at=coalesce(dead_lettered_at,clock_timestamp()),last_error='TARGET_LEASE_BUDGET_EXHAUSTED' where event_id=$1")
            .bind(event_id).execute(&mut **tx).await?;
    }
    Ok(())
}

#[async_trait]
impl crate::application::ports::ReplaySchedulePort for PgDeliverySchedule {
    async fn claim_replay_target(
        &self,
        event_id: Uuid,
        target: DeliveryTarget,
        config: &DeliveryScheduleConfigDto,
    ) -> Result<TargetDeliveryClaimDto> {
        config.validate()?;
        ensure!(
            matches!(target, DeliveryTarget::Internal | DeliveryTarget::External),
            "unsupported replay target"
        );
        ensure!(
            target != DeliveryTarget::External || config.external_enabled,
            "external replay publisher is disabled"
        );
        let mut tx = self.pool.begin().await?;
        let row = sqlx::query("select * from post_event_outbox where event_id=$1 for update")
            .bind(event_id)
            .fetch_optional(&mut *tx)
            .await?;
        let row = row.ok_or_else(|| anyhow::anyhow!("replay event not found"))?;
        let required: Option<Vec<String>> = row.get("delivery_required_targets");
        ensure!(
            row.get::<Option<DateTime<Utc>>, _>("locked_at").is_none(),
            "legacy event claim conflicts with replay"
        );
        ensure!(
            required.is_some()
                || row
                    .get::<Option<DateTime<Utc>>, _>("dead_lettered_at")
                    .is_none(),
            "dead letter requires audited resolution"
        );
        // Parent first, then all jobs in the same deterministic order used by
        // audited recovery. Read clock/expiry only after any lock wait.
        let jobs=sqlx::query("select target from post_event_delivery_jobs where event_id=$1 order by target for update")
            .bind(event_id).fetch_all(&mut *tx).await?;
        ensure!(jobs.len() == 4, "replay scheduling jobs missing");
        expire_replay_leases(&mut tx, event_id).await?;
        let exhausted:bool=sqlx::query_scalar("select exists(select 1 from post_event_delivery_jobs j where event_id=$1 and ($2::text[] is null or j.target=any($2)) and not exists(select 1 from event_publications p where p.event_id=j.event_id and p.target=j.target) and (state='DEAD' or attempt_count-progress_credits>=max_attempts))")
            .bind(event_id).bind(&required).fetch_one(&mut *tx).await?;
        ensure!(!exhausted, "dead letter requires audited resolution");
        if required.is_none() {
            let mut mask = vec!["TIMELINE", "CACHE", "INTERNAL"];
            if config.external_enabled {
                mask.push("EXTERNAL");
            }
            sqlx::query(
                "update post_event_outbox set delivery_required_targets=$2 where event_id=$1",
            )
            .bind(event_id)
            .bind(mask)
            .execute(&mut *tx)
            .await?;
            sqlx::query("update post_event_delivery_jobs set max_attempts=$2,retry_base_ms=$3,retry_cap_ms=$4 where event_id=$1")
                .bind(event_id).bind(config.max_attempts).bind(config.retry_base_ms as i64).bind(config.retry_cap_ms as i64).execute(&mut *tx).await?;
        }
        let job=sqlx::query("select *,next_attempt_at<=clock_timestamp() as due,lease_expires_at>clock_timestamp() as live from post_event_delivery_jobs where event_id=$1 and target=$2")
            .bind(event_id).bind(target.as_str()).fetch_one(&mut *tx).await?;
        let mut state: String = job.get("state");
        if state == "LEASED" {
            ensure!(
                job.get::<Option<bool>, _>("live") != Some(true),
                "live target lease conflicts with replay"
            );
            state = "RETRY".into();
        }
        ensure!(
            matches!(
                state.as_str(),
                "PENDING" | "RETRY" | "SUCCEEDED" | "SKIPPED"
            ),
            "dead letter requires audited resolution"
        );
        let completed = matches!(state.as_str(), "SUCCEEDED" | "SKIPPED");
        if !completed {
            ensure!(
                job.get::<i32, _>("attempt_count") - job.get::<i32, _>("progress_credits")
                    < job.get::<i32, _>("max_attempts"),
                "dead letter requires audited resolution"
            );
            ensure!(job.get::<bool, _>("due"), "target retry is not due");
            let blocked:bool=sqlx::query_scalar(r#"select exists(select 1 from post_event_outbox earlier join post_event_delivery_jobs previous on previous.event_id=earlier.event_id
                where earlier.aggregate_id=$1 and earlier.event_id<>$2 and previous.target=$3
                  and previous.state not in ('SUCCEEDED','SKIPPED')
                       and not(previous.state='LEASED' and previous.lease_kind='REPLAY' and previous.replay_restore_state in ('SUCCEEDED','SKIPPED'))
                  and not exists(select 1 from event_publications p where p.event_id=previous.event_id and p.target=previous.target)
                  and (($4::bigint is null and earlier.aggregate_version is null and (earlier.created_at,earlier.event_id)<($5,$2))
                    or ($4::bigint is not null and earlier.aggregate_version is null)
                    or ($4::bigint is not null and earlier.aggregate_version<$4)))"#)
                .bind(row.get::<Uuid,_>("aggregate_id")).bind(event_id).bind(target.as_str())
                .bind(row.get::<Option<i64>,_>("aggregate_version")).bind(row.get::<DateTime<Utc>,_>("created_at"))
                .fetch_one(&mut *tx).await?;
            ensure!(!blocked, "earlier target head must complete before replay");
        }
        let token = Uuid::new_v4();
        let attempt = Uuid::new_v4();
        let lease=sqlx::query("update post_event_delivery_jobs set state='LEASED',lease_kind='REPLAY',replay_restore_state=$3,attempt_count=attempt_count+case when $4 then 0 else 1 end,lease_token=$5,attempt_id=$6,lease_expires_at=clock_timestamp()+($7*interval '1 millisecond') where event_id=$1 and target=$2 returning attempt_count,lease_expires_at")
            .bind(event_id).bind(target.as_str()).bind(&state).bind(completed).bind(token).bind(attempt)
            .bind(config.lease_duration_ms as i64).fetch_one(&mut *tx).await?;
        sqlx::query("insert into post_event_delivery_attempts(attempt_id,event_id,target,source,phase,occurred_at,resolution_id) values($1,$2,$3,'ADMIN_REPLAY','STARTED',clock_timestamp(),$4)")
            .bind(attempt).bind(event_id).bind(target.as_str()).bind(job.get::<Option<Uuid>,_>("release_resolution_id"))
            .execute(&mut *tx).await?;
        let event = PgEventOutbox::map_event(row);
        let claim = TargetDeliveryClaimDto {
            event,
            target,
            lease_token: token,
            attempt_id: attempt,
            attempt_number: lease.get("attempt_count"),
            lease_expires_at: lease.get("lease_expires_at"),
        };
        tx.commit().await?;
        Ok(claim)
    }
}

async fn finalize_parent(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_id: Uuid,
) -> Result<bool> {
    Ok(sqlx::query(
        r#"update post_event_outbox event set published=true,published_at=clock_timestamp(),
                  locked_at=null,last_error=null,dead_lettered_at=null
           where event_id=$1 and not published and delivery_required_targets is not null
             and not exists(select 1 from unnest(event.delivery_required_targets) required(target)
                            left join post_event_delivery_jobs job on job.event_id=event.event_id and job.target=required.target
                            where job.state is distinct from 'SUCCEEDED'
                              and not(job.state='LEASED' and job.lease_kind='REPLAY' and job.replay_restore_state='SUCCEEDED' and exists(select 1 from event_publications p where p.event_id=job.event_id and p.target=job.target)))"#,
    ).bind(event_id).execute(&mut **tx).await?.rows_affected()==1)
}

#[async_trait]
impl DeliverySchedulePort for PgDeliverySchedule {
    async fn lease_status(&self, claim: &TargetDeliveryClaimDto) -> Result<DeliveryLeaseStatusDto> {
        let remaining: Option<i64> = sqlx::query_scalar(
            "select floor(extract(epoch from (lease_expires_at-clock_timestamp()))*1000)::bigint from post_event_delivery_jobs where event_id=$1 and target=$2 and state='LEASED' and lease_token=$3 and attempt_id=$4"
        ).bind(claim.event.event_id).bind(claim.target.as_str()).bind(claim.lease_token).bind(claim.attempt_id).fetch_optional(&self.pool).await?;
        Ok(match remaining {
            Some(ms) if ms > 0 => DeliveryLeaseStatusDto::Owned {
                remaining_ms: ms as u64,
            },
            _ => DeliveryLeaseStatusDto::Lost,
        })
    }
    async fn claim_target(
        &self,
        target: DeliveryTarget,
        older_than: DateTime<Utc>,
        limit: i32,
        config: &DeliveryScheduleConfigDto,
    ) -> Result<TargetDeliveryBatchDto> {
        config.validate()?;
        ensure!(
            (1..=10_000).contains(&limit),
            "invalid delivery claim limit"
        );
        let mut tx = self.pool.begin().await?;
        // Durable positive catch-up is maintained independently of head order.
        // It cannot be dead-lettered just because a later event's committed
        // prefix required several original effect deadlines.
        let progressed = self.progress_candidates(&mut tx, limit).await?;
        for event_id in progressed {
            credit_timeline_progress(&mut tx, event_id).await?;
        }
        // Bounded maintenance also restores completed forced replay leases on
        // published parents; recovery must not rely on an unpublished scan.
        let expired:Vec<Uuid>=sqlx::query_scalar("select event.event_id from post_event_outbox event join post_event_delivery_jobs job on job.event_id=event.event_id where job.target=$1 and job.state='LEASED' and job.lease_kind='REPLAY' and job.replay_restore_state in ('SUCCEEDED','SKIPPED') and job.lease_expires_at<=clock_timestamp() order by job.lease_expires_at,event.event_id for update of event skip locked limit $2")
            .bind(target.as_str()).bind(i64::from(limit)).fetch_all(&mut *tx).await?;
        for id in expired {
            expire_replay_leases(&mut tx, id).await?;
        }
        // Bound preparation to target heads as well as the claim itself;
        // blocked aggregate successors must not starve unrelated heads.
        let dependency_scheduling: bool = target == DeliveryTarget::Timeline
            && sqlx::query_scalar::<_, bool>(
                "select to_regprocedure('public.post_timeline_affinity_ready(uuid)') is not null",
            )
            .fetch_one(&mut *tx)
            .await?;
        let parents: Vec<Uuid> = if dependency_scheduling {
            self.dependency_candidates(&mut tx, older_than, limit)
                .await?
        } else {
            sqlx::query_scalar(
            r#"select event.event_id from post_event_outbox event
               join post_event_delivery_jobs job on job.event_id=event.event_id and job.target=$1
               where not event.published and event.created_at<=$2
                 and (event.locked_at is null or event.locked_at<=clock_timestamp()-interval '5 minutes')
                 and ((job.state in ('PENDING','RETRY') and job.next_attempt_at<=clock_timestamp())
                      or (job.state='LEASED' and job.lease_expires_at<=clock_timestamp())
                      or (job.state in ('SUCCEEDED','SKIPPED') and not exists (
                          select 1 from post_event_delivery_jobs outstanding
                          where outstanding.event_id=event.event_id
                            and (event.delivery_required_targets is null
                                 or outstanding.target=any(event.delivery_required_targets))
                            and outstanding.state<>'SUCCEEDED')))
                 and not exists(
                     select 1 from post_event_outbox earlier join post_event_delivery_jobs previous
                       on previous.event_id=earlier.event_id and previous.target=job.target
                     where earlier.aggregate_id=event.aggregate_id and earlier.event_id<>event.event_id
                       and previous.state not in ('SUCCEEDED','SKIPPED')
                       and not(previous.state='LEASED' and previous.lease_kind='REPLAY' and previous.replay_restore_state in ('SUCCEEDED','SKIPPED'))
                       and not exists(select 1 from event_publications p where p.event_id=previous.event_id and p.target=previous.target)
                       and ((event.aggregate_version is null and earlier.aggregate_version is null and (earlier.created_at,earlier.event_id)<(event.created_at,event.event_id))
                            or (event.aggregate_version is not null and earlier.aggregate_version is null)
                            or (event.aggregate_version is not null and earlier.aggregate_version is not null and earlier.aggregate_version<event.aggregate_version))
                 )
               order by event.created_at,event.event_id for update of event skip locked limit $3"#,
        ).bind(target.as_str()).bind(older_than).bind(i64::from(limit)).fetch_all(&mut *tx).await?
        };
        let mut required = vec!["TIMELINE", "CACHE", "INTERNAL"];
        if config.external_enabled {
            required.push("EXTERNAL");
        }
        let mut reconciled_published_events = 0;
        for &id in &parents {
            expire_replay_leases(&mut tx, id).await?;
            let initialized=sqlx::query("update post_event_outbox set delivery_required_targets=$2,locked_at=null where event_id=$1 and delivery_required_targets is null")
                .bind(id).bind(&required).execute(&mut *tx).await?.rows_affected()==1;
            if initialized {
                sqlx::query("update post_event_delivery_jobs set max_attempts=$2,retry_base_ms=$3,retry_cap_ms=$4 where event_id=$1")
                    .bind(id).bind(config.max_attempts).bind(config.retry_base_ms as i64).bind(config.retry_cap_ms as i64).execute(&mut *tx).await?;
            }
            // A real legacy/admin checkpoint is authoritative. Reconciliation
            // cancels any lease rather than letting its old token rewrite state.
            sqlx::query("update post_event_delivery_jobs job set state='SUCCEEDED',lease_token=null,attempt_id=null,lease_expires_at=null where event_id=$1 and state<>'SUCCEEDED' and not(state='LEASED' and lease_kind='REPLAY') and exists(select 1 from event_publications p where p.event_id=job.event_id and p.target=job.target)")
                .bind(id).execute(&mut *tx).await?;
            sqlx::query("update post_event_delivery_jobs job set state='SKIPPED',lease_token=null,attempt_id=null,lease_expires_at=null from post_event_outbox event where job.event_id=$1 and event.event_id=job.event_id and not(job.target=any(event.delivery_required_targets)) and job.state<>'SUCCEEDED' and not(job.state='LEASED' and job.lease_kind='REPLAY')")
                .bind(id).execute(&mut *tx).await?;
            // A replay can expire during a preparation/row-lock wait after
            // the first cleanup. Restore it through its own state machine;
            // generic automatic exhaustion must never transition REPLAY jobs.
            expire_replay_leases(&mut tx, id).await?;
            // Expiry proves uncertainty, not a FAILED delivery. Keep the old
            // STARTED marker unmatched when its budget is exhausted.
            let exhausted=sqlx::query("update post_event_delivery_jobs set state='DEAD',lease_token=null,attempt_id=null,lease_expires_at=null where event_id=$1 and state='LEASED' and lease_kind='AUTO' and lease_expires_at<=clock_timestamp() and attempt_count-progress_credits>=max_attempts")
                .bind(id).execute(&mut *tx).await?.rows_affected()>0;
            if exhausted {
                sqlx::query("update post_event_outbox set dead_lettered_at=coalesce(dead_lettered_at,clock_timestamp()),last_error='TARGET_LEASE_BUDGET_EXHAUSTED' where event_id=$1")
                    .bind(id).execute(&mut *tx).await?;
            }
            if finalize_parent(&mut tx, id).await? {
                reconciled_published_events += 1;
            }
        }
        let rows=sqlx::query(
            r#"select event.event_id,event.event_type,event.aggregate_id,event.actor_id,event.attributes,
                      event.payload,event.occurred_at,event.created_at,event.published,event.published_at,
                      event.aggregate_version,event.replay_resolution_id,event.origin_traceparent,job.release_resolution_id
               from post_event_outbox event join post_event_delivery_jobs job on job.event_id=event.event_id
               where event.event_id=any($1) and job.target=$2 and not event.published
                 and job.target=any(event.delivery_required_targets) and job.attempt_count-job.progress_credits<job.max_attempts
                 and ((job.state in ('PENDING','RETRY') and job.next_attempt_at<=clock_timestamp())
                      or (job.state='LEASED' and job.lease_expires_at<=clock_timestamp()))
                 and not exists(
                     select 1 from post_event_outbox earlier join post_event_delivery_jobs previous
                       on previous.event_id=earlier.event_id and previous.target=job.target
                     where earlier.aggregate_id=event.aggregate_id and earlier.event_id<>event.event_id
                       and previous.state not in ('SUCCEEDED','SKIPPED')
                       and not(previous.state='LEASED' and previous.lease_kind='REPLAY' and previous.replay_restore_state in ('SUCCEEDED','SKIPPED'))
                       and not exists(select 1 from event_publications p where p.event_id=previous.event_id and p.target=previous.target)
                       and ((event.aggregate_version is null and earlier.aggregate_version is null and (earlier.created_at,earlier.event_id)<(event.created_at,event.event_id))
                            or (event.aggregate_version is not null and earlier.aggregate_version is null)
                            or (event.aggregate_version is not null and earlier.aggregate_version is not null and earlier.aggregate_version<event.aggregate_version))
                 )
               order by event.created_at,event.event_id for update of job skip locked"#,
        ).bind(&parents).bind(target.as_str()).fetch_all(&mut *tx).await?;
        let mut claims = Vec::with_capacity(rows.len());
        for row in rows {
            // Recheck after parent/job acquisition in a fresh statement. This
            // remains a read-only hint; the projection resolver owns all effects.
            if dependency_scheduling
                && !sqlx::query_scalar::<_, bool>("select public.post_timeline_affinity_ready($1)")
                    .bind(row.get::<Uuid, _>("event_id"))
                    .fetch_one(&mut *tx)
                    .await?
            {
                continue;
            }
            let resolution_id: Option<Uuid> = row.get("release_resolution_id");
            let event = PgEventOutbox::map_event(row);
            let token = Uuid::new_v4();
            let attempt = Uuid::new_v4();
            let lease=sqlx::query("update post_event_delivery_jobs set state='LEASED',attempt_count=attempt_count+1,lease_token=$3,attempt_id=$4,lease_expires_at=clock_timestamp()+($5*interval '1 millisecond') where event_id=$1 and target=$2 returning attempt_count,lease_expires_at")
                .bind(event.event_id).bind(target.as_str()).bind(token).bind(attempt).bind(config.lease_duration_ms as i64).fetch_one(&mut *tx).await?;
            sqlx::query("insert into post_event_delivery_attempts(attempt_id,event_id,target,source,phase,occurred_at,resolution_id) values($1,$2,$3,$4,'STARTED',clock_timestamp(),$5)")
                .bind(attempt).bind(event.event_id).bind(target.as_str()).bind(if resolution_id.is_some(){"ADMIN_REPLAY"}else{"OUTBOX_WORKER"}).bind(resolution_id).execute(&mut *tx).await?;
            claims.push(TargetDeliveryClaimDto {
                event,
                target,
                lease_token: token,
                attempt_id: attempt,
                attempt_number: lease.get("attempt_count"),
                lease_expires_at: lease.get("lease_expires_at"),
            });
        }
        tx.commit().await?;
        Ok(TargetDeliveryBatchDto {
            claims,
            reconciled_published_events,
        })
    }
    async fn complete_target(
        &self,
        claim: &TargetDeliveryClaimDto,
    ) -> Result<DeliveryLeaseOutcomeDto> {
        self.finish(claim, true).await
    }
    async fn fail_target(&self, claim: &TargetDeliveryClaimDto) -> Result<DeliveryLeaseOutcomeDto> {
        self.finish(claim, false).await
    }
}

/// Called only under the parent-before-job ownership lock. Positive prefix
/// receipts authorize one refund, recorded atomically in append-only history.
pub(super) async fn credit_timeline_progress(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    event_id: Uuid,
) -> Result<()> {
    sqlx::query("select event_id from post_event_delivery_jobs where event_id=$1 and target='TIMELINE' for update").bind(event_id).fetch_optional(&mut **tx).await?;
    // Read expiry after any job-lock wait. A live/replaced lease must neither
    // receive a credit nor be retired from the pending-check index.
    let expired: bool = sqlx::query_scalar("select exists(select 1 from post_event_delivery_jobs where event_id=$1 and target='TIMELINE' and state='LEASED' and lease_expires_at<=clock_timestamp() and progress_checked_attempt_id is distinct from attempt_id)")
        .bind(event_id).fetch_one(&mut **tx).await?;
    if !expired {
        return Ok(());
    }
    let credited=sqlx::query("update post_event_delivery_jobs set progress_credits=progress_credits+1 where event_id=$1 and target='TIMELINE' and state='LEASED' and lease_expires_at<=clock_timestamp() and attempt_count>progress_credits and post_timeline_pending_progress_credit(event_id,attempt_id)=1 returning attempt_id")
        .bind(event_id).fetch_optional(&mut **tx).await?;
    if let Some(row) = credited {
        let attempt: Uuid = row.get("attempt_id");
        let marker=sqlx::query("insert into post_event_delivery_attempts(attempt_id,event_id,target,source,phase,occurred_at,resolution_id) select attempt_id,event_id,target,source,'PROGRESSED',clock_timestamp(),resolution_id from post_event_delivery_attempts where attempt_id=$1 and event_id=$2 and target='TIMELINE' and phase='STARTED'")
            .bind(attempt).bind(event_id).execute(&mut **tx).await?;
        ensure!(
            marker.rows_affected() == 1,
            "Timeline progress has no immutable STARTED marker"
        );
    }
    // The deferred projection fence prevents new receipts committing for an
    // expired attempt. A negative check is therefore final for this identity;
    // a future attempt_id automatically returns the job to the index.
    sqlx::query("update post_event_delivery_jobs set progress_checked_attempt_id=attempt_id where event_id=$1 and target='TIMELINE' and state='LEASED'")
        .bind(event_id).execute(&mut **tx).await?;
    Ok(())
}
