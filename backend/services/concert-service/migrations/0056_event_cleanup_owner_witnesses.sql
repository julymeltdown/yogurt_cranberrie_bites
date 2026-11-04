-- BE41 native local-owner evidence. Root transferred ONLY this new migration.
-- A correlation setting is never authority. Real caller fences remain required.
CREATE TABLE public.concert_event_cleanup_owner_pages (
 operation_id uuid PRIMARY KEY,
 job_id uuid NOT NULL,event_id uuid NOT NULL,actor_id uuid NOT NULL,
 step text NOT NULL,owner text NOT NULL,ordinal bigint NOT NULL CHECK(ordinal>=0),
 batch_limit smallint NOT NULL CHECK(batch_limit BETWEEN 1 AND 100),
 cursor_in bytea,cursor_out bytea,binding bytea NOT NULL CHECK(octet_length(binding)=32),
 native_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
 native_backend integer NOT NULL DEFAULT pg_backend_pid(),
 state text NOT NULL DEFAULT 'OPEN' CHECK(state IN ('OPEN','SEALED')),
 processed smallint NOT NULL DEFAULT 0 CHECK(processed BETWEEN 0 AND 100),
 receipt_id uuid,
 UNIQUE(job_id,step,owner,ordinal),
 FOREIGN KEY(job_id,event_id,actor_id,step,owner)
  REFERENCES public.concert_event_cleanup_progress(job_id,event_id,actor_id,step,owner),
 CHECK((owner='BEACONS' AND step='EXPIRE_BEACONS') OR (owner='PRIVATE_RETENTION' AND step='CLEANUP')),
 CHECK((ordinal=0 AND cursor_in IS NULL) OR (ordinal>0 AND cursor_in=int8send(ordinal))),
 CHECK(cursor_out IS NULL OR cursor_out=int8send(ordinal+1)),
 CHECK((state='OPEN' AND processed=0 AND receipt_id IS NULL AND cursor_out IS NULL)
  OR (state='SEALED' AND receipt_id IS NOT NULL))
);
CREATE TABLE public.concert_event_cleanup_owner_effects (
 operation_id uuid NOT NULL REFERENCES public.concert_event_cleanup_owner_pages,
 resource_key bytea NOT NULL,
 previous_revision bigint NOT NULL CHECK(previous_revision>=0),
 next_revision bigint CHECK(next_revision BETWEEN 1 AND 9007199254740991),
 native_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
 PRIMARY KEY(operation_id,resource_key),
 CHECK((get_byte(resource_key,0)=1 AND octet_length(resource_key)=18)
  OR (get_byte(resource_key,0)=2 AND octet_length(resource_key)=50)
  OR (get_byte(resource_key,0)=3 AND octet_length(resource_key)=18)
  OR (get_byte(resource_key,0) IN (4,5,6,10) AND octet_length(resource_key)=17))
);

CREATE FUNCTION public.concert_event_cleanup_owner_deadline() RETURNS void LANGUAGE plpgsql AS $$
BEGIN
 IF NOT coalesce(clock_timestamp()<nullif(current_setting('mcr.lifecycle_authority_until',true),'')::timestamptz,false)
 OR NOT coalesce(clock_timestamp()<nullif(current_setting('mcr.relationship_authority_until',true),'')::timestamptz,false) THEN
  RAISE EXCEPTION 'local cleanup authority fence unavailable' USING ERRCODE='23514';
 END IF;
END $$;
CREATE FUNCTION public.concert_event_cleanup_uuid(d bytea) RETURNS uuid LANGUAGE sql IMMUTABLE STRICT AS $$
 SELECT encode(set_byte(set_byte(substring(d from 1 for 16),6,(get_byte(d,6)&15)|80),8,(get_byte(d,8)&63)|128),'hex')::uuid
$$;

