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
CREATE FUNCTION public.concert_social_policy_from_outbox() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE r public.concert_command_receipts%ROWTYPE; o public.concert_outbox%ROWTYPE; audience uuid[]; who uuid; why text; gen bigint;
BEGIN
 IF NEW.kind<>'RESOURCE_INVALIDATED' OR NEW.receipt_id IS NULL THEN RETURN NEW; END IF;
 SELECT * INTO STRICT r FROM public.concert_command_receipts WHERE receipt_id=NEW.receipt_id;
 IF r.operation_id NOT IN ('setConcertHandle','removeConcertHandle','createConcertSocialRequest',
  'decideConcertSocialRequest','cancelConcertSocialRequest','revokeConcertSocialRequest',
  'setConcertIntro','publishConcertIntro','closeConcertIntro','blockConcertUser','unblockConcertUser') THEN RETURN NEW; END IF;
 IF r.state<>'SUCCEEDED' OR r.result_status<>200 OR r.resource_id IS DISTINCT FROM r.target_id OR r.resource_revision IS NULL THEN
  RAISE EXCEPTION 'unbound social policy outcome';
 END IF;
 SELECT * INTO STRICT o FROM public.concert_outbox q WHERE q.outbox_id=NEW.outbox_id AND q.receipt_id=r.receipt_id
  AND q.event_id=r.event_id AND q.resource_id=r.target_id AND q.resource_revision=r.resource_revision
  AND q.kind='RESOURCE_INVALIDATED' AND q.state='READY'
  AND public.concert_social_own_visible_xmin(q.xmin);
 IF r.operation_id IN ('blockConcertUser','unblockConcertUser') THEN
  audience:=ARRAY[r.actor_id,r.target_id]; why:='BLOCK_CHANGED';
 ELSIF r.operation_id IN ('setConcertHandle','removeConcertHandle') THEN
  SELECT array_agg(x ORDER BY x) INTO audience FROM (
   SELECT DISTINCT x FROM (
    SELECT r.actor_id AS x UNION ALL
    SELECT requester_id FROM public.concert_social_requests WHERE event_id=r.event_id AND recipient_id=r.actor_id AND (state IN ('PENDING','APPROVED') OR public.concert_social_own_visible_xmin(xmin)) UNION ALL
    SELECT recipient_id FROM public.concert_social_requests WHERE event_id=r.event_id AND requester_id=r.actor_id AND (state IN ('PENDING','APPROVED') OR public.concert_social_own_visible_xmin(xmin))
   ) users LIMIT 4097) bounded;
  why:='SOURCE_CHANGED';
 ELSIF r.operation_id IN ('setConcertIntro','publishConcertIntro','closeConcertIntro') THEN
  SELECT array_agg(x ORDER BY x) INTO audience FROM (
   SELECT DISTINCT x FROM (
    SELECT r.actor_id AS x UNION ALL
    SELECT requester_id FROM public.concert_social_requests WHERE event_id=r.event_id AND intro_id=r.target_id AND recipient_id=r.actor_id AND (state IN ('PENDING','APPROVED') OR public.concert_social_own_visible_xmin(xmin))
   ) users LIMIT 4097) bounded;
  why:='SOURCE_CHANGED';
 ELSE
  SELECT ARRAY[requester_id,recipient_id] INTO STRICT audience FROM public.concert_social_requests
   WHERE event_id=r.event_id AND request_id=r.target_id AND r.actor_id IN (requester_id,recipient_id)
    AND revision=r.resource_revision;
  why:='CONSENT_CHANGED';
 END IF;
 IF cardinality(audience)>4096 THEN RAISE EXCEPTION 'social policy audience capacity'; END IF;
 FOR who IN SELECT DISTINCT x FROM unnest(audience) x ORDER BY x LOOP
  INSERT INTO public.concert_social_policy_generations AS g(event_id,actor_id,revision) VALUES(r.event_id,who,2)
   ON CONFLICT(event_id,actor_id) DO UPDATE SET revision=g.revision+1 RETURNING revision INTO gen;
  INSERT INTO public.concert_social_policy_signals(signal_id,event_id,actor_id,policy_revision,reason,
   source_outbox,source_receipt,source_operation,source_resource,source_revision)
   VALUES(gen_random_uuid(),r.event_id,who,gen,why,o.outbox_id,r.receipt_id,r.operation_id,r.target_id,r.resource_revision);
 END LOOP;
 RETURN NEW;
END $$;
-- Deferred until the retained receipt has reached its final outcome. Receipt-
-- only witness fixtures are not an outbox source and emit no policy metadata.
-- The existing helper recognizes own savepoint/subtransaction writes.
CREATE CONSTRAINT TRIGGER concert_social_policy_success AFTER INSERT ON public.concert_outbox
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW
 EXECUTE FUNCTION public.concert_social_policy_from_outbox();
REVOKE ALL ON FUNCTION public.concert_social_policy_from_outbox() FROM PUBLIC;
