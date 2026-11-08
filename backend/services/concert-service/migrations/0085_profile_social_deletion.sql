-- WP-02 T-111: account deletion coverage for the new concert-service tables.
--
-- This migration does not change the existing saga. It adds:
--
--   * the profile-owned tables to the same per-user advisory lock the saga
--     already uses, so a writer cannot recreate content behind the deletion;
--   * an explicit reference-checked media erase, so a media file shared by two
--     posts survives until both references are gone;
--   * progress counters for the new tables in the same monotonic shape the saga
--     already uses for its other categories.
--
-- Ordering is children before parents, matching the saga:
--   intro_comments, intro_likes, intro_attachments, intro_posts,
--   profile_post_attachments, profile_posts, profile_rooms, profile_media.

-- The saga's common user lock key, spelled once here so both sides agree.
create function profile_social_lock_user(p_user uuid) returns void language plpgsql as $$
begin
    perform pg_advisory_xact_lock(hashtextextended('mcr-world-account-deletion:' || p_user::text, 0));
end $$;

-- Every normal writer takes the common user lock before touching a profile-owned
-- row, and is refused once the account is fenced. Deletion itself runs under the
-- fence and therefore bypasses the refusal through the explicit guard below.
create function profile_social_write_guard() returns trigger language plpgsql as $$
declare subject uuid;
begin
    -- A CASE expression resolves every branch against the actual record, so a
    -- branch naming a column this table lacks is an error even when it is not
    -- taken. IF/ELSIF evaluates only the branch it enters.
    if tg_table_name in ('intro_posts','profile_posts','intro_comments') then
        subject := new.author_profile_id;
    elsif tg_table_name = 'intro_likes' then
        subject := new.actor_profile_id;
    elsif tg_table_name in ('profile_rooms','profile_media') then
        subject := new.owner_profile_id;
    else
        subject := null;
    end if;
    if subject is null then
        return new;
    end if;
    perform profile_social_lock_user(subject);
    if exists (select 1 from concert_world_account_deletion_fences where user_id = subject) then
        raise exception 'profile social account is permanently fenced'
            using errcode = '23514', constraint = 'profile_social_account_fenced';
    end if;
    return new;
end $$;

create trigger intro_posts_account_guard
    before insert on intro_posts
    for each row execute function profile_social_write_guard();
create trigger profile_posts_account_guard
    before insert on profile_posts
    for each row execute function profile_social_write_guard();
create trigger profile_rooms_account_guard
    before insert on profile_rooms
    for each row execute function profile_social_write_guard();
create trigger intro_comments_account_guard
    before insert on intro_comments
    for each row execute function profile_social_write_guard();
create trigger intro_likes_account_guard
    before insert on intro_likes
    for each row execute function profile_social_write_guard();

-- Page indexes for the deletion walk, in the same shape the media saga uses.
create index intro_posts_owner_deletion on intro_posts (author_profile_id, intro_id);
create index profile_posts_owner_deletion on profile_posts (author_profile_id, post_id);
create index profile_media_owner_deletion on profile_media (owner_profile_id, media_id);

-- Media erase is reference-checked. A media file still referenced by a live
-- intro attachment or profile post attachment is NOT erased; the saga must
-- delete the referencing post first. This is the "survives until both are
-- deleted" rule from domain-contract.md §4.4.
create function profile_media_erase_unreferenced(p_media uuid) returns boolean
language plpgsql as $$
declare remaining integer;
begin
    select (select count(*) from intro_attachments where media_id = p_media)
         + (select count(*) from profile_post_attachments where media_id = p_media)
      into remaining;
    if remaining > 0 then
        return false;
    end if;
    update profile_media
       set state = 'DELETED', sanitized = null, width = null, height = null
     where media_id = p_media and state <> 'DELETED';
    return true;
end $$;

