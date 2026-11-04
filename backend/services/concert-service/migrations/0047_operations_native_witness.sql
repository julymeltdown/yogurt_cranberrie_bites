-- Transaction outcome correlation only. This table never certifies SUCCEEDED
-- command effects, grants a role, or replaces the canonical owner exclusion.
CREATE TABLE public.concert_operations_native_witnesses (
 instance_id uuid NOT NULL,
 scope_id uuid NOT NULL,
 binding_digest bytea NOT NULL CHECK(octet_length(binding_digest)=32),
 identity_digest bytea NOT NULL CHECK(octet_length(identity_digest)=32),
 request_digest bytea NOT NULL CHECK(octet_length(request_digest)=32 AND request_digest<>decode(repeat('00',32),'hex')),
 idempotency_key uuid CHECK(idempotency_key IS NULL OR idempotency_key<>'00000000-0000-0000-0000-000000000000'),
 binding jsonb NOT NULL CHECK(jsonb_typeof(binding)='object'),
 identity jsonb NOT NULL CHECK(jsonb_typeof(identity)='object'),
 settlement_reference bytea NOT NULL CHECK(octet_length(settlement_reference) BETWEEN 1 AND 2048),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(instance_id,scope_id,identity_digest),
 FOREIGN KEY(instance_id,scope_id,binding_digest)
  REFERENCES public.concert_product_owner_scopes(instance_id,scope_id,binding_digest),
 FOREIGN KEY(instance_id,scope_id,identity_digest)
  REFERENCES public.concert_product_owner_transactions(instance_id,scope_id,identity_digest)
);

CREATE FUNCTION public.concert_operations_native_witness_guard()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
 bound jsonb;
 tracked jsonb;
 reference jsonb;
 digest_bytes jsonb;
 reference_bytes jsonb;
BEGIN
 IF TG_OP<>'INSERT' THEN
  RAISE EXCEPTION 'operations native witness is immutable' USING ERRCODE='23514';
 END IF;
 IF NEW.identity->>'producer' IS DISTINCT FROM 'CONCERT'
  OR NEW.identity->>'producer_instance' IS DISTINCT FROM NEW.instance_id::text
  OR NEW.identity->>'full_xid' IS DISTINCT FROM pg_current_xact_id()::text
  OR NEW.identity->>'backend_pid' IS DISTINCT FROM pg_backend_pid()::text
  OR NEW.identity->>'database_oid' IS DISTINCT FROM (SELECT oid::text FROM pg_database WHERE datname=current_database())
  OR NEW.identity->>'system_identifier' IS DISTINCT FROM (SELECT system_identifier::text FROM pg_control_system())
  OR NEW.identity->>'backend_started_unix_micros' IS DISTINCT FROM
     (SELECT (extract(epoch FROM backend_start)*1000000)::bigint::text FROM pg_stat_activity WHERE pid=pg_backend_pid())
 THEN
  RAISE EXCEPTION 'operations witness requires actual native transaction' USING ERRCODE='23514';
 END IF;
 -- This lock serializes the protected transaction with Root seal. Confirmed
 -- Track acknowledgements predate this witness and every subsequent effect.
 SELECT s.binding,t.identity INTO bound,tracked
  FROM public.concert_product_owner_scopes s
  JOIN public.concert_product_owner_transactions t USING(instance_id,scope_id,binding_digest)
  WHERE s.instance_id=NEW.instance_id AND s.scope_id=NEW.scope_id
   AND s.binding_digest=NEW.binding_digest AND s.phase='ACTIVE'
   AND s.delivery_secret_hash IS NOT NULL AND t.identity_digest=NEW.identity_digest
   AND t.confirmed_at IS NOT NULL
  FOR SHARE OF s,t;
 IF bound IS NULL OR tracked IS NULL OR bound<>NEW.binding OR tracked<>NEW.identity THEN
  RAISE EXCEPTION 'operations witness requires exact published confirmed scope' USING ERRCODE='23514';
 END IF;
 SELECT jsonb_agg(get_byte(NEW.request_digest,n) ORDER BY n) INTO digest_bytes FROM generate_series(0,31) n;
 SELECT jsonb_agg(get_byte(NEW.settlement_reference,n) ORDER BY n) INTO reference_bytes
  FROM generate_series(0,octet_length(NEW.settlement_reference)-1) n;
 reference:=convert_from(NEW.settlement_reference,'UTF8')::jsonb;
 IF NEW.binding->>'parent_method' IS DISTINCT FROM '/concert.product.v1.ProductService/Dispatch'
  OR NEW.binding->>'operation' NOT IN (
   'createConcertReport','listOwnConcertReports','listConcertAdminReports','getConcertAdminReport',
   'getConcertSettings','setConcertSettings','setConcertEventPhase','listConcertAdminPlaces','setConcertPlace',
   'listConcertNotices','getConcertNotice','listConcertAdminNotices','createConcertNotice','updateConcertNotice')
  OR NEW.identity->'effect_binding_digest' IS DISTINCT FROM digest_bytes
  OR NEW.identity->'settlement_reference' IS DISTINCT FROM reference_bytes
  OR reference IS DISTINCT FROM jsonb_build_object(
   'kind','OPERATIONS_NATIVE_V1','operation',NEW.binding->>'operation',
   'request_id',NEW.binding->>'request_id','scope_id',NEW.scope_id,
   'request_digest',digest_bytes,'idempotency_key',NEW.idempotency_key)
  OR (NEW.binding->>'http_method'='GET') IS DISTINCT FROM (NEW.idempotency_key IS NULL)
 THEN
  RAISE EXCEPTION 'operations witness full request binding mismatch' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_operations_native_witness_guard BEFORE INSERT OR UPDATE OR DELETE
 ON public.concert_operations_native_witnesses FOR EACH ROW
 EXECUTE FUNCTION public.concert_operations_native_witness_guard();

CREATE FUNCTION public.concert_operations_native_receipt_guard()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 -- Run after final receipt projection, including replay and FAILED/PENDING.
 -- Existence binds correlation, never recasts the actual command outcome.
 IF NEW.idempotency_key IS NOT NULL AND NOT EXISTS(
  SELECT 1 FROM public.concert_command_receipts r
   WHERE r.actor_id=(NEW.binding->>'actor_id')::uuid
    AND r.event_id=(NEW.binding->>'event_id')::uuid
    AND r.operation_id=NEW.binding->>'operation'
    AND r.idempotency_key=NEW.idempotency_key
    AND r.state IN ('PENDING','SUCCEEDED','FAILED')
    AND r.expires_at>clock_timestamp())
 THEN
  RAISE EXCEPTION 'operations command witness requires exact receipt' USING ERRCODE='23514';
 END IF;
 IF clock_timestamp()>=to_timestamp((NEW.binding->>'expires_unix_micros')::numeric/1000000)
 THEN
  RAISE EXCEPTION 'operations witness original deadline expired' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER concert_operations_native_receipt_guard AFTER INSERT
 ON public.concert_operations_native_witnesses DEFERRABLE INITIALLY DEFERRED
 FOR EACH ROW EXECUTE FUNCTION public.concert_operations_native_receipt_guard();
