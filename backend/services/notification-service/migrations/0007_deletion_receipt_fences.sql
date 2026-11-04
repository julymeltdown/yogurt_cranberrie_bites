-- BE33 production deletion proof, version 1. Never backfill historical mutable
-- saga/audit rows: their past transaction and command cannot be established.
-- This grants no physical erasure policy and does not change retention purge.
CREATE TABLE notification_deletion_fences_v1 (
    subject_id uuid PRIMARY KEY REFERENCES notification_subject_states(subject_id),
    fence_revision bigint GENERATED ALWAYS AS IDENTITY UNIQUE CHECK (fence_revision > 0),
    established_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
    recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE (subject_id, fence_revision)
);
CREATE TABLE notification_deletion_receipts_v1 (
    saga_id uuid PRIMARY KEY CHECK (saga_id <> '00000000-0000-0000-0000-000000000000'),
    user_id uuid NOT NULL CHECK (user_id <> '00000000-0000-0000-0000-000000000000'),
    actor_id uuid NOT NULL CHECK (actor_id <> '00000000-0000-0000-0000-000000000000'),
    retention_until timestamptz NOT NULL CHECK (isfinite(retention_until)),
    -- timestamptz has microsecond precision. Keep the original command's exact
    -- nanoseconds as well, so a sub-microsecond rebound is not an exact retry.
    retention_seconds bigint NOT NULL,
    retention_nanos integer NOT NULL CHECK (retention_nanos BETWEEN 0 AND 999999999),
    fence_revision bigint NOT NULL CHECK (fence_revision > 0),
    audit_event_id uuid NOT NULL UNIQUE REFERENCES notification_audit_events(id),
    effect_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
    recorded_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    FOREIGN KEY (user_id, fence_revision)
        REFERENCES notification_deletion_fences_v1(subject_id, fence_revision),
    CHECK (audit_event_id = saga_id)
);

CREATE FUNCTION notification_deletion_proof_immutable_v1() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'notification deletion proof is immutable' USING ERRCODE = '23514';
END $$;
CREATE TRIGGER notification_deletion_fence_immutable_v1
    BEFORE UPDATE OR DELETE OR TRUNCATE ON notification_deletion_fences_v1
    FOR EACH STATEMENT EXECUTE FUNCTION notification_deletion_proof_immutable_v1();
CREATE TRIGGER notification_deletion_receipt_immutable_v1
    BEFORE UPDATE OR DELETE OR TRUNCATE ON notification_deletion_receipts_v1
    FOR EACH STATEMENT EXECUTE FUNCTION notification_deletion_proof_immutable_v1();

-- Permanent deny is anchored in the SAME existing state used by old creation
-- and Concert replay. No copied table substitutes for that serialization point.
CREATE FUNCTION notification_deletion_subject_guard_v1() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'TRUNCATE' THEN
        IF EXISTS (SELECT 1 FROM notification_subject_states WHERE suppress_as_recipient OR suppress_as_actor) THEN
            RAISE EXCEPTION 'notification subject deny is permanent' USING ERRCODE = '23514';
        END IF;
        RETURN NULL;
    END IF;
    IF OLD.suppress_as_recipient OR OLD.suppress_as_actor THEN
        IF TG_OP = 'DELETE' THEN
            RAISE EXCEPTION 'notification subject deny is permanent' USING ERRCODE = '23514';
        END IF;
        IF NEW.subject_id IS DISTINCT FROM OLD.subject_id
            OR (OLD.suppress_as_recipient AND NOT NEW.suppress_as_recipient)
            OR (OLD.suppress_as_actor AND NOT NEW.suppress_as_actor) THEN
            RAISE EXCEPTION 'notification subject deny is permanent' USING ERRCODE = '23514';
        END IF;
    END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER notification_deletion_subject_guard_v1
    BEFORE UPDATE OR DELETE ON notification_subject_states
    FOR EACH ROW EXECUTE FUNCTION notification_deletion_subject_guard_v1();
CREATE TRIGGER notification_deletion_subject_truncate_v1
    BEFORE TRUNCATE ON notification_subject_states
    FOR EACH STATEMENT EXECUTE FUNCTION notification_deletion_subject_guard_v1();

-- Historical rows keep their compatibility behavior. Once linked to a v1
-- proof, the original saga receipt AND original audit become immutable too.
CREATE FUNCTION notification_deletion_link_guard_v1() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE linked boolean;
BEGIN
    IF TG_OP = 'TRUNCATE' THEN
        linked := EXISTS (SELECT 1 FROM notification_deletion_receipts_v1);
    ELSIF TG_TABLE_NAME = 'notification_audit_events' THEN
        linked := EXISTS (SELECT 1 FROM notification_deletion_receipts_v1 WHERE audit_event_id = OLD.id);
    ELSE
        linked := EXISTS (SELECT 1 FROM notification_deletion_receipts_v1 WHERE saga_id = OLD.saga_id);
    END IF;
    IF linked THEN
        RAISE EXCEPTION 'notification deletion evidence is immutable' USING ERRCODE = '23514';
    END IF;
    IF TG_OP = 'TRUNCATE' THEN RETURN NULL; END IF;
    IF TG_OP = 'DELETE' THEN RETURN OLD; END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER notification_deletion_audit_guard_v1
    BEFORE UPDATE OR DELETE ON notification_audit_events
    FOR EACH ROW EXECUTE FUNCTION notification_deletion_link_guard_v1();
