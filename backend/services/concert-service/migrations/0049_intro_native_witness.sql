-- BE27 owned prerequisite schema. It is not the global migration ledger.
CREATE TABLE public.concert_intros (
  event_id uuid NOT NULL REFERENCES public.concert_events(event_id),
  intro_id uuid NOT NULL,
  owner_id uuid NOT NULL,
  revision bigint NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),
  state text NOT NULL CHECK(state IN ('DRAFT','PUBLISHED','CLOSED')),
  text text NOT NULL CHECK(char_length(text)<=1000),
  minimum_age smallint,
  maximum_age_exclusive smallint,
  expires_at timestamptz,
  created_at timestamptz NOT NULL,
  updated_at timestamptz NOT NULL,
  PRIMARY KEY(event_id,intro_id),
  UNIQUE(event_id,owner_id),
  CHECK((minimum_age IS NULL)=(maximum_age_exclusive IS NULL)),
  CHECK((minimum_age IS NULL AND maximum_age_exclusive IS NULL)
    OR (minimum_age BETWEEN 14 AND 100 AND maximum_age_exclusive BETWEEN 15 AND 101 AND minimum_age<maximum_age_exclusive)),
  CHECK((state='DRAFT') OR (btrim(text)<>'')),
  CHECK(updated_at>=created_at)
);
CREATE INDEX concert_intros_discovery ON public.concert_intros(event_id,created_at DESC,intro_id DESC) WHERE state='PUBLISHED';

-- The optional policy-edit consent seam for BE28/29 remains a required owner
-- port: it is validated inside the command transaction, not represented by a
-- permissive local fallback.

-- The adapter checks the transaction-scoped authority deadline before projection;
-- a production implementation must retain its owner-source locks through COMMIT.

CREATE FUNCTION public.concert_intro_update_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  IF NEW.event_id IS DISTINCT FROM OLD.event_id
     OR NEW.intro_id IS DISTINCT FROM OLD.intro_id
     OR NEW.owner_id IS DISTINCT FROM OLD.owner_id
     OR NEW.created_at IS DISTINCT FROM OLD.created_at THEN
    RAISE EXCEPTION 'introduction scope is immutable' USING ERRCODE='23514';
  END IF;
  IF OLD.state='CLOSED' THEN
    RAISE EXCEPTION 'closed introduction is terminal' USING ERRCODE='23514';
  END IF;
  IF NEW.revision<>OLD.revision+1 THEN
    RAISE EXCEPTION 'introduction revision must advance once' USING ERRCODE='23514';
  END IF;
  IF OLD.state='PUBLISHED' AND NEW.state='DRAFT' THEN
    RAISE EXCEPTION 'published introduction cannot return to draft' USING ERRCODE='23514';
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER concert_intro_update_guard BEFORE UPDATE ON public.concert_intros
  FOR EACH ROW EXECUTE FUNCTION public.concert_intro_update_guard();

