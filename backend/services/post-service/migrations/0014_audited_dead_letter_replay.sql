create table post_event_dead_letter_resolutions (
    resolution_id uuid primary key,
    event_id uuid not null references post_event_outbox(event_id) on delete restrict,
    actor_id uuid not null,
    reason text not null check (
        reason = btrim(reason)
        and char_length(reason) between 20 and 1024
        and octet_length(reason) <= 1024
        and reason !~ '[[:cntrl:]]'
    ),
    evidence_reference text not null check (
        evidence_reference ~ '^[A-Za-z0-9][A-Za-z0-9._:/#-]{2,255}$'
    ),
    created_at timestamptz not null
);

comment on table post_event_dead_letter_resolutions is
    'Append-only, idempotent operator decisions that release one dead-lettered outbox head for bounded automatic retry.';

create or replace function reject_post_event_dead_letter_resolution_mutation()
returns trigger
language plpgsql
as $$
begin
    raise exception 'post_event_dead_letter_resolutions is append-only';
end;
$$;

create trigger post_event_dead_letter_resolutions_append_only
before update or delete on post_event_dead_letter_resolutions
for each row execute function reject_post_event_dead_letter_resolution_mutation();

alter table post_event_outbox
    add column replay_resolution_id uuid null
        references post_event_dead_letter_resolutions(resolution_id) on delete restrict;

alter table post_event_delivery_attempts
    add column resolution_id uuid null
        references post_event_dead_letter_resolutions(resolution_id) on delete restrict,
    add constraint post_event_delivery_attempts_resolution_source_check
        check (resolution_id is null or source = 'ADMIN_REPLAY');
