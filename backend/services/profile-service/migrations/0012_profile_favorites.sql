-- WP-02 T-104: profile favorites.
--
-- At most five favorites per (profile, category). The cap is enforced by the
-- database, not only by the command, because a command-side check alone would
-- let a second writer or a direct statement exceed it. The trigger locks the
-- profile row first so two concurrent inserts for the same category serialize
-- and the sixth insert is refused deterministically.
create table profile_favorites (
    profile_id uuid not null references profile_identities(profile_id) on delete cascade,
    category text not null,
    catalog_item_id uuid not null check (catalog_item_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    position smallint not null,
    created_at timestamptz not null default clock_timestamp(),
    primary key (profile_id, category, catalog_item_id),
    constraint profile_favorites_category
        check (category in ('music','anime','game')),
    constraint profile_favorites_position
        check (position between 0 and 4),
    -- Two favorites cannot share a display slot in one category.
    constraint profile_favorites_slot
        unique (profile_id, category, position)
);

create index profile_favorites_profile
    on profile_favorites (profile_id, category, position);

create function profile_favorites_capacity() returns trigger language plpgsql as $$
declare total integer;
begin
    -- Serialize writers for this profile so the count below cannot race.
    perform 1 from profile_identities where profile_id = new.profile_id for update;
    select count(*) into total
    from profile_favorites
    where profile_id = new.profile_id and category = new.category;
    if total >= 5 then
        raise exception 'profile favorites category is full'
            using errcode = '23514', constraint = 'profile_favorites_capacity';
    end if;
    return new;
end $$;

create trigger profile_favorites_capacity
    before insert on profile_favorites
    for each row execute function profile_favorites_capacity();
