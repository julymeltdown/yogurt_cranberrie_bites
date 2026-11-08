-- WP-02 T-106: profile-scoped media.
--
-- The foundation's `concert_media` is event-scoped: `(event_id, owner_id)` with
-- `purpose IN ('POST','GIVEAWAY','REPORT')` and a `concert_participants` foreign
-- key. A profile must outlive any event and a user may never have been a concert
-- participant, so profile media gets its own table rather than a loosened
-- `concert_media`. The concert table and its operations stay untouched.
--
-- Shape is exactly http-contract.md §4.1, plus the named constraints a negative
-- test must be able to assert.
create table profile_media (
    media_id uuid primary key check (media_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    owner_profile_id uuid not null check (owner_profile_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    purpose text not null,
    content_type text not null,
    state text not null,
    declared_bytes integer not null,
    sanitized bytea,
    width integer,
    height integer,
    input_sha256 bytea not null,
    revision bigint not null default 1,
    created_at timestamptz not null default clock_timestamp(),
    updated_at timestamptz not null default clock_timestamp(),

    -- Closed purpose set. These are the profile purposes only; the concert
    -- purposes ('POST','GIVEAWAY','REPORT') belong to concert_media and are
    -- rejected here.
    constraint profile_media_purpose
        check (purpose in ('PROFILE_IMAGE','PROFILE_POST','INTRO_POST','ROOM_POSTER')),
    constraint profile_media_content_type
        check (content_type in ('image/jpeg','image/png','image/webp')),
    constraint profile_media_state
        check (state in ('PENDING','UPLOADED','READY','DELETED')),
    constraint profile_media_declared_bytes
        check (declared_bytes between 1 and 10485760),
    constraint profile_media_input_sha256
        check (octet_length(input_sha256) = 32),
    constraint profile_media_revision
        check (revision between 1 and 9007199254740991),
    constraint profile_media_dimension
        check (width is null or width between 1 and 4096),
    constraint profile_media_height
        check (height is null or height between 1 and 4096),
    constraint profile_media_pixels
        check (width is null or height is null or width::bigint * height <= 16000000),
    constraint profile_media_times
        check (updated_at >= created_at),

    -- State machine, mirroring concert_media:
    --   PENDING  has no bytes and no dimensions;
    --   UPLOADED has staged bytes (sanitized slot, exactly declared_bytes) and
    --            no dimensions yet;
    --   READY    has sanitized bytes plus width and height;
    --   DELETED  has nothing.
    constraint profile_media_state_bytes
        check ((state = 'PENDING'
                and sanitized is null and width is null and height is null)
            or (state = 'UPLOADED'
                and sanitized is not null and octet_length(sanitized) = declared_bytes
                and width is null and height is null)
            or (state = 'READY'
                and sanitized is not null
                and octet_length(sanitized) between 1 and 10485760
                and width is not null and height is not null)
            or (state = 'DELETED'
                and sanitized is null and width is null and height is null))
);

create index profile_media_owner on profile_media (owner_profile_id, purpose, created_at desc);
create index profile_media_cleanup on profile_media (created_at) where state in ('PENDING','UPLOADED');

-- Owner, purpose and content type are fixed at creation. Only the state machine
-- may advance, and published bytes cannot be rewritten.
create function profile_media_transition_guard() returns trigger language plpgsql as $$
begin
    if (new.media_id, new.owner_profile_id, new.purpose, new.content_type,
        new.declared_bytes, new.input_sha256, new.created_at)
       is distinct from
       (old.media_id, old.owner_profile_id, old.purpose, old.content_type,
        old.declared_bytes, old.input_sha256, old.created_at) then
        raise exception 'profile media identity is immutable'
            using errcode = '23514', constraint = 'profile_media_identity_immutable';
    end if;
    if not ((old.state = 'PENDING' and new.state in ('UPLOADED','DELETED'))
            or (old.state = 'UPLOADED' and new.state in ('READY','DELETED'))
            or (old.state = 'READY' and new.state in ('READY','DELETED'))
            or (old.state = 'DELETED' and new.state = 'DELETED')) then
        raise exception 'invalid profile media transition'
            using errcode = '23514', constraint = 'profile_media_transition';
    end if;
    if old.state = 'READY' and new.state = 'READY'
       and (new.sanitized is distinct from old.sanitized
            or new.width is distinct from old.width
            or new.height is distinct from old.height) then
        raise exception 'published profile media is immutable'
            using errcode = '23514', constraint = 'profile_media_published_immutable';
    end if;
    new.revision := old.revision + 1;
    new.updated_at := clock_timestamp();
    return new;
end $$;

create trigger profile_media_transition_guard
    before update on profile_media
    for each row execute function profile_media_transition_guard();
