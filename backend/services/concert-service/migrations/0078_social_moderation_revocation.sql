-- SOCIAL-only leaf of moderateConcertUser/SUSPEND. Parent BE15 remains PENDING.
-- This is neither ordinary successful-outbox authority nor an Auth deletion.
CREATE TABLE public.concert_social_moderation_revocations (
 owner_receipt uuid PRIMARY KEY,
 command_receipt uuid NOT NULL UNIQUE REFERENCES public.concert_moderation_commands(receipt_id),
 event_id uuid NOT NULL, actor_id uuid NOT NULL, target_id uuid NOT NULL,
 expected_revision bigint NOT NULL CHECK(expected_revision BETWEEN 1 AND 9007199254740991),
 intent_digest bytea NOT NULL CHECK(octet_length(intent_digest)=32),
 source_revision bigint NOT NULL CHECK(source_revision BETWEEN 2 AND 9007199254740991),
 restriction_result uuid NOT NULL REFERENCES public.concert_moderation_owner_results(result_id),
 world_result uuid NOT NULL REFERENCES public.concert_moderation_owner_results(result_id),
 restriction_receipt uuid NOT NULL, world_receipt uuid NOT NULL,
 restriction_native bytea NOT NULL, world_native bytea NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), completed_at timestamptz,
 UNIQUE(owner_receipt,event_id)
);
CREATE TABLE public.concert_social_moderation_claims (
 claim_id uuid PRIMARY KEY,
 owner_receipt uuid NOT NULL REFERENCES public.concert_social_moderation_revocations(owner_receipt),
 scope_id uuid NOT NULL, producer_instance uuid NOT NULL, request_id uuid NOT NULL,
 root_binding_digest bytea NOT NULL CHECK(octet_length(root_binding_digest)=32),
 authority_until timestamptz NOT NULL,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(owner_receipt,producer_instance,scope_id), CHECK(authority_until>created_at)
);
CREATE TABLE public.concert_social_moderation_audience (
 owner_receipt uuid NOT NULL, event_id uuid NOT NULL, actor_id uuid NOT NULL,
 policy_revision bigint NOT NULL CHECK(policy_revision BETWEEN 2 AND 9007199254740991),
 published_at timestamptz,
 PRIMARY KEY(owner_receipt,actor_id),
 FOREIGN KEY(owner_receipt,event_id) REFERENCES public.concert_social_moderation_revocations(owner_receipt,event_id),
 FOREIGN KEY(event_id,actor_id) REFERENCES public.concert_social_policy_generations(event_id,actor_id)
);
CREATE TABLE public.concert_social_moderation_witnesses (
 producer_instance uuid NOT NULL, scope_id uuid NOT NULL,
 identity_digest bytea NOT NULL CHECK(octet_length(identity_digest)=32),
 claim_id uuid NOT NULL REFERENCES public.concert_social_moderation_claims(claim_id),
 owner_receipt uuid NOT NULL REFERENCES public.concert_social_moderation_revocations(owner_receipt),
 phase text NOT NULL CHECK(phase IN('PREPARE','PUBLISH','COMPLETE')),
 audience_actor uuid,
 native_identity jsonb NOT NULL,
 PRIMARY KEY(producer_instance,scope_id,identity_digest),
 CHECK((phase='PUBLISH')=(audience_actor IS NOT NULL))
);
CREATE FUNCTION public.concert_social_moderation_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE j public.concert_social_moderation_revocations; c public.concert_moderation_commands;
 r public.concert_command_receipts; until_at timestamptz; bound jsonb; tracked jsonb;
 claim public.concert_social_moderation_claims; effect_bytes jsonb;
