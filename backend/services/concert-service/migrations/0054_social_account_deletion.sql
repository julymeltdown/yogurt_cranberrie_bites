-- BE42-SHARING-01. One leaf of the canonical Auth saga, never a Participant command.
-- 0052 owns the ONE common user fence. No event/participant cascade may remove
-- an incomplete deletion's remaining source; immutable leaf receipts survive it.
CREATE TABLE public.concert_social_account_deletions (
 saga_id uuid PRIMARY KEY,
 user_id uuid NOT NULL UNIQUE,
 actor_id uuid NOT NULL CHECK(actor_id<>'00000000-0000-0000-0000-000000000000'),
 target text NOT NULL DEFAULT 'SHARING' CHECK(target='SHARING'),
 retention_seconds bigint NOT NULL,
 retention_nanos integer NOT NULL CHECK(retention_nanos BETWEEN 0 AND 999999999),
 identity_binding bytea NOT NULL CHECK(octet_length(identity_binding)=32),
 command_binding bytea NOT NULL CHECK(octet_length(command_binding)=32),
 source_binding bytea NOT NULL CHECK(octet_length(source_binding)=32 AND source_binding<>decode(repeat('00',32),'hex')),
 fence_revision bigint NOT NULL CHECK(fence_revision=1),
 last_event uuid,
 active_event uuid,
 last_request uuid,
 progress_revision bigint NOT NULL DEFAULT 0 CHECK(progress_revision>=0),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 completed_at timestamptz,
 FOREIGN KEY(user_id,saga_id) REFERENCES public.concert_world_account_deletion_fences(user_id,saga_id),
 UNIQUE(saga_id,user_id,command_binding,source_binding,fence_revision),
 CHECK(last_request IS NULL OR active_event IS NOT NULL),
 CHECK(active_event IS NULL OR last_event IS NULL OR active_event>last_event)
);
CREATE TABLE public.concert_social_deletion_audience (
 saga_id uuid NOT NULL REFERENCES public.concert_social_account_deletions(saga_id),
 event_id uuid NOT NULL,
 actor_id uuid NOT NULL,
 policy_revision bigint NOT NULL CHECK(policy_revision BETWEEN 1 AND 9007199254740991),
 published_at timestamptz,
 PRIMARY KEY(saga_id,event_id,actor_id),
 UNIQUE(saga_id,event_id,actor_id,policy_revision)
);
-- No event FK on this source receipt: pruning a completed event cannot erase
-- which exact tuple actually withdrew its private view before acknowledgement.
ALTER TABLE public.concert_social_policy_signals
 ADD COLUMN deletion_saga uuid,
 ADD COLUMN deletion_user uuid,
 ADD COLUMN deletion_command_binding bytea,
 ADD COLUMN deletion_source_binding bytea,
 ADD COLUMN deletion_fence_revision bigint,
 ADD CONSTRAINT concert_social_signal_deletion_source FOREIGN KEY
 (deletion_saga,deletion_user,deletion_command_binding,deletion_source_binding,deletion_fence_revision)
 REFERENCES public.concert_social_account_deletions(saga_id,user_id,command_binding,source_binding,fence_revision),
 ADD CONSTRAINT concert_social_signal_deletion_audience FOREIGN KEY
 (deletion_saga,event_id,actor_id,policy_revision)
 REFERENCES public.concert_social_deletion_audience(saga_id,event_id,actor_id,policy_revision);
-- Drop only the old two-source CHECK, by its expression rather than relying on
-- PostgreSQL's unnamed constraint numbering. Preserve every other 0051 guard.
DO $$ DECLARE n text; BEGIN
 SELECT conname INTO STRICT n FROM pg_constraint
 WHERE conrelid='public.concert_social_policy_signals'::regclass AND contype='c'
 AND pg_get_constraintdef(oid) LIKE '%source_outbox IS NOT NULL%';
 EXECUTE format('ALTER TABLE public.concert_social_policy_signals DROP CONSTRAINT %I',n);
