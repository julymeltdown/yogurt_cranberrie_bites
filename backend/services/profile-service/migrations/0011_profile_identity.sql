-- WP-02 T-103: canonical profile identity.
--
-- The foundation's `profiles` table is keyed by the Auth user UUID and is the
-- only profile store today. The integration introduces a separate, immutable
-- `profile_id` so that every profile-owned table in the concert service can
-- reference a profile without a cross-database join and without inheriting the
-- Auth UUID's lifecycle.
--
-- This migration is additive only. It does not rewrite `profiles`, does not
-- change any existing column and does not touch an applied migration.

create table profile_identities (
    -- Generated once at creation and never rewritten. `gen_random_uuid` is the
    -- same source the foundation already uses for its epochs.
    profile_id uuid primary key default gen_random_uuid()
        check (profile_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    -- Canonical Auth user UUID. One live profile per account.
    account_id uuid not null check (account_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    nickname text not null,
    bio text,
    profile_image_media_id uuid,
    visibility text not null default 'PUBLIC',
    revision bigint not null default 1,
    created_at timestamptz not null default clock_timestamp(),
    updated_at timestamptz not null default clock_timestamp(),
    deleted_at timestamptz,
    constraint profile_identities_nickname_length
        check (char_length(nickname) between 1 and 40),
    constraint profile_identities_nickname_no_control
        check (nickname !~ '[\u0000-\u001f\u007f]'),
    constraint profile_identities_bio_length
        check (bio is null or char_length(bio) <= 160),
    constraint profile_identities_bio_no_control
        check (bio is null or bio !~ '[\u0000-\u001f\u007f]'),
    constraint profile_identities_visibility
        check (visibility in ('PUBLIC','PRIVATE')),
    constraint profile_identities_revision_range
        check (revision between 1 and 9007199254740991),
    constraint profile_identities_times
        check (updated_at >= created_at),
    constraint profile_identities_deleted_after_creation
        check (deleted_at is null or deleted_at >= created_at)
);

-- One live profile per account. A soft-deleted row does not block a new one, so
-- the unique index is partial rather than a table constraint.
create unique index profile_identities_live_account
    on profile_identities (account_id) where deleted_at is null;

create index profile_identities_live_visibility
    on profile_identities (visibility, account_id) where deleted_at is null;

-- `profile_id` is generated once and never rewritten. The account binding is
-- equally immutable: relinking an external account never moves a profile.
create function profile_identities_immutable_identity() returns trigger language plpgsql as $$
begin
    if new.profile_id is distinct from old.profile_id
       or new.account_id is distinct from old.account_id
       or new.created_at is distinct from old.created_at then
        raise exception 'profile identity is immutable'
            using errcode = '23514', constraint = 'profile_identities_immutable_identity';
    end if;
    if new.revision <> old.revision + 1 then
        raise exception 'profile revision must advance once'
            using errcode = '23514', constraint = 'profile_identities_revision_step';
    end if;
    new.updated_at := clock_timestamp();
    return new;
end $$;

create trigger profile_identities_immutable_identity
    before update on profile_identities
    for each row execute function profile_identities_immutable_identity();

create function profile_identities_no_truncate() returns trigger language plpgsql as $$
begin
    raise exception 'profile identity history cannot be truncated'
        using errcode = '23514', constraint = 'profile_identities_no_truncate';
end $$;

create trigger profile_identities_no_truncate
    before truncate on profile_identities
    for each statement execute function profile_identities_no_truncate();

-- Backfill: every existing profile row gets exactly one identity. The profile
-- id is derived deterministically from the account id so the migration is
-- repeatable in tests and never produces a different id on a re-run.
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
on conflict (profile_id) do nothing;

-- The backfill must be total: a profile row without an identity would be
-- invisible to every profile-owned table.
do $$
begin
    if exists (
        select 1 from profiles p
        where not exists (
            select 1 from profile_identities i
            where i.profile_id = md5('mcr profile identity v1:' || p.user_id::text)::uuid
        )
    ) then
        raise exception 'profile identity backfill is incomplete'
            using errcode = '23514', constraint = 'profile_identities_backfill_complete';
    end if;
    if exists (
        select 1 from profile_identities i
        group by i.account_id having count(*) > 1
    ) then
        raise exception 'profile identity backfill produced a duplicate account'
            using errcode = '23514', constraint = 'profile_identities_backfill_unique';
    end if;
end $$;
