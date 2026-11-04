-- Private native settlement evidence, never product authority or a public DTO.
-- Use ONLY with xmin from a row actually visible on this connection. PostgreSQL
-- never exposes another transaction's uncommitted row; an in-progress visible
-- xmin is therefore our write, including BE15's command_effects subtransaction.
-- Comparing xmin to the top-level xid would reject every savepoint mutation.
CREATE FUNCTION public.concert_social_own_visible_xmin(row_xmin xid) RETURNS boolean
LANGUAGE plpgsql VOLATILE STRICT AS $$
DECLARE top_xid numeric; distance numeric;
BEGIN
 IF row_xmin::text::numeric<3 THEN RETURN false; END IF;
 top_xid := pg_current_xact_id()::text::numeric;
 -- Own subtransactions are assigned AFTER the top-level xid, including across
 -- 32-bit wrap. Never subtract an epoch merely because their low bits are
 -- larger. A previous committed row is not an own effect. The half-range
 -- rejects old xids before asking pg_xact_status about a forward candidate.
 distance := mod(row_xmin::text::numeric-mod(top_xid,4294967296)+4294967296,4294967296);
 IF distance>=2147483648 THEN RETURN false; END IF;
 RETURN coalesce(pg_xact_status((top_xid+distance)::text::xid8)='in progress',false);
EXCEPTION WHEN invalid_parameter_value THEN
 -- An old visible xmin mapping into the future is never positive evidence.
 RETURN false;
END
$$;
CREATE TABLE public.concert_social_effect_witnesses (
 producer_instance uuid NOT NULL,
 scope_id uuid NOT NULL,
 identity_digest bytea NOT NULL CHECK(octet_length(identity_digest)=32),
 guard_binding_digest bytea NOT NULL CHECK(octet_length(guard_binding_digest)=32),
 effect_binding_digest bytea NOT NULL CHECK(octet_length(effect_binding_digest)=32),
 request_id uuid NOT NULL,
 actor_id uuid NOT NULL,
 event_id uuid NOT NULL,
 session_family_id uuid NOT NULL,
 operation text NOT NULL CHECK(operation IN (
   'listOwnConcertHandles','setConcertHandle','removeConcertHandle',
   'createConcertSocialRequest','listConcertSocialRequests','getConcertSocialRequest',
   'decideConcertSocialRequest','cancelConcertSocialRequest','revokeConcertSocialRequest',
   'revealConcertSocialHandle','listConcertBlocks','blockConcertUser','unblockConcertUser')),
 idempotency_key uuid,
 parent_digest bytea NOT NULL CHECK(octet_length(parent_digest)=32),
 body_digest bytea NOT NULL CHECK(octet_length(body_digest)=32),
 native_identity jsonb NOT NULL CHECK(jsonb_typeof(native_identity)='object'),
 resource_id uuid NOT NULL CHECK(resource_id<>'00000000-0000-0000-0000-000000000000'),
 resource_revision bigint NOT NULL CHECK(resource_revision BETWEEN 0 AND 9007199254740991),
 effect_kind text NOT NULL CHECK(effect_kind IN ('MUTATION','PROJECTION')),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(producer_instance,scope_id,identity_digest),
 FOREIGN KEY(producer_instance,scope_id,guard_binding_digest)
  REFERENCES public.concert_product_owner_scopes(instance_id,scope_id,binding_digest),
 FOREIGN KEY(producer_instance,scope_id,identity_digest)
  REFERENCES public.concert_product_owner_transactions(instance_id,scope_id,identity_digest)
);

CREATE FUNCTION public.concert_social_effect_witness_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE bound jsonb; tracked jsonb; actual_revision bigint; actual_xmin text;
 parent_bytes jsonb; body_bytes jsonb; effect_bytes jsonb; reference_bytes jsonb;
