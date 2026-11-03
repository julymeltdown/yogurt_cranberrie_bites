-- Drain old fanout writers before activation. Legacy PROCESSING rows retain
-- their old locked_at recovery boundary; new claims always own both fields.
alter table feed_fanout_jobs add column lease_token uuid null;
alter table feed_fanout_jobs add column lease_expires_at timestamptz null;
alter table feed_fanout_jobs add constraint feed_fanout_lease_pair
    check ((lease_token is null) = (lease_expires_at is null));
alter table feed_fanout_jobs add constraint feed_fanout_lease_state
    check (status = 'PROCESSING' or (lease_token is null and lease_expires_at is null));

alter table feed_fanout_jobs drop constraint feed_fanout_jobs_status_check;
alter table feed_fanout_jobs add constraint feed_fanout_jobs_status_check
    check(status in ('PENDING','PROCESSING','COMPLETED','DEAD_LETTERED','SUSPENDED'));
