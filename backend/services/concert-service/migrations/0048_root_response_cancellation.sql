-- No existing evidence is relabelled. Only the supervised, destroyed producer
-- plus sealed/drained callback barrier may create abandonment evidence.
ALTER TABLE public.concert_product_owner_scopes
 ADD COLUMN response_abandoned_at timestamptz,
 ADD COLUMN cancellation_settled_at timestamptz;
DO $$
DECLARE target text; matches integer;
BEGIN
 SELECT count(*),min(conname) INTO matches,target FROM pg_constraint
 WHERE conrelid='public.concert_product_owner_scopes'::regclass AND contype='c'
 AND pg_get_constraintdef(oid) LIKE '%sealed_at%'
 AND pg_get_constraintdef(oid) LIKE '%root_finished_at%';
 IF matches<>1 THEN RAISE EXCEPTION 'unexpected owner scope phase constraints'; END IF;
 EXECUTE format('ALTER TABLE public.concert_product_owner_scopes DROP CONSTRAINT %I',target);
END $$;
ALTER TABLE public.concert_product_owner_scopes
 ADD CONSTRAINT concert_owner_scope_terminal CHECK(
 (phase='ACTIVE' AND sealed_at IS NULL AND root_finished_at IS NULL)
 OR (phase='SEALED' AND sealed_at IS NOT NULL AND root_finished_at IS NULL)
 OR (phase='ROOT_FINISHED' AND sealed_at IS NOT NULL AND root_finished_at IS NOT NULL
     AND ((serialized_at IS NOT NULL AND delivery_secret_hash IS NOT NULL)
       OR (serialized_at IS NULL AND delivery_secret_hash IS NULL)
       OR response_abandoned_at IS NOT NULL))),
 ADD CONSTRAINT concert_owner_scope_abandonment CHECK(
 response_abandoned_at IS NULL OR (phase='ROOT_FINISHED' AND serialized_at IS NULL AND delivery_completed_at IS NULL)),
 ADD CONSTRAINT concert_owner_scope_cancel_settled CHECK(
 cancellation_settled_at IS NULL OR response_abandoned_at IS NOT NULL);

CREATE OR REPLACE FUNCTION public.concert_product_owner_scope_transition() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN
  RAISE EXCEPTION 'owner scope lifecycle evidence is immutable' USING ERRCODE='23514';
 END IF;
 -- New evidence has exactly two immutable transitions; all original
 -- transitions below require both new fields to remain unchanged.
 IF NEW.response_abandoned_at IS DISTINCT FROM OLD.response_abandoned_at
    OR NEW.cancellation_settled_at IS DISTINCT FROM OLD.cancellation_settled_at THEN
  IF (NEW.instance_id,NEW.scope_id,NEW.binding_digest,NEW.binding,NEW.created_at,NEW.delivery_secret_hash,NEW.serialized_at,NEW.sealed_at,NEW.delivery_completed_at)
       IS DISTINCT FROM (OLD.instance_id,OLD.scope_id,OLD.binding_digest,OLD.binding,OLD.created_at,OLD.delivery_secret_hash,OLD.serialized_at,OLD.sealed_at,OLD.delivery_completed_at)
    OR NEW.serialized_at IS NOT NULL OR NEW.delivery_completed_at IS NOT NULL
    OR NOT COALESCE(
       (OLD.phase='SEALED' AND NEW.phase='ROOT_FINISHED'
        AND OLD.response_abandoned_at IS NULL AND NEW.response_abandoned_at IS NOT NULL
        AND NEW.root_finished_at IS NOT NULL AND OLD.cancellation_settled_at IS NULL AND NEW.cancellation_settled_at IS NULL)
       OR (OLD.phase='ROOT_FINISHED' AND NEW.phase='ROOT_FINISHED'
        AND OLD.response_abandoned_at IS NOT NULL AND NEW.response_abandoned_at=OLD.response_abandoned_at
        AND NEW.root_finished_at=OLD.root_finished_at
        AND OLD.cancellation_settled_at IS NULL AND NEW.cancellation_settled_at IS NOT NULL
        AND NOT EXISTS(SELECT 1 FROM public.concert_product_child_intents
         WHERE instance_id=NEW.instance_id AND scope_id=NEW.scope_id AND completed_at IS NULL)),false) THEN
   RAISE EXCEPTION 'invalid response cancellation evidence' USING ERRCODE='23514';
  END IF;
  RETURN NEW;
 END IF;
 IF (NEW.instance_id,NEW.scope_id,NEW.binding_digest,NEW.binding,NEW.created_at)
    IS DISTINCT FROM (OLD.instance_id,OLD.scope_id,OLD.binding_digest,OLD.binding,OLD.created_at)
 OR NOT COALESCE((
      (OLD.phase='ACTIVE' AND NEW.phase='ACTIVE' AND NEW.sealed_at IS NULL AND NEW.root_finished_at IS NULL
       AND NEW.delivery_completed_at IS NULL
       AND ((OLD.delivery_secret_hash IS NULL AND NEW.delivery_secret_hash IS NOT NULL AND NEW.serialized_at IS NOT DISTINCT FROM OLD.serialized_at)
         OR (NEW.delivery_secret_hash=OLD.delivery_secret_hash AND OLD.serialized_at IS NULL AND NEW.serialized_at IS NOT NULL)))
   OR (OLD.phase='ACTIVE' AND NEW.phase='SEALED' AND NEW.root_finished_at IS NULL
       AND NEW.delivery_completed_at IS NULL
       AND NEW.delivery_secret_hash IS NOT DISTINCT FROM OLD.delivery_secret_hash
       AND NEW.serialized_at IS NOT DISTINCT FROM OLD.serialized_at)
   OR (OLD.phase='SEALED' AND NEW.phase='ROOT_FINISHED' AND NEW.sealed_at=OLD.sealed_at
       AND NEW.delivery_completed_at IS NULL
       AND NEW.delivery_secret_hash IS NOT DISTINCT FROM OLD.delivery_secret_hash
       AND NEW.serialized_at IS NOT DISTINCT FROM OLD.serialized_at)
   OR (OLD.phase='ROOT_FINISHED' AND NEW.phase='ROOT_FINISHED'
       AND OLD.delivery_completed_at IS NULL AND NEW.delivery_completed_at IS NOT NULL
       AND NEW.sealed_at=OLD.sealed_at AND NEW.root_finished_at=OLD.root_finished_at
       AND NEW.delivery_secret_hash=OLD.delivery_secret_hash AND NEW.serialized_at=OLD.serialized_at)),false) THEN
  RAISE EXCEPTION 'invalid owner scope lifecycle transition' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
