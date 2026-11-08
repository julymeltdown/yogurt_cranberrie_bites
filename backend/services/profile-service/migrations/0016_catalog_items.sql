-- WP-03 T-801: the favorites catalog.
--
-- The destination had no music/anime/game catalog at all. This is deliberately
-- the simplest honest thing that satisfies the frozen contract: one table of
-- selectable items, keyed by the canonical UUID the contract calls
-- `catalog_item_id`, plus a seed/import path (`backend/scripts/mcr/catalog_import.py`).
-- There is no provider-integration system here and none is implied.
--
-- What the donor contributes is the *policy* shape, not its key space. The
-- donor's `taste_catalog_items` uses prefixed text ids (`tci_…`) and five
-- domains. The frozen contract uses canonical UUIDs and exactly three
-- categories, so this table follows the contract and keeps the donor's useful
-- idea: a category is a closed set, and an exclusion is a property of the
-- catalog row, decided when the row is written, never a query-time string test.
--
-- K-pop exclusion is a catalog policy, not a filter:
--
--   * `catalog_items_music_excludes_kpop` makes a K-pop row impossible to store
--     in the music category at all, so no search can ever return one;
--   * `catalog_items_music_requires_exclusion_review` makes every music row
--     carry an explicit review marker, so a writer cannot silently import a
--     K-pop act by omitting the flag.
--
-- The second rule is what makes "absent by policy" true rather than "absent
-- because the seed happened not to contain any".

create table catalog_items (
    -- Canonical lowercase UUID, per the frozen contract. No prefixed ids.
    catalog_item_id uuid primary key default gen_random_uuid()
        check (catalog_item_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    -- The contract's category enum, lowercased to match `profile_favorites`.
    category text not null,
    -- The label a card shows. This is what "You both love MCR and NANA" prints.
    label text not null,
    -- Optional external reference. Never required; never a provider identity.
    external_url text,
    -- Where the row came from, for provenance. `manual` is the seed path.
    source text not null default 'manual',
    -- The K-pop exclusion marker. Only music rows may carry it, music rows must,
    -- and a music row that carries it cannot be stored.
    excluded boolean not null default false,
    exclusion_reason text,
    -- Retired rows stay for history but never appear in search.
    status text not null default 'ACTIVE',
    created_at timestamptz not null default clock_timestamp(),

    constraint catalog_items_category
        check (category in ('music','anime','game')),
    constraint catalog_items_label_length
        check (char_length(btrim(label)) between 1 and 80),
    constraint catalog_items_label_no_control
        check (label !~ '[\u0000-\u001f\u007f]'),
    constraint catalog_items_external_url
        check (external_url is null
               or (char_length(external_url) between 8 and 2048
                   and external_url ~* '^https://[^[:space:]]+$')),
    constraint catalog_items_source_shape
        check (source ~ '^[a-z][a-z0-9._-]{0,63}$'),
    constraint catalog_items_status
        check (status in ('ACTIVE','RETIRED')),
    constraint catalog_items_exclusion_reason
        check (exclusion_reason is null
               or (char_length(exclusion_reason) between 1 and 120
                   and exclusion_reason !~ '[\u0000-\u001f\u007f]')),

    -- The exclusion marker is meaningful only for music, and only music may
    -- carry it. A non-music row claiming an exclusion is a writer bug.
    constraint catalog_items_exclusion_category
        check (excluded = false or category = 'music'),
    constraint catalog_items_excluded_has_reason
        check (excluded = false or exclusion_reason is not null),

    -- Catalog policy, not a query filter: a K-pop music row cannot exist.
    constraint catalog_items_music_excludes_kpop
        check (not (category = 'music' and excluded)),
    -- Every music row carries the review decision explicitly.
    constraint catalog_items_music_requires_exclusion_review
        check (category <> 'music' or exclusion_reason is not null),

    -- One catalog row per (category, label). Case-insensitive, because two rows
    -- differing only in case would be the same favorite to a reader. A table
    -- constraint cannot use an expression, so this is a unique index; the name
    -- is what the negative test asserts.
    constraint catalog_items_label_not_blank
        check (btrim(label) <> '')
);

create unique index catalog_items_label_unique
    on catalog_items (category, lower(label));

-- Search ordering: category, then a stable normalized label, then the id. The
-- id tiebreak is what makes "the same query returns the same order twice" true
-- even for two rows whose labels differ only by case.
create index catalog_items_search
    on catalog_items (category, lower(label), catalog_item_id)
    where status = 'ACTIVE';

-- The seed path is idempotent: importing the same manifest twice is a no-op.
create index catalog_items_source_label
    on catalog_items (source, lower(label));
