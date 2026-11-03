-- New MCR physical meeting persistence. No source venue or identity data imported.
-- Operator composition supplies cutoff; absence deliberately disables meeting writes.
CREATE TABLE public.concert_meeting_settings (
    event_id uuid PRIMARY KEY REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    beacon_ttl_seconds integer NOT NULL DEFAULT 900 CHECK (beacon_ttl_seconds BETWEEN 1 AND 900),
    event_cutoff timestamptz NOT NULL
);
CREATE TABLE public.concert_meeting_beacons (
    beacon_id uuid PRIMARY KEY,
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    owner_id uuid NOT NULL,
    place_id uuid NOT NULL,
    map_revision bigint NOT NULL CHECK(map_revision BETWEEN 1 AND 9007199254740991),
    place_revision bigint NOT NULL CHECK(place_revision BETWEEN 1 AND 9007199254740991),
    selected_catalog_revision bigint NOT NULL CHECK(selected_catalog_revision BETWEEN 1 AND 60000),
    instructions text NOT NULL CHECK(char_length(instructions) BETWEEN 1 AND 1000 AND instructions ~ '[^[:space:]]' AND instructions !~ '[[:cntrl:]]'),
    identification text NOT NULL CHECK(char_length(identification) BETWEEN 1 AND 240 AND identification ~ '[^[:space:]]' AND identification !~ '[[:cntrl:]]'),
    audience jsonb NOT NULL CHECK(jsonb_typeof(audience)='object'),
    expires_at timestamptz NOT NULL,
    state text NOT NULL DEFAULT 'ACTIVE' CHECK(state IN ('ACTIVE','REVOKED')),
    revision bigint NOT NULL DEFAULT 1 CHECK(revision BETWEEN 1 AND 9007199254740991),
    created_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    UNIQUE(event_id,beacon_id),
    CHECK(updated_at >= created_at),
    CHECK(expires_at > updated_at)
);
CREATE INDEX concert_meeting_party_beacons ON public.concert_meeting_beacons(event_id,(audience->>'party_id')) WHERE audience->>'kind'='PARTY_MEMBERS';
CREATE INDEX concert_meeting_expiry ON public.concert_meeting_beacons(expires_at) WHERE state='ACTIVE';
CREATE FUNCTION public.concert_guard_meeting_beacon() RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE db_now timestamptz; settings public.concert_meeting_settings%ROWTYPE;
BEGIN
    db_now := clock_timestamp();
    IF TG_OP='UPDATE' THEN
        IF NEW.beacon_id<>OLD.beacon_id OR NEW.event_id<>OLD.event_id OR NEW.owner_id<>OLD.owner_id
           OR NEW.created_at IS DISTINCT FROM OLD.created_at OR NEW.revision<>OLD.revision+1
           OR OLD.state='REVOKED' THEN
            RAISE EXCEPTION 'invalid beacon scope, terminal state or revision' USING ERRCODE='23514';
        END IF;
        IF NEW.state='REVOKED' THEN
            -- Revocation does not renew the last physical confirmation or TTL.
            NEW.updated_at := OLD.updated_at;
            RETURN NEW;
        END IF;
        IF OLD.expires_at<=db_now THEN
            RAISE EXCEPTION 'expired beacon is terminal' USING ERRCODE='23514';
        END IF;
    ELSE
        IF NEW.revision<>1 OR NEW.state<>'ACTIVE' THEN
            RAISE EXCEPTION 'invalid initial beacon state' USING ERRCODE='23514';
        END IF;
        NEW.created_at := db_now;
    END IF;
    SELECT * INTO settings FROM public.concert_meeting_settings WHERE event_id=NEW.event_id FOR SHARE;
    IF NOT FOUND OR NEW.expires_at<=db_now OR NEW.expires_at>db_now+make_interval(secs=>settings.beacon_ttl_seconds)
       OR NEW.expires_at>settings.event_cutoff THEN
        RAISE EXCEPTION 'invalid beacon expiry' USING ERRCODE='23514';
    END IF;
    NEW.updated_at := db_now;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_meeting_beacon_guard BEFORE INSERT OR UPDATE ON public.concert_meeting_beacons
FOR EACH ROW EXECUTE FUNCTION public.concert_guard_meeting_beacon();
