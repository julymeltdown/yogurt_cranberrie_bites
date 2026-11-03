create table moderation_report_operation_receipts (
    operation_id uuid primary key,
    actor_id uuid not null,
    scope text not null check (scope = 'gateway.v1.CreateModerationReport'),
    idempotency_key text check (
        idempotency_key is null
        or octet_length(idempotency_key) between 1 and 128
    ),
    state text not null default 'REPLAYABLE'
        check (state in ('REPLAYABLE', 'EXPIRED')),
    outcome_json jsonb,
    committed_at timestamptz not null default now(),
    replay_until timestamptz not null,
    check (
        (state = 'REPLAYABLE' and outcome_json is not null)
        or (state = 'EXPIRED' and outcome_json is null)
    ),
    check (outcome_json is null or pg_column_size(outcome_json) <= 16384),
    check (replay_until >= committed_at)
);

create unique index moderation_report_operation_receipts_actor_key_idx
    on moderation_report_operation_receipts (actor_id, idempotency_key)
    where idempotency_key is not null;

create index moderation_report_operation_receipts_expiry_idx
    on moderation_report_operation_receipts (replay_until, operation_id)
    where state = 'REPLAYABLE';
