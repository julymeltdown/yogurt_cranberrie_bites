create table if not exists post_event_delivery_attempts (
    id bigint generated always as identity primary key,
    attempt_id uuid not null,
    event_id uuid not null references post_event_outbox(event_id),
    target text not null check (target in ('TIMELINE', 'CACHE', 'INTERNAL', 'EXTERNAL')),
    source text not null check (source in ('OUTBOX_WORKER', 'ADMIN_REPLAY')),
    phase text not null check (phase in ('STARTED', 'SUCCEEDED', 'FAILED')),
    occurred_at timestamptz not null,
    unique (attempt_id, phase)
);

create index if not exists post_event_delivery_attempts_event_idx
    on post_event_delivery_attempts (event_id, id);

create unique index if not exists post_event_delivery_attempts_one_terminal_idx
    on post_event_delivery_attempts (attempt_id)
    where phase in ('SUCCEEDED', 'FAILED');

comment on table post_event_delivery_attempts is
    'Append-only target attempt markers; intentionally excludes payloads and free-form errors.';

create or replace function reject_post_event_delivery_attempt_mutation()
returns trigger
language plpgsql
as $$
begin
    raise exception 'post_event_delivery_attempts is append-only';
end;
$$;

create trigger post_event_delivery_attempts_append_only
before update or delete on post_event_delivery_attempts
for each row execute function reject_post_event_delivery_attempt_mutation();