-- Independently derive the exact Rust BatchRequest hash from durable job/head.
CREATE FUNCTION public.concert_event_cleanup_owner_binding(
 j public.concert_closure_jobs,h public.concert_event_cleanup_progress
) RETURNS bytea LANGUAGE sql STABLE STRICT AS $$
 SELECT sha256(convert_to('mcr-event-cleanup-batch-v1','UTF8')||decode('00','hex')
 ||uuid_send(h.actor_id)||uuid_send(h.event_id)||uuid_send(h.job_id)
 ||convert_to(h.step,'UTF8')||decode('00','hex')||convert_to(h.owner,'UTF8')||decode('00','hex')
 ||int8send(j.retention_revision)
 ||int8send(floor(extract(epoch from j.closed_at))::bigint)
 ||int4send(((extract(epoch from j.closed_at)-floor(extract(epoch from j.closed_at)))*1000000000)::integer)
 ||int8send(floor(extract(epoch from j.cleanup_after))::bigint)
 ||int4send(((extract(epoch from j.cleanup_after)-floor(extract(epoch from j.cleanup_after)))*1000000000)::integer)
 ||int8send(h.ordinal)||int2send(h.batch_limit)||sha256(coalesce(h.cursor,''::bytea)))
$$;
CREATE FUNCTION public.concert_event_cleanup_private_active(ev uuid) RETURNS boolean LANGUAGE sql STABLE AS $$
 SELECT EXISTS(SELECT 1 FROM public.concert_social_grants WHERE event_id=ev AND state='ACTIVE')
 OR EXISTS(SELECT 1 FROM public.concert_social_requests WHERE event_id=ev AND state IN ('PENDING','APPROVED'))
 OR EXISTS(SELECT 1 FROM public.concert_meeting_beacons WHERE event_id=ev AND state='ACTIVE')
$$;
CREATE FUNCTION public.concert_event_cleanup_owner_remaining(ev uuid,own text) RETURNS boolean LANGUAGE sql STABLE AS $$
 SELECT CASE own WHEN 'BEACONS' THEN
  EXISTS(SELECT 1 FROM public.concert_meeting_beacons WHERE event_id=ev AND state='ACTIVE')
 WHEN 'PRIVATE_RETENTION' THEN
  EXISTS(SELECT 1 FROM public.concert_social_handles WHERE event_id=ev AND ciphertext IS NOT NULL)
  OR EXISTS(SELECT 1 FROM public.concert_social_grants WHERE event_id=ev)
  OR EXISTS(SELECT 1 FROM public.concert_social_bindings WHERE event_id=ev)
  OR EXISTS(SELECT 1 FROM public.concert_social_requests WHERE event_id=ev)
  OR EXISTS(SELECT 1 FROM public.concert_meeting_beacon_photos WHERE event_id=ev)
  OR EXISTS(SELECT 1 FROM public.concert_meeting_beacons WHERE event_id=ev)
 ELSE true END
$$;

CREATE FUNCTION public.concert_event_cleanup_private_candidates(ev uuid)
RETURNS TABLE(resource_key bytea) LANGUAGE sql STABLE AS $$
 SELECT decode('01','hex')||uuid_send(user_id)||decode(CASE platform WHEN 'INSTAGRAM' THEN '01' ELSE '02' END,'hex')
 FROM public.concert_social_handles WHERE event_id=ev AND ciphertext IS NOT NULL
 UNION ALL SELECT decode('02','hex')||uuid_send(request_id)||uuid_send(owner_id)||uuid_send(viewer_id)||decode(CASE platform WHEN 'INSTAGRAM' THEN '01' ELSE '02' END,'hex')
 FROM public.concert_social_grants WHERE event_id=ev AND state<>'ACTIVE'
 UNION ALL SELECT decode('03','hex')||uuid_send(b.request_id)||decode(CASE b.platform WHEN 'INSTAGRAM' THEN '01' ELSE '02' END,'hex')
 FROM public.concert_social_bindings b WHERE b.event_id=ev AND NOT EXISTS(
  SELECT 1 FROM public.concert_social_grants g WHERE (g.event_id,g.request_id,g.platform)=(b.event_id,b.request_id,b.platform))
 UNION ALL SELECT decode('04','hex')||uuid_send(r.request_id)
 FROM public.concert_social_requests r WHERE r.event_id=ev AND r.state NOT IN ('PENDING','APPROVED') AND NOT EXISTS(
  SELECT 1 FROM public.concert_social_bindings b WHERE (b.event_id,b.request_id)=(r.event_id,r.request_id))
 UNION ALL SELECT decode('05','hex')||uuid_send(p.beacon_id)
 FROM public.concert_meeting_beacon_photos p JOIN public.concert_meeting_beacons b USING(event_id,beacon_id)
 WHERE p.event_id=ev AND b.state='REVOKED'
 UNION ALL SELECT decode('06','hex')||uuid_send(b.beacon_id)
 FROM public.concert_meeting_beacons b WHERE b.event_id=ev AND b.state='REVOKED' AND NOT EXISTS(
  SELECT 1 FROM public.concert_meeting_beacon_photos p WHERE p.beacon_id=b.beacon_id)
