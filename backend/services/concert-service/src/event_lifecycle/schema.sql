-- BE41 candidate. Delivery must assign/copy to one forward-only migration.
-- No seed identities, operator roles or retention values.
CREATE TABLE public.concert_closure_jobs (
 job_id uuid PRIMARY KEY,event_id uuid NOT NULL UNIQUE REFERENCES public.concert_events(event_id),
 actor_id uuid NOT NULL,start_key uuid NOT NULL,original_event_revision bigint NOT NULL CHECK(original_event_revision>0),
 retention_revision bigint NOT NULL CHECK(retention_revision>0),closed_at timestamptz NOT NULL,cleanup_after timestamptz NOT NULL CHECK(cleanup_after>=closed_at),
 revision bigint NOT NULL DEFAULT 1 CHECK(revision BETWEEN 1 AND 9007199254740991),
 progress jsonb NOT NULL CHECK(jsonb_typeof(progress)='object'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(event_id,job_id),UNIQUE(event_id,actor_id,start_key),
 CHECK((progress->>'job_id')::uuid=job_id AND (progress->>'event_id')::uuid=event_id AND (progress->>'revision')::bigint=revision),
 CHECK(progress->>'state' IN ('PENDING','RUNNING','PARTIAL_FAILURE','COMPLETED')),
 CHECK(jsonb_array_length(progress->'steps')=7)
);
CREATE TRIGGER concert_closure_revision BEFORE INSERT OR UPDATE ON public.concert_closure_jobs FOR EACH ROW EXECUTE FUNCTION public.concert_guard_revision('job_id','event_id','actor_id','start_key','original_event_revision','retention_revision','closed_at','cleanup_after');
CREATE TABLE public.concert_closure_effects (
 event_id uuid NOT NULL,job_id uuid NOT NULL,step text NOT NULL CHECK(step IN ('READ_ONLY','FREEZE_MURAL','SNAPSHOT','EXPIRE_GRANTS','EXPIRE_BEACONS','EXPORT','CLEANUP')),
 owner_receipt uuid NOT NULL,owner_revision bigint NOT NULL CHECK(owner_revision BETWEEN 1 AND 9007199254740991),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),PRIMARY KEY(job_id,step),
 FOREIGN KEY(event_id,job_id) REFERENCES public.concert_closure_jobs(event_id,job_id)
);
CREATE TABLE public.concert_closure_audit (
 audit_id uuid PRIMARY KEY,event_id uuid NOT NULL,job_id uuid NOT NULL,actor_id uuid NOT NULL,
 command_key uuid,step text,outcome text NOT NULL CHECK(outcome IN ('ACCEPTED','SUCCEEDED','FAILED')),
 job_revision bigint NOT NULL CHECK(job_revision>0),created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 FOREIGN KEY(event_id,job_id) REFERENCES public.concert_closure_jobs(event_id,job_id)
);
CREATE FUNCTION public.concert_closure_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'closure evidence is append only' USING ERRCODE='23514'; END; $$;
CREATE TRIGGER concert_closure_effects_immutable BEFORE UPDATE OR DELETE ON public.concert_closure_effects FOR EACH ROW EXECUTE FUNCTION public.concert_closure_immutable();
CREATE TRIGGER concert_closure_audit_immutable BEFORE UPDATE OR DELETE ON public.concert_closure_audit FOR EACH ROW EXECUTE FUNCTION public.concert_closure_immutable();
CREATE FUNCTION public.concert_closure_deadline() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE deadline timestamptz;
BEGIN
 IF TG_TABLE_NAME='concert_command_receipts' THEN
  IF NEW.operation_id<>'closeConcertEvent' THEN RETURN NULL; END IF;
  IF TG_OP='UPDATE' THEN
   IF NEW.state='RETIRED' AND OLD.expires_at<=clock_timestamp() THEN RETURN NULL; END IF;
  END IF;
 END IF;
 deadline:=nullif(current_setting('mcr.lifecycle_authority_until',true),'')::timestamptz;
 IF deadline IS NULL OR clock_timestamp()>=deadline THEN RAISE EXCEPTION 'closure authority expired' USING ERRCODE='23514'; END IF;
 RETURN NULL;