END $$;
ALTER TABLE public.concert_social_policy_signals ADD CONSTRAINT concert_social_signal_closed_source CHECK (
 (deletion_saga IS NULL AND deletion_user IS NULL AND deletion_command_binding IS NULL
  AND deletion_source_binding IS NULL AND deletion_fence_revision IS NULL AND (
   (source_outbox IS NOT NULL AND source_receipt IS NOT NULL AND source_operation IS NOT NULL
    AND source_resource IS NOT NULL AND source_revision IS NOT NULL AND deadline_request IS NULL AND deadline_at IS NULL)
   OR (source_outbox IS NULL AND source_receipt IS NULL AND source_operation IS NULL AND source_resource IS NULL
    AND source_revision IS NULL AND deadline_request IS NOT NULL AND deadline_at IS NOT NULL AND reason='EXPIRED')))
 OR (deletion_saga IS NOT NULL AND deletion_user IS NOT NULL AND deletion_command_binding IS NOT NULL
  AND deletion_source_binding IS NOT NULL AND deletion_fence_revision IS NOT NULL
  AND deletion_fence_revision=1 AND reason='CANONICAL_POLICY_CHANGED'
  AND source_outbox IS NULL AND source_receipt IS NULL AND source_operation IS NULL AND source_resource IS NULL
  AND source_revision IS NULL AND deadline_request IS NULL AND deadline_at IS NULL)
);
CREATE UNIQUE INDEX concert_social_deletion_signal_once ON public.concert_social_policy_signals(deletion_saga,event_id,actor_id)
 WHERE deletion_saga IS NOT NULL;
CREATE INDEX concert_social_deletion_pending ON public.concert_social_deletion_audience(saga_id,event_id,actor_id)
 WHERE published_at IS NULL;

CREATE FUNCTION public.concert_social_deletion_receipt_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP<>'UPDATE' THEN RAISE EXCEPTION 'Social deletion source cannot be removed' USING ERRCODE='23514'; END IF;
 IF NOT public.concert_social_deletion_context(OLD.user_id) THEN
  RAISE EXCEPTION 'Social deletion source deadline unavailable' USING ERRCODE='23514';
 END IF;
 IF OLD.completed_at IS NOT NULL OR
  (NEW.saga_id,NEW.user_id,NEW.actor_id,NEW.target,NEW.retention_seconds,NEW.retention_nanos,
   NEW.identity_binding,NEW.command_binding,NEW.source_binding,NEW.fence_revision,NEW.created_at)
  IS DISTINCT FROM
  (OLD.saga_id,OLD.user_id,OLD.actor_id,OLD.target,OLD.retention_seconds,OLD.retention_nanos,
   OLD.identity_binding,OLD.command_binding,OLD.source_binding,OLD.fence_revision,OLD.created_at)
  OR NEW.progress_revision<>OLD.progress_revision+1
  OR (OLD.last_event IS NOT NULL AND (NEW.last_event IS NULL OR NEW.last_event<OLD.last_event))
  OR (NEW.active_event=OLD.active_event AND OLD.last_request IS NOT NULL
      AND (NEW.last_request IS NULL OR NEW.last_request<OLD.last_request)) THEN
  RAISE EXCEPTION 'Social deletion source is immutable or nonmonotonic' USING ERRCODE='23514';
 END IF;
 IF NEW.completed_at IS NOT NULL AND (NEW.active_event IS NOT NULL
  OR EXISTS(SELECT 1 FROM public.concert_social_deletion_audience WHERE saga_id=NEW.saga_id AND published_at IS NULL)
  OR EXISTS(SELECT 1 FROM public.concert_social_handles WHERE user_id=NEW.user_id AND ciphertext IS NOT NULL)
  OR EXISTS(SELECT 1 FROM public.concert_social_requests WHERE NEW.user_id IN(requester_id,recipient_id) AND state IN('PENDING','APPROVED'))
  OR EXISTS(SELECT 1 FROM public.concert_social_grants WHERE NEW.user_id IN(owner_id,viewer_id) AND state='ACTIVE')) THEN
  RAISE EXCEPTION 'Social deletion is incomplete' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_social_deletion_receipt_immutable BEFORE UPDATE OR DELETE
 ON public.concert_social_account_deletions FOR EACH ROW EXECUTE FUNCTION public.concert_social_deletion_receipt_guard();