$$;

CREATE FUNCTION public.concert_event_cleanup_owner_digest(op uuid) RETURNS uuid LANGUAGE sql STABLE AS $$
 SELECT public.concert_event_cleanup_uuid(sha256(
  convert_to('mcr-local-cleanup-postimage-v1','UTF8')||decode('00','hex')||p.binding
  ||int2send(p.processed)||coalesce(p.cursor_out,''::bytea)
  ||convert_to(coalesce((SELECT string_agg(encode(e.resource_key,'hex')||':'||e.previous_revision::text||':'||coalesce(e.next_revision::text,'DELETED'),';' ORDER BY e.resource_key)
    FROM public.concert_event_cleanup_owner_effects e WHERE e.operation_id=op),''),'UTF8')))
 FROM public.concert_event_cleanup_owner_pages p WHERE p.operation_id=op
$$;

CREATE FUNCTION public.concert_event_cleanup_owner_assert_effects(op uuid) RETURNS void LANGUAGE plpgsql AS $$
DECLARE p public.concert_event_cleanup_owner_pages; e public.concert_event_cleanup_owner_effects;
 k integer; a uuid; b uuid; d uuid; platform_value text; valid boolean;
BEGIN
 SELECT * INTO STRICT p FROM public.concert_event_cleanup_owner_pages WHERE operation_id=op;
 FOR e IN SELECT * FROM public.concert_event_cleanup_owner_effects WHERE operation_id=op ORDER BY resource_key LOOP
  IF e.native_xid<>p.native_xid THEN RAISE EXCEPTION 'foreign native cleanup effect'; END IF;
  k:=get_byte(e.resource_key,0);a:=encode(substring(e.resource_key from 2 for 16),'hex')::uuid;
  IF k IN (1,3) THEN platform_value:=CASE get_byte(e.resource_key,17) WHEN 1 THEN 'INSTAGRAM' WHEN 2 THEN 'X' END; END IF;
  CASE k
   WHEN 1 THEN
    SELECT EXISTS(SELECT 1 FROM public.concert_social_handles WHERE event_id=p.event_id AND user_id=a AND platform=platform_value
     AND ciphertext IS NULL AND nonce IS NULL AND key_id IS NULL AND revision=e.next_revision) INTO valid;
   WHEN 2 THEN
    b:=encode(substring(e.resource_key from 18 for 16),'hex')::uuid;d:=encode(substring(e.resource_key from 34 for 16),'hex')::uuid;
    platform_value:=CASE get_byte(e.resource_key,49) WHEN 1 THEN 'INSTAGRAM' WHEN 2 THEN 'X' END;
    SELECT NOT EXISTS(SELECT 1 FROM public.concert_social_grants WHERE event_id=p.event_id AND request_id=a AND owner_id=b AND viewer_id=d AND platform=platform_value) INTO valid;
   WHEN 3 THEN SELECT NOT EXISTS(SELECT 1 FROM public.concert_social_bindings WHERE event_id=p.event_id AND request_id=a AND platform=platform_value) INTO valid;
   WHEN 4 THEN SELECT NOT EXISTS(SELECT 1 FROM public.concert_social_requests WHERE event_id=p.event_id AND request_id=a) INTO valid;
   WHEN 5 THEN SELECT NOT EXISTS(SELECT 1 FROM public.concert_meeting_beacon_photos WHERE event_id=p.event_id AND beacon_id=a) INTO valid;
   WHEN 6 THEN SELECT NOT EXISTS(SELECT 1 FROM public.concert_meeting_beacons WHERE event_id=p.event_id AND beacon_id=a) INTO valid;
   WHEN 10 THEN SELECT EXISTS(SELECT 1 FROM public.concert_meeting_beacons WHERE event_id=p.event_id AND beacon_id=a AND state='REVOKED' AND revision=e.next_revision) INTO valid;
   ELSE valid:=false;
  END CASE;
  IF NOT coalesce(valid,false) THEN RAISE EXCEPTION 'cleanup postimage changed' USING ERRCODE='23514'; END IF;
 END LOOP;
