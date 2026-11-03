create table if not exists people_recommendation_candidates (
    viewer_id uuid not null,
    candidate_user_id uuid not null,
    source text not null,
    source_score double precision not null,
    feature_snapshot_json text not null default '{}',
    mutual_cluster uuid null,
    generated_at timestamptz not null,
    expires_at timestamptz not null,
    model_version text not null,
    primary key (viewer_id, candidate_user_id, source)
);

create index if not exists people_candidates_viewer_expiry_score_idx
    on people_recommendation_candidates
        (viewer_id, expires_at, source_score desc, candidate_user_id);

create table if not exists people_recommendation_snapshots (
    id uuid primary key,
    viewer_id uuid not null,
    rank_version text not null,
    created_at timestamptz not null,
    expires_at timestamptz not null
);

create index if not exists people_snapshots_viewer_expiry_idx
    on people_recommendation_snapshots (viewer_id, expires_at desc);

create table if not exists people_recommendation_snapshot_items (
    snapshot_id uuid not null references people_recommendation_snapshots(id) on delete cascade,
    candidate_user_id uuid not null,
    source text not null,
    reason_code text not null,
    score_micros bigint not null,
    rank_version text not null,
    feature_snapshot_json text not null default '{}',
    mutual_cluster uuid null,
    generated_at timestamptz not null,
    primary key (snapshot_id, candidate_user_id)
);

create index if not exists people_snapshot_items_page_idx
    on people_recommendation_snapshot_items
        (snapshot_id, score_micros desc, candidate_user_id desc);

create table if not exists people_recommendation_impressions (
    impression_id uuid primary key,
    snapshot_id uuid not null references people_recommendation_snapshots(id) on delete cascade,
    viewer_id uuid not null,
    candidate_user_id uuid not null,
    source text not null,
    reason_code text not null,
    rank_version text not null,
    position integer not null,
    score_micros bigint not null,
    request_id text not null,
    session_id uuid null,
    occurred_at timestamptz not null,
    propensity_micros integer not null,
    unique (snapshot_id, viewer_id, candidate_user_id)
);

create index if not exists people_impressions_viewer_candidate_time_idx
    on people_recommendation_impressions
        (viewer_id, candidate_user_id, occurred_at desc);

create table if not exists people_recommendation_events (
    event_id uuid primary key,
    schema_version integer not null,
    occurred_at timestamptz not null,
    viewer_id uuid not null,
    subject_type text not null,
    subject_id uuid not null,
    actor_id uuid null,
    surface text not null,
    position integer null,
    request_id text null,
    session_id uuid null,
    action text not null,
    source text not null,
    model_version text not null,
    impression_id uuid null,
    propensity_micros integer null,
    dwell_ms integer null,
    metadata_json text not null default '{}'
);

create index if not exists people_events_viewer_subject_time_idx
    on people_recommendation_events
        (viewer_id, subject_id, occurred_at desc);

create table if not exists people_recommendation_refresh_jobs (
    viewer_id uuid primary key,
    status text not null default 'PENDING',
    attempt_count integer not null default 0,
    next_attempt_at timestamptz not null default now(),
    claimed_at timestamptz null,
    last_error text null,
    updated_at timestamptz not null default now()
);

create index if not exists people_refresh_jobs_claim_idx
    on people_recommendation_refresh_jobs
        (status, next_attempt_at, updated_at);

create or replace function enqueue_people_recommendation_refresh()
returns trigger language plpgsql as $$
begin
    if tg_table_name = 'follows' then
        insert into people_recommendation_refresh_jobs (viewer_id, status, next_attempt_at, updated_at)
        values (new.follower_id, 'PENDING', now(), now()),
               (new.followee_id, 'PENDING', now(), now())
        on conflict (viewer_id) do update
        set status='PENDING', next_attempt_at=least(people_recommendation_refresh_jobs.next_attempt_at, now()), updated_at=now();
    elsif tg_table_name = 'follow_requests' then
        insert into people_recommendation_refresh_jobs (viewer_id, status, next_attempt_at, updated_at)
        values (new.follower_id, 'PENDING', now(), now()),
               (new.followee_id, 'PENDING', now(), now())
        on conflict (viewer_id) do update
        set status='PENDING', next_attempt_at=least(people_recommendation_refresh_jobs.next_attempt_at, now()), updated_at=now();
    elsif tg_table_name = 'user_blocks' then
        insert into people_recommendation_refresh_jobs (viewer_id, status, next_attempt_at, updated_at)
        values (new.blocker_id, 'PENDING', now(), now()),
               (new.blocked_id, 'PENDING', now(), now())
        on conflict (viewer_id) do update
        set status='PENDING', next_attempt_at=least(people_recommendation_refresh_jobs.next_attempt_at, now()), updated_at=now();
    elsif tg_table_name = 'user_mutes' then
        insert into people_recommendation_refresh_jobs (viewer_id, status, next_attempt_at, updated_at)
        values (new.muter_id, 'PENDING', now(), now()),
               (new.muted_id, 'PENDING', now(), now())
        on conflict (viewer_id) do update
        set status='PENDING', next_attempt_at=least(people_recommendation_refresh_jobs.next_attempt_at, now()), updated_at=now();
    end if;
    return new;
end;
$$;

drop trigger if exists follows_people_recommendation_refresh on follows;
create trigger follows_people_recommendation_refresh
    after insert or update on follows
    for each row execute function enqueue_people_recommendation_refresh();

drop trigger if exists follow_requests_people_recommendation_refresh on follow_requests;
create trigger follow_requests_people_recommendation_refresh
    after insert or update on follow_requests
    for each row execute function enqueue_people_recommendation_refresh();

drop trigger if exists blocks_people_recommendation_refresh on user_blocks;
create trigger blocks_people_recommendation_refresh
    after insert or update on user_blocks
    for each row execute function enqueue_people_recommendation_refresh();

drop trigger if exists mutes_people_recommendation_refresh on user_mutes;
create trigger mutes_people_recommendation_refresh
    after insert or update on user_mutes
    for each row execute function enqueue_people_recommendation_refresh();
