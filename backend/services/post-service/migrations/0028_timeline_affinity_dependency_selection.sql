-- Read-only scheduling hints. No source retirement, pair activation, authority,
-- receipt, credit or attempt is created by dependency selection.
create index post_outbox_dependency_window_idx on post_event_outbox(created_at,event_id)
    where not published;

create function post_timeline_affinity_ready(requested_event uuid)
returns boolean language sql stable set search_path=pg_catalog,public,pg_temp as $$
    -- The worker can recover earlier versions of this same Post under its own
    -- later claim. Check the next actual step, not the requested event's pair.
    -- Creation/no-affinity steps may make durable progress before a later wait.
    select not exists (
        select 1 from public.post_event_outbox requested
        left join public.timeline_projection_cursors cursor on cursor.aggregate_id=requested.aggregate_id
        join public.post_event_outbox next_event
            on next_event.aggregate_id=requested.aggregate_id
            and next_event.aggregate_version=coalesce(cursor.last_version,0)+1
            and next_event.aggregate_version<=requested.aggregate_version
        join public.feed_affinity_source_contributions c
            on c.namespace='OUTBOX' and c.source_id=next_event.event_id
        join public.feed_affinity_pair_baselines b using(epoch_id,viewer_id,author_id)
        left join public.feed_affinity_pair_heads h using(epoch_id,viewer_id,author_id)
        left join public.feed_affinity_pair_provenance o using(epoch_id,viewer_id,author_id)
        where requested.event_id=requested_event
          and (o.origin_mode is distinct from 'ORDERED_FRESH'
               or h.epoch_id is null
               or (h.provisioning_id,h.feature_epoch,h.writer_generation,h.feature_version)
                    is distinct from (b.provisioning_id,b.feature_epoch,b.writer_generation,b.feature_version)
               or not (
                   c.source_sequence=h.resolved_sequence+1
                   or (c.source_sequence<=h.resolved_sequence and exists (
                       select 1 from public.feed_affinity_resolution_receipts r
                       where r.namespace=c.namespace and r.source_id=c.source_id
                         and (r.identity_sha256,r.epoch_id,r.viewer_id,r.author_id,r.actor_id,
                              r.source_sequence,r.post_id,r.occurred_at,r.positive_delta,r.negative_delta)
                           is not distinct from
                             (c.identity_sha256,c.epoch_id,c.viewer_id,c.author_id,c.actor_id,
                              c.source_sequence,c.post_id,c.occurred_at,c.positive_delta,c.negative_delta)
                   ))
               ))
    );
$$;
revoke all on function post_timeline_affinity_ready(uuid) from public;
