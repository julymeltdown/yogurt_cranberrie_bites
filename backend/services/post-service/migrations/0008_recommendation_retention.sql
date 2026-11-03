-- Model-registry hardening is additive because 0007 is immutable released history.
do $$
begin
    if not exists (
        select 1 from pg_constraint
        where conname = 'feed_model_fallback_version_fk'
    ) then
        alter table feed_recommendation_models
            add constraint feed_model_fallback_version_fk
            foreign key (fallback_version)
            references feed_recommendation_models(model_version);
    end if;
    if not exists (
        select 1 from pg_constraint
        where conname = 'feed_model_revoked_disabled_check'
    ) then
        alter table feed_recommendation_models
            add constraint feed_model_revoked_disabled_check
            check (status <> 'REVOKED' or enabled = false);
    end if;
end
$$;

-- Phase R0/R4: indexes for bounded, multi-worker feed retention.
-- All purge queries order by these columns before FOR UPDATE SKIP LOCKED.

create index if not exists home_timeline_retention_idx
    on home_timeline_entries (expires_at, viewer_id, post_id);

create index if not exists high_fanout_retention_idx
    on high_fanout_posts (expires_at, post_id);

create index if not exists feed_snapshots_retention_idx
    on feed_recommendation_snapshots (expires_at, snapshot_id);

create index if not exists feed_rec_impressions_snapshot_retention_idx
    on feed_recommendation_impressions (snapshot_id, impression_id);

create index if not exists feed_events_retention_idx
    on feed_recommendation_events (occurred_at, event_id);

create index if not exists feed_experiment_retention_idx
    on feed_experiment_assignments (expires_at, viewer_id, experiment_key);

create index if not exists feed_embeddings_retention_idx
    on feed_embeddings (updated_at, subject_id, model_version);

create index if not exists feed_fanout_terminal_retention_idx
    on feed_fanout_jobs (updated_at, job_id)
    where status in ('COMPLETED', 'DEAD_LETTERED');
