-- BE42 Wall-owned private associations only. Generic BE15 receipts/outbox and
-- canonical witnesses retain their separate owner/retention rules.
-- Historical 0019/0058 remain unchanged; active charge guards are not replaced.
ALTER TABLE public.concert_mural_operations ALTER COLUMN actor_id DROP NOT NULL;
ALTER TABLE public.concert_mural_operations ALTER COLUMN next_allowed_at DROP NOT NULL;
ALTER TABLE public.concert_mural_operations ADD CONSTRAINT concert_mural_retired_author_pair
 CHECK ((actor_id IS NULL)=(next_allowed_at IS NULL));

CREATE TABLE public.concert_mural_deletion_progress (
 user_id uuid PRIMARY KEY,
 saga_id uuid NOT NULL,
 actor_id uuid NOT NULL,
 retention_until timestamptz NOT NULL,
 command_binding bytea NOT NULL CHECK(octet_length(command_binding)=32),
 source_binding bytea NOT NULL CHECK(octet_length(source_binding)=32),
 category text NOT NULL DEFAULT 'MURAL_PRIVATE_METADATA' CHECK(category='MURAL_PRIVATE_METADATA'),
 category_version integer NOT NULL DEFAULT 1 CHECK(category_version=1),
 processed_rows bigint NOT NULL DEFAULT 0 CHECK(processed_rows>=0),
 progress_revision bigint NOT NULL DEFAULT 0 CHECK(progress_revision>=0),
 wait_until timestamptz,
 completed_at timestamptz,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 FOREIGN KEY(user_id,saga_id) REFERENCES public.concert_world_account_deletion_fences(user_id,saga_id),
 CHECK(completed_at IS NULL OR (progress_revision>0 AND wait_until IS NULL))
);
CREATE FUNCTION public.concert_mural_deletion_progress_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='INSERT' THEN
  IF NEW.processed_rows<>0 OR NEW.progress_revision<>0 OR NEW.wait_until IS NOT NULL OR NEW.completed_at IS NOT NULL THEN
   RAISE EXCEPTION 'Wall deletion must begin empty' USING ERRCODE='23514';
  END IF;
  RETURN NEW;
 END IF;
 IF TG_OP<>'UPDATE' OR OLD.completed_at IS NOT NULL
  OR (to_jsonb(NEW)-'processed_rows'-'progress_revision'-'wait_until'-'completed_at'-'updated_at')
   IS DISTINCT FROM (to_jsonb(OLD)-'processed_rows'-'progress_revision'-'wait_until'-'completed_at'-'updated_at')
  OR NEW.progress_revision<>OLD.progress_revision+1 OR NEW.processed_rows<OLD.processed_rows THEN
  RAISE EXCEPTION 'immutable or nonmonotonic Wall deletion identity' USING ERRCODE='23514';
 END IF;
 NEW.updated_at:=clock_timestamp();
 RETURN NEW;
END $$;
CREATE TRIGGER concert_mural_deletion_progress_guard
 BEFORE INSERT OR UPDATE OR DELETE ON public.concert_mural_deletion_progress
 FOR EACH ROW EXECUTE FUNCTION public.concert_mural_deletion_progress_guard();
CREATE FUNCTION public.concert_mural_deletion_progress_no_truncate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'permanent Wall deletion progress' USING ERRCODE='23514'; END $$;
CREATE TRIGGER concert_mural_deletion_progress_no_truncate BEFORE TRUNCATE ON public.concert_mural_deletion_progress
 FOR EACH STATEMENT EXECUTE FUNCTION public.concert_mural_deletion_progress_no_truncate();

CREATE INDEX concert_mural_operation_private_author ON public.concert_mural_operations(actor_id,event_id,revision) WHERE actor_id IS NOT NULL;
CREATE INDEX concert_mural_limit_private_author ON public.concert_mural_limits(actor_id,event_id);
CREATE INDEX concert_mural_action_private_author ON public.concert_mural_tool_actions(actor_id,event_id,revision DESC);
CREATE INDEX concert_mural_correlation_private_author ON public.concert_mural_tool_correlations(actor_id,event_id,idempotency_key);
CREATE INDEX concert_mural_charge_private_author ON public.concert_mural_tool_charges(actor_id,committed_at,event_id);