CREATE TRIGGER concert_social_deletion_receipt_no_truncate BEFORE TRUNCATE
 ON public.concert_social_account_deletions FOR EACH STATEMENT EXECUTE FUNCTION public.concert_social_deletion_receipt_guard();
CREATE FUNCTION public.concert_social_deletion_audience_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP<>'UPDATE' THEN RAISE EXCEPTION 'Social deletion audience cannot be removed' USING ERRCODE='23514'; END IF;
 IF (NEW.saga_id,NEW.event_id,NEW.actor_id,NEW.policy_revision) IS DISTINCT FROM
    (OLD.saga_id,OLD.event_id,OLD.actor_id,OLD.policy_revision)
    OR OLD.published_at IS NOT NULL OR NEW.published_at IS NULL THEN
  RAISE EXCEPTION 'Social deletion audience is immutable' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_social_deletion_audience_immutable BEFORE UPDATE OR DELETE
 ON public.concert_social_deletion_audience FOR EACH ROW EXECUTE FUNCTION public.concert_social_deletion_audience_guard();
CREATE TRIGGER concert_social_deletion_audience_no_truncate BEFORE TRUNCATE
 ON public.concert_social_deletion_audience FOR EACH STATEMENT EXECUTE FUNCTION public.concert_social_deletion_audience_guard();

-- A trusted transaction may perform only removal effects under its actual
-- verified deletion receipt and ORIGINAL deadline. Never mint ordinary authority.
CREATE FUNCTION public.concert_social_deletion_context(subject uuid) RETURNS boolean LANGUAGE sql STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM public.concert_social_account_deletions d
 JOIN public.concert_world_account_deletion_fences f USING(user_id,saga_id)
 WHERE d.user_id=subject AND d.target='SHARING' AND d.completed_at IS NULL
 AND d.saga_id::text=nullif(current_setting('mcr.social_deletion_saga',true),'')
 AND encode(d.source_binding,'hex')=nullif(current_setting('mcr.social_deletion_source',true),'')
 AND d.identity_binding=f.identity_binding AND d.fence_revision=f.fence_revision
 AND clock_timestamp()<nullif(current_setting('mcr.social_deletion_until',true),'')::timestamptz)
$$;
CREATE OR REPLACE FUNCTION public.concert_relationship_deadline_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE deadline timestamptz; permitted boolean:=false;
BEGIN
 IF TG_TABLE_NAME='concert_social_handles' THEN
  permitted:=public.concert_social_deletion_context(NEW.user_id) AND TG_OP='UPDATE'
   AND NEW.key_id IS NULL AND NEW.nonce IS NULL AND NEW.ciphertext IS NULL;
 ELSIF TG_TABLE_NAME='concert_social_requests' THEN
  permitted:=(public.concert_social_deletion_context(NEW.requester_id) OR public.concert_social_deletion_context(NEW.recipient_id))
   AND TG_OP='UPDATE' AND NEW.state='REVOKED';
 ELSIF TG_TABLE_NAME='concert_social_grants' THEN
  permitted:=(public.concert_social_deletion_context(NEW.owner_id) OR public.concert_social_deletion_context(NEW.viewer_id))
   AND TG_OP='UPDATE' AND NEW.state='REVOKED';
 END IF;
 IF permitted THEN RETURN NULL; END IF;
 deadline:=nullif(current_setting('mcr.relationship_authority_until',true),'')::timestamptz;
 IF deadline IS NULL OR clock_timestamp()>=deadline THEN
  RAISE EXCEPTION 'relationship authority expired' USING ERRCODE='23514';
 END IF;
 IF TG_TABLE_NAME IN('concert_social_requests','concert_social_grants') THEN
  IF NEW.state IN('PENDING','APPROVED','ACTIVE') AND NEW.expires_at<=clock_timestamp() THEN
   RAISE EXCEPTION 'social authorization expired' USING ERRCODE='23514';
  END IF;
 END IF;
 RETURN NULL;
END $$;

