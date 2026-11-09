-- 0018: backfill profile identities for accounts created after migration 0011.
--
-- Migration 0011 created `profile_identities` and backfilled it once, from the
-- `profiles` rows that existed at that moment. Nothing created an identity at
-- runtime, so every account registered afterwards had a `profiles` row and no
-- identity. That is not a cosmetic gap:
--
--   * `profile_identities` keys every profile-owned table in the concert service
--     (`profile_posts`, `profile_media`, `profile_rooms`, `profile_avatars`,
--     `intro_posts`), so such an account could not own any of them;
--   * the intro audience decision resolves a viewer through
--     `profile_identities` (`claim_read.rs`). With no row the read is
--     `ClaimRead::None`, the viewer's attributes are `Unavailable`, and every
--     restricted post is denied — including for the author's intended audience.
--
-- The runtime path now creates the identity in the same transaction as the
-- profile row (see `PgProfileRepository::save`). This migration repairs the
-- accounts that were created while it did not.
--
-- The derivation is the SAME expression 0011 used, so an account repaired here
-- and an account created at runtime get the same profile id. A different
-- derivation would give one account two profiles.
insert into profile_identities (profile_id, account_id, nickname, bio, visibility, created_at, updated_at, deleted_at)
select
    md5('mcr profile identity v1:' || p.user_id::text)::uuid,
    p.user_id,
    case
        when char_length(btrim(p.nickname)) between 1 and 40 then btrim(p.nickname)
        when char_length(btrim(p.username)) between 1 and 40 then btrim(p.username)
        else left(coalesce(nullif(btrim(p.username), ''), 'member'), 40)
    end,
    case when char_length(p.bio) between 1 and 160 then p.bio else null end,
    coalesce(p.account_visibility, 'PUBLIC'),
    p.created_at,
    p.updated_at,
    p.deleted_at
from profiles p
-- Only the accounts that are missing an identity. An account that already has
-- one keeps it: rewriting `profile_id` here would orphan every row that
-- references it.
where not exists (
    select 1 from profile_identities i where i.account_id = p.user_id
)
on conflict (profile_id) do nothing;

-- The repair must be total. A profile row still without an identity would be
-- invisible to every profile-owned table, which is the exact defect this
-- migration exists to remove.
do $$
begin
    if exists (
        select 1 from profiles p
        where not exists (
            select 1 from profile_identities i where i.account_id = p.user_id
        )
    ) then
        raise exception 'profile identity gap backfill is incomplete'
            using errcode = '23514', constraint = 'profile_identities_gap_backfill_complete';
    end if;
    if exists (
        select 1 from profile_identities i
        where i.deleted_at is null
        group by i.account_id having count(*) > 1
    ) then
        raise exception 'profile identity gap backfill produced a duplicate account'
            using errcode = '23514', constraint = 'profile_identities_gap_backfill_unique';
    end if;
end $$;
