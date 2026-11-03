alter table user_deletion_sagas
    add column if not exists retry_generation integer not null default 0
    check (retry_generation >= 0);

-- Deliberately has no foreign key to the saga or user: operator decisions and
-- their idempotency receipts remain available after account-retention purge.
create table if not exists user_deletion_retry_resolutions (
    resolution_id uuid primary key,
    saga_id uuid not null,
    actor_id uuid not null,
    reason text not null check (char_length(reason) between 20 and 1024),
    evidence_reference text not null check (char_length(evidence_reference) between 3 and 256),
    retry_generation integer not null check (retry_generation > 0),
    created_at timestamptz not null default now()
);

create index if not exists user_deletion_retry_resolutions_saga_idx
    on user_deletion_retry_resolutions (saga_id, created_at desc);
create index if not exists user_deletion_retry_resolutions_actor_idx
    on user_deletion_retry_resolutions (actor_id, created_at desc);

create or replace function reject_user_deletion_retry_resolution_mutation()
returns trigger language plpgsql as $$
begin
    raise exception 'user deletion retry resolutions are immutable';
end;
$$;

create trigger user_deletion_retry_resolutions_immutable
before update or delete on user_deletion_retry_resolutions
for each row execute function reject_user_deletion_retry_resolution_mutation();
