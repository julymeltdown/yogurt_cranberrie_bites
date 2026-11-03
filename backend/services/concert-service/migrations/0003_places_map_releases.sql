-- Immutable map bytes are global by map identity/revision. Event bindings and
-- current place state are independent and can change without rewriting bytes.
CREATE TABLE public.concert_map_releases (
    map_id text NOT NULL CONSTRAINT concert_map_id_shape CHECK (map_id ~ '^[a-z][a-z0-9_]{0,63}$'),
    map_revision bigint NOT NULL CONSTRAINT concert_map_revision_positive CHECK (map_revision BETWEEN 1 AND 9007199254740991),
    content_hash text NOT NULL CONSTRAINT concert_map_content_hash CHECK (content_hash ~ '^sha256:[0-9a-f]{64}$'),
    geometry_hash text NOT NULL CONSTRAINT concert_map_geometry_hash CHECK (geometry_hash ~ '^sha256:[0-9a-f]{64}$'),
    canonical_bytes bytea NOT NULL CONSTRAINT concert_map_bytes_bounds CHECK (octet_length(canonical_bytes) BETWEEN 1 AND 1048576),
    PRIMARY KEY(map_id,map_revision),
    UNIQUE(map_id,map_revision,content_hash)
);
CREATE TABLE public.concert_map_release_places (
    map_id text NOT NULL, map_revision bigint NOT NULL, place_id uuid NOT NULL,
    label text NOT NULL CONSTRAINT concert_place_label CHECK (char_length(label) BETWEEN 1 AND 512 AND label !~ '[[:cntrl:]]'),
    place_kind text NOT NULL CONSTRAINT concert_place_kind CHECK (place_kind IN ('VIRTUAL_CONCEPT','PHYSICAL')),
    verification_state text NOT NULL CONSTRAINT concert_place_verification CHECK (verification_state IN ('CONCEPT_ONLY','UNVERIFIED','VERIFIED')),
    accessibility_notes text NOT NULL CONSTRAINT concert_place_notes CHECK (char_length(accessibility_notes) BETWEEN 1 AND 512 AND accessibility_notes !~ '[[:cntrl:]]'),
    source_kind text NOT NULL CONSTRAINT concert_place_source_kind CHECK (source_kind IN ('ORIGINAL_FIXTURE','ORIGINAL_ART','PLACE_REFERENCE')),
    source_uri text,
    review_state text NOT NULL CONSTRAINT concert_place_review CHECK (review_state IN ('UNREVIEWED','REVIEWED','REJECTED')),
    provenance jsonb NOT NULL,
    created_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    PRIMARY KEY(map_id,map_revision,place_id),
    FOREIGN KEY(map_id,map_revision) REFERENCES public.concert_map_releases(map_id,map_revision),
    CONSTRAINT concert_place_kind_consistency CHECK (
        (place_kind='VIRTUAL_CONCEPT' AND verification_state='CONCEPT_ONLY')
        OR (place_kind='PHYSICAL' AND verification_state IN ('UNVERIFIED','VERIFIED')
            AND source_kind='PLACE_REFERENCE' AND source_uri IS NOT NULL)
    ),
    CONSTRAINT concert_place_verified_review CHECK (verification_state<>'VERIFIED' OR review_state='REVIEWED'),
    CONSTRAINT concert_place_provenance_fields CHECK (
        jsonb_typeof(provenance)='object'
        AND provenance ?& ARRAY['source_id','source_kind','source_revision','source_uri','author','license','review_state','reviewed_by','reviewed_at','photo_permission']
        AND provenance - ARRAY['source_id','source_kind','source_revision','source_uri','author','license','review_state','reviewed_by','reviewed_at','photo_permission']='{}'::jsonb
        AND provenance->>'source_kind'=source_kind
        AND ((source_uri IS NULL AND provenance->'source_uri'='null'::jsonb)
             OR (source_uri IS NOT NULL AND provenance->'source_uri'=to_jsonb(source_uri)))
        AND provenance->>'review_state'=review_state
        AND ((review_state='REVIEWED'
              AND jsonb_typeof(provenance->'reviewed_by')='string'
              AND jsonb_typeof(provenance->'reviewed_at')='string')
             OR (review_state<>'REVIEWED'
                 AND provenance->'reviewed_by'='null'::jsonb
                 AND provenance->'reviewed_at'='null'::jsonb))
        AND jsonb_typeof(provenance->'source_id')='string'
        AND provenance->>'source_id' ~ '^[a-z][a-z0-9_]{0,63}$'
        AND jsonb_typeof(provenance->'source_revision')='string'
        AND char_length(provenance->>'source_revision') BETWEEN 1 AND 512
        AND provenance->>'source_revision' !~ '[[:cntrl:]]'
        AND jsonb_typeof(provenance->'author')='string'
        AND char_length(provenance->>'author') BETWEEN 1 AND 512
        AND provenance->>'author' !~ '[[:cntrl:]]'
        AND jsonb_typeof(provenance->'license')='string'
        AND char_length(provenance->>'license') BETWEEN 1 AND 512
        AND provenance->>'license' !~ '[[:cntrl:]]'
        AND (provenance->'reviewed_by'='null'::jsonb
             OR (jsonb_typeof(provenance->'reviewed_by')='string'
                 AND char_length(provenance->>'reviewed_by') BETWEEN 1 AND 512
                 AND provenance->>'reviewed_by' !~ '[[:cntrl:]]'))
        AND (provenance->'reviewed_at'='null'::jsonb
             OR (jsonb_typeof(provenance->'reviewed_at')='string'
                 AND pg_input_is_valid(provenance->>'reviewed_at','timestamp with time zone')))
        AND provenance->>'photo_permission' IN ('NOT_APPLICABLE','UNREVIEWED','GRANTED','DENIED')
    )
);
CREATE TABLE public.concert_events_map_releases (
    event_id uuid PRIMARY KEY REFERENCES public.concert_events(event_id) ON DELETE CASCADE,
    catalog_revision bigint NOT NULL CONSTRAINT concert_catalog_revision CHECK (catalog_revision BETWEEN 1 AND 60000),
    map_id text NOT NULL, map_revision bigint NOT NULL,
    bound_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    CONSTRAINT concert_events_map_binding FOREIGN KEY(map_id,map_revision) REFERENCES public.concert_map_releases(map_id,map_revision),
    CONSTRAINT concert_events_map_bound_at CHECK (bound_at <= statement_timestamp())
);
CREATE INDEX concert_events_map_releases_release ON public.concert_events_map_releases(map_id,map_revision);
CREATE TABLE public.concert_place_current_state (
    map_id text NOT NULL, map_revision bigint NOT NULL, place_id uuid NOT NULL,
    closure_state text NOT NULL CONSTRAINT concert_place_closure CHECK (closure_state IN ('OPEN','CLOSED')),
    revision bigint NOT NULL DEFAULT 1 CONSTRAINT concert_place_state_revision CHECK (revision BETWEEN 1 AND 9007199254740991),
    updated_at timestamptz NOT NULL DEFAULT statement_timestamp(),
    PRIMARY KEY(map_id,map_revision,place_id),
    FOREIGN KEY(map_id,map_revision,place_id) REFERENCES public.concert_map_release_places(map_id,map_revision,place_id),
    CONSTRAINT concert_place_current_times CHECK (updated_at <= statement_timestamp())
);
CREATE INDEX concert_place_current_state_release ON public.concert_place_current_state(map_id,map_revision);
CREATE FUNCTION public.concert_guard_initial_event_map() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.catalog_revision<>1 OR NOT EXISTS(
        SELECT 1 FROM public.concert_map_release_places
        WHERE map_id=NEW.map_id AND map_revision=NEW.map_revision
    ) THEN
        RAISE EXCEPTION 'initial event map revision or release membership is invalid' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_event_map_initial
    BEFORE INSERT ON public.concert_events_map_releases
    FOR EACH ROW EXECUTE FUNCTION public.concert_guard_initial_event_map();
