-- WP-02 T-109: intro attachments, likes and comments.
--
-- Three children of an intro post. None of them carries a reward, a score or a
-- grant: a like is a row, and nothing in this migration reacts to it. There is
-- deliberately NO trigger on intro_likes beyond the identity guard below, and no
-- trigger at all that awards, notifies or mutates another table in response to a
-- like. The notification for a like is written by the command, not by the
-- database, so it carries the command's authority and its idempotency key.

-- Attachments are associations only: deleting an intro post removes the
-- association, never the media file itself.
create table intro_attachments (
    intro_id uuid not null references intro_posts(intro_id) on delete cascade,
    media_id uuid not null references profile_media(media_id) on delete restrict,
    position smallint not null,
    created_at timestamptz not null default clock_timestamp(),
    primary key (intro_id, media_id),
    constraint intro_attachments_position check (position between 0 and 3),
    constraint intro_attachments_slot unique (intro_id, position)
);

create index intro_attachments_media on intro_attachments (media_id);

-- A like is `(intro, actor)`. The owner cannot like their own post; that rule
-- needs the author, which lives in intro_posts, so it is enforced here in the
-- database as well as in the command rather than left to the command alone.
create table intro_likes (
    intro_id uuid not null references intro_posts(intro_id) on delete cascade,
    actor_profile_id uuid not null check (actor_profile_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    created_at timestamptz not null default clock_timestamp(),
    primary key (intro_id, actor_profile_id)
);

-- Page order for the liker list.
create index intro_likes_page on intro_likes (intro_id, created_at desc, actor_profile_id desc);

create function intro_likes_no_self_like() returns trigger language plpgsql as $$
declare author uuid;
begin
    select author_profile_id into author from intro_posts where intro_id = new.intro_id;
    if author is null then
        raise exception 'intro post is unavailable'
            using errcode = '23514', constraint = 'intro_likes_intro_present';
    end if;
    if author = new.actor_profile_id then
        raise exception 'an author cannot like their own intro post'
            using errcode = '23514', constraint = 'intro_likes_no_self_like';
    end if;
    return new;
end $$;

create trigger intro_likes_no_self_like
    before insert on intro_likes
    for each row execute function intro_likes_no_self_like();

-- Comments are one level deep. A reply names its parent, the parent must belong
-- to the same intro, and a reply to a reply is refused so the thread cannot
-- deepen without bound.
create table intro_comments (
    comment_id uuid primary key check (comment_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    intro_id uuid not null references intro_posts(intro_id) on delete cascade,
    author_profile_id uuid not null check (author_profile_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    parent_comment_id uuid,
    body text not null,
    state text not null default 'PUBLISHED',
    created_at timestamptz not null default clock_timestamp(),
    constraint intro_comments_body_length check (char_length(body) between 1 and 500),
    constraint intro_comments_state check (state in ('PUBLISHED','DELETED')),
    constraint intro_comments_parent
        foreign key (parent_comment_id) references intro_comments(comment_id) on delete restrict,
    -- A comment cannot be its own parent.
    constraint intro_comments_parent_not_self check (parent_comment_id is null or parent_comment_id <> comment_id)
);

create index intro_comments_page on intro_comments (intro_id, created_at, comment_id);
create index intro_comments_parent on intro_comments (parent_comment_id) where parent_comment_id is not null;

create function intro_comments_parent_guard() returns trigger language plpgsql as $$
declare parent record;
begin
    if new.parent_comment_id is null then
        return new;
    end if;
    select intro_id, parent_comment_id into parent
    from intro_comments where comment_id = new.parent_comment_id;
    if parent is null then
        raise exception 'parent comment is unavailable'
            using errcode = '23514', constraint = 'intro_comments_parent_present';
    end if;
    if parent.intro_id <> new.intro_id then
        raise exception 'parent comment belongs to a different intro post'
            using errcode = '23514', constraint = 'intro_comments_parent_intro';
    end if;
    if parent.parent_comment_id is not null then
        raise exception 'replies to replies are not supported'
            using errcode = '23514', constraint = 'intro_comments_one_level';
    end if;
    return new;
end $$;

create trigger intro_comments_parent_guard
    before insert or update of parent_comment_id, intro_id on intro_comments
    for each row execute function intro_comments_parent_guard();

create function intro_comments_terminal() returns trigger language plpgsql as $$
begin
    if old.state = 'DELETED' and new.state <> 'DELETED' then
        raise exception 'deleted comment is terminal'
            using errcode = '23514', constraint = 'intro_comments_deleted_terminal';
    end if;
    return new;
end $$;

create trigger intro_comments_terminal
    before update on intro_comments
    for each row execute function intro_comments_terminal();
