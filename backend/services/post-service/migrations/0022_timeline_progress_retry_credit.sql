-- Lifetime attempt identities remain monotonic. Only a durable, owner-validated
-- TIMELINE prefix committed by the SAME expired attempt can refund that attempt's
-- failure-budget charge. Frozen max/backoff and all other targets remain intact.
alter table post_event_delivery_jobs add column progress_credits integer not null default 0;
alter table post_event_delivery_jobs add column progress_checked_attempt_id uuid null;
alter table post_event_delivery_jobs add constraint post_delivery_progress_credits
    check (progress_credits between 0 and attempt_count and (target='TIMELINE' or progress_credits=0));
alter table post_event_delivery_attempts drop constraint post_event_delivery_attempts_phase_check;
alter table post_event_delivery_attempts add constraint post_event_delivery_attempts_phase_check
    check (phase in ('STARTED','SUCCEEDED','FAILED','PROGRESSED'));
create index timeline_projection_attempt_progress_idx
    on timeline_projection_receipts(delivery_event_id,delivery_attempt_id,aggregate_version desc);

create function post_timeline_pending_progress_credit(delivery_event uuid, delivery_attempt uuid)
returns integer language sql stable strict as $$
    select case when exists (
        select 1 from timeline_projection_receipts receipt
        join post_event_outbox owner on owner.event_id=delivery_event
        where receipt.delivery_event_id=delivery_event
          and receipt.delivery_attempt_id=delivery_attempt
          and receipt.aggregate_id=owner.aggregate_id
          and receipt.aggregate_version<=owner.aggregate_version
    ) and exists (
        select 1 from post_event_delivery_attempts
        where event_id=delivery_event and attempt_id=delivery_attempt
          and target='TIMELINE' and phase='STARTED'
    ) and not exists (
        select 1 from post_event_delivery_attempts
        where attempt_id=delivery_attempt and phase in ('PROGRESSED','SUCCEEDED','FAILED')
    ) then 1 else 0 end;
$$;

create index post_delivery_timeline_progress_expiry_idx
    on post_event_delivery_jobs(lease_expires_at,event_id)
    where target='TIMELINE' and state='LEASED'
      and progress_checked_attempt_id is distinct from attempt_id;
