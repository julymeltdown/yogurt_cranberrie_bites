alter table follow_requests
    add column if not exists deleted_at timestamptz null,
    add column if not exists deleted_by uuid null,
    add column if not exists delete_reason text null,
    add column if not exists retention_until timestamptz null;

alter table user_blocks
    add column if not exists deleted_by uuid null,
    add column if not exists delete_reason text null,
    add column if not exists retention_until timestamptz null;

alter table user_mutes
    add column if not exists deleted_by uuid null,
    add column if not exists delete_reason text null,
    add column if not exists retention_until timestamptz null;

create index if not exists follow_requests_retention_due_idx
    on follow_requests (retention_until, id)
    where deleted_at is not null and retention_until is not null;
