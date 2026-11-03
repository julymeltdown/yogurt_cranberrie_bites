-- Drain old unfenced workers/replay writers before activating this protocol.
-- Automatic attempt budgets and immutable checkpoints are not replay budgets.
alter table post_event_delivery_jobs
    add column lease_kind text not null default 'AUTO'
        check (lease_kind in ('AUTO','REPLAY')),
    add column replay_restore_state text null
        check (replay_restore_state in ('PENDING','RETRY','SUCCEEDED','SKIPPED'));
alter table post_event_delivery_jobs add constraint post_replay_lease_identity check (
    (lease_kind='AUTO' and replay_restore_state is null)
    or (lease_kind='REPLAY' and state='LEASED' and replay_restore_state is not null)
);
