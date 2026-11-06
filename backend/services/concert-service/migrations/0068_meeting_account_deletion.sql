-- BE42 Party and Meeting owning-store progress only. This is not a whole
-- PrivateData receipt and does not alter public places, events or BE15 receipts.
CREATE FUNCTION public.concert_party_has_private_input(subject uuid)
RETURNS boolean LANGUAGE sql STABLE STRICT AS $$
 SELECT EXISTS(SELECT 1 FROM public.concert_community_parties p WHERE owner_id=subject
  OR aggregate->'applications' @> jsonb_build_array(jsonb_build_object('user',subject))
  OR aggregate->'members' @> jsonb_build_array(jsonb_build_object('user',subject)))
 OR EXISTS(SELECT 1 FROM public.concert_community_applications WHERE user_id=subject)
 OR EXISTS(SELECT 1 FROM public.concert_community_members WHERE user_id=subject)
$$;
CREATE FUNCTION public.concert_meeting_has_private_input(subject uuid)
RETURNS boolean LANGUAGE sql STABLE STRICT AS $$
 SELECT EXISTS(SELECT 1 FROM public.concert_meeting_beacons WHERE owner_id=subject
  OR (audience->>'kind'='EXPLICIT_USERS' AND audience->'user_ids' @> to_jsonb(ARRAY[subject])))
 OR EXISTS(SELECT 1 FROM public.concert_meeting_rally_states WHERE user_id=subject)
 OR EXISTS(SELECT 1 FROM public.concert_meeting_beacon_photos WHERE owner_id=subject)
$$;

CREATE TABLE public.concert_meeting_account_deletion (
 user_id uuid NOT NULL, owner_kind text NOT NULL CHECK(owner_kind IN ('PARTY','MEETING')),
 saga_id uuid NOT NULL, actor_id uuid NOT NULL, retention_until timestamptz NOT NULL,
 command_binding bytea NOT NULL CHECK(octet_length(command_binding)=32),
 identity_binding bytea NOT NULL CHECK(octet_length(identity_binding)=32),
 source_binding bytea NOT NULL CHECK(octet_length(source_binding)=32 AND source_binding<>decode(repeat('00',32),'hex')),
 complete boolean NOT NULL DEFAULT false, owner_revision bigint NOT NULL DEFAULT 0,
 native_xid xid8 NOT NULL DEFAULT pg_current_xact_id(),
 PRIMARY KEY(user_id,owner_kind),
 CHECK((complete AND owner_revision=1) OR (NOT complete AND owner_revision=0))
);
CREATE FUNCTION public.concert_meeting_deletion_journal_guard()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF TG_OP='DELETE' OR TG_OP='TRUNCATE' THEN
  RAISE EXCEPTION 'deletion owner journal is immutable' USING ERRCODE='23514';
 END IF;
 IF TG_OP='UPDATE' AND (OLD.complete OR
  to_jsonb(NEW)-ARRAY['complete','owner_revision','native_xid'] IS DISTINCT FROM
  to_jsonb(OLD)-ARRAY['complete','owner_revision','native_xid']) THEN
  RAISE EXCEPTION 'deletion owner command/source or terminal replay changed' USING ERRCODE='23514';
 END IF;
 IF NEW.native_xid<>pg_current_xact_id() OR NOT EXISTS(
  SELECT 1 FROM public.concert_world_account_deletion_fences f
  WHERE f.user_id=NEW.user_id AND f.saga_id=NEW.saga_id AND f.identity_binding=NEW.identity_binding) THEN
  RAISE EXCEPTION 'deletion owner requires exact common native fence' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END $$;
CREATE TRIGGER concert_meeting_deletion_journal_guard BEFORE INSERT OR UPDATE OR DELETE
 ON public.concert_meeting_account_deletion FOR EACH ROW EXECUTE FUNCTION public.concert_meeting_deletion_journal_guard();
CREATE TRIGGER concert_meeting_deletion_journal_no_truncate BEFORE TRUNCATE
 ON public.concert_meeting_account_deletion FOR EACH STATEMENT EXECUTE FUNCTION public.concert_meeting_deletion_journal_guard();

CREATE FUNCTION public.concert_meeting_deletion_commit_guard()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE deadline timestamptz; final public.concert_meeting_account_deletion;
BEGIN
 deadline:=nullif(current_setting('mcr.meeting_deletion_deadline',true),'')::timestamptz;
 IF deadline IS NULL OR clock_timestamp()>=deadline THEN
  RAISE EXCEPTION 'deletion owner original deadline expired' USING ERRCODE='23514';
 END IF;
 SELECT * INTO STRICT final FROM public.concert_meeting_account_deletion
  WHERE user_id=NEW.user_id AND owner_kind=NEW.owner_kind;
 IF final.complete AND (CASE final.owner_kind
   WHEN 'PARTY' THEN public.concert_party_has_private_input(final.user_id)
   WHEN 'MEETING' THEN public.concert_meeting_has_private_input(final.user_id)
   ELSE true END) THEN
  RAISE EXCEPTION 'deletion owner still has private input' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END $$;
CREATE CONSTRAINT TRIGGER concert_meeting_deletion_commit AFTER INSERT OR UPDATE
 ON public.concert_meeting_account_deletion DEFERRABLE INITIALLY DEFERRED
 FOR EACH ROW EXECUTE FUNCTION public.concert_meeting_deletion_commit_guard();
