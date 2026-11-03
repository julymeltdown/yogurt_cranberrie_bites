-- Existing lifetime claims do not identify attempts for the current page.
-- Preserve that uncertainty; never grant a fresh budget to an attempted legacy
-- job automatically. Drain old writers before applying this migration.
alter table feed_fanout_jobs add column page_attempt_count integer null default 0;
alter table feed_fanout_jobs add column max_page_attempts integer null;
alter table feed_fanout_jobs add column retry_base_ms bigint null;
alter table feed_fanout_jobs add column page_budget_provenance text not null default 'UNCLAIMED';
update feed_fanout_jobs set page_attempt_count = null, page_budget_provenance='LEGACY_UNVERIFIED' where attempt_count > 0;
alter table feed_fanout_jobs add constraint feed_fanout_page_attempt_count_check
    check (page_attempt_count is null or page_attempt_count between 0 and 100);
alter table feed_fanout_jobs add constraint feed_fanout_page_budget_check
    check (max_page_attempts is null or
           (max_page_attempts between 1 and 100 and page_attempt_count is not null
            and page_attempt_count <= max_page_attempts));
alter table feed_fanout_jobs add constraint feed_fanout_retry_base_check
    check ((max_page_attempts is null and retry_base_ms is null) or
           (max_page_attempts is not null and retry_base_ms is not null and retry_base_ms between 100 and 3600000));
alter table feed_fanout_jobs add constraint feed_fanout_page_provenance_check
    check ((page_budget_provenance='LEGACY_UNVERIFIED' and page_attempt_count is null and max_page_attempts is null)
        or (page_budget_provenance='UNCLAIMED' and page_attempt_count is not null and page_attempt_count=0 and max_page_attempts is null)
        or (page_budget_provenance='CURRENT_PAGE' and page_attempt_count is not null and max_page_attempts is not null));
create index feed_fanout_expired_owned_page_idx
    on feed_fanout_jobs (lease_expires_at, job_id)
    where status='PROCESSING' and max_page_attempts is not null
      and page_attempt_count<max_page_attempts;
create index feed_fanout_expired_page_budget_idx
    on feed_fanout_jobs (lease_expires_at nulls first, next_attempt_at, job_id)
    where completed_at is null and dead_lettered_at is null
      and max_page_attempts is not null and page_attempt_count >= max_page_attempts;

-- Inactive sources rotate through a separate reconciliation queue, without
-- occupying the bounded pending-claim window on every worker tick.
alter table feed_fanout_jobs add column next_source_check_at timestamptz not null default now();
create index feed_fanout_pending_page_idx
    on feed_fanout_jobs (next_attempt_at,created_at,job_id)
    where status='PENDING' and page_attempt_count is not null
      and page_attempt_count<coalesce(max_page_attempts,100)
      and completed_at is null and dead_lettered_at is null;
create index feed_fanout_source_recheck_idx
    on feed_fanout_jobs (next_source_check_at,job_id)
    where status='SUSPENDED' and page_attempt_count is not null
      and page_attempt_count<coalesce(max_page_attempts,100)
      and completed_at is null and dead_lettered_at is null;
