create index if not exists posts_author_retention_idx
    on posts (author_id, retention_until, id)
    where deleted_at is not null and retention_until is not null;
create index if not exists comments_author_retention_idx
    on comments (author_id, retention_until, id)
    where deleted_at is not null and retention_until is not null;
create index if not exists post_likes_user_retention_idx
    on post_likes (user_id, retention_until, post_id)
    where deleted_at is not null and retention_until is not null;
