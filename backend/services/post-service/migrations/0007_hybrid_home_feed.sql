-- Phase R2: additive hybrid timeline and recommendation serving structures.
-- Migration 0006_feed_ranking.sql is already part of the released history, so
-- this migration only expands that schema. Existing tables are deliberately
-- retained for rollback compatibility.

create index if not exists home_timeline_post_viewer_idx
    on home_timeline_entries (post_id, viewer_id);

alter table high_fanout_posts
    add column if not exists follower_count bigint not null default 0;

create index if not exists high_fanout_posts_expiry_post_idx
    on high_fanout_posts (expires_at, post_id);

create table if not exists feed_fanout_jobs (
    job_id uuid primary key,
    event_id uuid not null unique,
    post_id uuid not null references posts(id) on delete cascade,
    author_id uuid not null,
    post_created_at timestamptz not null,
    cursor text null,
    status text not null default 'PENDING',
    attempt_count integer not null default 0,
    next_attempt_at timestamptz not null,
    locked_at timestamptz null,
    last_error text null,
    completed_at timestamptz null,
    dead_lettered_at timestamptz null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    constraint feed_fanout_jobs_status_check
        check (status in ('PENDING', 'PROCESSING', 'COMPLETED', 'DEAD_LETTERED')),
    constraint feed_fanout_jobs_attempt_count_check check (attempt_count >= 0)
);

create index if not exists feed_fanout_jobs_claim_idx
    on feed_fanout_jobs (next_attempt_at, created_at, job_id)
    where completed_at is null and dead_lettered_at is null;

create table if not exists feed_recommendation_snapshots (
    snapshot_id uuid primary key,
    viewer_id uuid not null,
    request_id text not null,
    session_id uuid null,
    rank_version text not null,
    fallback_used boolean not null default false,
    failed_sources text[] not null default '{}',
    created_at timestamptz not null,
    expires_at timestamptz not null,
    constraint feed_rec_snapshots_expiry_check check (expires_at > created_at)
);

create index if not exists feed_rec_snapshots_viewer_created_idx
    on feed_recommendation_snapshots (viewer_id, created_at desc, snapshot_id desc);
create index if not exists feed_rec_snapshots_expiry_idx
    on feed_recommendation_snapshots (expires_at, snapshot_id);

create table if not exists feed_recommendation_snapshot_items (
    snapshot_id uuid not null
        references feed_recommendation_snapshots(snapshot_id) on delete cascade,
    position integer not null,
    post_id uuid not null references posts(id) on delete cascade,
    author_id uuid not null,
    source text not null,
    reason_code text not null,
    score_micros bigint not null,
    impression_id uuid not null unique,
    propensity_micros integer not null,
    primary key (snapshot_id, position),
    unique (snapshot_id, post_id),
    constraint feed_rec_snapshot_position_check check (position between 0 and 999),
    constraint feed_rec_snapshot_propensity_check
        check (propensity_micros between 1 and 1000000)
);

create index if not exists feed_rec_snapshot_items_post_idx
    on feed_recommendation_snapshot_items (post_id, snapshot_id);

create table if not exists feed_recommendation_impressions (
    impression_id uuid primary key,
    snapshot_id uuid not null
        references feed_recommendation_snapshots(snapshot_id) on delete cascade,
    viewer_id uuid not null,
    post_id uuid not null,
    author_id uuid not null,
    source text not null,
    reason_code text not null,
    position integer not null,
    rank_version text not null,
    request_id text not null,
    session_id uuid null,
    score_micros bigint not null,
    propensity_micros integer not null,
    occurred_at timestamptz not null,
    constraint feed_rec_impression_position_check check (position between 0 and 999),
    constraint feed_rec_impression_propensity_check
        check (propensity_micros between 1 and 1000000)
);

create index if not exists feed_rec_impressions_viewer_post_idx
    on feed_recommendation_impressions (viewer_id, post_id, occurred_at desc);
create index if not exists feed_rec_impressions_viewer_time_idx
    on feed_recommendation_impressions (viewer_id, occurred_at desc, impression_id desc);

-- The event table is introduced by 0006_feed_ranking.sql. Keep its JSONB
-- metadata contract and add only the indexes required by the new serving path.
create index if not exists feed_events_subject_time_idx
    on feed_recommendation_events (subject_id, occurred_at desc, event_id desc);