END $$;

CREATE FUNCTION public.concert_event_cleanup_owner_page_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE j public.concert_closure_jobs; h public.concert_event_cleanup_progress; phase_value text; expected bytea; count_value bigint;
BEGIN
 PERFORM public.concert_event_cleanup_owner_deadline();
 IF TG_OP='INSERT' THEN
  SELECT phase INTO phase_value FROM public.concert_events WHERE event_id=NEW.event_id FOR UPDATE;
  SELECT * INTO STRICT j FROM public.concert_closure_jobs WHERE job_id=NEW.job_id AND event_id=NEW.event_id FOR UPDATE;
  SELECT * INTO STRICT h FROM public.concert_event_cleanup_progress WHERE job_id=NEW.job_id AND step=NEW.step AND owner=NEW.owner FOR UPDATE;
  expected:=public.concert_event_cleanup_owner_binding(j,h);
  IF phase_value IS DISTINCT FROM 'READ_ONLY' OR j.actor_id<>NEW.actor_id
   OR j.progress->>'state' NOT IN ('PENDING','RUNNING')
   OR NOT EXISTS(SELECT 1 FROM jsonb_array_elements(j.progress->'steps') WITH ORDINALITY x(s,n)
    WHERE s->>'name'=NEW.step AND s->>'state' IN ('PENDING','FAILED') AND n=(SELECT min(n2) FROM jsonb_array_elements(j.progress->'steps') WITH ORDINALITY y(s2,n2) WHERE s2->>'state'<>'SUCCEEDED'))
   OR (h.event_id,h.actor_id,h.ordinal,h.batch_limit) IS DISTINCT FROM (NEW.event_id,NEW.actor_id,NEW.ordinal,NEW.batch_limit)
   OR h.cursor IS DISTINCT FROM NEW.cursor_in OR h.state='COMPLETE'
   OR NEW.binding<>expected OR NEW.operation_id<>public.concert_event_cleanup_uuid(expected)
   OR NEW.native_xid<>pg_current_xact_id() OR NEW.native_backend<>pg_backend_pid()
   OR NEW.state<>'OPEN' OR NEW.processed<>0 OR NEW.receipt_id IS NOT NULL OR NEW.cursor_out IS NOT NULL
   OR (NEW.owner='PRIVATE_RETENTION' AND (clock_timestamp()<j.cleanup_after OR public.concert_event_cleanup_private_active(NEW.event_id))) THEN
   RAISE EXCEPTION 'invalid native cleanup page binding' USING ERRCODE='23514';
  END IF;
 ELSE
  IF OLD.state<>'OPEN' OR NEW.state<>'SEALED'
   OR OLD.native_xid<>pg_current_xact_id() OR OLD.native_backend<>pg_backend_pid()
   OR to_jsonb(NEW)-ARRAY['state','processed','receipt_id','cursor_out'] IS DISTINCT FROM to_jsonb(OLD)-ARRAY['state','processed','receipt_id','cursor_out'] THEN
   RAISE EXCEPTION 'immutable native cleanup page' USING ERRCODE='23514';
  END IF;
  SELECT count(*) INTO count_value FROM public.concert_event_cleanup_owner_effects WHERE operation_id=NEW.operation_id;
  IF NEW.processed<>count_value OR count_value>NEW.batch_limit
   OR NEW.cursor_out IS DISTINCT FROM (CASE WHEN public.concert_event_cleanup_owner_remaining(NEW.event_id,NEW.owner) THEN int8send(NEW.ordinal+1) ELSE NULL END)
   OR (NEW.cursor_out IS NOT NULL AND NEW.processed=0) THEN
   RAISE EXCEPTION 'unproved cleanup page count or convergence' USING ERRCODE='23514';
  END IF;
  PERFORM public.concert_event_cleanup_owner_assert_effects(NEW.operation_id);
  -- The seal helper calculates the digest after updating these fields. A
  -- BEFORE trigger can instead derive the same value from NEW and immutable effects.
  IF NEW.receipt_id<>public.concert_event_cleanup_uuid(sha256(convert_to('mcr-local-cleanup-postimage-v1','UTF8')||decode('00','hex')||NEW.binding||int2send(NEW.processed)||coalesce(NEW.cursor_out,''::bytea)
   ||convert_to(coalesce((SELECT string_agg(encode(e.resource_key,'hex')||':'||e.previous_revision::text||':'||coalesce(e.next_revision::text,'DELETED'),';' ORDER BY e.resource_key) FROM public.concert_event_cleanup_owner_effects e WHERE e.operation_id=NEW.operation_id),''),'UTF8'))) THEN
   RAISE EXCEPTION 'invalid cleanup postimage digest' USING ERRCODE='23514';
  END IF;
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_event_cleanup_owner_page_guard BEFORE INSERT OR UPDATE ON public.concert_event_cleanup_owner_pages
 FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_owner_page_guard();

