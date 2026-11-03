-- Scheduling metadata is not outcome evidence. Missing or failed receipt
-- lookups remain UNKNOWN and never authorize redispatch or rejection.
alter table gateway_request_journal
    add column owner_receipt_attempt_count integer not null default 0
        check (owner_receipt_attempt_count >= 0),
    add column owner_receipt_next_check_at timestamptz,
    add column owner_receipt_claim_id uuid,
    add column owner_receipt_suspended_reason text
        check (owner_receipt_suspended_reason in ('NOT_APPLICABLE', 'INVALID_ACTOR')),
    add constraint gateway_owner_receipt_suspension_schedule_check check (
        owner_receipt_suspended_reason is null
        or (owner_receipt_claim_id is null and owner_receipt_next_check_at is null)
    );

-- Preserve an existing worker's 30-second lease across an upgrade. Unchecked
-- rows and operations becoming UNKNOWN after this migration start immediately.
update gateway_request_journal
set owner_receipt_next_check_at = owner_receipt_checked_at + interval '30 seconds'
where state = 'UNKNOWN' and outcome = 'UNKNOWN'
  and outcome_evidence <> 'MANUAL_RESOLUTION'
  and owner_receipt_checked_at is not null;

create index gateway_request_journal_owner_receipt_due_idx
    on gateway_request_journal (coalesce(owner_receipt_next_check_at, created_at), id)
    where state = 'UNKNOWN' and outcome = 'UNKNOWN'
      and outcome_evidence <> 'MANUAL_RESOLUTION'
      and owner_receipt_suspended_reason is null;

-- A repaired lookup tuple must not inherit a previous tuple's suspension or
-- claim. Clearing the token also fences any old owner answer still in flight.
create function gateway_reset_owner_receipt_schedule() returns trigger
language plpgsql as $$
begin
    if new.actor_id is distinct from old.actor_id
       or new.action is distinct from old.action
       or new.resource_path is distinct from old.resource_path then
        new.owner_receipt_attempt_count := 0;
        new.owner_receipt_checked_at := null;
        new.owner_receipt_next_check_at := null;
        new.owner_receipt_claim_id := null;
        new.owner_receipt_suspended_reason := null;
    end if;
    if new.state <> 'UNKNOWN' or new.outcome <> 'UNKNOWN'
       or new.outcome_evidence = 'MANUAL_RESOLUTION' then
        new.owner_receipt_next_check_at := null;
        new.owner_receipt_claim_id := null;
        new.owner_receipt_suspended_reason := null;
    end if;
    return new;
end;
$$;

create trigger gateway_reset_owner_receipt_schedule
before update on gateway_request_journal
for each row execute function gateway_reset_owner_receipt_schedule();
