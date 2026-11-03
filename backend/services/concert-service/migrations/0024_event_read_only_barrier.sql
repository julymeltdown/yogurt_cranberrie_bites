-- BE41 READ_ONLY community/composite evidence. World snapshots/receipts belong to 0023.
-- No content, participant identities, credentials or remote effects are copied here.
CREATE TABLE public.concert_closure_community_barriers (
 event_id uuid NOT NULL, job_id uuid NOT NULL, operation_id uuid NOT NULL,
 receipt_id uuid NOT NULL UNIQUE, revision bigint NOT NULL CHECK(revision=1),
 parties_cancelled bigint NOT NULL CHECK(parties_cancelled>=0),
 applications_cancelled bigint NOT NULL CHECK(applications_cancelled>=0),
 giveaways_cancelled bigint NOT NULL CHECK(giveaways_cancelled>=0),
 reservations_cancelled bigint NOT NULL CHECK(reservations_cancelled>=0),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(job_id), UNIQUE(operation_id),
 UNIQUE(event_id,job_id,operation_id,receipt_id,revision),
 FOREIGN KEY(event_id,job_id) REFERENCES public.concert_closure_jobs(event_id,job_id),
 CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'::uuid),
 CHECK(receipt_id<>'00000000-0000-0000-0000-000000000000'::uuid)
);
CREATE TABLE public.concert_closure_read_only_barriers (
 event_id uuid NOT NULL, job_id uuid NOT NULL, operation_id uuid NOT NULL,
 receipt_id uuid NOT NULL UNIQUE, revision bigint NOT NULL CHECK(revision=1),
 world_receipt_id uuid NOT NULL UNIQUE, world_revision bigint NOT NULL CHECK(world_revision BETWEEN 1 AND 9007199254740991),
 community_receipt_id uuid NOT NULL UNIQUE, community_revision bigint NOT NULL CHECK(community_revision=1),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(job_id), UNIQUE(operation_id),
 FOREIGN KEY(event_id,job_id) REFERENCES public.concert_closure_jobs(event_id,job_id),
 FOREIGN KEY(world_receipt_id) REFERENCES public.concert_world_read_only_receipts(receipt_id),
 FOREIGN KEY(event_id,job_id,operation_id,community_receipt_id,community_revision)
  REFERENCES public.concert_closure_community_barriers(event_id,job_id,operation_id,receipt_id,revision),
 CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'::uuid),
 CHECK(receipt_id<>'00000000-0000-0000-0000-000000000000'::uuid),
 CHECK(world_receipt_id<>'00000000-0000-0000-0000-000000000000'::uuid)
);
-- World owns 0023. Bind to its immutable receipt instead of trusting a made-up
-- UUID or copying its owner/victim data into the composite.
CREATE FUNCTION public.concert_closure_world_receipt_binding() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT EXISTS(SELECT 1 FROM public.concert_world_read_only_receipts w
  WHERE w.event_id=NEW.event_id AND w.job_id=NEW.job_id AND w.operation_id=NEW.operation_id
   AND w.receipt_id=NEW.world_receipt_id AND w.revision=NEW.world_revision) THEN
  RAISE EXCEPTION 'read-only World receipt binding mismatch' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_closure_world_receipt_binding BEFORE INSERT
 ON public.concert_closure_read_only_barriers FOR EACH ROW EXECUTE FUNCTION public.concert_closure_world_receipt_binding();
CREATE TRIGGER concert_closure_community_barriers_immutable BEFORE UPDATE OR DELETE
 ON public.concert_closure_community_barriers FOR EACH ROW EXECUTE FUNCTION public.concert_closure_immutable();
CREATE TRIGGER concert_closure_read_only_barriers_immutable BEFORE UPDATE OR DELETE
 ON public.concert_closure_read_only_barriers FOR EACH ROW EXECUTE FUNCTION public.concert_closure_immutable();
CREATE CONSTRAINT TRIGGER concert_closure_community_barriers_deadline AFTER INSERT
 ON public.concert_closure_community_barriers DEFERRABLE INITIALLY DEFERRED
 FOR EACH ROW EXECUTE FUNCTION public.concert_closure_deadline();
CREATE CONSTRAINT TRIGGER concert_closure_read_only_barriers_deadline AFTER INSERT
 ON public.concert_closure_read_only_barriers DEFERRABLE INITIALLY DEFERRED
 FOR EACH ROW EXECUTE FUNCTION public.concert_closure_deadline();
