-- BE42 Media leaf. Existing source hashes, declarations and policy identifiers
-- remain immutable; only bytes/dimensions/linkage/private associations are erased.
CREATE INDEX concert_media_account_page ON public.concert_media(owner_id,media_id);
CREATE INDEX concert_meeting_photo_account_page ON public.concert_meeting_beacon_photos(owner_id,media_id);

CREATE TABLE public.concert_media_account_deletion_progress (
 user_id uuid PRIMARY KEY,
 saga_id uuid NOT NULL UNIQUE,
 actor_id uuid NOT NULL CHECK(actor_id<>'00000000-0000-0000-0000-000000000000'),
 retention_until timestamptz NOT NULL,
 command_binding bytea NOT NULL CHECK(octet_length(command_binding)=32),
 source_binding bytea NOT NULL CHECK(octet_length(source_binding)=32 AND source_binding<>decode(repeat('00',32),'hex')),
 category_version integer NOT NULL CHECK(category_version=1),
 fence_revision bigint NOT NULL CHECK(fence_revision=1),
 cursor_media_id uuid,
 processed_media bigint NOT NULL DEFAULT 0 CHECK(processed_media>=0),
 erased_media bigint NOT NULL DEFAULT 0 CHECK(erased_media>=0),
 removed_associations bigint NOT NULL DEFAULT 0 CHECK(removed_associations>=0),
 progress_revision bigint NOT NULL DEFAULT 0 CHECK(progress_revision>=0),
 completed_at timestamptz,
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 FOREIGN KEY(user_id,saga_id) REFERENCES public.concert_world_account_deletion_fences(user_id,saga_id),
 CHECK(completed_at IS NULL OR progress_revision>0)
);

CREATE FUNCTION public.concert_media_account_progress_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='INSERT' THEN
  IF NEW.progress_revision<>0 OR NEW.processed_media<>0 OR NEW.erased_media<>0
     OR NEW.removed_associations<>0 OR NEW.cursor_media_id IS NOT NULL
     OR NEW.completed_at IS NOT NULL THEN
   RAISE EXCEPTION 'Media account progress must start incomplete' USING ERRCODE='23514';
  END IF;
  RETURN NEW;
 END IF;
 IF TG_OP<>'UPDATE' THEN
  RAISE EXCEPTION 'Media account progress cannot be removed' USING ERRCODE='23514';
 END IF;
 IF OLD.completed_at IS NOT NULL OR
   (NEW.user_id,NEW.saga_id,NEW.actor_id,NEW.retention_until,NEW.command_binding,
    NEW.source_binding,NEW.category_version,NEW.fence_revision,NEW.created_at)
   IS DISTINCT FROM
   (OLD.user_id,OLD.saga_id,OLD.actor_id,OLD.retention_until,OLD.command_binding,
    OLD.source_binding,OLD.category_version,OLD.fence_revision,OLD.created_at)
   OR NEW.progress_revision<>OLD.progress_revision+1
   OR NEW.processed_media<OLD.processed_media OR NEW.erased_media<OLD.erased_media
   OR NEW.removed_associations<OLD.removed_associations
   OR (OLD.cursor_media_id IS NOT NULL AND
       (NEW.cursor_media_id IS NULL OR NEW.cursor_media_id<OLD.cursor_media_id)) THEN
  RAISE EXCEPTION 'Media account progress is immutable or nonmonotonic' USING ERRCODE='23514';
 END IF;
 IF NEW.completed_at IS NOT NULL AND (
   EXISTS(SELECT 1 FROM public.concert_media WHERE owner_id=NEW.user_id AND
     (state<>'DELETED' OR staged IS NOT NULL OR sanitized IS NOT NULL OR
      width IS NOT NULL OR height IS NOT NULL OR target_id IS NOT NULL))
   OR EXISTS(SELECT 1 FROM public.concert_meeting_beacon_photos WHERE owner_id=NEW.user_id)) THEN
  RAISE EXCEPTION 'Media account category still contains private data' USING ERRCODE='23514';
 END IF;
 NEW.updated_at:=clock_timestamp();
 RETURN NEW;
END $$;
CREATE TRIGGER concert_media_account_progress_guard BEFORE INSERT OR UPDATE OR DELETE
 ON public.concert_media_account_deletion_progress FOR EACH ROW
 EXECUTE FUNCTION public.concert_media_account_progress_guard();
CREATE TRIGGER concert_media_account_progress_no_truncate BEFORE TRUNCATE
 ON public.concert_media_account_deletion_progress FOR EACH STATEMENT
 EXECUTE FUNCTION public.concert_media_account_progress_guard();

-- Ordinary application paths take the common user lock BEFORE any event or
-- target root. These storage guards also exclude direct SQL recreation. Pure
-- erasure needs no old audience/event/place proof and cannot recreate content.
CREATE FUNCTION public.concert_media_account_fence_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE owner_user uuid;
BEGIN
 IF TG_OP='UPDATE' AND NEW.state='DELETED' AND NEW.staged IS NULL
    AND NEW.sanitized IS NULL AND NEW.width IS NULL AND NEW.height IS NULL
    AND NEW.target_id IS NULL THEN RETURN NEW; END IF;
 IF current_setting('transaction_isolation')<>'read committed' THEN
  RAISE EXCEPTION 'Media fence requires current post-lock snapshots' USING ERRCODE='23514';
 END IF;
 IF TG_OP='DELETE' THEN owner_user:=OLD.owner_id; ELSE owner_user:=NEW.owner_id; END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended('mcr-world-account-deletion:'||owner_user::text,0));
 IF EXISTS(SELECT 1 FROM public.concert_world_account_deletion_fences WHERE user_id=owner_user) THEN
  RAISE EXCEPTION 'Media account is permanently fenced' USING ERRCODE='23514';
 END IF;
 IF TG_OP='DELETE' THEN RETURN OLD; END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_media_account_fence_guard BEFORE INSERT OR UPDATE OR DELETE
 ON public.concert_media FOR EACH ROW EXECUTE FUNCTION public.concert_media_account_fence_guard();

CREATE FUNCTION public.concert_meeting_photo_account_fence_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF current_setting('transaction_isolation')<>'read committed' THEN
  RAISE EXCEPTION 'Photo fence requires current post-lock snapshots' USING ERRCODE='23514';
 END IF;
 IF TG_OP='UPDATE' AND (NEW.owner_id,NEW.event_id,NEW.beacon_id) IS DISTINCT FROM
   (OLD.owner_id,OLD.event_id,OLD.beacon_id) THEN
  RAISE EXCEPTION 'Photo account binding is immutable' USING ERRCODE='23514';
 END IF;
 PERFORM pg_advisory_xact_lock(hashtextextended('mcr-world-account-deletion:'||NEW.owner_id::text,0));
 IF EXISTS(SELECT 1 FROM public.concert_world_account_deletion_fences WHERE user_id=NEW.owner_id) THEN
  RAISE EXCEPTION 'Photo account is permanently fenced' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_meeting_photo_account_fence_guard BEFORE INSERT OR UPDATE
 ON public.concert_meeting_beacon_photos FOR EACH ROW
 EXECUTE FUNCTION public.concert_meeting_photo_account_fence_guard();
