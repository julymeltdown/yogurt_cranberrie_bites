-- WP-03 T-802: the discovery mirror of the profile-owned favorites.
--
-- Discovery lives in the concert service, favorites live in the profile
-- service, and the two services do not share a database. Discovery therefore
-- reads a local mirror of exactly the two things it needs:
--
--   * `discovery_favorite_items` — the catalog items a profile selected, as
--     `(profile_id, category, catalog_item_id)`. The catalog id is the only
--     thing overlap is ever computed from, and there is no label, weight,
--     intensity or score column to compute anything else from.
--   * `discovery_profiles` — the profile facts a discovery card shows. There is
--     deliberately no `intro_id` column: a profile is discoverable whether or
--     not it ever published an intro post, and a card cannot depend on one.
--
-- The donor's overlap query starts from `intros` and joins the viewer's taste
-- features onto them. That is the bug this work package must not repeat: a
-- profile that never wrote an intro is invisible in the donor. Here the query
-- starts from `discovery_profiles`, and an intro post is not part of the
-- predicate at all.
--
-- There is no score column, no compatibility column, no closeness column and no
-- rank column anywhere in this schema. The absence is the point, and the
-- discovery test asserts that the response carries no numeric score field.

create table discovery_profiles (
    profile_id uuid primary key check (profile_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    -- The Auth account that owns the profile. Blocks are recorded against
    -- accounts, so discovery filters on it.
    account_id uuid not null check (account_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    nickname text not null,
    -- The public introduction. This is the profile's own bio, not an intro post.
    bio text,
    profile_image_media_id uuid,
    -- Optional one-line interaction style. Null means the user never set one,
    -- and the card then omits the line rather than inventing a default.
    interaction_style text,
    visibility text not null default 'PUBLIC',
    updated_at timestamptz not null default clock_timestamp(),

    constraint discovery_profiles_nickname
        check (char_length(nickname) between 1 and 40),
    constraint discovery_profiles_bio
        check (bio is null or char_length(bio) <= 160),
    constraint discovery_profiles_interaction_style
        check (interaction_style is null
               or (char_length(interaction_style) <= 120
                   and position(chr(10) in interaction_style) = 0
                   and position(chr(13) in interaction_style) = 0)),
    constraint discovery_profiles_visibility
        check (visibility in ('PUBLIC','PRIVATE'))
);

create index discovery_profiles_visible
    on discovery_profiles (profile_id) where visibility = 'PUBLIC';

create table discovery_favorite_items (
    profile_id uuid not null
        references discovery_profiles(profile_id) on delete cascade,
    category text not null,
    catalog_item_id uuid not null
        check (catalog_item_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    -- The label is a display convenience for the card. Overlap is never computed
    -- from it; the catalog id is the identity.
    label text not null,
    position smallint not null,

    primary key (profile_id, category, catalog_item_id),
    constraint discovery_favorite_items_category
        check (category in ('music','anime','game')),
    constraint discovery_favorite_items_position
        check (position between 0 and 4),
    constraint discovery_favorite_items_label
        check (char_length(label) between 1 and 80),
    -- Two favorites cannot share a display slot in one category.
    constraint discovery_favorite_items_slot
        unique (profile_id, category, position)
);

-- The overlap join reads this direction: given the viewer's catalog ids, find
-- the other profiles that selected them.
create index discovery_favorite_items_catalog
    on discovery_favorite_items (category, catalog_item_id, profile_id);

-- Discovery never ranks by favorite count, so the only ordering it needs is a
-- stable one for pagination.
create index discovery_favorite_items_profile
    on discovery_favorite_items (profile_id, category, position);

-- Blocks are recorded against accounts by the friend service. The concert
-- service keeps a local mirror of the two columns discovery needs, so the
-- visibility predicate is one local join and never a cross-service call inside
-- a page query.
create table discovery_blocks (
    blocker_account_id uuid not null,
    blocked_account_id uuid not null,
    created_at timestamptz not null default clock_timestamp(),
    deleted_at timestamptz,
    primary key (blocker_account_id, blocked_account_id),
    constraint discovery_blocks_not_self
        check (blocker_account_id <> blocked_account_id)
);

create index discovery_blocks_active
    on discovery_blocks (blocked_account_id, blocker_account_id)
    where deleted_at is null;

-- A profile's mirrored facts and favorites are written together by the
-- projection, so a discovery page never sees a half-updated profile.
create function discovery_favorite_items_capacity() returns trigger language plpgsql as $$
declare total integer;
begin
    perform 1 from discovery_profiles where profile_id = new.profile_id for update;
    select count(*) into total
    from discovery_favorite_items
    where profile_id = new.profile_id and category = new.category;
    if total >= 5 then
        raise exception 'discovery favorites category is full'
            using errcode = '23514', constraint = 'discovery_favorite_items_capacity';
    end if;
    return new;
end $$;

create trigger discovery_favorite_items_capacity
    before insert on discovery_favorite_items
    for each row execute function discovery_favorite_items_capacity();
