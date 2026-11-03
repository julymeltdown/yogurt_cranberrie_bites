-- BE36/38 only. No role assignments or real place/operator facts are seeded.
CREATE TABLE public.concert_reports (
 report_id uuid PRIMARY KEY, event_id uuid NOT NULL REFERENCES public.concert_events(event_id),
 reporter_id uuid NOT NULL, target_type text NOT NULL CHECK(target_type IN ('USER','POST','COMMENT','PARTY','GIVEAWAY','MURAL')),
 target_id uuid NOT NULL, reason text NOT NULL CHECK(reason IN ('HARASSMENT','PRIVACY','UNSAFE_MEETING','SPAM','OTHER')),
 text text NOT NULL CHECK(char_length(text)<=2000), evidence_media_ids uuid[] NOT NULL CHECK(cardinality(evidence_media_ids)<=4),
 state text NOT NULL DEFAULT 'OPEN' CHECK(state IN ('OPEN','IN_REVIEW','RESOLVED','DISMISSED')),
 revision bigint NOT NULL DEFAULT 1 CHECK(revision BETWEEN 1 AND 9007199254740991),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),updated_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE UNIQUE INDEX concert_reports_active_duplicate ON public.concert_reports(event_id,reporter_id,target_type,target_id) WHERE state IN ('OPEN','IN_REVIEW');
CREATE INDEX concert_reports_own ON public.concert_reports(event_id,reporter_id,report_id);
CREATE INDEX concert_reports_queue ON public.concert_reports(event_id,report_id);
CREATE TRIGGER concert_reports_revision BEFORE INSERT OR UPDATE ON public.concert_reports FOR EACH ROW EXECUTE FUNCTION public.concert_guard_revision('event_id','report_id','reporter_id','target_type','target_id');
CREATE TABLE public.concert_event_settings (
 event_id uuid PRIMARY KEY REFERENCES public.concert_events(event_id),
 parties boolean NOT NULL,giveaways boolean NOT NULL,intros boolean NOT NULL,social boolean NOT NULL,mural boolean NOT NULL,
 beacon_ttl_seconds bigint NOT NULL CHECK(beacon_ttl_seconds BETWEEN 1 AND 900),
 pickup_grace_seconds bigint NOT NULL CHECK(pickup_grace_seconds BETWEEN 0 AND 86400),
 social_grant_ttl_seconds bigint NOT NULL CHECK(social_grant_ttl_seconds BETWEEN 1 AND 86400),
 mural_cooldown_ms bigint NOT NULL CHECK(mural_cooldown_ms BETWEEN 0 AND 3600000)
);
-- BE38 owns notice storage. Feed consumers must read this projection after current authority.
CREATE TABLE public.concert_operation_notices (
 notice_id uuid PRIMARY KEY,event_id uuid NOT NULL REFERENCES public.concert_events(event_id),
 title text NOT NULL CHECK(char_length(title) BETWEEN 1 AND 120),text text NOT NULL CHECK(char_length(text) BETWEEN 1 AND 4000),
 source_uri text NOT NULL CHECK(source_uri ~ '^https://' AND char_length(source_uri)<=2048),
 confirmed_at timestamptz NOT NULL CHECK(confirmed_at<=statement_timestamp()),
 state text NOT NULL CHECK(state IN ('DRAFT','PUBLISHED','WITHDRAWN')),
 revision bigint NOT NULL DEFAULT 1 CHECK(revision BETWEEN 1 AND 9007199254740991),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),updated_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE INDEX concert_operation_notices_public ON public.concert_operation_notices(event_id,notice_id) WHERE state='PUBLISHED';
CREATE TRIGGER concert_operation_notices_revision BEFORE INSERT OR UPDATE ON public.concert_operation_notices FOR EACH ROW EXECUTE FUNCTION public.concert_guard_revision('event_id','notice_id');
CREATE TABLE public.concert_operations_audit (
 audit_id uuid PRIMARY KEY,event_id uuid NOT NULL REFERENCES public.concert_events(event_id),actor_id uuid NOT NULL,
 operation_id text NOT NULL,target_id uuid NOT NULL,receipt_id uuid REFERENCES public.concert_command_receipts(receipt_id),
 outcome text NOT NULL CHECK(outcome='SUCCEEDED'),change jsonb NOT NULL CHECK(jsonb_typeof(change)='object'),created_at timestamptz NOT NULL DEFAULT clock_timestamp()
);
CREATE FUNCTION public.concert_operations_deadline_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE deadline timestamptz;
BEGIN
 IF TG_TABLE_NAME='concert_command_receipts' THEN
  IF NEW.operation_id NOT IN ('createConcertReport','setConcertSettings','setConcertEventPhase','setConcertPlace','createConcertNotice','updateConcertNotice') THEN RETURN NULL; END IF;
  -- BE15 immutable correlation and shape guards still apply. Expired retirement
  -- clears result metadata and executes no protected user/domain effect.
  IF TG_OP='UPDATE' THEN
   IF NEW.state='RETIRED' AND OLD.expires_at<=clock_timestamp()
      AND NEW.resource_id IS NULL AND NEW.resource_revision IS NULL
      AND NEW.failure_reason IS NULL AND NEW.result_status IS NULL THEN RETURN NULL; END IF;
  END IF;
 END IF;
 deadline:=nullif(current_setting('mcr.operations_authority_until',true),'')::timestamptz;
 IF deadline IS NULL OR clock_timestamp()>=deadline THEN RAISE EXCEPTION 'operations authority expired' USING ERRCODE='23514'; END IF;
 RETURN NULL;
