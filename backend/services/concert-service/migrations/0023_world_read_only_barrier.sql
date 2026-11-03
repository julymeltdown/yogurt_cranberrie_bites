-- World-owned component evidence, committed inside the existing closure START transaction.
CREATE TABLE public.concert_world_read_only_receipts (
 event_id uuid NOT NULL, job_id uuid NOT NULL, operation_id uuid NOT NULL UNIQUE,
 receipt_id uuid NOT NULL UNIQUE, revision bigint NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),
 owners jsonb NOT NULL CHECK(jsonb_typeof(owners)='array' AND jsonb_array_length(owners)<=16),
 victims jsonb NOT NULL CHECK(jsonb_typeof(victims)='array' AND jsonb_array_length(victims)<=800),
 tickets_removed bigint NOT NULL CHECK(tickets_removed>=0),
 memberships_revoked bigint NOT NULL CHECK(memberships_revoked>=0),
 rooms_disabled bigint NOT NULL CHECK(rooms_disabled BETWEEN 0 AND 16),
 owners_retired bigint NOT NULL CHECK(owners_retired BETWEEN 0 AND 16),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(), PRIMARY KEY(event_id,job_id),
 FOREIGN KEY(event_id,job_id) REFERENCES public.concert_closure_jobs(event_id,job_id)
);
CREATE TRIGGER concert_world_read_only_receipts_immutable BEFORE UPDATE OR DELETE ON public.concert_world_read_only_receipts
 FOR EACH ROW EXECUTE FUNCTION public.concert_maintenance_immutable();
