alter table follows
    add column if not exists deleted_at timestamptz null,
    add column if not exists deleted_by uuid null,
    add column if not exists delete_reason text null,
    add column if not exists retention_until timestamptz null;

create table if not exists friend_audit_events (
    id uuid primary key,
    actor_id uuid null,
    follower_id uuid null,
    followee_id uuid null,
    action text not null,
    outcome text not null,
    request_id text null,
    ip_address text null,
    user_agent text null,
    metadata_json text not null default '{}',
    occurred_at timestamptz not null default now()
);

create index if not exists friend_audit_events_actor_idx
    on friend_audit_events (actor_id, occurred_at desc);
