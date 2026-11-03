-- Forward-only maintenance origin on the retained BE15 queue. No fake actor/receipt.
CREATE TABLE public.concert_maintenance_invalidations (
 maintenance_id uuid PRIMARY KEY,
 event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE RESTRICT,
 resource_id uuid NOT NULL,
 resource_revision bigint NOT NULL CHECK(resource_revision BETWEEN 1 AND 9007199254740991),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(maintenance_id,event_id,resource_id,resource_revision)
);
ALTER TABLE public.concert_outbox ALTER COLUMN receipt_id DROP NOT NULL;
ALTER TABLE public.concert_outbox DROP CONSTRAINT concert_outbox_kind_check;
ALTER TABLE public.concert_outbox ADD COLUMN maintenance_id uuid UNIQUE;
ALTER TABLE public.concert_outbox ADD CONSTRAINT concert_outbox_origin_check CHECK(
 (kind='RESOURCE_INVALIDATED' AND receipt_id IS NOT NULL AND maintenance_id IS NULL)
 OR (kind='MAINTENANCE_INVALIDATED' AND receipt_id IS NULL AND maintenance_id IS NOT NULL AND maintenance_id=outbox_id)
);
ALTER TABLE public.concert_outbox ADD CONSTRAINT concert_outbox_maintenance_reference
 FOREIGN KEY(maintenance_id,event_id,resource_id,resource_revision)
 REFERENCES public.concert_maintenance_invalidations(maintenance_id,event_id,resource_id,resource_revision) ON DELETE RESTRICT;
-- Original command receipt/event/resource foreign key remains unchanged.
CREATE FUNCTION public.concert_maintenance_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'maintenance tombstone is immutable' USING ERRCODE='23514'; END $$;
CREATE TRIGGER concert_maintenance_immutable BEFORE UPDATE OR DELETE ON public.concert_maintenance_invalidations
 FOR EACH ROW EXECUTE FUNCTION public.concert_maintenance_immutable();
