alter table post_event_outbox
    add column if not exists aggregate_version bigint null;

create table if not exists post_event_aggregate_sequences (
    aggregate_id uuid primary key,
    last_version bigint not null check (last_version >= 0)
);

-- Keep legacy events explicitly unsequenced. New events for an aggregate start
-- at version 1; workers drain any legacy unpublished rows before versioned rows.
insert into post_event_aggregate_sequences (aggregate_id, last_version)
select distinct aggregate_id, 0
from post_event_outbox
on conflict (aggregate_id) do nothing;

create or replace function assign_post_event_aggregate_version()
returns trigger
language plpgsql
as $$
begin
    -- The version is database-owned so every producer participates in the same
    -- per-aggregate serialization, including writers that omit this column.
    insert into post_event_aggregate_sequences (aggregate_id, last_version)
    values (new.aggregate_id, 1)
    on conflict (aggregate_id) do update
        set last_version = post_event_aggregate_sequences.last_version + 1
    returning last_version into new.aggregate_version;
    return new;
end;
$$;

drop trigger if exists post_event_outbox_assign_aggregate_version
    on post_event_outbox;
create trigger post_event_outbox_assign_aggregate_version
before insert on post_event_outbox
for each row execute function assign_post_event_aggregate_version();

create index if not exists post_outbox_aggregate_pending_idx
    on post_event_outbox (aggregate_id, aggregate_version, created_at, event_id)
    where published = false;
