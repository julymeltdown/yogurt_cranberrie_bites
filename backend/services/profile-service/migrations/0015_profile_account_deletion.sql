-- WP-02 T-111: account deletion coverage for the new profile-service tables.
--
-- The existing saga already tombstones the Auth user and soft-deletes the
-- legacy `profiles`/`avatars` rows. The tables added by this work package are
-- owned by the same account, so they are erased by the same saga step and in
-- the same order the saga already uses: children before parents.
--
-- Ordering for this service:
--   1. identity_verifications (child of the user, no other reference)
--   2. identity_claims        (child of the user)
--   3. profile_favorites      (child of the profile)
--   4. profile_identities     (the profile itself)
--
-- Nothing here changes an existing step, and nothing here can run before the
-- tombstone exists: the saga writes the tombstone first, and each function below
-- refuses a user that has no tombstone.

create function profile_erase_identity_claims(p_user uuid) returns bigint
language plpgsql as $$
declare removed bigint;
begin
    if not exists (select 1 from profile_user_deletion_tombstones where user_id = p_user) then
        raise exception 'identity claim erase requires an account deletion tombstone'
            using errcode = '23514', constraint = 'profile_erase_requires_tombstone';
    end if;
    delete from identity_verifications where user_id = p_user;
    delete from identity_claims where user_id = p_user;
    get diagnostics removed = row_count;
    return removed;
end $$;

create function profile_erase_favorites(p_profile uuid) returns bigint
language plpgsql as $$
declare removed bigint;
begin
    if not exists (
        select 1 from profile_identities i
        join profile_user_deletion_tombstones t on t.user_id = i.account_id
        where i.profile_id = p_profile
    ) then
        raise exception 'favorite erase requires an account deletion tombstone'
            using errcode = '23514', constraint = 'profile_erase_requires_tombstone';
    end if;
    delete from profile_favorites where profile_id = p_profile;
    get diagnostics removed = row_count;
    return removed;
end $$;

create function profile_erase_identity(p_profile uuid) returns bigint
language plpgsql as $$
declare removed bigint;
begin
    if not exists (
        select 1 from profile_identities i
        join profile_user_deletion_tombstones t on t.user_id = i.account_id
        where i.profile_id = p_profile
    ) then
        raise exception 'profile identity erase requires an account deletion tombstone'
            using errcode = '23514', constraint = 'profile_erase_requires_tombstone';
    end if;
    -- A profile row cannot be erased while it still owns favorites. The saga
    -- calls profile_erase_favorites first; this is the guard that proves it.
    if exists (select 1 from profile_favorites where profile_id = p_profile) then
        raise exception 'profile identity still owns favorites'
            using errcode = '23514', constraint = 'profile_erase_order';
    end if;
    delete from profile_identities where profile_id = p_profile;
    get diagnostics removed = row_count;
    return removed;
end $$;

-- Per-account deletion progress counters, extended with the new tables. The
-- counter row is created by the saga's first step and only ever advances.
create table profile_account_deletion_progress (
    user_id uuid primary key,
    saga_id uuid not null,
    erased_claims bigint not null default 0 check (erased_claims >= 0),
    erased_verifications bigint not null default 0 check (erased_verifications >= 0),
    erased_favorites bigint not null default 0 check (erased_favorites >= 0),
    erased_identities bigint not null default 0 check (erased_identities >= 0),
    progress_revision bigint not null default 0 check (progress_revision >= 0),
    created_at timestamptz not null default clock_timestamp(),
    updated_at timestamptz not null default clock_timestamp(),
    completed_at timestamptz,
    foreign key (user_id) references profile_user_deletion_tombstones(user_id),
    constraint profile_account_deletion_progress_completion
        check ((completed_at is null) or progress_revision > 0)
);

create function profile_account_deletion_progress_guard() returns trigger language plpgsql as $$
begin
    if tg_op = 'DELETE' then
        raise exception 'account deletion progress cannot be removed'
            using errcode = '23514', constraint = 'profile_account_deletion_progress_immutable';
    end if;
    if tg_op = 'UPDATE' then
        if (new.user_id, new.saga_id, new.created_at) is distinct from
           (old.user_id, old.saga_id, old.created_at)
           or new.progress_revision <> old.progress_revision + 1
           or new.erased_claims < old.erased_claims
           or new.erased_verifications < old.erased_verifications
           or new.erased_favorites < old.erased_favorites
           or new.erased_identities < old.erased_identities then
            raise exception 'account deletion progress is immutable or nonmonotonic'
                using errcode = '23514', constraint = 'profile_account_deletion_progress_monotonic';
        end if;
        -- Completion is only reachable when the account owns nothing left.
        if new.completed_at is not null and (
            exists (select 1 from identity_claims where user_id = new.user_id)
            or exists (select 1 from identity_verifications where user_id = new.user_id)
            or exists (
                select 1 from profile_identities i
                where i.account_id = new.user_id
                  and exists (select 1 from profile_favorites f where f.profile_id = i.profile_id)
            )
            or exists (select 1 from profile_identities i where i.account_id = new.user_id)
        ) then
            raise exception 'account deletion progress is not terminal'
                using errcode = '23514', constraint = 'profile_account_deletion_progress_terminal';
        end if;
        new.updated_at := clock_timestamp();
    end if;
    return new;
end $$;

create trigger profile_account_deletion_progress_guard
    before insert or update or delete on profile_account_deletion_progress
    for each row execute function profile_account_deletion_progress_guard();
