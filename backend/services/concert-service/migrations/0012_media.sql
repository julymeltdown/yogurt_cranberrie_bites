-- BE24 image-only private staging and durable sanitized publication. No public object URLs.
CREATE TABLE public.concert_media (
 media_id uuid PRIMARY KEY,
 event_id uuid NOT NULL,
 owner_id uuid NOT NULL,
 purpose text NOT NULL CHECK(purpose IN ('POST','GIVEAWAY','REPORT')),
 content_type text NOT NULL CHECK(content_type IN ('image/jpeg','image/png','image/webp')),
 declared_bytes integer NOT NULL CHECK(declared_bytes BETWEEN 1 AND 10485760),
 input_sha256 bytea NOT NULL CHECK(octet_length(input_sha256)=32),
 policy_event_revision bigint NOT NULL CHECK(policy_event_revision BETWEEN 1 AND 9007199254740991),
 policy_relationship_revision bigint NOT NULL CHECK(policy_relationship_revision BETWEEN 1 AND 9007199254740991),
 policy_authority_revision bigint NOT NULL CHECK(policy_authority_revision BETWEEN 1 AND 9007199254740991),
 state text NOT NULL CHECK(state IN ('PENDING','UPLOADED','READY','DELETED')),
 revision bigint NOT NULL DEFAULT 1 CHECK(revision BETWEEN 1 AND 9007199254740991),
 staged bytea,
 sanitized bytea,
 width integer,
 height integer,
 target_id uuid,
 expires_at timestamptz NOT NULL DEFAULT (clock_timestamp()+interval '900 seconds'),
 orphan_at timestamptz NOT NULL DEFAULT (clock_timestamp()+interval '86400 seconds'),
 created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 FOREIGN KEY(event_id,owner_id) REFERENCES public.concert_participants(event_id,user_id) ON DELETE CASCADE,
 CHECK(expires_at > created_at AND orphan_at > expires_at),
 CHECK(target_id IS NULL OR state='READY'),
 CHECK((state='PENDING' AND staged IS NULL AND sanitized IS NULL AND width IS NULL AND height IS NULL)
 OR (state='UPLOADED' AND staged IS NOT NULL AND octet_length(staged)=declared_bytes AND sanitized IS NULL AND width IS NULL AND height IS NULL)
 OR (state='READY' AND staged IS NULL AND octet_length(sanitized) BETWEEN 1 AND 10485760 AND sanitized IS NOT NULL
 AND width BETWEEN 1 AND 4096 AND width IS NOT NULL AND height BETWEEN 1 AND 4096 AND height IS NOT NULL AND width::bigint*height<=16000000)
 OR (state='DELETED' AND staged IS NULL AND sanitized IS NULL AND width IS NULL AND height IS NULL AND target_id IS NULL))
);
CREATE INDEX concert_media_owner ON public.concert_media(event_id,owner_id);
CREATE INDEX concert_media_cleanup ON public.concert_media(orphan_at) WHERE target_id IS NULL AND state<>'DELETED';
CREATE TRIGGER concert_media_revision_guard BEFORE INSERT OR UPDATE ON public.concert_media
 FOR EACH ROW EXECUTE FUNCTION public.concert_guard_revision('media_id','event_id','owner_id','purpose','content_type','declared_bytes','input_sha256','policy_event_revision','policy_relationship_revision','policy_authority_revision','expires_at','orphan_at');
-- Published bytes and linkage cannot be rewritten or resurrected through another producer.
CREATE FUNCTION public.concert_media_transition_guard() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
 IF NOT ((OLD.state='PENDING' AND NEW.state IN ('UPLOADED','DELETED'))
 OR (OLD.state='UPLOADED' AND NEW.state IN ('READY','DELETED'))
 OR (OLD.state='READY' AND NEW.state IN ('READY','DELETED'))) THEN
  RAISE EXCEPTION 'invalid media transition' USING ERRCODE='23514';
 END IF;
 IF NEW.state='READY' AND OLD.state='READY' AND
 (NEW.sanitized IS DISTINCT FROM OLD.sanitized OR NEW.width IS DISTINCT FROM OLD.width
 OR NEW.height IS DISTINCT FROM OLD.height OR
 (OLD.target_id IS NOT NULL AND NEW.target_id IS DISTINCT FROM OLD.target_id)) THEN
  RAISE EXCEPTION 'published media is immutable' USING ERRCODE='23514';
 END IF;
 RETURN NEW;
END;
$$;
CREATE TRIGGER concert_media_transition_guard BEFORE UPDATE ON public.concert_media
 FOR EACH ROW EXECUTE FUNCTION public.concert_media_transition_guard();
