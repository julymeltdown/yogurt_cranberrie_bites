-- New MCR physical meeting persistence. No source venue or identity data imported.
-- Operator composition supplies cutoff; absence deliberately disables meeting writes.
CREATE TABLE public.concert_meeting_settings (
    event_id uuid PRIMARY KEY REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    beacon_ttl_seconds integer NOT NULL DEFAULT 900 CHECK (beacon_ttl_seconds BETWEEN 1 AND 900),
    event_cutoff timestamptz NOT NULL
);
CREATE FUNCTION public.concert_meeting_audience_valid(value jsonb) RETURNS boolean
LANGUAGE plpgsql IMMUTABLE STRICT AS $$
DECLARE item jsonb; seen text[] := '{}';
BEGIN
    IF jsonb_typeof(value)<>'object' THEN RETURN false; END IF;
    IF value->>'kind'='PARTY_MEMBERS' THEN
        RETURN value ?& ARRAY['kind','party_id'] AND value-ARRAY['kind','party_id']='{}'::jsonb
            AND jsonb_typeof(value->'party_id')='string'
            AND (value->>'party_id') ~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$';
    ELSIF value->>'kind'='EXPLICIT_USERS' THEN
        IF NOT value ?& ARRAY['kind','user_ids'] OR value-ARRAY['kind','user_ids']<>'{}'::jsonb
           OR jsonb_typeof(value->'user_ids')<>'array' THEN RETURN false; END IF;
        IF jsonb_array_length(value->'user_ids') NOT BETWEEN 1 AND 50 THEN RETURN false; END IF;
        FOR item IN SELECT * FROM jsonb_array_elements(value->'user_ids') LOOP
            IF jsonb_typeof(item)<>'string' OR (item#>>'{}') !~ '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
               OR (item#>>'{}')=ANY(seen) THEN RETURN false; END IF;
            seen := array_append(seen,item#>>'{}');
        END LOOP;
        RETURN true;
    END IF;
    RETURN false;
END;
$$;
CREATE TABLE public.concert_meeting_beacons (
    beacon_id uuid PRIMARY KEY,
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    owner_id uuid NOT NULL,
    place_id uuid NOT NULL,
    map_revision bigint NOT NULL CHECK(map_revision BETWEEN 1 AND 9007199254740991),
    place_revision bigint NOT NULL CHECK(place_revision BETWEEN 1 AND 9007199254740991),
    selected_catalog_revision bigint NOT NULL CHECK(selected_catalog_revision BETWEEN 1 AND 60000),
    selected_party_meeting_revision bigint CHECK(selected_party_meeting_revision BETWEEN 1 AND 9007199254740991),
    selected_party_audience_revision bigint CHECK(selected_party_audience_revision BETWEEN 1 AND 9007199254740991),
    CHECK ((audience->>'kind'='PARTY_MEMBERS' AND selected_party_meeting_revision IS NOT NULL AND selected_party_audience_revision IS NOT NULL)
        OR (audience->>'kind'='EXPLICIT_USERS' AND selected_party_meeting_revision IS NULL AND selected_party_audience_revision IS NULL)),
    instructions text NOT NULL CHECK(char_length(instructions) BETWEEN 1 AND 1000 AND instructions ~ '[^[:space:]]' AND instructions !~ '[[:cntrl:]]'),
    identification text NOT NULL CHECK(char_length(identification) BETWEEN 1 AND 240 AND identification ~ '[^[:space:]]' AND identification !~ '[[:cntrl:]]'),
    audience jsonb NOT NULL CHECK(public.concert_meeting_audience_valid(audience)),
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
            IF to_jsonb(NEW)-ARRAY['state','revision','updated_at'] IS DISTINCT FROM to_jsonb(OLD)-ARRAY['state','revision','updated_at'] THEN
                RAISE EXCEPTION 'revocation cannot alter physical guidance' USING ERRCODE='23514';
            END IF;
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
    IF NOT FOUND THEN RAISE EXCEPTION 'meeting settings unavailable' USING ERRCODE='23514'; END IF;
    -- Settings locks can block: take the effective clock only after acquiring them.
    db_now := clock_timestamp();
    IF TG_OP='INSERT' THEN NEW.created_at:=db_now;
    ELSIF OLD.expires_at<=db_now THEN RAISE EXCEPTION 'expired beacon is terminal' USING ERRCODE='23514'; END IF;
    IF NEW.expires_at<=db_now OR NEW.expires_at>db_now+make_interval(secs=>settings.beacon_ttl_seconds)
       OR NEW.expires_at>settings.event_cutoff THEN
        RAISE EXCEPTION 'invalid beacon expiry' USING ERRCODE='23514';
    END IF;
    NEW.updated_at := db_now;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_meeting_beacon_guard BEFORE INSERT OR UPDATE ON public.concert_meeting_beacons
FOR EACH ROW EXECUTE FUNCTION public.concert_guard_meeting_beacon();

-- Party-owned authority is queried through its port; no foreign party policy or
-- membership writes live here. Per-actor reports retain a membership-generation fence.
CREATE TABLE public.concert_meeting_rally_states (
    event_id uuid NOT NULL REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    party_id uuid NOT NULL,
    user_id uuid NOT NULL,
    state text NOT NULL CHECK(state IN ('ON_MY_WAY','HERE','MET','NEED_HELP','LEFT')),
    meeting_revision bigint NOT NULL CHECK(meeting_revision BETWEEN 1 AND 9007199254740991),
    membership_revision bigint NOT NULL CHECK(membership_revision BETWEEN 1 AND 9007199254740991),
    revision bigint NOT NULL DEFAULT 1 CHECK(revision BETWEEN 1 AND 9007199254740991),
    self_reported_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(event_id,party_id,user_id)
);
CREATE FUNCTION public.concert_guard_meeting_rally() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='INSERT' THEN
        IF NEW.revision<>1 THEN RAISE EXCEPTION 'invalid initial rally revision' USING ERRCODE='23514'; END IF;
    ELSE
        IF NEW.event_id<>OLD.event_id OR NEW.party_id<>OLD.party_id OR NEW.user_id<>OLD.user_id OR NEW.revision<>OLD.revision+1 THEN
            RAISE EXCEPTION 'invalid rally scope or revision' USING ERRCODE='23514';
        END IF;
    END IF;
    NEW.self_reported_at:=clock_timestamp();
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_meeting_rally_guard BEFORE INSERT OR UPDATE ON public.concert_meeting_rally_states
FOR EACH ROW EXECUTE FUNCTION public.concert_guard_meeting_rally();
