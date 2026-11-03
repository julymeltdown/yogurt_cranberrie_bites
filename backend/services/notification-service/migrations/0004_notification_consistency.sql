-- Stable per-subject serialization and explicit recipient/actor suppression.
--
-- Acquire the legacy deletion tables in its existing write order before taking
-- any backfill snapshot. Read-only suppression checks remain available, while
-- suppression and notification writes wait for this migration to commit.
lock table notification_recipient_suppressions in share row exclusive mode;
lock table notifications in share row exclusive mode;

-- A suppression row may not exist yet, so it cannot itself be the lock target.
-- This service-owned state row is permanent, contains no user content, and is
-- locked by both notification creation and account deletion. Creation locks the
-- recipient and actor rows in UUID order to avoid reversed-identity deadlocks.
create table if not exists notification_subject_states (
    subject_id uuid primary key,
    suppress_as_recipient boolean not null default false,
    suppress_as_actor boolean not null default false,
    reason text null,
    suppressed_at timestamptz null,
    retention_until timestamptz null,
    created_at timestamptz not null default now()
);

-- Existing suppressions were created only by account deletion. Preserve their
-- recipient behavior and make the actor rule explicit for late event delivery.
insert into notification_subject_states (
    subject_id, suppress_as_recipient, suppress_as_actor,
    reason, suppressed_at, retention_until
)
select user_id, true, true, reason, suppressed_at, retention_until
from notification_recipient_suppressions
on conflict (subject_id) do update
set suppress_as_recipient = true,
    suppress_as_actor = true,
    reason = excluded.reason,
    suppressed_at = least(
        coalesce(notification_subject_states.suppressed_at, excluded.suppressed_at),
        excluded.suppressed_at
    ),
    retention_until = excluded.retention_until;

-- Repair any live row that a pre-migration legacy check-then-save race inserted
-- after its subject had already been suppressed. There is no trustworthy legacy
-- coordinator identity to backfill into deleted_by, so this migration does not
-- invent one. If both recipient and actor are suppressed, retain through the
-- later non-null deadline.
with suppressed_notifications as (
    select
        notification.id,
        min(suppression.suppressed_at) as suppressed_at,
        max(suppression.retention_until) as retention_until
    from notifications as notification
    join notification_recipient_suppressions as suppression
      on suppression.user_id = notification.user_id
      or suppression.user_id = notification.actor_id
    where notification.deleted_at is null
    group by notification.id
)
update notifications as notification
set deleted_at = suppressed.suppressed_at,
    delete_reason = coalesce(notification.delete_reason, 'account_deletion'),
    retention_until = case
        when notification.retention_until is null then suppressed.retention_until
        when suppressed.retention_until is null then notification.retention_until
        else greatest(notification.retention_until, suppressed.retention_until)
    end
from suppressed_notifications as suppressed
where notification.id = suppressed.id;

-- During a rolling deployment, an older binary can still write only the legacy
-- suppression table. Mirror those writes so a new binary never accepts a late
-- recipient or actor event after deletion. The trigger is service-local and
-- idempotent; new binaries update both representations in one transaction.
create or replace function sync_notification_subject_state_from_legacy()
returns trigger language plpgsql as $$
begin
    insert into notification_subject_states (
        subject_id, suppress_as_recipient, suppress_as_actor,
        reason, suppressed_at, retention_until
    ) values (
        new.user_id, true, true, new.reason, new.suppressed_at, new.retention_until
    )
    on conflict (subject_id) do update
    set suppress_as_recipient = true,
        suppress_as_actor = true,
        reason = excluded.reason,
        suppressed_at = least(
            coalesce(notification_subject_states.suppressed_at, excluded.suppressed_at),
            excluded.suppressed_at
        ),
        retention_until = excluded.retention_until;
    return new;
end $$;

drop trigger if exists notification_legacy_suppression_state_sync
    on notification_recipient_suppressions;
create trigger notification_legacy_suppression_state_sync
after insert or update on notification_recipient_suppressions
for each row execute function sync_notification_subject_state_from_legacy();

-- Legacy binaries perform a suppression check and the notification insert in
-- separate statements. Mirroring the later suppression write is insufficient:
-- an old writer can pass its check while deletion is in flight. Enforce the
-- recipient/actor invariant at the table boundary so every binary participates
-- in the same ordered subject locks before a notification row can be inserted.
create or replace function enforce_notification_subject_state()
returns trigger language plpgsql as $$
declare
    locked_subject_id uuid;
begin
    for locked_subject_id in
        select candidate.subject_id
        from (values (new.user_id), (new.actor_id)) as candidate(subject_id)
        where candidate.subject_id is not null
        group by candidate.subject_id
        order by candidate.subject_id
    loop
        insert into notification_subject_states (subject_id, created_at)
        values (locked_subject_id, now())
        on conflict (subject_id) do nothing;

        perform subject_id
        from notification_subject_states
        where subject_id = locked_subject_id
        for update;
    end loop;

    if exists (
        select 1
        from notification_subject_states
        where subject_id = new.user_id
          and suppress_as_recipient
    ) then
        raise exception 'notification recipient % is suppressed', new.user_id
            using errcode = '23514',
                  constraint = 'notifications_recipient_not_suppressed';
    end if;

    if new.actor_id is not null and exists (
        select 1
        from notification_subject_states
        where subject_id = new.actor_id
          and suppress_as_actor
    ) then
        raise exception 'notification actor % is suppressed', new.actor_id
            using errcode = '23514',
                  constraint = 'notifications_actor_not_suppressed';
    end if;

    return new;
end $$;

drop trigger if exists notification_subject_state_guard on notifications;
create trigger notification_subject_state_guard
before insert or update of user_id, actor_id on notifications
for each row execute function enforce_notification_subject_state();

-- Supports deterministic bounded retention claims with FOR UPDATE SKIP LOCKED.
create index if not exists notifications_retention_claim_idx
    on notifications (retention_until asc, id asc)
    where deleted_at is not null and retention_until is not null;

-- Immutable saga identity prevents a retried coordinator command from reusing
-- one saga id for a different subject, actor, or retention deadline. The
-- operation remains idempotent while conflicting reuse fails in one transaction.
create table if not exists notification_user_deletion_receipts (
    saga_id uuid primary key,
    user_id uuid not null,
    actor_id uuid not null,
    retention_until timestamptz not null,
    applied_at timestamptz not null default now()
);

create index if not exists notification_user_deletion_receipts_user_idx
    on notification_user_deletion_receipts (user_id, applied_at desc);
