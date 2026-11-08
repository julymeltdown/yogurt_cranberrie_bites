-- WP-08 T-702: the profile-owned avatar.
--
-- The foundation stores the emo character per concert participant in
-- `concert_avatar_settings`, keyed by `(event_id, user_id)` with a foreign key to
-- `concert_participants`. A personal profile must outlive any concert — and a
-- user may never have been a participant at all — so the profile avatar gets its
-- own table keyed by the profile alone. The concert table and its operations are
-- untouched.
--
-- Shape is domain-contract.md §4 (`owner_profile_id UNIQUE`, `config jsonb`,
-- `revision`, `updated_at`), plus the named constraints a negative test asserts.
create table profile_avatars (
    owner_profile_id uuid primary key
        check (owner_profile_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    -- The canonical wire config (`core.AvatarConfig`), stored as one closed
    -- object. The avatar catalog validates its contents; the column only fixes
    -- the shape so a non-object can never be stored.
    config jsonb not null,
    revision bigint not null default 1,
    created_at timestamptz not null default clock_timestamp(),
    updated_at timestamptz not null default clock_timestamp(),

    constraint profile_avatars_config_object
        check (jsonb_typeof(config) = 'object'),
    constraint profile_avatars_revision
        check (revision between 1 and 9007199254740991),
    constraint profile_avatars_times
        check (updated_at >= created_at)
);

-- The config is replaced wholesale by a save, and every save advances the
-- revision exactly once. A save that does not move the revision is a bug, not a
-- no-op, so it is refused rather than silently accepted. The owner is immutable:
-- a profile avatar can never be re-pointed at another profile.
create function profile_avatars_revision_guard() returns trigger language plpgsql as $$
begin
    if new.owner_profile_id is distinct from old.owner_profile_id
       or new.created_at is distinct from old.created_at then
        raise exception 'avatar owner is immutable'
            using errcode = '23514', constraint = 'profile_avatars_owner_immutable';
    end if;
    if new.revision <> old.revision + 1 then
        raise exception 'avatar revision must advance once'
            using errcode = '23514', constraint = 'profile_avatars_revision_step';
    end if;
    new.updated_at := clock_timestamp();
    return new;
end $$;

create trigger profile_avatars_revision_guard
    before update on profile_avatars
    for each row execute function profile_avatars_revision_guard();

-- The profile avatar is part of the account's private data and must be removed
-- by the account-deletion saga like every other profile-owned row. The existing
-- profile social deletion ledger (0085) records the profile-owned cleanup; this
-- table joins it, so a deletion cannot silently leave an avatar behind.
comment on table profile_avatars is
    'WP-08: profile-owned avatar. No event_id, no participant key.';
