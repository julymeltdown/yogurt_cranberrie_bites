create table if not exists moderation_reports (
    id uuid primary key,
    reporter_id uuid not null,
    target_type text not null,
    target_id uuid not null,
    category text not null,
    reason text not null,
    status text not null default 'OPEN',
    created_at timestamptz not null default now(),
    resolved_at timestamptz null,
    unique (reporter_id, target_type, target_id, category),
    check (target_type in ('USER', 'POST', 'COMMENT')),
    check (category in ('SPAM', 'HARASSMENT', 'HATE', 'VIOLENCE', 'OTHER')),
    check (status in ('OPEN', 'RESOLVED', 'DISMISSED'))
);

create table if not exists moderation_decisions (
    id uuid primary key,
    report_id uuid not null references moderation_reports(id) on delete cascade,
    actor_id uuid not null,
    decision_type text not null,
    note text not null default '',
    created_at timestamptz not null default now(),
    unique (report_id, decision_type),
    check (decision_type in (
        'DISMISS', 'HIDE_CONTENT', 'RESTORE_CONTENT', 'RESTRICT_USER', 'LIFT_RESTRICTION'
    ))
);

create table if not exists user_restrictions (
    user_id uuid primary key,
    report_id uuid not null references moderation_reports(id),
    reason text not null,
    restricted_by uuid not null,
    restricted_at timestamptz not null default now(),
    expires_at timestamptz null,
    lifted_at timestamptz null
);

create index if not exists moderation_reports_reporter_created_idx
    on moderation_reports (reporter_id, created_at desc, id desc);
create index if not exists moderation_reports_open_created_idx
    on moderation_reports (created_at asc, id asc)
    where status = 'OPEN';
create index if not exists moderation_decisions_report_created_idx
    on moderation_decisions (report_id, created_at asc, id asc);
create index if not exists user_restrictions_active_idx
    on user_restrictions (user_id, expires_at)
    where lifted_at is null;
