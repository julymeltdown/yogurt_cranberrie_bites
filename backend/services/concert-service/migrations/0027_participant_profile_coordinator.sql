-- BE-PARTICIPANT-LIVE. A PENDING row is a recovery registry, never permission.
-- COMMIT is written in the same native transaction as the participant CAS.
-- Neither a timeout nor a missing decision establishes an ABORT decision.
CREATE TABLE public.concert_participant_profile_operations (
    operation_id uuid PRIMARY KEY CHECK (operation_id <> '00000000-0000-0000-0000-000000000000'),
    actor_id uuid NOT NULL CHECK (actor_id <> '00000000-0000-0000-0000-000000000000'),
    event_id uuid NOT NULL CHECK (event_id <> '00000000-0000-0000-0000-000000000000'),
    session_family_id uuid NOT NULL CHECK (session_family_id <> '00000000-0000-0000-0000-000000000000'),
    parent_digest bytea NOT NULL CHECK (octet_length(parent_digest) = 32),
    body_digest bytea NOT NULL CHECK (octet_length(body_digest) = 32),
    guard_binding_digest bytea NOT NULL CHECK (octet_length(guard_binding_digest) = 32),
    release_digest text NOT NULL CHECK (release_digest ~ '^[0-9a-f]{64}$'),
    expected_revision bigint NOT NULL CHECK (expected_revision BETWEEN 1 AND 9007199254740990),
    concert_system_identifier text NOT NULL CHECK (concert_system_identifier ~ '^[1-9][0-9]{0,19}$'),
    concert_database_oid bigint NOT NULL CHECK (concert_database_oid BETWEEN 1 AND 4294967295),
    concert_full_xid text NOT NULL CHECK (concert_full_xid ~ '^[1-9][0-9]{0,19}$'),
    concert_backend_pid integer NOT NULL CHECK (concert_backend_pid > 0),
    concert_backend_started timestamptz NOT NULL,
    profile_epoch uuid NOT NULL CHECK (profile_epoch <> '00000000-0000-0000-0000-000000000000'),
    profile_revision bigint NOT NULL CHECK (profile_revision BETWEEN 1 AND 9007199254740991),
    -- Deterministic expected Profile receipt, fixed before either store writes.
    profile_prepared_gid text NOT NULL CHECK (profile_prepared_gid = 'mcr_sp_' || replace(operation_id::text, '-', '')),
    profile_effect_digest bytea NOT NULL CHECK (octet_length(profile_effect_digest) = 32),
    decision text NOT NULL DEFAULT 'PENDING' CHECK (decision IN ('PENDING', 'COMMIT')),
    committed_revision bigint,
    registered_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    decided_at timestamptz,
    CHECK ((decision = 'PENDING' AND committed_revision IS NULL AND decided_at IS NULL)
        OR (decision = 'COMMIT' AND profile_prepared_gid IS NOT NULL
            AND committed_revision = expected_revision + 1 AND decided_at IS NOT NULL))
);

-- Prevent accidental rebinding or overwriting a terminal decision. Recovery
-- may decide; it cannot change identity or the expected prepared effect.
CREATE FUNCTION public.concert_participant_profile_operation_immutable()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.operation_id IS DISTINCT FROM NEW.operation_id
       OR OLD.actor_id IS DISTINCT FROM NEW.actor_id
       OR OLD.event_id IS DISTINCT FROM NEW.event_id
       OR OLD.session_family_id IS DISTINCT FROM NEW.session_family_id
       OR OLD.parent_digest IS DISTINCT FROM NEW.parent_digest
       OR OLD.body_digest IS DISTINCT FROM NEW.body_digest
       OR OLD.guard_binding_digest IS DISTINCT FROM NEW.guard_binding_digest
       OR OLD.release_digest IS DISTINCT FROM NEW.release_digest
       OR OLD.expected_revision IS DISTINCT FROM NEW.expected_revision
       OR OLD.concert_system_identifier IS DISTINCT FROM NEW.concert_system_identifier
       OR OLD.concert_database_oid IS DISTINCT FROM NEW.concert_database_oid
       OR OLD.concert_full_xid IS DISTINCT FROM NEW.concert_full_xid
       OR OLD.concert_backend_pid IS DISTINCT FROM NEW.concert_backend_pid
       OR OLD.concert_backend_started IS DISTINCT FROM NEW.concert_backend_started
       OR OLD.registered_at IS DISTINCT FROM NEW.registered_at
       OR OLD.profile_epoch IS DISTINCT FROM NEW.profile_epoch
       OR OLD.profile_revision IS DISTINCT FROM NEW.profile_revision
       OR OLD.profile_prepared_gid IS DISTINCT FROM NEW.profile_prepared_gid
       OR OLD.profile_effect_digest IS DISTINCT FROM NEW.profile_effect_digest
       OR OLD.decision = 'COMMIT'
    THEN
        RAISE EXCEPTION 'participant profile operation is immutable' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_participant_profile_operation_immutable
BEFORE UPDATE ON public.concert_participant_profile_operations
FOR EACH ROW EXECUTE FUNCTION public.concert_participant_profile_operation_immutable();

-- No FK/cascade: deleting an event must not erase evidence needed to settle an
-- already prepared Profile transaction. Retention is an explicit recovery duty.
CREATE INDEX concert_participant_profile_operations_pending
ON public.concert_participant_profile_operations (registered_at)
WHERE decision = 'PENDING';
