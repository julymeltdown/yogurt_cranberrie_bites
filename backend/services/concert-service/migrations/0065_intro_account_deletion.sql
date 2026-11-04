-- BE27/BE42 explicit owner erasure. 0049 and every normal writer guard remain
-- immutable. This is an effect/idempotency receipt, NOT aggregate category ACK.
CREATE TABLE public.concert_intro_account_deletions (
 user_id uuid PRIMARY KEY,
 saga_id uuid NOT NULL,
 actor_id uuid NOT NULL CHECK(actor_id <> '00000000-0000-0000-0000-000000000000'),
 category text NOT NULL DEFAULT 'INTRO_TEXT' CHECK(category='INTRO_TEXT'),
 category_version integer NOT NULL DEFAULT 1 CHECK(category_version=1),
 retention_seconds bigint NOT NULL,
 retention_nanos integer NOT NULL CHECK(retention_nanos BETWEEN 0 AND 999999999),
 identity_binding bytea NOT NULL CHECK(octet_length(identity_binding)=32),
 command_binding bytea NOT NULL CHECK(octet_length(command_binding)=32),
 source_binding bytea NOT NULL CHECK(octet_length(source_binding)=32 AND source_binding<>decode(repeat('00',32),'hex')),
 fence_revision bigint NOT NULL CHECK(fence_revision=1),
 last_event uuid,
 deleted_intros bigint NOT NULL DEFAULT 0 CHECK(deleted_intros>=0),
 progress_revision bigint NOT NULL DEFAULT 0 CHECK(progress_revision>=0),
 active_xid text,
 authority_until timestamptz,
 completed_at timestamptz,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 FOREIGN KEY(user_id,saga_id) REFERENCES public.concert_world_account_deletion_fences(user_id,saga_id),
 CHECK((active_xid IS NULL)=(authority_until IS NULL))
);
CREATE FUNCTION public.concert_intro_deletion_receipt_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN
  RAISE EXCEPTION 'Intro effect receipt is immutable' USING ERRCODE='23514';
 ELSIF TG_OP='INSERT' THEN
  IF NEW.progress_revision<>0 OR NEW.deleted_intros<>0 OR NEW.last_event IS NOT NULL
     OR NEW.active_xid IS NOT NULL OR NEW.completed_at IS NOT NULL THEN
   RAISE EXCEPTION 'Intro effect receipt must begin empty' USING ERRCODE='23514';
  END IF;
 ELSE
  IF OLD.completed_at IS NOT NULL
    OR (NEW.user_id,NEW.saga_id,NEW.actor_id,NEW.category,NEW.category_version,
        NEW.retention_seconds,NEW.retention_nanos,NEW.identity_binding,
        NEW.command_binding,NEW.source_binding,NEW.fence_revision,NEW.created_at)
       IS DISTINCT FROM
       (OLD.user_id,OLD.saga_id,OLD.actor_id,OLD.category,OLD.category_version,
        OLD.retention_seconds,OLD.retention_nanos,OLD.identity_binding,
        OLD.command_binding,OLD.source_binding,OLD.fence_revision,OLD.created_at)
    OR NEW.active_xid IS DISTINCT FROM pg_current_xact_id()::text
    OR NEW.authority_until IS NULL OR NEW.authority_until<=clock_timestamp()
    OR NEW.authority_until>clock_timestamp()+interval '5 seconds'
    OR NEW.progress_revision NOT BETWEEN OLD.progress_revision AND OLD.progress_revision+1
    OR NEW.deleted_intros<OLD.deleted_intros
    OR (OLD.last_event IS NOT NULL AND (NEW.last_event IS NULL OR NEW.last_event<OLD.last_event)) THEN
   RAISE EXCEPTION 'Intro effect receipt binding or progress invalid' USING ERRCODE='23514';
  END IF;
  IF NEW.progress_revision=OLD.progress_revision AND
    (NEW.last_event,NEW.deleted_intros,NEW.completed_at) IS DISTINCT FROM
    (OLD.last_event,OLD.deleted_intros,OLD.completed_at) THEN
   RAISE EXCEPTION 'Intro effect progress must advance' USING ERRCODE='23514';
  END IF;
  IF NEW.completed_at IS NOT NULL AND (NEW.progress_revision<1 OR EXISTS(
      SELECT 1 FROM public.concert_intros WHERE owner_id=NEW.user_id)) THEN
   RAISE EXCEPTION 'Intro effect is not terminal' USING ERRCODE='23514';
  END IF;
 END IF;
 IF NOT EXISTS(SELECT 1 FROM public.concert_world_account_deletion_fences f
   WHERE f.user_id=NEW.user_id AND f.saga_id=NEW.saga_id
    AND f.identity_binding=NEW.identity_binding AND f.fence_revision=NEW.fence_revision) THEN
  RAISE EXCEPTION 'Intro effect requires exact common fence' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_intro_deletion_receipt_guard BEFORE INSERT OR UPDATE OR DELETE
 ON public.concert_intro_account_deletions FOR EACH ROW
 EXECUTE FUNCTION public.concert_intro_deletion_receipt_guard();
