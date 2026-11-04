-- BE42 Sessions leaf. Identity and permanent fence belong to migration 0052.
CREATE TABLE public.concert_world_session_deletion_progress (
 user_id uuid PRIMARY KEY,
 saga_id uuid NOT NULL UNIQUE,
 command_binding bytea NOT NULL CHECK(octet_length(command_binding)=32),
 source_binding bytea NOT NULL CHECK(octet_length(source_binding)=32),
 removed_count bigint NOT NULL DEFAULT 0 CHECK(removed_count>=0),
 progress_revision bigint NOT NULL DEFAULT 0 CHECK(progress_revision>=0),
 completed_at timestamptz,
 UNIQUE(user_id,saga_id),
 FOREIGN KEY(user_id,saga_id)
  REFERENCES public.concert_world_account_deletion_fences(user_id,saga_id)
);
CREATE FUNCTION public.concert_world_session_deletion_progress_guard()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP<>'UPDATE' THEN RAISE EXCEPTION 'immutable session deletion operation'; END IF;
 IF OLD.completed_at IS NOT NULL
 OR (NEW.user_id,NEW.saga_id,NEW.command_binding,NEW.source_binding)
    IS DISTINCT FROM (OLD.user_id,OLD.saga_id,OLD.command_binding,OLD.source_binding)
 OR NEW.removed_count<OLD.removed_count
 OR NEW.progress_revision<>OLD.progress_revision+1 THEN
  RAISE EXCEPTION 'immutable or regressing session deletion operation';
 END IF;
 IF NEW.completed_at IS NOT NULL AND EXISTS(
  SELECT 1 FROM public.concert_world_session_deletion_victims
   WHERE user_id=NEW.user_id AND completed_at IS NULL) THEN
  RAISE EXCEPTION 'native effects incomplete';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_world_session_deletion_progress_guard
BEFORE UPDATE OR DELETE ON public.concert_world_session_deletion_progress
FOR EACH ROW EXECUTE FUNCTION public.concert_world_session_deletion_progress_guard();
CREATE TRIGGER concert_world_session_deletion_progress_no_truncate
BEFORE TRUNCATE ON public.concert_world_session_deletion_progress
FOR EACH STATEMENT EXECUTE FUNCTION public.concert_world_session_deletion_progress_guard();

-- Exact native obligations survive dispatcher reconstruction, uncertain replies
-- and database restart. This private journal is not a public presence feed.
CREATE TABLE public.concert_world_session_deletion_victims (
 user_id uuid NOT NULL,
 saga_id uuid NOT NULL,
 victim_id bytea NOT NULL CHECK(octet_length(victim_id)=32),
 event_id uuid NOT NULL,
 owner_process_id uuid NOT NULL,
 owner_epoch uuid NOT NULL,
 binding jsonb NOT NULL CHECK(jsonb_typeof(binding)='object'),
 completed_at timestamptz,
 PRIMARY KEY(user_id,victim_id),
 FOREIGN KEY(user_id,saga_id)
  REFERENCES public.concert_world_session_deletion_progress(user_id,saga_id)
);
CREATE FUNCTION public.concert_world_session_deletion_victim_guard()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP IN ('INSERT','UPDATE') THEN
  PERFORM pg_advisory_xact_lock(hashtextextended(
   'mcr-world-account-deletion:' || NEW.user_id::text,0));
 END IF;
 IF TG_OP='INSERT' THEN
  IF NEW.completed_at IS NOT NULL THEN RAISE EXCEPTION 'unapplied native obligation'; END IF;
  IF EXISTS(SELECT 1 FROM public.concert_world_session_deletion_progress
     WHERE user_id=NEW.user_id AND completed_at IS NOT NULL)
    AND NOT EXISTS(SELECT 1 FROM public.concert_world_session_deletion_victims
     WHERE user_id=NEW.user_id AND victim_id=NEW.victim_id AND saga_id=NEW.saga_id
     AND event_id=NEW.event_id AND owner_process_id=NEW.owner_process_id
     AND owner_epoch=NEW.owner_epoch AND binding=NEW.binding) THEN
   RAISE EXCEPTION 'completed native journal';
  END IF;
  RETURN NEW;
 END IF;
 IF TG_OP<>'UPDATE' THEN RAISE EXCEPTION 'immutable native obligation'; END IF;
 IF OLD.completed_at IS NOT NULL OR NEW.completed_at IS NULL
  OR (NEW.user_id,NEW.saga_id,NEW.victim_id,NEW.event_id,NEW.owner_process_id,NEW.owner_epoch,NEW.binding)
     IS DISTINCT FROM
     (OLD.user_id,OLD.saga_id,OLD.victim_id,OLD.event_id,OLD.owner_process_id,OLD.owner_epoch,OLD.binding) THEN
  RAISE EXCEPTION 'immutable native obligation';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_world_session_deletion_victim_guard
 BEFORE INSERT OR UPDATE OR DELETE ON public.concert_world_session_deletion_victims
 FOR EACH ROW EXECUTE FUNCTION public.concert_world_session_deletion_victim_guard();
CREATE TRIGGER concert_world_session_deletion_victim_no_truncate
 BEFORE TRUNCATE ON public.concert_world_session_deletion_victims
 FOR EACH STATEMENT EXECUTE FUNCTION public.concert_world_session_deletion_victim_guard();
CREATE INDEX concert_world_session_deletion_victims_pending
 ON public.concert_world_session_deletion_victims(user_id,event_id,owner_process_id,owner_epoch,victim_id)
 WHERE completed_at IS NULL;

-- Defense in depth for every membership producer, including process restarts.
-- Production producers take this user lock FIRST, before event/row locks.
CREATE FUNCTION public.concert_world_deleted_membership_guard()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 PERFORM pg_advisory_xact_lock(hashtextextended(
  'mcr-world-account-deletion:' || NEW.user_id::text,0));
 IF EXISTS(SELECT 1 FROM public.concert_world_account_deletion_fences
           WHERE user_id=NEW.user_id) THEN
  RAISE EXCEPTION 'world membership unavailable' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_world_deleted_membership_guard
BEFORE INSERT OR UPDATE ON public.concert_world_memberships
FOR EACH ROW EXECUTE FUNCTION public.concert_world_deleted_membership_guard();
CREATE INDEX concert_world_membership_account_cleanup
 ON public.concert_world_memberships(user_id,event_id);
