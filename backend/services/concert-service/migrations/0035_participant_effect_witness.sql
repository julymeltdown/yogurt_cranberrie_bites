-- Correlation evidence only: authority still comes from the held owner scope.
-- A COMMITTED outcome requires this immutable row in the effect transaction.
CREATE TABLE public.concert_participant_effect_witnesses (
    operation_id uuid PRIMARY KEY CHECK(operation_id <> '00000000-0000-0000-0000-000000000000'),
    actor_id uuid NOT NULL CHECK(actor_id <> '00000000-0000-0000-0000-000000000000'),
    event_id uuid NOT NULL CHECK(event_id <> '00000000-0000-0000-0000-000000000000'),
    session_family_id uuid NOT NULL CHECK(session_family_id <> '00000000-0000-0000-0000-000000000000'),
    producer_instance_id uuid NOT NULL,
    scope_id uuid NOT NULL,
    parent_digest bytea NOT NULL CHECK(octet_length(parent_digest)=32),
    guard_binding_digest bytea NOT NULL CHECK(octet_length(guard_binding_digest)=32),
    body_digest bytea NOT NULL CHECK(octet_length(body_digest)=32),
    release_digest text NOT NULL CHECK(release_digest ~ '^[0-9a-f]{64}$'),
    effect_binding_digest bytea NOT NULL CHECK(octet_length(effect_binding_digest)=32),
    identity_digest bytea NOT NULL CHECK(octet_length(identity_digest)=32),
    mutation_kind text NOT NULL CHECK(mutation_kind IN ('AVATAR','PREFERENCES')),
    expected_revision bigint NOT NULL CHECK(expected_revision BETWEEN 1 AND 9007199254740990),
    committed_revision bigint NOT NULL CHECK(committed_revision=expected_revision+1),
    concert_system_identifier text NOT NULL CHECK(concert_system_identifier ~ '^[1-9][0-9]{0,19}$'),
    concert_database_oid bigint NOT NULL CHECK(concert_database_oid BETWEEN 1 AND 4294967295),
    concert_full_xid text NOT NULL CHECK(concert_full_xid ~ '^[1-9][0-9]{0,19}$'),
    concert_backend_pid integer NOT NULL CHECK(concert_backend_pid>0),
    concert_backend_started timestamptz NOT NULL,
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    FOREIGN KEY(producer_instance_id,scope_id,guard_binding_digest)
      REFERENCES public.concert_product_owner_scopes(instance_id,scope_id,binding_digest),
    FOREIGN KEY(producer_instance_id,scope_id,identity_digest)
      REFERENCES public.concert_product_owner_transactions(instance_id,scope_id,identity_digest)
);

CREATE FUNCTION public.concert_participant_effect_witness_guard()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
    bound jsonb;
    tracked jsonb;
    actual_revision bigint;
    actual_xmin text;
    parent_bytes jsonb;
    body_bytes jsonb;
    effect_bytes jsonb;
    reference_bytes jsonb;
BEGIN
    IF TG_OP <> 'INSERT' THEN
        RAISE EXCEPTION 'participant effect witness is immutable' USING ERRCODE='23514';
    END IF;
    IF NEW.concert_full_xid <> pg_current_xact_id()::text
       OR NEW.concert_backend_pid <> pg_backend_pid()
       OR NEW.concert_database_oid <> (SELECT oid::bigint FROM pg_database WHERE datname=current_database())
       OR NEW.concert_system_identifier <> (SELECT system_identifier::text FROM pg_control_system())
       OR NEW.concert_backend_started <> (SELECT backend_start FROM pg_stat_activity WHERE pid=pg_backend_pid())
    THEN
        RAISE EXCEPTION 'participant witness requires actual effect transaction' USING ERRCODE='23514';
    END IF;
    -- Registration and publication precede effects. Locking the scope also
    -- serializes this final witness with durable seal/cancellation.
    SELECT s.binding,t.identity INTO bound,tracked
      FROM public.concert_product_owner_scopes s
      JOIN public.concert_product_owner_transactions t USING(instance_id,scope_id,binding_digest)
      WHERE s.instance_id=NEW.producer_instance_id AND s.scope_id=NEW.scope_id
        AND s.binding_digest=NEW.guard_binding_digest AND s.phase='ACTIVE'
        AND s.delivery_secret_hash IS NOT NULL AND t.identity_digest=NEW.identity_digest
        AND t.confirmed_at IS NOT NULL
      FOR SHARE OF s;
    IF bound IS NULL OR tracked IS NULL THEN
        RAISE EXCEPTION 'participant witness requires published confirmed scope' USING ERRCODE='23514';
    END IF;
    SELECT jsonb_agg(get_byte(NEW.parent_digest,n) ORDER BY n) INTO parent_bytes FROM generate_series(0,31) n;
    SELECT jsonb_agg(get_byte(NEW.body_digest,n) ORDER BY n) INTO body_bytes FROM generate_series(0,31) n;
    SELECT jsonb_agg(get_byte(NEW.effect_binding_digest,n) ORDER BY n) INTO effect_bytes FROM generate_series(0,31) n;
    SELECT jsonb_agg(get_byte(uuid_send(NEW.operation_id),n) ORDER BY n) INTO reference_bytes FROM generate_series(0,15) n;
    IF NOT bound @> jsonb_build_object(
        'actor_id',NEW.actor_id,'event_id',NEW.event_id,'session_family_id',NEW.session_family_id,
        'request_id',NEW.operation_id,'parent_digest',parent_bytes,'body_digest',body_bytes)
       OR NOT tracked = jsonb_build_object(
        'producer','CONCERT','system_identifier',NEW.concert_system_identifier,
        'database_oid',NEW.concert_database_oid,'full_xid',NEW.concert_full_xid,
        'backend_pid',NEW.concert_backend_pid,
        'backend_started_unix_micros',(extract(epoch FROM NEW.concert_backend_started)*1000000)::bigint,
        'effect_binding_digest',effect_bytes,'producer_instance',NEW.producer_instance_id,
        'settlement_reference',reference_bytes)
    THEN
        RAISE EXCEPTION 'participant witness registration mismatch' USING ERRCODE='23514';
    END IF;
    IF NEW.mutation_kind='AVATAR' THEN
        SELECT revision,xmin::text INTO actual_revision,actual_xmin
          FROM public.concert_avatar_settings WHERE event_id=NEW.event_id AND user_id=NEW.actor_id;
    ELSE
        SELECT revision,xmin::text INTO actual_revision,actual_xmin
          FROM public.concert_participants WHERE event_id=NEW.event_id AND user_id=NEW.actor_id;
    END IF;
    IF actual_revision IS DISTINCT FROM NEW.committed_revision
       OR actual_xmin IS DISTINCT FROM mod(pg_current_xact_id()::text::numeric,4294967296)::text
    THEN
        RAISE EXCEPTION 'participant witness requires same-transaction revision effect' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_participant_effect_witness_guard BEFORE INSERT OR UPDATE OR DELETE
ON public.concert_participant_effect_witnesses FOR EACH ROW
EXECUTE FUNCTION public.concert_participant_effect_witness_guard();
