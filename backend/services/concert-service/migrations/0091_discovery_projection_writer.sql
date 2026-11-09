-- WP-03 follow-up: the discovery projection watermark and its source cursor.
--
-- Migration 0086 created the projection tables but nothing wrote them, so
-- production discovery returned an empty list. The writer is a bounded
-- reconciliation job (see docs/mcr/integration/discovery-projection.md for why
-- that mechanism was chosen over an outbox or a synchronous call).
--
-- This migration adds exactly the bookkeeping that job needs and nothing else:
--
--   * `discovery_projection_state` — one row holding the watermark. The job
--     reads the profile service from `(source_updated_at, source_profile_id)`
--     forward and advances the watermark only after a page is committed, so a
--     crash re-reads the same page rather than skipping it.
--   * `discovery_projection_runs` — one row per completed pass, so an operator
--     can see when the projection last converged and whether a pass failed. It
--     is an observation table, never an input to a query.
--
-- Deliberately absent from both tables: any score, rank, weight or count of
-- favorites. The discovery query's "no score column" property is asserted by
-- tests/discovery.rs, and this migration must not weaken it. `discovery_projection_runs`
-- counts *pages applied*, which is a job diagnostic about the writer, not a
-- property of any profile, and the discovery query never reads it.

create table discovery_projection_state (
    -- One singleton row. The check makes a second row impossible, so the
    -- watermark cannot fork into two writers that disagree.
    singleton boolean primary key default true check (singleton),
    -- The highest `(updated_at, profile_id)` the job has applied. Both are null
    -- before the first pass, which means "start from the beginning".
    source_updated_at timestamptz,
    source_profile_id uuid,
    -- When the last successful pass finished. Diagnostic only.
    updated_at timestamptz not null default clock_timestamp(),
    -- The last failure, cleared by the next successful pass. Diagnostic only.
    last_failure text,
    constraint discovery_projection_state_cursor_paired
        check ((source_updated_at is null) = (source_profile_id is null)),
    constraint discovery_projection_state_failure_shape
        check (last_failure is null
               or (char_length(last_failure) between 1 and 400
                   and last_failure !~ '[\u0000-\u001f\u007f]'))
);

-- The singleton starts empty: the first pass begins at the beginning of time.
insert into discovery_projection_state(singleton) values (true);

create table discovery_projection_runs (
    run_id uuid primary key default gen_random_uuid(),
    started_at timestamptz not null,
    finished_at timestamptz not null default clock_timestamp(),
    -- How many source pages this pass applied, and how many profiles it touched.
    pages integer not null default 0 check (pages >= 0),
    profiles integer not null default 0 check (profiles >= 0),
    -- Whether the pass reached the end of the source (caught up) or stopped at
    -- its bounded page budget with more work waiting.
    caught_up boolean not null,
    constraint discovery_projection_runs_times check (finished_at >= started_at)
);

create index discovery_projection_runs_recent
    on discovery_projection_runs (finished_at desc);

-- The watermark is read and advanced by the job. A profile's projection rows are
-- written in the same transaction as the watermark advance, so a discovery page
-- never observes a profile whose favorites were half-applied.
create function discovery_projection_state_advance() returns trigger language plpgsql as $$
begin
    if new.singleton is distinct from old.singleton then
        raise exception 'discovery projection state is a singleton'
            using errcode = '23514', constraint = 'discovery_projection_state_singleton';
    end if;
    return new;
end $$;

create trigger discovery_projection_state_advance
    before update on discovery_projection_state
    for each row execute function discovery_projection_state_advance();

-- A block row is mirrored, never invented. `discovery_blocks` already exists
-- from 0086; this index makes the job's "which blocks changed" read bounded.
create index discovery_blocks_changed
    on discovery_blocks (created_at desc, blocker_account_id, blocked_account_id);
