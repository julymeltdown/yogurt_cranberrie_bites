create table if not exists notifications (
    id uuid primary key,
    user_id uuid not null,
    type text not null,
    payload_json text not null default '',
    created_at timestamptz not null,
    read_at timestamptz null,
    event_id uuid not null,
    actor_id uuid null,
    occurred_at timestamptz not null
);

create unique index if not exists notifications_user_event_type_idx
    on notifications (user_id, event_id, type);
create index if not exists notifications_user_created_idx
    on notifications (user_id, created_at desc, id desc);
