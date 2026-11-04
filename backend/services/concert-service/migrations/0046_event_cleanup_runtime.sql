-- BE41 local progress only. No roles, policy defaults, foreign-service SQL or
-- synthetic downstream receipts. Opaque cursors never enter public job/audit DTOs.
CREATE TABLE public.concert_event_cleanup_progress (
 job_id uuid NOT NULL, event_id uuid NOT NULL, actor_id uuid NOT NULL,
 step text NOT NULL, owner text NOT NULL,
 batch_limit smallint NOT NULL CHECK(batch_limit BETWEEN 1 AND 100),
 ordinal bigint NOT NULL DEFAULT 0 CHECK(ordinal>=0),
 cursor bytea CHECK(octet_length(cursor) BETWEEN 1 AND 2048),
 processed bigint NOT NULL DEFAULT 0 CHECK(processed>=ordinal),
 state text NOT NULL DEFAULT 'PENDING' CHECK(state IN ('PENDING','PARTIAL_FAILURE','COMPLETE')),
 failure text CHECK(failure IN ('AUTHORITY_UNAVAILABLE','OWNER_UNAVAILABLE','OWNER_DENIED','SOURCE_CHANGED')),
 PRIMARY KEY(job_id,step,owner),
 UNIQUE(job_id,event_id,actor_id,step,owner),
 FOREIGN KEY(event_id,job_id) REFERENCES public.concert_closure_jobs(event_id,job_id),
 CHECK((ordinal=0)=(cursor IS NULL)),
 CHECK((state='PARTIAL_FAILURE')=(failure IS NOT NULL)),
 CHECK((step='FREEZE_MURAL' AND owner='MURAL')
 OR (step='SNAPSHOT' AND owner='PUBLIC_SNAPSHOT')
 OR (step='EXPIRE_GRANTS' AND owner='SOCIAL_GRANTS')
 OR (step='EXPIRE_BEACONS' AND owner IN ('BEACONS','WORLD','COMMUNITY'))
 OR (step='EXPORT' AND owner='PUBLIC_EXPORT')
 OR (step='CLEANUP' AND owner IN ('PRIVATE_RETENTION','POST_RETENTION','MEDIA_RETENTION','WORLD','COMMUNITY')))
);
CREATE TABLE public.concert_event_cleanup_batches (
 operation_id uuid PRIMARY KEY CHECK(operation_id<>'00000000-0000-0000-0000-000000000000'),
 job_id uuid NOT NULL,event_id uuid NOT NULL,actor_id uuid NOT NULL,step text NOT NULL,owner text NOT NULL,
 ordinal bigint NOT NULL CHECK(ordinal>=0),binding bytea NOT NULL CHECK(octet_length(binding)=32),
 cursor_in bytea CHECK(octet_length(cursor_in) BETWEEN 1 AND 2048),
 cursor_out bytea CHECK(octet_length(cursor_out) BETWEEN 1 AND 2048),
 outcome text NOT NULL CHECK(outcome IN ('PAGE','CONVERGED')),
 receipt_id uuid NOT NULL CHECK(receipt_id<>'00000000-0000-0000-0000-000000000000'),
 owner_revision bigint NOT NULL CHECK(owner_revision BETWEEN 1 AND 9007199254740991),
 processed smallint NOT NULL CHECK(processed BETWEEN 0 AND 100),
 cumulative bigint NOT NULL CHECK(cumulative>=processed),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 UNIQUE(job_id,step,owner,ordinal),
 FOREIGN KEY(job_id,event_id,actor_id,step,owner) REFERENCES public.concert_event_cleanup_progress(job_id,event_id,actor_id,step,owner),
 CHECK((ordinal=0)=(cursor_in IS NULL)),
 CHECK((outcome='PAGE')=(cursor_out IS NOT NULL)),
 CHECK(outcome<>'PAGE' OR (processed>0 AND cursor_out IS DISTINCT FROM cursor_in))
);
CREATE FUNCTION public.concert_event_cleanup_scope() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE j public.concert_closure_jobs; phase text; next_step text; deadline timestamptz;
BEGIN
 deadline:=nullif(current_setting('mcr.lifecycle_authority_until',true),'')::timestamptz;
 IF deadline IS NULL OR clock_timestamp()>=deadline THEN RAISE EXCEPTION 'cleanup authority expired' USING ERRCODE='23514'; END IF;
 SELECT e.phase INTO phase FROM public.concert_events e WHERE e.event_id=NEW.event_id;
 SELECT * INTO j FROM public.concert_closure_jobs WHERE job_id=NEW.job_id AND event_id=NEW.event_id;
 SELECT s->>'name' INTO next_step FROM jsonb_array_elements(j.progress->'steps') WITH ORDINALITY AS x(s,n) WHERE s->>'state'<>'SUCCEEDED' ORDER BY n LIMIT 1;
 IF phase IS DISTINCT FROM 'READ_ONLY' OR j.actor_id IS DISTINCT FROM NEW.actor_id
 OR j.progress->>'state' NOT IN ('PENDING','RUNNING') OR next_step IS DISTINCT FROM NEW.step
 OR (NEW.step='CLEANUP' AND clock_timestamp()<j.cleanup_after) THEN
  RAISE EXCEPTION 'cleanup scope unavailable' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END; $$;
