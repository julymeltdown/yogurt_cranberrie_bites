-- Consumer inbox for the feed projection.
--
-- Delivery is at-least-once. The same event therefore arrives more than once:
-- a crash between the broker acknowledgement and the delivery checkpoint causes
-- a retry, and an operator replay after a fix sends stored events on purpose.
--
-- The projection is a delta, not an assignment: a like adds one to a counter and
-- a comment raises an affinity score. Applying a delta twice inflates the
-- result, so replay previously double-counted engagement and skewed ranking.
--
-- Recording applied events makes the projection idempotent, and it is recorded
-- in the same transaction as the projection, so a projection that fails rolls
-- back its inbox row too. An inbox row that survived a failed projection would
-- mark an event applied that never was, which loses it instead of duplicating
-- it.
create table if not exists feed_projection_inbox (
    consumer text not null,
    event_id uuid not null,
    applied_at timestamptz not null,
    primary key (consumer, event_id)
);

-- Supports the retention sweep and lets an operator see how far a consumer has
-- progressed without scanning the whole inbox.
create index if not exists feed_projection_inbox_applied_idx
    on feed_projection_inbox (applied_at);
