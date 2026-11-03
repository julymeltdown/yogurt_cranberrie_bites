-- A receipt is committed in the SAME transaction as the business write/outbox.
-- No foreign key to content: deleting content must never re-enable a command.
create table post_mutation_receipts (
    operation_id uuid primary key,
    actor_id uuid not null,
    idempotency_key varchar(128),
    scope text not null,
    command_fingerprint text not null,
    outcome_json jsonb,
    committed_at timestamptz not null default now(),
    replay_until timestamptz not null default now() + interval '24 hours',
    check (idempotency_key is null or length(idempotency_key) between 1 and 128),
    check (scope in ('post.v1.CreatePost', 'post.v1.CreateComment',
                    'post.v1.DeletePost', 'post.v1.LikePost', 'post.v1.UnlikePost'))
);
create unique index post_mutation_receipts_actor_key
    on post_mutation_receipts (actor_id, idempotency_key)
    where idempotency_key is not null;
create index post_mutation_receipts_replay_until
    on post_mutation_receipts (replay_until) where outcome_json is not null;
