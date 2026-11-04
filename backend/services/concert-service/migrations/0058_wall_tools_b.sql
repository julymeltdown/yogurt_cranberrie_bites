-- Wall B; native witness adapted from0049 at7d5d91bd. No prior migration is changed.
-- Provision explicitly; absence disables B. Index0 assigns a role without recoloring.
CREATE TABLE public.concert_mural_tool_config (
 event_id uuid PRIMARY KEY REFERENCES public.concert_mural_state(event_id),
 background_index smallint NOT NULL CHECK(background_index=0)
);
CREATE FUNCTION public.concert_mural_tool_config_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'immutable Wall background role' USING ERRCODE='23514'; END $$;
CREATE TRIGGER concert_mural_tool_config_immutable BEFORE UPDATE ON public.concert_mural_tool_config
 FOR EACH ROW EXECUTE FUNCTION public.concert_mural_tool_config_immutable();
CREATE TABLE public.concert_mural_tool_correlations (
 event_id uuid NOT NULL REFERENCES public.concert_mural_state(event_id), actor_id uuid NOT NULL,
 operation_id text NOT NULL CHECK(operation_id IN ('placeConcertPixel','eraseConcertMuralPixel','fillConcertMuralRegion','undoConcertMuralAction')),
 idempotency_key uuid NOT NULL, command jsonb NOT NULL CHECK(jsonb_typeof(command)='object' AND octet_length(command::text)<=1024),
 receipt_id uuid REFERENCES public.concert_command_receipts(receipt_id),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(event_id,actor_id,operation_id,idempotency_key), UNIQUE(event_id,actor_id,idempotency_key)
);
CREATE TABLE public.concert_mural_tool_actions (
 action_id uuid PRIMARY KEY, event_id uuid NOT NULL REFERENCES public.concert_mural_state(event_id),
 actor_id uuid NOT NULL, operation_id text NOT NULL, idempotency_key uuid NOT NULL,
 receipt_id uuid UNIQUE REFERENCES public.concert_command_receipts(receipt_id),
 kind text NOT NULL CHECK(kind IN ('PENCIL','ERASE','FILL','UNDO')),
 revision bigint NOT NULL CHECK(revision BETWEEN 2 AND 9007199254740991),
 command jsonb NOT NULL CHECK(jsonb_typeof(command)='object' AND octet_length(command::text)<=1024),
 cells jsonb NOT NULL CHECK(jsonb_typeof(cells)='array' AND jsonb_array_length(cells) BETWEEN 1 AND 256),
 changed_cells integer NOT NULL CHECK(changed_cells BETWEEN 1 AND 256 AND changed_cells=jsonb_array_length(cells)),
 committed_at timestamptz NOT NULL, next_allowed_at timestamptz NOT NULL CHECK(next_allowed_at>=committed_at),
 undo_expires_at timestamptz NOT NULL CHECK(undo_expires_at>committed_at AND undo_expires_at<=committed_at+interval '600 seconds'),
 reset_required boolean NOT NULL, undoes uuid UNIQUE REFERENCES public.concert_mural_tool_actions(action_id),
 UNIQUE(event_id,revision), UNIQUE(event_id,actor_id,operation_id,idempotency_key),
 FOREIGN KEY(event_id,actor_id,operation_id,idempotency_key)
 REFERENCES public.concert_mural_tool_correlations(event_id,actor_id,operation_id,idempotency_key),
 CHECK((kind='UNDO')=(undoes IS NOT NULL)), CHECK((kind='PENCIL')=(NOT reset_required)),
 CHECK(operation_id=CASE kind WHEN 'PENCIL' THEN 'placeConcertPixel' WHEN 'ERASE' THEN 'eraseConcertMuralPixel' WHEN 'FILL' THEN 'fillConcertMuralRegion' WHEN 'UNDO' THEN 'undoConcertMuralAction' END)
);
CREATE INDEX concert_mural_tool_own_history ON public.concert_mural_tool_actions(event_id,actor_id,revision DESC);
-- This durable allowance ledger intentionally has NO receipt/history FK. Retirement
-- of lookup/history data cannot replenish the rolling budget.
CREATE TABLE public.concert_mural_tool_charges (
 action_id uuid PRIMARY KEY, event_id uuid NOT NULL REFERENCES public.concert_mural_state(event_id),
 actor_id uuid NOT NULL, committed_at timestamptz NOT NULL,
 changed_cells integer NOT NULL CHECK(changed_cells BETWEEN 1 AND 256)
);
CREATE INDEX concert_mural_tool_window ON public.concert_mural_tool_charges(event_id,actor_id,committed_at);
CREATE FUNCTION public.concert_mural_tool_charge_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='UPDATE' OR OLD.committed_at>clock_timestamp()-interval '600 seconds' THEN
  RAISE EXCEPTION 'retained Wall cell charge is immutable' USING ERRCODE='23514';
 END IF;
 RETURN OLD;