create index if not exists feed_events_viewer_time_event_idx
    on feed_recommendation_events (viewer_id, occurred_at desc, event_id desc);

create table if not exists post_engagement_velocity (
    post_id uuid primary key references posts(id) on delete cascade,
    like_events bigint not null default 0,
    comment_events bigint not null default 0,
    velocity_score double precision not null default 0,
    updated_at timestamptz not null,
    constraint post_velocity_like_count_check check (like_events >= 0),
    constraint post_velocity_comment_count_check check (comment_events >= 0),
    constraint post_velocity_score_check check (velocity_score >= 0)
);

create index if not exists post_engagement_velocity_score_idx
    on post_engagement_velocity (velocity_score desc, updated_at desc, post_id);

create table if not exists feed_author_affinities (
    viewer_id uuid not null,
    author_id uuid not null,
    affinity_score double precision not null default 0,
    negative_score double precision not null default 0,
    last_interacted_at timestamptz not null,
    feature_version text not null default 'feed-affinity-v1',
    primary key (viewer_id, author_id),
    constraint feed_affinity_score_check check (affinity_score between 0 and 1),
    constraint feed_negative_score_check check (negative_score between 0 and 1)
);

insert into feed_author_affinities (
    viewer_id, author_id, affinity_score, negative_score,
    last_interacted_at, feature_version
)
select viewer_id, author_id,
       greatest(0, least(1, affinity_score)),
       greatest(0, least(1, negative_score)),
       updated_at,
       'feed-affinity-v1'
from feed_author_affinity
on conflict (viewer_id, author_id) do nothing;

create index if not exists feed_affinity_viewer_score_idx
    on feed_author_affinities (viewer_id, affinity_score desc, author_id);

create table if not exists feed_recent_actions (
    event_id uuid primary key
        references feed_recommendation_events(event_id) on delete cascade,
    viewer_id uuid not null,
    post_id uuid not null,
    action text not null,
    occurred_at timestamptz not null
);

create index if not exists feed_recent_actions_viewer_time_idx
    on feed_recent_actions (viewer_id, occurred_at desc, event_id desc);

create table if not exists feed_recommendation_models (
    model_version text primary key,
    model_name text not null,
    model_kind text not null,
    rollout_mode text not null,
    artifact_uri text null,
    artifact_sha256 text null,
    feature_schema_version text not null,
    embedding_dimension integer null,
    fallback_version text null,
    training_examples bigint not null default 0,
    offline_recall_at_100 double precision null,
    status text not null default 'DRAFT',
    enabled boolean not null default false,
    created_at timestamptz not null default now(),
    activated_at timestamptz null,
    constraint feed_model_rollout_check
        check (rollout_mode in ('DETERMINISTIC', 'SHADOW', 'CANARY', 'LEARNED')),
    constraint feed_model_status_check
        check (status in ('DRAFT', 'SHADOW', 'READY', 'ACTIVE', 'RETIRED', 'REVOKED')),
    constraint feed_model_remote_artifact_check
        check (
            model_kind = 'LINEAR'
            or artifact_uri ~ '^(s3|gs|https|oci)://'
        ),
    constraint feed_model_active_timestamp_check
        check (status <> 'ACTIVE' or activated_at is not null),
    constraint feed_model_learned_contract_check
        check (
            model_kind = 'LINEAR'
            or (
                artifact_uri is not null
                and artifact_sha256 is not null
                and fallback_version is not null
                and embedding_dimension is not null
            )
        ),
    constraint feed_model_training_examples_check check (training_examples >= 0),
    constraint feed_model_recall_check
        check (offline_recall_at_100 is null or offline_recall_at_100 between 0 and 1),
    constraint feed_model_embedding_dimension_check
        check (embedding_dimension is null or embedding_dimension between 16 and 2048),
    constraint feed_model_artifact_sha_check
        check (artifact_sha256 is null or artifact_sha256 ~ '^[0-9a-f]{64}$')
);