CREATE FUNCTION public.concert_guard_initial_current_place() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF NEW.revision<>1 THEN
        RAISE EXCEPTION 'initial place closure revision must be one' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_place_current_initial
    BEFORE INSERT ON public.concert_place_current_state
    FOR EACH ROW EXECUTE FUNCTION public.concert_guard_initial_current_place();
CREATE FUNCTION public.concert_guard_current_place_state() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN
        RAISE EXCEPTION 'current place state rows cannot be deleted' USING ERRCODE='23514';
    END IF;
    IF NEW.map_id<>OLD.map_id OR NEW.map_revision<>OLD.map_revision
       OR NEW.place_id<>OLD.place_id OR OLD.revision=9007199254740991
       OR NEW.revision<>OLD.revision+1
       OR NEW.updated_at<OLD.updated_at THEN
        RAISE EXCEPTION 'current place state revision or identity is invalid' USING ERRCODE='23514';
    END IF;
    NEW.updated_at := greatest(statement_timestamp(),OLD.updated_at);
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_place_current_revision
    BEFORE UPDATE OR DELETE ON public.concert_place_current_state
    FOR EACH ROW EXECUTE FUNCTION public.concert_guard_current_place_state();
CREATE FUNCTION public.concert_guard_event_map_revision() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF OLD.event_id<>NEW.event_id OR NEW.bound_at IS NULL
        OR NEW.catalog_revision IS NULL OR OLD.catalog_revision=60000
        OR NEW.catalog_revision<>OLD.catalog_revision+1
        OR NEW.bound_at<OLD.bound_at
        OR NOT EXISTS(
            SELECT 1 FROM public.concert_map_release_places
            WHERE map_id=NEW.map_id AND map_revision=NEW.map_revision
        ) THEN
        RAISE EXCEPTION 'event map binding revision must advance by one' USING ERRCODE='23514';
    END IF;
    NEW.bound_at := greatest(statement_timestamp(),OLD.bound_at);
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_event_map_revision
    BEFORE UPDATE ON public.concert_events_map_releases
    FOR EACH ROW EXECUTE FUNCTION public.concert_guard_event_map_revision();
