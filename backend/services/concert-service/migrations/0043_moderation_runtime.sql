-- BE39 additive saga ledger. No peer stores, role rows, or existing migrations modified.
CREATE TABLE public.concert_moderation_commands (
 receipt_id uuid PRIMARY KEY REFERENCES public.concert_command_receipts(receipt_id),
 event_id uuid NOT NULL REFERENCES public.concert_events(event_id), actor_id uuid NOT NULL, target_id uuid NOT NULL,
 operation_id text NOT NULL CHECK(operation_id IN ('moderateConcertUser','moderateConcertContent')),
 expected_revision bigint NOT NULL CHECK(expected_revision BETWEEN 1 AND 9007199254740991),
 input jsonb NOT NULL CHECK(jsonb_typeof(input)='object'),
 steps text[] NOT NULL CHECK(cardinality(steps) BETWEEN 1 AND 3),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE TABLE public.concert_moderation_owner_results (
 result_id uuid PRIMARY KEY, receipt_id uuid NOT NULL REFERENCES public.concert_moderation_commands(receipt_id),
 step text NOT NULL CHECK(step IN ('EVENT_RESTRICTION','EVENT_WORLD_TERMINATION','EVENT_PRIVATE_AUTHORITY_REVOCATION','POST_CONTENT','PARTY_CONTENT','GIVEAWAY_CONTENT','INTRO_CONTENT')),
 intent_digest bytea NOT NULL CHECK(octet_length(intent_digest)=32),
 outcome text NOT NULL CHECK(outcome IN ('PENDING','APPLIED','FAILED')),
 owner_receipt_id uuid, owner_revision bigint CHECK(owner_revision BETWEEN 1 AND 9007199254740991),
 native_identity bytea CHECK(octet_length(native_identity) BETWEEN 1 AND 4096),
 failure_reason text CHECK(failure_reason IN ('INVALID_TRANSITION','VERSION_CONFLICT','DEPENDENCY_UNAVAILABLE','INTERNAL_ERROR','RESOURCE_UNAVAILABLE','INVALID_INPUT','EVENT_READ_ONLY')),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), UNIQUE(receipt_id,step,outcome),
 CHECK((outcome='PENDING' AND owner_receipt_id IS NULL AND owner_revision IS NULL AND native_identity IS NULL AND failure_reason IS NULL)
 OR (outcome='APPLIED' AND owner_receipt_id IS NOT NULL AND owner_revision IS NOT NULL AND native_identity IS NOT NULL AND failure_reason IS NULL)
 OR (outcome='FAILED' AND owner_receipt_id IS NULL AND owner_revision IS NULL AND native_identity IS NULL AND failure_reason IS NOT NULL))
);
CREATE UNIQUE INDEX concert_moderation_owner_terminal ON public.concert_moderation_owner_results(receipt_id,step) WHERE outcome IN ('APPLIED','FAILED');
CREATE TABLE public.concert_moderation_audit (
 audit_id uuid PRIMARY KEY, receipt_id uuid NOT NULL REFERENCES public.concert_moderation_commands(receipt_id),
 event_id uuid NOT NULL REFERENCES public.concert_events(event_id),actor_id uuid NOT NULL,target_id uuid NOT NULL,
 operation_id text NOT NULL CHECK(operation_id IN ('moderateConcertUser','moderateConcertContent')),
 outcome text NOT NULL CHECK(outcome IN ('PENDING','SUCCEEDED','FAILED')),created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(receipt_id,outcome)
);
CREATE UNIQUE INDEX concert_moderation_audit_terminal ON public.concert_moderation_audit(receipt_id) WHERE outcome IN ('SUCCEEDED','FAILED');
CREATE INDEX concert_moderation_audit_event_order ON public.concert_moderation_audit(event_id,created_at DESC,audit_id DESC);
CREATE FUNCTION public.concert_moderation_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'moderation ledger is append only' USING ERRCODE='23514'; END; $$;
CREATE TRIGGER concert_moderation_command_immutable BEFORE UPDATE OR DELETE ON public.concert_moderation_commands FOR EACH ROW EXECUTE FUNCTION public.concert_moderation_immutable();
CREATE TRIGGER concert_moderation_owner_immutable BEFORE UPDATE OR DELETE ON public.concert_moderation_owner_results FOR EACH ROW EXECUTE FUNCTION public.concert_moderation_immutable();
CREATE TRIGGER concert_moderation_audit_immutable BEFORE UPDATE OR DELETE ON public.concert_moderation_audit FOR EACH ROW EXECUTE FUNCTION public.concert_moderation_immutable();
CREATE FUNCTION public.concert_moderation_deadline() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE deadline timestamptz;
BEGIN
 deadline:=nullif(current_setting('mcr.moderation_authority_until',true),'')::timestamptz;
 IF deadline IS NULL OR deadline<=clock_timestamp() THEN RAISE EXCEPTION 'moderation authority expired' USING ERRCODE='23514'; END IF;
 RETURN NULL;
