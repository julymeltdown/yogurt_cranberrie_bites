create table if not exists home_timeline_entries (
    viewer_id uuid not null,
    post_id uuid not null references posts(id) on delete cascade,
    author_id uuid not null,
    source text not null,
    source_score double precision not null,
    inserted_at timestamptz not null,
    expires_at timestamptz not null,
    primary key (viewer_id, post_id, source)
);

create index if not exists home_timeline_viewer_time_idx
    on home_timeline_entries (viewer_id, inserted_at desc, post_id desc);
create index if not exists home_timeline_expiry_idx
    on home_timeline_entries (expires_at);

create table if not exists high_fanout_posts (
    post_id uuid primary key references posts(id) on delete cascade,
    author_id uuid not null,
    created_at timestamptz not null,
    expires_at timestamptz not null
);
create index if not exists high_fanout_posts_author_time_idx
    on high_fanout_posts (author_id, created_at desc, post_id desc);

create table if not exists feed_author_affinity (
    viewer_id uuid not null,
    author_id uuid not null,
    affinity_score double precision not null default 0,
    negative_score double precision not null default 0,
    updated_at timestamptz not null default now(),
    primary key (viewer_id, author_id)
);

create table if not exists feed_snapshots (
    id uuid primary key,
    viewer_id uuid not null,
    rank_version text not null,
    created_at timestamptz not null,
    expires_at timestamptz not null
);
create index if not exists feed_snapshots_viewer_expiry_idx
    on feed_snapshots (viewer_id, expires_at desc);

create table if not exists feed_snapshot_items (
    snapshot_id uuid not null references feed_snapshots(id) on delete cascade,
    post_id uuid not null,
    author_id uuid not null,
    source text not null,
    reason_code text not null,
    score_micros bigint not null,
    rank_version text not null,
    feature_snapshot_json jsonb not null default '{}',
    created_at timestamptz not null,
    primary key (snapshot_id, post_id)
);
create index if not exists feed_snapshot_items_rank_idx
    on feed_snapshot_items (snapshot_id, score_micros desc, created_at desc, post_id desc);

create table if not exists feed_impressions (
    impression_id uuid primary key,
    snapshot_id uuid not null references feed_snapshots(id) on delete cascade,
    viewer_id uuid not null,
    post_id uuid not null,
    author_id uuid not null,
    source text not null,
    reason_code text not null,
    position integer not null,
    rank_version text not null,
    score_micros bigint not null,
    request_id text not null,
    session_id uuid null,
    propensity_micros integer not null,
    occurred_at timestamptz not null
);
create index if not exists feed_impressions_viewer_time_idx
    on feed_impressions (viewer_id, occurred_at desc);
create index if not exists feed_impressions_post_time_idx
    on feed_impressions (post_id, occurred_at desc);

create table if not exists feed_recommendation_events (
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
    metadata_json jsonb not null default '{}'
);
create index if not exists feed_events_viewer_time_idx
    on feed_recommendation_events (viewer_id, occurred_at desc);

create table if not exists recommendation_model_registry (
    surface text not null,
    model_version text not null,
    status text not null,
    artifact_uri text null,
    activated_at timestamptz null,
    created_at timestamptz not null default now(),
    primary key (surface, model_version)
);

create table if not exists recommendation_rollouts (
    surface text primary key,
    mode text not null,
    deterministic_version text not null,
    learned_version text null,
    canary_percent integer not null default 0,
    updated_at timestamptz not null default now(),
    constraint recommendation_rollouts_canary_range check (canary_percent between 0 and 100)
);

insert into recommendation_model_registry (surface, model_version, status, activated_at)
values ('HOME_FEED', 'feed-linear-v1', 'READY', now())
on conflict (surface, model_version) do nothing;

insert into recommendation_rollouts (
    surface, mode, deterministic_version, learned_version, canary_percent
)
values ('HOME_FEED', 'DETERMINISTIC', 'feed-linear-v1', null, 0)
on conflict (surface) do nothing;