CREATE TRIGGER notification_deletion_audit_truncate_v1
    BEFORE TRUNCATE ON notification_audit_events
    FOR EACH STATEMENT EXECUTE FUNCTION notification_deletion_link_guard_v1();
CREATE TRIGGER notification_deletion_legacy_guard_v1
    BEFORE UPDATE OR DELETE ON notification_user_deletion_receipts
    FOR EACH ROW EXECUTE FUNCTION notification_deletion_link_guard_v1();
CREATE TRIGGER notification_deletion_legacy_truncate_v1
    BEFORE TRUNCATE ON notification_user_deletion_receipts
    FOR EACH STATEMENT EXECUTE FUNCTION notification_deletion_link_guard_v1();

-- Updating deleted_at alone must not resurrect a notification for a denied
-- subject. Existing ordered creation locks remain unchanged; no new lock order
-- is introduced into UPDATE of a notification already locked by its writer.
CREATE FUNCTION notification_deletion_recreation_guard_v1() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.deleted_at IS NULL AND EXISTS (
        SELECT 1 FROM notification_subject_states
        WHERE (subject_id = NEW.user_id AND suppress_as_recipient)
           OR (subject_id = NEW.actor_id AND suppress_as_actor)
    ) THEN
        RAISE EXCEPTION 'notification recreation denied' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER notification_deletion_recreation_guard_v1
    BEFORE UPDATE OF deleted_at ON notifications
    FOR EACH ROW EXECUTE FUNCTION notification_deletion_recreation_guard_v1();

CREATE FUNCTION notification_deletion_fence_insert_v1() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.established_xid IS DISTINCT FROM pg_current_xact_id()
       OR NOT EXISTS (SELECT 1 FROM notification_subject_states
                      WHERE subject_id = NEW.subject_id AND suppress_as_recipient AND suppress_as_actor)
       OR EXISTS (SELECT 1 FROM notifications
                  WHERE (user_id = NEW.subject_id OR actor_id = NEW.subject_id) AND deleted_at IS NULL) THEN
        RAISE EXCEPTION 'notification deletion fence lacks actual effects' USING ERRCODE = '23514';
    END IF;
    RETURN NEW;
END $$;
CREATE TRIGGER notification_deletion_fence_insert_v1
    BEFORE INSERT ON notification_deletion_fences_v1
    FOR EACH ROW EXECUTE FUNCTION notification_deletion_fence_insert_v1();

-- Deferred until the actual commit: a queued job, old mutable receipt, unrelated
-- transaction, missing audit, or effects rolled back later cannot certify it.
CREATE FUNCTION notification_deletion_receipt_commit_v1() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.effect_xid IS DISTINCT FROM pg_current_xact_id()
       OR NEW.retention_until IS DISTINCT FROM
          (timestamptz 'epoch' + NEW.retention_seconds * interval '1 second'
           + (NEW.retention_nanos / 1000) * interval '1 microsecond')
       OR NOT EXISTS (SELECT 1 FROM notification_subject_states
                      WHERE subject_id = NEW.user_id AND suppress_as_recipient AND suppress_as_actor)
       OR EXISTS (SELECT 1 FROM notifications
                  WHERE (user_id = NEW.user_id OR actor_id = NEW.user_id) AND deleted_at IS NULL)
       OR NOT EXISTS (
           SELECT 1 FROM notification_user_deletion_receipts r
           WHERE r.saga_id = NEW.saga_id AND r.user_id = NEW.user_id
             AND r.actor_id = NEW.actor_id AND r.retention_until = NEW.retention_until
             AND r.xmin::text::bigint = mod(pg_current_xact_id()::text::numeric, 4294967296)::bigint)
       OR NOT EXISTS (
           SELECT 1 FROM notification_audit_events a
           WHERE a.id = NEW.audit_event_id AND a.user_id = NEW.user_id AND a.actor_id = NEW.actor_id
             AND a.action = 'USER_DELETION_APPLIED' AND a.outcome = 'SUCCESS'
             AND a.notification_id IS NULL
             AND a.xmin::text::bigint = mod(pg_current_xact_id()::text::numeric, 4294967296)::bigint
             AND jsonb_typeof(a.metadata_json::jsonb->'retentionUntil') = 'string'
             AND (a.metadata_json::jsonb - 'retentionUntil') = jsonb_build_object(
                 'sagaId', NEW.saga_id, 'retentionSeconds', NEW.retention_seconds,
                 'retentionNanos', NEW.retention_nanos, 'fenceRevision', NEW.fence_revision,
                 'proofVersion', 1)) THEN
        RAISE EXCEPTION 'notification deletion receipt lacks actual transaction proof' USING ERRCODE = '23514';
    END IF;
    RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER notification_deletion_receipt_commit_v1
    AFTER INSERT ON notification_deletion_receipts_v1
    DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
    EXECUTE FUNCTION notification_deletion_receipt_commit_v1();
