-- WP-02 T-110: profile posts and their attachments.
--
-- A profile post is independent of any introduction post: it has its own
-- visibility, its own state and no `intro_id`. Attachments are associations
-- only, exactly like intro attachments.
create table profile_posts (
    post_id uuid primary key check (post_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    author_profile_id uuid not null check (author_profile_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    description text not null default '',
    visibility text not null default 'PUBLIC',
    state text not null default 'PUBLISHED',
    expires_at timestamptz,
    revision bigint not null default 1,
    created_at timestamptz not null default clock_timestamp(),
    updated_at timestamptz not null default clock_timestamp(),
    deleted_at timestamptz,
    constraint profile_posts_description
        check (char_length(description) <= 500),
    constraint profile_posts_visibility
        check (visibility in ('PUBLIC','PRIVATE')),
    constraint profile_posts_state
        check (state in ('PUBLISHED','DELETED')),
    constraint profile_posts_revision
        check (revision between 1 and 9007199254740991),
    constraint profile_posts_expiry
        check (expires_at is null or expires_at > created_at),
    constraint profile_posts_times
        check (updated_at >= created_at),
    -- A deleted post carries a deletion instant; a live post does not.
    constraint profile_posts_deleted_shape
        check ((state = 'DELETED') = (deleted_at is not null))
);

create index profile_posts_author
    on profile_posts (author_profile_id, created_at desc, post_id desc)
    where state = 'PUBLISHED';

create function profile_posts_guard() returns trigger language plpgsql as $$
begin
    if new.post_id is distinct from old.post_id
       or new.author_profile_id is distinct from old.author_profile_id
       or new.created_at is distinct from old.created_at then
        raise exception 'profile post scope is immutable'
            using errcode = '23514', constraint = 'profile_posts_scope_immutable';
    end if;
    if old.state = 'DELETED' and new.state <> 'DELETED' then
        raise exception 'deleted profile post is terminal'
            using errcode = '23514', constraint = 'profile_posts_deleted_terminal';
    end if;
    if new.revision <> old.revision + 1 then
        raise exception 'profile post revision must advance once'
            using errcode = '23514', constraint = 'profile_posts_revision_step';
    end if;
    new.updated_at := clock_timestamp();
    return new;
end $$;

create trigger profile_posts_guard
    before update on profile_posts
    for each row execute function profile_posts_guard();

create table profile_post_attachments (
    post_id uuid not null references profile_posts(post_id) on delete cascade,
    media_id uuid not null references profile_media(media_id) on delete restrict,
    position smallint not null,
    created_at timestamptz not null default clock_timestamp(),
    primary key (post_id, media_id),
    constraint profile_post_attachments_position check (position between 0 and 3),
    constraint profile_post_attachments_slot unique (post_id, position)
);

create index profile_post_attachments_media on profile_post_attachments (media_id);

-- A profile post may only attach media created for PROFILE_POST. The purpose is
-- closed, so an INTRO_POST asset cannot be smuggled into a profile post by
-- relabelling it: the media row's purpose is immutable (0080).
create function profile_post_attachments_purpose() returns trigger language plpgsql as $$
declare purpose text;
declare owner uuid;
begin
    select m.purpose, m.owner_profile_id into purpose, owner
    from profile_media m where m.media_id = new.media_id;
    if purpose is null then
        raise exception 'media is unavailable'
            using errcode = '23514', constraint = 'profile_post_attachments_media_present';
    end if;
    if purpose <> 'PROFILE_POST' then
        raise exception 'media purpose is not PROFILE_POST'
            using errcode = '23514', constraint = 'profile_post_attachments_purpose';
    end if;
    -- The attachment must belong to the post's author.
    if not exists (
        select 1 from profile_posts p
        where p.post_id = new.post_id and p.author_profile_id = owner
    ) then
        raise exception 'media is not owned by the post author'
            using errcode = '23514', constraint = 'profile_post_attachments_owner';
    end if;
    return new;
end $$;

create trigger profile_post_attachments_purpose
    before insert on profile_post_attachments
    for each row execute function profile_post_attachments_purpose();

-- The same ownership and purpose rule for intro attachments.
create function intro_attachments_purpose() returns trigger language plpgsql as $$
declare purpose text;
declare owner uuid;
begin
    select m.purpose, m.owner_profile_id into purpose, owner
    from profile_media m where m.media_id = new.media_id;
    if purpose is null then
        raise exception 'media is unavailable'
            using errcode = '23514', constraint = 'intro_attachments_media_present';
    end if;
    if purpose <> 'INTRO_POST' then
        raise exception 'media purpose is not INTRO_POST'
            using errcode = '23514', constraint = 'intro_attachments_purpose';
    end if;
    if not exists (
        select 1 from intro_posts p
        where p.intro_id = new.intro_id and p.author_profile_id = owner
    ) then
        raise exception 'media is not owned by the intro author'
            using errcode = '23514', constraint = 'intro_attachments_owner';
    end if;
    return new;
end $$;

create trigger intro_attachments_purpose
    before insert on intro_attachments
    for each row execute function intro_attachments_purpose();