-- BE27 runtime registration. The retained prerequisite schema above is copied
-- byte for byte; its original file remains untouched.
CREATE TABLE public.concert_intro_native_witness (
  witness_id uuid PRIMARY KEY,
  system_identifier text NOT NULL CHECK(system_identifier ~ '^[0-9]+$'),
  database_oid bigint NOT NULL CHECK(database_oid > 0),
  full_xid text NOT NULL CHECK(full_xid ~ '^[0-9]+$'),
  backend_pid integer NOT NULL CHECK(backend_pid > 0),
  backend_started_micros bigint NOT NULL CHECK(backend_started_micros > 0),
  producer_instance uuid NOT NULL,
  effect_digest bytea NOT NULL CHECK(octet_length(effect_digest)=32),
  settlement_reference bytea NOT NULL CHECK(octet_length(settlement_reference) BETWEEN 1 AND 2048),
  authority_until timestamptz NOT NULL,
  scope_id uuid NOT NULL,
  identity_digest bytea NOT NULL CHECK(octet_length(identity_digest)=32),
  native_identity jsonb NOT NULL CHECK(jsonb_typeof(native_identity)='object'),
  FOREIGN KEY(producer_instance,scope_id,identity_digest)
    REFERENCES public.concert_product_owner_transactions(instance_id,scope_id,identity_digest),
  UNIQUE(system_identifier,database_oid,full_xid,effect_digest)
);
CREATE FUNCTION public.concert_intro_witness_native_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE registered jsonb; bound jsonb; effect_bytes jsonb; reference_bytes jsonb;
BEGIN
  IF NEW.system_identifier IS DISTINCT FROM (SELECT system_identifier::text FROM pg_control_system())
    OR NEW.database_oid IS DISTINCT FROM (SELECT oid::bigint FROM pg_database WHERE datname=current_database())
    OR NEW.full_xid IS DISTINCT FROM pg_current_xact_id()::text
    OR NEW.backend_pid IS DISTINCT FROM pg_backend_pid()
    OR NEW.backend_started_micros IS DISTINCT FROM
      (SELECT (extract(epoch FROM backend_start)*1000000)::bigint FROM pg_stat_activity WHERE pid=pg_backend_pid()) THEN
    RAISE EXCEPTION 'intro witness requires actual transaction' USING ERRCODE='23514';
  END IF;
  SELECT t.identity,s.binding INTO registered,bound FROM public.concert_product_owner_scopes s
    JOIN public.concert_product_owner_transactions t USING(instance_id,scope_id,binding_digest)
    WHERE s.instance_id=NEW.producer_instance AND s.scope_id=NEW.scope_id
      AND s.binding_digest=NEW.effect_digest AND s.phase='ACTIVE'
      AND s.delivery_secret_hash IS NOT NULL AND t.identity_digest=NEW.identity_digest
      AND t.confirmed_at IS NOT NULL AND t.auth_registration_id IS NOT NULL
      AND t.friend_registration_id IS NOT NULL;
  -- No lock on the root scope row here: later actual public-profile child
  -- intents must update that root on a separate connection. The confirmed
  -- transaction identity is immutable and retained by the FK above. Root/owner
  -- authority remains the actual registered scope, not this witness lookup.
  SELECT jsonb_agg(get_byte(NEW.effect_digest,n) ORDER BY n) INTO effect_bytes FROM generate_series(0,31) n;
  SELECT jsonb_agg(get_byte(NEW.settlement_reference,n) ORDER BY n) INTO reference_bytes FROM generate_series(0,octet_length(NEW.settlement_reference)-1) n;
  IF registered IS NULL OR registered IS DISTINCT FROM NEW.native_identity
    OR NEW.native_identity->>'producer' IS DISTINCT FROM 'CONCERT'
    OR NEW.native_identity->>'system_identifier' IS DISTINCT FROM NEW.system_identifier
    OR NEW.native_identity->>'database_oid' IS DISTINCT FROM NEW.database_oid::text
    OR NEW.native_identity->>'full_xid' IS DISTINCT FROM NEW.full_xid
    OR NEW.native_identity->>'backend_pid' IS DISTINCT FROM NEW.backend_pid::text
    OR NEW.native_identity->>'backend_started_unix_micros' IS DISTINCT FROM NEW.backend_started_micros::text
    OR NEW.native_identity->>'producer_instance' IS DISTINCT FROM NEW.producer_instance::text
    OR NEW.native_identity->'effect_binding_digest' IS DISTINCT FROM effect_bytes
    OR NEW.native_identity->'settlement_reference' IS DISTINCT FROM reference_bytes
    OR NEW.authority_until > to_timestamp((bound->>'expires_unix_micros')::numeric/1000000)
    OR clock_timestamp() >= NEW.authority_until THEN
    RAISE EXCEPTION 'intro witness requires confirmed native scope' USING ERRCODE='23514';
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER concert_intro_witness_native_guard BEFORE INSERT ON public.concert_intro_native_witness
  FOR EACH ROW EXECUTE FUNCTION public.concert_intro_witness_native_guard();
CREATE FUNCTION public.concert_intro_runtime_commit_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE until_at timestamptz;
BEGIN
  until_at := nullif(current_setting('mcr.intro_authority_until',true),'')::timestamptz;
  IF until_at IS NULL OR clock_timestamp() >= until_at
    OR NOT EXISTS (SELECT 1 FROM public.concert_intro_native_witness
      WHERE full_xid=pg_current_xact_id()::text AND backend_pid=pg_backend_pid()
        AND authority_until>=until_at) THEN
    RAISE EXCEPTION 'intro authority unavailable' USING ERRCODE='23514';
  END IF;
  RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER concert_intro_runtime_commit_guard
  AFTER INSERT OR UPDATE ON public.concert_intros DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION public.concert_intro_runtime_commit_guard();
CREATE CONSTRAINT TRIGGER concert_intro_witness_commit_guard
  AFTER INSERT ON public.concert_intro_native_witness DEFERRABLE INITIALLY DEFERRED
  FOR EACH ROW EXECUTE FUNCTION public.concert_intro_runtime_commit_guard();
CREATE FUNCTION public.concert_intro_witness_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'intro settlement witness is immutable' USING ERRCODE='23514';
END $$;
CREATE TRIGGER concert_intro_witness_immutable BEFORE UPDATE OR DELETE ON public.concert_intro_native_witness
  FOR EACH ROW EXECUTE FUNCTION public.concert_intro_witness_immutable();

CREATE TRIGGER concert_intro_witness_truncate_guard BEFORE TRUNCATE ON public.concert_intro_native_witness
  FOR EACH STATEMENT EXECUTE FUNCTION public.concert_intro_witness_immutable();