-- Reject silent rebinding of an existing map revision to different bytes.
CREATE FUNCTION public.concert_guard_immutable_map() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP='DELETE' THEN
        RAISE EXCEPTION 'map releases are immutable' USING ERRCODE='23514';
    END IF;
    IF NEW.map_id<>OLD.map_id OR NEW.map_revision<>OLD.map_revision
       OR NEW.content_hash<>OLD.content_hash OR NEW.geometry_hash<>OLD.geometry_hash
       OR NEW.canonical_bytes<>OLD.canonical_bytes
       OR octet_length(NEW.canonical_bytes) NOT BETWEEN 1 AND 1048576 THEN
        RAISE EXCEPTION 'map release identity is immutable' USING ERRCODE='23514';
    END IF;
    RETURN NEW;
END;
$$;
CREATE TRIGGER concert_map_release_immutable BEFORE UPDATE OR DELETE ON public.concert_map_releases
    FOR EACH ROW EXECUTE FUNCTION public.concert_guard_immutable_map();

-- Published release membership is part of the immutable release. Closure and
-- event catalog bindings live in separate mutable tables.
CREATE FUNCTION public.concert_guard_immutable_release_place() RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    RAISE EXCEPTION 'map release places are immutable' USING ERRCODE='23514';
END;
$$;
CREATE TRIGGER concert_map_release_place_immutable
    BEFORE UPDATE OR DELETE ON public.concert_map_release_places
    FOR EACH ROW EXECUTE FUNCTION public.concert_guard_immutable_release_place();
