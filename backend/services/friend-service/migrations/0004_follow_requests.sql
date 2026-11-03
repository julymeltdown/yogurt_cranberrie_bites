create table if not exists follow_requests (
    id uuid primary key,
    follower_id uuid not null,
    followee_id uuid not null,
    status text not null,
    created_at timestamptz not null,
    decided_at timestamptz null,
    check (follower_id <> followee_id),
    check (status in ('PENDING', 'ACCEPTED', 'REJECTED', 'CANCELLED'))
);

create unique index if not exists follow_requests_pending_pair_idx
    on follow_requests (follower_id, followee_id)
    where status = 'PENDING';
create index if not exists follow_requests_incoming_idx
    on follow_requests (followee_id, created_at desc, id desc)
    where status = 'PENDING';
create index if not exists follow_requests_outgoing_idx
    on follow_requests (follower_id, created_at desc, id desc)
    where status = 'PENDING';