create table profile_social_account_deletions (
    user_id uuid primary key check (user_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    saga_id uuid not null,
    actor_id uuid not null check (actor_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    category text not null default 'PROFILE_SOCIAL' check (category = 'PROFILE_SOCIAL'),
    category_version integer not null default 1 check (category_version = 1),
    retention_seconds bigint not null,
    retention_nanos integer not null check (retention_nanos between 0 and 999999999),
    identity_binding bytea not null check (octet_length(identity_binding) = 32),
    command_binding bytea not null check (octet_length(command_binding) = 32),
    source_binding bytea not null check (octet_length(source_binding) = 32
                                          and source_binding <> decode(repeat('00',32),'hex')),
    fence_revision bigint not null check (fence_revision > 0),
    deleted_intro_posts bigint not null default 0 check (deleted_intro_posts >= 0),
    deleted_profile_posts bigint not null default 0 check (deleted_profile_posts >= 0),
    deleted_rooms bigint not null default 0 check (deleted_rooms >= 0),
    erased_media bigint not null default 0 check (erased_media >= 0),
    retained_media bigint not null default 0 check (retained_media >= 0),
    progress_revision bigint not null default 0 check (progress_revision >= 0),
    completed_at timestamptz,
    created_at timestamptz not null default clock_timestamp(),
    updated_at timestamptz not null default clock_timestamp(),
    foreign key (user_id, saga_id) references concert_world_account_deletion_fences(user_id, saga_id),
    constraint profile_social_account_deletions_terminal
        check ((completed_at is null) or progress_revision > 0)
);

create function profile_social_account_deletions_guard() returns trigger language plpgsql as $$
begin
    if tg_op = 'DELETE' then
        raise exception 'profile social deletion receipt is immutable'
            using errcode = '23514', constraint = 'profile_social_account_deletions_immutable';
    end if;
    if tg_op = 'INSERT' then
        if new.progress_revision <> 0 or new.deleted_intro_posts <> 0
           or new.deleted_profile_posts <> 0 or new.deleted_rooms <> 0
           or new.erased_media <> 0 or new.retained_media <> 0
           or new.completed_at is not null then
            raise exception 'profile social deletion must start empty'
                using errcode = '23514', constraint = 'profile_social_account_deletions_start';
        end if;
        return new;
    end if;
    if (new.user_id, new.saga_id, new.actor_id, new.category, new.category_version,
        new.retention_seconds, new.retention_nanos, new.identity_binding,
        new.command_binding, new.source_binding, new.fence_revision, new.created_at)
       is distinct from
       (old.user_id, old.saga_id, old.actor_id, old.category, old.category_version,
        old.retention_seconds, old.retention_nanos, old.identity_binding,
        old.command_binding, old.source_binding, old.fence_revision, old.created_at)
       or old.completed_at is not null
       or new.progress_revision <> old.progress_revision + 1
       or new.deleted_intro_posts < old.deleted_intro_posts
       or new.deleted_profile_posts < old.deleted_profile_posts
       or new.deleted_rooms < old.deleted_rooms
       or new.erased_media < old.erased_media
       or new.retained_media < old.retained_media then
        raise exception 'profile social deletion receipt is immutable or nonmonotonic'
            using errcode = '23514', constraint = 'profile_social_account_deletions_monotonic';
    end if;
    -- Completion requires that the account owns nothing left in these tables.
    if new.completed_at is not null and (
        exists (select 1 from intro_posts where author_profile_id = new.user_id)
        or exists (select 1 from profile_posts where author_profile_id = new.user_id)
        or exists (select 1 from profile_rooms where owner_profile_id = new.user_id)
        or exists (select 1 from profile_media where owner_profile_id = new.user_id and state <> 'DELETED')
    ) then
        raise exception 'profile social category is not terminal'
            using errcode = '23514', constraint = 'profile_social_account_deletions_terminal';
    end if;
    new.updated_at := clock_timestamp();
    return new;
end $$;

create trigger profile_social_account_deletions_guard
    before insert or update or delete on profile_social_account_deletions
    for each row execute function profile_social_account_deletions_guard();

create trigger profile_social_account_deletions_no_truncate
    before truncate on profile_social_account_deletions
    for each statement execute function profile_social_account_deletions_guard();