CREATE FUNCTION public.concert_event_cleanup_owner_effect_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE p public.concert_event_cleanup_owner_pages;
BEGIN
 SELECT * INTO STRICT p FROM public.concert_event_cleanup_owner_pages WHERE operation_id=NEW.operation_id FOR UPDATE;
 IF pg_trigger_depth()<2 OR p.state<>'OPEN' OR p.native_xid<>pg_current_xact_id()
  OR p.native_backend<>pg_backend_pid() OR NEW.native_xid<>p.native_xid
  OR nullif(current_setting('mcr.cleanup_operation_id',true),'')::uuid IS DISTINCT FROM p.operation_id
  OR (SELECT count(*) FROM public.concert_event_cleanup_owner_effects WHERE operation_id=p.operation_id)>=p.batch_limit THEN
  RAISE EXCEPTION 'cleanup requires bounded actual native row effect' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_event_cleanup_owner_effect_guard BEFORE INSERT ON public.concert_event_cleanup_owner_effects
 FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_owner_effect_guard();

CREATE FUNCTION public.concert_event_cleanup_owner_capture() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE op uuid; p public.concert_event_cleanup_owner_pages; key_value bytea; old_revision bigint; new_revision bigint;
BEGIN
 op:=nullif(current_setting('mcr.cleanup_operation_id',true),'')::uuid;
 IF op IS NULL THEN RETURN NULL; END IF;
 SELECT * INTO STRICT p FROM public.concert_event_cleanup_owner_pages WHERE operation_id=op FOR UPDATE;
 PERFORM public.concert_event_cleanup_owner_deadline();
 IF OLD.event_id<>p.event_id THEN RAISE EXCEPTION 'cross-event cleanup effect'; END IF;
 IF p.owner='BEACONS' THEN
  IF TG_TABLE_NAME='concert_meeting_beacons' AND TG_OP='UPDATE' THEN
   IF OLD.state='ACTIVE' AND NEW.state='REVOKED' THEN
    key_value:=decode('0a','hex')||uuid_send(OLD.beacon_id);old_revision:=OLD.revision;new_revision:=NEW.revision;
   END IF;
  END IF;
 ELSIF p.owner='PRIVATE_RETENTION' THEN
  IF TG_TABLE_NAME='concert_social_handles' THEN
   IF TG_OP='UPDATE' AND OLD.ciphertext IS NOT NULL THEN
    IF NEW.ciphertext IS NULL AND NEW.nonce IS NULL AND NEW.key_id IS NULL
     AND to_jsonb(NEW)-ARRAY['ciphertext','nonce','key_id','revision','updated_at'] IS NOT DISTINCT FROM to_jsonb(OLD)-ARRAY['ciphertext','nonce','key_id','revision','updated_at'] THEN
     key_value:=decode('01','hex')||uuid_send(OLD.user_id)||decode(CASE OLD.platform WHEN 'INSTAGRAM' THEN '01' ELSE '02' END,'hex');old_revision:=OLD.revision;new_revision:=NEW.revision;
    END IF;
   END IF;
  ELSIF TG_OP='DELETE' THEN
   CASE TG_TABLE_NAME
    WHEN 'concert_social_grants' THEN
     IF OLD.state='ACTIVE' THEN RAISE EXCEPTION 'active grant retention refused'; END IF;
     key_value:=decode('02','hex')||uuid_send(OLD.request_id)||uuid_send(OLD.owner_id)||uuid_send(OLD.viewer_id)||decode(CASE OLD.platform WHEN 'INSTAGRAM' THEN '01' ELSE '02' END,'hex');old_revision:=OLD.revision;
    WHEN 'concert_social_bindings' THEN key_value:=decode('03','hex')||uuid_send(OLD.request_id)||decode(CASE OLD.platform WHEN 'INSTAGRAM' THEN '01' ELSE '02' END,'hex');old_revision:=0;
    WHEN 'concert_social_requests' THEN
     IF OLD.state IN ('PENDING','APPROVED') THEN RAISE EXCEPTION 'active request retention refused'; END IF;
     key_value:=decode('04','hex')||uuid_send(OLD.request_id);old_revision:=OLD.revision;
    WHEN 'concert_meeting_beacon_photos' THEN
     IF NOT EXISTS(SELECT 1 FROM public.concert_meeting_beacons WHERE event_id=p.event_id AND beacon_id=OLD.beacon_id AND state='REVOKED') THEN RAISE EXCEPTION 'active beacon photo retention refused'; END IF;
     key_value:=decode('05','hex')||uuid_send(OLD.beacon_id);old_revision:=OLD.beacon_revision;
    WHEN 'concert_meeting_beacons' THEN
     IF OLD.state<>'REVOKED' THEN RAISE EXCEPTION 'active beacon retention refused'; END IF;
     key_value:=decode('06','hex')||uuid_send(OLD.beacon_id);old_revision:=OLD.revision;
    ELSE RAISE EXCEPTION 'unsupported private cleanup effect';
   END CASE;
  END IF;
 END IF;
 IF key_value IS NULL THEN RAISE EXCEPTION 'unsupported local cleanup effect'; END IF;
 INSERT INTO public.concert_event_cleanup_owner_effects(operation_id,resource_key,previous_revision,next_revision)
 VALUES(op,key_value,old_revision,new_revision);
 RETURN NULL;
