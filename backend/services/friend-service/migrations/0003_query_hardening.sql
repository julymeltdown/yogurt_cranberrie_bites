create index if not exists follows_active_followee_cursor_idx
    on follows (followee_id, created_at desc, follower_id desc)
    where deleted_at is null;

create index if not exists follows_active_follower_cursor_idx
    on follows (follower_id, created_at desc, followee_id desc)
    where deleted_at is null;

create index if not exists follows_retention_due_idx
    on follows (retention_until, follower_id, followee_id)
    where deleted_at is not null and retention_until is not null;
