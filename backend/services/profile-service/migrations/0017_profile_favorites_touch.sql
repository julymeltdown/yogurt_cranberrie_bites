-- Discovery projection: make a favorites change visible to the source watermark.
--
-- The projection writer reads profiles in `(changed_at, profile_id)` order and
-- advances a watermark, so it only ever sees rows whose change time moved
-- forward. `profile_identities.updated_at` moves when the profile row is
-- updated, and `profile_favorites.created_at` records when a pick was added —
-- but a *removal* leaves no trace at all, so a watermark on either column alone
-- would silently stop tracking a profile whose picks were cleared.
--
-- This migration closes that hole at the source. A favorites insert or delete
-- bumps the owning profile's `revision` and `updated_at`, using the same
-- revision discipline the identity trigger already enforces (exactly +1 per
-- update). The projection's `changed_at` is then the profile row's
-- `updated_at`, which moves for every favorites change — add or remove.
--
-- Why a trigger rather than a command: the favorites table is written by more
-- than one path (the favorites command, the account-deletion saga, a direct
-- operator repair), and a rule that lives only in one command is a rule the
-- other writers do not obey. The trigger makes "a favorites change is a profile
-- change" true for every writer.
--
-- The trigger is deliberately narrow: it touches only the owning profile row's
-- revision and timestamp. It does not cascade, does not notify, and writes no
-- other table.

create function profile_favorites_touch_profile() returns trigger language plpgsql as $$
declare
    target uuid;
begin
    -- A delete names the old row; an insert names the new one.
    if tg_op = 'DELETE' then
        target := old.profile_id;
    else
        target := new.profile_id;
    end if;

    -- The identity trigger requires `revision = old.revision + 1`, so this is a
    -- single-row update that satisfies it exactly. `updated_at` is set by that
    -- same trigger to `clock_timestamp()`.
    update profile_identities
       set revision = revision + 1
     where profile_id = target;

    if tg_op = 'DELETE' then
        return old;
    end if;
    return new;
end $$;

create trigger profile_favorites_touch_profile
    after insert or delete on profile_favorites
    for each row execute function profile_favorites_touch_profile();