END $$;
CREATE TRIGGER concert_cleanup_capture_handles AFTER UPDATE OR DELETE ON public.concert_social_handles FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_owner_capture();
CREATE TRIGGER concert_cleanup_capture_grants AFTER UPDATE OR DELETE ON public.concert_social_grants FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_owner_capture();
CREATE TRIGGER concert_cleanup_capture_bindings AFTER UPDATE OR DELETE ON public.concert_social_bindings FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_owner_capture();
CREATE TRIGGER concert_cleanup_capture_requests AFTER UPDATE OR DELETE ON public.concert_social_requests FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_owner_capture();
CREATE TRIGGER concert_cleanup_capture_photos AFTER UPDATE OR DELETE ON public.concert_meeting_beacon_photos FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_owner_capture();
CREATE TRIGGER concert_cleanup_capture_beacons AFTER UPDATE OR DELETE ON public.concert_meeting_beacons FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_owner_capture();

CREATE FUNCTION public.concert_event_cleanup_private_apply(ev uuid,k bytea) RETURNS void LANGUAGE plpgsql AS $$
DECLARE p public.concert_event_cleanup_owner_pages; a uuid; b uuid; d uuid; platform_value text; changed bigint;
BEGIN
 SELECT * INTO STRICT p FROM public.concert_event_cleanup_owner_pages WHERE operation_id=nullif(current_setting('mcr.cleanup_operation_id',true),'')::uuid FOR UPDATE;
 IF p.event_id<>ev OR p.owner<>'PRIVATE_RETENTION' OR p.state<>'OPEN' OR p.native_xid<>pg_current_xact_id() OR p.native_backend<>pg_backend_pid() THEN RAISE EXCEPTION 'missing native retention page'; END IF;
 a:=encode(substring(k from 2 for 16),'hex')::uuid;
 IF get_byte(k,0) IN (1,3) THEN platform_value:=CASE get_byte(k,17) WHEN 1 THEN 'INSTAGRAM' WHEN 2 THEN 'X' END; END IF;
 CASE get_byte(k,0)
  WHEN 1 THEN UPDATE public.concert_social_handles SET key_id=NULL,nonce=NULL,ciphertext=NULL,revision=revision+1 WHERE event_id=ev AND user_id=a AND platform=platform_value AND ciphertext IS NOT NULL;
  WHEN 2 THEN
   b:=encode(substring(k from 18 for 16),'hex')::uuid;d:=encode(substring(k from 34 for 16),'hex')::uuid;
   platform_value:=CASE get_byte(k,49) WHEN 1 THEN 'INSTAGRAM' WHEN 2 THEN 'X' END;
   DELETE FROM public.concert_social_grants WHERE event_id=ev AND request_id=a AND owner_id=b AND viewer_id=d AND platform=platform_value AND state<>'ACTIVE';
  WHEN 3 THEN DELETE FROM public.concert_social_bindings b WHERE b.event_id=ev AND b.request_id=a AND b.platform=platform_value AND NOT EXISTS(SELECT 1 FROM public.concert_social_grants g WHERE (g.event_id,g.request_id,g.platform)=(b.event_id,b.request_id,b.platform));
  WHEN 4 THEN DELETE FROM public.concert_social_requests r WHERE r.event_id=ev AND r.request_id=a AND r.state NOT IN ('PENDING','APPROVED') AND NOT EXISTS(SELECT 1 FROM public.concert_social_bindings b WHERE (b.event_id,b.request_id)=(r.event_id,r.request_id));
  WHEN 5 THEN DELETE FROM public.concert_meeting_beacon_photos ph WHERE ph.event_id=ev AND ph.beacon_id=a AND EXISTS(SELECT 1 FROM public.concert_meeting_beacons b WHERE b.event_id=ev AND b.beacon_id=a AND b.state='REVOKED');
  WHEN 6 THEN DELETE FROM public.concert_meeting_beacons b WHERE b.event_id=ev AND b.beacon_id=a AND b.state='REVOKED' AND NOT EXISTS(SELECT 1 FROM public.concert_meeting_beacon_photos ph WHERE ph.beacon_id=b.beacon_id);
  ELSE RAISE EXCEPTION 'unsupported retention resource key';
 END CASE;
 GET DIAGNOSTICS changed=ROW_COUNT;
 IF changed<>1 THEN RAISE EXCEPTION 'retention source changed'; END IF;
