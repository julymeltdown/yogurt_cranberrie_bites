-- Child intent admission only needs the current ACTIVE phase. A command's
-- native witness holds SHARE on the same scope row through commit, so INSERT
-- admission must not request UPDATE on that row. Sealing still takes UPDATE and
-- therefore waits for admitted child insertions; completion keeps UPDATE.
CREATE OR REPLACE FUNCTION public.concert_product_child_intent_transition()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE current_phase text; current_count bigint;
BEGIN
 IF TG_OP='DELETE' THEN
  RAISE EXCEPTION 'child intent evidence is immutable' USING ERRCODE='23514';
 END IF;
 IF TG_OP='INSERT' THEN
  SELECT phase INTO current_phase FROM public.concert_product_owner_scopes
   WHERE instance_id=NEW.instance_id AND scope_id=NEW.scope_id FOR SHARE;
  SELECT count(*) INTO current_count FROM public.concert_product_child_intents
   WHERE instance_id=NEW.instance_id AND scope_id=NEW.scope_id;
  IF current_phase IS DISTINCT FROM 'ACTIVE' OR NEW.completed_at IS NOT NULL OR current_count>=202 THEN
   RAISE EXCEPTION 'child intent admission closed' USING ERRCODE='23514';
  END IF;
 ELSE
  SELECT phase INTO current_phase FROM public.concert_product_owner_scopes
   WHERE instance_id=NEW.instance_id AND scope_id=NEW.scope_id FOR UPDATE;
  IF current_phase IS DISTINCT FROM 'ROOT_FINISHED'
   OR (NEW.instance_id,NEW.scope_id,NEW.binding_digest,NEW.kind,NEW.intent_id,NEW.key_id,NEW.sealed_reference,NEW.reference_digest,NEW.created_at)
    IS DISTINCT FROM (OLD.instance_id,OLD.scope_id,OLD.binding_digest,OLD.kind,OLD.intent_id,OLD.key_id,OLD.sealed_reference,OLD.reference_digest,OLD.created_at)
   OR OLD.completed_at IS NOT NULL OR NEW.completed_at IS NULL THEN
   RAISE EXCEPTION 'invalid child intent completion' USING ERRCODE='23514';
  END IF;
 END IF;
 RETURN NEW;
END $$;