BEGIN
 IF TG_OP IN('DELETE','TRUNCATE') THEN RAISE EXCEPTION 'Social moderation evidence is retained' USING ERRCODE='23514'; END IF;
 SELECT * INTO STRICT j FROM public.concert_social_moderation_revocations WHERE owner_receipt=NEW.owner_receipt;
 SELECT * INTO STRICT c FROM public.concert_moderation_commands WHERE receipt_id=j.command_receipt;
 SELECT * INTO STRICT r FROM public.concert_command_receipts WHERE receipt_id=j.command_receipt;
 until_at:=nullif(current_setting('mcr.moderation_authority_until',true),'')::timestamptz;
 IF until_at IS NULL OR until_at<=clock_timestamp() OR r.expires_at<=clock_timestamp()
  OR r.state<>'PENDING' OR c.operation_id<>'moderateConcertUser' OR c.input->>'action'<>'SUSPEND'
  OR (j.event_id,j.actor_id,j.target_id,j.expected_revision) IS DISTINCT FROM(c.event_id,c.actor_id,c.target_id,c.expected_revision)
  OR (r.event_id,r.actor_id,r.target_id,r.operation_id) IS DISTINCT FROM(c.event_id,c.actor_id,c.target_id,c.operation_id)
  OR NOT EXISTS(SELECT 1 FROM public.concert_moderation_owner_results WHERE receipt_id=j.command_receipt
   AND step='EVENT_PRIVATE_AUTHORITY_REVOCATION' AND outcome='PENDING' AND intent_digest=j.intent_digest)
  OR NOT EXISTS(SELECT 1 FROM public.concert_event_admissions WHERE event_id=j.event_id AND user_id=j.target_id
   AND state='SUSPENDED' AND restriction_revision=j.source_revision)
  OR NOT EXISTS(SELECT 1 FROM public.concert_moderation_owner_results WHERE result_id=j.restriction_result
   AND receipt_id=j.command_receipt AND step='EVENT_RESTRICTION' AND outcome='APPLIED'
   AND owner_receipt_id=j.restriction_receipt AND owner_revision=j.source_revision AND native_identity=j.restriction_native)
  OR NOT EXISTS(SELECT 1 FROM public.concert_moderation_owner_results WHERE result_id=j.world_result
   AND receipt_id=j.command_receipt AND step='EVENT_WORLD_TERMINATION' AND outcome='APPLIED'
   AND owner_receipt_id=j.world_receipt AND owner_revision=j.source_revision AND native_identity=j.world_native)
  OR EXISTS(SELECT 1 FROM (VALUES(j.restriction_receipt,'EVENT_RESTRICTION'),(j.world_receipt,'EVENT_WORLD_TERMINATION')) x(id,step)
   WHERE NOT EXISTS(SELECT 1 FROM public.concert_moderation_public_owner_receipts p
    JOIN public.concert_moderation_public_owner_witnesses w ON w.owner_receipt=p.owner_receipt
     AND w.identity_digest=p.completion_identity_digest AND w.identity=p.completion_identity AND w.phase='COMPLETE'
    WHERE p.owner_receipt=x.id AND p.step=x.step AND p.command_receipt=j.command_receipt
     AND (p.event_id,p.actor_id,p.target_id,p.expected_revision,p.source_revision)=(j.event_id,j.actor_id,j.target_id,j.expected_revision,j.source_revision)
     AND p.completed_at IS NOT NULL))
 THEN RAISE EXCEPTION 'Social moderation source or authority unavailable' USING ERRCODE='23514'; END IF;
 IF TG_TABLE_NAME='concert_social_moderation_revocations' AND TG_OP='INSERT' AND NEW.completed_at IS NOT NULL THEN
  RAISE EXCEPTION 'Social moderation must start incomplete' USING ERRCODE='23514';
 END IF;
 IF TG_TABLE_NAME='concert_social_moderation_witnesses' THEN
  SELECT * INTO STRICT claim FROM public.concert_social_moderation_claims WHERE claim_id=NEW.claim_id;
  SELECT s.binding,t.identity INTO bound,tracked FROM public.concert_product_owner_scopes s
   JOIN public.concert_product_owner_transactions t USING(instance_id,scope_id,binding_digest)
   WHERE s.instance_id=NEW.producer_instance AND s.scope_id=NEW.scope_id AND t.identity_digest=NEW.identity_digest
    AND s.binding_digest=claim.root_binding_digest AND s.phase='ACTIVE' AND s.delivery_secret_hash IS NOT NULL
    AND t.confirmed_at IS NOT NULL FOR SHARE OF s,t;
  SELECT jsonb_agg(get_byte(j.intent_digest,n) ORDER BY n) INTO effect_bytes FROM generate_series(0,31) n;
  IF bound IS NULL OR tracked IS DISTINCT FROM NEW.native_identity
   OR claim.owner_receipt<>j.owner_receipt OR claim.scope_id<>NEW.scope_id OR claim.producer_instance<>NEW.producer_instance
   OR claim.authority_until<=clock_timestamp() OR claim.request_id::text IS DISTINCT FROM bound->>'request_id'
   OR bound->>'actor_id' IS DISTINCT FROM j.actor_id::text OR bound->>'event_id' IS DISTINCT FROM j.event_id::text
   OR bound->>'operation' IS DISTINCT FROM 'moderateConcertUser' OR bound->>'http_method' IS DISTINCT FROM 'POST'
   OR bound->>'parent_method' IS DISTINCT FROM '/concert.product.v1.ProductService/Dispatch'
   OR bound->>'http_target' IS DISTINCT FROM '/api/concert/events/'||j.event_id::text||'/admin/users/'||j.target_id::text||'/moderation'
   OR clock_timestamp()>=to_timestamp((bound->>'expires_unix_micros')::numeric/1000000)
   OR NEW.native_identity->>'producer' IS DISTINCT FROM 'CONCERT'
   OR NEW.native_identity->>'producer_instance' IS DISTINCT FROM NEW.producer_instance::text
   OR NEW.native_identity->>'system_identifier' IS DISTINCT FROM (SELECT system_identifier::text FROM pg_control_system())
   OR NEW.native_identity->>'database_oid' IS DISTINCT FROM (SELECT oid::text FROM pg_database WHERE datname=current_database())
   OR NEW.native_identity->>'full_xid' IS DISTINCT FROM pg_current_xact_id()::text
   OR NEW.native_identity->>'backend_pid' IS DISTINCT FROM pg_backend_pid()::text
   OR NEW.native_identity->>'backend_started_unix_micros' IS DISTINCT FROM (SELECT (extract(epoch FROM backend_start)*1000000)::bigint::text FROM pg_stat_activity WHERE pid=pg_backend_pid())
   OR NEW.native_identity->'effect_binding_digest' IS DISTINCT FROM effect_bytes
   OR (SELECT convert_from(decode(string_agg(lpad(to_hex(v::integer),2,'0') ,'' ORDER BY ord),'hex'),'UTF8')::jsonb
       FROM jsonb_array_elements_text(NEW.native_identity->'settlement_reference') WITH ORDINALITY x(v,ord))
     IS DISTINCT FROM jsonb_build_object('kind','MODERATION_PRIVATE_REVOCATION_V1','owner_receipt',j.owner_receipt,
       'command_receipt',j.command_receipt,'intent_digest',encode(j.intent_digest,'hex'),'source_revision',j.source_revision,
       'scope_id',NEW.scope_id,'request_id',claim.request_id)
   OR (NEW.phase='COMPLETE' AND j.completed_at IS NULL)
   OR (NEW.phase='PUBLISH' AND NOT EXISTS(SELECT 1 FROM public.concert_social_moderation_audience
      WHERE owner_receipt=j.owner_receipt AND actor_id=NEW.audience_actor AND published_at IS NOT NULL))
  THEN RAISE EXCEPTION 'Social moderation requires actual tracked current native effect' USING ERRCODE='23514'; END IF;
 END IF;
 IF TG_OP='INSERT' AND TG_TABLE_NAME<>'concert_social_moderation_witnesses' AND NOT EXISTS(SELECT 1 FROM public.concert_social_moderation_witnesses w WHERE w.owner_receipt=j.owner_receipt AND w.native_identity->>'full_xid'=pg_current_xact_id()::text AND (TG_TABLE_NAME<>'concert_social_moderation_claims' OR w.claim_id=(to_jsonb(NEW)->>'claim_id')::uuid)) THEN
  RAISE EXCEPTION 'Social moderation source requires actual native witness' USING ERRCODE='23514';
 END IF;
 IF TG_TABLE_NAME='concert_social_moderation_revocations' AND TG_OP='UPDATE' THEN
  IF (to_jsonb(NEW)-'completed_at') IS DISTINCT FROM(to_jsonb(OLD)-'completed_at')
   OR OLD.completed_at IS NOT NULL OR NEW.completed_at IS NULL
   OR NOT EXISTS(SELECT 1 FROM public.concert_social_moderation_witnesses WHERE owner_receipt=j.owner_receipt
    AND phase='COMPLETE' AND native_identity->>'full_xid'=pg_current_xact_id()::text)
   OR EXISTS(SELECT 1 FROM public.concert_social_moderation_audience WHERE owner_receipt=j.owner_receipt AND published_at IS NULL)
   OR EXISTS(SELECT 1 FROM public.concert_social_requests WHERE event_id=j.event_id AND j.target_id IN(requester_id,recipient_id) AND state IN('PENDING','APPROVED'))
   OR EXISTS(SELECT 1 FROM public.concert_social_grants WHERE event_id=j.event_id AND j.target_id IN(owner_id,viewer_id) AND state='ACTIVE')
  THEN RAISE EXCEPTION 'Social moderation completion is immutable or incomplete' USING ERRCODE='23514'; END IF;
 ELSIF TG_TABLE_NAME='concert_social_moderation_audience' AND TG_OP='UPDATE' THEN
  IF (to_jsonb(NEW)-'published_at') IS DISTINCT FROM(to_jsonb(OLD)-'published_at') OR OLD.published_at IS NOT NULL OR NEW.published_at IS NULL
   OR NOT EXISTS(SELECT 1 FROM public.concert_social_moderation_witnesses WHERE owner_receipt=j.owner_receipt
    AND phase='PUBLISH' AND audience_actor=NEW.actor_id AND native_identity->>'full_xid'=pg_current_xact_id()::text)
  THEN RAISE EXCEPTION 'Social moderation audience is immutable' USING ERRCODE='23514'; END IF;
 ELSIF TG_OP='UPDATE' THEN
  RAISE EXCEPTION 'Social moderation journal is immutable' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END $$;