BEGIN
 IF TG_OP<>'INSERT' THEN
  RAISE EXCEPTION 'social effect witness is immutable' USING ERRCODE='23514';
 END IF;
 IF NEW.native_identity->>'producer' IS DISTINCT FROM 'CONCERT'
  OR NEW.native_identity->>'producer_instance' IS DISTINCT FROM NEW.producer_instance::text
  OR NEW.native_identity->>'system_identifier' IS DISTINCT FROM (SELECT system_identifier::text FROM pg_control_system())
  OR NEW.native_identity->>'database_oid' IS DISTINCT FROM (SELECT oid::text FROM pg_database WHERE datname=current_database())
  OR NEW.native_identity->>'full_xid' IS DISTINCT FROM pg_current_xact_id()::text
  OR NEW.native_identity->>'backend_pid' IS DISTINCT FROM pg_backend_pid()::text
  OR NEW.native_identity->>'backend_started_unix_micros' IS DISTINCT FROM
     (SELECT ((extract(epoch FROM backend_start)*1000000)::bigint)::text FROM pg_stat_activity WHERE pid=pg_backend_pid())
 THEN
  RAISE EXCEPTION 'social witness requires actual transaction' USING ERRCODE='23514';
 END IF;
 SELECT s.binding,t.identity INTO bound,tracked
  FROM public.concert_product_owner_scopes s
  JOIN public.concert_product_owner_transactions t USING(instance_id,scope_id,binding_digest)
  WHERE s.instance_id=NEW.producer_instance AND s.scope_id=NEW.scope_id
   AND s.binding_digest=NEW.guard_binding_digest AND s.phase='ACTIVE'
   AND s.delivery_secret_hash IS NOT NULL AND t.identity_digest=NEW.identity_digest
   AND t.confirmed_at IS NOT NULL
  FOR SHARE OF s;
 IF bound IS NULL OR tracked IS NULL OR tracked<>NEW.native_identity THEN
  RAISE EXCEPTION 'social witness requires published confirmed scope' USING ERRCODE='23514';
 END IF;
 SELECT jsonb_agg(get_byte(NEW.parent_digest,n) ORDER BY n) INTO parent_bytes FROM generate_series(0,31) n;
 SELECT jsonb_agg(get_byte(NEW.body_digest,n) ORDER BY n) INTO body_bytes FROM generate_series(0,31) n;
 SELECT jsonb_agg(get_byte(NEW.effect_binding_digest,n) ORDER BY n) INTO effect_bytes FROM generate_series(0,31) n;
 SELECT jsonb_agg(get_byte(uuid_send(NEW.request_id),n) ORDER BY n) INTO reference_bytes FROM generate_series(0,15) n;
 IF NOT bound @> jsonb_build_object('actor_id',NEW.actor_id,'event_id',NEW.event_id,
    'session_family_id',NEW.session_family_id,'request_id',NEW.request_id,
    'operation',NEW.operation,'parent_digest',parent_bytes,'body_digest',body_bytes)
  OR NEW.native_identity->'effect_binding_digest' IS DISTINCT FROM effect_bytes
  OR NEW.native_identity->'settlement_reference' IS DISTINCT FROM reference_bytes
  OR clock_timestamp() >= to_timestamp((bound->>'expires_unix_micros')::numeric/1000000)
 THEN
  RAISE EXCEPTION 'social witness scope mismatch' USING ERRCODE='23514';
 END IF;
 -- A mutation witness must see an actual retained aggregate revision written by
 -- this transaction. A successful replay/read is a PROJECTION instead.
 IF NEW.effect_kind='MUTATION' THEN
  IF NEW.operation IN ('setConcertHandle','removeConcertHandle') THEN
   IF NEW.resource_id<>NEW.actor_id THEN
    RAISE EXCEPTION 'social handle witness owner mismatch' USING ERRCODE='23514';
   END IF;
   SELECT revision,xmin::text INTO actual_revision,actual_xmin
    FROM public.concert_social_handles WHERE event_id=NEW.event_id AND user_id=NEW.actor_id
     AND platform=substring(split_part(bound->>'http_target','?',1) from '/(INSTAGRAM|X)$')
     AND revision=NEW.resource_revision
     AND public.concert_social_own_visible_xmin(xmin);
  ELSIF NEW.operation IN ('createConcertSocialRequest','decideConcertSocialRequest',
      'cancelConcertSocialRequest','revokeConcertSocialRequest') THEN
   SELECT revision,xmin::text INTO actual_revision,actual_xmin
    FROM public.concert_social_requests WHERE event_id=NEW.event_id AND request_id=NEW.resource_id
     AND (requester_id=NEW.actor_id OR recipient_id=NEW.actor_id);
  ELSIF NEW.operation IN ('blockConcertUser','unblockConcertUser') THEN
   SELECT revision,xmin::text INTO actual_revision,actual_xmin
    FROM public.concert_relationship_pairs WHERE event_id=NEW.event_id
     AND low_user=least(NEW.actor_id,NEW.resource_id) AND high_user=greatest(NEW.actor_id,NEW.resource_id);
  END IF;
  IF actual_revision IS DISTINCT FROM NEW.resource_revision
   OR NOT coalesce(public.concert_social_own_visible_xmin(actual_xmin::xid),false) THEN
   RAISE EXCEPTION 'social witness requires same transaction revision effect' USING ERRCODE='23514';
  END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_social_effect_witness_guard BEFORE INSERT OR UPDATE OR DELETE
 ON public.concert_social_effect_witnesses FOR EACH ROW
 EXECUTE FUNCTION public.concert_social_effect_witness_guard();

-- A witnessed command must also finish its exact retained receipt successfully.
-- This runs after PgCommandRepository updates the receipt, before COMMIT. An
-- early witness followed by caught/failed/pending work cannot attest success.
CREATE FUNCTION public.concert_social_effect_receipt_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.operation IN ('setConcertHandle','removeConcertHandle','createConcertSocialRequest',
   'decideConcertSocialRequest','cancelConcertSocialRequest','revokeConcertSocialRequest',
   'blockConcertUser','unblockConcertUser') THEN
  IF NEW.idempotency_key IS NULL OR NOT EXISTS(
   SELECT 1 FROM public.concert_command_receipts r WHERE r.actor_id=NEW.actor_id
    AND r.event_id=NEW.event_id AND r.operation_id=NEW.operation
    AND r.idempotency_key=NEW.idempotency_key AND r.state='SUCCEEDED'
    AND r.target_id=NEW.resource_id AND r.resource_id=NEW.resource_id
    AND r.resource_revision=NEW.resource_revision AND r.expires_at>clock_timestamp()) THEN
   RAISE EXCEPTION 'social witness requires exact successful receipt' USING ERRCODE='23514';
  END IF;
 ELSIF NEW.idempotency_key IS NOT NULL OR NEW.effect_kind<>'PROJECTION' THEN
  RAISE EXCEPTION 'social read witness shape invalid' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER concert_social_effect_receipt_guard AFTER INSERT
 ON public.concert_social_effect_witnesses DEFERRABLE INITIALLY DEFERRED
 FOR EACH ROW EXECUTE FUNCTION public.concert_social_effect_receipt_guard();