insert into feed_recommendation_models (
    model_version, model_name, model_kind, rollout_mode, feature_schema_version,
    training_examples, offline_recall_at_100, status, enabled, activated_at
)
values (
    'feed-linear-v1', 'home-feed-ranker', 'LINEAR', 'DETERMINISTIC', 'feed-features-v1',
    0, null, 'ACTIVE', true, now()
)
on conflict (model_version) do update
set model_name = excluded.model_name,
    model_kind = excluded.model_kind,
    rollout_mode = excluded.rollout_mode,
    feature_schema_version = excluded.feature_schema_version,
    status = excluded.status,
    enabled = excluded.enabled;


create table if not exists feed_recommendation_model_readiness (
    model_version text primary key
        references feed_recommendation_models(model_version) on delete cascade,
    evaluated_at timestamptz not null,
    observation_days integer not null,
    impressions bigint not null,
    positive_outcomes bigint not null,
    negative_outcomes bigint not null,
    distinct_viewers bigint not null,
    source_coverage_basis_points integer not null,
    position_coverage_basis_points integer not null,
    event_loss_basis_points integer not null,
    snapshot_replay_basis_points integer not null,
    shadow_requests bigint not null,
    shadow_error_basis_points integer not null,
    shadow_p95_latency_ms integer not null,
    shadow_policy_difference_basis_points integer not null,
    shadow_top_k_overlap_basis_points integer not null,
    ready boolean not null default false,
    blockers text[] not null default '{}',
    constraint feed_readiness_non_negative_check check (
        observation_days >= 0 and impressions >= 0 and positive_outcomes >= 0
        and negative_outcomes >= 0 and distinct_viewers >= 0 and shadow_requests >= 0
    ),
    constraint feed_readiness_basis_points_check check (
        source_coverage_basis_points between 0 and 10000
        and position_coverage_basis_points between 0 and 10000
        and event_loss_basis_points between 0 and 10000
        and snapshot_replay_basis_points between 0 and 10000
        and shadow_error_basis_points between 0 and 10000
        and shadow_policy_difference_basis_points between 0 and 10000
        and shadow_top_k_overlap_basis_points between 0 and 10000
    )
);

create table if not exists feed_recommendation_rollouts (
    surface text primary key,
    mode text not null,
    deterministic_version text not null
        references feed_recommendation_models(model_version),
    learned_version text null
        references feed_recommendation_models(model_version),
    canary_percent integer not null default 0,
    updated_at timestamptz not null default now(),
    constraint feed_rollout_mode_check
        check (mode in ('DETERMINISTIC', 'SHADOW', 'CANARY', 'LEARNED')),
    constraint feed_rollout_canary_check check (canary_percent between 0 and 100),
    constraint feed_rollout_learned_check
        check (mode = 'DETERMINISTIC' or learned_version is not null)
);

insert into feed_recommendation_rollouts (
    surface, mode, deterministic_version, learned_version, canary_percent
)
values ('HOME_FEED', 'DETERMINISTIC', 'feed-linear-v1', null, 0)
on conflict (surface) do nothing;

create table if not exists feed_experiment_assignments (
    viewer_id uuid not null,
    experiment_key text not null,
    variant text not null,
    rollout_mode text not null,
    assigned_at timestamptz not null,
    expires_at timestamptz not null,
    primary key (viewer_id, experiment_key),
    constraint feed_experiment_expiry_check check (expires_at > assigned_at),
    constraint feed_experiment_rollout_check
        check (rollout_mode in ('DETERMINISTIC', 'SHADOW', 'CANARY', 'LEARNED'))
);

create index if not exists feed_experiment_expiry_idx
    on feed_experiment_assignments (expires_at, viewer_id);

-- `real[]` keeps the migration dependency-free. Learned ANN serving remains
-- disabled until an explicit pgvector adapter and validated model manifest are
-- configured in a later release.
create table if not exists feed_embeddings (
    subject_type text not null,
    subject_id uuid not null,
    model_version text not null
        references feed_recommendation_models(model_version),
    embedding real[] not null,
    updated_at timestamptz not null,
    primary key (subject_type, subject_id, model_version),
    constraint feed_embedding_subject_type_check
        check (subject_type in ('USER', 'POST')),
    constraint feed_embedding_bounds_check
        check (cardinality(embedding) between 16 and 2048)
);