-- Deferred validation sees the actual journal inserted in the same transaction.
CREATE CONSTRAINT TRIGGER social_moderation_source AFTER INSERT OR UPDATE ON public.concert_social_moderation_revocations DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_social_moderation_guard();
CREATE CONSTRAINT TRIGGER social_moderation_claim AFTER INSERT OR UPDATE ON public.concert_social_moderation_claims DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_social_moderation_guard();
CREATE CONSTRAINT TRIGGER social_moderation_audience AFTER INSERT OR UPDATE ON public.concert_social_moderation_audience DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_social_moderation_guard();
CREATE CONSTRAINT TRIGGER social_moderation_witness AFTER INSERT OR UPDATE ON public.concert_social_moderation_witnesses DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_social_moderation_guard();
DO $$ DECLARE t text; BEGIN
 FOREACH t IN ARRAY ARRAY['concert_social_moderation_revocations','concert_social_moderation_claims','concert_social_moderation_audience','concert_social_moderation_witnesses'] LOOP
  EXECUTE format('CREATE TRIGGER social_moderation_no_delete BEFORE DELETE ON public.%I FOR EACH ROW EXECUTE FUNCTION public.concert_social_moderation_guard()',t);
  EXECUTE format('CREATE TRIGGER social_moderation_no_truncate BEFORE TRUNCATE ON public.%I FOR EACH STATEMENT EXECUTE FUNCTION public.concert_social_moderation_guard()',t);
 END LOOP;
END $$;