END;
$$;
CREATE CONSTRAINT TRIGGER concert_reports_deadline AFTER INSERT OR UPDATE ON public.concert_reports DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_operations_deadline_guard();
CREATE CONSTRAINT TRIGGER concert_event_settings_deadline AFTER INSERT OR UPDATE ON public.concert_event_settings DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_operations_deadline_guard();
CREATE CONSTRAINT TRIGGER concert_operation_notices_deadline AFTER INSERT OR UPDATE ON public.concert_operation_notices DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_operations_deadline_guard();
CREATE CONSTRAINT TRIGGER concert_operations_audit_deadline AFTER INSERT ON public.concert_operations_audit DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_operations_deadline_guard();
CREATE CONSTRAINT TRIGGER concert_operations_receipt_deadline AFTER INSERT OR UPDATE ON public.concert_command_receipts DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_operations_deadline_guard();
CREATE FUNCTION public.concert_operations_audit_immutable() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'operations audit is append only' USING ERRCODE='23514'; END; $$;
CREATE TRIGGER concert_operations_audit_immutable BEFORE UPDATE OR DELETE ON public.concert_operations_audit FOR EACH ROW EXECUTE FUNCTION public.concert_operations_audit_immutable();

-- Once BE38 mutates a release-local closure, bind that release durably to one
-- event. Existing never-operated shared fixtures remain readable, while future
-- publication cannot inherit another event's administrative closure/revision.
CREATE TABLE public.concert_operated_place_releases (
 map_id text NOT NULL,map_revision bigint NOT NULL,event_id uuid NOT NULL REFERENCES public.concert_events(event_id),
 PRIMARY KEY(map_id,map_revision),FOREIGN KEY(map_id,map_revision) REFERENCES public.concert_map_releases(map_id,map_revision)
);
CREATE FUNCTION public.concert_operated_place_binding_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE owner_event uuid;
BEGIN
 PERFORM pg_advisory_xact_lock(hashtextextended(NEW.map_id::text||':'||NEW.map_revision::text,0));
 SELECT event_id INTO owner_event FROM public.concert_operated_place_releases WHERE map_id=NEW.map_id AND map_revision=NEW.map_revision;
 IF owner_event IS NOT NULL AND owner_event<>NEW.event_id THEN
  RAISE EXCEPTION 'operated release belongs to another event' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END;
$$;
CREATE TRIGGER concert_operated_place_binding_guard BEFORE INSERT OR UPDATE ON public.concert_events_map_releases FOR EACH ROW EXECUTE FUNCTION public.concert_operated_place_binding_guard();
CREATE CONSTRAINT TRIGGER concert_operated_place_deadline AFTER INSERT OR UPDATE ON public.concert_operated_place_releases DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_operations_deadline_guard();
CREATE FUNCTION public.concert_operated_place_owner_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'operated place ownership is immutable' USING ERRCODE='23514'; END;
$$;
CREATE TRIGGER concert_operated_place_owner_immutable BEFORE UPDATE OR DELETE ON public.concert_operated_place_releases FOR EACH ROW EXECUTE FUNCTION public.concert_operated_place_owner_immutable();
CREATE FUNCTION public.concert_operated_place_owner_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 PERFORM pg_advisory_xact_lock(hashtextextended(NEW.map_id::text||':'||NEW.map_revision::text,0));
 IF EXISTS(SELECT 1 FROM public.concert_events_map_releases WHERE map_id=NEW.map_id AND map_revision=NEW.map_revision AND event_id<>NEW.event_id) THEN
  RAISE EXCEPTION 'shared release cannot acquire an event operator owner' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END;
$$;
CREATE TRIGGER concert_operated_place_owner_guard BEFORE INSERT ON public.concert_operated_place_releases FOR EACH ROW EXECUTE FUNCTION public.concert_operated_place_owner_guard();
