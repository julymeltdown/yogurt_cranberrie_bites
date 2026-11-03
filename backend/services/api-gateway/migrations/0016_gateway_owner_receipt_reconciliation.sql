-- A matching owner receipt proves that an UNKNOWN operation committed. The
-- Gateway records that as COMMITTED with OWNER_RECEIPT evidence. This timestamp
-- leases one bounded lookup attempt, not a permanent negative result: a missing
-- or incomplete receipt cannot fence a late owner commit.
alter table gateway_request_journal
    add column owner_receipt_checked_at timestamptz;

alter table gateway_request_journal
    drop constraint gateway_request_journal_outcome_evidence_check,
    drop constraint gateway_request_journal_outcome_evidence_pair_check;

alter table gateway_request_journal
    add constraint gateway_request_journal_outcome_evidence_check
    check (outcome_evidence in (
        'INTENT_RECORDED', 'QUEUE_ACCEPTED', 'SUCCESS_RESPONSE',
        'PRE_DISPATCH_REJECTION', 'RESPONSE_STATUS_ONLY',
        'OWNER_REPLAY_RECEIPT', 'OWNER_RECEIPT', 'MANUAL_RESOLUTION',
        'STALE_OPERATION', 'LEGACY_UNRESOLVED'
    )),
    add constraint gateway_request_journal_outcome_evidence_pair_check
    check (
        (outcome = 'PENDING' and outcome_evidence in ('INTENT_RECORDED', 'QUEUE_ACCEPTED'))
        or (outcome = 'COMMITTED' and outcome_evidence in (
            'SUCCESS_RESPONSE', 'OWNER_REPLAY_RECEIPT', 'OWNER_RECEIPT', 'MANUAL_RESOLUTION'
        ))
        or (outcome = 'REJECTED_BEFORE_COMMIT' and outcome_evidence in (
            'PRE_DISPATCH_REJECTION', 'MANUAL_RESOLUTION'
        ))
        or (outcome = 'UNKNOWN' and outcome_evidence in (
            'RESPONSE_STATUS_ONLY', 'MANUAL_RESOLUTION', 'STALE_OPERATION',
            'LEGACY_UNRESOLVED'
        ))
    );

create index gateway_request_journal_owner_receipt_pending_idx
    on gateway_request_journal (coalesce(owner_receipt_checked_at, created_at), id)
    where state = 'UNKNOWN' and outcome = 'UNKNOWN'
      and outcome_evidence <> 'MANUAL_RESOLUTION';