END $$;
CREATE TRIGGER concert_mural_tool_charge_guard BEFORE UPDATE OR DELETE ON public.concert_mural_tool_charges
 FOR EACH ROW EXECUTE FUNCTION public.concert_mural_tool_charge_guard();
CREATE FUNCTION public.concert_mural_tool_action_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF OLD.receipt_id IS NOT NULL OR NEW.receipt_id IS NULL
   OR (to_jsonb(NEW)-'receipt_id'-'undo_expires_at') IS DISTINCT FROM (to_jsonb(OLD)-'receipt_id'-'undo_expires_at')
   OR NEW.undo_expires_at>OLD.undo_expires_at THEN
  RAISE EXCEPTION 'immutable Wall action audit' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_mural_tool_action_immutable BEFORE UPDATE ON public.concert_mural_tool_actions
 FOR EACH ROW EXECUTE FUNCTION public.concert_mural_tool_action_immutable();
ALTER TABLE public.concert_command_receipts DROP CONSTRAINT concert_command_receipts_failure_reason_check;
ALTER TABLE public.concert_command_receipts ADD CONSTRAINT concert_command_receipts_failure_reason_check
 CHECK(failure_reason IN ('INVALID_TRANSITION','VERSION_CONFLICT','DEPENDENCY_UNAVAILABLE','INTERNAL_ERROR',
 'PARTY_FULL','DUPLICATE_APPLICATION','GIVEAWAY_SOLD_OUT','DUPLICATE_ACTIVE_RESERVATION','RESERVATION_EXPIRED',
 'PICKUP_CODE_INVALID','RESOURCE_UNAVAILABLE','INVALID_INPUT','EVENT_READ_ONLY','MAP_VERSION_MISMATCH',
 'MURAL_PAUSED','MURAL_FROZEN','MURAL_COOLDOWN','MURAL_REGION_LOCKED','MURAL_NO_CHANGE','MURAL_FILL_TOO_LARGE',
 'MURAL_BUDGET_EXCEEDED','MURAL_UNDO_EXPIRED','MURAL_UNDO_NOT_LATEST','MURAL_UNDO_UNAVAILABLE',
 'MURAL_TOOL_UNAVAILABLE','MURAL_ACTION_UNRESOLVED'));
