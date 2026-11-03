create table gateway_operation_manual_reconciliations (
    resolution_id uuid primary key,
    operation_id uuid not null references gateway_request_journal(id) on delete restrict,
    revision integer not null check (revision > 0),
    supersedes_resolution_id uuid null,
    disposition text not null check (disposition in ('COMMITTED', 'NOT_COMMITTED', 'UNRESOLVED')),
    evidence_kind text not null check (evidence_kind in (
        'DATABASE_AUDIT', 'OWNER_CONFIRMATION', 'SERVICE_LOG', 'INCIDENT_RECORD', 'OTHER'
    )),
    evidence_reference text not null check (
        octet_length(evidence_reference) between 3 and 256
        and evidence_reference ~ '^[A-Za-z0-9][A-Za-z0-9._:/#-]*$'
    ),
    reason text not null check (octet_length(reason) between 20 and 1024),
    reviewer_id text not null check (octet_length(reviewer_id) between 1 and 256),
    created_at timestamptz not null default now(),
    unique (operation_id, revision),
    unique (supersedes_resolution_id),
    unique (operation_id, resolution_id),
    foreign key (operation_id, supersedes_resolution_id)
        references gateway_operation_manual_reconciliations(operation_id, resolution_id)
        on delete restrict
);

create index gateway_operation_manual_reconciliations_operation_idx
    on gateway_operation_manual_reconciliations (operation_id, revision desc);

create function reject_gateway_operation_manual_reconciliation_mutation()
returns trigger language plpgsql as $$
begin
    raise exception 'gateway operation reconciliation records are append-only';
    return null;
end;
$$;

create trigger gateway_operation_manual_reconciliations_append_only
    before update or delete on gateway_operation_manual_reconciliations
    for each row execute function reject_gateway_operation_manual_reconciliation_mutation();

create trigger gateway_operation_manual_reconciliations_no_truncate
    before truncate on gateway_operation_manual_reconciliations
    for each statement execute function reject_gateway_operation_manual_reconciliation_mutation();
