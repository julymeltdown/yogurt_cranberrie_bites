-- VENU-BE-L01: explicit location consent, selected viewers and one replaceable fix.
-- The plan referenced 0053; 0053..0072 are already occupied at the base commit,
-- so the next free version is used. No location history is stored.
CREATE TABLE public.concert_venue_location_sharing (
    event_id uuid NOT NULL,
    owner_id uuid NOT NULL,
    state text NOT NULL CONSTRAINT concert_venue_sharing_state CHECK (state IN ('ACTIVE','PAUSED','REVOKED')),
    revision bigint NOT NULL CONSTRAINT concert_venue_sharing_revision CHECK (revision BETWEEN 1 AND 9007199254740991),
    expires_at timestamptz,
    updated_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    CONSTRAINT concert_venue_sharing_key PRIMARY KEY(event_id,owner_id),
    CONSTRAINT concert_venue_sharing_owner FOREIGN KEY(event_id,owner_id)
        REFERENCES public.concert_participants(event_id,user_id) ON DELETE CASCADE,
    CONSTRAINT concert_venue_sharing_expiry CHECK (
        (state='ACTIVE' AND expires_at IS NOT NULL)
        OR (state IN ('PAUSED','REVOKED') AND expires_at IS NULL))
);
CREATE TABLE public.concert_venue_location_viewers (
    event_id uuid NOT NULL,
    owner_id uuid NOT NULL,
    viewer_id uuid NOT NULL,
    CONSTRAINT concert_venue_viewers_key PRIMARY KEY(event_id,owner_id,viewer_id),
    CONSTRAINT concert_venue_viewers_owner FOREIGN KEY(event_id,owner_id)
        REFERENCES public.concert_venue_location_sharing(event_id,owner_id) ON DELETE CASCADE,
    CONSTRAINT concert_venue_viewers_no_self CHECK (viewer_id<>owner_id)
);
CREATE INDEX concert_venue_viewers_selected
    ON public.concert_venue_location_viewers(event_id,viewer_id,owner_id);
CREATE TABLE public.concert_venue_location_fixes (
    event_id uuid NOT NULL,
    owner_id uuid NOT NULL,
    latitude double precision NOT NULL CONSTRAINT concert_venue_fix_latitude CHECK (latitude BETWEEN -90 AND 90),
    longitude double precision NOT NULL CONSTRAINT concert_venue_fix_longitude CHECK (longitude BETWEEN -180 AND 180),
    accuracy_m double precision NOT NULL CONSTRAINT concert_venue_fix_accuracy CHECK (accuracy_m>=0),
    captured_at timestamptz NOT NULL,
    received_at timestamptz NOT NULL DEFAULT clock_timestamp(),
    sharing_revision bigint NOT NULL CONSTRAINT concert_venue_fix_revision CHECK (sharing_revision BETWEEN 1 AND 9007199254740991),
    CONSTRAINT concert_venue_fix_key PRIMARY KEY(event_id,owner_id),
    CONSTRAINT concert_venue_fix_owner FOREIGN KEY(event_id,owner_id)
        REFERENCES public.concert_venue_location_sharing(event_id,owner_id) ON DELETE CASCADE
);