CREATE TRIGGER concert_event_cleanup_progress_scope BEFORE INSERT OR UPDATE ON public.concert_event_cleanup_progress FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_scope();
CREATE TRIGGER concert_event_cleanup_batches_scope BEFORE INSERT ON public.concert_event_cleanup_batches FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_scope();
CREATE FUNCTION public.concert_event_cleanup_head_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='INSERT' THEN
  IF NEW.ordinal<>0 OR NEW.cursor IS NOT NULL OR NEW.processed<>0 OR NEW.state<>'PENDING' OR NEW.failure IS NOT NULL THEN RAISE EXCEPTION 'cleanup starts at zero' USING ERRCODE='23514'; END IF;
 ELSIF (NEW.job_id,NEW.event_id,NEW.actor_id,NEW.step,NEW.owner,NEW.batch_limit) IS DISTINCT FROM (OLD.job_id,OLD.event_id,OLD.actor_id,OLD.step,OLD.owner,OLD.batch_limit) THEN
  RAISE EXCEPTION 'cleanup immutable scope' USING ERRCODE='23514';
 ELSIF pg_trigger_depth()=1 THEN
  IF OLD.state='COMPLETE' OR NEW.state='COMPLETE'
  OR (NEW.ordinal,NEW.cursor,NEW.processed) IS DISTINCT FROM (OLD.ordinal,OLD.cursor,OLD.processed) THEN
   RAISE EXCEPTION 'cleanup requires batch evidence' USING ERRCODE='23514';
  END IF;
 END IF;
 RETURN NEW;
END; $$;
CREATE TRIGGER concert_event_cleanup_head_guard BEFORE INSERT OR UPDATE ON public.concert_event_cleanup_progress FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_head_guard();
CREATE FUNCTION public.concert_event_cleanup_batch_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE h public.concert_event_cleanup_progress; prior_revision bigint;
BEGIN
 SELECT * INTO STRICT h FROM public.concert_event_cleanup_progress WHERE job_id=NEW.job_id AND step=NEW.step AND owner=NEW.owner FOR UPDATE;
 SELECT owner_revision INTO prior_revision FROM public.concert_event_cleanup_batches WHERE job_id=NEW.job_id AND step=NEW.step AND owner=NEW.owner ORDER BY ordinal DESC LIMIT 1;
 IF h.state='COMPLETE' OR h.ordinal<>NEW.ordinal OR h.cursor IS DISTINCT FROM NEW.cursor_in
 OR NEW.processed>h.batch_limit OR NEW.cumulative<>h.processed+NEW.processed
 OR NEW.owner_revision<coalesce(prior_revision,1) THEN RAISE EXCEPTION 'cleanup batch conflict' USING ERRCODE='23514'; END IF;
 RETURN NEW;
END; $$;
CREATE TRIGGER concert_event_cleanup_batch_guard BEFORE INSERT ON public.concert_event_cleanup_batches FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_batch_guard();
CREATE FUNCTION public.concert_event_cleanup_advance() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 UPDATE public.concert_event_cleanup_progress SET
 ordinal=NEW.ordinal+CASE WHEN NEW.outcome='PAGE' THEN 1 ELSE 0 END,
 cursor=CASE WHEN NEW.outcome='PAGE' THEN NEW.cursor_out ELSE NEW.cursor_in END,
 processed=NEW.cumulative,state=CASE WHEN NEW.outcome='PAGE' THEN 'PENDING' ELSE 'COMPLETE' END,failure=NULL
 WHERE job_id=NEW.job_id AND step=NEW.step AND owner=NEW.owner;
 RETURN NULL;
END; $$;
CREATE TRIGGER concert_event_cleanup_advance AFTER INSERT ON public.concert_event_cleanup_batches FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_advance();
CREATE FUNCTION public.concert_event_cleanup_immutable() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN RAISE EXCEPTION 'cleanup evidence is append only' USING ERRCODE='23514'; END; $$;
CREATE TRIGGER concert_event_cleanup_batches_immutable BEFORE UPDATE OR DELETE ON public.concert_event_cleanup_batches FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_immutable();
CREATE TRIGGER concert_event_cleanup_progress_no_delete BEFORE DELETE ON public.concert_event_cleanup_progress FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_immutable();
CREATE TRIGGER concert_event_cleanup_batches_no_truncate BEFORE TRUNCATE ON public.concert_event_cleanup_batches FOR EACH STATEMENT EXECUTE FUNCTION public.concert_event_cleanup_immutable();
CREATE TRIGGER concert_event_cleanup_progress_no_truncate BEFORE TRUNCATE ON public.concert_event_cleanup_progress FOR EACH STATEMENT EXECUTE FUNCTION public.concert_event_cleanup_immutable();
-- Recheck at COMMIT: a deadline checked before a delayed owner is insufficient.
CREATE CONSTRAINT TRIGGER concert_event_cleanup_progress_deadline AFTER INSERT OR UPDATE ON public.concert_event_cleanup_progress DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_closure_deadline();
CREATE CONSTRAINT TRIGGER concert_event_cleanup_batches_deadline AFTER INSERT ON public.concert_event_cleanup_batches DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_closure_deadline();
