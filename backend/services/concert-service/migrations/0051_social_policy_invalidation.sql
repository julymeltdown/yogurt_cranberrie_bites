-- Metadata only. A generation invalidates private projections; it grants nothing.
-- Atomic with the retained successful receipt/outbox. No trigger reads handles.
CREATE TABLE public.concert_social_policy_generations (
 event_id uuid NOT NULL REFERENCES public.concert_events ON DELETE CASCADE,
 actor_id uuid NOT NULL CHECK(actor_id<>'00000000-0000-0000-0000-000000000000'),
 revision bigint NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),
 PRIMARY KEY(event_id,actor_id)
);
CREATE TABLE public.concert_social_policy_signals (
 signal_id uuid PRIMARY KEY,
 event_id uuid NOT NULL,
 actor_id uuid NOT NULL,
 policy_revision bigint NOT NULL,
 reason text NOT NULL CHECK(reason IN ('SOURCE_CHANGED','CONSENT_CHANGED','BLOCK_CHANGED','EXPIRED','CANONICAL_POLICY_CHANGED')),
 source_outbox uuid,
 source_receipt uuid,
 source_operation text,
 source_resource uuid,
 source_revision bigint,
 deadline_request uuid,
 deadline_at timestamptz,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 published_at timestamptz,
 FOREIGN KEY(event_id,actor_id) REFERENCES public.concert_social_policy_generations ON DELETE CASCADE,
 UNIQUE(event_id,actor_id,policy_revision),
 UNIQUE(source_outbox,actor_id),
 UNIQUE(event_id,deadline_request,deadline_at,actor_id),
 CHECK((source_outbox IS NOT NULL AND source_receipt IS NOT NULL AND source_operation IS NOT NULL
   AND source_resource IS NOT NULL AND source_revision IS NOT NULL AND deadline_request IS NULL AND deadline_at IS NULL)
  OR (source_outbox IS NULL AND source_receipt IS NULL AND source_operation IS NULL AND source_resource IS NULL
   AND source_revision IS NULL AND deadline_request IS NOT NULL AND deadline_at IS NOT NULL AND reason='EXPIRED'))
);
CREATE INDEX concert_social_policy_pending ON public.concert_social_policy_signals(created_at,signal_id) WHERE published_at IS NULL;
-- Keep source snapshots after ordinary outbox pruning. They contain no ciphertext,
-- handle, credential or body; original queue UUIDs never enter the public frame.
CREATE FUNCTION public.concert_social_policy_from_receipt() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE o public.concert_outbox%ROWTYPE; audience uuid[]; who uuid; why text; gen bigint;
BEGIN
 IF NEW.operation_id NOT IN ('setConcertHandle','removeConcertHandle','createConcertSocialRequest',
  'decideConcertSocialRequest','cancelConcertSocialRequest','revokeConcertSocialRequest',
  'setConcertIntro','publishConcertIntro','closeConcertIntro','blockConcertUser','unblockConcertUser') THEN RETURN NEW; END IF;
 IF NEW.result_status<>200 OR NEW.resource_id IS DISTINCT FROM NEW.target_id OR NEW.resource_revision IS NULL THEN
  RAISE EXCEPTION 'unbound social policy outcome';
 END IF;
 SELECT * INTO STRICT o FROM public.concert_outbox q WHERE q.receipt_id=NEW.receipt_id
  AND q.event_id=NEW.event_id AND q.resource_id=NEW.target_id AND q.resource_revision=NEW.resource_revision
  AND q.kind='RESOURCE_INVALIDATED' AND q.state='READY'
  AND q.xmin::text=mod(pg_current_xact_id()::text::numeric,4294967296)::text;
 IF NEW.operation_id IN ('blockConcertUser','unblockConcertUser') THEN
  audience:=ARRAY[NEW.actor_id,NEW.target_id]; why:='BLOCK_CHANGED';
 ELSIF NEW.operation_id IN ('setConcertHandle','removeConcertHandle') THEN
  SELECT array_agg(x ORDER BY x) INTO audience FROM (
   SELECT DISTINCT x FROM (
    SELECT NEW.actor_id AS x UNION ALL
    SELECT requester_id FROM public.concert_social_requests WHERE event_id=NEW.event_id AND recipient_id=NEW.actor_id AND (state IN ('PENDING','APPROVED') OR xmin::text=mod(pg_current_xact_id()::text::numeric,4294967296)::text) UNION ALL
    SELECT recipient_id FROM public.concert_social_requests WHERE event_id=NEW.event_id AND requester_id=NEW.actor_id AND (state IN ('PENDING','APPROVED') OR xmin::text=mod(pg_current_xact_id()::text::numeric,4294967296)::text)
   ) users LIMIT 4097) bounded;
  why:='SOURCE_CHANGED';
 ELSIF NEW.operation_id IN ('setConcertIntro','publishConcertIntro','closeConcertIntro') THEN
  SELECT array_agg(x ORDER BY x) INTO audience FROM (
   SELECT DISTINCT x FROM (
    SELECT NEW.actor_id AS x UNION ALL
    SELECT requester_id FROM public.concert_social_requests WHERE event_id=NEW.event_id AND intro_id=NEW.target_id AND recipient_id=NEW.actor_id AND (state IN ('PENDING','APPROVED') OR xmin::text=mod(pg_current_xact_id()::text::numeric,4294967296)::text)
   ) users LIMIT 4097) bounded;
  why:='SOURCE_CHANGED';
 ELSE
  SELECT ARRAY[requester_id,recipient_id] INTO STRICT audience FROM public.concert_social_requests
   WHERE event_id=NEW.event_id AND request_id=NEW.target_id AND NEW.actor_id IN (requester_id,recipient_id)
    AND revision=NEW.resource_revision;
  why:='CONSENT_CHANGED';
 END IF;
 IF cardinality(audience)>4096 THEN RAISE EXCEPTION 'social policy audience capacity'; END IF;
 FOR who IN SELECT DISTINCT x FROM unnest(audience) x ORDER BY x LOOP
  INSERT INTO public.concert_social_policy_generations AS g(event_id,actor_id,revision) VALUES(NEW.event_id,who,2)
   ON CONFLICT(event_id,actor_id) DO UPDATE SET revision=g.revision+1 RETURNING revision INTO gen;
  INSERT INTO public.concert_social_policy_signals(signal_id,event_id,actor_id,policy_revision,reason,
   source_outbox,source_receipt,source_operation,source_resource,source_revision)
   VALUES(gen_random_uuid(),NEW.event_id,who,gen,why,o.outbox_id,NEW.receipt_id,NEW.operation_id,NEW.target_id,NEW.resource_revision);
 END LOOP;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_social_policy_success AFTER UPDATE OF state ON public.concert_command_receipts
 FOR EACH ROW WHEN (NEW.state='SUCCEEDED' AND OLD.state IS DISTINCT FROM NEW.state)
 EXECUTE FUNCTION public.concert_social_policy_from_receipt();
REVOKE ALL ON FUNCTION public.concert_social_policy_from_receipt() FROM PUBLIC;
