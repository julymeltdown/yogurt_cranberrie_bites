-- PARTY-NATIVE-SETTLEMENT-01. Native transaction correlation, not business
-- success or authority. Adapted from 0047 at 7d5d91bd; prior SQL is untouched.
CREATE TABLE public.concert_party_native_witnesses (
 instance_id uuid NOT NULL,
 scope_id uuid NOT NULL,
 binding_digest bytea NOT NULL CHECK(octet_length(binding_digest)=32),
 identity_digest bytea NOT NULL CHECK(octet_length(identity_digest)=32),
 request_digest bytea NOT NULL CHECK(octet_length(request_digest)=32 AND request_digest<>decode(repeat('00',32),'hex')),
 idempotency_key uuid CHECK(idempotency_key IS NULL OR idempotency_key<>'00000000-0000-0000-0000-000000000000'),
 native_role text NOT NULL CHECK(native_role IN ('COMMAND','READ','LOOKUP')),
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
CREATE INDEX concert_party_native_current_transaction
 ON public.concert_party_native_witnesses((identity->>'full_xid'),native_role);

CREATE FUNCTION public.concert_party_native_assert(w public.concert_party_native_witnesses)
RETURNS void LANGUAGE plpgsql AS $$
DECLARE
 bound jsonb;
 tracked jsonb;
 reference jsonb;
 digest_bytes jsonb;
 reference_bytes jsonb;
 body_bytes bytea;
 expected_digest bytea;
 http_method text;
 http_target text;
 operation text;
BEGIN
 IF w.identity->>'producer' IS DISTINCT FROM 'CONCERT'
  OR w.identity->>'producer_instance' IS DISTINCT FROM w.instance_id::text
  OR w.identity->>'full_xid' IS DISTINCT FROM pg_current_xact_id()::text
  OR w.identity->>'backend_pid' IS DISTINCT FROM pg_backend_pid()::text
  OR w.identity->>'database_oid' IS DISTINCT FROM (SELECT oid::text FROM pg_database WHERE datname=current_database())
  OR w.identity->>'system_identifier' IS DISTINCT FROM (SELECT system_identifier::text FROM pg_control_system())
  OR w.identity->>'backend_started_unix_micros' IS DISTINCT FROM
     (SELECT (extract(epoch FROM backend_start)*1000000)::bigint::text FROM pg_stat_activity WHERE pid=pg_backend_pid())
 THEN
  RAISE EXCEPTION 'party witness requires actual native transaction' USING ERRCODE='23514';
 END IF;
 -- Held through actual commit. Root seal cannot cross these confirmed rows.
 SELECT s.binding,t.identity INTO bound,tracked
  FROM public.concert_product_owner_scopes s
  JOIN public.concert_product_owner_transactions t USING(instance_id,scope_id,binding_digest)
  WHERE s.instance_id=w.instance_id AND s.scope_id=w.scope_id
   AND s.binding_digest=w.binding_digest AND s.phase='ACTIVE'
   AND s.delivery_secret_hash IS NOT NULL AND t.identity_digest=w.identity_digest
   AND t.confirmed_at IS NOT NULL AND t.auth_registration_id IS NOT NULL
   AND t.friend_registration_id IS NOT NULL FOR SHARE OF s,t;
 IF bound IS NULL OR tracked IS NULL OR bound<>w.binding OR tracked<>w.identity THEN
  RAISE EXCEPTION 'party witness requires exact confirmed published parent' USING ERRCODE='23514';
 END IF;
 SELECT jsonb_agg(get_byte(w.request_digest,n) ORDER BY n) INTO digest_bytes FROM generate_series(0,31) n;
 SELECT jsonb_agg(get_byte(w.settlement_reference,n) ORDER BY n) INTO reference_bytes
  FROM generate_series(0,octet_length(w.settlement_reference)-1) n;
 reference:=convert_from(w.settlement_reference,'UTF8')::jsonb;
 operation:=bound->>'operation';http_method:=bound->>'http_method';http_target:=bound->>'http_target';
 IF bound->>'parent_method' IS DISTINCT FROM '/concert.product.v1.ProductService/Dispatch'
  OR operation NOT IN ('listConcertParties','getConcertParty','createConcertParty','applyConcertParty',
    'decideConcertApplication','cancelConcertApplication','removeConcertMember','cancelConcertParty')
  OR w.identity->'effect_binding_digest' IS DISTINCT FROM digest_bytes
  OR w.identity->'settlement_reference' IS DISTINCT FROM reference_bytes
  OR reference IS DISTINCT FROM jsonb_build_object('kind','PARTY_NATIVE_V1','operation',operation,
     'request_id',bound->>'request_id','scope_id',w.scope_id,'request_digest',digest_bytes,
     'idempotency_key',w.idempotency_key,'role',w.native_role)
  OR (http_method='GET') IS DISTINCT FROM (w.idempotency_key IS NULL)
  OR (w.native_role='READ' AND operation NOT IN ('listConcertParties','getConcertParty'))
  OR (w.native_role='COMMAND' AND operation IN ('listConcertParties','getConcertParty'))
  OR (w.native_role='LOOKUP' AND operation NOT IN ('decideConcertApplication','cancelConcertApplication'))
 THEN
  RAISE EXCEPTION 'party witness full request or role mismatch' USING ERRCODE='23514';
 END IF;
 -- Recompute PartyLiveBinding::digest from the exact persisted canonical parent,
 -- including the admitted HTTP body digest, target and command key. No generic
 -- request-hash assertion can substitute for the protected producer binding.
 SELECT decode(string_agg(lpad(to_hex(value::integer),2,'0') ,'' ORDER BY ord),'hex') INTO body_bytes
  FROM jsonb_array_elements_text(bound->'body_digest') WITH ORDINALITY e(value,ord);
 IF octet_length(body_bytes) IS DISTINCT FROM 32 THEN
  RAISE EXCEPTION 'party body digest mismatch' USING ERRCODE='23514';
 END IF;
 expected_digest:=sha256(convert_to('MCR party-live binding v1','UTF8')||decode('00','hex')
  ||uuid_send((bound->>'actor_id')::uuid)||uuid_send((bound->>'event_id')::uuid)
  ||uuid_send((bound->>'session_family_id')::uuid)||uuid_send((bound->>'request_id')::uuid)
  ||int8send((bound->>'expires_unix_micros')::bigint)
  ||int8send(octet_length(convert_to(http_method,'UTF8'))::bigint)||convert_to(http_method,'UTF8')
  ||int8send(octet_length(convert_to(http_target,'UTF8'))::bigint)||convert_to(http_target,'UTF8')
  ||body_bytes||decode('01','hex')||w.binding_digest
  ||CASE WHEN w.idempotency_key IS NULL THEN decode('00','hex')
    ELSE decode('01','hex')||uuid_send(w.idempotency_key) END);
 IF expected_digest IS DISTINCT FROM w.request_digest THEN
  RAISE EXCEPTION 'party request digest is not exact parent binding' USING ERRCODE='23514';
 END IF;
 IF clock_timestamp()>=to_timestamp((bound->>'expires_unix_micros')::numeric/1000000) THEN
  RAISE EXCEPTION 'party original deadline expired' USING ERRCODE='23514';
 END IF;
END $$;

CREATE FUNCTION public.concert_party_native_witness_guard()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP<>'INSERT' THEN
  RAISE EXCEPTION 'party native witness is immutable' USING ERRCODE='23514';
 END IF;
 PERFORM public.concert_party_native_assert(NEW);
 RETURN NEW;
END $$;
CREATE TRIGGER concert_party_native_witness_guard BEFORE INSERT OR UPDATE OR DELETE
 ON public.concert_party_native_witnesses FOR EACH ROW
 EXECUTE FUNCTION public.concert_party_native_witness_guard();
CREATE TRIGGER concert_party_native_witness_no_truncate BEFORE TRUNCATE
 ON public.concert_party_native_witnesses FOR EACH STATEMENT
 EXECUTE FUNCTION public.concert_party_native_witness_guard();

-- Installed by the deferred trigger after the actual BE15 receipt exists. The
-- receipt ID is retained independently of its later mutable business state.
CREATE TABLE public.concert_party_native_command_receipts (
 instance_id uuid NOT NULL,
 scope_id uuid NOT NULL,
 identity_digest bytea NOT NULL,
 receipt_id uuid NOT NULL REFERENCES public.concert_command_receipts(receipt_id) ON DELETE RESTRICT,
 receipt jsonb NOT NULL CHECK(jsonb_typeof(receipt)='object'),
 PRIMARY KEY(instance_id,scope_id,identity_digest),
 FOREIGN KEY(instance_id,scope_id,identity_digest)
  REFERENCES public.concert_party_native_witnesses(instance_id,scope_id,identity_digest)
);
CREATE FUNCTION public.concert_party_native_command_receipt_guard()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE
 w public.concert_party_native_witnesses;
 actual jsonb;
BEGIN
 IF TG_OP<>'INSERT' THEN
  RAISE EXCEPTION 'party native receipt witness is immutable' USING ERRCODE='23514';
 END IF;
 SELECT * INTO w FROM public.concert_party_native_witnesses
  WHERE instance_id=NEW.instance_id AND scope_id=NEW.scope_id AND identity_digest=NEW.identity_digest;
 IF NOT FOUND OR w.native_role<>'COMMAND' THEN
  RAISE EXCEPTION 'party native receipt requires command witness' USING ERRCODE='23514';
 END IF;
 PERFORM public.concert_party_native_assert(w);
 SELECT to_jsonb(r) INTO actual FROM public.concert_command_receipts r
  WHERE r.receipt_id=NEW.receipt_id AND r.actor_id=(w.binding->>'actor_id')::uuid
   AND r.event_id=(w.binding->>'event_id')::uuid AND r.operation_id=w.binding->>'operation'
   AND r.idempotency_key=w.idempotency_key AND r.state IN ('PENDING','SUCCEEDED','FAILED')
   AND r.expires_at>clock_timestamp() FOR SHARE OF r;
 IF actual IS NULL OR actual IS DISTINCT FROM NEW.receipt THEN
  RAISE EXCEPTION 'party native receipt is not exact actual command receipt' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_party_native_command_receipt_guard BEFORE INSERT OR UPDATE OR DELETE
 ON public.concert_party_native_command_receipts FOR EACH ROW
 EXECUTE FUNCTION public.concert_party_native_command_receipt_guard();
CREATE TRIGGER concert_party_native_command_receipt_no_truncate BEFORE TRUNCATE
 ON public.concert_party_native_command_receipts FOR EACH STATEMENT
 EXECUTE FUNCTION public.concert_party_native_command_receipt_guard();

CREATE FUNCTION public.concert_party_native_receipt_guard()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 -- Recheck the actual current native connection, original deadline and exact
 -- still-held Product parent at COMMIT, not merely at early registration.
 PERFORM public.concert_party_native_assert(NEW);
 IF NEW.native_role='COMMAND' THEN
  INSERT INTO public.concert_party_native_command_receipts(instance_id,scope_id,identity_digest,receipt_id,receipt)
   SELECT NEW.instance_id,NEW.scope_id,NEW.identity_digest,r.receipt_id,to_jsonb(r)
    FROM public.concert_command_receipts r
    WHERE r.actor_id=(NEW.binding->>'actor_id')::uuid
     AND r.event_id=(NEW.binding->>'event_id')::uuid AND r.operation_id=NEW.binding->>'operation'
     AND r.idempotency_key=NEW.idempotency_key
     AND r.state IN ('PENDING','SUCCEEDED','FAILED') AND r.expires_at>clock_timestamp();
  IF NOT FOUND THEN
   RAISE EXCEPTION 'party command witness requires exact actual receipt' USING ERRCODE='23514';
  END IF;
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER concert_party_native_receipt_guard AFTER INSERT
 ON public.concert_party_native_witnesses DEFERRABLE INITIALLY DEFERRED
 FOR EACH ROW EXECUTE FUNCTION public.concert_party_native_receipt_guard();

CREATE FUNCTION public.concert_party_native_no_lookup_effect()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 -- Deferred also catches writes executed BEFORE someone inserts a forged READ
 -- or LOOKUP witness later in this transaction. Existing business producers with
 -- no Party native witness retain their semantics; policy pair reservations and
 -- private child lifecycle bookkeeping are not Party command effects.
 IF EXISTS(SELECT 1 FROM public.concert_party_native_witnesses
   WHERE identity->>'full_xid'=pg_current_xact_id()::text AND native_role IN ('LOOKUP','READ')) THEN
  RAISE EXCEPTION 'party read or lookup cannot commit command effects' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER concert_party_native_lookup_parties AFTER INSERT OR UPDATE OR DELETE
 ON public.concert_community_parties DEFERRABLE INITIALLY DEFERRED
 FOR EACH ROW EXECUTE FUNCTION public.concert_party_native_no_lookup_effect();
CREATE CONSTRAINT TRIGGER concert_party_native_lookup_applications AFTER INSERT OR UPDATE OR DELETE
 ON public.concert_community_applications DEFERRABLE INITIALLY DEFERRED
 FOR EACH ROW EXECUTE FUNCTION public.concert_party_native_no_lookup_effect();
CREATE CONSTRAINT TRIGGER concert_party_native_lookup_members AFTER INSERT OR UPDATE OR DELETE
 ON public.concert_community_members DEFERRABLE INITIALLY DEFERRED
 FOR EACH ROW EXECUTE FUNCTION public.concert_party_native_no_lookup_effect();
CREATE CONSTRAINT TRIGGER concert_party_native_lookup_receipts AFTER INSERT OR UPDATE OR DELETE
 ON public.concert_command_receipts DEFERRABLE INITIALLY DEFERRED
 FOR EACH ROW EXECUTE FUNCTION public.concert_party_native_no_lookup_effect();
CREATE CONSTRAINT TRIGGER concert_party_native_lookup_outbox AFTER INSERT OR UPDATE OR DELETE
 ON public.concert_outbox DEFERRABLE INITIALLY DEFERRED
 FOR EACH ROW EXECUTE FUNCTION public.concert_party_native_no_lookup_effect();