CREATE TABLE public.concert_mural_tools_b_native_witness (
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
CREATE FUNCTION public.concert_mural_tools_b_witness_native_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE registered jsonb; bound jsonb; effect_bytes jsonb; reference_bytes jsonb;
BEGIN
  IF NEW.system_identifier IS DISTINCT FROM (SELECT system_identifier::text FROM pg_control_system())
    OR NEW.database_oid IS DISTINCT FROM (SELECT oid::bigint FROM pg_database WHERE datname=current_database())
    OR NEW.full_xid IS DISTINCT FROM pg_current_xact_id()::text
    OR NEW.backend_pid IS DISTINCT FROM pg_backend_pid()
    OR NEW.backend_started_micros IS DISTINCT FROM
      (SELECT (extract(epoch FROM backend_start)*1000000)::bigint FROM pg_stat_activity WHERE pid=pg_backend_pid()) THEN
    RAISE EXCEPTION 'mural_tools_b witness requires actual transaction' USING ERRCODE='23514';
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
    RAISE EXCEPTION 'mural_tools_b witness requires confirmed native scope' USING ERRCODE='23514';
  END IF;
  RETURN NEW;
END $$;
CREATE TRIGGER concert_mural_tools_b_witness_native_guard BEFORE INSERT ON public.concert_mural_tools_b_native_witness
  FOR EACH ROW EXECUTE FUNCTION public.concert_mural_tools_b_witness_native_guard();
CREATE FUNCTION public.concert_mural_tools_b_runtime_commit_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE until_at timestamptz;
BEGIN
  until_at := nullif(current_setting('mcr.mural_tools_b_authority_until',true),'')::timestamptz;
  IF until_at IS NULL OR clock_timestamp() >= until_at
    OR NOT EXISTS (SELECT 1 FROM public.concert_mural_tools_b_native_witness
      WHERE full_xid=pg_current_xact_id()::text AND backend_pid=pg_backend_pid()
        AND authority_until>=until_at) THEN
    RAISE EXCEPTION 'mural_tools_b authority unavailable' USING ERRCODE='23514';
  END IF;
  RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER concert_mural_tools_b_witness_commit_guard AFTER INSERT ON public.concert_mural_tools_b_native_witness DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_mural_tools_b_runtime_commit_guard();
CREATE FUNCTION public.concert_mural_tools_b_witness_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
  RAISE EXCEPTION 'mural_tools_b settlement witness is immutable' USING ERRCODE='23514';
END $$;
CREATE TRIGGER concert_mural_tools_b_witness_immutable BEFORE UPDATE OR DELETE ON public.concert_mural_tools_b_native_witness
  FOR EACH ROW EXECUTE FUNCTION public.concert_mural_tools_b_witness_immutable();

CREATE TRIGGER concert_mural_tools_b_witness_truncate_guard BEFORE TRUNCATE ON public.concert_mural_tools_b_native_witness
  FOR EACH STATEMENT EXECUTE FUNCTION public.concert_mural_tools_b_witness_immutable();

-- Deferred binding means no successful B/pencil action can commit without its
-- actual receipt, quota charge, private immutable audit and canonical witness.
CREATE FUNCTION public.concert_mural_tool_action_commit_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE a public.concert_mural_tool_actions; matched boolean;
BEGIN
 SELECT * INTO a FROM public.concert_mural_tool_actions WHERE action_id=NEW.action_id;
 IF a.receipt_id IS NULL OR NOT EXISTS(SELECT 1 FROM public.concert_command_receipts r
   WHERE r.receipt_id=a.receipt_id AND r.actor_id=a.actor_id AND r.event_id=a.event_id
   AND r.operation_id=a.operation_id AND r.idempotency_key=a.idempotency_key
   AND r.state='SUCCEEDED' AND r.resource_id=a.event_id AND r.resource_revision=a.revision
   AND r.result_status=200 AND r.failure_reason IS NULL AND a.undo_expires_at<=r.expires_at)
 OR NOT EXISTS(SELECT 1 FROM public.concert_mural_tool_charges q WHERE q.action_id=a.action_id
   AND q.event_id=a.event_id AND q.actor_id=a.actor_id AND q.committed_at=a.committed_at
   AND q.changed_cells=a.changed_cells)
 OR a.command->>'operation_id' IS DISTINCT FROM a.operation_id
 OR (a.command->'body'->>'expected_revision')::bigint<>a.revision-1 THEN
  RAISE EXCEPTION 'unbound Wall action audit' USING ERRCODE='23514';
 END IF;
 SELECT count(DISTINCT (v->>'index')::integer)=a.changed_cells AND
   bool_and(coalesce(jsonb_typeof(v)='object' AND v ?& ARRAY['index','before','after']
     AND (v-'index'-'before'-'after')='{}'::jsonb
     AND jsonb_typeof(v->'index')='number' AND jsonb_typeof(v->'before')='number' AND jsonb_typeof(v->'after')='number'
     AND (v->>'index')::integer BETWEEN 0 AND 16383 AND (v->>'before')::integer BETWEEN 0 AND 15
     AND (v->>'after')::integer BETWEEN 0 AND 15 AND v->>'before'<>v->>'after',false))
   INTO matched FROM jsonb_array_elements(a.cells) v;
 IF matched IS DISTINCT FROM true THEN RAISE EXCEPTION 'invalid Wall history' USING ERRCODE='23514'; END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER concert_mural_tool_action_binding AFTER INSERT OR UPDATE ON public.concert_mural_tool_actions
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_mural_tool_action_commit_guard();
CREATE CONSTRAINT TRIGGER concert_mural_tool_action_native AFTER INSERT OR UPDATE ON public.concert_mural_tool_actions
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_mural_tools_b_runtime_commit_guard();

-- TRUNCATE cannot bypass an active actor allowance.
CREATE FUNCTION public.concert_mural_tool_charge_truncate_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF EXISTS(SELECT 1 FROM public.concert_mural_tool_charges WHERE committed_at>clock_timestamp()-interval '600 seconds') THEN
  RAISE EXCEPTION 'retained Wall cell charges cannot be truncated' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END $$;
CREATE TRIGGER concert_mural_tool_charge_truncate_guard BEFORE TRUNCATE ON public.concert_mural_tool_charges
 FOR EACH STATEMENT EXECUTE FUNCTION public.concert_mural_tool_charge_truncate_guard();