END $$;

CREATE FUNCTION public.concert_event_cleanup_owner_seal(op uuid) RETURNS void LANGUAGE plpgsql AS $$
DECLARE p public.concert_event_cleanup_owner_pages; n smallint; next_cursor bytea; digest uuid;
BEGIN
 SELECT * INTO STRICT p FROM public.concert_event_cleanup_owner_pages WHERE operation_id=op FOR UPDATE;
 SELECT count(*)::smallint INTO n FROM public.concert_event_cleanup_owner_effects WHERE operation_id=op;
 next_cursor:=CASE WHEN public.concert_event_cleanup_owner_remaining(p.event_id,p.owner) THEN int8send(p.ordinal+1) ELSE NULL END;
 digest:=public.concert_event_cleanup_uuid(sha256(convert_to('mcr-local-cleanup-postimage-v1','UTF8')||decode('00','hex')||p.binding||int2send(n)||coalesce(next_cursor,''::bytea)
  ||convert_to(coalesce((SELECT string_agg(encode(e.resource_key,'hex')||':'||e.previous_revision::text||':'||coalesce(e.next_revision::text,'DELETED'),';' ORDER BY e.resource_key) FROM public.concert_event_cleanup_owner_effects e WHERE e.operation_id=op),''),'UTF8')));
 UPDATE public.concert_event_cleanup_owner_pages SET state='SEALED',processed=n,cursor_out=next_cursor,receipt_id=digest WHERE operation_id=op;
