-- Extend the canonical Auth saga, never create another identity/deletion store.
-- Run with old Auth writers/workers stopped: older binaries cannot parse these
-- targets and must not create four-step sagas after this migration.
lock table user_deletion_sagas, user_deletion_steps in share row exclusive mode;

alter table user_deletion_steps drop constraint user_deletion_steps_target_check;
alter table user_deletion_steps add constraint user_deletion_steps_target_check
    check (target in ('PROFILE', 'FRIEND', 'POST', 'NOTIFICATION',
        'CONCERT_TICKETS', 'CONCERT_SESSIONS', 'CONCERT_SHARING', 'CONCERT_PRIVATE_DATA'));

-- Existing completed deletions have not performed Concert cleanup. Reopen only
-- the sagas actually extended; preserve operator-required DEAD_LETTERED state,
-- successful legacy steps, retry generations and original retention deadlines.
with extended as (
    insert into user_deletion_steps (saga_id, target, status, next_attempt_at)
    select g.id, t.target, 'PENDING', now()
    from user_deletion_sagas g
    cross join (values ('CONCERT_TICKETS'), ('CONCERT_SESSIONS'),
        ('CONCERT_SHARING'), ('CONCERT_PRIVATE_DATA')) t(target)
    on conflict (saga_id, target) do nothing
    returning saga_id
)
update user_deletion_sagas g
set status = case when g.status = 'COMPLETED' then 'IN_PROGRESS' else g.status end,
    completed_at = case when g.status = 'COMPLETED' then null else g.completed_at end,
    updated_at = now()
where g.id in (select saga_id from extended);