END; $$;
CREATE CONSTRAINT TRIGGER concert_closure_jobs_deadline AFTER INSERT OR UPDATE ON public.concert_closure_jobs DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_closure_deadline();
CREATE CONSTRAINT TRIGGER concert_closure_effects_deadline AFTER INSERT ON public.concert_closure_effects DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_closure_deadline();
CREATE CONSTRAINT TRIGGER concert_closure_audit_deadline AFTER INSERT ON public.concert_closure_audit DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_closure_deadline();
CREATE CONSTRAINT TRIGGER concert_closure_receipts_deadline AFTER INSERT OR UPDATE ON public.concert_command_receipts DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_closure_deadline();

-- Contains only approved anonymous final mural. Post text/media remain owner reads.
CREATE TABLE public.concert_closure_archives (
 event_id uuid PRIMARY KEY,job_id uuid NOT NULL,revision bigint NOT NULL DEFAULT 1 CHECK(revision BETWEEN 1 AND 9007199254740991),
 source_revision bigint NOT NULL CHECK(source_revision BETWEEN 1 AND 9007199254740991),
 final_mural jsonb NOT NULL CHECK(jsonb_typeof(final_mural)='object'),
 FOREIGN KEY(event_id,job_id) REFERENCES public.concert_closure_jobs(event_id,job_id),
 CHECK((final_mural->>'event_id')::uuid=event_id AND final_mural->>'state'='FROZEN')
);
CREATE FUNCTION public.concert_closure_progress_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE names text[]:=ARRAY['READ_ONLY','FREEZE_MURAL','SNAPSHOT','EXPIRE_GRANTS','EXPIRE_BEACONS','EXPORT','CLEANUP'];
 item jsonb;old_item jsonb;i integer;unfinished boolean:=false;failed boolean:=false;state text;changed integer:=0;
BEGIN
 IF NEW.progress-ARRAY['job_id','event_id','state','revision','steps']<>'{}'::jsonb OR NOT NEW.progress ?& ARRAY['job_id','event_id','state','revision','steps'] THEN RAISE EXCEPTION 'invalid closure projection'; END IF;
 FOR i IN 0..6 LOOP
  item:=NEW.progress->'steps'->i;
  IF NOT item ?& ARRAY['name','state'] OR item-ARRAY['name','state']<>'{}'::jsonb OR item->>'name'<>names[i+1] OR item->>'state' NOT IN ('PENDING','SUCCEEDED','FAILED') THEN RAISE EXCEPTION 'invalid closure step'; END IF;
  IF i=0 AND item->>'state'<>'SUCCEEDED' THEN RAISE EXCEPTION 'read-only is required'; END IF;
  IF unfinished AND item->>'state' IN ('SUCCEEDED','FAILED') THEN RAISE EXCEPTION 'out of order closure success'; END IF;
  IF failed AND item->>'state'='FAILED' THEN RAISE EXCEPTION 'multiple failed boundaries'; END IF;
  unfinished:=unfinished OR item->>'state'<>'SUCCEEDED';failed:=failed OR item->>'state'='FAILED';
  IF TG_OP='INSERT' THEN
   IF i>0 AND item->>'state'<>'PENDING' THEN RAISE EXCEPTION 'unapplied initial step'; END IF;
  ELSE
   old_item:=OLD.progress->'steps'->i;
   IF OLD.progress->>'state'='PARTIAL_FAILURE' AND old_item IS DISTINCT FROM item AND NOT (old_item->>'state'='FAILED' AND item->>'state'='PENDING' AND NEW.progress->>'state'='PENDING') THEN RAISE EXCEPTION 'failure requires explicit resume admission'; END IF;
   IF old_item->>'state'='SUCCEEDED' AND item->>'state'<>'SUCCEEDED' THEN RAISE EXCEPTION 'completed step is immutable'; END IF;
   IF old_item IS DISTINCT FROM item THEN changed:=changed+1; END IF;
  END IF;
  IF i>0 AND item->>'state'='SUCCEEDED' AND NOT EXISTS(SELECT 1 FROM public.concert_closure_effects e WHERE e.job_id=NEW.job_id AND e.step=names[i+1]) THEN RAISE EXCEPTION 'missing applied owner evidence'; END IF;
 END LOOP;
 state:=NEW.progress->>'state';
 IF (state='COMPLETED')<>(NOT unfinished) OR (state='PARTIAL_FAILURE')<>failed THEN RAISE EXCEPTION 'false closure result'; END IF;
 IF TG_OP='INSERT' AND state<>'PENDING' THEN RAISE EXCEPTION 'invalid initial job'; END IF;
 IF TG_OP='UPDATE' AND changed<>1 THEN RAISE EXCEPTION 'one closure boundary per commit'; END IF;
 RETURN NEW;
