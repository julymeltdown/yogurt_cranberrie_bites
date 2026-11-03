-- Back profile search with a trigram index.
--
-- Search matches against username, nickname, and bio with contains patterns and
-- ran as a sequential scan without a supporting index. Trigram GIN indexes serve
-- that pattern shape.
--
-- Three single-column indexes are used rather than one composite: the query ORs
-- the columns, so the planner can combine them, and a composite index would not
-- be usable for a leading-column-agnostic OR.
create extension if not exists pg_trgm;

create index if not exists profiles_username_trgm_idx
    on profiles using gin (lower(username) gin_trgm_ops)
    where deleted_at is null;

create index if not exists profiles_nickname_trgm_idx
    on profiles using gin (lower(nickname) gin_trgm_ops)
    where deleted_at is null;

-- bio is searched too, so it needs its own index. Leaving it out would force a
-- sequential scan for that branch of the OR and lose the benefit of the others.
create index if not exists profiles_bio_trgm_idx
    on profiles using gin (lower(bio) gin_trgm_ops)
    where deleted_at is null;