-- Every owning writer and deletion page uses the exact canonical user-first
-- lock before any event/row lock. These guards also reject forgotten writers.
-- Only a matching source-bound deletion page may scrub the two private fields
-- of an existing public Pencil operation; all public bytes remain unchanged.
CREATE FUNCTION public.concert_mural_private_write_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE subject uuid; retired uuid;
BEGIN
 IF TG_TABLE_NAME='concert_mural_operations' AND TG_OP='UPDATE' AND OLD.actor_id IS NULL THEN
  RAISE EXCEPTION 'retired Wall author cannot be recreated' USING ERRCODE='23514';
 END IF;
 subject:=NEW.actor_id;
 IF TG_TABLE_NAME='concert_mural_operations' AND subject IS NULL THEN
  IF TG_OP<>'UPDATE' THEN
   RAISE EXCEPTION 'new Wall operation requires author and cooldown' USING ERRCODE='23514';
  END IF;
  subject:=OLD.actor_id;
  retired:=nullif(current_setting('mcr.mural_private_deletion_user',true),'')::uuid;
  PERFORM pg_advisory_xact_lock(hashtextextended('mcr-world-account-deletion:'||subject::text,0));
  IF subject IS NULL OR retired IS DISTINCT FROM subject OR NEW.next_allowed_at IS NOT NULL
   OR (to_jsonb(NEW)-'actor_id'-'next_allowed_at') IS DISTINCT FROM (to_jsonb(OLD)-'actor_id'-'next_allowed_at')
   OR NOT EXISTS(SELECT 1 FROM public.concert_mural_deletion_progress p
     JOIN public.concert_world_account_deletion_fences f USING(user_id,saga_id)
     WHERE p.user_id=subject AND p.completed_at IS NULL)
   OR NOT coalesce(clock_timestamp()<nullif(current_setting('mcr.mural_private_deletion_until',true),'')::timestamptz,false) THEN
   RAISE EXCEPTION 'Wall author retirement requires current bound owner page' USING ERRCODE='23514';
  END IF;
  RETURN NEW;
 END IF;
 IF TG_TABLE_NAME='concert_mural_operations' THEN
  IF NEW.next_allowed_at IS NULL THEN
   RAISE EXCEPTION 'new Wall operation requires cooldown' USING ERRCODE='23514';
  END IF;
 END IF;
 IF subject IS NULL OR (TG_OP='UPDATE' AND NEW.actor_id IS DISTINCT FROM OLD.actor_id) THEN
  RAISE EXCEPTION 'invalid Wall private author transition' USING ERRCODE='23514';
 END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended('mcr-world-account-deletion:'||subject::text,0));
 IF EXISTS(SELECT 1 FROM public.concert_world_account_deletion_fences WHERE user_id=subject) THEN
  RAISE EXCEPTION 'deleted Wall author cannot be recreated' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_mural_operation_private_write BEFORE INSERT OR UPDATE ON public.concert_mural_operations
 FOR EACH ROW EXECUTE FUNCTION public.concert_mural_private_write_guard();
CREATE TRIGGER concert_mural_limit_private_write BEFORE INSERT OR UPDATE ON public.concert_mural_limits
 FOR EACH ROW EXECUTE FUNCTION public.concert_mural_private_write_guard();
CREATE TRIGGER concert_mural_action_private_write BEFORE INSERT OR UPDATE ON public.concert_mural_tool_actions
 FOR EACH ROW EXECUTE FUNCTION public.concert_mural_private_write_guard();
CREATE TRIGGER concert_mural_correlation_private_write BEFORE INSERT OR UPDATE ON public.concert_mural_tool_correlations
 FOR EACH ROW EXECUTE FUNCTION public.concert_mural_private_write_guard();
CREATE TRIGGER concert_mural_charge_private_write BEFORE INSERT OR UPDATE ON public.concert_mural_tool_charges
 FOR EACH ROW EXECUTE FUNCTION public.concert_mural_private_write_guard();
