-- WP-02 T-107: personal room.
--
-- A room belongs to a profile, never to an event and never to an intro post.
-- There is deliberately no `event_id`, no `intro_id` and no participant key: the
-- room is part of the profile, so it survives every concert.
create table profile_rooms (
    owner_profile_id uuid primary key check (owner_profile_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    layout jsonb not null,
    visibility text not null default 'PRIVATE',
    revision bigint not null default 1,
    created_at timestamptz not null default clock_timestamp(),
    updated_at timestamptz not null default clock_timestamp(),
    constraint profile_rooms_visibility
        check (visibility in ('PUBLIC','PRIVATE')),
    constraint profile_rooms_revision
        check (revision between 1 and 9007199254740991),
    constraint profile_rooms_layout_object
        check (jsonb_typeof(layout) = 'object'),
    constraint profile_rooms_times
        check (updated_at >= created_at)
);

create index profile_rooms_public on profile_rooms (updated_at desc) where visibility = 'PUBLIC';

-- The layout is replaced wholesale by a save, and every save advances the
-- revision exactly once. A save that does not move the revision is a bug, not a
-- no-op, so it is refused rather than silently accepted.
create function profile_rooms_revision_guard() returns trigger language plpgsql as $$
begin
    if new.owner_profile_id is distinct from old.owner_profile_id
       or new.created_at is distinct from old.created_at then
        raise exception 'room owner is immutable'
            using errcode = '23514', constraint = 'profile_rooms_owner_immutable';
    end if;
    if new.revision <> old.revision + 1 then
        raise exception 'room revision must advance once'
            using errcode = '23514', constraint = 'profile_rooms_revision_step';
    end if;
    new.updated_at := clock_timestamp();
    return new;
end $$;

create trigger profile_rooms_revision_guard
    before update on profile_rooms
    for each row execute function profile_rooms_revision_guard();
