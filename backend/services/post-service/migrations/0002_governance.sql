alter table posts
    add column if not exists updated_at timestamptz not null default now(),
    add column if not exists deleted_at timestamptz null,
    add column if not exists deleted_by uuid null,
    add column if not exists delete_reason text null,
    add column if not exists retention_until timestamptz null;

alter table comments
    add column if not exists updated_at timestamptz not null default now(),
    add column if not exists deleted_at timestamptz null,
    add column if not exists deleted_by uuid null,
    add column if not exists delete_reason text null,
    add column if not exists retention_until timestamptz null;

alter table post_likes
    add column if not exists deleted_at timestamptz null,
    add column if not exists deleted_by uuid null,
    add column if not exists delete_reason text null,
    add column if not exists retention_until timestamptz null;

alter table post_event_outbox
    add column if not exists attempt_count integer not null default 0,
    add column if not exists last_error text null,
    add column if not exists locked_at timestamptz null,
    add column if not exists dead_lettered_at timestamptz null;

create table if not exists post_audit_events (
    id uuid primary key,
    actor_id uuid null,
    post_id uuid null,
    comment_id uuid null,
    action text not null,
    outcome text not null,
    request_id text null,
    ip_address text null,
    user_agent text null,
    metadata_json text not null default '{}',
    occurred_at timestamptz not null default now()
);

create index if not exists post_audit_events_post_idx
    on post_audit_events (post_id, occurred_at desc);
create index if not exists post_outbox_retry_idx
    on post_event_outbox (dead_lettered_at, locked_at, attempt_count, created_at);
