-- Phase R3: model and experiment contracts. Learned serving is default-off;
-- these tables only make activation explicit and auditable.

create table if not exists people_recommendation_models (
    model_version text primary key,
    model_name text not null,
    model_kind text not null,
    rollout_mode text not null,
    status text not null,
    artifact_uri text null,
    artifact_sha256 text null,
    feature_schema_version text not null,
    fallback_version text null
        references people_recommendation_models(model_version),
    embedding_dimension integer null,
    training_examples bigint not null default 0,
    offline_recall_at_100 double precision null,
    enabled boolean not null default false,
    created_at timestamptz not null default now(),
    activated_at timestamptz null,
    constraint people_model_rollout_check
        check (rollout_mode in ('DETERMINISTIC', 'SHADOW', 'CANARY', 'LEARNED')),
    constraint people_model_status_check
        check (status in ('DRAFT', 'SHADOW', 'READY', 'ACTIVE', 'RETIRED', 'REVOKED')),
    constraint people_model_training_examples_check check (training_examples >= 0),
    constraint people_model_recall_check
        check (offline_recall_at_100 is null or offline_recall_at_100 between 0 and 1),
    constraint people_model_embedding_dimension_check
        check (embedding_dimension is null or embedding_dimension between 16 and 2048),
    constraint people_model_digest_check
        check (artifact_sha256 is null or artifact_sha256 ~ '^[0-9a-f]{64}$'),
    constraint people_model_active_timestamp_check
        check (status <> 'ACTIVE' or activated_at is not null),
    constraint people_model_revoked_disabled_check
        check (status <> 'REVOKED' or enabled = false),
    constraint people_model_remote_artifact_check
        check (
            model_kind = 'LINEAR'
            or artifact_uri ~ '^(s3|gs|https|oci)://'
        ),
    constraint people_model_learned_contract_check
        check (
            model_kind = 'LINEAR'
            or (
                artifact_uri is not null
                and artifact_sha256 is not null
                and fallback_version is not null
                and embedding_dimension is not null
            )
        )
);

insert into people_recommendation_models (
    model_version, model_name, model_kind, rollout_mode, status,
    feature_schema_version, training_examples, enabled, activated_at
)
values (
    'people-linear-v1', 'people-recommendation-ranker', 'LINEAR',
    'DETERMINISTIC', 'ACTIVE', 'people-features-v1', 0, true, now()
)
on conflict (model_version) do update
set model_name = excluded.model_name,
    model_kind = excluded.model_kind,
    rollout_mode = excluded.rollout_mode,
    status = excluded.status,
    feature_schema_version = excluded.feature_schema_version,
    enabled = excluded.enabled;


create table if not exists people_recommendation_model_readiness (
    model_version text primary key
        references people_recommendation_models(model_version) on delete cascade,
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
    constraint people_readiness_non_negative_check check (
        observation_days >= 0 and impressions >= 0 and positive_outcomes >= 0
        and negative_outcomes >= 0 and distinct_viewers >= 0 and shadow_requests >= 0
    ),
    constraint people_readiness_basis_points_check check (
        source_coverage_basis_points between 0 and 10000
        and position_coverage_basis_points between 0 and 10000
        and event_loss_basis_points between 0 and 10000
        and snapshot_replay_basis_points between 0 and 10000
        and shadow_error_basis_points between 0 and 10000
        and shadow_policy_difference_basis_points between 0 and 10000
        and shadow_top_k_overlap_basis_points between 0 and 10000
    )
);

create table if not exists people_recommendation_rollouts (
    surface text primary key,
    mode text not null,
    deterministic_version text not null
        references people_recommendation_models(model_version),
    learned_version text null
        references people_recommendation_models(model_version),
    canary_percent integer not null default 0,
    updated_at timestamptz not null default now(),
    constraint people_rollout_mode_check
        check (mode in ('DETERMINISTIC', 'SHADOW', 'CANARY', 'LEARNED')),
    constraint people_rollout_canary_range check (canary_percent between 0 and 100),
    constraint people_rollout_learned_version_check
        check (mode = 'DETERMINISTIC' or learned_version is not null)
);

insert into people_recommendation_rollouts (
    surface, mode, deterministic_version, learned_version, canary_percent
)
values ('PEOPLE_RECOMMENDATION', 'DETERMINISTIC', 'people-linear-v1', null, 0)
on conflict (surface) do nothing;

create table if not exists people_experiment_assignments (
    viewer_id uuid not null,
    experiment_key text not null,
    variant text not null,
    rollout_mode text not null,
    assigned_at timestamptz not null,
    expires_at timestamptz not null,
    primary key (viewer_id, experiment_key),
    constraint people_experiment_expiry_check check (expires_at > assigned_at),
    constraint people_experiment_rollout_check
        check (rollout_mode in ('DETERMINISTIC', 'SHADOW', 'CANARY', 'LEARNED'))
);

create index if not exists people_experiment_expiry_idx
    on people_experiment_assignments (expires_at, viewer_id);

-- Stored as a bounded array while the ANN adapter remains disabled. A future
-- pgvector migration can backfill from this table without changing the online
-- serving contract.
create table if not exists people_embeddings (
    subject_type text not null,
    subject_id uuid not null,
    model_version text not null
        references people_recommendation_models(model_version),
    embedding real[] not null,
    updated_at timestamptz not null,
    primary key (subject_type, subject_id, model_version),
    constraint people_embedding_subject_type_check check (subject_type = 'USER'),
    constraint people_embedding_bounds_check
        check (cardinality(embedding) between 16 and 2048)
);
