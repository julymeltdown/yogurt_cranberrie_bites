-- Forward-only BE09/FE20 authorized photo support. Published 0008/0012 are unchanged.
ALTER TABLE public.concert_media DROP CONSTRAINT concert_media_purpose_check;
ALTER TABLE public.concert_media ADD CONSTRAINT concert_media_purpose_check
 CHECK(purpose IN ('POST','GIVEAWAY','REPORT','BEACON'));

-- One current attachment per beacon; no bytes/URLs/private guidance duplicated.
CREATE TABLE public.concert_meeting_beacon_photos (
 event_id uuid NOT NULL,
 beacon_id uuid PRIMARY KEY,
 owner_id uuid NOT NULL,
 media_id uuid NOT NULL UNIQUE REFERENCES public.concert_media(media_id),
 beacon_revision bigint NOT NULL CHECK(beacon_revision BETWEEN 1 AND 9007199254740991),
 media_revision bigint NOT NULL CHECK(media_revision BETWEEN 1 AND 9007199254740991),
 permission_acknowledged boolean NOT NULL CHECK(permission_acknowledged),
 permission_confirmed_at timestamptz NOT NULL DEFAULT clock_timestamp(),
 expires_at timestamptz NOT NULL,
 FOREIGN KEY(event_id,beacon_id) REFERENCES public.concert_meeting_beacons(event_id,beacon_id) ON DELETE CASCADE,
 FOREIGN KEY(event_id,owner_id) REFERENCES public.concert_participants(event_id,user_id) ON DELETE CASCADE,
 CHECK(expires_at>permission_confirmed_at)
);
CREATE INDEX concert_meeting_photo_expiry ON public.concert_meeting_beacon_photos(expires_at,beacon_id);

CREATE FUNCTION public.concert_meeting_photo_binding_guard() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE b public.concert_meeting_beacons%ROWTYPE; m public.concert_media%ROWTYPE;
 proof_until timestamptz;
BEGIN
 -- Re-read final binding: repeated changes within one transaction must validate
 -- the final association, not a superseded queued NEW tuple.
 SELECT * INTO NEW FROM public.concert_meeting_beacon_photos WHERE beacon_id=NEW.beacon_id;
 IF NOT FOUND THEN RETURN NULL; END IF;
 SELECT * INTO b FROM public.concert_meeting_beacons WHERE event_id=NEW.event_id AND beacon_id=NEW.beacon_id FOR SHARE;
 IF NOT FOUND OR b.owner_id<>NEW.owner_id OR b.state<>'ACTIVE' OR b.revision<>NEW.beacon_revision
 OR b.expires_at<>NEW.expires_at OR clock_timestamp()>=b.expires_at THEN
  RAISE EXCEPTION 'invalid current beacon photo binding' USING ERRCODE='23514';
 END IF;
 SELECT * INTO m FROM public.concert_media WHERE media_id=NEW.media_id FOR SHARE;
 IF NOT FOUND OR m.event_id<>NEW.event_id OR m.owner_id<>NEW.owner_id OR m.purpose<>'BEACON'
 OR m.state<>'READY' OR m.target_id IS DISTINCT FROM NEW.beacon_id OR m.revision<>NEW.media_revision THEN
  RAISE EXCEPTION 'invalid current beacon media binding' USING ERRCODE='23514';
 END IF;
 proof_until:=nullif(current_setting('mcr.relationship_authority_until',true),'')::timestamptz;
 IF proof_until IS NOT NULL AND clock_timestamp()>=proof_until THEN
  RAISE EXCEPTION 'photo authority expired before commit' USING ERRCODE='23514';
 END IF;
 RETURN NULL;
END;
$$;
CREATE CONSTRAINT TRIGGER concert_meeting_photo_binding_commit AFTER INSERT OR UPDATE
 ON public.concert_meeting_beacon_photos DEFERRABLE INITIALLY DEFERRED
 FOR EACH ROW EXECUTE FUNCTION public.concert_meeting_photo_binding_guard();
