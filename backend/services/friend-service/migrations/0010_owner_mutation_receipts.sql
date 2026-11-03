-- The owner receipt commits with the relationship effect. Keep command
-- fingerprints and outcomes private to FriendService; the Gateway receives
-- only a status projection from the dedicated internal RPC.
create table friend_mutation_receipts (
    operation_id uuid primary key,
    actor_id uuid not null,
    idempotency_key varchar(128),
    scope text not null,
    command_fingerprint text not null,
    outcome_json jsonb,
    committed_at timestamptz not null default now(),
    replay_until timestamptz not null default now() + interval '24 hours',
    check (idempotency_key is null or length(idempotency_key) between 1 and 128),
    check (scope in ('friend.v1.FollowUser', 'friend.v1.UnfollowUser'))
);
create unique index friend_mutation_receipts_actor_key
    on friend_mutation_receipts (actor_id, idempotency_key)
    where idempotency_key is not null;
create index friend_mutation_receipts_replay_until
    on friend_mutation_receipts (replay_until) where outcome_json is not null;
