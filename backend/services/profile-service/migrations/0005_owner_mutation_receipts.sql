-- Keep the privacy effect, audit record, and owner replay evidence atomic.
-- The Gateway receives only a status projection from the dedicated internal RPC.
create table profile_mutation_receipts (
    operation_id uuid primary key,
    actor_id uuid not null,
    idempotency_key varchar(128),
    scope text not null,
    command_fingerprint text not null,
    outcome_json jsonb,
    committed_at timestamptz not null default now(),
    replay_until timestamptz not null default now() + interval '24 hours',
    check (idempotency_key is null or length(idempotency_key) between 1 and 128),
    check (scope = 'profile.v1.UpdateProfilePrivacy')
);

create unique index profile_mutation_receipts_actor_key
    on profile_mutation_receipts (actor_id, idempotency_key)
    where idempotency_key is not null;
create index profile_mutation_receipts_replay_until
    on profile_mutation_receipts (replay_until) where outcome_json is not null;
