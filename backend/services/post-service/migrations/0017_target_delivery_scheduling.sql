-- Operational target state is mutable; immutable attempt/publication history
-- remains in its existing tables. Deploy only with old outbox workers drained.
alter table post_event_outbox add column delivery_required_targets text[] null;
alter table post_event_outbox add constraint post_delivery_targets_known check (
    delivery_required_targets <@ array['TIMELINE','CACHE','INTERNAL','EXTERNAL']::text[]
);

-- Bind release provenance to the event that the immutable decision names.
alter table post_event_dead_letter_resolutions
    add constraint post_resolution_event_identity unique (resolution_id,event_id);

create table post_event_delivery_jobs (
    event_id uuid not null references post_event_outbox(event_id) on delete cascade,
    target text not null check (target in ('TIMELINE','CACHE','INTERNAL','EXTERNAL')),
    state text not null check (state in ('PENDING','LEASED','RETRY','SUCCEEDED','SKIPPED','DEAD')),
    attempt_count integer not null default 0 check (attempt_count >= 0),
    max_attempts integer not null default 5 check (max_attempts between 1 and 1000),
    retry_base_ms bigint not null default 1000 check (retry_base_ms between 1 and 86400000),
    retry_cap_ms bigint not null default 300000 check (retry_cap_ms between retry_base_ms and 86400000),
    release_resolution_id uuid null,
    foreign key (release_resolution_id,event_id)
        references post_event_dead_letter_resolutions(resolution_id,event_id) on delete restrict,
    next_attempt_at timestamptz not null default now(),
    lease_token uuid null,
    attempt_id uuid null,
    lease_expires_at timestamptz null,
    primary key (event_id,target),
    check ((state = 'LEASED') = (lease_token is not null and attempt_id is not null and lease_expires_at is not null)),
    check (state = 'LEASED' or (lease_token is null and attempt_id is null and lease_expires_at is null))
);

create index post_delivery_jobs_ready_idx
    on post_event_delivery_jobs (target,next_attempt_at,event_id)
    where state in ('PENDING','RETRY','LEASED');

insert into post_event_delivery_jobs (event_id,target,state,attempt_count,release_resolution_id)
select event.event_id, target.name,
       case when publication.event_id is not null then 'SUCCEEDED'
            when event.published then 'SKIPPED'
            when event.dead_lettered_at is not null then 'DEAD'
            else 'PENDING' end,
       case when event.dead_lettered_at is not null and publication.event_id is null
            then event.attempt_count else 0 end,
       case when not event.published and publication.event_id is null
            then event.replay_resolution_id else null end
from post_event_outbox event
cross join (values ('TIMELINE'),('CACHE'),('INTERNAL'),('EXTERNAL')) target(name)
left join event_publications publication
    on publication.event_id=event.event_id and publication.target=target.name;

-- Published legacy omissions are not inferred to be successful deliveries.
update post_event_outbox event
set delivery_required_targets = array(
    select job.target from post_event_delivery_jobs job
    where job.event_id=event.event_id and job.state='SUCCEEDED' order by job.target
)
where event.published;

create function create_post_delivery_jobs() returns trigger language plpgsql as $$
begin
    insert into post_event_delivery_jobs (event_id,target,state)
    select new.event_id, name, 'PENDING'
    from (values ('TIMELINE'),('CACHE'),('INTERNAL'),('EXTERNAL')) target(name);
    return new;
end;
$$;
create trigger post_delivery_jobs_after_insert after insert on post_event_outbox
for each row execute function create_post_delivery_jobs();

comment on table post_event_delivery_jobs is
    'Per-target scheduling/leases; completed legacy checkpoints are inherited, omitted deliveries are not successes. Old unfenced workers must be stopped before this scheduler is enabled.';