CREATE FUNCTION public.concert_intro_deletion_commit_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NEW.authority_until IS NULL OR clock_timestamp()>=NEW.authority_until THEN
  RAISE EXCEPTION 'Intro deletion authority expired' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER concert_intro_deletion_commit_guard AFTER UPDATE
 ON public.concert_intro_account_deletions DEFERRABLE INITIALLY DEFERRED
 FOR EACH ROW EXECUTE FUNCTION public.concert_intro_deletion_commit_guard();

CREATE FUNCTION public.concert_intro_account_write_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE subject uuid;
BEGIN
 subject:=CASE WHEN TG_OP='DELETE' THEN OLD.owner_id ELSE NEW.owner_id END;
 PERFORM pg_advisory_xact_lock(hashtextextended('mcr-world-account-deletion:'||subject::text,0));
 IF TG_OP='DELETE' THEN
  -- An uncovered DELETE is NOT deletion authority. Only this exact current
  -- fenced owner transaction may execute the separately provisioned transition.
  IF NOT EXISTS(SELECT 1 FROM public.concert_intro_account_deletions d
     JOIN public.concert_world_account_deletion_fences f USING(user_id,saga_id)
     WHERE d.user_id=subject AND d.identity_binding=f.identity_binding
       AND d.fence_revision=f.fence_revision AND d.active_xid=pg_current_xact_id()::text
       AND d.authority_until>clock_timestamp() AND d.completed_at IS NULL) THEN
   RAISE EXCEPTION 'Intro erase requires owner deletion transaction' USING ERRCODE='23514';
  END IF;
  RETURN OLD;
 END IF;
 IF EXISTS(SELECT 1 FROM public.concert_world_account_deletion_fences WHERE user_id=subject)
    OR (TG_OP='UPDATE' AND EXISTS(SELECT 1 FROM public.concert_world_account_deletion_fences WHERE user_id=OLD.owner_id)) THEN
  RAISE EXCEPTION 'Deleted Intro owner cannot recreate private text' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_intro_account_write_guard BEFORE INSERT OR UPDATE OR DELETE
 ON public.concert_intros FOR EACH ROW EXECUTE FUNCTION public.concert_intro_account_write_guard();
CREATE FUNCTION public.concert_intro_deletion_no_truncate() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'Intro owner history cannot be truncated' USING ERRCODE='23514'; END $$;
CREATE TRIGGER concert_intro_deletion_no_truncate BEFORE TRUNCATE ON public.concert_intro_account_deletions
 FOR EACH STATEMENT EXECUTE FUNCTION public.concert_intro_deletion_no_truncate();
CREATE TRIGGER concert_intro_private_no_truncate BEFORE TRUNCATE ON public.concert_intros
 FOR EACH STATEMENT EXECUTE FUNCTION public.concert_intro_deletion_no_truncate();
CREATE INDEX concert_intro_owner_deletion ON public.concert_intros(owner_id,event_id);
REVOKE ALL ON FUNCTION public.concert_intro_deletion_receipt_guard() FROM PUBLIC;
REVOKE ALL ON FUNCTION public.concert_intro_deletion_commit_guard() FROM PUBLIC;
REVOKE ALL ON FUNCTION public.concert_intro_account_write_guard() FROM PUBLIC;
REVOKE ALL ON FUNCTION public.concert_intro_deletion_no_truncate() FROM PUBLIC;
