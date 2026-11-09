-- Discovery projection: the block consumer's durable cursor.
--
-- `discovery_blocks` mirrors the friend service's block state so the discovery
-- query can filter blocked pairs in one local join. The signals that drive it
-- are read from the friend service's invalidation journal, which is a
-- **replayable** stream: a consumer advances its own cursor only after it has
-- applied what it read, and a crash re-reads from the last committed position.
--
-- This table is that position. It is deliberately separate from
-- `discovery_projection_state` (which tracks the *profile* stream) because the
-- two streams advance independently: a profile pass can be caught up while the
-- block cursor is far behind, and vice versa. Folding them into one row would
-- make one stream's progress hide the other's lag.
--
-- The cursor columns mirror `OwnerCursor`: an epoch (the journal's incarnation)
-- and a sequence. A journal that restarts with a new epoch is a different
-- stream, so the epoch is stored and compared rather than assumed constant.
--
-- There is no score, rank or count column here; this is a position, not a
-- property of any profile.

create table discovery_block_cursor (
    -- One singleton row per consumer. The consumer id is the concert service's
    -- stable identity for this stream, so a second consumer could be added
    -- later without a schema change.
    consumer_id uuid primary key
        check (consumer_id <> '00000000-0000-0000-0000-000000000000'::uuid),
    -- The journal epoch the cursor belongs to. Null before the first read.
    epoch uuid,
    -- The last applied sequence within that epoch. Null before the first read.
    sequence bigint,
    updated_at timestamptz not null default clock_timestamp(),
    -- The last failure, cleared by the next successful poll. Diagnostic only.
    last_failure text,

    constraint discovery_block_cursor_paired
        check ((epoch is null) = (sequence is null)),
    constraint discovery_block_cursor_sequence
        check (sequence is null or sequence >= 0),
    constraint discovery_block_cursor_failure_shape
        check (last_failure is null
               or (char_length(last_failure) between 1 and 400
                   and last_failure !~ '[\u0000-\u001f\u007f]'))
);

-- The cursor is a position, not a ranking. Nothing here names a profile, so the
-- discovery query's "no score column" property is untouched by this table.
create function discovery_block_cursor_guard() returns trigger language plpgsql as $$
begin
    if new.consumer_id is distinct from old.consumer_id then
        raise exception 'discovery block cursor consumer is immutable'
            using errcode = '23514', constraint = 'discovery_block_cursor_consumer_immutable';
    end if;
    -- A cursor never moves backwards within an epoch. A new epoch may reset the
    -- sequence, which is why the comparison is scoped to the same epoch.
    if old.epoch is not null
       and new.epoch = old.epoch
       and new.sequence < old.sequence then
        raise exception 'discovery block cursor cannot regress'
            using errcode = '23514', constraint = 'discovery_block_cursor_regress';
    end if;
    new.updated_at := clock_timestamp();
    return new;
end $$;

create trigger discovery_block_cursor_guard
    before update on discovery_block_cursor
    for each row execute function discovery_block_cursor_guard();
