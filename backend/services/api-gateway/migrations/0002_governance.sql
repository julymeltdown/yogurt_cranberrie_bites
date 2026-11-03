alter table route_contracts
    add column if not exists deleted_at timestamptz null,
    add column if not exists deleted_by text null,
    add column if not exists delete_reason text null,
    add column if not exists retention_until timestamptz null;

alter table policy_sets
    add column if not exists deleted_at timestamptz null,
    add column if not exists deleted_by text null,
    add column if not exists delete_reason text null,
    add column if not exists retention_until timestamptz null;

alter table client_profiles
    add column if not exists deleted_at timestamptz null,
    add column if not exists deleted_by text null,
    add column if not exists delete_reason text null,
    add column if not exists retention_until timestamptz null;

create table if not exists admin_audit_events (
    id uuid primary key,
    actor_id text null,
    action text not null,
    resource_type text not null,
    resource_id text null,
    outcome text not null,
    request_id text null,
    ip_address text null,
    user_agent text null,
    metadata_json text not null default '{}',
    occurred_at timestamptz not null default now()
);

create index if not exists admin_audit_events_actor_idx
    on admin_audit_events (actor_id, occurred_at desc);
create index if not exists admin_audit_events_resource_idx
    on admin_audit_events (resource_type, resource_id, occurred_at desc);
