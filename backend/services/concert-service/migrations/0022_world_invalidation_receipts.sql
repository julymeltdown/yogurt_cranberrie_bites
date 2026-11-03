-- Consumer-only tombstones on the existing Concert outbox. Never a second queue.
CREATE TABLE public.concert_world_invalidation_source (
 singleton boolean PRIMARY KEY CHECK(singleton), owner_epoch uuid NOT NULL UNIQUE
);
INSERT INTO public.concert_world_invalidation_source VALUES(true,gen_random_uuid());
CREATE TRIGGER concert_world_invalidation_source_immutable BEFORE UPDATE OR DELETE ON public.concert_world_invalidation_source
 FOR EACH ROW EXECUTE FUNCTION public.concert_maintenance_immutable();
CREATE TABLE public.concert_world_invalidation_receipts (
 consumer_id uuid NOT NULL, event_id uuid NOT NULL, signal_id uuid NOT NULL,
 owner_epoch uuid NOT NULL, receipt_id uuid NOT NULL, operation_id text NOT NULL,
 resource_id uuid NOT NULL, revision bigint NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),
 scope_kind text NOT NULL CHECK(scope_kind IN ('PAIR','EVENT')), scope_a uuid NOT NULL, scope_b uuid NOT NULL,
 victims jsonb NOT NULL CHECK(jsonb_typeof(victims)='array' AND jsonb_array_length(victims)<=800),
 superseded boolean NOT NULL, applied_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 PRIMARY KEY(consumer_id,event_id,signal_id)
);
-- Survive outbox pruning, receipt retirement and process restart. No cascade.
CREATE TRIGGER concert_world_invalidation_receipts_immutable BEFORE UPDATE OR DELETE ON public.concert_world_invalidation_receipts
 FOR EACH ROW EXECUTE FUNCTION public.concert_maintenance_immutable();
CREATE TABLE public.concert_world_invalidation_cursors (
 consumer_id uuid NOT NULL,event_id uuid NOT NULL,owner_epoch uuid NOT NULL,
 scope_kind text NOT NULL CHECK(scope_kind IN ('PAIR','EVENT')),scope_a uuid NOT NULL,scope_b uuid NOT NULL,
 revision bigint NOT NULL CHECK(revision BETWEEN 1 AND 9007199254740991),
 PRIMARY KEY(consumer_id,event_id,owner_epoch,scope_kind,scope_a,scope_b)
);
