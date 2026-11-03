create table if not exists notification_recipient_suppressions (
    user_id uuid primary key,
    reason text not null,
    suppressed_at timestamptz not null default now(),
    retention_until timestamptz null
);

create index if not exists notification_recipient_suppressions_retention_idx
    on notification_recipient_suppressions (retention_until, user_id)
    where retention_until is not null;
