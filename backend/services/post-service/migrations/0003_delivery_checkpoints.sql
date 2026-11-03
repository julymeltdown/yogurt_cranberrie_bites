-- Keep one durable checkpoint per event/effect before enforcing idempotency.
delete from event_publications as duplicate
using event_publications as retained
where duplicate.event_id = retained.event_id
  and duplicate.target = retained.target
  and duplicate.id > retained.id;

create unique index if not exists event_publications_event_target_idx
    on event_publications (event_id, target);

create index if not exists post_outbox_claim_idx
    on post_event_outbox (created_at, event_id)
    where published = false and dead_lettered_at is null;