END $$;
CREATE FUNCTION public.concert_event_cleanup_owner_verify(op uuid) RETURNS void LANGUAGE plpgsql AS $$
DECLARE p public.concert_event_cleanup_owner_pages;
BEGIN
 SELECT * INTO STRICT p FROM public.concert_event_cleanup_owner_pages WHERE operation_id=op FOR SHARE;
 PERFORM public.concert_event_cleanup_owner_deadline();
 PERFORM public.concert_event_cleanup_owner_assert_effects(op);
 IF p.state<>'SEALED' OR p.processed<>(SELECT count(*) FROM public.concert_event_cleanup_owner_effects WHERE operation_id=op)
  OR p.receipt_id IS DISTINCT FROM public.concert_event_cleanup_owner_digest(op)
  OR (p.cursor_out IS NULL AND public.concert_event_cleanup_owner_remaining(p.event_id,p.owner)) THEN
  RAISE EXCEPTION 'unverified native cleanup receipt' USING ERRCODE='23514';
 END IF;
END $$;

CREATE FUNCTION public.concert_event_cleanup_owner_append_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE p public.concert_event_cleanup_owner_pages;
BEGIN
 IF NEW.owner NOT IN ('BEACONS','PRIVATE_RETENTION') THEN RETURN NEW; END IF;
 SELECT * INTO STRICT p FROM public.concert_event_cleanup_owner_pages WHERE operation_id=NEW.operation_id FOR SHARE;
 PERFORM public.concert_event_cleanup_owner_verify(NEW.operation_id);
 IF p.native_xid<>pg_current_xact_id() OR p.native_backend<>pg_backend_pid()
  OR (p.job_id,p.event_id,p.actor_id,p.step,p.owner,p.ordinal,p.binding,p.receipt_id,p.processed)
   IS DISTINCT FROM (NEW.job_id,NEW.event_id,NEW.actor_id,NEW.step,NEW.owner,NEW.ordinal,NEW.binding,NEW.receipt_id,NEW.processed)
  OR p.cursor_in IS DISTINCT FROM NEW.cursor_in OR p.cursor_out IS DISTINCT FROM NEW.cursor_out
  OR NEW.owner_revision<>1 OR NEW.outcome<>(CASE WHEN p.cursor_out IS NULL THEN 'CONVERGED' ELSE 'PAGE' END) THEN
  RAISE EXCEPTION 'cleanup append lacks exact native witness' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_event_cleanup_owner_append_guard BEFORE INSERT ON public.concert_event_cleanup_batches
 FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_owner_append_guard();
CREATE FUNCTION public.concert_event_cleanup_owner_commit() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE p public.concert_event_cleanup_owner_pages;
BEGIN
 SELECT * INTO STRICT p FROM public.concert_event_cleanup_owner_pages WHERE operation_id=NEW.operation_id;
 PERFORM public.concert_event_cleanup_owner_verify(p.operation_id);
 IF NOT EXISTS(SELECT 1 FROM public.concert_event_cleanup_batches b WHERE b.operation_id=p.operation_id AND b.receipt_id=p.receipt_id AND b.binding=p.binding AND b.processed=p.processed
  AND b.job_id=p.job_id AND b.event_id=p.event_id AND b.actor_id=p.actor_id AND b.step=p.step AND b.owner=p.owner AND b.ordinal=p.ordinal
  AND b.cursor_in IS NOT DISTINCT FROM p.cursor_in AND b.cursor_out IS NOT DISTINCT FROM p.cursor_out) THEN
  RAISE EXCEPTION 'native cleanup effect requires atomic engine progress' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER concert_event_cleanup_owner_commit AFTER INSERT OR UPDATE ON public.concert_event_cleanup_owner_pages
 DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_owner_commit();
CREATE TRIGGER concert_cleanup_pages_no_delete BEFORE DELETE ON public.concert_event_cleanup_owner_pages FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_immutable();
CREATE TRIGGER concert_cleanup_pages_no_truncate BEFORE TRUNCATE ON public.concert_event_cleanup_owner_pages FOR EACH STATEMENT EXECUTE FUNCTION public.concert_event_cleanup_immutable();
CREATE TRIGGER concert_cleanup_effects_immutable BEFORE UPDATE OR DELETE ON public.concert_event_cleanup_owner_effects FOR EACH ROW EXECUTE FUNCTION public.concert_event_cleanup_immutable();
CREATE TRIGGER concert_cleanup_effects_no_truncate BEFORE TRUNCATE ON public.concert_event_cleanup_owner_effects FOR EACH STATEMENT EXECUTE FUNCTION public.concert_event_cleanup_immutable();
