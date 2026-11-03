-- Profile and Auth own separate databases, so keep a local identity tombstone
-- to prevent stale Gateway tokens or delayed internal writes from recreating
-- a profile or avatar after the deletion saga's Profile step has committed.
create table profile_user_deletion_tombstones (
    user_id uuid primary key,
    saga_id uuid not null unique,
    actor_id uuid not null,
    retention_until timestamptz not null,
    applied_at timestamptz not null default now()
);

create or replace function profile_lock_subject_ids(subject_ids uuid[])
returns void
language plpgsql
as $$
declare
    lock_key bigint;
begin
    for lock_key in
        select distinct hashtextextended(subject_id::text, 0) as lock_key
        from unnest(coalesce(subject_ids, '{}'::uuid[])) as subjects(subject_id)
        where subject_id is not null
        order by lock_key
    loop
        perform pg_advisory_xact_lock(lock_key);
    end loop;
end;
$$;

create or replace function profile_assert_subjects_not_deleted(subject_ids uuid[])
returns void
language plpgsql
as $$
declare
    subject_id uuid;
begin
    perform profile_lock_subject_ids(subject_ids);

    for subject_id in
        select distinct candidate
        from unnest(coalesce(subject_ids, '{}'::uuid[])) as subjects(candidate)
        where candidate is not null
        order by candidate
    loop
        if exists (
            select 1 from profile_user_deletion_tombstones tombstone
            where tombstone.user_id = subject_id
        ) then
            raise exception 'Profile mutation references a deleted user'
                using errcode = '23514',
                      constraint = 'profile_user_deletion_fence';
        end if;
    end loop;
end;
$$;

create or replace function profile_guard_user_deletion_fence()
returns trigger
language plpgsql
as $$
begin
    -- Profile hiding and account deletion both soft-delete rows. The saga
    -- writes the tombstone first; allowing this transition lets its cleanup
    -- finish while still rejecting every active insert/reactivation.
    if new.deleted_at is not null then
        return new;
    end if;

    perform profile_assert_subjects_not_deleted(array[new.user_id]);
    return new;
end;
$$;

create trigger profiles_user_deletion_fence
    before insert or update on profiles
    for each row execute function profile_guard_user_deletion_fence();
create trigger avatars_user_deletion_fence
    before insert or update on avatars
    for each row execute function profile_guard_user_deletion_fence();