CREATE FUNCTION public.concert_social_permanent_deletion_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE users uuid[]; who uuid; blocked boolean; allowed boolean:=false;
BEGIN
 IF TG_TABLE_NAME='concert_social_handles' THEN
  IF TG_OP='DELETE' THEN users:=ARRAY[OLD.user_id]; ELSE users:=ARRAY[NEW.user_id]; END IF;
 ELSIF TG_TABLE_NAME='concert_social_requests' THEN
  IF TG_OP='DELETE' THEN users:=ARRAY[OLD.requester_id,OLD.recipient_id]; ELSE users:=ARRAY[NEW.requester_id,NEW.recipient_id]; END IF;
 ELSIF TG_TABLE_NAME='concert_social_grants' THEN
  IF TG_OP='DELETE' THEN users:=ARRAY[OLD.owner_id,OLD.viewer_id]; ELSE users:=ARRAY[NEW.owner_id,NEW.viewer_id]; END IF;
 ELSE
  IF TG_OP='DELETE' THEN
   SELECT ARRAY[requester_id,recipient_id] INTO users FROM public.concert_social_requests
    WHERE event_id=OLD.event_id AND request_id=OLD.request_id;
  ELSE
   SELECT ARRAY[requester_id,recipient_id] INTO users FROM public.concert_social_requests
    WHERE event_id=NEW.event_id AND request_id=NEW.request_id;
  END IF;
 END IF;
 -- Verified removals already hold the common deleted-user lock FIRST. Do
 -- not acquire a peer's common lock after the event lock: opposite deletions
 -- and ordinary sorted pair writers must never introduce a lock-order cycle.
 IF TG_OP='UPDATE' AND EXISTS(SELECT 1 FROM unnest(users) u WHERE public.concert_social_deletion_context(u)) THEN
  IF TG_TABLE_NAME='concert_social_handles' THEN
   allowed:=NEW.key_id IS NULL AND NEW.nonce IS NULL AND NEW.ciphertext IS NULL;
  ELSIF TG_TABLE_NAME IN('concert_social_requests','concert_social_grants') THEN
   allowed:=NEW.state='REVOKED';
  END IF;
 END IF;
 FOR who IN SELECT DISTINCT x FROM unnest(users) x ORDER BY x LOOP
  IF NOT allowed AND NOT pg_try_advisory_xact_lock(hashtextextended('mcr-world-account-deletion:'||who::text,0)) THEN
   RAISE EXCEPTION 'Social deletion lock unavailable' USING ERRCODE='55P03';
  END IF;
  SELECT EXISTS(SELECT 1 FROM public.concert_world_account_deletion_fences WHERE user_id=who) INTO blocked;
  IF NOT blocked THEN CONTINUE; END IF;
  IF TG_OP='DELETE' THEN
   IF EXISTS(SELECT 1 FROM public.concert_social_account_deletions WHERE user_id=who AND completed_at IS NOT NULL) THEN CONTINUE; END IF;
  ELSIF allowed THEN
   CONTINUE;
  END IF;
  RAISE EXCEPTION 'canonical deleted user is fenced' USING ERRCODE='23514';
 END LOOP;
 IF TG_OP='DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
END $$;
CREATE TRIGGER concert_social_handles_deleted BEFORE INSERT OR UPDATE OR DELETE ON public.concert_social_handles
 FOR EACH ROW EXECUTE FUNCTION public.concert_social_permanent_deletion_guard();
CREATE TRIGGER concert_social_requests_deleted BEFORE INSERT OR UPDATE OR DELETE ON public.concert_social_requests
 FOR EACH ROW EXECUTE FUNCTION public.concert_social_permanent_deletion_guard();
CREATE TRIGGER concert_social_grants_deleted BEFORE INSERT OR UPDATE OR DELETE ON public.concert_social_grants
 FOR EACH ROW EXECUTE FUNCTION public.concert_social_permanent_deletion_guard();
CREATE TRIGGER concert_social_bindings_deleted BEFORE INSERT OR UPDATE OR DELETE ON public.concert_social_bindings
 FOR EACH ROW EXECUTE FUNCTION public.concert_social_permanent_deletion_guard();

