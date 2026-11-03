-- No MAX/version bootstrap: existing history is replayed only from a proven
-- version-1 creation, with contiguous owner events and per-effect provenance.
-- Legacy NULL-version history and missing source prefixes require explicit adoption.
create table timeline_projection_cursors (
    aggregate_id uuid primary key,
    last_version bigint not null default 0 check (last_version >= 0),
    updated_at timestamptz not null default clock_timestamp()
);
create table timeline_projection_receipts (
    event_id uuid primary key,
    aggregate_id uuid not null references timeline_projection_cursors(aggregate_id),
    aggregate_version bigint not null check (aggregate_version > 0),
    identity_sha256 text not null check (identity_sha256 ~ '^[0-9a-f]{64}$'),
    disposition text not null check (disposition in ('APPLIED','ADOPTED','SUPPRESSED','NOOP')),
    delivery_event_id uuid not null,
    delivery_attempt_id uuid not null,
    applied_at timestamptz not null default clock_timestamp(),
    unique (aggregate_id, aggregate_version)
);
-- Receipts deliberately outlive outbox pruning. No new retention duration is
-- invented; D2/D7 adoption/retention and old-writer drain remain cutover gates.
create index post_outbox_projection_sequence_idx
    on post_event_outbox (aggregate_id, aggregate_version)
    where aggregate_version is not null;
create index post_outbox_projection_legacy_idx
    on post_event_outbox (aggregate_id) where aggregate_version is null;

-- Deferred commit-time authority check runs after all effect/insertion waits.
-- Job ownership stays locked by apply until commit. Receipt IDs survive source
-- pruning; no outbox/attempt foreign key or permanent lease-token copy is added.
create function timeline_projection_assert_delivery_authority()
returns trigger language plpgsql as $$
begin
    if not exists (
        select 1 from post_event_delivery_jobs
        where event_id=new.delivery_event_id and target='TIMELINE'
          and state='LEASED' and attempt_id=new.delivery_attempt_id
          and lease_expires_at>clock_timestamp()
    ) then
        raise exception 'Timeline delivery authority expired before commit'
            using errcode='23514', constraint='timeline_projection_delivery_authority';
    end if;
    return new;
end;
$$;
create constraint trigger timeline_projection_delivery_authority
    after insert on timeline_projection_receipts
    deferrable initially deferred
    for each row execute function timeline_projection_assert_delivery_authority();