END; $$;
CREATE CONSTRAINT TRIGGER concert_moderation_command_deadline AFTER INSERT ON public.concert_moderation_commands DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_moderation_deadline();
CREATE CONSTRAINT TRIGGER concert_moderation_owner_deadline AFTER INSERT ON public.concert_moderation_owner_results DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_moderation_deadline();
CREATE CONSTRAINT TRIGGER concert_moderation_audit_deadline AFTER INSERT ON public.concert_moderation_audit DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_moderation_deadline();
CREATE FUNCTION public.concert_moderation_binding_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE command public.concert_moderation_commands; original public.concert_command_receipts;
BEGIN
 SELECT * INTO original FROM public.concert_command_receipts WHERE receipt_id=NEW.receipt_id FOR UPDATE;
 IF TG_TABLE_NAME='concert_moderation_commands' THEN
  IF original.state<>'PENDING' OR original.actor_id<>NEW.actor_id OR original.event_id<>NEW.event_id OR original.target_id<>NEW.target_id OR original.operation_id<>NEW.operation_id THEN RAISE EXCEPTION 'moderation receipt binding' USING ERRCODE='23514'; END IF;
  IF NEW.input->>'reason' IS NULL OR char_length(btrim(NEW.input->>'reason'))=0 OR char_length(NEW.input->>'reason')>500 OR (NEW.input->>'expected_revision')::bigint IS DISTINCT FROM NEW.expected_revision THEN RAISE EXCEPTION 'moderation command shape' USING ERRCODE='23514'; END IF;
  IF NEW.operation_id='moderateConcertUser' THEN
   IF (NEW.input->>'action'='SUSPEND' AND NEW.steps<>ARRAY['EVENT_RESTRICTION','EVENT_WORLD_TERMINATION','EVENT_PRIVATE_AUTHORITY_REVOCATION']) OR (NEW.input->>'action'='UNSUSPEND' AND NEW.steps<>ARRAY['EVENT_RESTRICTION']) OR coalesce(NEW.input->>'action','') NOT IN ('SUSPEND','UNSUSPEND') THEN RAISE EXCEPTION 'moderation user steps' USING ERRCODE='23514'; END IF;
  ELSE
   IF coalesce(NEW.input->>'action','') NOT IN ('HIDE','RESTORE') OR NEW.input->>'target_id' IS DISTINCT FROM NEW.target_id::text OR NEW.steps<>ARRAY[CASE NEW.input->>'target_type' WHEN 'POST' THEN 'POST_CONTENT' WHEN 'COMMENT' THEN 'POST_CONTENT' WHEN 'PARTY' THEN 'PARTY_CONTENT' WHEN 'GIVEAWAY' THEN 'GIVEAWAY_CONTENT' WHEN 'INTRO' THEN 'INTRO_CONTENT' ELSE 'INVALID' END] THEN RAISE EXCEPTION 'moderation content steps' USING ERRCODE='23514'; END IF;
  END IF;
  RETURN NEW;
 END IF;
 SELECT * INTO command FROM public.concert_moderation_commands WHERE receipt_id=NEW.receipt_id;
 IF TG_TABLE_NAME='concert_moderation_owner_results' THEN
  IF original.state<>'PENDING' OR NOT NEW.step=ANY(command.steps) OR EXISTS(SELECT 1 FROM public.concert_moderation_owner_results WHERE receipt_id=NEW.receipt_id AND step=NEW.step AND outcome IN ('APPLIED','FAILED')) THEN RAISE EXCEPTION 'moderation owner outcome binding' USING ERRCODE='23514'; END IF;
  IF NEW.outcome<>'PENDING' AND NOT EXISTS(SELECT 1 FROM public.concert_moderation_owner_results WHERE receipt_id=NEW.receipt_id AND step=NEW.step AND outcome='PENDING' AND intent_digest=NEW.intent_digest) THEN RAISE EXCEPTION 'moderation intent missing' USING ERRCODE='23514'; END IF;
 ELSE
  IF (NEW.event_id,NEW.actor_id,NEW.target_id,NEW.operation_id)<>(command.event_id,command.actor_id,command.target_id,command.operation_id) THEN RAISE EXCEPTION 'moderation audit binding' USING ERRCODE='23514'; END IF;
  IF NEW.outcome<>original.state THEN RAISE EXCEPTION 'moderation audit result mismatch' USING ERRCODE='23514'; END IF;
  IF NEW.outcome='SUCCEEDED' AND EXISTS(SELECT 1 FROM unnest(command.steps) AS required(step_name) WHERE NOT EXISTS(SELECT 1 FROM public.concert_moderation_owner_results r WHERE r.receipt_id=NEW.receipt_id AND r.step=required.step_name AND r.outcome='APPLIED')) THEN RAISE EXCEPTION 'moderation incomplete owners' USING ERRCODE='23514'; END IF;
  IF NEW.outcome='FAILED' AND NOT EXISTS(SELECT 1 FROM public.concert_moderation_owner_results WHERE receipt_id=NEW.receipt_id AND outcome='FAILED') THEN RAISE EXCEPTION 'moderation failed owner missing' USING ERRCODE='23514'; END IF;
 END IF;
 RETURN NEW;
END; $$;
CREATE TRIGGER concert_moderation_command_binding BEFORE INSERT ON public.concert_moderation_commands FOR EACH ROW EXECUTE FUNCTION public.concert_moderation_binding_guard();
CREATE TRIGGER concert_moderation_owner_binding BEFORE INSERT ON public.concert_moderation_owner_results FOR EACH ROW EXECUTE FUNCTION public.concert_moderation_binding_guard();
CREATE TRIGGER concert_moderation_audit_binding BEFORE INSERT ON public.concert_moderation_audit FOR EACH ROW EXECUTE FUNCTION public.concert_moderation_binding_guard();
