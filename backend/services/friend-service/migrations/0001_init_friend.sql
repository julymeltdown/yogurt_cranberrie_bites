create table if not exists follows (
    follower_id uuid not null,
    followee_id uuid not null,
    created_at timestamptz not null default now(),
    primary key (follower_id, followee_id)
);

create index if not exists follows_followee_idx on follows (followee_id, created_at desc);
create index if not exists follows_follower_idx on follows (follower_id, created_at desc);
