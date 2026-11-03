-- Phase R0/R4: indexes for bounded, multi-worker recommendation retention.
-- All purge queries order by these columns before FOR UPDATE SKIP LOCKED.

create index if not exists people_candidates_retention_idx
    on people_recommendation_candidates
        (expires_at, viewer_id, candidate_user_id, source);

create index if not exists people_snapshots_retention_idx
    on people_recommendation_snapshots (expires_at, id);

create index if not exists people_impressions_snapshot_retention_idx
    on people_recommendation_impressions (snapshot_id, impression_id);

create index if not exists people_events_retention_idx
    on people_recommendation_events (occurred_at, event_id);

create index if not exists people_experiment_retention_idx
    on people_experiment_assignments (expires_at, viewer_id, experiment_key);

create index if not exists people_embeddings_retention_idx
    on people_embeddings (updated_at, subject_id, model_version);

create index if not exists people_refresh_dead_retention_idx
    on people_recommendation_refresh_jobs (updated_at, viewer_id)
    where status = 'DEAD_LETTERED';
