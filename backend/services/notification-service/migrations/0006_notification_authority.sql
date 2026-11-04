-- Notification is a genuine producing owner. These records contain no public
-- payload or root completion capability and outlive inbox retention/deletion.
CREATE TABLE concert_notification_producer_instance (
 singleton boolean PRIMARY KEY DEFAULT true CHECK(singleton),
 instance_id uuid NOT NULL UNIQUE CHECK(instance_id <> '00000000-0000-0000-0000-000000000000')
);
CREATE TABLE concert_notification_authority_intents (
 child_id uuid PRIMARY KEY CHECK(child_id <> '00000000-0000-0000-0000-000000000000'),
 binding_proto bytea NOT NULL CHECK(octet_length(binding_proto) BETWEEN 1 AND 32768),
 binding_digest bytea NOT NULL CHECK(octet_length(binding_digest)=32),
 request_digest bytea NOT NULL CHECK(octet_length(request_digest)=32),
 capability_hash bytea NOT NULL CHECK(octet_length(capability_hash)=32),
 sealed boolean NOT NULL DEFAULT false,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
-- Written on a separate owner connection BEFORE Track and before protected
-- effects. Keeping this separate from the intent header permits the actual
-- repeatable-read inbox transaction to retain its original snapshot.
CREATE TABLE concert_notification_native_references (
 child_id uuid PRIMARY KEY REFERENCES concert_notification_authority_intents(child_id),
 identity_json jsonb NOT NULL CHECK(jsonb_typeof(identity_json)='object'),
 identity_digest bytea NOT NULL UNIQUE CHECK(octet_length(identity_digest)=32),
 auth_registration uuid,
 friend_registration uuid,
 CHECK(auth_registration IS NULL OR auth_registration <> '00000000-0000-0000-0000-000000000000'),
 CHECK(friend_registration IS NULL OR friend_registration <> '00000000-0000-0000-0000-000000000000')
);
-- Inserted on the actual inbox transaction after BOTH real Track replies. A
-- committed native XID without this matching witness is UNKNOWN, never success.
CREATE TABLE concert_notification_transaction_witnesses (
 child_id uuid PRIMARY KEY REFERENCES concert_notification_authority_intents(child_id),
 identity_json jsonb NOT NULL CHECK(jsonb_typeof(identity_json)='object'),
 identity_digest bytea NOT NULL CHECK(octet_length(identity_digest)=32),
 committed_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE FUNCTION concert_notification_evidence_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 RAISE EXCEPTION 'notification authority evidence is immutable' USING ERRCODE='23514';
END $$;
CREATE TRIGGER notification_instance_immutable BEFORE UPDATE OR DELETE OR TRUNCATE ON concert_notification_producer_instance
 FOR EACH STATEMENT EXECUTE FUNCTION concert_notification_evidence_immutable();
CREATE TRIGGER notification_witness_immutable BEFORE UPDATE OR DELETE OR TRUNCATE ON concert_notification_transaction_witnesses
 FOR EACH STATEMENT EXECUTE FUNCTION concert_notification_evidence_immutable();
CREATE TRIGGER notification_intent_retention BEFORE DELETE OR TRUNCATE ON concert_notification_authority_intents
 FOR EACH STATEMENT EXECUTE FUNCTION concert_notification_evidence_immutable();
CREATE TRIGGER notification_native_retention BEFORE DELETE OR TRUNCATE ON concert_notification_native_references
 FOR EACH STATEMENT EXECUTE FUNCTION concert_notification_evidence_immutable();
CREATE FUNCTION concert_notification_intent_transition() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF (to_jsonb(NEW)-'sealed') IS DISTINCT FROM (to_jsonb(OLD)-'sealed') OR OLD.sealed OR NOT NEW.sealed THEN
   RAISE EXCEPTION 'notification intent transition denied' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER notification_intent_transition BEFORE UPDATE ON concert_notification_authority_intents
 FOR EACH ROW EXECUTE FUNCTION concert_notification_intent_transition();
CREATE FUNCTION concert_notification_native_transition() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE closed boolean;
BEGIN
 SELECT sealed INTO STRICT closed FROM concert_notification_authority_intents WHERE child_id=NEW.child_id FOR SHARE;
 IF closed THEN RAISE EXCEPTION 'notification intent sealed' USING ERRCODE='23514'; END IF;
 IF TG_OP='UPDATE' AND (
    (to_jsonb(NEW)-'auth_registration'-'friend_registration') IS DISTINCT FROM (to_jsonb(OLD)-'auth_registration'-'friend_registration')
    OR (OLD.auth_registration IS NOT NULL AND NEW.auth_registration IS DISTINCT FROM OLD.auth_registration)
    OR (OLD.friend_registration IS NOT NULL AND NEW.friend_registration IS DISTINCT FROM OLD.friend_registration)) THEN
   RAISE EXCEPTION 'notification native identity rewrite denied' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER notification_native_transition BEFORE INSERT OR UPDATE ON concert_notification_native_references
 FOR EACH ROW EXECUTE FUNCTION concert_notification_native_transition();
CREATE FUNCTION concert_notification_native_witness() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE closed boolean; actual_start timestamptz;
BEGIN
 -- This row lock is retained until the ACTUAL inbox commit; sealing serializes
 -- against it. A seal that won first causes rollback, including under RR.
 SELECT sealed INTO STRICT closed FROM concert_notification_authority_intents WHERE child_id=NEW.child_id FOR SHARE;
 SELECT backend_start INTO STRICT actual_start FROM pg_stat_activity WHERE pid=pg_backend_pid();
 IF closed OR NOT EXISTS(SELECT 1 FROM concert_notification_producer_instance WHERE singleton) OR NEW.identity_json->>'full_xid' IS DISTINCT FROM pg_current_xact_id()::text
    OR NEW.identity_json->>'system_identifier' IS DISTINCT FROM (SELECT system_identifier::text FROM pg_control_system())
    OR NEW.identity_json->>'database_oid' IS DISTINCT FROM (SELECT oid::text FROM pg_database WHERE datname=current_database())
    OR NEW.identity_json->>'backend_pid' IS DISTINCT FROM pg_backend_pid()::text
    OR (NEW.identity_json->>'backend_started_micros')::bigint IS DISTINCT FROM (extract(epoch FROM actual_start)*1000000)::bigint
    OR NEW.identity_json->>'producer_instance' IS DISTINCT FROM (SELECT instance_id::text FROM concert_notification_producer_instance WHERE singleton)
 THEN RAISE EXCEPTION 'notification witness is not this native transaction' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER notification_native_witness BEFORE INSERT ON concert_notification_transaction_witnesses
 FOR EACH ROW EXECUTE FUNCTION concert_notification_native_witness();

CREATE CONSTRAINT TRIGGER notification_witness_deadline AFTER INSERT ON concert_notification_transaction_witnesses
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION concert_notification_authority_guard();
