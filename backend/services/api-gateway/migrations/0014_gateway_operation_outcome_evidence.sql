alter table gateway_request_journal
    add column outcome text not null default 'PENDING',
    add column outcome_evidence text not null default 'INTENT_RECORDED';

update gateway_request_journal
set outcome = case
        when state in ('IN_PROGRESS', 'QUEUED') then 'PENDING'
        when state = 'COMPLETED'
            and response_status >= 200
            and response_status < 300
            and response_status <> 202 then 'COMMITTED'
        else 'UNKNOWN'
    end,
    outcome_evidence = case
        when state = 'IN_PROGRESS' then 'INTENT_RECORDED'
        when state = 'QUEUED' then 'QUEUE_ACCEPTED'
        when state = 'COMPLETED'
            and response_status >= 200
            and response_status < 300
            and response_status <> 202 then 'SUCCESS_RESPONSE'
        when state = 'COMPLETED' then 'RESPONSE_STATUS_ONLY'
        else 'LEGACY_UNRESOLVED'
    end;

-- Legacy COMPLETED recorded that the Gateway stored a response, not that a
-- non-success command was proven not to commit. Put those rows on the same
-- operator-reconciliation path as newly observed ambiguous outcomes.
update gateway_request_journal
set state = 'UNKNOWN'
where state = 'COMPLETED'
  and (
      response_status is null
      or response_status < 200
      or response_status >= 300
      or response_status = 202
  );

-- Preserve the latest audited human decision already recorded by earlier
-- Gateway versions. The journal projection is rebuilt from immutable history;
-- an old UNKNOWN row must not silently discard an existing resolution.
with latest_resolution as (
    select distinct on (operation_id)
        operation_id,
        disposition
    from gateway_operation_manual_reconciliations
    order by operation_id, revision desc
)
update gateway_request_journal journal
set outcome = case latest_resolution.disposition
        when 'COMMITTED' then 'COMMITTED'
        when 'NOT_COMMITTED' then 'REJECTED_BEFORE_COMMIT'
        else 'UNKNOWN'
    end,
    outcome_evidence = 'MANUAL_RESOLUTION'
from latest_resolution
where journal.id = latest_resolution.operation_id
  and journal.state = 'UNKNOWN';

alter table gateway_request_journal
    add constraint gateway_request_journal_outcome_check
    check (outcome in ('PENDING', 'COMMITTED', 'REJECTED_BEFORE_COMMIT', 'UNKNOWN')),
    add constraint gateway_request_journal_outcome_evidence_check
    check (outcome_evidence in (
        'INTENT_RECORDED', 'QUEUE_ACCEPTED', 'SUCCESS_RESPONSE',
        'PRE_DISPATCH_REJECTION', 'RESPONSE_STATUS_ONLY',
        'OWNER_REPLAY_RECEIPT', 'MANUAL_RESOLUTION',
        'STALE_OPERATION', 'LEGACY_UNRESOLVED'
    )),
    add constraint gateway_request_journal_outcome_evidence_pair_check
    check (
        (outcome = 'PENDING' and outcome_evidence in ('INTENT_RECORDED', 'QUEUE_ACCEPTED'))
        or (outcome = 'COMMITTED' and outcome_evidence in (
            'SUCCESS_RESPONSE', 'OWNER_REPLAY_RECEIPT', 'MANUAL_RESOLUTION'
        ))
        or (outcome = 'REJECTED_BEFORE_COMMIT' and outcome_evidence in (
            'PRE_DISPATCH_REJECTION', 'MANUAL_RESOLUTION'
        ))
        or (outcome = 'UNKNOWN' and outcome_evidence in (
            'RESPONSE_STATUS_ONLY', 'MANUAL_RESOLUTION', 'STALE_OPERATION',
            'LEGACY_UNRESOLVED'
        ))
    ),
    add constraint gateway_request_journal_state_outcome_check
    check (
        (state in ('IN_PROGRESS', 'QUEUED') and outcome = 'PENDING')
        or (state in ('COMPLETED', 'UNKNOWN') and outcome <> 'PENDING')
    );
