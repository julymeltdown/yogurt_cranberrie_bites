alter table gateway_request_journal
    drop constraint if exists gateway_request_journal_state_check;

alter table gateway_request_journal
    add constraint gateway_request_journal_state_check
    check (state in ('IN_PROGRESS', 'QUEUED', 'COMPLETED', 'UNKNOWN'));
