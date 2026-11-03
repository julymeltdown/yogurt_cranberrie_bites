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
 FOREIGN KEY(event_id,job_id,operation_id,community_receipt_id,community_revision)
  REFERENCES public.concert_closure_community_barriers(event_id,job_id,operation_id,receipt_id,revision),
 CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'::uuid),
 CHECK(receipt_id<>'00000000-0000-0000-0000-000000000000'::uuid),
 CHECK(world_receipt_id<>'00000000-0000-0000-0000-000000000000'::uuid)
);
-- The Rust composite validates World binding through the frozen owning API; its
-- independent receipt schema/name is owned by World, not duplicated by BE41.
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