END; $$;
CREATE TRIGGER concert_closure_progress_guard BEFORE INSERT OR UPDATE ON public.concert_closure_jobs FOR EACH ROW EXECUTE FUNCTION public.concert_closure_progress_guard();
CREATE FUNCTION public.concert_closure_phase_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE phase text;
BEGIN
 SELECT e.phase INTO phase FROM public.concert_events e WHERE e.event_id=NEW.event_id;
 IF NOT EXISTS(SELECT 1 FROM public.concert_closure_effects WHERE job_id=NEW.job_id AND step='READ_ONLY') THEN RAISE EXCEPTION 'missing admission barrier evidence'; END IF;
 IF phase IS DISTINCT FROM (CASE WHEN NEW.progress->>'state'='COMPLETED' THEN 'ARCHIVED' ELSE 'READ_ONLY' END) THEN RAISE EXCEPTION 'closure phase disagrees with job'; END IF;
 RETURN NULL;
END; $$;
CREATE CONSTRAINT TRIGGER concert_closure_phase_guard AFTER INSERT OR UPDATE ON public.concert_closure_jobs DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_closure_phase_guard();
CREATE FUNCTION public.concert_closure_archive_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='INSERT' THEN
  IF NEW.revision<>1 THEN RAISE EXCEPTION 'initial archive revision'; END IF;
 ELSE
  IF (NEW.event_id,NEW.job_id,NEW.final_mural) IS DISTINCT FROM (OLD.event_id,OLD.job_id,OLD.final_mural)
     OR NEW.revision<>OLD.revision+1 OR NEW.source_revision<=OLD.source_revision THEN
   RAISE EXCEPTION 'immutable mural or invalid archive revision';
  END IF;
 END IF;
 RETURN NEW;
END; $$;
CREATE TRIGGER concert_closure_archive_guard BEFORE INSERT OR UPDATE ON public.concert_closure_archives FOR EACH ROW EXECUTE FUNCTION public.concert_closure_archive_guard();
-- Component receipts are actual local SQL effects, not remote success assertions.
CREATE TABLE public.concert_closure_privacy_effects (
 event_id uuid NOT NULL,job_id uuid NOT NULL,operation_id uuid PRIMARY KEY,
 component text NOT NULL CHECK(component IN ('SOCIAL_EXPIRY','BEACON_EXPIRY','PRIVACY_RETENTION')),
 affected bigint NOT NULL CHECK(affected>=0),created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 FOREIGN KEY(event_id,job_id) REFERENCES public.concert_closure_jobs(event_id,job_id),UNIQUE(job_id,component)
);
CREATE TRIGGER concert_closure_privacy_effects_immutable BEFORE UPDATE OR DELETE ON public.concert_closure_privacy_effects FOR EACH ROW EXECUTE FUNCTION public.concert_closure_immutable();
CREATE CONSTRAINT TRIGGER concert_closure_privacy_effects_deadline AFTER INSERT ON public.concert_closure_privacy_effects DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_closure_deadline();
