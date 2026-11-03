-- Back search with a trigram index and escape LIKE wildcards.
--
-- Search ran as 'lower(column) like ''%term%''' with no supporting index, so
-- every search was a sequential scan. A trigram GIN index serves contains
-- patterns, which is the shape these queries use.
--
-- The index is partial to match the queries, which always exclude soft-deleted
-- rows, so it stays smaller than an index over the whole table.
create extension if not exists pg_trgm;

create index if not exists posts_content_trgm_idx
    on posts using gin (lower(content) gin_trgm_ops)
    where deleted_at is null and moderation_hidden_at is null;