CREATE FUNCTION public.concert_social_deletion_signal_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' THEN
  IF OLD.deletion_saga IS NOT NULL AND OLD.published_at IS NULL THEN
   RAISE EXCEPTION 'pending deletion publication cannot be cascaded away' USING ERRCODE='23514';
  END IF;
  RETURN OLD;
 END IF;
 IF (OLD.deletion_saga IS NOT NULL OR NEW.deletion_saga IS NOT NULL) AND
  (NEW.signal_id,NEW.event_id,NEW.actor_id,NEW.policy_revision,NEW.reason,NEW.deletion_saga,NEW.deletion_user,
   NEW.deletion_command_binding,NEW.deletion_source_binding,NEW.deletion_fence_revision,NEW.created_at)
  IS DISTINCT FROM
  (OLD.signal_id,OLD.event_id,OLD.actor_id,OLD.policy_revision,OLD.reason,OLD.deletion_saga,OLD.deletion_user,
   OLD.deletion_command_binding,OLD.deletion_source_binding,OLD.deletion_fence_revision,OLD.created_at) THEN
  RAISE EXCEPTION 'deletion publication source is immutable' USING ERRCODE='23514';
 END IF;
 IF OLD.deletion_saga IS NOT NULL AND OLD.published_at IS NOT NULL THEN
  RAISE EXCEPTION 'deletion publication is immutable' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_social_deletion_signal_guard BEFORE UPDATE OR DELETE ON public.concert_social_policy_signals
 FOR EACH ROW EXECUTE FUNCTION public.concert_social_deletion_signal_guard();

-- TRUNCATE does not fire row guards, including when a parent uses CASCADE.
-- Refuse wholesale removal while it could erase pending discovery/publication.
CREATE FUNCTION public.concert_social_deletion_truncate_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF EXISTS(SELECT 1 FROM public.concert_world_account_deletion_fences f
           LEFT JOIN public.concert_social_account_deletions d USING(user_id,saga_id)
           WHERE d.completed_at IS NULL)
  OR EXISTS(SELECT 1 FROM public.concert_social_policy_signals WHERE deletion_saga IS NOT NULL) THEN
  RAISE EXCEPTION 'Social deletion source cannot be truncated' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END $$;
CREATE TRIGGER concert_social_handles_deletion_no_truncate BEFORE TRUNCATE ON public.concert_social_handles
 FOR EACH STATEMENT EXECUTE FUNCTION public.concert_social_deletion_truncate_guard();
CREATE TRIGGER concert_social_requests_deletion_no_truncate BEFORE TRUNCATE ON public.concert_social_requests
 FOR EACH STATEMENT EXECUTE FUNCTION public.concert_social_deletion_truncate_guard();
CREATE TRIGGER concert_social_grants_deletion_no_truncate BEFORE TRUNCATE ON public.concert_social_grants
 FOR EACH STATEMENT EXECUTE FUNCTION public.concert_social_deletion_truncate_guard();
CREATE TRIGGER concert_social_bindings_deletion_no_truncate BEFORE TRUNCATE ON public.concert_social_bindings
 FOR EACH STATEMENT EXECUTE FUNCTION public.concert_social_deletion_truncate_guard();
CREATE TRIGGER concert_social_signals_deletion_no_truncate BEFORE TRUNCATE ON public.concert_social_policy_signals
 FOR EACH STATEMENT EXECUTE FUNCTION public.concert_social_deletion_truncate_guard();

CREATE INDEX concert_social_deletion_requester ON public.concert_social_requests(requester_id,event_id,request_id);
CREATE INDEX concert_social_deletion_recipient ON public.concert_social_requests(recipient_id,event_id,request_id);
CREATE INDEX concert_social_deletion_handles ON public.concert_social_handles(user_id,event_id);
CREATE INDEX concert_social_deletion_grant_owner ON public.concert_social_grants(owner_id,event_id,request_id) WHERE state='ACTIVE';
CREATE INDEX concert_social_deletion_grant_viewer ON public.concert_social_grants(viewer_id,event_id,request_id) WHERE state='ACTIVE';
