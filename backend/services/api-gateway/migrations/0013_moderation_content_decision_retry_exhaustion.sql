alter table moderation_content_decision_outbox
    drop constraint moderation_content_decision_outbox_state_check;

alter table moderation_content_decision_outbox
    add constraint moderation_content_decision_outbox_state_check
    check (state in (
        'PENDING',
        'TARGET_UNAVAILABLE',
        'COMPLETED',
        'RESOLVED_UNAVAILABLE',
        'RETRY_EXHAUSTED'
    ));

alter table moderation_content_decision_outbox
    add column replay_count integer not null default 0 check (replay_count >= 0);

create table moderation_content_decision_replays (
    resolution_id uuid primary key,
    report_id uuid not null references moderation_reports(id) on delete restrict,
    actor_id uuid not null,
    attempt_count integer not null check (attempt_count > 0),
    last_error_code text not null check (last_error_code in (
        'POST_SERVICE_CALL_FAILED',
        'GATEWAY_DECISION_STORE_FAILED'
    )),
    reason text not null check (octet_length(reason) between 20 and 1024),
    evidence_reference text not null check (
        octet_length(evidence_reference) between 3 and 256
        and evidence_reference ~ '^[A-Za-z0-9][A-Za-z0-9._:/#-]*$'
    ),
    created_at timestamptz not null default now(),
    unique (report_id, resolution_id)
);

create index moderation_content_decision_replays_report_idx
    on moderation_content_decision_replays (report_id, created_at, resolution_id);

create function reject_moderation_content_decision_replay_mutation()
returns trigger language plpgsql as $$
begin
    raise exception 'moderation content decision replay records are append-only';
    return null;
end;
$$;

create trigger moderation_content_decision_replays_append_only
    before update or delete on moderation_content_decision_replays
    for each row execute function reject_moderation_content_decision_replay_mutation();

create trigger moderation_content_decision_replays_no_truncate
    before truncate on moderation_content_decision_replays
    for each statement execute function reject_moderation_content_decision_replay_mutation();
