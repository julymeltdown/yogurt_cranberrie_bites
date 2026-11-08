-- WP-02 T-108: intro posts with audience columns.
--
-- The foundation's introduction entity (`concert_intros`, 0049) is event-scoped,
-- has no gender condition and is erased by event cleanup. The profile product
-- needs an introduction that belongs to a profile, carries a paired age window
-- and a gender condition, and outlives every event.
--
-- There is deliberately no `event_id`, no `room_id` and no participant key.
create table intro_posts (
    intro_id uuid primary key check (intro_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    author_profile_id uuid not null check (author_profile_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    body text not null,
    favorites_snapshot jsonb not null default '[]'::jsonb,
    currently_seeking text,
    interaction_style text,
    audience_gender text not null default 'any',
    audience_min_age smallint,
    audience_max_age_exclusive smallint,
    state text not null,
    revision bigint not null default 1,
    published_at timestamptz,
    expires_at timestamptz,
    created_at timestamptz not null default clock_timestamp(),
    updated_at timestamptz not null default clock_timestamp(),

    constraint intro_posts_body_length check (char_length(body) <= 1000),
    -- A NUL cannot be stored in a text value at all, so the body's NUL rule is
    -- the storage layer's own. The body is multi-line, so no newline check.
    constraint intro_posts_currently_seeking
        check (currently_seeking is null
               or (char_length(currently_seeking) <= 120
                   and position(chr(10) in currently_seeking) = 0
                   and position(chr(13) in currently_seeking) = 0)),
    constraint intro_posts_interaction_style
        check (interaction_style is null
               or (char_length(interaction_style) <= 120
                   and position(chr(10) in interaction_style) = 0
                   and position(chr(13) in interaction_style) = 0)),
    constraint intro_posts_favorites_snapshot
        check (jsonb_typeof(favorites_snapshot) = 'array'
               and jsonb_array_length(favorites_snapshot) <= 15),

    -- Closed gender condition. `unknown` is the donor's explicit "not
    -- specified" bucket and is never a wildcard.
    constraint intro_posts_audience_gender
        check (audience_gender in ('any','female','male','unknown')),

    -- Both age bounds are present or both are absent. A half-open window is
    -- meaningless and would make the audience decision ambiguous.
    constraint intro_posts_audience_bounds_paired
        check ((audience_min_age is null) = (audience_max_age_exclusive is null)),
    constraint intro_posts_audience_bounds
        check (audience_min_age is null
               or (audience_min_age between 14 and 100
                   and audience_max_age_exclusive between 15 and 101
                   and audience_min_age < audience_max_age_exclusive)),

    constraint intro_posts_state
        check (state in ('DRAFT','PUBLISHED','DELETED')),
    -- A published post must have a body and a publication instant; a draft must
    -- not pretend to have been published.
    constraint intro_posts_published_shape
        check ((state = 'PUBLISHED' and published_at is not null and btrim(body) <> '')
            or (state <> 'PUBLISHED' and published_at is null)),
    constraint intro_posts_expiry
        check (expires_at is null or published_at is null or expires_at > published_at),
    constraint intro_posts_revision
        check (revision between 1 and 9007199254740991),
    constraint intro_posts_times
        check (updated_at >= created_at)
);

-- Discovery page order. Only published posts are discoverable, so the index is
-- partial; a draft or deleted post never appears in a page scan.
create index intro_posts_discovery
    on intro_posts (published_at desc, intro_id desc) where state = 'PUBLISHED';

create index intro_posts_author
    on intro_posts (author_profile_id, created_at desc, intro_id desc);

create function intro_posts_revision_guard() returns trigger language plpgsql as $$
begin
    if new.intro_id is distinct from old.intro_id
       or new.author_profile_id is distinct from old.author_profile_id
       or new.created_at is distinct from old.created_at then
        raise exception 'intro post scope is immutable'
            using errcode = '23514', constraint = 'intro_posts_scope_immutable';
    end if;
    -- DELETED is terminal. A deleted post is never revived.
    if old.state = 'DELETED' and new.state <> 'DELETED' then
        raise exception 'deleted intro post is terminal'
            using errcode = '23514', constraint = 'intro_posts_deleted_terminal';
    end if;
    if new.revision <> old.revision + 1 then
        raise exception 'intro post revision must advance once'
            using errcode = '23514', constraint = 'intro_posts_revision_step';
    end if;
    new.updated_at := clock_timestamp();
    return new;
end $$;

create trigger intro_posts_revision_guard
    before update on intro_posts
    for each row execute function intro_posts_revision_guard();
