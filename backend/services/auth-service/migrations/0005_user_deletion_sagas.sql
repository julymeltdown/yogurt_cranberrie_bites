create table if not exists user_deletion_sagas (
    id uuid primary key,
    user_id uuid not null unique references users(id) on delete cascade,
    actor_id uuid not null,
    status text not null check (status in ('IN_PROGRESS', 'COMPLETED', 'DEAD_LETTERED')),
    retention_until timestamptz not null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    completed_at timestamptz null
);

create table if not exists user_deletion_steps (
    saga_id uuid not null references user_deletion_sagas(id) on delete cascade,
    target text not null check (target in ('PROFILE', 'FRIEND', 'POST', 'NOTIFICATION')),
    status text not null check (status in ('PENDING', 'IN_PROGRESS', 'RETRY', 'SUCCEEDED', 'DEAD_LETTERED')),
    attempt_count integer not null default 0 check (attempt_count >= 0),
    next_attempt_at timestamptz not null default now(),
    locked_at timestamptz null,
    last_error text null,
    completed_at timestamptz null,
    primary key (saga_id, target)
);

create index if not exists user_deletion_steps_due_idx
    on user_deletion_steps (next_attempt_at, saga_id, target)
    where status in ('PENDING', 'RETRY');

create index if not exists user_deletion_steps_lease_idx
    on user_deletion_steps (locked_at, saga_id, target)
    where status = 'IN_PROGRESS';
