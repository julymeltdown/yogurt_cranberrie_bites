create table moderation_content_decision_outbox (
    report_id uuid primary key references moderation_reports(id) on delete cascade,
    actor_id uuid not null,
    decision_type text not null,
    note text not null,
    state text not null default 'PENDING',
    attempt_count integer not null default 0,
    next_attempt_at timestamptz not null default now(),
    lease_owner uuid null,
    lease_expires_at timestamptz null,
    last_error_code text null,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now(),
    completed_at timestamptz null,
    check (decision_type in ('HIDE_CONTENT', 'RESTORE_CONTENT')),
    check (state in ('PENDING', 'TARGET_UNAVAILABLE', 'COMPLETED')),
    check (attempt_count >= 0),
    check ((lease_owner is null) = (lease_expires_at is null)),
    check ((state = 'COMPLETED') = (completed_at is not null))
);

create index moderation_content_decision_outbox_due_idx
    on moderation_content_decision_outbox (next_attempt_at, created_at, report_id)
    where state = 'PENDING';
