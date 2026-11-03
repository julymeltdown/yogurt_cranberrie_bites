//! Production fanout statements shared with native EXPLAIN verification.
//! Candidate selection uses once-per-statement database time (STABLE) so the
//! planner can bound index scans. Authority changes still use fresh database
//! clock checks after locks; candidate time cannot renew or revoke a live lease.

pub const EXHAUSTED_MAINTENANCE_SQL: &str = r#"
            with exhausted as (
                select job_id from feed_fanout_jobs
                where completed_at is null and dead_lettered_at is null
                  and max_page_attempts is not null and page_attempt_count >= max_page_attempts
                  and (lease_expires_at is null or lease_expires_at <= statement_timestamp())
                order by lease_expires_at nulls first,next_attempt_at,job_id
                for update skip locked limit 100
            )
            update feed_fanout_jobs job set status='DEAD_LETTERED',dead_lettered_at=clock_timestamp(),
                locked_at=null,lease_token=null,lease_expires_at=null,
                last_error='current page retry budget exhausted',updated_at=clock_timestamp()
            from exhausted where job.job_id=exhausted.job_id
              and job.completed_at is null and job.dead_lettered_at is null
              and job.page_attempt_count>=job.max_page_attempts
              and (job.lease_expires_at is null or job.lease_expires_at<=clock_timestamp())
        "#;

pub const EXPIRED_MAINTENANCE_SQL: &str = r#"
            with expired as (
                select job_id from feed_fanout_jobs
                where status='PROCESSING' and max_page_attempts is not null
                  and page_attempt_count < max_page_attempts
                  and lease_expires_at<=statement_timestamp()
                order by lease_expires_at,job_id for update skip locked limit 100
            )
            update feed_fanout_jobs job set status='PENDING',locked_at=null,lease_token=null,lease_expires_at=null,
                next_attempt_at=clock_timestamp() +
                    (least(3600000::bigint,job.retry_base_ms*(1::bigint << least(greatest(job.page_attempt_count-1,0),20)))*interval '1 millisecond'),
                last_error='page lease expired; outcome unconfirmed',updated_at=clock_timestamp()
            from expired where job.job_id=expired.job_id and job.status='PROCESSING'
              and job.lease_expires_at<=clock_timestamp() and job.page_attempt_count<job.max_page_attempts
        "#;

pub const SOURCE_RECHECK_SQL: &str = r#"
            with rechecks as (
                select job_id from feed_fanout_jobs
                where status='SUSPENDED' and completed_at is null and dead_lettered_at is null
                  and page_attempt_count is not null
                  and page_attempt_count<coalesce(max_page_attempts,100)
                  and next_source_check_at<=statement_timestamp()
                order by next_source_check_at,job_id for update skip locked limit 100
            )
            update feed_fanout_jobs job set
                status=case when exists(select 1 from posts p where p.id=job.post_id and p.author_id=job.author_id
                    and p.deleted_at is null and p.moderation_hidden_at is null and p.visibility in ('PUBLIC','FRIENDS'))
                    then 'PENDING' else 'SUSPENDED' end,
                next_source_check_at=clock_timestamp()+interval '500 milliseconds',updated_at=clock_timestamp()
            from rechecks where job.job_id=rechecks.job_id
        "#;

pub const CLAIM_SQL: &str = r#"
            with candidates as materialized (
                select job_id,post_id,author_id,next_attempt_at,created_at from feed_fanout_jobs
                where status='PENDING' and completed_at is null and dead_lettered_at is null
                  and page_attempt_count is not null
                  and page_attempt_count<coalesce(max_page_attempts,100)
                  and attempt_count<2147483647 and next_attempt_at<=statement_timestamp()
                  and not(job_id=any($5::uuid[]))
                order by next_attempt_at,created_at,job_id for update skip locked limit 100
            ), available as materialized (
                select c.*,exists(select 1 from posts p where p.id=c.post_id and p.author_id=c.author_id
                    and p.deleted_at is null and p.moderation_hidden_at is null and p.visibility in ('PUBLIC','FRIENDS')) as eligible
                from candidates c
            ), suspended as (
                update feed_fanout_jobs job set status='SUSPENDED',
                    next_source_check_at=clock_timestamp()+interval '500 milliseconds',updated_at=clock_timestamp()
                from available where job.job_id=available.job_id and not available.eligible returning job.job_id
            ), claimed as (
                select job_id from available where eligible
                order by next_attempt_at,created_at,job_id limit $2
            )
            update feed_fanout_jobs job set locked_at=clock_timestamp(),status='PROCESSING',
                attempt_count=job.attempt_count+1,updated_at=clock_timestamp(),
                page_attempt_count=job.page_attempt_count+1,
                max_page_attempts=coalesce(job.max_page_attempts,$3),
                retry_base_ms=coalesce(job.retry_base_ms,$4),page_budget_provenance='CURRENT_PAGE',
                lease_token=gen_random_uuid(),lease_expires_at=clock_timestamp()+($1*interval '1 millisecond')
            from claimed where job.job_id=claimed.job_id
            returning job.job_id,job.event_id,job.post_id,job.author_id,job.post_created_at,
                      job.cursor,job.attempt_count,job.lease_token,job.lease_expires_at,
                      job.page_attempt_count,job.max_page_attempts
        "#;
