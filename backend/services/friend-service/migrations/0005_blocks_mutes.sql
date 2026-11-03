create table if not exists user_blocks (
    blocker_id uuid not null,
    blocked_id uuid not null,
    created_at timestamptz not null,
    deleted_at timestamptz null,
    primary key (blocker_id, blocked_id),
    check (blocker_id <> blocked_id)
);

create table if not exists user_mutes (
    muter_id uuid not null,
    muted_id uuid not null,
    created_at timestamptz not null,
    deleted_at timestamptz null,
    primary key (muter_id, muted_id),
    check (muter_id <> muted_id)
);

create index if not exists user_blocks_blocked_active_idx
    on user_blocks (blocked_id, blocker_id)
    where deleted_at is null;
create index if not exists user_mutes_muter_active_idx
    on user_mutes (muter_id, muted_id)
    where deleted_at is null;
